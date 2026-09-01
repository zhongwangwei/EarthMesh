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
pub enum PromotionPatchTopology {
    WholeSphere,
    Disk,
    Annulus { protected_hole_id: u64 },
    MultiHole { protected_holes: Vec<u64> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedCoarseRegion {
    pub id: u64,
    pub retained_parents: BTreeSet<TriangleAddress>,
    pub descendant_source_faces: BTreeSet<usize>,
    pub boundary_cycle: Vec<usize>,
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
    pub topology: PromotionPatchTopology,
    pub protected_coarse_regions: Vec<ProtectedCoarseRegion>,
    pub fine_exterior_seed_face: Option<usize>,
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
    build_promotion_patch_for_transition(source, component, level, &BTreeSet::new())
}

pub fn build_promotion_patch_for_transition(
    source: &MotherGrid,
    component: &ViolationComponent,
    level: PromotionLevel,
    transition_parents: &BTreeSet<TriangleAddress>,
) -> Result<PromotionPatch, String> {
    build_promotion_patch_for_transition_with_protected_regions(
        source,
        component,
        level,
        transition_parents,
        &[],
        None,
    )
}

pub fn build_promotion_patch_for_transition_with_protected_regions(
    source: &MotherGrid,
    component: &ViolationComponent,
    level: PromotionLevel,
    transition_parents: &BTreeSet<TriangleAddress>,
    protected_regions: &[ProtectedCoarseRegion],
    fine_exterior_seed_face: Option<usize>,
) -> Result<PromotionPatch, String> {
    if level == PromotionLevel::P0LocalTopologyOnly {
        return Err("P0 does not restore source faces".into());
    }
    validate_source_faces(source, &component.source_faces)?;
    for (index, region) in protected_regions.iter().enumerate() {
        validate_protected_region(source, region)?;
        if protected_regions[..index]
            .iter()
            .any(|other| other.id >= region.id)
        {
            return Err("protected coarse regions must have unique increasing ids".into());
        }
        if protected_regions[..index].iter().any(|other| {
            !other
                .descendant_source_faces
                .is_disjoint(&region.descendant_source_faces)
        }) {
            return Err("protected coarse regions must be face-disjoint".into());
        }
    }
    let effective_protected = if level == PromotionLevel::P5SafeMotherFallback {
        &[][..]
    } else {
        protected_regions
    };
    let connected_interior = connect_faces(source, component.source_faces.clone())?;
    let interior_seed = if connected_interior.len() == source.mesh.triangle_count() {
        None
    } else {
        Some(resolve_exterior_seed(
            source,
            &connected_interior,
            transition_parents,
            fine_exterior_seed_face,
        )?)
    };
    let interior_faces = fill_unprotected_holes(
        source,
        connected_interior,
        interior_seed,
        effective_protected,
    )?;
    let mut source_faces = interior_faces.clone();
    match level {
        PromotionLevel::P0LocalTopologyOnly | PromotionLevel::P1RestoreSourceFaces => {}
        PromotionLevel::P2RestoreOneParentRing => {
            source_faces = expand_one_parent_ring(source, &source_faces, &component.parent_faces)?;
        }
        PromotionLevel::P3RestoreTwoParentRings => {
            source_faces = expand_one_parent_ring(source, &source_faces, &component.parent_faces)?;
            source_faces = expand_one_parent_ring(source, &source_faces, &BTreeSet::new())?;
        }
        PromotionLevel::P4RestoreWholeTransitionComponent => {
            if transition_parents.is_empty() {
                return Err("P4 requires the current transition-component parents".into());
            }
            source_faces = expand_one_parent_ring(source, &source_faces, &component.parent_faces)?;
            source_faces = expand_one_parent_ring(source, &source_faces, &BTreeSet::new())?;
            source_faces.extend(source.mesh.active_triangle_slots().filter(|&face| {
                source_parent(source, face).is_ok_and(|parent| transition_parents.contains(&parent))
            }));
        }
        PromotionLevel::P5SafeMotherFallback => {
            source_faces = source.mesh.active_triangle_slots().collect();
        }
    }
    source_faces = connect_faces(source, source_faces)?;
    let final_seed = if source_faces.len() == source.mesh.triangle_count() {
        None
    } else {
        Some(resolve_exterior_seed(
            source,
            &source_faces,
            transition_parents,
            fine_exterior_seed_face,
        )?)
    };
    source_faces = fill_unprotected_holes(source, source_faces, final_seed, effective_protected)?;
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
    let topology = classify_patch_topology(
        source,
        &source_faces,
        &boundary_cycles,
        effective_protected,
        final_seed,
    )?;
    let source_mesh_fingerprint = mesh_fingerprint(&source.mesh);
    let mut patch = PromotionPatch {
        id: component.id,
        level,
        source_faces,
        hierarchy_parents,
        interior_faces,
        collar_faces,
        boundary_cycles,
        topology,
        protected_coarse_regions: effective_protected.to_vec(),
        fine_exterior_seed_face: final_seed,
        protected_exterior_faces,
        source_mesh_fingerprint,
        patch_fingerprint: 0,
    };
    patch.patch_fingerprint = patch_fingerprint(&patch);
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
    for region in &patch.protected_coarse_regions {
        validate_protected_region(source, region)?;
        if !region
            .descendant_source_faces
            .is_subset(&patch.protected_exterior_faces)
        {
            return Err("protected coarse region is not outside promotion".into());
        }
    }
    if patch
        .fine_exterior_seed_face
        .is_some_and(|seed| !patch.protected_exterior_faces.contains(&seed))
    {
        return Err("fine exterior seed is not outside promotion".into());
    }
    if classify_patch_topology(
        source,
        &patch.source_faces,
        &patch.boundary_cycles,
        &patch.protected_coarse_regions,
        patch.fine_exterior_seed_face,
    )? != patch.topology
    {
        return Err("promotion patch topology classification mismatch".into());
    }
    let parents = patch
        .source_faces
        .iter()
        .map(|&face| source_parent(source, face))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if parents != patch.hierarchy_parents {
        return Err("promotion hierarchy-parent coverage mismatch".into());
    }
    if patch.patch_fingerprint != patch_fingerprint(patch) {
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

fn resolve_exterior_seed(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
    transition_parents: &BTreeSet<TriangleAddress>,
    requested: Option<usize>,
) -> Result<usize, String> {
    if let Some(seed) = requested {
        if !source.mesh.is_triangle_live(seed) || faces.contains(&seed) {
            return Err("fine exterior seed must be a live source face outside the patch".into());
        }
        return Ok(seed);
    }
    source
        .mesh
        .active_triangle_slots()
        .find(|face| {
            !faces.contains(face)
                && (transition_parents.is_empty()
                    || source_parent(source, *face)
                        .is_ok_and(|parent| !transition_parents.contains(&parent)))
        })
        .or_else(|| {
            source
                .mesh
                .active_triangle_slots()
                .find(|face| !faces.contains(face))
        })
        .ok_or_else(|| "promotion patch has no fine exterior face".into())
}

fn fill_unprotected_holes(
    source: &MotherGrid,
    mut faces: BTreeSet<usize>,
    exterior_seed: Option<usize>,
    protected_regions: &[ProtectedCoarseRegion],
) -> Result<BTreeSet<usize>, String> {
    let complement = source
        .mesh
        .active_triangle_slots()
        .filter(|face| !faces.contains(face))
        .collect::<BTreeSet<_>>();
    if complement.is_empty() {
        return Ok(faces);
    }
    let seed = exterior_seed
        .ok_or_else(|| "non-global promotion patch needs an exterior seed".to_string())?;
    if !complement.contains(&seed) {
        return Err("fine exterior seed was absorbed by promotion".into());
    }
    let components = face_components(source, &complement);
    let exterior = components
        .iter()
        .position(|component| component.contains(&seed))
        .ok_or_else(|| "fine exterior seed has no complement component".to_string())?;
    let mut protected_components = BTreeSet::new();
    for region in protected_regions {
        if !region.descendant_source_faces.is_disjoint(&faces) {
            return Err(format!(
                "promotion overlaps protected coarse region {}",
                region.id
            ));
        }
        let matches = components
            .iter()
            .enumerate()
            .filter(|(_, component)| region.descendant_source_faces.is_subset(component))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 || matches[0] == exterior {
            return Err(format!(
                "protected coarse region {} is not one non-exterior complement component",
                region.id
            ));
        }
        protected_components.insert(matches[0]);
    }
    for (index, hole) in components.into_iter().enumerate() {
        if index != exterior && !protected_components.contains(&index) {
            faces.extend(hole);
        }
    }
    Ok(faces)
}

pub fn build_protected_coarse_region(
    source: &MotherGrid,
    id: u64,
    retained_parents: BTreeSet<TriangleAddress>,
) -> Result<ProtectedCoarseRegion, String> {
    if retained_parents.is_empty() {
        return Err("protected coarse region requires retained parents".into());
    }
    let descendant_source_faces = source
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            source_parent(source, face).is_ok_and(|parent| retained_parents.contains(&parent))
        })
        .collect::<BTreeSet<_>>();
    if descendant_source_faces.is_empty() || !faces_are_connected(source, &descendant_source_faces)
    {
        return Err("protected coarse descendants must be non-empty and connected".into());
    }
    let mut cycles = boundary_cycles(source, &descendant_source_faces)?;
    if cycles.len() != 1 {
        return Err("protected coarse region must have one boundary cycle".into());
    }
    let region = ProtectedCoarseRegion {
        id,
        retained_parents,
        descendant_source_faces,
        boundary_cycle: cycles.remove(0),
    };
    validate_protected_region(source, &region)?;
    Ok(region)
}

fn validate_protected_region(
    source: &MotherGrid,
    region: &ProtectedCoarseRegion,
) -> Result<(), String> {
    if region.retained_parents.is_empty() || region.descendant_source_faces.is_empty() {
        return Err("protected coarse region is empty".into());
    }
    validate_source_faces(source, &region.descendant_source_faces)?;
    if !faces_are_connected(source, &region.descendant_source_faces) {
        return Err("protected coarse region is not connected".into());
    }
    let expected_faces = source
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            source_parent(source, face)
                .is_ok_and(|parent| region.retained_parents.contains(&parent))
        })
        .collect::<BTreeSet<_>>();
    if expected_faces != region.descendant_source_faces
        || boundary_cycles(source, &region.descendant_source_faces)?
            != vec![region.boundary_cycle.clone()]
    {
        return Err("protected coarse region evidence does not match the source hierarchy".into());
    }
    Ok(())
}

