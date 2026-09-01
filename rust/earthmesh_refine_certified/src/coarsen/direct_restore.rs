//! Incumbent-preserving direct restoration of exact fine-compatible sectors.

use super::core_condensation::{rebuild_from_leaf_set_with_custom_face_slots, HierarchyLeafSet};
use super::{
    solve_elastic_patch_with_max_min_trust_start, ElasticBlockLimits, ElasticBlockOutcome,
    ElasticPatch, ElasticTargetMode, GeometryStartId, HierarchyLeafMesh, SectorRecoveryAtlas,
};
use crate::certificate::{spherical_triangle_angles, Certificate};
use crate::{MotherGrid, TriangleAddress};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq)]
pub struct DirectSectorRestoreTrial {
    pub sector_id: u64,
    pub mesh: HierarchyLeafMesh,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub helper_source_vertices: usize,
    pub outside_topology_bitwise_equal: bool,
    pub outside_coordinates_bitwise_equal: bool,
    pub edge_incidence_at_most_two: bool,
    pub angle_range_deg: Option<(f64, f64)>,
    pub local_geometry_attempted: bool,
    pub strict_certified: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DirectSectorRestoreOutcome {
    Certified(Box<DirectSectorRestoreTrial>),
    GeometryNotCertified {
        trial: Box<DirectSectorRestoreTrial>,
        reason: String,
    },
    RequiresBoundaryParentPeel {
        sector_id: u64,
        adjacent_parents: BTreeSet<TriangleAddress>,
    },
    RequiresLocalCollar {
        sector_id: u64,
    },
    InvalidInput {
        sector_id: u64,
        reason: String,
    },
}

pub(super) struct SectorRestoreMaterialization {
    pub mesh: HierarchyLeafMesh,
    pub removed_mixed_faces: BTreeSet<usize>,
    pub restored_source_faces: BTreeSet<usize>,
    pub restored_addresses: BTreeSet<TriangleAddress>,
    pub released_children: BTreeSet<TriangleAddress>,
    pub inserted_custom_triangles: BTreeSet<[usize; 3]>,
    pub helper_source_vertices: usize,
}

pub fn restore_fine_compatible_sector(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    sector_id: u64,
    local_geometry_iterations: usize,
) -> DirectSectorRestoreOutcome {
    match restore_fine_compatible_sector_inner(
        source,
        incumbent,
        incumbent_patch,
        atlas,
        sector_id,
        local_geometry_iterations,
    ) {
        Ok(outcome) => outcome,
        Err(reason) => DirectSectorRestoreOutcome::InvalidInput { sector_id, reason },
    }
}

fn restore_fine_compatible_sector_inner(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    incumbent_patch: &ElasticPatch,
    atlas: &SectorRecoveryAtlas,
    sector_id: u64,
    local_geometry_iterations: usize,
) -> Result<DirectSectorRestoreOutcome, String> {
    let coverage = atlas
        .sectors
        .get(&sector_id)
        .ok_or_else(|| format!("exact sector {sector_id} is absent"))?;
    let mixed_faces = custom_faces_by_sector(incumbent, atlas)?
        .remove(&sector_id)
        .ok_or_else(|| format!("incumbent has no custom faces for sector {sector_id}"))?;
    if mixed_faces.len() != coverage.custom_triangles.len() {
        return Err(format!(
            "sector {sector_id} has {} incumbent faces but {} exact triangles",
            mixed_faces.len(),
            coverage.custom_triangles.len()
        ));
    }
    let actual_boundary = mixed_boundary_source_edges(incumbent, &mixed_faces)?;
    if actual_boundary != coverage.boundary_edges
        || coverage
            .boundary_cycles
            .iter()
            .flatten()
            .any(|source_site| {
                !incumbent
                    .source_vertex_slots
                    .iter()
                    .flatten()
                    .any(|candidate| candidate == source_site)
            })
    {
        let adjacent_parents = adjacent_coarse_parents(incumbent, &mixed_faces, source.subdivision);
        return Ok(if adjacent_parents.is_empty() {
            DirectSectorRestoreOutcome::RequiresLocalCollar { sector_id }
        } else {
            DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
                sector_id,
                adjacent_parents,
            }
        });
    }

