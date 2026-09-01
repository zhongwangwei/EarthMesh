//! Incumbent-preserving release of one boundary coarse parent.

use super::annulus::{parent_by_source_face, parent_graph};
use super::core_condensation::source_face_slot;
use super::direct_restore::{
    closed_edge_incidence, custom_sectors_adjacent_to_parent, logical_exterior_equal_with_custom,
    materialize_sector_restores_with_replacements, mesh_angle_range, outside_coordinates_equal,
};
use super::{
    build_protected_coarse_region, solve_elastic_patch_with_max_min_trust_start,
    ElasticBlockLimits, ElasticBlockOutcome, ElasticPatch, ElasticTargetMode, GeometryStartId,
    HierarchyLeafMesh, SectorRecoveryAtlas,
};
use crate::{Certificate, MotherGrid, TriangleAddress};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryParentEar {
    pub parent: TriangleAddress,
    pub retained_degree: usize,
    pub retained_after_peel: BTreeSet<TriangleAddress>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryParentPeelTrial {
    pub sector_id: u64,
    pub parent: TriangleAddress,
    pub mesh: HierarchyLeafMesh,
    pub restored_sector_ids: BTreeSet<u64>,
    pub split_interface_parents: BTreeSet<TriangleAddress>,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub restored_children: BTreeSet<TriangleAddress>,
    pub helper_source_vertices: usize,
    pub topology_closed: bool,
    pub outside_topology_bitwise_equal: bool,
    pub outside_coordinates_bitwise_equal: bool,
    pub edge_incidence_at_most_two: bool,
    pub angle_range_deg: Option<(f64, f64)>,
    pub local_geometry_attempted: bool,
    pub strict_certified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryParentPeelOutcome {
    Certified(Box<BoundaryParentPeelTrial>),
    GeometryNotCertified {
        trial: Box<BoundaryParentPeelTrial>,
        reason: String,
    },
    TopologyNotClosed {
        trial: Box<BoundaryParentPeelTrial>,
        reason: String,
    },
    NotCoarseCoreEar {
        sector_id: u64,
        parent: TriangleAddress,
    },
    InvalidInput {
        sector_id: u64,
        parent: TriangleAddress,
        reason: String,
    },
}

pub fn coarse_core_ears(
    source: &MotherGrid,
    retained_parents: &BTreeSet<TriangleAddress>,
) -> Result<Vec<BoundaryParentEar>, String> {
    if retained_parents.len() < 2 {
        return Ok(Vec::new());
    }
    let parents_by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let graph = parent_graph(source, &parents_by_face).map_err(|error| format!("{error:?}"))?;
    let mut ears = Vec::new();
    for &parent in retained_parents {
        let neighbours = graph
            .get(&parent)
            .ok_or_else(|| format!("retained parent {parent:?} is absent from the source graph"))?;
        let retained_degree = neighbours.intersection(retained_parents).count();
        if retained_degree > 2 {
            continue;
        }
        let retained_after_peel = retained_parents
            .iter()
            .copied()
            .filter(|candidate| candidate != &parent)
            .collect::<BTreeSet<_>>();
        if retained_after_peel.is_empty()
            || build_protected_coarse_region(source, 0, retained_after_peel.clone()).is_err()
        {
            continue;
        }
        ears.push(BoundaryParentEar {
            parent,
            retained_degree,
            retained_after_peel,
        });
    }
    Ok(ears)
}

#[allow(clippy::too_many_arguments)]
pub fn peel_boundary_parent_for_sector(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    retained_parents: &BTreeSet<TriangleAddress>,
    sector_id: u64,
    parent: TriangleAddress,
    local_geometry_iterations: usize,
) -> BoundaryParentPeelOutcome {
    match peel_boundary_parent_for_sector_inner(
        source,
        incumbent,
        incumbent_patch,
        atlas,
        retained_parents,
        sector_id,
        parent,
        local_geometry_iterations,
    ) {
        Ok(outcome) => outcome,
        Err(reason) => BoundaryParentPeelOutcome::InvalidInput {
            sector_id,
            parent,
            reason,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn peel_boundary_parent_for_sector_inner(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    retained_parents: &BTreeSet<TriangleAddress>,
    sector_id: u64,
    parent: TriangleAddress,
    local_geometry_iterations: usize,
) -> Result<BoundaryParentPeelOutcome, String> {
    if !coarse_core_ears(source, retained_parents)?
        .iter()
        .any(|ear| ear.parent == parent)
    {
        return Ok(BoundaryParentPeelOutcome::NotCoarseCoreEar { sector_id, parent });
    }
    let mut restored_sector_ids =
        custom_sectors_adjacent_to_parent(source, incumbent, atlas, parent)?;
    restored_sector_ids.insert(sector_id);
    let replacements = split_retained_interfaces(source, incumbent, parent, retained_parents)?;
    let split_interface_parents = replacements.keys().copied().collect::<BTreeSet<_>>();
    let replacement_source_faces = split_interface_parents
        .iter()
        .map(|parent| source_child_faces(source, *parent))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    let materialized = materialize_sector_restores_with_replacements(
        source,
        incumbent,
        atlas,
        &restored_sector_ids,
        &BTreeSet::from([parent]),
        &replacements,
    )?;
    if materialized.released_children.len() != 4 {
        return Err(format!(
            "boundary parent peel restored {} children instead of four",
            materialized.released_children.len()
        ));
    }
    let removed_addresses = split_interface_parents
        .iter()
        .copied()
        .chain([parent])
        .collect::<BTreeSet<_>>();
    let parent_faces = incumbent
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            incumbent.triangle_addresses[face]
                .is_some_and(|address| removed_addresses.contains(&address))
        })
        .collect::<BTreeSet<_>>();
    if parent_faces.len() != removed_addresses.len() {
        return Err(format!(
            "local peel has {} hierarchy faces for {} removed parents",
            parent_faces.len(),
            removed_addresses.len()
        ));
    }
    let removed_faces = materialized
        .removed_mixed_faces
        .union(&parent_faces)
        .copied()
        .collect::<BTreeSet<_>>();
    let inserted_addresses = materialized
        .restored_addresses
        .union(&materialized.released_children)
        .copied()
        .collect::<BTreeSet<_>>();
    let context = PeelTrialContext {
        sector_id,
        parent,
        incumbent,
        restored_sector_ids,
        split_interface_parents,
        replacement_source_faces,
        removed_addresses,
        removed_faces,
        removed_mixed_faces: materialized.removed_mixed_faces,
        restored_source_faces: materialized.restored_source_faces,
        restored_children: materialized.released_children,
        inserted_addresses,
        inserted_custom_triangles: materialized.inserted_custom_triangles,
        helper_source_vertices: materialized.helper_source_vertices,
    };
    let candidate = materialized.mesh;
    let make_trial =
        |mesh: HierarchyLeafMesh, local_geometry_attempted: bool, strict_certified: bool| {
            peel_trial(&context, mesh, local_geometry_attempted, strict_certified)
        };
    let topology_trial = make_trial(candidate.clone(), false, false);
    if !topology_trial.topology_closed
        || !topology_trial.outside_topology_bitwise_equal
        || !topology_trial.outside_coordinates_bitwise_equal
        || !topology_trial.edge_incidence_at_most_two
    {
        return Ok(BoundaryParentPeelOutcome::TopologyNotClosed {
            trial: Box::new(topology_trial),
            reason: "local peel did not preserve a closed exact exterior".into(),
        });
    }
    if Certificate::internal()
        .verify_geometry(&candidate.mesh)
        .is_ok()
    {
        return Ok(BoundaryParentPeelOutcome::Certified(Box::new(make_trial(
            candidate, false, true,
        ))));
    }
    if local_geometry_iterations == 0 {
        return Ok(BoundaryParentPeelOutcome::GeometryNotCertified {
            trial: Box::new(topology_trial),
            reason: "local geometry was not requested".into(),
        });
    }
    let movable_source_vertices = cavity_interior_vertices(source, &context)?;
    let movable_compact_vertices = candidate
        .source_vertex_slots
        .iter()
        .enumerate()
        .filter_map(|(compact, source_site)| {
            source_site
                .is_some_and(|site| movable_source_vertices.contains(&site))
                .then_some(compact)
        })
        .collect::<Vec<_>>();
    if movable_compact_vertices.is_empty() {
        return Ok(BoundaryParentPeelOutcome::GeometryNotCertified {
            trial: Box::new(topology_trial),
            reason: "local peel cavity has no interior movable vertex".into(),
        });
    }
    let movable = movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let patch = ElasticPatch {
        domain_id: incumbent_patch.domain_id,
        topology: incumbent_patch.topology.clone(),
        reference_positions: candidate.mesh.vertices().to_vec(),
        fixed_compact_vertices: candidate
            .mesh
            .active_vertex_slots()
            .filter(|site| !movable.contains(site))
            .collect(),
        movable_compact_vertices,
        guard_faces: candidate.mesh.active_triangle_slots().collect(),
        target_mode: ElasticTargetMode::TrialReference,
        target_field: Default::default(),
    };
    let outcome = solve_elastic_patch_with_max_min_trust_start(
        &candidate,
        patch,
        ElasticBlockLimits {
            elastic_iterations: local_geometry_iterations,
        },
        GeometryStartId::MaterializedSource,
    );
    Ok(match outcome {
        ElasticBlockOutcome::Certified(trial) => {
            BoundaryParentPeelOutcome::Certified(Box::new(make_trial(trial.mesh, true, true)))
        }
        ElasticBlockOutcome::ElasticNoImprovement {
            witness, reason, ..
        }
        | ElasticBlockOutcome::SearchBudgetExhausted {
            witness, reason, ..
        }
        | ElasticBlockOutcome::RequiresDifferentTopology {
            witness, reason, ..
        } => BoundaryParentPeelOutcome::GeometryNotCertified {
            trial: Box::new(make_trial(witness.mesh, true, false)),
            reason,
        },
        ElasticBlockOutcome::InvalidPatch { reason } => {
            BoundaryParentPeelOutcome::GeometryNotCertified {
                trial: Box::new(make_trial(candidate, true, false)),
                reason,
            }
        }
    })
}

struct PeelTrialContext<'a> {
    sector_id: u64,
    parent: TriangleAddress,
    incumbent: &'a HierarchyLeafMesh,
    restored_sector_ids: BTreeSet<u64>,
    split_interface_parents: BTreeSet<TriangleAddress>,
    replacement_source_faces: BTreeSet<usize>,
    removed_addresses: BTreeSet<TriangleAddress>,
    removed_faces: BTreeSet<usize>,
    removed_mixed_faces: BTreeSet<usize>,
    restored_source_faces: BTreeSet<usize>,
    restored_children: BTreeSet<TriangleAddress>,
    inserted_addresses: BTreeSet<TriangleAddress>,
    inserted_custom_triangles: BTreeSet<[usize; 3]>,
    helper_source_vertices: usize,
}

fn peel_trial(
    context: &PeelTrialContext<'_>,
    mesh: HierarchyLeafMesh,
    local_geometry_attempted: bool,
    strict_certified: bool,
) -> BoundaryParentPeelTrial {
    let topology_closed = mesh.mesh.open_edge_count() == 0;
    let outside_topology_bitwise_equal = logical_exterior_equal_with_custom(
        context.incumbent,
        &context.removed_faces,
        &context.removed_addresses,
        &mesh,
        &context.inserted_addresses,
        &context.inserted_custom_triangles,
    );
    let outside_coordinates_bitwise_equal =
        outside_coordinates_equal(context.incumbent, &context.removed_faces, &mesh);
    let edge_incidence_at_most_two = closed_edge_incidence(&mesh);
    let angle_range_deg = mesh_angle_range(&mesh);
    BoundaryParentPeelTrial {
        sector_id: context.sector_id,
        parent: context.parent,
        mesh,
        restored_sector_ids: context.restored_sector_ids.clone(),
        split_interface_parents: context.split_interface_parents.clone(),
        removed_mixed_faces: context.removed_mixed_faces.clone(),
        restored_source_faces: context.restored_source_faces.clone(),
        restored_children: context.restored_children.clone(),
        helper_source_vertices: context.helper_source_vertices,
        topology_closed,
        outside_topology_bitwise_equal,
        outside_coordinates_bitwise_equal,
        edge_incidence_at_most_two,
        angle_range_deg,
        local_geometry_attempted,
        strict_certified,
    }
}

fn cavity_interior_vertices(
    source: &MotherGrid,
    context: &PeelTrialContext<'_>,
) -> Result<BTreeSet<usize>, String> {
    let cavity_faces = context
        .restored_source_faces
        .iter()
        .copied()
        .chain(
            context
                .restored_children
                .iter()
                .map(|&address| source_face_slot(source, address))
                .collect::<Result<Vec<_>, _>>()?,
        )
        .chain(context.replacement_source_faces.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut edge_counts = BTreeMap::<Edge, usize>::new();
    for &face in &cavity_faces {
        let triangle = source.mesh.triangles()[face];
        for (left, right) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            *edge_counts
                .entry((left.min(right), left.max(right)))
                .or_default() += 1;
        }
    }
    let boundary = edge_counts
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .flat_map(|(edge, _)| [edge.0, edge.1])
        .collect::<BTreeSet<_>>();
    Ok(cavity_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .filter(|site| !boundary.contains(site))
        .collect())
}

fn split_retained_interfaces(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    parent: TriangleAddress,
    retained_parents: &BTreeSet<TriangleAddress>,
) -> Result<BTreeMap<TriangleAddress, Vec<[usize; 3]>>, String> {
    let parent_face = active_parent_face(incumbent, parent)?;
    let parent_triangle = compact_source_triangle(incumbent, parent_face)?;
    let parent_vertices = source_child_vertices(source, parent)?;
    let mut replacements = BTreeMap::new();
    for neighbour_face in incumbent.mesh.neighbours()[parent_face] {
        let Some(neighbour) = incumbent
            .triangle_addresses
            .get(neighbour_face)
            .copied()
            .flatten()
            .filter(|candidate| candidate != &parent && retained_parents.contains(candidate))
        else {
            continue;
        };
        let neighbour_triangle = compact_source_triangle(incumbent, neighbour_face)?;
        let shared_endpoints = parent_triangle
            .iter()
            .copied()
            .filter(|site| neighbour_triangle.contains(site))
            .collect::<Vec<_>>();
        if shared_endpoints.len() != 2 {
            return Err(format!(
                "retained interface {parent:?}/{neighbour:?} has {} coarse endpoints",
                shared_endpoints.len()
            ));
        }
        let neighbour_vertices = source_child_vertices(source, neighbour)?;
        let midpoint = parent_vertices
            .intersection(&neighbour_vertices)
            .copied()
            .filter(|site| !shared_endpoints.contains(site))
            .collect::<Vec<_>>();
        if midpoint.len() != 1 {
            return Err(format!(
                "retained interface {parent:?}/{neighbour:?} has {} source midpoints",
                midpoint.len()
            ));
        }
        let opposite = neighbour_triangle
            .iter()
            .copied()
            .find(|site| !shared_endpoints.contains(site))
            .ok_or_else(|| format!("retained neighbour {neighbour:?} has no opposite corner"))?;
        replacements.insert(
            neighbour,
            vec![
                [shared_endpoints[0], midpoint[0], opposite],
                [midpoint[0], shared_endpoints[1], opposite],
            ],
        );
    }
    Ok(replacements)
}

fn active_parent_face(mesh: &HierarchyLeafMesh, parent: TriangleAddress) -> Result<usize, String> {
    let faces = mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| mesh.triangle_addresses[face] == Some(parent))
        .collect::<Vec<_>>();
    match faces.as_slice() {
        [face] => Ok(*face),
        _ => Err(format!(
            "boundary parent {parent:?} has {} incumbent faces",
            faces.len()
        )),
    }
}

fn compact_source_triangle(mesh: &HierarchyLeafMesh, face: usize) -> Result<[usize; 3], String> {
    mesh.mesh.triangles()[face]
        .map(|compact| {
            mesh.source_vertex_slots[compact]
                .ok_or_else(|| format!("hierarchy face {face} uses a non-source vertex"))
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "triangle conversion failed".into())
}

fn source_child_faces(source: &MotherGrid, parent: TriangleAddress) -> Result<Vec<usize>, String> {
    parent
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?
        .into_iter()
        .map(|child| source_face_slot(source, child))
        .collect()
}

fn source_child_vertices(
    source: &MotherGrid,
    parent: TriangleAddress,
) -> Result<BTreeSet<usize>, String> {
    Ok(source_child_faces(source, parent)?
        .into_iter()
        .flat_map(|face| source.mesh.triangles()[face])
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        build_sector_recovery_atlas, build_stratified_annulus, n6_legacy_mixed_fixture,
        restore_fine_compatible_sector, DirectSectorRestoreOutcome, FullPolygonMergeLimits,
        FullPolygonMergeOutcome, GeometryDomainId, TransitionTopologyCandidate,
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
    fn one_boundary_parent_peel_closes_without_exterior_drift() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let stratified = build_stratified_annulus(&source, &component).unwrap();
        let FullPolygonMergeOutcome::Closed(topology) = super::super::solve_full_polygon_merge(
            &source,
            &component,
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
        let retained = component
            .core_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let ears = coarse_core_ears(&source, &retained)
            .unwrap()
            .into_iter()
            .map(|ear| ear.parent)
            .collect::<BTreeSet<_>>();
        let closed = atlas.sectors.keys().any(|&sector_id| {
            let DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                adjacent_parents, ..
            } = restore_fine_compatible_sector(&source, &mesh, &patch, &atlas, sector_id, 0)
            else {
                return false;
            };
            adjacent_parents.intersection(&ears).any(|&parent| {
                let outcome = peel_boundary_parent_for_sector(
                    &source, &mesh, &patch, &atlas, &retained, sector_id, parent, 0,
                );
                match outcome {
                    BoundaryParentPeelOutcome::Certified(trial)
                    | BoundaryParentPeelOutcome::GeometryNotCertified { trial, .. } => {
                        trial.topology_closed
                            && trial.outside_topology_bitwise_equal
                            && trial.outside_coordinates_bitwise_equal
                            && trial.edge_incidence_at_most_two
                            && trial.restored_children.len() == 4
                            && !trial.split_interface_parents.is_empty()
                    }
                    _ => false,
                }
            })
        });
        assert!(closed, "at least one boundary parent peel must close");
    }
}