fn classify_patch_topology(
    source: &MotherGrid,
    faces: &BTreeSet<usize>,
    boundary_cycles: &[Vec<usize>],
    protected_regions: &[ProtectedCoarseRegion],
    exterior_seed: Option<usize>,
) -> Result<PromotionPatchTopology, String> {
    let topology = if faces.len() == source.mesh.triangle_count() {
        if !boundary_cycles.is_empty() || exterior_seed.is_some() || !protected_regions.is_empty() {
            return Err("whole-sphere promotion has boundary or protected-hole evidence".into());
        }
        PromotionPatchTopology::WholeSphere
    } else if protected_regions.is_empty() {
        if boundary_cycles.len() != 1 {
            return Err("disk promotion must have one boundary cycle".into());
        }
        PromotionPatchTopology::Disk
    } else if protected_regions.len() == 1 && boundary_cycles.len() == 2 {
        PromotionPatchTopology::Annulus {
            protected_hole_id: protected_regions[0].id,
        }
    } else if boundary_cycles.len() == protected_regions.len() + 1 {
        PromotionPatchTopology::MultiHole {
            protected_holes: protected_regions.iter().map(|region| region.id).collect(),
        }
    } else {
        return Err("promotion boundary count does not match protected coarse holes".into());
    };
    let expected_euler = match &topology {
        PromotionPatchTopology::WholeSphere => 2,
        PromotionPatchTopology::Disk => 1,
        PromotionPatchTopology::Annulus { .. } => 0,
        PromotionPatchTopology::MultiHole { protected_holes } => 1 - protected_holes.len() as isize,
    };
    let actual_euler = patch_euler(source, faces);
    if actual_euler != expected_euler {
        return Err(format!(
            "promotion patch Euler mismatch: expected {expected_euler}, got {actual_euler}"
        ));
    }
    Ok(topology)
}