    let overlapping_parents = incumbent
        .mesh
        .active_triangle_slots()
        .filter_map(|face| incumbent.triangle_addresses[face])
        .filter(|&address| {
            !source_descendant_faces(source, address).is_disjoint(&coverage.source_faces)
        })
        .collect::<BTreeSet<_>>();
    if !overlapping_parents.is_empty() {
        return Ok(DirectSectorRestoreOutcome::RequiresBoundaryParentPeel {
            sector_id,
            adjacent_parents: overlapping_parents,
        });
    }
    let materialized =
        materialize_sector_restore(source, incumbent, atlas, sector_id, &BTreeSet::new())?;
    let SectorRestoreMaterialization {
        mesh: candidate,
        removed_mixed_faces: mixed_faces,
        restored_source_faces,
        restored_addresses,
        helper_source_vertices,
        ..
    } = materialized;

    let trial_context = DirectTrialContext {
        sector_id,
        incumbent,
        removed_mixed_faces: &mixed_faces,
        restored_source_faces: &restored_source_faces,
        restored_addresses: &restored_addresses,
        helper_source_vertices,
    };
    let make_trial =
        |mesh: HierarchyLeafMesh, strict_certified: bool, local_geometry_attempted: bool| {
            direct_trial(
                &trial_context,
                mesh,
                local_geometry_attempted,
                strict_certified,
            )
        };
    if Certificate::internal()
        .verify_geometry(&candidate.mesh)
        .is_ok()
    {
        return Ok(DirectSectorRestoreOutcome::Certified(Box::new(make_trial(
            candidate, true, false,
        ))));
    }
    if local_geometry_iterations == 0 {
        return Ok(DirectSectorRestoreOutcome::GeometryNotCertified {
            trial: Box::new(make_trial(candidate, false, false)),
            reason: "local geometry was not requested".into(),
        });
    }
    let movable_sources = coverage
        .source_faces
        .iter()
        .flat_map(|&face| source.mesh.triangles()[face])
        .filter(|source_site| {
            !coverage
                .boundary_cycles
                .iter()
                .flatten()
                .any(|boundary| boundary == source_site)
        })
        .collect::<BTreeSet<_>>();
    let movable_compact_vertices = candidate
        .source_vertex_slots
        .iter()
        .enumerate()
        .filter_map(|(compact, source_site)| {
            source_site
                .is_some_and(|source_site| movable_sources.contains(&source_site))
                .then_some(compact)
        })
        .collect::<Vec<_>>();
    if movable_compact_vertices.is_empty() {
        return Ok(DirectSectorRestoreOutcome::GeometryNotCertified {
            trial: Box::new(make_trial(candidate, false, false)),
            reason: "fine-compatible sector has no interior movable source vertex".into(),
        });
    }
    let movable = movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let local_patch = ElasticPatch {
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
        local_patch,
        ElasticBlockLimits {
            elastic_iterations: local_geometry_iterations,
        },
        GeometryStartId::MaterializedSource,
    );
    Ok(match outcome {
        ElasticBlockOutcome::Certified(trial) => {
            DirectSectorRestoreOutcome::Certified(Box::new(make_trial(trial.mesh, true, true)))
        }
        ElasticBlockOutcome::ElasticNoImprovement {
            witness, reason, ..
        }
        | ElasticBlockOutcome::SearchBudgetExhausted {
            witness, reason, ..
        }
        | ElasticBlockOutcome::RequiresDifferentTopology {
            witness, reason, ..
        } => DirectSectorRestoreOutcome::GeometryNotCertified {
            trial: Box::new(make_trial(witness.mesh, false, true)),
            reason,
        },
        ElasticBlockOutcome::InvalidPatch { reason } => {
            DirectSectorRestoreOutcome::GeometryNotCertified {
                trial: Box::new(make_trial(candidate, false, true)),
                reason,
            }
        }
    })
}

pub(super) fn materialize_sector_restore(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    sector_id: u64,
    released_parents: &BTreeSet<TriangleAddress>,
) -> Result<SectorRestoreMaterialization, String> {
    materialize_sector_restores(
        source,
        incumbent,
        atlas,
        &BTreeSet::from([sector_id]),
        released_parents,
    )
}

pub(super) fn materialize_sector_restores(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    sector_ids: &BTreeSet<u64>,
    released_parents: &BTreeSet<TriangleAddress>,
) -> Result<SectorRestoreMaterialization, String> {
    materialize_sector_restores_with_replacements(
        source,
        incumbent,
        atlas,
        sector_ids,
        released_parents,
        &BTreeMap::new(),
    )
}

