use super::{
    angle_atlas::validate_spatial_context,
    component_transaction::solve_component_transaction_at_level,
    plan_hierarchy_components_from_parent_requirements, ComponentRollbackReport,
    ComponentTransactionLimits, ComponentTransactionOutcome, ComponentTransactionState,
    DomainQualityRejectReason, ExplicitParentRequirement, HierarchyComponent, SpatialFaceContext,
};
use crate::{
    certificate::{AngleContractId, Certificate},
    fingerprint::mesh_fingerprint,
    mother_grid::{mother_cell_count, MotherGrid, TriangleAddress},
    outcome::FinalCertificationEvidence,
    remap::ConservativeRemap,
    requirement::{certify_final_cell_requirements_with_remap, SourceLevelField},
};
use earthmesh_mesh::MeshState;
use earthmesh_quality::domain::QualityZone;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    time::Instant,
};

type ComponentQualityGate<'a> = dyn FnMut(&HierarchyComponent, &ComponentTransactionState) -> Option<DomainQualityRejectReason>
    + 'a;

#[derive(Debug, Clone, PartialEq)]
pub struct CoarseningPriority {
    pub maximum_quality_priority: f64,
    pub mean_quality_priority: f64,
    pub minimum_distance_to_target: f64,
    /// Transition descendants in the target or its protected boundary band.
    pub transition_target_overlap: usize,
    pub compression_gain: usize,
    pub estimated_quality_risk: f64,
    pub stable_component_key: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoarseningScheduleStats {
    pub components_considered: usize,
    pub components_committed: usize,
    pub components_rejected_for_global_hard: usize,
    pub components_rejected_for_target_quality: usize,
}

impl CoarseningScheduleStats {
    fn consider(&mut self) {
        self.components_considered = self.components_considered.saturating_add(1);
    }

    fn commit(&mut self) {
        self.components_committed = self.components_committed.saturating_add(1);
    }

