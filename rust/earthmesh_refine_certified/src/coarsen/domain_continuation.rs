//! Deterministic best-preserving continuation across nested movable domains.

use super::geometry_witness::{angle_range, point_bits_equal};
use super::{
    embed_geometry_witness, solve_elastic_patch_with_active_trust_start_and_scale,
    ElasticBlockLimits, ElasticBlockOutcome, ElasticPatch, GeometryDomainId, GeometryDomainWitness,
    GeometryFailureWitness, GeometryStartId, HierarchyLeafMesh,
};
use std::{collections::BTreeSet, fmt::Write as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainContinuationMode {
    ColdStart,
    InheritedBestMonotone,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainContinuationSchedule {
    pub halo_only_iterations: usize,
    pub alternating_iterations: usize,
    pub joint_iterations: usize,
    pub initial_new_ring_trust_fraction: f64,
    pub final_new_ring_trust_fraction: f64,
}

impl DomainContinuationSchedule {
    pub const fn frozen_n6() -> Self {
        Self {
            halo_only_iterations: 16,
            alternating_iterations: 24,
            joint_iterations: 24,
            initial_new_ring_trust_fraction: 0.25,
            final_new_ring_trust_fraction: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationStageId {
    NewRingOnly,
    AlternatingNewRing,
    AlternatingOldOuter,
    AlternatingInterface,
    Joint,
}

impl ContinuationStageId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NewRingOnly => "NewRingOnly",
            Self::AlternatingNewRing => "AlternatingNewRing",
            Self::AlternatingOldOuter => "AlternatingOldOuter",
            Self::AlternatingInterface => "AlternatingInterface",
            Self::Joint => "Joint",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainContinuationStageReport {
    pub stage: ContinuationStageId,
    pub requested_iterations: usize,
    pub completed_iterations: usize,
    pub trust_fraction: f64,
    pub movable_vertices: usize,
    pub moved_compact_vertices: Vec<usize>,
    pub candidate_signed_margin_deg: f64,
    pub accepted_current: bool,
    pub improved_best: bool,
    pub outcome: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainContinuationResult {
    pub mode: DomainContinuationMode,
    pub schedule: DomainContinuationSchedule,
    pub source_domain: GeometryDomainId,
    pub target_domain: GeometryDomainId,
    pub initial_angle_range_deg: (f64, f64),
    pub best_angle_range_deg: (f64, f64),
    pub last_angle_range_deg: (f64, f64),
    pub initial_signed_margin_deg: f64,
    pub best_signed_margin_deg: f64,
    pub last_signed_margin_deg: f64,
    pub elastic_iterations: usize,
    pub stages: Vec<DomainContinuationStageReport>,
    pub best_witness: Box<GeometryFailureWitness>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DomainContinuationOutcome {
    Completed(Box<DomainContinuationResult>),
    Invalid { reason: String },
}

pub fn continue_nested_domain(
    inherited: &GeometryDomainWitness,
    target_patch: ElasticPatch,
    schedule: DomainContinuationSchedule,
    mode: DomainContinuationMode,
) -> DomainContinuationOutcome {
    if let Err(reason) = validate_schedule(schedule) {
        return DomainContinuationOutcome::Invalid { reason };
    }
    let (mut current, embedding) = match embed_geometry_witness(inherited, &target_patch) {
        Ok(value) => value,
        Err(error) => {
            return DomainContinuationOutcome::Invalid {
                reason: format!("nested witness embedding failed: {error:?}"),
            }
        }
    };
    if matches!(mode, DomainContinuationMode::ColdStart) {
        for &site in &target_patch.movable_compact_vertices {
            current
                .mesh
                .move_vertex(site, target_patch.reference_positions[site]);
        }
    } else if embedding.source_signed_margin_deg.to_bits()
        != embedding.embedded_signed_margin_deg.to_bits()
    {
        return DomainContinuationOutcome::Invalid {
            reason: "inherited embedding changed the source signed margin".into(),
        };
    }

    let initial_angle_range_deg = match angle_range(&current) {
        Ok(range) => range,
        Err(error) => {
            return DomainContinuationOutcome::Invalid {
                reason: format!("initial continuation angle scan failed: {error:?}"),
            }
        }
    };
    let initial_signed_margin_deg = signed_margin(initial_angle_range_deg);
    let mut best = current.clone();
    let mut best_angle_range_deg = initial_angle_range_deg;
    let mut best_signed_margin_deg = initial_signed_margin_deg;
    let mut current_signed_margin_deg = initial_signed_margin_deg;
    let mut last_angle_range_deg = initial_angle_range_deg;
    let mut last_signed_margin_deg = initial_signed_margin_deg;
    let mut stages = Vec::new();
    let mut elastic_iterations = 0;

    let source_movable = inherited
        .patch()
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let target_movable = target_patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let new_ring = target_movable
        .difference(&source_movable)
        .copied()
        .collect::<BTreeSet<_>>();
    if new_ring.is_empty() {
        return DomainContinuationOutcome::Invalid {
            reason: "nested continuation released no new vertices".into(),
        };
    }
    let old_outer = adjacent_subset(&current, &source_movable, &new_ring);
    let interface = source_movable
        .difference(&old_outer)
        .copied()
        .collect::<BTreeSet<_>>();

    let mut run_stage = |stage: ContinuationStageId,
                         requested_iterations: usize,
                         trust_fraction: f64,
                         movable: &BTreeSet<usize>|
     -> Result<bool, String> {
        if requested_iterations == 0 || movable.is_empty() {
            return Ok(false);
        }
        let stage_patch = target_patch
            .restricted_to_movable(&current, movable)
            .map_err(|reason| format!("{} patch failed: {reason}", stage.as_str()))?;
        let before = current.clone();
        let outcome = solve_elastic_patch_with_active_trust_start_and_scale(
            &before,
            stage_patch,
            ElasticBlockLimits {
                elastic_iterations: requested_iterations,
            },
            GeometryStartId::MaterializedSource,
            trust_fraction,
        );
        let (candidate, completed_iterations, outcome_name) = outcome_candidate(outcome)?;
        let moved_compact_vertices = changed_vertices(&before, &candidate);
        if moved_compact_vertices
            .iter()
            .any(|site| !movable.contains(site))
        {
            return Err(format!("{} moved a fixed block vertex", stage.as_str()));
        }
        let candidate_angle_range = angle_range(&candidate)
            .map_err(|error| format!("{} angle scan failed: {error:?}", stage.as_str()))?;
        let candidate_margin = signed_margin(candidate_angle_range);
        let accepted_current = margin_is_not_worse(candidate_margin, current_signed_margin_deg);
        let improved_best = candidate_margin > best_signed_margin_deg + 1.0e-12;
        last_angle_range_deg = candidate_angle_range;
        last_signed_margin_deg = candidate_margin;
        elastic_iterations += completed_iterations;
        if accepted_current {
            current = candidate;
            current_signed_margin_deg = candidate_margin;
            if improved_best {
                best = current.clone();
                best_angle_range_deg = candidate_angle_range;
                best_signed_margin_deg = candidate_margin;
            }
        } else {
            restore_exact_best(&mut current, &best);
            current_signed_margin_deg = best_signed_margin_deg;
        }
        stages.push(DomainContinuationStageReport {
            stage,
            requested_iterations,
            completed_iterations,
            trust_fraction,
            movable_vertices: movable.len(),
            moved_compact_vertices,
            candidate_signed_margin_deg: candidate_margin,
            accepted_current,
            improved_best,
            outcome: outcome_name,
        });
        Ok(best_signed_margin_deg >= 0.0)
    };

    if let Err(reason) = run_stage(
        ContinuationStageId::NewRingOnly,
        schedule.halo_only_iterations,
        schedule.initial_new_ring_trust_fraction,
        &new_ring,
    ) {
        return DomainContinuationOutcome::Invalid { reason };
    }

    let alternating = [
        (ContinuationStageId::AlternatingNewRing, &new_ring),
        (ContinuationStageId::AlternatingOldOuter, &old_outer),
        (ContinuationStageId::AlternatingInterface, &interface),
    ]
    .into_iter()
    .filter(|(_, block)| !block.is_empty())
    .collect::<Vec<_>>();
    for iteration in 0..schedule.alternating_iterations {
        let (stage, block) = alternating[iteration % alternating.len()];
        let progress = (iteration + 1) as f64 / schedule.alternating_iterations.max(1) as f64;
        let trust_fraction = schedule.initial_new_ring_trust_fraction
            + progress
                * (schedule.final_new_ring_trust_fraction
                    - schedule.initial_new_ring_trust_fraction);
        match run_stage(stage, 1, trust_fraction, block) {
            Ok(true) => break,
            Ok(false) => {}
            Err(reason) => return DomainContinuationOutcome::Invalid { reason },
        }
    }

    if let Err(reason) = run_stage(
        ContinuationStageId::Joint,
        schedule.joint_iterations,
        schedule.final_new_ring_trust_fraction,
        &target_movable,
    ) {
        return DomainContinuationOutcome::Invalid { reason };
    }

    DomainContinuationOutcome::Completed(Box::new(DomainContinuationResult {
        mode,
        schedule,
        source_domain: inherited.domain_id,
        target_domain: target_patch.domain_id,
        initial_angle_range_deg,
        best_angle_range_deg,
        last_angle_range_deg,
        initial_signed_margin_deg,
        best_signed_margin_deg,
        last_signed_margin_deg,
        elastic_iterations,
        stages,
        best_witness: Box::new(GeometryFailureWitness {
            mesh: best,
            patch: target_patch,
        }),
    }))
}

pub fn domain_continuation_evidence_json(
    result: &DomainContinuationResult,
    commit_sha: Option<&str>,
) -> String {
    let stages = result
        .stages
        .iter()
        .map(|stage| {
            format!(
                "{{\"stage\":\"{}\",\"requested_iterations\":{},\"completed_iterations\":{},\"trust_fraction\":{:.12},\"movable_vertices\":{},\"moved_vertices\":{},\"candidate_signed_margin_deg\":{:.12},\"accepted_current\":{},\"improved_best\":{},\"outcome\":\"{}\"}}",
                stage.stage.as_str(),
                stage.requested_iterations,
                stage.completed_iterations,
                stage.trust_fraction,
                stage.movable_vertices,
                stage.moved_compact_vertices.len(),
                stage.candidate_signed_margin_deg,
                stage.accepted_current,
                stage.improved_best,
                stage.outcome,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut json = String::new();
    write!(
        json,
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr52MonotoneContinuation\",\"commit_sha\":{},\"source_domain\":\"{}\",\"target_domain\":\"{}\",\"initial_angle_min_deg\":{:.12},\"initial_angle_max_deg\":{:.12},\"best_angle_min_deg\":{:.12},\"best_angle_max_deg\":{:.12},\"last_angle_min_deg\":{:.12},\"last_angle_max_deg\":{:.12},\"initial_signed_margin_deg\":{:.12},\"best_signed_margin_deg\":{:.12},\"last_signed_margin_deg\":{:.12},\"improvement_deg\":{:.12},\"elastic_iterations\":{},\"stages\":[{}]}}",
        commit_sha.map_or_else(|| "null".into(), |sha| format!("\"{sha}\"")),
        result.source_domain.as_str(),
        result.target_domain.as_str(),
        result.initial_angle_range_deg.0,
        result.initial_angle_range_deg.1,
        result.best_angle_range_deg.0,
        result.best_angle_range_deg.1,
        result.last_angle_range_deg.0,
        result.last_angle_range_deg.1,
        result.initial_signed_margin_deg,
        result.best_signed_margin_deg,
        result.last_signed_margin_deg,
        result.best_signed_margin_deg - result.initial_signed_margin_deg,
        result.elastic_iterations,
        stages,
    )
    .unwrap();
    json
}

fn validate_schedule(schedule: DomainContinuationSchedule) -> Result<(), String> {
    for (label, fraction) in [
        (
            "initial new-ring trust fraction",
            schedule.initial_new_ring_trust_fraction,
        ),
        (
            "final new-ring trust fraction",
            schedule.final_new_ring_trust_fraction,
        ),
    ] {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) || fraction == 0.0 {
            return Err(format!("{label} must be finite and in (0, 1]"));
        }
    }
    if schedule.initial_new_ring_trust_fraction > schedule.final_new_ring_trust_fraction {
        return Err("initial trust fraction exceeds final trust fraction".into());
    }
    Ok(())
}

fn adjacent_subset(
    mesh: &HierarchyLeafMesh,
    candidates: &BTreeSet<usize>,
    neighbours: &BTreeSet<usize>,
) -> BTreeSet<usize> {
    let mut adjacent = BTreeSet::new();
    for face in mesh.mesh.active_triangle_slots() {
        let [a, b, c] = mesh.mesh.triangles()[face];
        for (left, right) in [(a, b), (b, c), (c, a), (b, a), (c, b), (a, c)] {
            if candidates.contains(&left) && neighbours.contains(&right) {
                adjacent.insert(left);
            }
        }
    }
    adjacent
}

fn outcome_candidate(
    outcome: ElasticBlockOutcome,
) -> Result<(HierarchyLeafMesh, usize, &'static str), String> {
    match outcome {
        ElasticBlockOutcome::Certified(trial) => {
            Ok((trial.mesh, trial.report.elastic_iterations, "Certified"))
        }
        ElasticBlockOutcome::ElasticNoImprovement {
            elastic_iterations,
            witness,
            ..
        } => Ok((witness.mesh, elastic_iterations, "ElasticNoImprovement")),
        ElasticBlockOutcome::SearchBudgetExhausted {
            elastic_iterations,
            witness,
            ..
        } => Ok((witness.mesh, elastic_iterations, "SearchBudgetExhausted")),
        ElasticBlockOutcome::RequiresDifferentTopology {
            elastic_iterations,
            witness,
            ..
        } => Ok((
            witness.mesh,
            elastic_iterations,
            "RequiresDifferentTopology",
        )),
        ElasticBlockOutcome::InvalidPatch { reason } => Err(reason),
    }
}

fn changed_vertices(before: &HierarchyLeafMesh, after: &HierarchyLeafMesh) -> Vec<usize> {
    before
        .mesh
        .vertices()
        .iter()
        .copied()
        .zip(after.mesh.vertices().iter().copied())
        .enumerate()
        .filter_map(|(site, (left, right))| (!point_bits_equal(left, right)).then_some(site))
        .collect()
}

fn signed_margin(range: (f64, f64)) -> f64 {
    (range.0 - 40.2).min(79.8 - range.1)
}

fn margin_is_not_worse(candidate: f64, current: f64) -> bool {
    candidate + 1.0e-12 >= current
}

fn restore_exact_best(current: &mut HierarchyLeafMesh, best: &HierarchyLeafMesh) {
    current.clone_from(best);
}

#[cfg(test)]
mod tests {
    use super::super::geometry_witness::tests::fixture;
    use super::*;

    fn result(schedule: DomainContinuationSchedule) -> DomainContinuationResult {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let DomainContinuationOutcome::Completed(result) = continue_nested_domain(
            &witness,
            target,
            schedule,
            DomainContinuationMode::InheritedBestMonotone,
        ) else {
            panic!("synthetic nested continuation must complete");
        };
        *result
    }

    #[test]
    fn plus_two_best_never_worse_than_inherited_plus_one() {
        let result = result(DomainContinuationSchedule {
            halo_only_iterations: 1,
            alternating_iterations: 3,
            joint_iterations: 1,
            initial_new_ring_trust_fraction: 0.25,
            final_new_ring_trust_fraction: 1.0,
        });
        assert!(result.best_signed_margin_deg >= result.initial_signed_margin_deg);
    }

    #[test]
    fn rejected_step_restores_exact_best_coordinates() {
        let (_, witness, _, _) = fixture(&BTreeSet::new());
        let best = witness.mesh().clone();
        let mut current = best.clone();
        let site = witness.patch().movable_compact_vertices[0];
        current
            .mesh
            .move_vertex(site, earthmesh_mesh::CartesianPoint::new(0.25, 0.5, 0.75));
        assert!(!margin_is_not_worse(-2.0, -1.0));
        restore_exact_best(&mut current, &best);
        assert!(current
            .mesh
            .vertices()
            .iter()
            .copied()
            .zip(best.mesh.vertices().iter().copied())
            .all(|(left, right)| point_bits_equal(left, right)));
    }

    #[test]
    fn new_ring_only_stage_keeps_old_ring_fixed() {
        let (_, witness, target, _) = fixture(&BTreeSet::new());
        let old = witness
            .patch()
            .movable_compact_vertices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let new = target
            .movable_compact_vertices
            .iter()
            .copied()
            .filter(|site| !old.contains(site))
            .collect::<BTreeSet<_>>();
        let DomainContinuationOutcome::Completed(result) = continue_nested_domain(
            &witness,
            target,
            DomainContinuationSchedule {
                halo_only_iterations: 1,
                alternating_iterations: 0,
                joint_iterations: 0,
                initial_new_ring_trust_fraction: 0.25,
                final_new_ring_trust_fraction: 1.0,
            },
            DomainContinuationMode::InheritedBestMonotone,
        ) else {
            panic!("new-ring continuation must complete");
        };
        assert_eq!(result.stages.len(), 1);
        assert_eq!(result.stages[0].movable_vertices, new.len());
        assert!(result.stages[0]
            .moved_compact_vertices
            .iter()
            .all(|site| new.contains(site) && !old.contains(site)));
    }

    #[test]
    fn block_schedule_is_deterministic() {
        let schedule = DomainContinuationSchedule {
            halo_only_iterations: 1,
            alternating_iterations: 6,
            joint_iterations: 1,
            initial_new_ring_trust_fraction: 0.25,
            final_new_ring_trust_fraction: 1.0,
        };
        assert_eq!(result(schedule).stages, result(schedule).stages);
    }

    #[test]
    fn same_input_produces_same_json() {
        let schedule = DomainContinuationSchedule {
            halo_only_iterations: 1,
            alternating_iterations: 3,
            joint_iterations: 1,
            initial_new_ring_trust_fraction: 0.25,
            final_new_ring_trust_fraction: 1.0,
        };
        let first = result(schedule);
        let second = result(schedule);
        assert_eq!(
            domain_continuation_evidence_json(&first, Some("test")),
            domain_continuation_evidence_json(&second, Some("test"))
        );
    }
}