fn patch_euler(source: &MotherGrid, faces: &BTreeSet<usize>) -> isize {
    let mut vertices = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for &face in faces {
        let triangle = source.mesh.triangles()[face];
        vertices.extend(triangle);
        edges.extend([
            canonical_edge(triangle[0], triangle[1]),
            canonical_edge(triangle[1], triangle[2]),
            canonical_edge(triangle[2], triangle[0]),
        ]);
    }
    vertices.len() as isize - edges.len() as isize + faces.len() as isize
}

fn canonical_edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
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

fn patch_fingerprint(patch: &PromotionPatch) -> u64 {
    let mut values = vec![
        patch.source_mesh_fingerprint,
        patch.id,
        patch.level as u64,
        patch.source_faces.len() as u64,
    ];
    values.extend(patch.source_faces.iter().map(|&face| face as u64));
    for cycle in &patch.boundary_cycles {
        values.push(cycle.len() as u64);
        values.extend(cycle.iter().map(|&site| site as u64));
    }
    match &patch.topology {
        PromotionPatchTopology::WholeSphere => values.push(0),
        PromotionPatchTopology::Disk => values.push(1),
        PromotionPatchTopology::Annulus { protected_hole_id } => {
            values.extend([2, *protected_hole_id]);
        }
        PromotionPatchTopology::MultiHole { protected_holes } => {
            values.extend([3, protected_holes.len() as u64]);
            values.extend(protected_holes);
        }
    }
    values.push(
        patch
            .fine_exterior_seed_face
            .map_or(u64::MAX, |seed| seed as u64),
    );
    for region in &patch.protected_coarse_regions {
        values.extend([region.id, region.descendant_source_faces.len() as u64]);
        values.extend(
            region
                .descendant_source_faces
                .iter()
                .map(|&face| face as u64),
        );
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

    fn annular_patch() -> (MotherGrid, ProtectedCoarseRegion, PromotionPatch) {
        let source = MotherGrid::generate(4).unwrap();
        let parent = source
            .triangle_addresses
            .iter()
            .flatten()
            .find_map(|address| address.parent_2_to_1())
            .unwrap();
        let protected =
            build_protected_coarse_region(&source, 11, BTreeSet::from([parent])).unwrap();
        let protected_vertices = protected
            .descendant_source_faces
            .iter()
            .flat_map(|&face| source.mesh.triangles()[face])
            .collect::<BTreeSet<_>>();
        let exterior_parent = source
            .triangle_addresses
            .iter()
            .flatten()
            .filter_map(|address| address.parent_2_to_1())
            .find(|candidate| {
                *candidate != parent
                    && source
                        .mesh
                        .active_triangle_slots()
                        .filter(|&face| source_parent(&source, face).unwrap() == *candidate)
                        .flat_map(|face| source.mesh.triangles()[face])
                        .all(|vertex| !protected_vertices.contains(&vertex))
            })
            .unwrap();
        let exterior_faces = source
            .mesh
            .active_triangle_slots()
            .filter(|&face| source_parent(&source, face).unwrap() == exterior_parent)
            .collect::<BTreeSet<_>>();
        let ring = source
            .mesh
            .active_triangle_slots()
            .filter(|face| {
                !protected.descendant_source_faces.contains(face) && !exterior_faces.contains(face)
            })
            .collect::<BTreeSet<_>>();
        let exterior_seed = *exterior_faces.first().unwrap();
        let patch = build_promotion_patch_for_transition_with_protected_regions(
            &source,
            &component(&source, ring),
            PromotionLevel::P1RestoreSourceFaces,
            &BTreeSet::new(),
            std::slice::from_ref(&protected),
            Some(exterior_seed),
        )
        .unwrap();
        (source, protected, patch)
    }

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
    fn full_promotion_ladder_only_expands() {
        let source = MotherGrid::generate(4).unwrap();
        let component = component(&source, BTreeSet::from([2]));
        let transition = component.parent_faces.clone();
        let levels = [
            PromotionLevel::P1RestoreSourceFaces,
            PromotionLevel::P2RestoreOneParentRing,
            PromotionLevel::P3RestoreTwoParentRings,
            PromotionLevel::P4RestoreWholeTransitionComponent,
            PromotionLevel::P5SafeMotherFallback,
        ];
        let patches = levels.map(|level| {
            build_promotion_patch_for_transition(&source, &component, level, &transition).unwrap()
        });
        assert!(patches
            .windows(2)
            .all(|pair| pair[0].source_faces.is_subset(&pair[1].source_faces)));
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
    fn annular_support_preserves_protected_coarse_hole() {
        let (_, protected, patch) = annular_patch();
        assert_eq!(patch.protected_coarse_regions, [protected]);
        assert!(matches!(
            patch.topology,
            PromotionPatchTopology::Annulus {
                protected_hole_id: 11
            }
        ));
    }

    #[test]
    fn fill_unprotected_holes_does_not_fill_coarse_core() {
        let (_, protected, patch) = annular_patch();
        assert!(protected
            .descendant_source_faces
            .is_subset(&patch.protected_exterior_faces));
        assert!(protected
            .descendant_source_faces
            .is_disjoint(&patch.source_faces));
    }

    #[test]
    fn sphere_exterior_component_uses_explicit_seed() {
        let (_, _, patch) = annular_patch();
        assert!(patch
            .fine_exterior_seed_face
            .is_some_and(|seed| patch.protected_exterior_faces.contains(&seed)));
    }

    #[test]
    fn annulus_has_two_boundary_cycles_and_euler_zero() {
        let (source, _, patch) = annular_patch();
        assert_eq!(patch.boundary_cycles.len(), 2);
        assert_eq!(patch_euler(&source, &patch.source_faces), 0);
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
