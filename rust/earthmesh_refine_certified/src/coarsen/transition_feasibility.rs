//! PR #34 proof-only evidence for transition topology feasibility.
//!
//! This module is deliberately not wired into production coarsening.  It can
//! certify a witness only through the normal internal geometry certificate, and
//! it refuses to turn search exhaustion or an unsupported continuous domain into
//! a no-go theorem.

use super::{
    plan_hierarchy_components_from_parent_requirements, ElasticPatch, ExplicitParentRequirement,
    HierarchyComponent, TransitionTopologyLimits, TransitionTopologyOutcome,
    TransitionTopologyTrial,
};
use crate::{
    certificate::{
        interval::{next_down, next_up, Interval},
        spherical_triangle_angles, Certificate,
    },
    mother_grid::{MotherGrid, TriangleAddress, TriangleOrientation},
};
use earthmesh_mesh::{CartesianPoint, MeshState};
use std::collections::BTreeSet;

const STRICT_MIN_DEGREES: f64 = 40.2;
const STRICT_MAX_DEGREES: f64 = 79.8;
// Bounds deliberately outside the true cosine values; proof arithmetic then
// uses nextafter widening and no trigonometric calls.
const MIN_ANGLE_COS_UPPER: f64 = 0.763_796_028_635;
const MAX_ANGLE_COS_LOWER: f64 = 0.177_084_740_319;
const RADIANS_TO_DEGREES_LOWER: f64 = 57.295_779_513_082;
const TRIVIAL_MARGIN_UPPER_DEGREES: f64 = 19.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionFeasibilityOutcomeKind {
    CertifiedFeasible,
    CertifiedInfeasible,
    UnknownBudgetExhausted,
}

impl TransitionFeasibilityOutcomeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::CertifiedFeasible => "CertifiedFeasible",
            Self::CertifiedInfeasible => "CertifiedInfeasible",
            Self::UnknownBudgetExhausted => "UnknownBudgetExhausted",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeasibleWitness {
    pub min_angle_degrees: f64,
    pub max_angle_degrees: f64,
    pub movable_positions: Vec<CartesianPoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeasibilityProof {
    CertifiedFeasible {
        fixed_vertices: usize,
        movable_vertices: usize,
        guard_faces: usize,
        boxes: u64,
        best_numerical_margin_degrees: f64,
        best_upper_bound_degrees: f64,
        witness: FeasibleWitness,
    },
    CertifiedInfeasible {
        fixed_vertices: usize,
        movable_vertices: usize,
        guard_faces: usize,
        boxes: u64,
        best_numerical_margin_degrees: f64,
        upper_margin_degrees: f64,
    },
    UnknownBudgetExhausted {
        fixed_vertices: usize,
        movable_vertices: usize,
        guard_faces: usize,
        boxes: u64,
        best_numerical_margin_degrees: Option<f64>,
        best_upper_bound_degrees: Option<f64>,
        reason: String,
    },
}

impl FeasibilityProof {
    fn kind(&self) -> TransitionFeasibilityOutcomeKind {
        match self {
            Self::CertifiedFeasible { .. } => TransitionFeasibilityOutcomeKind::CertifiedFeasible,
            Self::CertifiedInfeasible { .. } => {
                TransitionFeasibilityOutcomeKind::CertifiedInfeasible
            }
            Self::UnknownBudgetExhausted { .. } => {
                TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted
            }
        }
    }

    fn boxes(&self) -> u64 {
        match self {
            Self::CertifiedFeasible { boxes, .. }
            | Self::CertifiedInfeasible { boxes, .. }
            | Self::UnknownBudgetExhausted { boxes, .. } => *boxes,
        }
    }

    fn fixed_vertices(&self) -> usize {
        match self {
            Self::CertifiedFeasible { fixed_vertices, .. }
            | Self::CertifiedInfeasible { fixed_vertices, .. }
            | Self::UnknownBudgetExhausted { fixed_vertices, .. } => *fixed_vertices,
        }
    }

    fn movable_vertices(&self) -> usize {
        match self {
            Self::CertifiedFeasible {
                movable_vertices, ..
            }
            | Self::CertifiedInfeasible {
                movable_vertices, ..
            }
            | Self::UnknownBudgetExhausted {
                movable_vertices, ..
            } => *movable_vertices,
        }
    }

    fn guard_faces(&self) -> usize {
        match self {
            Self::CertifiedFeasible { guard_faces, .. }
            | Self::CertifiedInfeasible { guard_faces, .. }
            | Self::UnknownBudgetExhausted { guard_faces, .. } => *guard_faces,
        }
    }

    fn best_numerical_margin_degrees(&self) -> Option<f64> {
        match self {
            Self::CertifiedFeasible {
                best_numerical_margin_degrees,
                ..
            }
            | Self::CertifiedInfeasible {
                best_numerical_margin_degrees,
                ..
            } => Some(*best_numerical_margin_degrees),
            Self::UnknownBudgetExhausted {
                best_numerical_margin_degrees,
                ..
            } => *best_numerical_margin_degrees,
        }
    }

    fn best_upper_bound_degrees(&self) -> Option<f64> {
        match self {
            Self::CertifiedFeasible {
                best_upper_bound_degrees,
                ..
            } => Some(*best_upper_bound_degrees),
            Self::CertifiedInfeasible {
                upper_margin_degrees,
                ..
            } => Some(*upper_margin_degrees),
            Self::UnknownBudgetExhausted {
                best_upper_bound_degrees,
                ..
            } => *best_upper_bound_degrees,
        }
    }

    fn witness_positions(&self) -> &[CartesianPoint] {
        match self {
            Self::CertifiedFeasible { witness, .. } => &witness.movable_positions,
            Self::CertifiedInfeasible { .. } | Self::UnknownBudgetExhausted { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionTopologyFeasibilityEvidence {
    pub topology_id: usize,
    pub fixed_vertices: usize,
    pub movable_vertices: usize,
    pub guard_faces: usize,
    pub triangle_count: usize,
    pub min_angle_degrees: f64,
    pub max_angle_degrees: f64,
    pub numerical_margin_degrees: f64,
    pub interval_upper_margin_degrees: Option<f64>,
    pub boxes: u64,
    pub outcome: TransitionFeasibilityOutcomeKind,
    pub witness_positions: Vec<CartesianPoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionFeasibilityProof {
    pub fixture: String,
    pub source_subdivision: usize,
    pub component_id: u64,
    pub core_parent_count: usize,
    pub transition_parent_count: usize,
    pub family_topology_count: usize,
    pub topology_budget: usize,
    pub topology_family_closed: bool,
    pub interval_box_budget_per_topology: u64,
    pub interval_boxes: u64,
    pub best_numerical_margin_degrees: Option<f64>,
    pub interval_upper_margin_degrees: Option<f64>,
    pub outcome: TransitionFeasibilityOutcomeKind,
    pub conclusion: String,
    pub topologies: Vec<TransitionTopologyFeasibilityEvidence>,
}

impl TransitionFeasibilityProof {
    pub fn to_machine_readable_json(&self) -> String {
        let topologies = self
            .topologies
            .iter()
            .map(TransitionTopologyFeasibilityEvidence::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"fixture\":\"{}\",\"source_subdivision\":{},\"component_id\":{},\"core_parent_count\":{},\"transition_parent_count\":{},\"family_topology_count\":{},\"topology_budget\":{},\"topology_family_closed\":{},\"interval_box_budget_per_topology\":{},\"interval_boxes\":{},\"best_numerical_margin_degrees\":{},\"interval_upper_margin_degrees\":{},\"outcome\":\"{}\",\"conclusion\":\"{}\",\"topologies\":[{}]}}",
            json_escape(&self.fixture),
            self.source_subdivision,
            self.component_id,
            self.core_parent_count,
            self.transition_parent_count,
            self.family_topology_count,
            self.topology_budget,
            self.topology_family_closed,
            self.interval_box_budget_per_topology,
            self.interval_boxes,
            json_number_or_null(self.best_numerical_margin_degrees),
            json_number_or_null(self.interval_upper_margin_degrees),
            self.outcome.as_str(),
            json_escape(&self.conclusion),
            topologies
        )
    }
}

impl TransitionTopologyFeasibilityEvidence {
    fn to_json(&self) -> String {
        format!(
            "{{\"topology_id\":{},\"fixed_vertices\":{},\"movable_vertices\":{},\"guard_faces\":{},\"triangle_count\":{},\"min_angle_degrees\":{},\"max_angle_degrees\":{},\"numerical_margin_degrees\":{},\"interval_upper_margin_degrees\":{},\"boxes\":{},\"outcome\":\"{}\",\"witness_positions\":[{}]}}",
            self.topology_id,
            self.fixed_vertices,
            self.movable_vertices,
            self.guard_faces,
            self.triangle_count,
            finite_json_number(self.min_angle_degrees),
            finite_json_number(self.max_angle_degrees),
            finite_json_number(self.numerical_margin_degrees),
            json_number_or_null(self.interval_upper_margin_degrees),
            self.boxes,
            self.outcome.as_str(),
            json_points(&self.witness_positions),
        )
    }
}

pub fn prove_transition_topology_trial(
    trial: &TransitionTopologyTrial,
    box_budget: u64,
) -> FeasibilityProof {
    let patch = match ElasticPatch::from_transition(trial) {
        Ok(patch) => patch,
        Err(reason) => {
            return FeasibilityProof::UnknownBudgetExhausted {
                fixed_vertices: 0,
                movable_vertices: 0,
                guard_faces: 0,
                boxes: 0,
                best_numerical_margin_degrees: None,
                best_upper_bound_degrees: None,
                reason,
            };
        }
    };
    prove_patch(&trial.mesh.mesh, &patch, box_budget)
}

pub fn n6_legacy_mixed_fixture() -> Result<(MotherGrid, HierarchyComponent), String> {
    let source = MotherGrid::generate(6)?;
    let parents = MotherGrid::generate(3)?;
    let eligible = n6_eligible_parents();
    let requirements = parents
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .map(|parent| ExplicitParentRequirement {
            parent,
            maximum_required_level: usize::from(!eligible.contains(&parent)),
            available: true,
        })
        .collect::<Vec<_>>();
    let mut components =
        plan_hierarchy_components_from_parent_requirements(&source, &requirements, 0, 1)?
            .components;
    if components.len() != 1 {
        return Err(format!(
            "frozen N6 fixture produced {} components instead of one",
            components.len()
        ));
    }
    Ok((source, components.pop().unwrap()))
}

pub fn analyze_legacy_transition_family(
    source: &MotherGrid,
    component: &HierarchyComponent,
    topology_budget: usize,
    interval_box_budget_per_topology: u64,
) -> TransitionFeasibilityProof {
    let limits = TransitionTopologyLimits {
        topology_states: topology_budget,
        maximum_halo_expansions: 2,
    };
    let mut cursor = 0usize;
    let mut topologies = Vec::new();

    let (topology_family_closed, final_reason) = loop {
        if cursor >= topology_budget {
            break (
                false,
                "topology budget exhausted before legacy family closed".into(),
            );
        }
        match limits.solve_from_cursor(source, component, cursor) {
            TransitionTopologyOutcome::Closed(trial) => {
                if trial.report.topology_states <= cursor {
                    break (false, "legacy cursor did not advance".into());
                }
                topologies.push(evidence_for_trial(&trial, interval_box_budget_per_topology));
                cursor = trial.report.topology_states;
            }
            TransitionTopologyOutcome::ProvenInfeasible { reason, .. } => break (true, reason),
            TransitionTopologyOutcome::InvalidBoundary { reason, .. } => {
                break (false, format!("legacy boundary invalid: {reason}"));
            }
            TransitionTopologyOutcome::SearchBudgetExhausted { .. }
            | TransitionTopologyOutcome::RequiresWiderHalo { .. } => {
                break (
                    false,
                    "legacy enumeration did not close inside the supplied budget".into(),
                );
            }
        }
    };

    let best = topologies
        .iter()
        .map(|evidence| evidence.numerical_margin_degrees)
        .max_by(f64::total_cmp);
    let upper = topologies
        .iter()
        .filter_map(|evidence| evidence.interval_upper_margin_degrees)
        .max_by(f64::total_cmp);
    let all_infeasible = !topologies.is_empty()
        && topologies.iter().all(|evidence| {
            evidence.outcome == TransitionFeasibilityOutcomeKind::CertifiedInfeasible
        });
    let any_feasible = topologies
        .iter()
        .any(|evidence| evidence.outcome == TransitionFeasibilityOutcomeKind::CertifiedFeasible);
    let outcome = if any_feasible {
        TransitionFeasibilityOutcomeKind::CertifiedFeasible
    } else if topology_family_closed && all_infeasible {
        TransitionFeasibilityOutcomeKind::CertifiedInfeasible
    } else {
        TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted
    };
    let conclusion = match outcome {
        TransitionFeasibilityOutcomeKind::CertifiedFeasible => {
            "legacy family has a witness accepted by Certificate::internal().verify_geometry".into()
        }
        TransitionFeasibilityOutcomeKind::CertifiedInfeasible => {
            format!("legacy interval boxes are all strictly pruned: {final_reason}")
        }
        TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted => {
            if topology_family_closed {
                format!(
                    "legacy topology enumeration closed after {} emitted topologies, but at least one continuous interval proof is unknown: {final_reason}",
                    topologies.len()
                )
            } else {
                format!("legacy topology enumeration is not closed: {final_reason}")
            }
        }
    };

    TransitionFeasibilityProof {
        fixture: "n6_hidden_mixed_exact_32_parent_component".into(),
        source_subdivision: source.subdivision,
        component_id: component.id,
        core_parent_count: component.core_parents.len(),
        transition_parent_count: component.transition_parents.len(),
        family_topology_count: topologies.len(),
        topology_budget,
        topology_family_closed,
        interval_box_budget_per_topology,
        interval_boxes: topologies.iter().map(|evidence| evidence.boxes).sum(),
        best_numerical_margin_degrees: best,
        interval_upper_margin_degrees: upper,
        outcome,
        conclusion,
        topologies,
    }
}

fn prove_patch(mesh: &MeshState, patch: &ElasticPatch, box_budget: u64) -> FeasibilityProof {
    prove_geometry_domain(
        mesh,
        patch.fixed_compact_vertices.len(),
        &patch.guard_faces,
        &patch.movable_compact_vertices,
        vec![CartesianBox::full_sphere(); patch.movable_compact_vertices.len()],
        box_budget,
    )
}

fn prove_geometry_domain(
    mesh: &MeshState,
    fixed_vertices: usize,
    guard_face_slots: &[usize],
    movable_slots: &[usize],
    initial_domain: Vec<CartesianBox>,
    box_budget: u64,
) -> FeasibilityProof {
    debug_assert_eq!(initial_domain.len(), movable_slots.len());
    let movable_vertices = movable_slots.len();
    let guard_faces = guard_face_slots.len();
    let witness_positions = movable_slots
        .iter()
        .map(|&vertex| mesh.vertices()[vertex])
        .collect::<Vec<_>>();
    let numerical = margin_for_faces(mesh, guard_face_slots);

    if let Ok(report) = Certificate::internal().verify_geometry(mesh) {
        let margin = (report.min_angle_degrees - STRICT_MIN_DEGREES)
            .min(STRICT_MAX_DEGREES - report.max_angle_degrees);
        return FeasibilityProof::CertifiedFeasible {
            fixed_vertices,
            movable_vertices,
            guard_faces,
            boxes: 1,
            best_numerical_margin_degrees: margin,
            best_upper_bound_degrees: if initial_domain.iter().all(|point| point.exact_point) {
                margin
            } else {
                TRIVIAL_MARGIN_UPPER_DEGREES
            },
            witness: FeasibleWitness {
                min_angle_degrees: report.min_angle_degrees,
                max_angle_degrees: report.max_angle_degrees,
                movable_positions: witness_positions,
            },
        };
    }

    if box_budget == 0 {
        return FeasibilityProof::UnknownBudgetExhausted {
            fixed_vertices,
            movable_vertices,
            guard_faces,
            boxes: 0,
            best_numerical_margin_degrees: numerical,
            best_upper_bound_degrees: None,
            reason: "zero interval box budget".into(),
        };
    }

    // [-1, 1]^3 contains the complete unit sphere for every movable vertex.
    // Ignoring the unit-norm constraint only enlarges the search domain, so a
    // full prune is a valid no-go proof; an incomplete prune remains Unknown.
    prove_interval_box_domain(
        mesh,
        fixed_vertices,
        guard_faces,
        guard_face_slots,
        movable_slots,
        initial_domain,
        box_budget,
        numerical,
    )
}

fn evidence_for_trial(
    trial: &TransitionTopologyTrial,
    box_budget: u64,
) -> TransitionTopologyFeasibilityEvidence {
    let proof = prove_transition_topology_trial(trial, box_budget);
    let guard_faces = ElasticPatch::from_transition(trial)
        .map(|patch| patch.guard_faces)
        .unwrap_or_default();
    let (min_angle, max_angle) = angle_range(&trial.mesh.mesh, &guard_faces);
    let numerical_margin = (min_angle - STRICT_MIN_DEGREES).min(STRICT_MAX_DEGREES - max_angle);
    TransitionTopologyFeasibilityEvidence {
        topology_id: trial.candidate.topology_id,
        fixed_vertices: proof.fixed_vertices(),
        movable_vertices: proof.movable_vertices(),
        guard_faces: proof.guard_faces(),
        triangle_count: trial.candidate.source_triangles.len(),
        min_angle_degrees: min_angle,
        max_angle_degrees: max_angle,
        numerical_margin_degrees: proof
            .best_numerical_margin_degrees()
            .unwrap_or(numerical_margin),
        interval_upper_margin_degrees: proof.best_upper_bound_degrees(),
        boxes: proof.boxes(),
        outcome: proof.kind(),
        witness_positions: proof.witness_positions().to_vec(),
    }
}

fn prove_interval_box_domain(
    mesh: &MeshState,
    fixed_vertices: usize,
    guard_faces: usize,
    guard_face_slots: &[usize],
    movable_slots: &[usize],
    initial_domain: Vec<CartesianBox>,
    box_budget: u64,
    numerical_margin_degrees: Option<f64>,
) -> FeasibilityProof {
    debug_assert_eq!(initial_domain.len(), movable_slots.len());
    let movable_vertices = movable_slots.len();
    let mut pending = vec![initial_domain];
    let mut boxes = 0u64;
    let mut pruned_upper_margin = f64::NEG_INFINITY;

    while let Some(domain) = pending.pop() {
        if boxes == box_budget {
            pending.push(domain);
            break;
        }
        boxes += 1;
        if let Some(upper_margin) =
            interval_violation_upper_margin(mesh, guard_face_slots, movable_slots, &domain)
        {
            pruned_upper_margin = pruned_upper_margin.max(upper_margin);
            continue;
        }
        let Some((left, right)) = split_domain(domain) else {
            return FeasibilityProof::UnknownBudgetExhausted {
                fixed_vertices,
                movable_vertices,
                guard_faces,
                boxes,
                best_numerical_margin_degrees: numerical_margin_degrees,
                best_upper_bound_degrees: Some(TRIVIAL_MARGIN_UPPER_DEGREES),
                reason: "closed singleton box is not interval-certifiable".into(),
            };
        };
        pending.push(right);
        pending.push(left);
    }

    if pending.is_empty() && pruned_upper_margin.is_finite() && pruned_upper_margin < 0.0 {
        return FeasibilityProof::CertifiedInfeasible {
            fixed_vertices,
            movable_vertices,
            guard_faces,
            boxes,
            best_numerical_margin_degrees: numerical_margin_degrees.unwrap_or(pruned_upper_margin),
            upper_margin_degrees: pruned_upper_margin,
        };
    }

    FeasibilityProof::UnknownBudgetExhausted {
        fixed_vertices,
        movable_vertices,
        guard_faces,
        boxes,
        best_numerical_margin_degrees: numerical_margin_degrees,
        best_upper_bound_degrees: Some(TRIVIAL_MARGIN_UPPER_DEGREES),
        reason: "interval box budget exhausted before the global sphere superset closed".into(),
    }
}

#[derive(Clone, Copy)]
struct CartesianBox {
    x: Interval,
    y: Interval,
    z: Interval,
    exact_point: bool,
}

impl CartesianBox {
    fn point(point: CartesianPoint) -> Self {
        Self {
            x: Interval::point(point.x),
            y: Interval::point(point.y),
            z: Interval::point(point.z),
            exact_point: true,
        }
    }

    fn full_sphere() -> Self {
        let axis = Interval { lo: -1.0, hi: 1.0 };
        Self {
            x: axis,
            y: axis,
            z: axis,
            exact_point: false,
        }
    }

    fn scaled(self, scale: Interval) -> Self {
        Self {
            x: self.x.mul_out(scale),
            y: self.y.mul_out(scale),
            z: self.z.mul_out(scale),
            exact_point: false,
        }
    }

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x.sub_out(rhs.x),
            y: self.y.sub_out(rhs.y),
            z: self.z.sub_out(rhs.z),
            exact_point: false,
        }
    }

    fn dot(self, rhs: Self) -> Interval {
        self.x
            .mul_out(rhs.x)
            .add_out(self.y.mul_out(rhs.y))
            .add_out(self.z.mul_out(rhs.z))
    }
}

fn interval_violation_upper_margin(
    mesh: &MeshState,
    faces: &[usize],
    movable_slots: &[usize],
    domain: &[CartesianBox],
) -> Option<f64> {
    let mut upper_margin = f64::INFINITY;
    for &face in faces {
        if !mesh.is_triangle_live(face) {
            return None;
        }
        let corners = mesh.triangles()[face];
        for corner in 0..3 {
            let point = |site| match movable_slots.binary_search(&site) {
                Ok(index) => domain[index],
                Err(_) => CartesianBox::point(mesh.vertices()[site]),
            };
            if let Some(bound) = angle_violation_upper_margin(
                point(corners[corner]),
                point(corners[(corner + 1) % 3]),
                point(corners[(corner + 2) % 3]),
            ) {
                upper_margin = upper_margin.min(bound);
            }
        }
    }
    upper_margin.is_finite().then_some(upper_margin)
}

fn angle_violation_upper_margin(a: CartesianBox, b: CartesianBox, c: CartesianBox) -> Option<f64> {
    let aa = a.dot(a);
    let tangent_b = b.scaled(aa).sub(a.scaled(a.dot(b)));
    let tangent_c = c.scaled(aa).sub(a.scaled(a.dot(c)));
    let dot = tangent_b.dot(tangent_c);
    let norm_b_sq = tangent_b.dot(tangent_b);
    let norm_c_sq = tangent_c.dot(tangent_c);
    if norm_b_sq.lo <= 0.0 || norm_c_sq.lo <= 0.0 {
        return None;
    }
    if dot.hi <= 0.0 {
        return Some(next_up(STRICT_MAX_DEGREES - 90.0));
    }
    if dot.lo <= 0.0 {
        return None;
    }

    let dot_sq_lower = next_down(dot.lo * dot.lo);
    let dot_sq_upper = next_up(dot.hi * dot.hi);
    let norms_lower = next_down(norm_b_sq.lo * norm_c_sq.lo);
    let norms_upper = next_up(norm_b_sq.hi * norm_c_sq.hi);
    if norms_lower <= 0.0 {
        return None;
    }

    let min_cos_sq_upper = next_up(MIN_ANGLE_COS_UPPER * MIN_ANGLE_COS_UPPER);
    if dot_sq_lower > next_up(min_cos_sq_upper * norms_upper) {
        let cos_lower = next_down(next_down(dot_sq_lower / norms_upper).max(0.0).sqrt());
        return negative_degree_margin(next_down(cos_lower - MIN_ANGLE_COS_UPPER));
    }

    let max_cos_sq_lower = next_down(MAX_ANGLE_COS_LOWER * MAX_ANGLE_COS_LOWER);
    if dot_sq_upper < next_down(max_cos_sq_lower * norms_lower) {
        let cos_upper = next_up(next_up(dot_sq_upper / norms_lower).max(0.0).sqrt());
        return negative_degree_margin(next_down(MAX_ANGLE_COS_LOWER - cos_upper));
    }
    None
}

fn negative_degree_margin(cosine_gap_lower: f64) -> Option<f64> {
    if cosine_gap_lower <= 0.0 {
        return None;
    }
    let degrees = next_down(cosine_gap_lower * RADIANS_TO_DEGREES_LOWER);
    (degrees > 0.0).then(|| next_up(-degrees))
}

fn split_domain(mut domain: Vec<CartesianBox>) -> Option<(Vec<CartesianBox>, Vec<CartesianBox>)> {
    let mut widest = None;
    for (point, bounds) in domain.iter().enumerate() {
        if bounds.exact_point {
            continue;
        }
        for (axis, interval) in [bounds.x, bounds.y, bounds.z].into_iter().enumerate() {
            let width = interval.hi - interval.lo;
            if width.is_finite() && width > 0.0 && widest.is_none_or(|(_, _, best)| width > best) {
                widest = Some((point, axis, width));
            }
        }
    }
    let (point, axis, _) = widest?;
    let interval = match axis {
        0 => domain[point].x,
        1 => domain[point].y,
        _ => domain[point].z,
    };
    let middle = interval.lo + (interval.hi - interval.lo) * 0.5;
    if middle <= interval.lo || middle >= interval.hi {
        return None;
    }
    let mut right = domain.clone();
    let (left_axis, right_axis) = match axis {
        0 => (&mut domain[point].x, &mut right[point].x),
        1 => (&mut domain[point].y, &mut right[point].y),
        _ => (&mut domain[point].z, &mut right[point].z),
    };
    left_axis.hi = next_up(middle);
    right_axis.lo = next_down(middle);
    Some((domain, right))
}

fn margin_for_faces(mesh: &MeshState, faces: &[usize]) -> Option<f64> {
    let mut best = f64::INFINITY;
    for &face in faces {
        if !mesh.is_triangle_live(face) {
            return None;
        }
        let corners = mesh.triangles()[face];
        let angles = spherical_triangle_angles(corners.map(|site| mesh.vertices()[site]))?;
        for angle in angles {
            best = best.min((angle - STRICT_MIN_DEGREES).min(STRICT_MAX_DEGREES - angle));
        }
    }
    best.is_finite().then_some(best)
}

fn angle_range(mesh: &MeshState, faces: &[usize]) -> (f64, f64) {
    let mut min_angle = f64::INFINITY;
    let mut max_angle = f64::NEG_INFINITY;
    for &face in faces {
        if !mesh.is_triangle_live(face) {
            continue;
        }
        let Some(angles) =
            spherical_triangle_angles(mesh.triangles()[face].map(|site| mesh.vertices()[site]))
        else {
            continue;
        };
        for angle in angles {
            min_angle = min_angle.min(angle);
            max_angle = max_angle.max(angle);
        }
    }
    if min_angle.is_finite() && max_angle.is_finite() {
        (min_angle, max_angle)
    } else {
        (0.0, 180.0)
    }
}

fn n6_eligible_parents() -> BTreeSet<TriangleAddress> {
    use TriangleOrientation::{Down as D, Up as U};
    [
        (0, 1, 0, U),
        (0, 1, 0, D),
        (0, 2, 0, U),
        (3, 0, 1, U),
        (3, 0, 1, D),
        (3, 0, 2, U),
        (4, 0, 0, U),
        (4, 0, 0, D),
        (4, 0, 1, U),
        (4, 0, 1, D),
        (4, 0, 2, U),
        (4, 1, 0, U),
        (4, 1, 0, D),
        (4, 1, 1, U),
        (4, 2, 0, U),
        (6, 2, 0, U),
        (7, 0, 0, U),
        (7, 0, 0, D),
        (7, 0, 1, U),
        (7, 0, 1, D),
        (7, 0, 2, U),
        (7, 1, 0, U),
        (7, 1, 0, D),
        (7, 1, 1, U),
        (7, 2, 0, U),
        (8, 0, 0, U),
        (16, 0, 1, U),
        (16, 0, 1, D),
        (16, 0, 2, U),
        (17, 0, 1, D),
        (17, 0, 2, U),
        (17, 1, 1, U),
    ]
    .into_iter()
    .map(|(base_face, i, j, orientation)| TriangleAddress {
        base_face,
        i,
        j,
        n: 3,
        orientation,
    })
    .collect()
}

fn finite_json_number(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.12}")
    } else {
        "null".into()
    }
}

fn json_number_or_null(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(finite_json_number)
        .unwrap_or_else(|| "null".into())
}

fn json_points(points: &[CartesianPoint]) -> String {
    points
        .iter()
        .map(|point| {
            format!(
                "[{}, {}, {}]",
                finite_json_number(point.x),
                finite_json_number(point.y),
                finite_json_number(point.z)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_mesh::MeshState;

    fn skinny_patch() -> MeshState {
        let vertices = vec![
            CartesianPoint::new(0.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 0.0, 0.0),
            unit(1.0, 0.0, 0.0),
            unit(0.999, 0.0447, 0.0),
            unit(0.0, 1.0, 0.0),
        ];
        MeshState::from_parts(
            vec![
                vertices[0],
                vertices[1],
                vertices[2],
                vertices[3],
                vertices[4],
            ],
            vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        )
        .unwrap()
    }

    fn unit(x: f64, y: f64, z: f64) -> CartesianPoint {
        let m = (x * x + y * y + z * z).sqrt();
        CartesianPoint::new(x / m, y / m, z / m)
    }

    #[test]
    fn synthetic_feasible_witness_requires_internal_certificate() {
        let grid = MotherGrid::generate(6).unwrap();
        let face = grid.mesh.active_triangle_slots().next().unwrap();
        let movable = grid.mesh.triangles()[face][0];
        let proof = prove_geometry_domain(
            &grid.mesh,
            2,
            &[face],
            &[movable],
            vec![CartesianBox::point(grid.mesh.vertices()[movable])],
            1,
        );
        assert!(matches!(
            proof,
            FeasibilityProof::CertifiedFeasible { witness, .. }
                if witness.min_angle_degrees >= STRICT_MIN_DEGREES
                    && witness.max_angle_degrees <= STRICT_MAX_DEGREES
        ));
    }

    #[test]
    fn synthetic_infeasible_point_box_is_strictly_negative() {
        let mesh = skinny_patch();
        let margin = margin_for_faces(&mesh, &[2]).unwrap();
        assert!(margin < 0.0);
        let proof = prove_interval_box_domain(
            &mesh,
            2,
            1,
            &[2],
            &[2],
            vec![CartesianBox::point(mesh.vertices()[2])],
            1,
            Some(margin),
        );
        assert!(matches!(
            proof,
            FeasibilityProof::CertifiedInfeasible {
                upper_margin_degrees,
                ..
            } if upper_margin_degrees < 0.0
        ));
    }

    #[test]
    fn zero_budget_cannot_be_infeasible() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let trial = match (TransitionTopologyLimits {
            topology_states: 1,
            maximum_halo_expansions: 0,
        })
        .solve_from_cursor(&source, &component, 0)
        {
            TransitionTopologyOutcome::Closed(trial) => trial,
            other => panic!("expected one closed topology, got {other:?}"),
        };
        assert!(matches!(
            prove_transition_topology_trial(&trial, 0),
            FeasibilityProof::UnknownBudgetExhausted { boxes: 0, .. }
        ));
    }

    #[test]
    fn invalid_boundary_cannot_close_the_family_as_infeasible() {
        let (source, mut component) = n6_legacy_mixed_fixture().unwrap();
        let core = component.core_parents[0];
        component.parents = vec![core];
        component.core_parents = vec![core];
        component.transition_parents = vec![core];
        let proof = analyze_legacy_transition_family(&source, &component, 1, 1);
        assert!(!proof.topology_family_closed);
        assert_eq!(
            proof.outcome,
            TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted
        );
    }

    #[test]
    fn topology_budget_exhaustion_cannot_close_the_family() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let proof = analyze_legacy_transition_family(&source, &component, 1, 1);
        assert!(!proof.topology_family_closed);
        assert_eq!(
            proof.outcome,
            TransitionFeasibilityOutcomeKind::UnknownBudgetExhausted
        );
    }
}
