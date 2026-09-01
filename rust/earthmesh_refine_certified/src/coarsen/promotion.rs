//! Exact source-face promotion patches for CLDP.

use super::{transition_topology::cycles_from_edges, ViolationComponent};
use crate::{mesh_fingerprint, MotherGrid, TriangleAddress, TriangleOrientation};
use earthmesh_mesh::CartesianPoint;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PromotionLevel {
    P0LocalTopologyOnly,
    P1RestoreSourceFaces,
    P2RestoreOneParentRing,
    P3RestoreTwoParentRings,
    P4RestoreWholeTransitionComponent,
    P5SafeMotherFallback,
}

impl PromotionLevel {
    pub fn next(self) -> Option<Self> {
        Some(match self {
            Self::P0LocalTopologyOnly => Self::P1RestoreSourceFaces,
            Self::P1RestoreSourceFaces => Self::P2RestoreOneParentRing,
            Self::P2RestoreOneParentRing => Self::P3RestoreTwoParentRings,
            Self::P3RestoreTwoParentRings => Self::P4RestoreWholeTransitionComponent,
            Self::P4RestoreWholeTransitionComponent => Self::P5SafeMotherFallback,
            Self::P5SafeMotherFallback => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionPatch {
    pub id: u64,
    pub level: PromotionLevel,
    pub source_faces: BTreeSet<usize>,
    pub hierarchy_parents: BTreeSet<TriangleAddress>,
    pub interior_faces: BTreeSet<usize>,
    pub collar_faces: BTreeSet<usize>,
    pub boundary_cycles: Vec<Vec<usize>>,
    pub protected_exterior_faces: BTreeSet<usize>,
    pub source_mesh_fingerprint: u64,
    pub patch_fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RestoredSourceFace {
    pub source_face: usize,
    pub triangle: [usize; 3],
    pub hierarchy_address: TriangleAddress,
    pub coordinates: [CartesianPoint; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RestoredSourcePatch {
    pub patch: PromotionPatch,
    pub faces: Vec<RestoredSourceFace>,
    pub restored_fingerprint: u64,
}

pub fn build_promotion_patch(
    source: &MotherGrid,
    component: &ViolationComponent,
    level: PromotionLevel,
) -> Result<PromotionPatch, String> {
    if !matches!(
        level,
        PromotionLevel::P1RestoreSourceFaces | PromotionLevel::P2RestoreOneParentRing
    ) {
        return Err("PR65 constructs only P1 or P2 promotion patches".into());
    }
    validate_source_faces(source, &component.source_faces)?;
    let interior_faces = fill_holes(
        source,
        connect_faces(source, component.source_faces.clone())?,
    );
    let mut source_faces = interior_faces.clone();
    if level == PromotionLevel::P2RestoreOneParentRing {
        source_faces = expand_one_parent_ring(source, &source_faces, &component.parent_faces)?;
        source_faces = fill_holes(source, source_faces);
    }
    let collar_faces = source_faces
        .difference(&interior_faces)
        .copied()
        .collect::<BTreeSet<_>>();
    let protected_exterior_faces = source
        .mesh
        .active_triangle_slots()
        .filter(|face| !source_faces.contains(face))
        .collect();
    let hierarchy_parents = source_faces
        .iter()
        .map(|&face| source_parent(source, face))
        .collect::<Result<_, _>>()?;
    let boundary_cycles = boundary_cycles(source, &source_faces)?;
    if boundary_cycles.len() > 1 {
        return Err("promotion patch retains a hole after finite hole filling".into());
    }
    let source_mesh_fingerprint = mesh_fingerprint(&source.mesh);
    let patch_fingerprint = patch_fingerprint(
        source_mesh_fingerprint,
        component.id,
        level,
        &source_faces,
        &boundary_cycles,
    );
    let patch = PromotionPatch {
        id: component.id,
        level,
        source_faces,
        hierarchy_parents,
        interior_faces,
        collar_faces,
        boundary_cycles,
        protected_exterior_faces,
        source_mesh_fingerprint,
        patch_fingerprint,
    };
    validate_promotion_patch(source, &patch)?;
    Ok(patch)
}

pub fn restore_source_patch(
    source: &MotherGrid,
    patch: PromotionPatch,
) -> Result<RestoredSourcePatch, String> {
    validate_promotion_patch(source, &patch)?;
    let faces = patch
        .source_faces
        .iter()
        .map(|&source_face| {
            let triangle = source.mesh.triangles()[source_face];
            let hierarchy_address = source.triangle_addresses[source_face]
                .ok_or_else(|| format!("source face {source_face} has no hierarchy address"))?;
            Ok(RestoredSourceFace {
                source_face,
                triangle,
                hierarchy_address,
                coordinates: triangle.map(|site| source.mesh.vertices()[site]),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let restored_fingerprint = restored_fingerprint(&faces);
    Ok(RestoredSourcePatch {
        patch,
        faces,
        restored_fingerprint,
    })
}

pub fn validate_promotion_patch(source: &MotherGrid, patch: &PromotionPatch) -> Result<(), String> {
    if patch.source_mesh_fingerprint != mesh_fingerprint(&source.mesh) {
        return Err("promotion patch source fingerprint mismatch".into());
    }
    validate_source_faces(source, &patch.source_faces)?;
    if !faces_are_connected(source, &patch.source_faces) {
        return Err("promotion source-face union is not edge-connected".into());
    }
    if !patch.interior_faces.is_disjoint(&patch.collar_faces)
        || patch
            .interior_faces
            .union(&patch.collar_faces)
            .copied()
            .collect::<BTreeSet<_>>()
            != patch.source_faces
    {
        return Err("promotion interior/collar coverage is not exact".into());
    }
    let all = source.mesh.active_triangle_slots().collect::<BTreeSet<_>>();
    if !patch
        .source_faces
        .is_disjoint(&patch.protected_exterior_faces)
        || patch
            .source_faces
            .union(&patch.protected_exterior_faces)
            .copied()
            .collect::<BTreeSet<_>>()
            != all
    {
        return Err("promotion/exterior source coverage is not exact".into());
    }
    if boundary_cycles(source, &patch.source_faces)? != patch.boundary_cycles {
        return Err("promotion boundary cycles do not match its source-face union".into());
    }
    let parents = patch
        .source_faces
        .iter()
        .map(|&face| source_parent(source, face))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if parents != patch.hierarchy_parents {
        return Err("promotion hierarchy-parent coverage mismatch".into());
    }
    if patch.patch_fingerprint
        != patch_fingerprint(
            patch.source_mesh_fingerprint,
            patch.id,
            patch.level,
            &patch.source_faces,
            &patch.boundary_cycles,
        )
    {
        return Err("promotion patch fingerprint mismatch".into());
    }
    Ok(())
}

fn validate_source_faces(source: &MotherGrid, faces: &BTreeSet<usize>) -> Result<(), String> {
    if faces.is_empty() {
        return Err("promotion patch has no source faces".into());
    }
    if let Some(face) = faces
        .iter()
        .find(|&&face| !source.mesh.is_triangle_live(face))
    {
        return Err(format!("promotion source face {face} is not active"));
    }
    Ok(())
}

fn faces_are_connected(source: &MotherGrid, faces: &BTreeSet<usize>) -> bool {
    let Some(&start) = faces.first() else {
        return false;
    };
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(face) = queue.pop_front() {
        if visited.insert(face) {
            queue.extend(
                source.mesh.neighbours()[face]
                    .into_iter()
                    .filter(|next| faces.contains(next)),
            );
        }
    }
    visited.len() == faces.len()
}

fn connect_faces(
    source: &MotherGrid,
    mut faces: BTreeSet<usize>,
) -> Result<BTreeSet<usize>, String> {
    loop {
        let components = face_components(source, &faces);
        if components.len() <= 1 {
            return Ok(faces);
        }
        let start = &components[0];
        let targets = components
            .iter()
            .skip(1)
            .flat_map(|component| component.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut queue = start.iter().copied().collect::<VecDeque<_>>();
        let mut predecessor = start
            .iter()
            .copied()
            .map(|face| (face, None))
            .collect::<BTreeMap<_, _>>();
        let target = loop {
            let Some(face) = queue.pop_front() else {
                return Err("source face graph cannot connect promotion support".into());
            };
            if targets.contains(&face) {
                break face;
            }
            let mut neighbours = source.mesh.neighbours()[face];
            neighbours.sort_unstable();
            for neighbour in neighbours {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    predecessor.entry(neighbour)
                {
                    entry.insert(Some(face));
                    queue.push_back(neighbour);
                }
            }
        };
        let mut current = Some(target);
        while let Some(face) = current {
            faces.insert(face);
            current = predecessor[&face];
        }
    }
}

fn fill_holes(source: &MotherGrid, mut faces: BTreeSet<usize>) -> BTreeSet<usize> {
    let complement = source
        .mesh
        .active_triangle_slots()
        .filter(|face| !faces.contains(face))
        .collect::<BTreeSet<_>>();
    let mut components = face_components(source, &complement);
    components.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| left.first().cmp(&right.first()))
    });
    for hole in components.into_iter().skip(1) {
        faces.extend(hole);
    }
    faces
}

fn face_components(source: &MotherGrid, faces: &BTreeSet<usize>) -> Vec<BTreeSet<usize>> {
    let mut remaining = faces.clone();
    let mut components = Vec::new();
    while let Some(&start) = remaining.first() {
        let mut component = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        while let Some(face) = queue.pop_front() {
            if !remaining.remove(&face) {
                continue;
            }
            component.insert(face);
            queue.extend(
                source.mesh.neighbours()[face]
                    .into_iter()
                    .filter(|next| remaining.contains(next)),
            );
        }
        components.push(component);
    }
    components
}

fn expand_one_parent_ring(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
    declared_parents: &BTreeSet<TriangleAddress>,
) -> Result<BTreeSet<usize>, String> {
    let mut parents = declared_parents.clone();
    for &face in faces {
        parents.insert(source_parent(source, face)?);
    }
    let mut expanded_parents = parents.clone();
    for face in source.mesh.active_triangle_slots() {
        if parents.contains(&source_parent(source, face)?) {
            for neighbour in source.mesh.neighbours()[face] {
                expanded_parents.insert(source_parent(source, neighbour)?);
            }
        }
    }
    let mut expanded_faces = BTreeSet::new();
    for face in source.mesh.active_triangle_slots() {
        if expanded_parents.contains(&source_parent(source, face)?) {
            expanded_faces.insert(face);
        }
    }
    Ok(expanded_faces)
}

fn source_parent(source: &MotherGrid, face: usize) -> Result<TriangleAddress, String> {
    source
        .triangle_addresses
        .get(face)
        .copied()
        .flatten()
        .map(|address| address.parent_2_to_1().unwrap_or(address))
        .ok_or_else(|| format!("source face {face} has no hierarchy address"))
}

fn boundary_cycles(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
) -> Result<Vec<Vec<usize>>, String> {
    let mut edges = BTreeMap::<(usize, usize), (usize, usize, usize)>::new();
    for &face in faces {
        let triangle = source.mesh.triangles()[face];
        for (from, to) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let key = (from.min(to), from.max(to));
            let entry = edges.entry(key).or_insert((0, from, to));
            entry.0 += 1;
            if entry.0 > 2 {
                return Err(format!("non-manifold source edge {key:?}"));
            }
        }
    }
    cycles_from_edges(
        edges
            .into_values()
            .filter_map(|(count, from, to)| (count == 1).then_some((from, to)))
            .collect(),
    )
}

fn patch_fingerprint(
    source_fingerprint: u64,
    id: u64,
    level: PromotionLevel,
    faces: &BTreeSet<usize>,
    cycles: &[Vec<usize>],
) -> u64 {
    let mut values = vec![source_fingerprint, id, level as u64, faces.len() as u64];
    values.extend(faces.iter().map(|&face| face as u64));
    for cycle in cycles {
        values.push(cycle.len() as u64);
        values.extend(cycle.iter().map(|&site| site as u64));
    }
    fingerprint(values)
}

fn restored_fingerprint(faces: &[RestoredSourceFace]) -> u64 {
    let mut values = Vec::new();
    for face in faces {
        values.extend([
            face.source_face as u64,
            face.hierarchy_address.base_face as u64,
            face.hierarchy_address.i as u64,
            face.hierarchy_address.j as u64,
            face.hierarchy_address.n as u64,
            match face.hierarchy_address.orientation {
                TriangleOrientation::Up => 0,
                TriangleOrientation::Down => 1,
            },
        ]);
        values.extend(face.triangle.map(|site| site as u64));
        for point in face.coordinates {
            values.extend([point.x.to_bits(), point.y.to_bits(), point.z.to_bits()]);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn component(source: &MotherGrid, faces: BTreeSet<usize>) -> ViolationComponent {
        let parents = faces
            .iter()
            .map(|&face| source_parent(source, face).unwrap())
            .collect();
        ViolationComponent {
            id: 7,
            angles: Vec::new(),
            source_faces: faces,
            parent_faces: parents,
            support_vertices: BTreeSet::new(),
            active_constraint_vertices: BTreeSet::new(),
        }
    }

    #[test]
    fn restore_source_patch_matches_safe_mother() {
        let source = MotherGrid::generate(4).unwrap();
        let patch = build_promotion_patch(
            &source,
            &component(&source, BTreeSet::from([2])),
            PromotionLevel::P1RestoreSourceFaces,
        )
        .unwrap();
        let restored = restore_source_patch(&source, patch).unwrap();
        for face in restored.faces {
            assert_eq!(face.triangle, source.mesh.triangles()[face.source_face]);
            assert_eq!(
                Some(face.hierarchy_address),
                source.triangle_addresses[face.source_face]
            );
            assert_eq!(
                face.coordinates,
                face.triangle.map(|site| source.mesh.vertices()[site])
            );
        }
    }

    #[test]
    fn promotion_never_lowers_level() {
        let source = MotherGrid::generate(4).unwrap();
        let component = component(&source, BTreeSet::from([2]));
        let p1 = build_promotion_patch(&source, &component, PromotionLevel::P1RestoreSourceFaces)
            .unwrap();
        let p2 = build_promotion_patch(&source, &component, PromotionLevel::P2RestoreOneParentRing)
            .unwrap();
        assert!(p1.source_faces.is_subset(&p2.source_faces));
        assert!(p1.level < p2.level);
        assert_eq!(p1.level.next(), Some(p2.level));
    }

    #[test]
    fn promotion_patch_coverage_exact() {
        let source = MotherGrid::generate(4).unwrap();
        let component = component(&source, BTreeSet::from([2, 3]));
        let patch =
            build_promotion_patch(&source, &component, PromotionLevel::P2RestoreOneParentRing)
                .unwrap();
        validate_promotion_patch(&source, &patch).unwrap();
    }

    #[test]
    fn hole_is_filled_or_rejected() {
        let source = MotherGrid::generate(2).unwrap();
        let all = source.mesh.active_triangle_slots().collect::<BTreeSet<_>>();
        let first = *all.first().unwrap();
        let second = all
            .iter()
            .copied()
            .find(|&face| {
                face != first
                    && !source.mesh.neighbours()[first].contains(&face)
                    && source.mesh.neighbours()[face]
                        .into_iter()
                        .all(|neighbour| neighbour != first)
            })
            .unwrap();
        let faces = all
            .difference(&BTreeSet::from([first, second]))
            .copied()
            .collect();
        let patch = build_promotion_patch(
            &source,
            &component(&source, faces),
            PromotionLevel::P1RestoreSourceFaces,
        )
        .unwrap();
        assert_eq!(patch.protected_exterior_faces.len(), 1);
        assert_eq!(patch.boundary_cycles.len(), 1);
    }
}