pub(super) fn materialize_sector_restores_with_replacements(
    source: &MotherGrid,
    incumbent: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    sector_ids: &BTreeSet<u64>,
    released_parents: &BTreeSet<TriangleAddress>,
    custom_leaf_replacements: &BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
) -> Result<SectorRestoreMaterialization, String> {
    if sector_ids.is_empty() {
        return Err("sector restore requires at least one exact sector".into());
    }
    let owners = custom_faces_by_sector(incumbent, atlas)?;
    let mixed_faces = sector_ids
        .iter()
        .map(|sector_id| {
            owners
                .get(sector_id)
                .cloned()
                .ok_or_else(|| format!("incumbent has no custom faces for sector {sector_id}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    let restored_source_faces = sector_ids
        .iter()
        .map(|sector_id| {
            atlas
                .sectors
                .get(sector_id)
                .map(|coverage| coverage.source_faces.iter().copied())
                .ok_or_else(|| format!("exact sector {sector_id} is absent"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    let restored_addresses = restored_source_faces
        .iter()
        .map(|&face| {
            source.triangle_addresses[face]
                .ok_or_else(|| format!("source face {face} has no hierarchy address"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut leaves = BTreeSet::new();
    let mut released_children = BTreeSet::new();
    let mut seen_released = BTreeSet::new();
    let mut seen_replacements = BTreeSet::new();
    let mut retained_custom_source_faces = BTreeSet::new();
    let mut retained_custom_triangles = Vec::new();
    for face in incumbent.mesh.active_triangle_slots() {
        if mixed_faces.contains(&face) {
            continue;
        }
        if let Some(address) = incumbent.triangle_addresses[face] {
            if released_parents.contains(&address) {
                let children = address
                    .children_2_to_1()
                    .ok_or_else(|| format!("cannot release invalid parent {address:?}"))?;
                if children[0].n != source.subdivision {
                    return Err(format!(
                        "released parent {address:?} is not one level above the source grid"
                    ));
                }
                released_children.extend(children);
                seen_released.insert(address);
                continue;
            }
            if let Some(triangles) = custom_leaf_replacements.get(&address) {
                retained_custom_source_faces.extend(source_descendant_faces(source, address));
                retained_custom_triangles.extend(triangles.iter().copied());
                seen_replacements.insert(address);
                continue;
            }
            let descendants = source_descendant_faces(source, address);
            if !descendants.is_disjoint(&restored_source_faces) {
                return Err(format!(
                    "selected sectors still overlap unreleased hierarchy leaf {address:?}"
                ));
            }
            leaves.insert(address);
            continue;
        }
        let owner = owners
            .iter()
            .find_map(|(&owner, faces)| faces.contains(&face).then_some(owner))
            .ok_or_else(|| format!("custom face {face} has no exact sector owner"))?;
        retained_custom_source_faces.extend(&atlas.sectors[&owner].source_faces);
        retained_custom_triangles.push(source_triangle(incumbent, face)?);
    }
    if &seen_released != released_parents {
        return Err(format!(
            "released parents are not active incumbent leaves: {:?}",
            released_parents
                .difference(&seen_released)
                .collect::<Vec<_>>()
        ));
    }
    let expected_replacements = custom_leaf_replacements
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    if seen_replacements != expected_replacements {
        return Err(format!(
            "custom replacement parents are not active incumbent leaves: {:?}",
            expected_replacements
                .difference(&seen_replacements)
                .collect::<Vec<_>>()
        ));
    }
    leaves.extend(restored_addresses.iter().copied());
    leaves.extend(released_children.iter().copied());
    let mut candidate = rebuild_from_leaf_set_with_custom_face_slots(
        source,
        &HierarchyLeafSet { leaves },
        &retained_custom_source_faces,
        &retained_custom_triangles,
    )?;
    restore_incumbent_positions(incumbent, &mut candidate);
    let incumbent_sources = incumbent
        .source_vertex_slots
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let helper_source_vertices = candidate
        .source_vertex_slots
        .iter()
        .flatten()
        .filter(|source| !incumbent_sources.contains(source))
        .count();
    Ok(SectorRestoreMaterialization {
        mesh: candidate,
        removed_mixed_faces: mixed_faces,
        restored_source_faces,
        restored_addresses,
        released_children,
        inserted_custom_triangles: custom_leaf_replacements
            .values()
            .flatten()
            .copied()
            .map(canonical_triangle)
            .collect(),
        helper_source_vertices,
    })
}

pub(super) fn custom_sectors_adjacent_to_parent(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
    parent: TriangleAddress,
) -> Result<BTreeSet<u64>, String> {
    let parent_faces = mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| mesh.triangle_addresses[face] == Some(parent))
        .collect::<Vec<_>>();
    if parent_faces.len() != 1 {
        return Err(format!(
            "boundary parent {parent:?} has {} incumbent faces",
            parent_faces.len()
        ));
    }
    let mut counts = BTreeMap::<Edge, usize>::new();
    for face in source_descendant_faces(source, parent) {
        for edge in triangle_edges(source.mesh.triangles()[face]) {
            *counts.entry(edge).or_default() += 1;
        }
    }
    let boundary = counts
        .into_iter()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect::<BTreeSet<_>>();
    Ok(atlas
        .sectors
        .iter()
        .filter_map(|(&sector, coverage)| {
            (!coverage.boundary_edges.is_disjoint(&boundary)).then_some(sector)
        })
        .collect())
}

fn custom_faces_by_sector(
    mesh: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
) -> Result<BTreeMap<u64, BTreeSet<usize>>, String> {
    let mut result = BTreeMap::<u64, BTreeSet<usize>>::new();
    for face in mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| mesh.triangle_addresses[face].is_none())
    {
        let triangle = canonical_triangle(source_triangle(mesh, face)?);
        let owner = atlas
            .custom_face_owner
            .get(&triangle)
            .copied()
            .ok_or_else(|| format!("custom face {face} is absent from exact sector atlas"))?;
        result.entry(owner).or_default().insert(face);
    }
    Ok(result)
}

fn source_triangle(mesh: &HierarchyLeafMesh, face: usize) -> Result<[usize; 3], String> {
    mesh.mesh.triangles()[face]
        .map(|compact| {
            mesh.source_vertex_slots[compact]
                .ok_or_else(|| format!("mixed face {face} uses a non-source vertex"))
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "triangle conversion failed".into())
}

fn mixed_boundary_source_edges(
    mesh: &HierarchyLeafMesh,
    faces: &BTreeSet<usize>,
) -> Result<BTreeSet<Edge>, String> {
    let mut counts = BTreeMap::<Edge, usize>::new();
    for &face in faces {
        for edge in triangle_edges(source_triangle(mesh, face)?) {
            *counts.entry(edge).or_default() += 1;
        }
    }
    if counts.values().any(|&count| count > 2) {
        return Err("direct sector has a non-manifold incumbent edge".into());
    }
    Ok(counts
        .into_iter()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect())
}

fn adjacent_coarse_parents(
    mesh: &HierarchyLeafMesh,
    faces: &BTreeSet<usize>,
    source_n: usize,
) -> BTreeSet<TriangleAddress> {
    faces
        .iter()
        .flat_map(|&face| mesh.mesh.neighbours()[face])
        .filter(|face| !faces.contains(face))
        .filter_map(|face| mesh.triangle_addresses.get(face).copied().flatten())
        .filter(|address| address.n < source_n)
        .collect()
}

fn restore_incumbent_positions(incumbent: &HierarchyLeafMesh, candidate: &mut HierarchyLeafMesh) {
    let old = incumbent
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| {
            source.map(|source| (source, incumbent.mesh.vertices()[compact]))
        })
        .collect::<BTreeMap<_, _>>();
    for (compact, source) in candidate.source_vertex_slots.iter().copied().enumerate() {
        if let Some(point) = source.and_then(|source| old.get(&source)).copied() {
            candidate.mesh.move_vertex(compact, point);
        }
    }
}

struct DirectTrialContext<'a> {
    sector_id: u64,
    incumbent: &'a HierarchyLeafMesh,
    removed_mixed_faces: &'a BTreeSet<usize>,
    restored_source_faces: &'a BTreeSet<usize>,
    restored_addresses: &'a BTreeSet<TriangleAddress>,
    helper_source_vertices: usize,
}

fn direct_trial(
    context: &DirectTrialContext<'_>,
    mesh: HierarchyLeafMesh,
    local_geometry_attempted: bool,
    strict_certified: bool,
) -> DirectSectorRestoreTrial {
    let angle_range_deg = mesh_angle_range(&mesh);
    let outside_topology_bitwise_equal = logical_exterior_equal(
        context.incumbent,
        context.removed_mixed_faces,
        &BTreeSet::new(),
        &mesh,
        context.restored_addresses,
    );
    let outside_coordinates_bitwise_equal =
        outside_coordinates_equal(context.incumbent, context.removed_mixed_faces, &mesh);
    let edge_incidence_at_most_two = closed_edge_incidence(&mesh);
    DirectSectorRestoreTrial {
        sector_id: context.sector_id,
        angle_range_deg,
        mesh,
        removed_mixed_faces: context.removed_mixed_faces.clone(),
        restored_source_faces: context.restored_source_faces.clone(),
        helper_source_vertices: context.helper_source_vertices,
        outside_topology_bitwise_equal,
        outside_coordinates_bitwise_equal,
        edge_incidence_at_most_two,
        local_geometry_attempted,
        strict_certified,
    }
}

pub(super) fn logical_exterior_equal(
    before: &HierarchyLeafMesh,
    removed_faces: &BTreeSet<usize>,
    removed_addresses: &BTreeSet<TriangleAddress>,
    after: &HierarchyLeafMesh,
    inserted_addresses: &BTreeSet<TriangleAddress>,
) -> bool {
    logical_exterior_equal_with_custom(
        before,
        removed_faces,
        removed_addresses,
        after,
        inserted_addresses,
        &BTreeSet::new(),
    )
}

pub(super) fn logical_exterior_equal_with_custom(
    before: &HierarchyLeafMesh,
    removed_faces: &BTreeSet<usize>,
    removed_addresses: &BTreeSet<TriangleAddress>,
    after: &HierarchyLeafMesh,
    inserted_addresses: &BTreeSet<TriangleAddress>,
    inserted_custom_triangles: &BTreeSet<[usize; 3]>,
) -> bool {
    face_signatures(
        before,
        Some(removed_faces),
        removed_addresses,
        &BTreeSet::new(),
    ) == face_signatures(after, None, inserted_addresses, inserted_custom_triangles)
}

pub(super) fn outside_coordinates_equal(
    before: &HierarchyLeafMesh,
    removed_faces: &BTreeSet<usize>,
    after: &HierarchyLeafMesh,
) -> bool {
    let outside_sources = before
        .mesh
        .active_triangle_slots()
        .filter(|face| !removed_faces.contains(face))
        .flat_map(|face| before.mesh.triangles()[face])
        .filter_map(|compact| before.source_vertex_slots[compact])
        .collect::<BTreeSet<_>>();
    let old_positions = source_positions(before);
    let new_positions = source_positions(after);
    outside_sources.iter().all(|source| {
        old_positions.get(source).is_some_and(|old| {
            new_positions
                .get(source)
                .is_some_and(|new| point_bits_equal(*old, *new))
        })
    })
}

pub(super) fn closed_edge_incidence(mesh: &HierarchyLeafMesh) -> bool {
    edge_incidence(&mesh.mesh).values().all(|&count| count <= 2) && mesh.mesh.open_edge_count() == 0
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum FaceSignature {
    Hierarchy(TriangleAddress),
    Custom([usize; 3]),
}

fn face_signatures(
    mesh: &HierarchyLeafMesh,
    excluded_faces: Option<&BTreeSet<usize>>,
    excluded_addresses: &BTreeSet<TriangleAddress>,
    excluded_custom_triangles: &BTreeSet<[usize; 3]>,
) -> BTreeMap<FaceSignature, usize> {
    let mut result = BTreeMap::new();
    for face in mesh.mesh.active_triangle_slots() {
        if excluded_faces.is_some_and(|faces| faces.contains(&face)) {
            continue;
        }
        let signature = if let Some(address) = mesh.triangle_addresses[face] {
            if excluded_addresses.contains(&address) {
                continue;
            }
            FaceSignature::Hierarchy(address)
        } else if let Ok(triangle) = source_triangle(mesh, face) {
            let triangle = canonical_triangle(triangle);
            if excluded_custom_triangles.contains(&triangle) {
                continue;
            }
            FaceSignature::Custom(triangle)
        } else {
            continue;
        };
        *result.entry(signature).or_default() += 1;
    }
    result
}

fn source_positions(mesh: &HierarchyLeafMesh) -> BTreeMap<usize, earthmesh_mesh::CartesianPoint> {
    mesh.source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| {
            source.map(|source| (source, mesh.mesh.vertices()[compact]))
        })
        .collect()
}

fn point_bits_equal(
    left: earthmesh_mesh::CartesianPoint,
    right: earthmesh_mesh::CartesianPoint,
) -> bool {
    left.x.to_bits() == right.x.to_bits()
        && left.y.to_bits() == right.y.to_bits()
        && left.z.to_bits() == right.z.to_bits()
}

fn edge_incidence(mesh: &earthmesh_mesh::MeshState) -> BTreeMap<Edge, usize> {
    let mut counts = BTreeMap::new();
    for face in mesh.active_triangle_slots() {
        for edge in triangle_edges(mesh.triangles()[face]) {
            *counts.entry(edge).or_default() += 1;
        }
    }
    counts
}

pub(super) fn mesh_angle_range(mesh: &HierarchyLeafMesh) -> Option<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for face in mesh.mesh.active_triangle_slots() {
        for angle in spherical_triangle_angles(
            mesh.mesh.triangles()[face].map(|site| mesh.mesh.vertices()[site]),
        )? {
            minimum = minimum.min(angle);
            maximum = maximum.max(angle);
        }
    }
    (minimum.is_finite() && maximum.is_finite()).then_some((minimum, maximum))
}

fn source_descendant_faces(source: &MotherGrid, ancestor: TriangleAddress) -> BTreeSet<usize> {
    source
        .triangle_addresses
        .iter()
        .enumerate()
        .filter_map(|(face, address)| {
            address
                .is_some_and(|address| is_descendant(address, ancestor))
                .then_some(face)
        })
        .collect()
}

fn is_descendant(mut address: TriangleAddress, ancestor: TriangleAddress) -> bool {
    while address.n > ancestor.n {
        let Some(parent) = address.parent_2_to_1() else {
            return false;
        };
        address = parent;
    }
    address == ancestor
}

fn triangle_edges(triangle: [usize; 3]) -> [Edge; 3] {
    [
        edge(triangle[0], triangle[1]),
        edge(triangle[1], triangle[2]),
        edge(triangle[2], triangle[0]),
    ]
}

fn edge(left: usize, right: usize) -> Edge {
    (left.min(right), left.max(right))
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        build_sector_recovery_atlas, build_stratified_annulus, n6_legacy_mixed_fixture,
        FullPolygonMergeLimits, FullPolygonMergeOutcome, GeometryDomainId,
        TransitionTopologyCandidate,
    };
    use std::sync::OnceLock;

    fn fixture() -> &'static (MotherGrid, SectorRecoveryAtlas, HierarchyLeafMesh) {
        static FIXTURE: OnceLock<(MotherGrid, SectorRecoveryAtlas, HierarchyLeafMesh)> =
            OnceLock::new();
        FIXTURE.get_or_init(|| {
            let (source, component) = n6_legacy_mixed_fixture().unwrap();
            let stratified = build_stratified_annulus(&source, &component).unwrap();
            let FullPolygonMergeOutcome::Closed(trial) = super::super::solve_full_polygon_merge(
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
                &trial.evidence.selected_topology_keys,
            )
            .unwrap();
            (source, atlas, trial.global_trial.mesh)
        })
    }

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
    fn fine_compatible_restore_preserves_the_logical_exterior() {
        let (source, atlas, mesh) = fixture();
        let patch = patch(mesh);
        let trial = atlas
            .sectors
            .keys()
            .find_map(|&sector_id| {
                match restore_fine_compatible_sector(source, mesh, &patch, atlas, sector_id, 0) {
                    DirectSectorRestoreOutcome::Certified(trial)
                    | DirectSectorRestoreOutcome::GeometryNotCertified { trial, .. } => Some(trial),
                    _ => None,
                }
            })
            .expect("Frozen N6 has a fine-compatible exact sector");
        assert!(trial.outside_topology_bitwise_equal);
        assert!(trial.outside_coordinates_bitwise_equal);
        assert!(trial.edge_incidence_at_most_two);
        assert!(!trial.removed_mixed_faces.is_empty());
        assert!(!trial.restored_source_faces.is_empty());
    }

    #[test]
    fn coarse_interface_returns_a_typed_blocker_without_mutating_incumbent() {
        let (source, atlas, mesh) = fixture();
        let patch = patch(mesh);
        let before = mesh.clone();
        assert!(atlas.sectors.keys().any(|&sector_id| matches!(
            restore_fine_compatible_sector(source, mesh, &patch, atlas, sector_id, 0),
            DirectSectorRestoreOutcome::RequiresBoundaryParentPeel { .. }
                | DirectSectorRestoreOutcome::RequiresLocalCollar { .. }
        )));
        assert_eq!(*mesh, before);
    }
}