    fn reject(&mut self, rejection: DomainQualityRejectReason) {
        match rejection {
            DomainQualityRejectReason::HardCertificateFailed
            | DomainQualityRejectReason::InvalidScore
            | DomainQualityRejectReason::GlobalHardViolation
            | DomainQualityRejectReason::RequirementResidual
            | DomainQualityRejectReason::TopologyResidual
            | DomainQualityRejectReason::DualResidual
            | DomainQualityRejectReason::RemapResidual => {
                self.components_rejected_for_global_hard =
                    self.components_rejected_for_global_hard.saturating_add(1);
            }
            DomainQualityRejectReason::ExportDamage(_)
            | DomainQualityRejectReason::NotStrictImprovement => {
                self.components_rejected_for_target_quality = self
                    .components_rejected_for_target_quality
                    .saturating_add(1);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElasticCmrcConfig {
    pub angle_contract: AngleContractId,
    pub max_level: usize,
    pub max_adjacent_level_delta: usize,
    pub initial_transition_rings: usize,
    pub maximum_transition_rings: usize,
    pub topology_states_per_component: usize,
    pub elastic_iterations_per_topology: usize,
    pub interval_boxes_per_component: usize,
    pub total_transition_states: usize,
    pub allow_safe_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentOutcomeKind {
    Certified,
    RejectedForQuality,
    NoTopology,
    ElasticNoImprovement,
    SearchBudgetExhausted,
    RequiresWiderHalo,
    NotCertifiable,
    PromotedToFine,
}

impl ComponentOutcomeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certified => "certified",
            Self::RejectedForQuality => "rejected_for_quality",
            Self::NoTopology => "no_topology",
            Self::ElasticNoImprovement => "elastic_no_improvement",
            Self::SearchBudgetExhausted => "search_budget_exhausted",
            Self::RequiresWiderHalo => "requires_wider_halo",
            Self::NotCertifiable => "not_certifiable",
            Self::PromotedToFine => "promoted_to_fine",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticComponentRecord {
    pub component_id: u64,
    pub source_level: usize,
    pub target_level: usize,
    pub parent_count: usize,
    pub core_parent_count: usize,
    pub transition_parent_count: usize,
    pub core_vertices_removed: usize,
    pub topology_states: usize,
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
    pub transition_ring_width: usize,
    pub outcome: ComponentOutcomeKind,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticLevelReport {
    pub source_level: usize,
    pub target_level: usize,
    pub components_total: usize,
    pub components_committed: usize,
    pub components_promoted: usize,
    pub components_exhausted: usize,
    pub components_rejected_for_global_hard: usize,
    pub components_rejected_for_target_quality: usize,
    pub delivered_histogram: BTreeMap<usize, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticCmrcReport {
    pub initial_faces: usize,
    pub final_faces: usize,
    pub initial_vertices: usize,
    pub final_vertices: usize,
    pub requested_histogram: BTreeMap<usize, usize>,
    pub delivered_histogram: BTreeMap<usize, usize>,
    pub components_total: usize,
    pub components_committed: usize,
    pub components_promoted: usize,
    pub components_exhausted: usize,
    pub components_rejected_for_global_hard: usize,
    pub components_rejected_for_target_quality: usize,
    pub total_topology_states: usize,
    pub total_elastic_iterations: usize,
    pub total_interval_boxes: usize,
    pub core_vertices_removed: usize,
    pub search_complete: bool,
    pub levels: Vec<ElasticLevelReport>,
    pub components: Vec<ElasticComponentRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticCmrcResult {
    pub state: ComponentTransactionState,
    pub report: ElasticCmrcReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElasticCmrcOutcome {
    Completed(Box<ElasticCmrcResult>),
    NotCertifiable { reason: String },
    InvalidInput { reason: String },
}

/// Reorder an existing component family from low-priority exterior work toward
/// the protected target. The component set is unchanged.
pub fn sort_components_outside_in(
    source: &MotherGrid,
    components: &mut [HierarchyComponent],
    face_context: &BTreeMap<usize, SpatialFaceContext>,
) -> Result<Vec<CoarseningPriority>, String> {
    if components.is_empty() {
        return Ok(Vec::new());
    }
    let coarse_n = components[0]
        .parents
        .first()
        .ok_or_else(|| "outside-in component has no parents".to_string())?
        .n;
    let mut owners = BTreeMap::new();
    let mut transition_parents = Vec::with_capacity(components.len());
    for (component_index, component) in components.iter().enumerate() {
        if component.parents.is_empty()
            || component.parents.iter().any(|parent| parent.n != coarse_n)
        {
            return Err("outside-in components must have non-empty parents at one level".into());
        }
        for &parent in &component.parents {
            if owners.insert(parent, component_index).is_some() {
                return Err(format!("outside-in parent {parent:?} has multiple owners"));
            }
        }
        let transitions = component
            .transition_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if transitions
            .iter()
            .any(|parent| parent.n != coarse_n || !component.parents.contains(parent))
        {
            return Err(format!(
                "outside-in component {:?} has an invalid transition parent",
                component.parents
            ));
        }
        transition_parents.push(transitions);
    }

    #[derive(Clone, Copy)]
    struct Aggregate {
        maximum: f64,
        sum: f64,
        samples: usize,
        minimum_distance: f64,
        protected_transition_overlap: usize,
    }
    let mut aggregates = vec![
        Aggregate {
            maximum: 0.0,
            sum: 0.0,
            samples: 0,
            minimum_distance: f64::INFINITY,
            protected_transition_overlap: 0,
        };
        components.len()
    ];
    for face in source.mesh.active_triangle_slots() {
        let context = face_context
            .get(&face)
            .ok_or_else(|| format!("outside-in scheduler is missing context for face {face}"))?;
        validate_spatial_context(face, context)?;
        let mut parent = source
            .triangle_addresses
            .get(face)
            .copied()
            .flatten()
            .ok_or_else(|| format!("outside-in source face {face} has no hierarchy address"))?;
        while parent.n > coarse_n {
            parent = parent.parent_2_to_1().ok_or_else(|| {
                format!("outside-in face {face} cannot reach parent level {coarse_n}")
            })?;
        }
        if parent.n != coarse_n {
            return Err(format!(
                "outside-in face {face} subdivision {} is below component subdivision {coarse_n}",
                parent.n
            ));
        }
        let Some(&component_index) = owners.get(&parent) else {
            continue;
        };
        let aggregate = &mut aggregates[component_index];
        aggregate.maximum = aggregate.maximum.max(context.quality.maximum_priority);
        aggregate.sum += context.quality.mean_priority;
        aggregate.samples += 1;
        aggregate.minimum_distance = aggregate
            .minimum_distance
            .min(context.quality.minimum_distance_to_target);
        if transition_parents[component_index].contains(&parent)
            && matches!(
                context.quality.zone,
                QualityZone::TargetCore | QualityZone::BoundaryProtection
            )
        {
            aggregate.protected_transition_overlap += 1;
        }
    }

    let mut ranked = components
        .iter()
        .cloned()
        .zip(aggregates)
        .map(|(component, aggregate)| {
            if aggregate.samples == 0 {
                return Err(format!(
                    "outside-in component {:?} has no source descendants",
                    component.parents
                ));
            }
            let mean = aggregate.sum / aggregate.samples as f64;
            let priority = CoarseningPriority {
                maximum_quality_priority: aggregate.maximum,
                mean_quality_priority: mean,
                minimum_distance_to_target: aggregate.minimum_distance,
                transition_target_overlap: aggregate.protected_transition_overlap,
                compression_gain: component.parents.len().saturating_mul(3),
                estimated_quality_risk: mean,
                stable_component_key: stable_component_key(&component),
            };
            Ok((component, priority))
        })
        .collect::<Result<Vec<_>, String>>()?;
    ranked.sort_by(|left, right| compare_coarsening_priority(&left.1, &right.1));
    for (slot, (component, _)) in components.iter_mut().zip(&ranked) {
        *slot = component.clone();
    }
    Ok(ranked.into_iter().map(|(_, priority)| priority).collect())
}

fn compare_coarsening_priority(
    left: &CoarseningPriority,
    right: &CoarseningPriority,
) -> std::cmp::Ordering {
    left.maximum_quality_priority
        .total_cmp(&right.maximum_quality_priority)
        .then_with(|| {
            right
                .minimum_distance_to_target
                .total_cmp(&left.minimum_distance_to_target)
        })
        .then_with(|| {
            left.transition_target_overlap
                .cmp(&right.transition_target_overlap)
        })
        .then_with(|| right.compression_gain.cmp(&left.compression_gain))
        .then_with(|| {
            left.estimated_quality_risk
                .total_cmp(&right.estimated_quality_risk)
        })
        .then_with(|| left.stable_component_key.cmp(&right.stable_component_key))
}

fn stable_component_key(component: &HierarchyComponent) -> String {
    let mut key = String::new();
    for parent in &component.parents {
        let orientation = match parent.orientation {
            crate::mother_grid::TriangleOrientation::Up => 'u',
            crate::mother_grid::TriangleOrientation::Down => 'd',
        };
        write!(
            key,
            "{:02}/{}/{}/{}/{};",
            parent.base_face, parent.n, parent.i, parent.j, orientation
        )
        .expect("writing to String cannot fail");
    }
    key
}

pub fn run_elastic_component_epochs(
    grid: MotherGrid,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    required_levels: &[usize],
    config: &ElasticCmrcConfig,
) -> ElasticCmrcOutcome {
    run_elastic_component_epochs_impl(
        grid,
        source_mesh,
        source_levels,
        required_levels,
        None,
        None,
        config,
    )
}

pub fn run_elastic_component_epochs_with_quality_context<F>(
    grid: MotherGrid,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    required_levels: &[usize],
    face_context: &BTreeMap<usize, SpatialFaceContext>,
    mut quality_gate: F,
    config: &ElasticCmrcConfig,
) -> ElasticCmrcOutcome
where
    F: FnMut(&HierarchyComponent, &ComponentTransactionState) -> Option<DomainQualityRejectReason>,
{
    run_elastic_component_epochs_impl(
        grid,
        source_mesh,
        source_levels,
        required_levels,
        Some(face_context),
        Some(&mut quality_gate),
        config,
    )
}

fn run_elastic_component_epochs_impl(
    grid: MotherGrid,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    required_levels: &[usize],
    face_context: Option<&BTreeMap<usize, SpatialFaceContext>>,
    mut quality_gate: Option<&mut ComponentQualityGate<'_>>,
    config: &ElasticCmrcConfig,
) -> ElasticCmrcOutcome {
    if let Err(reason) = validate_inputs(&grid, source_mesh, source_levels, required_levels, config)
    {
        return ElasticCmrcOutcome::InvalidInput { reason };
    }
    let mut state = match ComponentTransactionState::new(&grid, config.max_level) {
        Ok(state) => state,
        Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
    };
    let initial_faces = grid.mesh.triangle_count();
    let initial_vertices = grid.mesh.vertex_count();
    let mut remaining_topology_states = config.total_transition_states;
    let mut next_component_id = 0u64;
    let source_active_sites = grid.mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut report = ElasticCmrcReport {
        initial_faces,
        final_faces: initial_faces,
        initial_vertices,
        final_vertices: initial_vertices,
        requested_histogram: histogram(
            grid.mesh
                .active_vertex_slots()
                .map(|site| required_levels[site]),
        ),
        delivered_histogram: histogram(state.target_levels().unwrap().levels().iter().copied()),
        components_total: 0,
        components_committed: 0,
        components_promoted: 0,
        components_exhausted: 0,
        components_rejected_for_global_hard: 0,
        components_rejected_for_target_quality: 0,
        total_topology_states: 0,
        total_elastic_iterations: 0,
        total_interval_boxes: 0,
        core_vertices_removed: 0,
        search_complete: true,
        levels: Vec::new(),
        components: Vec::new(),
    };

    for source_level in (1..=config.max_level).rev() {
        let timing_enabled = std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1");
        let level_started = Instant::now();
        let target_level = source_level - 1;
        let shift = config.max_level - source_level;
        let fine_n = grid.subdivision >> shift;
        let owned_level_grid;
        let level_grid = if fine_n == grid.subdivision {
            &grid
        } else {
            owned_level_grid = match MotherGrid::generate(fine_n) {
                Ok(grid) => grid,
                Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
            };
            &owned_level_grid
        };
        let level_source_slots = match state.level_source_slots(&grid, level_grid) {
            Ok(slots) => slots,
            Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
        };
        let requirements =
            match explicit_parent_requirements(&grid, &state, level_grid, required_levels) {
                Ok(requirements) => requirements,
                Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
            };
        let mut plan = match plan_hierarchy_components_from_parent_requirements(
            level_grid,
            &requirements,
            target_level,
            config.initial_transition_rings,
        ) {
            Ok(plan) => plan,
            Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
        };
        if let Some(face_context) = face_context {
            if let Err(reason) =
                sort_components_outside_in(&grid, &mut plan.components, face_context)
            {
                return ElasticCmrcOutcome::InvalidInput { reason };
            }
        } else {
            sort_components(
                &mut plan.components,
                &plan.parent_requirements,
                target_level,
            );
        }
        let components_total = plan.components.len();
        let mut transition_components_remaining = plan
            .components
            .iter()
            .filter(|component| !component.transition_parents.is_empty())
            .count();
        let mut committed = 0usize;
        let mut promoted = 0usize;
        let mut exhausted = 0usize;
        let mut certified_state_fingerprint = None;
        let mut quality_stats = CoarseningScheduleStats::default();
        let planning_elapsed = level_started.elapsed();
        let components_started = Instant::now();

        for mut component in plan.components {
            component.id = next_component_id;
            next_component_id = next_component_id.saturating_add(1);
            if quality_gate.is_some() {
                quality_stats.consider();
            }
            let topology_states = if component.transition_parents.is_empty() {
                0
            } else {
                let fair_share =
                    remaining_topology_states.div_ceil(transition_components_remaining);
                transition_components_remaining -= 1;
                config.topology_states_per_component.min(fair_share)
            };
            let limits = ComponentTransactionLimits {
                topology_states,
                elastic_iterations: config.elastic_iterations_per_topology,
                interval_boxes: config.interval_boxes_per_component,
                halo_expansions: config
                    .maximum_transition_rings
                    .saturating_sub(config.initial_transition_rings),
            };
            let before_quality_state = quality_gate.is_some().then(|| state.clone());
            let outcome = solve_component_transaction_at_level(
                &grid,
                source_levels,
                &mut state,
                level_grid,
                &source_active_sites,
                &level_source_slots,
                &component,
                target_level,
                config.max_adjacent_level_delta,
                limits,
                config.angle_contract,
            );
            let component_record = match outcome {
                ComponentTransactionOutcome::Certified(commit) => {
                    let rejection = quality_gate
                        .as_mut()
                        .and_then(|gate| gate(&component, &state));
                    if let Some(rejection) = rejection {
                        state = before_quality_state.expect("quality gate snapshots the state");
                        quality_stats.reject(rejection);
                        ElasticComponentRecord {
                            component_id: component.id,
                            source_level,
                            target_level,
                            parent_count: component.parents.len(),
                            core_parent_count: component.core_parents.len(),
                            transition_parent_count: component.transition_parents.len(),
                            core_vertices_removed: 0,
                            topology_states: commit.topology_states,
                            elastic_iterations: commit.elastic_iterations,
                            interval_boxes: commit.interval_boxes,
                            transition_ring_width: config.initial_transition_rings
                                + commit.halo_expansions,
                            outcome: ComponentOutcomeKind::RejectedForQuality,
                            reason: Some(format!("{rejection:?}")),
                        }
                    } else {
                        if quality_gate.is_some() {
                            quality_stats.commit();
                        }
                        committed += 1;
                        report.components_committed += 1;
                        report.core_vertices_removed += commit.core_vertices_removed;
                        report.total_topology_states += commit.topology_states;
                        report.total_elastic_iterations += commit.elastic_iterations;
                        report.total_interval_boxes += commit.interval_boxes;
                        certified_state_fingerprint = Some(commit.after_fingerprint);
                        remaining_topology_states =
                            remaining_topology_states.saturating_sub(commit.topology_states);
                        ElasticComponentRecord {
                            component_id: component.id,
                            source_level,
                            target_level,
                            parent_count: component.parents.len(),
                            core_parent_count: component.core_parents.len(),
                            transition_parent_count: component.transition_parents.len(),
                            core_vertices_removed: commit.core_vertices_removed,
                            topology_states: commit.topology_states,
                            elastic_iterations: commit.elastic_iterations,
                            interval_boxes: commit.interval_boxes,
                            transition_ring_width: config.initial_transition_rings
                                + commit.halo_expansions,
                            outcome: ComponentOutcomeKind::Certified,
                            reason: None,
                        }
                    }
                }
                ComponentTransactionOutcome::InvalidInput(rollback) => {
                    return ElasticCmrcOutcome::InvalidInput {
                        reason: format!(
                            "component {} at {source_level}->{target_level}: {}",
                            component.id, rollback.reason
                        ),
                    };
                }
                ComponentTransactionOutcome::NoTopology(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::NoTopology,
                    rollback,
                ),
                ComponentTransactionOutcome::ElasticNoImprovement(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::ElasticNoImprovement,
                    rollback,
                ),
                ComponentTransactionOutcome::SearchBudgetExhausted(rollback) => {
                    exhausted += 1;
                    report.components_exhausted += 1;
                    report.search_complete = false;
                    rollback_record(
                        &component,
                        source_level,
                        target_level,
                        config.initial_transition_rings,
                        ComponentOutcomeKind::SearchBudgetExhausted,
                        rollback,
                    )
                }
                ComponentTransactionOutcome::RequiresWiderHalo(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::RequiresWiderHalo,
                    rollback,
                ),
                ComponentTransactionOutcome::NotCertifiable(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::NotCertifiable,
                    rollback,
                ),
            };
            if component_record.outcome != ComponentOutcomeKind::Certified {
                promoted += 1;
                report.components_promoted += 1;
                report.total_topology_states += component_record.topology_states;
                report.total_elastic_iterations += component_record.elastic_iterations;
                report.total_interval_boxes += component_record.interval_boxes;
                remaining_topology_states =
                    remaining_topology_states.saturating_sub(component_record.topology_states);
            }
            report.components.push(component_record);
        }
        let components_elapsed = components_started.elapsed();
        let certification_started = Instant::now();
        report.components_total += components_total;
        report.components_rejected_for_global_hard +=
            quality_stats.components_rejected_for_global_hard;
        report.components_rejected_for_target_quality +=
            quality_stats.components_rejected_for_target_quality;
        let reused_component_certificate = certified_state_fingerprint == Some(state.fingerprint());
        if !reused_component_certificate {
            if let Err(reason) = certify_stage(
                &grid,
                source_levels,
                &state,
                config.max_adjacent_level_delta,
                config.angle_contract,
            ) {
                return ElasticCmrcOutcome::NotCertifiable {
                    reason: format!("stage {source_level}->{target_level}: {reason}"),
                };
            }
        }
        let certification_elapsed = certification_started.elapsed();
        if timing_enabled {
            eprintln!(
                "earthmesh_cli: cmrc_timing phase=elastic_level source_level={source_level} target_level={target_level} planning_ms={} components_ms={} certification_ms={} reused_component_certificate={reused_component_certificate} total_ms={}",
                planning_elapsed.as_millis(),
                components_elapsed.as_millis(),
                certification_elapsed.as_millis(),
                level_started.elapsed().as_millis()
            );
        }
        let delivered_histogram = match state.target_levels() {
            Ok(levels) => histogram(levels.levels().iter().copied()),
            Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
        };
        report.levels.push(ElasticLevelReport {
            source_level,
            target_level,
            components_total,
            components_committed: committed,
            components_promoted: promoted,
            components_exhausted: exhausted,
            components_rejected_for_global_hard: quality_stats.components_rejected_for_global_hard,
            components_rejected_for_target_quality: quality_stats
                .components_rejected_for_target_quality,
            delivered_histogram: delivered_histogram.clone(),
        });
        report.delivered_histogram = delivered_histogram;
    }

    report.final_faces = state.mesh().mesh.triangle_count();
    report.final_vertices = state.mesh().mesh.vertex_count();
    ElasticCmrcOutcome::Completed(Box::new(ElasticCmrcResult { state, report }))
}

fn validate_inputs(
    grid: &MotherGrid,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    required_levels: &[usize],
    config: &ElasticCmrcConfig,
) -> Result<(), String> {
    if mesh_fingerprint(&grid.mesh) != mesh_fingerprint(source_mesh) {
        return Err("CMRC grid and source mesh fingerprints differ".into());
    }
    if source_levels.active_sites()
        != grid
            .mesh
            .active_vertex_slots()
            .collect::<Vec<_>>()
            .as_slice()
    {
        return Err("source level field does not match the source grid".into());
    }
    if required_levels.len() != grid.mesh.vertices().len() {
        return Err("required level slots must match source vertices".into());
    }
    if config.maximum_transition_rings < config.initial_transition_rings {
        return Err("maximum transition rings must not be smaller than initial rings".into());
    }
    let factor = 1usize
        .checked_shl(config.max_level as u32)
        .ok_or_else(|| "CMRC maximum level overflows hierarchy scale".to_string())?;
    if !grid.subdivision.is_multiple_of(factor) {
        return Err(format!(
            "source subdivision {} is not divisible by 2^{}",
            grid.subdivision, config.max_level
        ));
    }
    Ok(())
}

fn explicit_parent_requirements(
    source: &MotherGrid,
    state: &ComponentTransactionState,
    level_grid: &MotherGrid,
    required_levels: &[usize],
) -> Result<Vec<ExplicitParentRequirement>, String> {
    let coarse_n = level_grid.subdivision / 2;
    let parent_count =
        mother_cell_count(coarse_n).ok_or_else(|| "hierarchy parent count overflow".to_string())?;
    let mut requirements = vec![None; parent_count];
    for child in level_grid.triangle_addresses.iter().flatten().copied() {
        let parent = child
            .parent_2_to_1()
            .ok_or_else(|| format!("level child {child:?} has no parent"))?;
        let dense = parent.dense_index(coarse_n)?;
        if requirements[dense].is_some() {
            continue;
        }
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?;
        let available = children
            .iter()
            .all(|child| state.leaf_set().leaves.contains(child));
        let mut maximum_required_level = 0usize;
        visit_source_descendant_faces(source, parent, &mut |face| {
            for site in source.mesh.triangles()[face] {
                maximum_required_level = maximum_required_level.max(
                    *required_levels
                        .get(site)
                        .ok_or_else(|| format!("source site {site} has no required level"))?,
                );
            }
            Ok(())
        })?;
        requirements[dense] = Some(ExplicitParentRequirement {
            parent,
            maximum_required_level,
            available,
        });
    }
    requirements
        .into_iter()
        .enumerate()
        .map(|(dense, requirement)| {
            requirement.ok_or_else(|| format!("hierarchy parent slot {dense} was not populated"))
        })
        .collect()
}

fn visit_source_descendant_faces(
    source: &MotherGrid,
    address: TriangleAddress,
    visit: &mut impl FnMut(usize) -> Result<(), String>,
) -> Result<(), String> {
    if address.n == source.subdivision {
        let face = address
            .dense_index(source.subdivision)?
            .checked_add(2)
            .ok_or_else(|| format!("source face slot overflow for {address:?}"))?;
        return visit(face);
    }
    if address.n == 0
        || address.n > source.subdivision
        || !source.subdivision.is_multiple_of(address.n)
        || !(source.subdivision / address.n).is_power_of_two()
    {
        return Err(format!(
            "hierarchy address {address:?} is not an ancestor of source subdivision {}",
            source.subdivision
        ));
    }
    for child in address
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy address {address:?}"))?
    {
        visit_source_descendant_faces(source, child, visit)?;
    }
    Ok(())
}

fn sort_components(
    components: &mut [HierarchyComponent],
    requirements: &[super::ParentRequirement],
    coarse_level: usize,
) {
    let coarse_n = components
        .iter()
        .flat_map(|component| component.parents.first())
        .map(|parent| parent.n)
        .next()
        .unwrap_or(1);
    let margin = |component: &HierarchyComponent| {
        let maximum = component
            .parents
            .iter()
            .filter_map(|parent| parent.dense_index(coarse_n).ok())
            .filter_map(|dense| requirements.get(dense))
            .map(|requirement| requirement.maximum_required_level)
            .max()
            .unwrap_or(coarse_level);
        coarse_level.saturating_sub(maximum)
    };
    components.sort_by(|left, right| {
        right
            .core_parents
            .len()
            .cmp(&left.core_parents.len())
            .then_with(|| left.boundary_edges.len().cmp(&right.boundary_edges.len()))
            .then_with(|| {
                right
                    .transition_parents
                    .is_empty()
                    .cmp(&left.transition_parents.is_empty())
            })
            .then_with(|| {
                left.transition_parents
                    .len()
                    .cmp(&right.transition_parents.len())
            })
            .then_with(|| margin(right).cmp(&margin(left)))
            .then_with(|| left.parents.cmp(&right.parents))
    });
}

fn rollback_record(
    component: &HierarchyComponent,
    source_level: usize,
    target_level: usize,
    initial_transition_rings: usize,
    outcome: ComponentOutcomeKind,
    rollback: ComponentRollbackReport,
) -> ElasticComponentRecord {
    ElasticComponentRecord {
        component_id: component.id,
        source_level,
        target_level,
        parent_count: component.parents.len(),
        core_parent_count: component.core_parents.len(),
        transition_parent_count: component.transition_parents.len(),
        core_vertices_removed: 0,
        topology_states: rollback.topology_states,
        elastic_iterations: rollback.elastic_iterations,
        interval_boxes: rollback.interval_boxes,
        transition_ring_width: initial_transition_rings + rollback.halo_expansions,
        outcome,
        reason: Some(rollback.reason),
    }
}

fn certify_stage(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &ComponentTransactionState,
    max_adjacent_level_delta: usize,
    angle_contract: AngleContractId,
) -> Result<(), String> {
    Certificate::internal_for(angle_contract)
        .verify_geometry(&state.mesh().mesh)
        .map_err(|error| format!("internal geometry: {error:?}"))?;
    Certificate::final_delivery_for(angle_contract)
        .verify_geometry(&state.mesh().mesh)
        .map_err(|error| format!("final geometry: {error:?}"))?;
    let target_levels = state.target_levels()?;
    let remap = ConservativeRemap::between_voronoi_meshes(&source.mesh, &state.mesh().mesh)?;
    let final_cells = certify_final_cell_requirements_with_remap(
        &source.mesh,
        source_levels,
        &state.mesh().mesh,
        &target_levels,
        max_adjacent_level_delta,
        &remap,
    )
    .map_err(|error| format!("final cells: {error:?}"))?;
    let remap_certificate =
        remap.certify_spherical_overlap(source_levels.levels().len(), target_levels.levels().len());
    FinalCertificationEvidence::from_final_cells(&final_cells, remap_certificate)
        .map_err(|reason| format!("remap: {reason}"))?;
    Ok(())
}

fn histogram(levels: impl IntoIterator<Item = usize>) -> BTreeMap<usize, usize> {
    let mut histogram = BTreeMap::new();
    for level in levels {
        *histogram.entry(level).or_default() += 1;
    }
    histogram
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mother_grid::TriangleAddress;

    fn parent(source: &MotherGrid, base_face: u8) -> TriangleAddress {
        source
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .find(|address| address.base_face == base_face)
            .and_then(TriangleAddress::parent_2_to_1)
            .unwrap()
    }

    fn component(id: u64, parent: TriangleAddress) -> HierarchyComponent {
        HierarchyComponent {
            id,
            parents: vec![parent],
            boundary_edges: Vec::new(),
            core_parents: Vec::new(),
            transition_parents: vec![parent],
        }
    }

    fn context(
        source: &MotherGrid,
        target: TriangleAddress,
        boundary: TriangleAddress,
    ) -> BTreeMap<usize, SpatialFaceContext> {
        source
            .mesh
            .active_triangle_slots()
            .map(|face| {
                let mut item = SpatialFaceContext::default();
                let parent = source.triangle_addresses[face]
                    .unwrap()
                    .parent_2_to_1()
                    .unwrap();
                if parent == target {
                    item.quality.zone = QualityZone::TargetCore;
                    item.quality.maximum_priority = 1.0;
                    item.quality.mean_priority = 1.0;
                    item.quality.minimum_distance_to_target = 0.0;
                } else if parent == boundary {
                    item.quality.zone = QualityZone::BoundaryProtection;
                    item.quality.maximum_priority = 1.0;
                    item.quality.mean_priority = 1.0;
                    item.quality.minimum_distance_to_target = 1.0;
                } else {
                    item.quality.zone = QualityZone::DeepExterior;
                    item.quality.maximum_priority = 0.0;
                    item.quality.mean_priority = 0.0;
                    item.quality.minimum_distance_to_target = 10.0;
                }
                (face, item)
            })
            .collect()
    }

    fn parent_neighbours(grid: &MotherGrid, parent: TriangleAddress) -> BTreeSet<TriangleAddress> {
        let children = parent
            .children_2_to_1()
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>();
        grid.mesh
            .active_triangle_slots()
            .filter(|&face| {
                grid.triangle_addresses[face].is_some_and(|child| children.contains(&child))
            })
            .flat_map(|face| grid.mesh.neighbours()[face])
            .filter_map(|face| {
                grid.triangle_addresses[face].and_then(TriangleAddress::parent_2_to_1)
            })
            .filter(|&other| other != parent)
            .collect()
    }

    fn parent_ball(
        grid: &MotherGrid,
        center: TriangleAddress,
        rings: usize,
    ) -> BTreeSet<TriangleAddress> {
        let mut ball = BTreeSet::from([center]);
        let mut frontier = ball.clone();
        for _ in 0..rings {
            let next = frontier
                .iter()
                .flat_map(|&parent| parent_neighbours(grid, parent))
                .filter(|parent| !ball.contains(parent))
                .collect::<BTreeSet<_>>();
            ball.extend(next.iter().copied());
            frontier = next;
        }
        ball
    }

    fn lower_patch_requirements(
        grid: &MotherGrid,
        parents: &BTreeSet<TriangleAddress>,
        required: &mut [usize],
    ) {
        for face in grid.mesh.active_triangle_slots() {
            if grid.triangle_addresses[face]
                .and_then(TriangleAddress::parent_2_to_1)
                .is_some_and(|parent| parents.contains(&parent))
            {
                for site in grid.mesh.triangles()[face] {
                    required[site] = 2;
                }
            }
        }
    }

    #[test]
    fn outside_component_runs_before_target_component() {
        let source = MotherGrid::generate(2).unwrap();
        let target = parent(&source, 0);
        let boundary = parent(&source, 1);
        let outside = parent(&source, 10);
        let mut components = vec![
            component(0, target),
            component(1, boundary),
            component(2, outside),
        ];
        let family = components
            .iter()
            .map(|component| component.parents.clone())
            .collect::<BTreeSet<_>>();

        let priorities = sort_components_outside_in(
            &source,
            &mut components,
            &context(&source, target, boundary),
        )
        .unwrap();

        assert_eq!(
            components.iter().map(|item| item.id).collect::<Vec<_>>(),
            [2, 1, 0]
        );
        assert_eq!(priorities[0].maximum_quality_priority, 0.0);
        assert_eq!(priorities[1].transition_target_overlap, 4);
        assert_eq!(priorities[2].transition_target_overlap, 4);
        assert_eq!(
            components
                .iter()
                .map(|component| component.parents.clone())
                .collect::<BTreeSet<_>>(),
            family
        );
    }

    #[test]
    fn stable_order_independent_of_input_iteration() {
        let source = MotherGrid::generate(2).unwrap();
        let target = parent(&source, 0);
        let boundary = parent(&source, 1);
        let outside = parent(&source, 10);
        let face_context = context(&source, target, boundary);
        let mut forward = vec![
            component(0, target),
            component(1, boundary),
            component(2, outside),
        ];
        let mut reverse = forward.iter().cloned().rev().collect::<Vec<_>>();

        sort_components_outside_in(&source, &mut forward, &face_context).unwrap();
        sort_components_outside_in(&source, &mut reverse, &face_context).unwrap();

        assert_eq!(forward, reverse);
    }

    #[test]
    fn hard_component_does_not_starve_other_components() {
        let mut stats = CoarseningScheduleStats::default();
        stats.consider();
        stats.reject(DomainQualityRejectReason::GlobalHardViolation);
        stats.consider();
        stats.commit();
        stats.consider();
        stats.reject(DomainQualityRejectReason::NotStrictImprovement);

        assert_eq!(stats.components_considered, 3);
        assert_eq!(stats.components_committed, 1);
        assert_eq!(stats.components_rejected_for_global_hard, 1);
        assert_eq!(stats.components_rejected_for_target_quality, 1);
    }

    #[test]
    fn epoch_runner_uses_outside_in_order_without_changing_family() {
        let source = MotherGrid::generate(8).unwrap();
        let coarse = MotherGrid::generate(4).unwrap();
        let target_center = coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .find(|address| address.base_face == 0)
            .unwrap();
        let outside_center = coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .find(|address| address.base_face == 10)
            .unwrap();
        let target_patch = parent_ball(&source, target_center, 2);
        let outside_patch = parent_ball(&source, outside_center, 1);
        assert!(target_patch.is_disjoint(&outside_patch));

        let mut required = vec![3; source.mesh.vertices().len()];
        lower_patch_requirements(&source, &target_patch, &mut required);
        lower_patch_requirements(&source, &outside_patch, &mut required);
        let source_levels = SourceLevelField::from_active_voronoi_cells(
            &source.mesh,
            source
                .mesh
                .active_vertex_slots()
                .map(|site| required[site])
                .collect(),
        )
        .unwrap();
        let face_context = source
            .mesh
            .active_triangle_slots()
            .map(|face| {
                let mut item = SpatialFaceContext::default();
                let parent = source.triangle_addresses[face]
                    .unwrap()
                    .parent_2_to_1()
                    .unwrap();
                if target_patch.contains(&parent) {
                    item.quality.zone = QualityZone::TargetCore;
                    item.quality.maximum_priority = 1.0;
                    item.quality.mean_priority = 1.0;
                    item.quality.minimum_distance_to_target = 0.0;
                } else {
                    item.quality.zone = QualityZone::DeepExterior;
                    item.quality.maximum_priority = 0.0;
                    item.quality.mean_priority = 0.0;
                    item.quality.minimum_distance_to_target = 10.0;
                }
                (face, item)
            })
            .collect::<BTreeMap<_, _>>();
        let config = ElasticCmrcConfig {
            angle_contract: AngleContractId::LegacyStrict40To80,
            max_level: 3,
            max_adjacent_level_delta: 1,
            initial_transition_rings: 1,
            maximum_transition_rings: 1,
            topology_states_per_component: 1,
            elastic_iterations_per_topology: 0,
            interval_boxes_per_component: 100_000,
            total_transition_states: 2,
            allow_safe_fallback: false,
        };

        let state = ComponentTransactionState::new(&source, config.max_level).unwrap();
        let requirements =
            explicit_parent_requirements(&source, &state, &source, &required).unwrap();
        let mut plan = plan_hierarchy_components_from_parent_requirements(
            &source,
            &requirements,
            2,
            config.initial_transition_rings,
        )
        .unwrap();
        assert_eq!(plan.components.len(), 2);
        let mut legacy = plan.components.clone();
        sort_components(&mut legacy, &plan.parent_requirements, 2);
        let priorities =
            sort_components_outside_in(&source, &mut plan.components, &face_context).unwrap();
        let expected_counts = plan
            .components
            .iter()
            .map(|component| component.parents.len())
            .collect::<Vec<_>>();
        assert_eq!(priorities[0].maximum_quality_priority, 0.0);
        assert_eq!(priorities[1].maximum_quality_priority, 1.0);
        assert_ne!(legacy, plan.components);

        let outcome = run_elastic_component_epochs_with_quality_context(
            source.clone(),
            &source.mesh,
            &source_levels,
            &required,
            &face_context,
            |_, _| None,
            &config,
        );
        let ElasticCmrcOutcome::Completed(result) = outcome else {
            panic!("outside-in epoch runner must complete: {outcome:?}")
        };
        assert_eq!(
            result
                .report
                .components
                .iter()
                .take(2)
                .map(|component| component.parent_count)
                .collect::<Vec<_>>(),
            expected_counts
        );
    }

    #[test]
    fn quality_gate_rejection_is_reported_and_rolls_back() {
        let source = MotherGrid::generate(2).unwrap();
        let required = vec![0; source.mesh.vertices().len()];
        let source_levels = SourceLevelField::from_active_voronoi_cells(
            &source.mesh,
            source.mesh.active_vertex_slots().map(|_| 0).collect(),
        )
        .unwrap();
        let face_context = source
            .mesh
            .active_triangle_slots()
            .map(|face| {
                let mut item = SpatialFaceContext::default();
                item.quality.zone = QualityZone::DeepExterior;
                item.quality.maximum_priority = 0.0;
                item.quality.mean_priority = 0.0;
                item.quality.minimum_distance_to_target = 10.0;
                (face, item)
            })
            .collect::<BTreeMap<_, _>>();
        let config = ElasticCmrcConfig {
            angle_contract: AngleContractId::LegacyStrict40To80,
            max_level: 1,
            max_adjacent_level_delta: 1,
            initial_transition_rings: 1,
            maximum_transition_rings: 1,
            topology_states_per_component: 1,
            elastic_iterations_per_topology: 0,
            interval_boxes_per_component: 100_000,
            total_transition_states: 1,
            allow_safe_fallback: false,
        };

        let outcome = run_elastic_component_epochs_with_quality_context(
            source.clone(),
            &source.mesh,
            &source_levels,
            &required,
            &face_context,
            |_, _| Some(DomainQualityRejectReason::GlobalHardViolation),
            &config,
        );
        let ElasticCmrcOutcome::Completed(result) = outcome else {
            panic!("quality rejection must preserve a publishable state: {outcome:?}")
        };

        assert_eq!(result.report.components_total, 1);
        assert_eq!(result.report.components_committed, 0);
        assert_eq!(result.report.components_rejected_for_global_hard, 1);
        assert_eq!(result.report.components_rejected_for_target_quality, 0);
        assert_eq!(
            result.report.components[0].outcome,
            ComponentOutcomeKind::RejectedForQuality
        );
        assert_eq!(
            result.report.components[0].outcome.as_str(),
            "rejected_for_quality"
        );
        assert_eq!(result.state.mesh().mesh, source.mesh);
    }
}
