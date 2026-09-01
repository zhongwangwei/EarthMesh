//! Exact sector provenance used by local annular recovery.

use super::full_polygon_reachability::effective_sector_polygons;
use super::promotion::build_local_recovery_patch;
use super::transition_topology::cycles_from_edges;
use super::{
    build_protected_coarse_region, BandComponentKind, FullPolygonTopologyKey, HierarchyLeafMesh,
    PromotionPatchTopology, ProtectedCoarseRegion, StratifiedAnnulus, ViolatingAngle,
    ViolationComponent,
};
use crate::certificate::interval::Interval;
use crate::{MotherGrid, TriangleAddress};
use earthmesh_mesh::spherical_triangle_area_unit;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundaryEvidence {
    edge_counts: BTreeMap<Edge, usize>,
    cycle: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExactSectorCoverage {
    pub sector_id: u64,
    pub band_id: usize,
    pub source_faces: BTreeSet<usize>,
    pub custom_triangles: BTreeSet<[usize; 3]>,
    pub boundary_edges: BTreeSet<(usize, usize)>,
    pub boundary_cycles: Vec<Vec<usize>>,
    pub source_area_interval: Interval,
    pub custom_area_interval: Interval,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SectorRecoveryAtlas {
    pub sectors: BTreeMap<u64, ExactSectorCoverage>,
    pub custom_face_owner: BTreeMap<[usize; 3], u64>,
    pub source_face_owner: BTreeMap<usize, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAtom {
    HierarchyLeaf {
        address: TriangleAddress,
        mixed_face: usize,
        source_faces: BTreeSet<usize>,
    },
    Sector {
        sector_id: u64,
        mixed_faces: BTreeSet<usize>,
        source_faces: BTreeSet<usize>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalRecoveryComponent {
    pub id: u64,
    pub atoms: Vec<RecoveryAtom>,
    pub mixed_faces: BTreeSet<usize>,
    pub source_faces: BTreeSet<usize>,
    pub boundary_cycles: Vec<Vec<usize>>,
    pub protected_coarse_regions: Vec<ProtectedCoarseRegion>,
    pub topology: PromotionPatchTopology,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectorRecoveryError {
    InvalidExactSector(String),
    AmbiguousRecoveryOwnership {
        mixed_face: usize,
        owner_count: usize,
    },
}

pub fn build_sector_recovery_atlas(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    topology_keys: &[FullPolygonTopologyKey],
) -> Result<SectorRecoveryAtlas, SectorRecoveryError> {
    let polygons = effective_sector_polygons(stratified).map_err(invalid)?;
    let disk_bands = stratified
        .bands
        .iter()
        .filter_map(|band| {
            matches!(band.kind, BandComponentKind::SectorDisk { .. }).then_some((
                band.band_id,
                band.face_slots.iter().copied().collect::<BTreeSet<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    if disk_bands.len() != polygons.len() {
        return Err(invalid(format!(
            "exact recovery needs one source-face disk per sector: {} disks, {} polygons",
            disk_bands.len(),
            polygons.len()
        )));
    }

    let mut keys = BTreeMap::new();
    for key in topology_keys {
        if keys.insert(key.sector_id, key).is_some() {
            return Err(invalid(format!(
                "sector {} has multiple selected topology keys",
                key.sector_id
            )));
        }
    }
    let polygon_ids = polygons
        .iter()
        .map(|polygon| polygon.id)
        .collect::<BTreeSet<_>>();
    if keys.keys().copied().collect::<BTreeSet<_>>() != polygon_ids {
        return Err(invalid(
            "selected topology keys do not cover every exact sector once".into(),
        ));
    }

    let mut sectors = BTreeMap::new();
    let mut custom_face_owner = BTreeMap::new();
    let mut source_face_owner = BTreeMap::new();
    for (polygon, (band_id, source_faces)) in polygons.into_iter().zip(disk_bands) {
        let key = keys[&polygon.id];
        let custom_triangles = key
            .triangles
            .iter()
            .copied()
            .map(canonical_triangle)
            .collect::<BTreeSet<_>>();
        if custom_triangles.len() != key.triangles.len() {
            return Err(invalid(format!(
                "sector {} repeats a custom triangle",
                polygon.id
            )));
        }
        let source_boundary = source_boundary(source, &source_faces)?;
        let polygon_boundary = expanded_polygon_boundary(source, &polygon.vertices)?;
        let custom_boundary = custom_boundary(source, &custom_triangles)?;
        if source_boundary != polygon_boundary || source_boundary != custom_boundary {
            return Err(invalid(format!(
                "sector {} source and topology boundaries differ",
                polygon.id
            )));
        }
        if patch_euler(source, &source_faces) != 1 || triangle_complex_euler(&custom_triangles) != 1
        {
            return Err(invalid(format!(
                "sector {} is not one topological disk",
                polygon.id
            )));
        }
        let source_area_interval = area_interval(
            source,
            source_faces
                .iter()
                .map(|&face| source.mesh.triangles()[face]),
        )?;
        let custom_area_interval = area_interval(source, custom_triangles.iter().copied())?;
        let difference = source_area_interval.sub_out(custom_area_interval);
        if source_area_interval.hi < custom_area_interval.lo
            || custom_area_interval.hi < source_area_interval.lo
            || !difference.contains(0.0)
        {
            return Err(invalid(format!(
                "sector {} source/custom area intervals do not close: [{}, {}] vs [{}, {}]",
                polygon.id,
                source_area_interval.lo,
                source_area_interval.hi,
                custom_area_interval.lo,
                custom_area_interval.hi
            )));
        }
        for &face in &source_faces {
            if source_face_owner.insert(face, polygon.id).is_some() {
                return Err(invalid(format!(
                    "source annulus face {face} belongs to multiple sectors"
                )));
            }
        }
        for &triangle in &custom_triangles {
            if custom_face_owner.insert(triangle, polygon.id).is_some() {
                return Err(invalid(format!(
                    "custom triangle {triangle:?} belongs to multiple sectors"
                )));
            }
        }
        let mut coverage = ExactSectorCoverage {
            sector_id: polygon.id,
            band_id,
            source_faces,
            custom_triangles,
            boundary_edges: source_boundary.edge_counts.keys().copied().collect(),
            boundary_cycles: vec![source_boundary.cycle],
            source_area_interval,
            custom_area_interval,
            fingerprint: 0,
        };
        coverage.fingerprint = coverage_fingerprint(&coverage);
        sectors.insert(polygon.id, coverage);
    }
    let annulus = stratified
        .coupled
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if source_face_owner.keys().copied().collect::<BTreeSet<_>>() != annulus {
        return Err(invalid(
            "exact sector source faces do not partition the annulus".into(),
        ));
    }
    Ok(SectorRecoveryAtlas {
        sectors,
        custom_face_owner,
        source_face_owner,
    })
}

pub fn build_strict_recovery_atoms(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    angles: &[ViolatingAngle],
    atlas: &SectorRecoveryAtlas,
) -> Result<Vec<RecoveryAtom>, SectorRecoveryError> {
    let mixed_faces_by_sector = verify_mixed_custom_ownership(mesh, atlas)?;
    let mut hierarchy = BTreeMap::<usize, (TriangleAddress, BTreeSet<usize>)>::new();
    let mut sectors = BTreeSet::<u64>::new();
    for angle in angles.iter().filter(|angle| angle.signed_margin_deg < 0.0) {
        let face = angle.face;
        if let Some(address) = mesh.triangle_addresses.get(face).copied().flatten() {
            hierarchy
                .entry(face)
                .or_insert_with(|| (address, source_descendant_faces(source, address)));
            continue;
        }
        let Some(triangle) = mesh.mesh.triangles().get(face).map(|triangle| {
            triangle.map(|site| mesh.source_vertex_slots.get(site).copied().flatten())
        }) else {
            return Err(ambiguous(face, 0));
        };
        let [Some(a), Some(b), Some(c)] = triangle else {
            return Err(ambiguous(face, 0));
        };
        let key = canonical_triangle([a, b, c]);
        let Some(&sector_id) = atlas.custom_face_owner.get(&key) else {
            return Err(ambiguous(face, 0));
        };
        sectors.insert(sector_id);
    }
    if hierarchy
        .values()
        .any(|(_, source_faces)| source_faces.is_empty())
    {
        return Err(invalid(
            "hierarchy recovery atom has no exact source descendants".into(),
        ));
    }
    let mut atoms = hierarchy
        .into_iter()
        .map(
            |(mixed_face, (address, source_faces))| RecoveryAtom::HierarchyLeaf {
                address,
                mixed_face,
                source_faces,
            },
        )
        .collect::<Vec<_>>();
    atoms.extend(sectors.into_iter().map(|sector_id| RecoveryAtom::Sector {
        sector_id,
        mixed_faces: mixed_faces_by_sector[&sector_id].clone(),
        source_faces: atlas.sectors[&sector_id].source_faces.clone(),
    }));
    Ok(atoms)
}

pub fn build_local_recovery_components(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    strict_angles: &[ViolatingAngle],
    atoms: &[RecoveryAtom],
    retained_parents: &BTreeSet<TriangleAddress>,
) -> Result<Vec<LocalRecoveryComponent>, SectorRecoveryError> {
    if atoms.is_empty() {
        return Ok(Vec::new());
    }
    let evidence = atoms
        .iter()
        .map(|atom| atom_evidence(source, mesh, strict_angles, atom))
        .collect::<Result<Vec<_>, _>>()?;
    let strict_faces = strict_angles
        .iter()
        .filter(|angle| angle.signed_margin_deg < 0.0)
        .map(|angle| angle.face)
        .collect::<BTreeSet<_>>();
    let owned_faces = evidence
        .iter()
        .flat_map(|item| item.mixed_faces.iter().copied())
        .collect::<BTreeSet<_>>();
    if !strict_faces.is_subset(&owned_faces) {
        return Err(invalid(
            "strict mixed faces are not covered by local recovery atoms".into(),
        ));
    }

    // ponytail: the Frozen N6 atom set is bounded (<100); replace this stable
    // O(n²) union pass only if production telemetry shows clustering dominates.
    let mut groups = (0..atoms.len()).collect::<Vec<_>>();
    for left in 0..evidence.len() {
        for right in left + 1..evidence.len() {
            if atom_evidence_touches(&evidence[left], &evidence[right]) {
                let old = groups[right];
                let new = groups[left];
                for group in &mut groups {
                    if *group == old {
                        *group = new;
                    }
                }
            }
        }
    }
    let mut grouped = BTreeMap::<usize, Vec<usize>>::new();
    for (atom, group) in groups.into_iter().enumerate() {
        grouped.entry(group).or_default().push(atom);
    }

    grouped
        .into_values()
        .enumerate()
        .map(|(id, indices)| {
            let component_atoms = indices
                .iter()
                .map(|&index| atoms[index].clone())
                .collect::<Vec<_>>();
            let mixed_faces = indices
                .iter()
                .flat_map(|&index| evidence[index].mixed_faces.iter().copied())
                .collect::<BTreeSet<_>>();
            let source_seed_faces = indices
                .iter()
                .flat_map(|&index| evidence[index].source_faces.iter().copied())
                .collect::<BTreeSet<_>>();
            let angles = strict_angles
                .iter()
                .filter(|angle| angle.signed_margin_deg < 0.0 && mixed_faces.contains(&angle.face))
                .cloned()
                .collect::<Vec<_>>();
            let active_constraint_vertices = angles
                .iter()
                .flat_map(|angle| {
                    angle
                        .movable_vertices
                        .iter()
                        .chain(&angle.fixed_vertices)
                        .copied()
                })
                .collect();
            let component = ViolationComponent {
                id: id as u64,
                angles,
                source_faces: source_seed_faces.clone(),
                parent_faces: BTreeSet::new(),
                support_vertices: source_seed_faces
                    .iter()
                    .flat_map(|&face| source.mesh.triangles()[face])
                    .collect(),
                active_constraint_vertices,
            };
            let remaining_parents = retained_parents
                .iter()
                .copied()
                .filter(|&parent| {
                    source_descendant_faces(source, parent).is_disjoint(&source_seed_faces)
                })
                .collect::<BTreeSet<_>>();
            let protected_regions = build_protected_regions(source, &remaining_parents)?;
            let patch = build_local_recovery_patch(
                source,
                &component,
                retained_parents,
                &protected_regions,
            )
            .map_err(invalid)?;
            Ok(LocalRecoveryComponent {
                id: id as u64,
                atoms: component_atoms,
                mixed_faces,
                source_faces: patch.source_faces,
                boundary_cycles: patch.boundary_cycles,
                protected_coarse_regions: patch.protected_coarse_regions,
                topology: patch.topology,
            })
        })
        .collect()
}

#[derive(Debug)]
struct AtomEvidence {
    mixed_faces: BTreeSet<usize>,
    source_faces: BTreeSet<usize>,
    mixed_edges: BTreeSet<Edge>,
    source_boundary_edges: BTreeSet<Edge>,
    active_vertices: BTreeSet<usize>,
}

fn atom_evidence(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    strict_angles: &[ViolatingAngle],
    atom: &RecoveryAtom,
) -> Result<AtomEvidence, SectorRecoveryError> {
    let (mixed_faces, source_faces) = match atom {
        RecoveryAtom::HierarchyLeaf {
            mixed_face,
            source_faces,
            ..
        } => (BTreeSet::from([*mixed_face]), source_faces.clone()),
        RecoveryAtom::Sector {
            mixed_faces,
            source_faces,
            ..
        } => (mixed_faces.clone(), source_faces.clone()),
    };
    if mixed_faces.is_empty() || source_faces.is_empty() {
        return Err(invalid("local recovery atom is empty".into()));
    }
    let mixed_edges = mixed_faces
        .iter()
        .map(|&face| {
            mesh.mesh
                .triangles()
                .get(face)
                .copied()
                .ok_or_else(|| invalid(format!("mixed face {face} is absent")))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(triangle_edges)
        .collect();
    let source_boundary_edges = source_boundary(source, &source_faces)?
        .edge_counts
        .into_keys()
        .collect();
    let active_vertices = strict_angles
        .iter()
        .filter(|angle| angle.signed_margin_deg < 0.0 && mixed_faces.contains(&angle.face))
        .flat_map(|angle| angle.movable_vertices.iter().copied())
        .collect();
    Ok(AtomEvidence {
        mixed_faces,
        source_faces,
        mixed_edges,
        source_boundary_edges,
        active_vertices,
    })
}

fn atom_evidence_touches(left: &AtomEvidence, right: &AtomEvidence) -> bool {
    !left.mixed_edges.is_disjoint(&right.mixed_edges)
        || !left.active_vertices.is_disjoint(&right.active_vertices)
        || !left
            .source_boundary_edges
            .is_disjoint(&right.source_boundary_edges)
}

fn build_protected_regions(
    source: &MotherGrid,
    retained_parents: &BTreeSet<TriangleAddress>,
) -> Result<Vec<ProtectedCoarseRegion>, SectorRecoveryError> {
    let parents = retained_parents.iter().copied().collect::<Vec<_>>();
    let descendants = parents
        .iter()
        .copied()
        .map(|parent| source_descendant_faces(source, parent))
        .collect::<Vec<_>>();
    if descendants.iter().any(BTreeSet::is_empty) {
        return Err(invalid(
            "retained coarse parent has no source descendants".into(),
        ));
    }
    let mut groups = (0..parents.len()).collect::<Vec<_>>();
    for left in 0..parents.len() {
        for right in left + 1..parents.len() {
            let adjacent = descendants[left].iter().any(|&face| {
                source.mesh.neighbours()[face]
                    .into_iter()
                    .any(|neighbour| descendants[right].contains(&neighbour))
            });
            if adjacent {
                let old = groups[right];
                let new = groups[left];
                for group in &mut groups {
                    if *group == old {
                        *group = new;
                    }
                }
            }
        }
    }
    let mut grouped = BTreeMap::<usize, BTreeSet<TriangleAddress>>::new();
    for (index, group) in groups.into_iter().enumerate() {
        grouped.entry(group).or_default().insert(parents[index]);
    }
    grouped
        .into_values()
        .enumerate()
        .map(|(id, parents)| {
            build_protected_coarse_region(source, id as u64, parents).map_err(invalid)
        })
        .collect()
}

fn verify_mixed_custom_ownership(
    mesh: &HierarchyLeafMesh,
    atlas: &SectorRecoveryAtlas,
) -> Result<BTreeMap<u64, BTreeSet<usize>>, SectorRecoveryError> {
    let mut faces_by_key = BTreeMap::<[usize; 3], BTreeSet<usize>>::new();
    for face in mesh.mesh.active_triangle_slots().filter(|&face| {
        mesh.triangle_addresses
            .get(face)
            .copied()
            .flatten()
            .is_none()
    }) {
        let triangle = mesh.mesh.triangles()[face]
            .map(|site| mesh.source_vertex_slots.get(site).copied().flatten());
        let [Some(a), Some(b), Some(c)] = triangle else {
            return Err(ambiguous(face, 0));
        };
        faces_by_key
            .entry(canonical_triangle([a, b, c]))
            .or_default()
            .insert(face);
    }
    if let Some((_, faces)) = faces_by_key.iter().find(|(_, faces)| faces.len() != 1) {
        return Err(ambiguous(
            *faces.first().expect("non-empty face owner"),
            faces.len(),
        ));
    }
    let actual = faces_by_key.keys().copied().collect::<BTreeSet<_>>();
    let expected = atlas
        .custom_face_owner
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    if actual != expected {
        if let Some(extra) = actual.difference(&expected).next() {
            return Err(ambiguous(
                *faces_by_key[extra]
                    .first()
                    .expect("custom face set is non-empty"),
                0,
            ));
        }
        return Err(invalid(format!(
            "incumbent is missing {} exact custom sector faces",
            expected.difference(&actual).count()
        )));
    }
    let mut by_sector = BTreeMap::<u64, BTreeSet<usize>>::new();
    for (key, faces) in faces_by_key {
        by_sector
            .entry(atlas.custom_face_owner[&key])
            .or_default()
            .extend(faces);
    }
    Ok(by_sector)
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

fn source_boundary(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
) -> Result<BoundaryEvidence, SectorRecoveryError> {
    let mut counts = BTreeMap::<(usize, usize), (usize, usize, usize)>::new();
    for &face in faces {
        let triangle = source
            .mesh
            .triangles()
            .get(face)
            .copied()
            .ok_or_else(|| invalid(format!("source face {face} is absent")))?;
        for (from, to) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let entry = counts.entry(edge(from, to)).or_insert((0, from, to));
            entry.0 += 1;
            if entry.0 > 2 {
                return Err(invalid(format!(
                    "source edge {:?} is non-manifold",
                    edge(from, to)
                )));
            }
        }
    }
    let directed = counts
        .values()
        .filter_map(|&(count, from, to)| (count == 1).then_some((from, to)))
        .collect::<Vec<_>>();
    let cycles = cycles_from_edges(directed).map_err(invalid)?;
    let [cycle] = cycles.as_slice() else {
        return Err(invalid(format!(
            "source sector has {} boundary cycles, expected one",
            cycles.len()
        )));
    };
    boundary_evidence(cycle.clone())
}

fn expanded_polygon_boundary(
    source: &MotherGrid,
    vertices: &[usize],
) -> Result<BoundaryEvidence, SectorRecoveryError> {
    if vertices.len() < 3 {
        return Err(invalid(
            "sector polygon has fewer than three vertices".into(),
        ));
    }
    let mut cycle = vec![vertices[0]];
    for (&from, &to) in vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
    {
        let path = expand_source_edge(source, from, to)?;
        if cycle.last() != path.first() {
            return Err(invalid("expanded polygon boundary is disconnected".into()));
        }
        cycle.extend(path.into_iter().skip(1));
    }
    if cycle.pop() != Some(vertices[0]) {
        return Err(invalid("expanded polygon boundary does not close".into()));
    }
    boundary_evidence(cycle)
}

fn custom_boundary(
    source: &MotherGrid,
    triangles: &BTreeSet<[usize; 3]>,
) -> Result<BoundaryEvidence, SectorRecoveryError> {
    let mut counts = BTreeMap::<(usize, usize), usize>::new();
    for &triangle in triangles {
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0] {
            return Err(invalid(format!("degenerate custom triangle {triangle:?}")));
        }
        for boundary in triangle_edges(triangle) {
            *counts.entry(boundary).or_default() += 1;
        }
    }
    if counts.values().any(|&count| count > 2) {
        return Err(invalid("custom sector has a non-manifold edge".into()));
    }
    let mut expanded = BTreeMap::<Edge, usize>::new();
    for (boundary, _) in counts.into_iter().filter(|(_, count)| *count == 1) {
        let path = expand_source_edge(source, boundary.0, boundary.1)?;
        for pair in path.windows(2) {
            *expanded.entry(edge(pair[0], pair[1])).or_default() += 1;
        }
    }
    if expanded.values().any(|&count| count != 1) {
        return Err(invalid(
            "custom sector boundary repeats a source edge".into(),
        ));
    }
    boundary_from_undirected_edges(expanded)
}

fn expand_source_edge(
    source: &MotherGrid,
    from: usize,
    to: usize,
) -> Result<Vec<usize>, SectorRecoveryError> {
    let source_edges = source_edges(source);
    let direct = edge(from, to);
    if source_edges.contains(&direct) {
        return Ok(vec![from, to]);
    }
    let midpoints = source
        .mesh
        .active_vertex_slots()
        .filter(|&candidate| {
            source_edges.contains(&edge(from, candidate))
                && source_edges.contains(&edge(candidate, to))
        })
        .collect::<Vec<_>>();
    let [midpoint] = midpoints.as_slice() else {
        return Err(invalid(format!(
            "sector boundary {from}-{to} has {} two-source-edge expansions",
            midpoints.len()
        )));
    };
    Ok(vec![from, *midpoint, to])
}

fn boundary_evidence(cycle: Vec<usize>) -> Result<BoundaryEvidence, SectorRecoveryError> {
    if cycle.len() < 3 || cycle.iter().copied().collect::<BTreeSet<_>>().len() != cycle.len() {
        return Err(invalid("sector boundary is not a simple cycle".into()));
    }
    let mut edge_counts = BTreeMap::new();
    for (&from, &to) in cycle
        .iter()
        .zip(cycle.iter().cycle().skip(1))
        .take(cycle.len())
    {
        *edge_counts.entry(edge(from, to)).or_default() += 1;
    }
    if edge_counts.values().any(|&count| count != 1) {
        return Err(invalid("sector boundary repeats an edge".into()));
    }
    Ok(BoundaryEvidence {
        edge_counts,
        cycle: canonical_cycle(&cycle),
    })
}

fn boundary_from_undirected_edges(
    edge_counts: BTreeMap<Edge, usize>,
) -> Result<BoundaryEvidence, SectorRecoveryError> {
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(left, right) in edge_counts.keys() {
        adjacency.entry(left).or_default().insert(right);
        adjacency.entry(right).or_default().insert(left);
    }
    if adjacency.len() < 3 || adjacency.values().any(|neighbours| neighbours.len() != 2) {
        return Err(invalid(
            "custom sector boundary is not one degree-two cycle".into(),
        ));
    }
    let start = *adjacency.keys().next().expect("non-empty boundary");
    let mut cycle = vec![start];
    let mut previous = usize::MAX;
    let mut current = start;
    loop {
        let next = adjacency[&current]
            .iter()
            .copied()
            .find(|candidate| *candidate != previous)
            .ok_or_else(|| invalid("custom sector boundary walk stopped".into()))?;
        if next == start {
            break;
        }
        if cycle.contains(&next) {
            return Err(invalid(
                "custom sector boundary repeats a vertex before closing".into(),
            ));
        }
        cycle.push(next);
        previous = current;
        current = next;
    }
    if cycle.len() != adjacency.len() {
        return Err(invalid(
            "custom sector boundary contains multiple cycles".into(),
        ));
    }
    Ok(BoundaryEvidence {
        edge_counts,
        cycle: canonical_cycle(&cycle),
    })
}

fn canonical_cycle(cycle: &[usize]) -> Vec<usize> {
    let mut candidates = Vec::with_capacity(cycle.len() * 2);
    for oriented in [cycle.to_vec(), cycle.iter().rev().copied().collect()] {
        for offset in 0..oriented.len() {
            candidates.push(
                oriented
                    .iter()
                    .copied()
                    .cycle()
                    .skip(offset)
                    .take(oriented.len())
                    .collect::<Vec<_>>(),
            );
        }
    }
    candidates.into_iter().min().unwrap_or_default()
}

fn source_edges(source: &MotherGrid) -> BTreeSet<(usize, usize)> {
    source
        .mesh
        .active_triangle_slots()
        .flat_map(|face| triangle_edges(source.mesh.triangles()[face]))
        .collect()
}

fn area_interval(
    source: &MotherGrid,
    triangles: impl Iterator<Item = [usize; 3]>,
) -> Result<Interval, SectorRecoveryError> {
    let mut area = 0.0;
    let mut count = 0usize;
    for triangle in triangles {
        let points = triangle.map(|vertex| {
            source
                .mesh
                .vertices()
                .get(vertex)
                .copied()
                .ok_or_else(|| invalid(format!("source vertex {vertex} is absent")))
        });
        let [Ok(a), Ok(b), Ok(c)] = points else {
            return Err(invalid(
                "sector triangle references an absent vertex".into(),
            ));
        };
        area += spherical_triangle_area_unit([a, b, c]).abs();
        count += 1;
    }
    // l'Huilier loses relative precision for small spherical excesses.  A
    // sqrt(epsilon) enclosure is conservative for this diagnostic sum while
    // boundary equality and Euler remain the exact coverage proof.
    let radius = f64::EPSILON.sqrt() * (count.max(1) as f64) * area.abs().max(1.0);
    Ok(Interval::around(area, radius))
}

fn patch_euler(source: &MotherGrid, faces: &BTreeSet<usize>) -> isize {
    let triangles = faces
        .iter()
        .map(|&face| source.mesh.triangles()[face])
        .collect::<BTreeSet<_>>();
    triangle_complex_euler(&triangles)
}

fn triangle_complex_euler(triangles: &BTreeSet<[usize; 3]>) -> isize {
    let vertices = triangles.iter().flatten().copied().collect::<BTreeSet<_>>();
    let edges = triangles
        .iter()
        .flat_map(|&triangle| triangle_edges(triangle))
        .collect::<BTreeSet<_>>();
    vertices.len() as isize - edges.len() as isize + triangles.len() as isize
}

fn coverage_fingerprint(coverage: &ExactSectorCoverage) -> u64 {
    let mut values = vec![
        coverage.sector_id,
        coverage.band_id as u64,
        coverage.source_faces.len() as u64,
    ];
    values.extend(coverage.source_faces.iter().map(|&face| face as u64));
    values.push(coverage.custom_triangles.len() as u64);
    values.extend(
        coverage
            .custom_triangles
            .iter()
            .flatten()
            .map(|&vertex| vertex as u64),
    );
    values.push(coverage.boundary_edges.len() as u64);
    values.extend(
        coverage
            .boundary_edges
            .iter()
            .flat_map(|&(left, right)| [left as u64, right as u64]),
    );
    for cycle in &coverage.boundary_cycles {
        values.push(cycle.len() as u64);
        values.extend(cycle.iter().map(|&vertex| vertex as u64));
    }
    values.extend([
        coverage.source_area_interval.lo.to_bits(),
        coverage.source_area_interval.hi.to_bits(),
        coverage.custom_area_interval.lo.to_bits(),
        coverage.custom_area_interval.hi.to_bits(),
    ]);
    fingerprint(values)
}

fn fingerprint(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in values {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn triangle_edges(triangle: [usize; 3]) -> [(usize, usize); 3] {
    [
        edge(triangle[0], triangle[1]),
        edge(triangle[1], triangle[2]),
        edge(triangle[2], triangle[0]),
    ]
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn edge(from: usize, to: usize) -> (usize, usize) {
    (from.min(to), from.max(to))
}

fn invalid(reason: String) -> SectorRecoveryError {
    SectorRecoveryError::InvalidExactSector(reason)
}

fn ambiguous(mixed_face: usize, owner_count: usize) -> SectorRecoveryError {
    SectorRecoveryError::AmbiguousRecoveryOwnership {
        mixed_face,
        owner_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        build_stratified_annulus, n6_legacy_mixed_fixture, solve_full_polygon_merge,
        AngleBoundKind, FullPolygonMergeLimits, FullPolygonMergeOutcome,
    };
    use std::sync::OnceLock;

    fn fixture() -> &'static (
        MotherGrid,
        StratifiedAnnulus,
        SectorRecoveryAtlas,
        HierarchyLeafMesh,
        BTreeSet<TriangleAddress>,
    ) {
        static FIXTURE: OnceLock<(
            MotherGrid,
            StratifiedAnnulus,
            SectorRecoveryAtlas,
            HierarchyLeafMesh,
            BTreeSet<TriangleAddress>,
        )> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let (source, component) = n6_legacy_mixed_fixture().unwrap();
            let stratified = build_stratified_annulus(&source, &component).unwrap();
            let FullPolygonMergeOutcome::Closed(trial) = solve_full_polygon_merge(
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
            (
                source,
                stratified,
                atlas,
                trial.global_trial.mesh,
                component.core_parents.iter().copied().collect(),
            )
        })
    }

    fn angle(face: usize, signed_margin_deg: f64) -> ViolatingAngle {
        ViolatingAngle {
            face,
            corner_site: 0,
            angle_deg: 40.2 + signed_margin_deg,
            signed_margin_deg,
            bound: AngleBoundKind::Lower,
            source_support_faces: BTreeSet::new(),
            parent_support: BTreeSet::new(),
            topology_sector: None,
            face_band: None,
            movable_vertices: Vec::new(),
            fixed_vertices: Vec::new(),
        }
    }

    #[test]
    fn sector_source_faces_partition_annulus() {
        let (_, stratified, atlas, _, _) = fixture();
        assert_eq!(atlas.sectors.len(), 14);
        assert_eq!(
            atlas
                .source_face_owner
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            stratified
                .coupled
                .annulus_face_slots
                .iter()
                .copied()
                .collect()
        );
    }

    #[test]
    fn sector_custom_triangles_partition_transition() {
        let (_, _, atlas, _, _) = fixture();
        assert_eq!(
            atlas.custom_face_owner.len(),
            atlas
                .sectors
                .values()
                .map(|sector| sector.custom_triangles.len())
                .sum()
        );
    }

    #[test]
    fn sector_boundary_matches_polygon() {
        let (_, _, atlas, _, _) = fixture();
        assert!(atlas.sectors.values().all(|sector| {
            sector.boundary_cycles.len() == 1 && !sector.boundary_edges.is_empty()
        }));
    }

    #[test]
    fn sector_source_and_custom_area_intervals_close() {
        let (_, _, atlas, _, _) = fixture();
        assert!(atlas.sectors.values().all(|sector| {
            sector
                .source_area_interval
                .sub_out(sector.custom_area_interval)
                .contains(0.0)
        }));
    }

    #[test]
    fn every_strict_custom_face_has_one_sector_owner() {
        let (source, _, atlas, mesh, _) = fixture();
        let face = mesh
            .mesh
            .active_triangle_slots()
            .find(|&face| mesh.triangle_addresses[face].is_none())
            .unwrap();
        let atoms = build_strict_recovery_atoms(source, mesh, &[angle(face, -0.1)], atlas).unwrap();
        assert!(matches!(
            atoms.as_slice(),
            [RecoveryAtom::Sector {
                sector_id,
                mixed_faces,
                ..
            }] if mixed_faces.contains(&face)
                && mixed_faces.len() == atlas.sectors[sector_id].custom_triangles.len()
        ));
    }

    #[test]
    fn every_hierarchy_face_has_one_leaf_owner() {
        let (source, _, atlas, mesh, _) = fixture();
        let face = mesh
            .mesh
            .active_triangle_slots()
            .find(|&face| mesh.triangle_addresses[face].is_some())
            .unwrap();
        let atoms = build_strict_recovery_atoms(source, mesh, &[angle(face, -0.1)], atlas).unwrap();
        assert!(matches!(
            atoms.as_slice(),
            [RecoveryAtom::HierarchyLeaf { mixed_face, source_faces, .. }]
                if *mixed_face == face && !source_faces.is_empty()
        ));
    }

    #[test]
    fn guard_angle_never_creates_recovery_atom() {
        let (source, _, atlas, mesh, _) = fixture();
        let face = mesh.mesh.active_triangle_slots().next().unwrap();
        assert!(
            build_strict_recovery_atoms(source, mesh, &[angle(face, 0.1)], atlas)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn shared_vertex_only_is_separate_until_it_is_an_active_constraint() {
        let (source, _, _, mesh, _) = fixture();
        let hierarchy_faces = mesh
            .mesh
            .active_triangle_slots()
            .filter(|&face| mesh.triangle_addresses[face].is_some())
            .collect::<Vec<_>>();
        let (left, right, shared) = hierarchy_faces
            .iter()
            .enumerate()
            .find_map(|(index, &left)| {
                hierarchy_faces[index + 1..].iter().find_map(|&right| {
                    let left_vertices = mesh.mesh.triangles()[left]
                        .into_iter()
                        .collect::<BTreeSet<_>>();
                    let right_vertices = mesh.mesh.triangles()[right]
                        .into_iter()
                        .collect::<BTreeSet<_>>();
                    let shared = left_vertices
                        .intersection(&right_vertices)
                        .copied()
                        .collect::<Vec<_>>();
                    (shared.len() == 1).then_some((left, right, shared[0]))
                })
            })
            .expect("Frozen N6 has hierarchy faces meeting at one vertex");
        let atoms = [left, right]
            .into_iter()
            .map(|mixed_face| RecoveryAtom::HierarchyLeaf {
                address: mesh.triangle_addresses[mixed_face].unwrap(),
                mixed_face,
                source_faces: source_descendant_faces(
                    source,
                    mesh.triangle_addresses[mixed_face].unwrap(),
                ),
            })
            .collect::<Vec<_>>();
        let separate = build_local_recovery_components(
            source,
            mesh,
            &[angle(left, -0.1), angle(right, -0.1)],
            &atoms,
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(separate.len(), 2);
        assert!(separate
            .iter()
            .all(|component| component.topology == PromotionPatchTopology::Disk));

        let mut coupled_angles = [angle(left, -0.1), angle(right, -0.1)];
        for item in &mut coupled_angles {
            item.movable_vertices = vec![shared];
        }
        assert_eq!(
            build_local_recovery_components(
                source,
                mesh,
                &coupled_angles,
                &atoms,
                &BTreeSet::new(),
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn complete_sector_ring_is_annular_and_preserves_the_coarse_core() {
        let (source, _, atlas, mesh, core_parents) = fixture();
        let angles = atlas
            .sectors
            .values()
            .map(|sector| {
                let key = *sector.custom_triangles.first().unwrap();
                let face = mesh
                    .mesh
                    .active_triangle_slots()
                    .find(|&face| {
                        mesh.triangle_addresses[face].is_none()
                            && canonical_triangle(
                                mesh.mesh.triangles()[face]
                                    .map(|site| mesh.source_vertex_slots[site].unwrap()),
                            ) == key
                    })
                    .unwrap();
                angle(face, -0.1)
            })
            .collect::<Vec<_>>();
        let atoms = build_strict_recovery_atoms(source, mesh, &angles, atlas).unwrap();
        let components =
            build_local_recovery_components(source, mesh, &angles, &atoms, core_parents).unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].atoms.len(), 14);
        assert_eq!(components[0].boundary_cycles.len(), 2);
        assert!(matches!(
            components[0].topology,
            PromotionPatchTopology::Annulus { .. }
        ));
        assert_eq!(components[0].protected_coarse_regions.len(), 1);
        assert_eq!(
            components[0].protected_coarse_regions[0].retained_parents,
            *core_parents
        );
    }
}
