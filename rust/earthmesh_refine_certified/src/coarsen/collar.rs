//! Finite CLDP collar expansion with strict geometry certification.

use super::core_condensation::rebuild_from_leaf_set_with_custom_face_slots;
use super::{
    build_promotion_patch_for_transition, solve_elastic_patch_with_max_min_trust_start,
    ElasticBlockLimits, ElasticBlockOutcome, ElasticPatch, ElasticTargetMode,
    GeometryFailureWitness, GeometryStartId, HierarchyComponent, HierarchyLeafMesh,
    HierarchyLeafSet, PromotionLevel, PromotionPatch, ViolationComponent, ViolationSupportAtlas,
};
use crate::certificate::spherical_triangle_angles;
use crate::{Certificate, GeometryCertificateReport, MotherGrid, TriangleAddress};
use earthmesh_mesh::{normalize_cartesian_to_radius, CartesianPoint, Sign};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromotionBudget {
    pub local_topology_states: usize,
    pub local_geometry_iterations: usize,
    pub maximum_patch_rings: usize,
    pub maximum_helper_vertices: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionFailureReason {
    PatchBoundaryMismatch(String),
    HelperVertexBudget { required: usize, available: usize },
    OrientationGuard,
    GeometryNotCertified,
    NoCompressedExterior,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromotionTrialEvidence {
    pub level: PromotionLevel,
    pub promoted_source_faces: usize,
    pub collar_source_faces: usize,
    pub helper_source_vertices: usize,
    pub homotopy_lambda: Option<f64>,
    pub angle_range_deg: Option<(f64, f64)>,
    pub strict: bool,
    pub protected_exterior_preserved: bool,
    pub reason: Option<PromotionFailureReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromotionTrial {
    pub mesh: HierarchyLeafMesh,
    pub promotion_patch: PromotionPatch,
    pub geometry: GeometryCertificateReport,
    pub evidence: PromotionTrialEvidence,
    pub adaptive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromotionOutcome {
    Certified(Box<PromotionTrial>),
    NeedsLargerPatch {
        next_level: PromotionLevel,
        reason: PromotionFailureReason,
    },
    SearchBudgetExhausted {
        incumbent_preserved: Box<GeometryFailureWitness>,
    },
    SafeMotherFallback(Box<PromotionTrial>),
    InvalidInput(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpandingCollarResult {
    pub outcome: PromotionOutcome,
    pub trials: Vec<PromotionTrialEvidence>,
}

pub fn solve_expanding_collar(
    source: &MotherGrid,
    hierarchy_component: &HierarchyComponent,
    incumbent: &GeometryFailureWitness,
    atlas: &ViolationSupportAtlas,
    component: &ViolationComponent,
    budget: PromotionBudget,
) -> ExpandingCollarResult {
    if budget.local_geometry_iterations == 0 || budget.maximum_helper_vertices == 0 {
        return invalid("promotion geometry and helper budgets must be positive");
    }
    if !atlas.components.iter().any(|item| item.id == component.id) {
        return invalid("promotion component is absent from the violation atlas");
    }
    let transition_parents = hierarchy_component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut levels = vec![PromotionLevel::P1RestoreSourceFaces];
    if budget.maximum_patch_rings >= 1 {
        levels.push(PromotionLevel::P2RestoreOneParentRing);
    }
    if budget.maximum_patch_rings >= 2 {
        levels.push(PromotionLevel::P3RestoreTwoParentRings);
    }
    levels.push(PromotionLevel::P4RestoreWholeTransitionComponent);
    let mut trials = Vec::new();
    for level in levels {
        let patch = match build_promotion_patch_for_transition(
            source,
            component,
            level,
            &transition_parents,
        ) {
            Ok(patch) => patch,
            Err(reason) => return invalid(reason),
        };
        match try_patch(source, incumbent, atlas, patch, budget) {
            Ok(trial) => {
                trials.push(trial.evidence.clone());
                return ExpandingCollarResult {
                    outcome: PromotionOutcome::Certified(Box::new(trial)),
                    trials,
                };
            }
            Err(evidence) => trials.push(evidence),
        }
    }
    safe_fallback(source, component, &transition_parents, trials)
}

fn try_patch(
    source: &MotherGrid,
    incumbent: &GeometryFailureWitness,
    atlas: &ViolationSupportAtlas,
    patch: PromotionPatch,
    budget: PromotionBudget,
) -> Result<PromotionTrial, PromotionTrialEvidence> {
    if patch.protected_exterior_faces.is_empty() {
        return Err(failed_evidence(
            &patch,
            0,
            None,
            None,
            PromotionFailureReason::NoCompressedExterior,
        ));
    }
    let materialized = match materialize_collar(source, incumbent, atlas, &patch) {
        Ok(materialized) => materialized,
        Err(reason) => {
            return Err(failed_evidence(
                &patch,
                0,
                None,
                None,
                PromotionFailureReason::PatchBoundaryMismatch(reason),
            ))
        }
    };
    if materialized.helper_source_vertices > budget.maximum_helper_vertices {
        return Err(failed_evidence(
            &patch,
            materialized.helper_source_vertices,
            None,
            angle_range(&materialized.mesh.mesh),
            PromotionFailureReason::HelperVertexBudget {
                required: materialized.helper_source_vertices,
                available: budget.maximum_helper_vertices,
            },
        ));
    }
    for lambda in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut candidate = materialized.mesh.clone();
        apply_homotopy(source, incumbent, &patch, &mut candidate, lambda);
        let range = angle_range(&candidate.mesh);
        if !orientations_positive(&candidate.mesh) {
            return Err(failed_evidence(
                &patch,
                materialized.helper_source_vertices,
                Some(lambda),
                range,
                PromotionFailureReason::OrientationGuard,
            ));
        }
        if !is_mixed(&candidate, source.subdivision) {
            return Err(failed_evidence(
                &patch,
                materialized.helper_source_vertices,
                Some(lambda),
                range,
                PromotionFailureReason::NoCompressedExterior,
            ));
        }
        if let Ok(geometry) = Certificate::internal().verify_geometry(&candidate.mesh) {
            return Ok(PromotionTrial {
                mesh: candidate,
                promotion_patch: patch.clone(),
                geometry,
                evidence: success_evidence(
                    &patch,
                    materialized.helper_source_vertices,
                    lambda,
                    range,
                ),
                adaptive: true,
            });
        }
        if lambda < 1.0 {
            continue;
        }
        let elastic_patch = collar_elastic_patch(source, &candidate, incumbent, &patch);
        let outcome = solve_elastic_patch_with_max_min_trust_start(
            &candidate,
            elastic_patch,
            ElasticBlockLimits {
                elastic_iterations: budget.local_geometry_iterations,
            },
            GeometryStartId::MaterializedSource,
        );
        if let ElasticBlockOutcome::Certified(trial) = outcome {
            if !is_mixed(&trial.mesh, source.subdivision) {
                return Err(failed_evidence(
                    &patch,
                    materialized.helper_source_vertices,
                    Some(lambda),
                    Some((
                        trial.geometry.min_angle_degrees,
                        trial.geometry.max_angle_degrees,
                    )),
                    PromotionFailureReason::NoCompressedExterior,
                ));
            }
            let range = Some((
                trial.geometry.min_angle_degrees,
                trial.geometry.max_angle_degrees,
            ));
            return Ok(PromotionTrial {
                mesh: trial.mesh,
                promotion_patch: patch.clone(),
                geometry: trial.geometry,
                evidence: success_evidence(
                    &patch,
                    materialized.helper_source_vertices,
                    lambda,
                    range,
                ),
                adaptive: true,
            });
        }
    }
    Err(failed_evidence(
        &patch,
        materialized.helper_source_vertices,
        Some(1.0),
        angle_range(&materialized.mesh.mesh),
        PromotionFailureReason::GeometryNotCertified,
    ))
}

struct MaterializedCollar {
    mesh: HierarchyLeafMesh,
    helper_source_vertices: usize,
}

fn materialize_collar(
    source: &MotherGrid,
    incumbent: &GeometryFailureWitness,
    atlas: &ViolationSupportAtlas,
    patch: &PromotionPatch,
) -> Result<MaterializedCollar, String> {
    let source_slots = source
        .triangle_addresses
        .iter()
        .enumerate()
        .filter_map(|(face, address)| address.map(|address| (address, face)))
        .collect::<BTreeMap<_, _>>();
    let provenance = atlas
        .custom_triangle_provenance
        .iter()
        .map(|item| {
            (
                canonical_triangle(item.triangle),
                &item.covered_source_faces,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut leaves = BTreeSet::new();
    let mut retained_custom_faces = BTreeSet::new();
    let mut retained_custom_triangles = Vec::new();
    for face in incumbent.mesh.mesh.active_triangle_slots() {
        if let Some(address) = incumbent.mesh.triangle_addresses[face] {
            let coverage = descendant_slots(address, source.subdivision, &source_slots)?;
            if !coverage.is_disjoint(&patch.source_faces) {
                if !coverage.is_subset(&patch.source_faces) {
                    return Err(format!(
                        "promotion boundary cuts hierarchy leaf {address:?}"
                    ));
                }
            } else {
                leaves.insert(address);
            }
            continue;
        }
        let triangle = incumbent.mesh.mesh.triangles()[face].map(|compact| {
            incumbent.mesh.source_vertex_slots[compact]
                .ok_or_else(|| format!("custom face {face} uses a non-source vertex"))
        });
        let [a, b, c] = triangle;
        let triangle = [a?, b?, c?];
        let coverage = provenance
            .get(&canonical_triangle(triangle))
            .ok_or_else(|| format!("custom face {face} has no source provenance"))?;
        if !coverage.is_disjoint(&patch.source_faces) {
            if !coverage.is_subset(&patch.source_faces) {
                return Err(format!(
                    "promotion boundary cuts custom face {face} support"
                ));
            }
        } else {
            retained_custom_faces.extend(coverage.iter().copied());
            retained_custom_triangles.push(triangle);
        }
    }
    for &face in &patch.source_faces {
        leaves.insert(
            source.triangle_addresses[face]
                .ok_or_else(|| format!("promoted source face {face} has no address"))?,
        );
    }
    let leaf_set = HierarchyLeafSet { leaves };
    let mesh = rebuild_from_leaf_set_with_custom_face_slots(
        source,
        &leaf_set,
        &retained_custom_faces,
        &retained_custom_triangles,
    )?;
    let incumbent_sources = incumbent
        .mesh
        .source_vertex_slots
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let helper_source_vertices = mesh
        .source_vertex_slots
        .iter()
        .flatten()
        .filter(|source| !incumbent_sources.contains(source))
        .count();
    Ok(MaterializedCollar {
        mesh,
        helper_source_vertices,
    })
}

fn descendant_slots(
    address: TriangleAddress,
    source_n: usize,
    slots: &BTreeMap<TriangleAddress, usize>,
) -> Result<BTreeSet<usize>, String> {
    let mut frontier = vec![address];
    while frontier.first().is_some_and(|face| face.n < source_n) {
        let mut next = Vec::with_capacity(frontier.len() * 4);
        for face in frontier {
            next.extend(
                face.children_2_to_1()
                    .ok_or_else(|| format!("invalid hierarchy leaf {face:?}"))?,
            );
        }
        frontier = next;
    }
    frontier
        .into_iter()
        .map(|face| {
            slots
                .get(&face)
                .copied()
                .ok_or_else(|| format!("source descendant {face:?} is absent"))
        })
        .collect()
}

fn apply_homotopy(
    source: &MotherGrid,
    incumbent: &GeometryFailureWitness,
    patch: &PromotionPatch,
    target: &mut HierarchyLeafMesh,
    lambda: f64,
) {
    let incumbent_positions = incumbent
        .mesh
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| {
            source.map(|source| (source, incumbent.mesh.mesh.vertices()[compact]))
        })
        .collect::<BTreeMap<_, _>>();
    let boundary = patch
        .boundary_cycles
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let promoted_vertices = patch
        .source_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .collect::<BTreeSet<_>>();
    for (compact, source_slot) in target.source_vertex_slots.iter().copied().enumerate() {
        let Some(source_slot) = source_slot else {
            continue;
        };
        let safe = source.mesh.vertices()[source_slot];
        let incumbent = incumbent_positions.get(&source_slot).copied();
        let point = if boundary.contains(&source_slot) {
            incumbent
                .and_then(|old| interpolate_on_sphere(safe, old, lambda))
                .unwrap_or(safe)
        } else if promoted_vertices.contains(&source_slot) {
            safe
        } else {
            incumbent.unwrap_or(safe)
        };
        target.mesh.move_vertex(compact, point);
    }
}

fn interpolate_on_sphere(
    safe: CartesianPoint,
    incumbent: CartesianPoint,
    lambda: f64,
) -> Option<CartesianPoint> {
    normalize_cartesian_to_radius(
        CartesianPoint::new(
            safe.x * (1.0 - lambda) + incumbent.x * lambda,
            safe.y * (1.0 - lambda) + incumbent.y * lambda,
            safe.z * (1.0 - lambda) + incumbent.z * lambda,
        ),
        1.0,
    )
    .ok()
}

fn collar_elastic_patch(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    incumbent: &GeometryFailureWitness,
    patch: &PromotionPatch,
) -> ElasticPatch {
    let movable_sources = patch
        .collar_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .chain(patch.boundary_cycles.iter().flatten().copied())
        .collect::<BTreeSet<_>>();
    let movable_compact_vertices = mesh
        .source_vertex_slots
        .iter()
        .enumerate()
        .filter_map(|(compact, source)| {
            source
                .is_some_and(|source| movable_sources.contains(&source))
                .then_some(compact)
        })
        .collect::<Vec<_>>();
    let movable = movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    ElasticPatch {
        domain_id: incumbent.patch.domain_id,
        topology: incumbent.patch.topology.clone(),
        reference_positions: mesh.mesh.vertices().to_vec(),
        fixed_compact_vertices: mesh
            .mesh
            .active_vertex_slots()
            .filter(|site| !movable.contains(site))
            .collect(),
        movable_compact_vertices,
        guard_faces: mesh.mesh.active_triangle_slots().collect(),
        target_mode: ElasticTargetMode::TrialReference,
        target_field: Default::default(),
    }
}

fn is_mixed(mesh: &HierarchyLeafMesh, source_n: usize) -> bool {
    mesh.mesh
        .active_triangle_slots()
        .any(|face| mesh.triangle_addresses[face].is_none_or(|address| address.n < source_n))
}

fn safe_fallback(
    source: &MotherGrid,
    component: &ViolationComponent,
    transition_parents: &BTreeSet<TriangleAddress>,
    mut trials: Vec<PromotionTrialEvidence>,
) -> ExpandingCollarResult {
    let patch = match build_promotion_patch_for_transition(
        source,
        component,
        PromotionLevel::P5SafeMotherFallback,
        transition_parents,
    ) {
        Ok(patch) => patch,
        Err(reason) => return invalid(reason),
    };
    let geometry = match Certificate::internal().verify_mother_grid(source) {
        Ok(geometry) => geometry,
        Err(error) => return invalid(format!("safe mother certification failed: {error}")),
    };
    let evidence = PromotionTrialEvidence {
        level: PromotionLevel::P5SafeMotherFallback,
        promoted_source_faces: patch.source_faces.len(),
        collar_source_faces: patch.collar_faces.len(),
        helper_source_vertices: 0,
        homotopy_lambda: Some(0.0),
        angle_range_deg: Some((geometry.min_angle_degrees, geometry.max_angle_degrees)),
        strict: true,
        protected_exterior_preserved: false,
        reason: None,
    };
    trials.push(evidence.clone());
    let mesh = HierarchyLeafMesh {
        mesh: source.mesh.clone(),
        triangle_addresses: source.triangle_addresses.clone(),
        source_vertex_slots: source
            .mesh
            .vertices()
            .iter()
            .enumerate()
            .map(|(site, _)| source.mesh.is_vertex_live(site).then_some(site))
            .collect(),
    };
    ExpandingCollarResult {
        outcome: PromotionOutcome::SafeMotherFallback(Box::new(PromotionTrial {
            mesh,
            promotion_patch: patch,
            geometry,
            evidence,
            adaptive: false,
        })),
        trials,
    }
}

fn orientations_positive(mesh: &earthmesh_mesh::MeshState) -> bool {
    mesh.active_triangle_slots().all(|face| {
        let [a, b, c] = mesh.triangles()[face].map(|site| mesh.vertices()[site]);
        earthmesh_mesh::orientation_on_sphere(a, b, c) == Ok(Sign::Positive)
    })
}

fn angle_range(mesh: &earthmesh_mesh::MeshState) -> Option<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for face in mesh.active_triangle_slots() {
        for angle in
            spherical_triangle_angles(mesh.triangles()[face].map(|site| mesh.vertices()[site]))?
        {
            minimum = minimum.min(angle);
            maximum = maximum.max(angle);
        }
    }
    (minimum.is_finite() && maximum.is_finite()).then_some((minimum, maximum))
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn success_evidence(
    patch: &PromotionPatch,
    helper_source_vertices: usize,
    lambda: f64,
    range: Option<(f64, f64)>,
) -> PromotionTrialEvidence {
    PromotionTrialEvidence {
        level: patch.level,
        promoted_source_faces: patch.source_faces.len(),
        collar_source_faces: patch.collar_faces.len(),
        helper_source_vertices,
        homotopy_lambda: Some(lambda),
        angle_range_deg: range,
        strict: true,
        protected_exterior_preserved: true,
        reason: None,
    }
}

fn failed_evidence(
    patch: &PromotionPatch,
    helper_source_vertices: usize,
    lambda: Option<f64>,
    range: Option<(f64, f64)>,
    reason: PromotionFailureReason,
) -> PromotionTrialEvidence {
    PromotionTrialEvidence {
        level: patch.level,
        promoted_source_faces: patch.source_faces.len(),
        collar_source_faces: patch.collar_faces.len(),
        helper_source_vertices,
        homotopy_lambda: lambda,
        angle_range_deg: range,
        strict: false,
        protected_exterior_preserved: true,
        reason: Some(reason),
    }
}

fn invalid(reason: impl Into<String>) -> ExpandingCollarResult {
    ExpandingCollarResult {
        outcome: PromotionOutcome::InvalidInput(reason.into()),
        trials: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        condense_hierarchy_core, GeometryDomainId, SectorRecoveryAtlas, TransitionTopologyCandidate,
    };

    fn source_leaf_mesh(source: &MotherGrid) -> HierarchyLeafMesh {
        HierarchyLeafMesh {
            mesh: source.mesh.clone(),
            triangle_addresses: source.triangle_addresses.clone(),
            source_vertex_slots: source
                .mesh
                .vertices()
                .iter()
                .enumerate()
                .map(|(site, _)| source.mesh.is_vertex_live(site).then_some(site))
                .collect(),
        }
    }

    fn witness(mesh: HierarchyLeafMesh, component_id: u64) -> GeometryFailureWitness {
        GeometryFailureWitness {
            patch: ElasticPatch {
                domain_id: GeometryDomainId::PlusTwoOrdinaryRings,
                topology: TransitionTopologyCandidate {
                    component_id,
                    topology_id: 0,
                    core_parents: Vec::new(),
                    custom_transition_triangles: BTreeMap::new(),
                    source_triangles: Vec::new(),
                    source_active_vertices: Vec::new(),
                    source_degree_forecast: BTreeMap::new(),
                },
                reference_positions: mesh.mesh.vertices().to_vec(),
                fixed_compact_vertices: Vec::new(),
                movable_compact_vertices: mesh.mesh.active_vertex_slots().collect(),
                guard_faces: mesh.mesh.active_triangle_slots().collect(),
                target_mode: ElasticTargetMode::TrialReference,
                target_field: Default::default(),
            },
            mesh,
        }
    }

    fn fixture(
        source: &MotherGrid,
    ) -> (
        HierarchyComponent,
        ViolationComponent,
        ViolationSupportAtlas,
    ) {
        let face = source.mesh.active_triangle_slots().next().unwrap();
        let parent = source.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .unwrap();
        let violation = ViolationComponent {
            id: 9,
            angles: Vec::new(),
            source_faces: BTreeSet::from([face]),
            parent_faces: BTreeSet::from([parent]),
            support_vertices: source.mesh.triangles()[face].into_iter().collect(),
            active_constraint_vertices: BTreeSet::new(),
        };
        let hierarchy = HierarchyComponent {
            id: 9,
            parents: vec![parent],
            boundary_edges: Vec::new(),
            core_parents: vec![parent],
            transition_parents: vec![parent],
        };
        let atlas = ViolationSupportAtlas {
            total_angles: 0,
            evidence_sets: Default::default(),
            custom_triangle_provenance: Vec::new(),
            sector_recovery_atlas: SectorRecoveryAtlas::default(),
            recovery_atoms: Vec::new(),
            components: vec![violation.clone()],
            patch_expansion_graph: BTreeMap::new(),
            support_inflation: Default::default(),
        };
        (hierarchy, violation, atlas)
    }

    fn budget() -> PromotionBudget {
        PromotionBudget {
            local_topology_states: 1,
            local_geometry_iterations: 1,
            maximum_patch_rings: 2,
            maximum_helper_vertices: 64,
        }
    }

    #[test]
    fn collar_topology_degree_link_euler_charge() {
        let source = MotherGrid::generate(4).unwrap();
        let (hierarchy, violation, atlas) = fixture(&source);
        let incumbent = witness(source_leaf_mesh(&source), hierarchy.id);
        let result = solve_expanding_collar(
            &source,
            &hierarchy,
            &incumbent,
            &atlas,
            &violation,
            budget(),
        );
        let trial = match result.outcome {
            PromotionOutcome::Certified(trial) | PromotionOutcome::SafeMotherFallback(trial) => {
                trial
            }
            _ => panic!("exact source geometry must certify through the finite ladder"),
        };
        assert_eq!(trial.geometry.euler, 2);
        assert_eq!(trial.geometry.charge, 12);
        assert_eq!(trial.geometry.open_edges, 0);
        assert_eq!(trial.geometry.topology_errors, 0);
    }

    #[test]
    fn collar_failure_expands_patch() {
        let source = MotherGrid::generate(4).unwrap();
        let (hierarchy, violation, atlas) = fixture(&source);
        let parent = hierarchy.parents[0];
        let incumbent = witness(
            condense_hierarchy_core(&source, &[parent]).unwrap().mesh,
            hierarchy.id,
        );
        let mut limited = budget();
        limited.maximum_helper_vertices = 1;
        let result =
            solve_expanding_collar(&source, &hierarchy, &incumbent, &atlas, &violation, limited);
        assert!(result.trials.len() > 1);
        assert!(result
            .trials
            .windows(2)
            .all(|pair| pair[0].level < pair[1].level));
    }

    #[test]
    fn full_component_promotion_reaches_safe_mesh() {
        let source = MotherGrid::generate(4).unwrap();
        let (hierarchy, violation, _) = fixture(&source);
        let result = safe_fallback(
            &source,
            &violation,
            &hierarchy.transition_parents.into_iter().collect(),
            Vec::new(),
        );
        let PromotionOutcome::SafeMotherFallback(trial) = result.outcome else {
            panic!("finite expansion must end at the certified safe mother")
        };
        assert!(!trial.adaptive);
        assert_eq!(
            crate::mesh_fingerprint(&trial.mesh.mesh),
            crate::mesh_fingerprint(&source.mesh)
        );
    }
}
