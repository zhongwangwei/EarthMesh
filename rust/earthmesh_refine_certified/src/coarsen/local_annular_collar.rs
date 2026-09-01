//! Finite one/two-parent local collar around a protected coarse core.

use super::boundary_parent_peel::{source_child_faces, split_retained_interfaces};
use super::direct_restore::{
    closed_edge_incidence, custom_sectors_adjacent_to_parent, logical_exterior_equal_with_custom,
    materialize_sector_restores_with_replacements, mesh_angle_range, outside_coordinates_equal,
};
use super::{
    build_protected_coarse_region, coarse_core_ears, solve_elastic_patch_with_max_min_trust_start,
    ElasticBlockLimits, ElasticBlockOutcome, ElasticPatch, ElasticTargetMode, GeometryStartId,
    HierarchyLeafMesh, LocalRecoveryComponent, ProtectedCoarseRegion, RecoveryAtom,
    SectorRecoveryAtlas,
};
use crate::{Certificate, MotherGrid, TriangleAddress};
use earthmesh_mesh::{normalize_cartesian_to_radius, CartesianPoint};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalAnnularCollarLevel {
    OneParentPeel,
    TwoParentPeel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalAnnularCollarLimits {
    pub topology_states: usize,
    pub geometry_iterations: usize,
    pub maximum_parent_peels: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalAnnularCollarEvidence {
    pub level: LocalAnnularCollarLevel,
    pub target_sector_id: u64,
    pub restored_sector_ids: BTreeSet<u64>,
    pub released_parents: BTreeSet<TriangleAddress>,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub protected_regions: usize,
    pub promoted_source_faces: usize,
    pub split_interface_parents: usize,
    pub topology_closed: bool,
    pub protected_core_preserved: bool,
    pub outside_topology_bitwise_equal: bool,
    pub outside_coordinates_bitwise_equal: bool,
    pub fixed_outside_link_contracts: bool,
    pub edge_incidence_at_most_two: bool,
    pub homotopy_lambda: Option<f64>,
    pub angle_range_deg: Option<(f64, f64)>,
    pub local_geometry_attempted: bool,
    pub strict_certified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalAnnularCollarTrial {
    pub mesh: HierarchyLeafMesh,
    pub evidence: LocalAnnularCollarEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalAnnularCollarOutcome {
    Certified(Box<LocalAnnularCollarTrial>),
    MaterializedNotCertified {
        best: Box<LocalAnnularCollarTrial>,
        trials: Vec<LocalAnnularCollarEvidence>,
    },
    TopologyFamilyExhausted {
        trials: Vec<LocalAnnularCollarEvidence>,
    },
    SearchBudgetExhausted {
        trials: Vec<LocalAnnularCollarEvidence>,
    },
    InvalidInput(String),
}

#[allow(clippy::too_many_arguments)]
pub fn solve_local_annular_collar(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    component: &LocalRecoveryComponent,
    retained_parents: &BTreeSet<TriangleAddress>,
    target_sector_id: u64,
    adjacent_parents: &BTreeSet<TriangleAddress>,
    limits: LocalAnnularCollarLimits,
) -> LocalAnnularCollarOutcome {
    match solve_local_annular_collar_inner(
        source,
        incumbent,
        incumbent_patch,
        atlas,
        component,
        retained_parents,
        target_sector_id,
        adjacent_parents,
        limits,
    ) {
        Ok(outcome) => outcome,
        Err(reason) => LocalAnnularCollarOutcome::InvalidInput(reason),
    }
}

#[allow(clippy::too_many_arguments)]
fn solve_local_annular_collar_inner(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    component: &LocalRecoveryComponent,
    retained_parents: &BTreeSet<TriangleAddress>,
    target_sector_id: u64,
    adjacent_parents: &BTreeSet<TriangleAddress>,
    limits: LocalAnnularCollarLimits,
) -> Result<LocalAnnularCollarOutcome, String> {
    if limits.topology_states == 0 || limits.maximum_parent_peels == 0 {
        return Err("local annular collar requires positive topology and parent budgets".into());
    }
    if adjacent_parents.is_empty()
        || adjacent_parents.len() > limits.maximum_parent_peels.min(2)
        || !adjacent_parents.is_subset(retained_parents)
    {
        return Err("local annular collar has invalid adjacent coarse parents".into());
    }
    if component.protected_coarse_regions.is_empty()
        || !component.atoms.iter().any(|atom| {
            matches!(atom, RecoveryAtom::Sector { sector_id, .. } if *sector_id == target_sector_id)
        })
    {
        return Err("target sector is not in a protected local recovery component".into());
    }
    let ears = coarse_core_ears(source, retained_parents)?
        .into_iter()
        .map(|ear| ear.parent)
        .collect::<BTreeSet<_>>();
    let mut candidates = adjacent_parents
        .intersection(&ears)
        .copied()
        .map(|parent| BTreeSet::from([parent]))
        .collect::<Vec<_>>();
    if adjacent_parents.len() == 2 && limits.maximum_parent_peels >= 2 {
        candidates.push(adjacent_parents.clone());
    }
    candidates.sort();
    candidates.dedup();
    let budget_exhausted = candidates.len() > limits.topology_states;
    candidates.truncate(limits.topology_states);

    let mut evidence = Vec::new();
    let mut best = None::<LocalAnnularCollarTrial>;
    for released_parents in candidates {
        let trial = match materialize_collar_trial(
            source,
            incumbent,
            incumbent_patch,
            atlas,
            component,
            retained_parents,
            target_sector_id,
            &released_parents,
            limits.geometry_iterations,
        ) {
            Ok(trial) => trial,
            Err(failed) => {
                evidence.push(*failed);
                continue;
            }
        };
        evidence.push(trial.evidence.clone());
        if trial.evidence.strict_certified {
            return Ok(LocalAnnularCollarOutcome::Certified(Box::new(trial)));
        }
        if best.as_ref().is_none_or(|current| {
            collar_margin(trial.evidence.angle_range_deg)
                > collar_margin(current.evidence.angle_range_deg)
        }) {
            best = Some(trial);
        }
    }
    Ok(if let Some(best) = best {
        LocalAnnularCollarOutcome::MaterializedNotCertified {
            best: Box::new(best),
            trials: evidence,
        }
    } else if budget_exhausted {
        LocalAnnularCollarOutcome::SearchBudgetExhausted { trials: evidence }
    } else {
        LocalAnnularCollarOutcome::TopologyFamilyExhausted { trials: evidence }
    })
}

#[allow(clippy::too_many_arguments)]
fn materialize_collar_trial(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    component: &LocalRecoveryComponent,
    retained_parents: &BTreeSet<TriangleAddress>,
    target_sector_id: u64,
    released_parents: &BTreeSet<TriangleAddress>,
    geometry_iterations: usize,
) -> Result<LocalAnnularCollarTrial, Box<LocalAnnularCollarEvidence>> {
    let level = if released_parents.len() == 1 {
        LocalAnnularCollarLevel::OneParentPeel
    } else {
        LocalAnnularCollarLevel::TwoParentPeel
    };
    let retained_after_release = retained_parents
        .difference(released_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    if retained_after_release.is_empty() {
        return Err(Box::new(empty_evidence(
            level,
            target_sector_id,
            released_parents,
            retained_after_release,
        )));
    }
    let mut restored_sector_ids = BTreeSet::from([target_sector_id]);
    for &parent in released_parents {
        match custom_sectors_adjacent_to_parent(source, incumbent, atlas, parent) {
            Ok(sectors) => restored_sector_ids.extend(sectors),
            Err(_) => {
                return Err(Box::new(empty_evidence(
                    level,
                    target_sector_id,
                    released_parents,
                    retained_after_release.clone(),
                )))
            }
        }
    }
    let replacements =
        match split_retained_interfaces(source, incumbent, released_parents, retained_parents) {
            Ok(replacements) => replacements,
            Err(_) => {
                return Err(Box::new(empty_evidence(
                    level,
                    target_sector_id,
                    released_parents,
                    retained_after_release.clone(),
                )))
            }
        };
    let split_interface_parents = replacements.keys().copied().collect::<BTreeSet<_>>();
    let modified_parents = released_parents
        .union(&split_interface_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    let retained_after = retained_parents
        .difference(&modified_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    let protected_regions = match rebuild_protected_regions(source, component, &modified_parents) {
        Ok(regions) if !retained_after.is_empty() && !regions.is_empty() => regions,
        _ => {
            return Err(Box::new(empty_evidence(
                level,
                target_sector_id,
                released_parents,
                retained_after,
            )))
        }
    };
    let materialized = match materialize_sector_restores_with_replacements(
        source,
        incumbent,
        atlas,
        &restored_sector_ids,
        released_parents,
        &replacements,
    ) {
        Ok(materialized) => materialized,
        Err(_) => {
            return Err(Box::new(empty_evidence(
                level,
                target_sector_id,
                released_parents,
                retained_after,
            )))
        }
    };
    let removed_addresses = released_parents
        .union(&split_interface_parents)
        .copied()
        .collect::<BTreeSet<_>>();
    let hierarchy_faces = incumbent
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            incumbent.triangle_addresses[face]
                .is_some_and(|address| removed_addresses.contains(&address))
        })
        .collect::<BTreeSet<_>>();
    let removed_faces = materialized
        .removed_mixed_faces
        .union(&hierarchy_faces)
        .copied()
        .collect::<BTreeSet<_>>();
    let inserted_addresses = materialized
        .restored_addresses
        .union(&materialized.released_children)
        .copied()
        .collect::<BTreeSet<_>>();
    let cavity_source_faces = match cavity_source_faces(
        source,
        &materialized.restored_source_faces,
        &materialized.released_children,
        &split_interface_parents,
    ) {
        Ok(faces) => faces,
        Err(_) => {
            return Err(Box::new(empty_evidence(
                level,
                target_sector_id,
                released_parents,
                retained_after,
            )))
        }
    };
    let protected_core_preserved = protected_regions.iter().all(|region| {
        region
            .descendant_source_faces
            .is_disjoint(&cavity_source_faces)
    });
    let candidate = materialized.mesh;
    let outside_topology_bitwise_equal = logical_exterior_equal_with_custom(
        incumbent,
        &removed_faces,
        &removed_addresses,
        &candidate,
        &inserted_addresses,
        &materialized.inserted_custom_triangles,
    );
    let outside_coordinates_bitwise_equal =
        outside_coordinates_equal(incumbent, &removed_faces, &candidate);
    let topology_closed = candidate.mesh.open_edge_count() == 0;
    let edge_incidence_at_most_two = closed_edge_incidence(&candidate);
    let fixed_outside_link_contracts = outside_topology_bitwise_equal;
    let base_evidence = LocalAnnularCollarEvidence {
        level,
        target_sector_id,
        restored_sector_ids,
        released_parents: released_parents.clone(),
        retained_parents: retained_after,
        protected_regions: protected_regions.len(),
        promoted_source_faces: cavity_source_faces.len(),
        split_interface_parents: split_interface_parents.len(),
        topology_closed,
        protected_core_preserved,
        outside_topology_bitwise_equal,
        outside_coordinates_bitwise_equal,
        fixed_outside_link_contracts,
        edge_incidence_at_most_two,
        homotopy_lambda: None,
        angle_range_deg: mesh_angle_range(&candidate),
        local_geometry_attempted: false,
        strict_certified: false,
    };
    if !topology_closed
        || !protected_core_preserved
        || !outside_topology_bitwise_equal
        || !outside_coordinates_bitwise_equal
        || !edge_incidence_at_most_two
    {
        return Err(Box::new(base_evidence));
    }
    let movable_sources = cavity_interior_vertices(source, &cavity_source_faces);
    let mut best_mesh = candidate.clone();
    let mut best_evidence = base_evidence.clone();
    for lambda in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut trial = candidate.clone();
        apply_homotopy(source, incumbent, &movable_sources, &mut trial, lambda);
        let range = mesh_angle_range(&trial);
        if collar_margin(range) > collar_margin(best_evidence.angle_range_deg) {
            best_mesh = trial.clone();
            best_evidence.angle_range_deg = range;
            best_evidence.homotopy_lambda = Some(lambda);
        }
        if Certificate::internal().verify_geometry(&trial.mesh).is_ok() {
            let mut certified = base_evidence;
            certified.angle_range_deg = range;
            certified.homotopy_lambda = Some(lambda);
            certified.strict_certified = true;
            return Ok(LocalAnnularCollarTrial {
                mesh: trial,
                evidence: certified,
            });
        }
    }
    if geometry_iterations == 0 || movable_sources.is_empty() {
        return Ok(LocalAnnularCollarTrial {
            mesh: best_mesh,
            evidence: best_evidence,
        });
    }
    let movable_compact_vertices = best_mesh
        .source_vertex_slots
        .iter()
        .enumerate()
        .filter_map(|(compact, source_site)| {
            source_site
                .is_some_and(|source_site| movable_sources.contains(&source_site))
                .then_some(compact)
        })
        .collect::<Vec<_>>();
    let movable = movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let elastic_patch = ElasticPatch {
        domain_id: incumbent_patch.domain_id,
        topology: incumbent_patch.topology.clone(),
        reference_positions: best_mesh.mesh.vertices().to_vec(),
        fixed_compact_vertices: best_mesh
            .mesh
            .active_vertex_slots()
            .filter(|site| !movable.contains(site))
            .collect(),
        movable_compact_vertices,
        guard_faces: best_mesh.mesh.active_triangle_slots().collect(),
        target_mode: ElasticTargetMode::TrialReference,
        target_field: Default::default(),
    };
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &best_mesh,
        elastic_patch,
        ElasticBlockLimits {
            elastic_iterations: geometry_iterations,
        },
        GeometryStartId::MaterializedSource,
    );
    let (mesh, strict) = match outcome {
        ElasticBlockOutcome::Certified(trial) => (trial.mesh, true),
        ElasticBlockOutcome::ElasticNoImprovement { witness, .. }
        | ElasticBlockOutcome::SearchBudgetExhausted { witness, .. }
        | ElasticBlockOutcome::RequiresDifferentTopology { witness, .. } => (witness.mesh, false),
        ElasticBlockOutcome::InvalidPatch { .. } => (best_mesh, false),
    };
    best_evidence.angle_range_deg = mesh_angle_range(&mesh);
    best_evidence.local_geometry_attempted = true;
    best_evidence.strict_certified = strict;
    Ok(LocalAnnularCollarTrial {
        mesh,
        evidence: best_evidence,
    })
}

fn rebuild_protected_regions(
    source: &MotherGrid,
    component: &LocalRecoveryComponent,
    released_parents: &BTreeSet<TriangleAddress>,
) -> Result<Vec<ProtectedCoarseRegion>, String> {
    component
        .protected_coarse_regions
        .iter()
        .filter_map(|region| {
            let retained = region
                .retained_parents
                .difference(released_parents)
                .copied()
                .collect::<BTreeSet<_>>();
            (!retained.is_empty())
                .then(|| build_protected_coarse_region(source, region.id, retained))
        })
        .collect()
}

fn cavity_source_faces(
    source: &MotherGrid,
    restored_source_faces: &BTreeSet<usize>,
    released_children: &BTreeSet<TriangleAddress>,
    split_interface_parents: &BTreeSet<TriangleAddress>,
) -> Result<BTreeSet<usize>, String> {
    let mut faces = restored_source_faces.clone();
    for &child in released_children {
        faces.insert(super::core_condensation::source_face_slot(source, child)?);
    }
    for &parent in split_interface_parents {
        faces.extend(source_child_faces(source, parent)?);
    }
    Ok(faces)
}

fn cavity_interior_vertices(
    source: &MotherGrid,
    cavity_faces: &BTreeSet<usize>,
) -> BTreeSet<usize> {
    let mut counts = BTreeMap::<Edge, usize>::new();
    for &face in cavity_faces {
        let triangle = source.mesh.triangles()[face];
        for (left, right) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            *counts
                .entry((left.min(right), left.max(right)))
                .or_default() += 1;
        }
    }
    let boundary = counts
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .flat_map(|(edge, _)| [edge.0, edge.1])
        .collect::<BTreeSet<_>>();
    cavity_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .filter(|site| !boundary.contains(site))
        .collect()
}

fn apply_homotopy(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    movable_sources: &BTreeSet<usize>,
    target: &mut HierarchyLeafMesh,
    lambda: f64,
) {
    let old = incumbent
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| {
            source.map(|source| (source, incumbent.mesh.vertices()[compact]))
        })
        .collect::<BTreeMap<_, _>>();
    for (compact, source_site) in target.source_vertex_slots.iter().copied().enumerate() {
        let Some(source_site) = source_site.filter(|site| movable_sources.contains(site)) else {
            continue;
        };
        let safe = source.mesh.vertices()[source_site];
        let point = old
            .get(&source_site)
            .and_then(|old| interpolate_on_sphere(safe, *old, lambda))
            .unwrap_or(safe);
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

fn collar_margin(range: Option<(f64, f64)>) -> f64 {
    range.map_or(f64::NEG_INFINITY, |range| {
        (range.0 - 40.2).min(79.8 - range.1)
    })
}

fn empty_evidence(
    level: LocalAnnularCollarLevel,
    target_sector_id: u64,
    released_parents: &BTreeSet<TriangleAddress>,
    retained_parents: BTreeSet<TriangleAddress>,
) -> LocalAnnularCollarEvidence {
    LocalAnnularCollarEvidence {
        level,
        target_sector_id,
        restored_sector_ids: BTreeSet::new(),
        released_parents: released_parents.clone(),
        retained_parents,
        protected_regions: 0,
        promoted_source_faces: 0,
        split_interface_parents: 0,
        topology_closed: false,
        protected_core_preserved: false,
        outside_topology_bitwise_equal: false,
        outside_coordinates_bitwise_equal: false,
        fixed_outside_link_contracts: false,
        edge_incidence_at_most_two: false,
        homotopy_lambda: None,
        angle_range_deg: None,
        local_geometry_attempted: false,
        strict_certified: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        build_protected_coarse_region, build_sector_recovery_atlas, build_stratified_annulus,
        n6_legacy_mixed_fixture, restore_fine_compatible_sector, DirectSectorRestoreOutcome,
        FullPolygonMergeLimits, FullPolygonMergeOutcome, GeometryDomainId, PromotionPatchTopology,
        TransitionTopologyCandidate,
    };

    fn patch(mesh: &HierarchyLeafMesh) -> ElasticPatch {
        ElasticPatch {
            domain_id: GeometryDomainId::PlusTwoOrdinaryRings,
            topology: TransitionTopologyCandidate {
                component_id: 0,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: Vec::new(),
                source_active_vertices: Vec::new(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: mesh.mesh.vertices().to_vec(),
            fixed_compact_vertices: mesh.mesh.active_vertex_slots().collect(),
            movable_compact_vertices: Vec::new(),
            guard_faces: mesh.mesh.active_triangle_slots().collect(),
            target_mode: ElasticTargetMode::TrialReference,
            target_field: Default::default(),
        }
    }

    #[test]
    fn two_parent_collar_closes_and_preserves_the_protected_core() {
        let (source, hierarchy) = n6_legacy_mixed_fixture().unwrap();
        let stratified = build_stratified_annulus(&source, &hierarchy).unwrap();
        let FullPolygonMergeOutcome::Closed(topology) = super::super::solve_full_polygon_merge(
            &source,
            &hierarchy,
            FullPolygonMergeLimits {
                topology_states: 500,
            },
        ) else {
            panic!("Frozen N6 full-polygon topology must close")
        };
        let atlas = build_sector_recovery_atlas(
            &source,
            &stratified,
            &topology.evidence.selected_topology_keys,
        )
        .unwrap();
        let mesh = topology.global_trial.mesh;
        let patch = patch(&mesh);
        let retained = hierarchy
            .core_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let protected = build_protected_coarse_region(&source, 0, retained.clone()).unwrap();
        let (sector_id, adjacent_parents) = atlas
            .sectors
            .keys()
            .find_map(|&sector_id| {
                let DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                    adjacent_parents, ..
                } = restore_fine_compatible_sector(&source, &mesh, &patch, &atlas, sector_id, 0)
                else {
                    return None;
                };
                (adjacent_parents.len() == 2).then_some((sector_id, adjacent_parents))
            })
            .expect("Frozen N6 has a two-parent blocked sector");
        let coverage = &atlas.sectors[&sector_id];
        let component = LocalRecoveryComponent {
            id: 0,
            atoms: vec![RecoveryAtom::Sector {
                sector_id,
                mixed_faces: BTreeSet::new(),
                source_faces: coverage.source_faces.clone(),
            }],
            mixed_faces: BTreeSet::new(),
            source_faces: coverage.source_faces.clone(),
            boundary_cycles: coverage.boundary_cycles.clone(),
            protected_coarse_regions: vec![protected],
            topology: PromotionPatchTopology::Annulus {
                protected_hole_id: 0,
            },
        };
        let outcome = solve_local_annular_collar(
            &source,
            &mesh,
            &patch,
            &atlas,
            &component,
            &retained,
            sector_id,
            &adjacent_parents,
            LocalAnnularCollarLimits {
                topology_states: 3,
                geometry_iterations: 0,
                maximum_parent_peels: 2,
            },
        );
        let LocalAnnularCollarOutcome::MaterializedNotCertified { best, .. } = outcome else {
            match outcome {
                LocalAnnularCollarOutcome::TopologyFamilyExhausted { trials }
                | LocalAnnularCollarOutcome::SearchBudgetExhausted { trials } => {
                    panic!("two-parent collar did not materialize: {trials:?}")
                }
                LocalAnnularCollarOutcome::InvalidInput(reason) => panic!("{reason}"),
                LocalAnnularCollarOutcome::Certified(_) => {
                    panic!("fixture unexpectedly certified")
                }
                LocalAnnularCollarOutcome::MaterializedNotCertified { .. } => unreachable!(),
            }
        };
        assert_eq!(best.evidence.level, LocalAnnularCollarLevel::TwoParentPeel);
        assert!(best.evidence.topology_closed);
        assert!(best.evidence.protected_core_preserved);
        assert!(best.evidence.fixed_outside_link_contracts);
        assert!(best.evidence.outside_coordinates_bitwise_equal);
        assert!(best.evidence.edge_incidence_at_most_two);
    }
}
