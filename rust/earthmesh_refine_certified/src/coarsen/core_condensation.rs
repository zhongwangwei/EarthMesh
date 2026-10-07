use crate::mother_grid::{push_oriented, MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::{CartesianPoint, MeshState};
use std::collections::BTreeSet;
use std::time::Instant;

/// Exact hierarchy leaves. Each leaf is a source face or one of its ancestors.
pub type HierarchyFaceKey = TriangleAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyLeafSet {
    pub leaves: BTreeSet<HierarchyFaceKey>,
}

impl HierarchyLeafSet {
    /// Sorted first, the addresses build the set in one pass; inserted one
    /// by one, tens of millions of them were much of a large search's time
    /// (guide 11.135). Whatever is amiss -- a face without an address, at
    /// the wrong level, or twice -- is reported by the one-by-one build, as
    /// it always was.
    pub fn from_mother_grid(grid: &MotherGrid) -> Result<Self, String> {
        let mut addresses = Vec::with_capacity(grid.triangle_addresses.len());
        for face in grid.mesh.active_triangle_slots() {
            match grid
                .triangle_addresses
                .get(face)
                .and_then(|address| *address)
            {
                Some(address) if address.n == grid.subdivision || grid.region.is_some() => {
                    addresses.push(address);
                }
                _ => return Self::inserted_one_by_one(grid),
            }
        }
        addresses.sort_unstable();
        if addresses.windows(2).any(|pair| pair[0] == pair[1]) {
            return Self::inserted_one_by_one(grid);
        }
        Ok(Self {
            leaves: addresses.into_iter().collect(),
        })
    }

    /// The set built face by face in slot order, which names the first face
    /// that has no address, the wrong level or a repeated address.
    fn inserted_one_by_one(grid: &MotherGrid) -> Result<Self, String> {
        let mut leaves = BTreeSet::new();
        for face in grid.mesh.active_triangle_slots() {
            let address = grid
                .triangle_addresses
                .get(face)
                .and_then(|address| *address)
                .ok_or_else(|| format!("active face {face} has no hierarchy address"))?;
            // A region may build faces of coarser levels where nothing finer is
            // needed (design B1g); a whole grid has only its own level.
            if address.n != grid.subdivision && grid.region.is_none() {
                return Err(format!(
                    "active face {face} address subdivision {} does not match source subdivision {}",
                    address.n, grid.subdivision
                ));
            }
            if !leaves.insert(address) {
                return Err(format!(
                    "duplicate active hierarchy face address {address:?}"
                ));
            }
        }
        Ok(Self { leaves })
    }

    /// The leaves of `faces` alone -- a search window's (guide 11.144).
    pub(crate) fn from_faces(grid: &MotherGrid, faces: &BTreeSet<usize>) -> Result<Self, String> {
        let mut addresses = Vec::with_capacity(faces.len());
        for &face in faces {
            addresses.push(
                grid.triangle_addresses
                    .get(face)
                    .and_then(|address| *address)
                    .ok_or_else(|| format!("window face {face} has no hierarchy address"))?,
            );
        }
        addresses.sort_unstable();
        if let Some(pair) = addresses.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(format!("duplicate window face address {:?}", pair[0]));
        }
        Ok(Self {
            leaves: addresses.into_iter().collect(),
        })
    }

    pub fn condense_core(&mut self, parents: &[TriangleAddress]) -> Result<usize, String> {
        let unique = parents.iter().copied().collect::<BTreeSet<_>>();
        // A core that holds a large part of the leaves -- a whole region's at
        // the first level -- rebuilds the set in one merge; a small one
        // removes and inserts. The set is the same either way, and a missing
        // child is reported by the second, as it always was.
        if unique.len().saturating_mul(4 * MERGE_FRACTION) >= self.leaves.len() {
            if let Some(leaves) = self.condensed_by_merge(&unique) {
                self.leaves = leaves;
                return Ok(unique.len());
            }
        }
        let mut removals = Vec::with_capacity(unique.len().saturating_mul(4));
        for parent in &unique {
            let children = parent
                .children_2_to_1()
                .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?;
            for child in children {
                if !self.leaves.contains(&child) {
                    return Err(format!(
                        "parent {parent:?} is not a complete core leaf patch; missing child {child:?}"
                    ));
                }
                removals.push(child);
            }
        }
        for child in removals {
            self.leaves.remove(&child);
        }
        self.leaves.extend(unique.iter().copied());
        Ok(unique.len())
    }

    /// The leaves with every parent's four children replaced by the parent,
    /// walking the sorted leaves and the sorted children together; `None`
    /// when a parent has no children or a child is not a leaf.
    fn condensed_by_merge(
        &self,
        unique: &BTreeSet<TriangleAddress>,
    ) -> Option<BTreeSet<HierarchyFaceKey>> {
        let mut removals = Vec::with_capacity(unique.len().saturating_mul(4));
        for parent in unique {
            removals.extend(parent.children_2_to_1()?);
        }
        removals.sort_unstable();
        removals.dedup();
        let mut removed = removals.iter().copied().peekable();
        let mut kept = Vec::with_capacity(self.leaves.len().saturating_sub(removals.len()));
        for &leaf in &self.leaves {
            if removed.peek().is_some_and(|&child| child < leaf) {
                return None;
            }
            if removed.peek() == Some(&leaf) {
                removed.next();
            } else {
                kept.push(leaf);
            }
        }
        if removed.peek().is_some() {
            return None;
        }
        kept.extend(unique.iter().copied());
        Some(kept.into_iter().collect())
    }
}

/// `condense_core` rebuilds the leaf set by merging when the core's children
/// number at least one leaf in this many.
const MERGE_FRACTION: usize = 8;

/// A single materialized trial. Mixed fine/coarse interfaces may remain open
/// until the transition-topology stage closes them.
#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyLeafMesh {
    pub mesh: MeshState,
    pub triangle_addresses: Vec<Option<TriangleAddress>>,
    pub source_vertex_slots: Vec<Option<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreCondensationReport {
    pub parents_condensed: usize,
    pub child_faces_removed: usize,
    pub parent_faces_inserted: usize,
    pub vertices_removed: usize,
    pub core_search_states: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreCondensationTrial {
    pub leaf_set: HierarchyLeafSet,
    pub mesh: HierarchyLeafMesh,
    pub report: CoreCondensationReport,
}

pub fn rebuild_from_leaf_set(
    source: &MotherGrid,
    leaf_set: &HierarchyLeafSet,
) -> Result<HierarchyLeafMesh, String> {
    rebuild_from_leaf_set_with_custom_triangles(source, leaf_set, &BTreeSet::new(), &[])
}

pub(super) fn rebuild_from_leaf_set_with_custom_triangles(
    source: &MotherGrid,
    leaf_set: &HierarchyLeafSet,
    custom_parents: &BTreeSet<TriangleAddress>,
    custom_triangles: &[[usize; 3]],
) -> Result<HierarchyLeafMesh, String> {
    rebuild_custom_within(source, leaf_set, custom_parents, custom_triangles, None)
}

/// `rebuild_from_leaf_set_with_custom_triangles`, over `within` alone when
/// given (`rebuild_within`).
pub(super) fn rebuild_custom_within(
    source: &MotherGrid,
    leaf_set: &HierarchyLeafSet,
    custom_parents: &BTreeSet<TriangleAddress>,
    custom_triangles: &[[usize; 3]],
    within: Option<&BTreeSet<usize>>,
) -> Result<HierarchyLeafMesh, String> {
    let mut custom_face_slots = BTreeSet::new();
    for &parent in custom_parents {
        for child in source_faces_under(source, parent)? {
            custom_face_slots.insert(source_face_slot(source, child)?);
        }
    }
    rebuild_within(
        source,
        leaf_set,
        &custom_face_slots,
        custom_triangles,
        within,
    )
}

pub(super) fn rebuild_from_leaf_set_with_custom_face_slots(
    source: &MotherGrid,
    leaf_set: &HierarchyLeafSet,
    custom_face_slots: &BTreeSet<usize>,
    custom_triangles: &[[usize; 3]],
) -> Result<HierarchyLeafMesh, String> {
    rebuild_within(source, leaf_set, custom_face_slots, custom_triangles, None)
}

/// Which source faces a rebuild covers, as it claims them: every active one
/// in a table the size of the mesh, or a window's in a set (guide 11.144).
enum Coverage<'a> {
    Whole {
        covered: Vec<bool>,
        /// The faces to cover, when a window's.
        within: Option<&'a BTreeSet<usize>>,
    },
    Window {
        faces: &'a BTreeSet<usize>,
        claimed: std::collections::HashSet<usize>,
    },
}

impl Coverage<'_> {
    /// Claims `slot`; whether it was claimed already.
    fn claim(&mut self, slot: usize) -> Result<bool, String> {
        match self {
            Coverage::Whole { covered, .. } => Ok(std::mem::replace(&mut covered[slot], true)),
            Coverage::Window { faces, claimed } => {
                if !faces.contains(&slot) {
                    return Err(format!("source face {slot} lies outside the search window"));
                }
                Ok(!claimed.insert(slot))
            }
        }
    }

    /// The first face to cover that is not.
    fn first_uncovered(&self, source: &MotherGrid) -> Option<usize> {
        match self {
            Coverage::Whole {
                covered,
                within: None,
            } => source
                .mesh
                .active_triangle_slots()
                .find(|&face| !covered[face]),
            Coverage::Whole {
                covered,
                within: Some(faces),
            } => faces.iter().copied().find(|&face| !covered[face]),
            Coverage::Window { faces, claimed } => {
                faces.iter().copied().find(|face| !claimed.contains(face))
            }
        }
    }
}

/// The mesh of `leaf_set` and the custom triangles, covering every active
/// source face, or with `within` only those faces -- a search window's, open
/// along its edge (guide 11.144). A window's sites and faces are numbered as
/// the whole mesh numbers them, in the same order.
pub(super) fn rebuild_within(
    source: &MotherGrid,
    leaf_set: &HierarchyLeafSet,
    custom_face_slots: &BTreeSet<usize>,
    custom_triangles: &[[usize; 3]],
    within: Option<&BTreeSet<usize>>,
) -> Result<HierarchyLeafMesh, String> {
    let timing = std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1") && within.is_none();
    let mut started = Instant::now();
    // Nested materialization detail: never add these durations to component totals.
    let mut log_detail = |phase: &str| {
        if timing {
            eprintln!(
                "earthmesh_cli: cmrc_detail phase=rebuild_{phase} elapsed_us={}",
                started.elapsed().as_micros()
            );
            started = Instant::now();
        }
    };
    let source_n = source.subdivision;
    if source_n == 0 {
        return Err("source mother subdivision must be positive".into());
    }

    // A window of a sixteenth of the level or more is kept in tables the
    // size of the level, as the whole is; a smaller one in sets.
    let sparse =
        within.is_some_and(|faces| faces.len().saturating_mul(16) < source.mesh.triangles().len());
    let mut covered = match within {
        Some(faces) if sparse => Coverage::Window {
            faces,
            claimed: std::collections::HashSet::with_capacity(faces.len()),
        },
        _ => Coverage::Whole {
            covered: vec![false; source.mesh.triangles().len()],
            within,
        },
    };
    let mut leaf_triangles = Vec::<[usize; 3]>::new();
    let mut leaf_addresses = Vec::<Option<TriangleAddress>>::new();

    for &slot in custom_face_slots {
        if !source.mesh.is_triangle_live(slot) {
            return Err(format!("custom source face {slot} is not active"));
        }
        if covered.claim(slot)? {
            return Err(format!("source face {slot} is covered more than once"));
        }
    }

    for &leaf in &leaf_set.leaves {
        if source_has_face(source, leaf) {
            let slot = source_face_slot(source, leaf)?;
            if covered.claim(slot)? {
                return Err(format!("source face {slot} is covered more than once"));
            }
            leaf_triangles.push(source.mesh.triangles()[slot]);
            leaf_addresses.push(Some(leaf));
            continue;
        }
        let mut corner_counts = std::collections::BTreeMap::<usize, usize>::new();
        for child in source_faces_under(source, leaf)? {
            let slot = source_face_slot(source, child)?;
            if covered.claim(slot)? {
                return Err(format!("source face {slot} is covered more than once"));
            }
            for site in source.mesh.triangles()[slot] {
                *corner_counts.entry(site).or_default() += 1;
            }
        }
        let corner_count = corner_counts.values().filter(|&&count| count == 1).count();
        if corner_count != 3 {
            return Err(format!(
                "hierarchy leaf {leaf:?} has {corner_count} source corner sites, expected 3"
            ));
        }
        let corners = [
            source_corner_site(source, leaf, 0)?,
            source_corner_site(source, leaf, 1)?,
            source_corner_site(source, leaf, 2)?,
        ];
        if corners
            .iter()
            .any(|site| corner_counts.get(site) != Some(&1))
        {
            return Err(format!(
                "hierarchy leaf {leaf:?} source corner ordering is inconsistent"
            ));
        }
        leaf_triangles.push(corners);
        leaf_addresses.push(Some(leaf));
    }

    for &triangle in custom_triangles {
        leaf_triangles.push(triangle);
        leaf_addresses.push(None);
    }

    if let Some(face) = covered.first_uncovered(source) {
        return Err(format!(
            "active source face {face} is not covered by the hierarchy leaves"
        ));
    }
    log_detail("coverage");

    for site in leaf_triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
    {
        if !source.mesh.is_vertex_live(site) {
            return Err(format!("hierarchy leaf uses inactive source site {site}"));
        }
    }
    // Sites numbered in source order from 2, as the whole mesh numbers them.
    let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
    let mut source_vertex_slots = vec![None, None];
    let new_of = |old: usize, new: &std::collections::HashMap<usize, usize>| new[&old];
    let mut triangles = vec![[1usize; 3]; 2];
    let mut triangle_addresses = vec![None, None];
    if !sparse {
        let mut used_sites = vec![false; source.mesh.vertices().len()];
        for site in leaf_triangles
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
        {
            used_sites[site] = true;
        }
        let mut old_to_new = vec![None; source.mesh.vertices().len()];
        for old in source.mesh.active_vertex_slots() {
            if used_sites[old] {
                old_to_new[old] = Some(vertices.len());
                vertices.push(source.mesh.vertices()[old]);
                source_vertex_slots.push(Some(old));
            }
        }
        log_detail("compact");
        for (triangle, address) in leaf_triangles.into_iter().zip(leaf_addresses) {
            let tri = triangle.map(|old| old_to_new[old].expect("used source site was compacted"));
            push_oriented(&mut triangles, &vertices, tri)?;
            triangle_addresses.push(address);
        }
    } else {
        let used_sites = leaf_triangles
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut old_to_new = std::collections::HashMap::with_capacity(used_sites.len());
        for old in used_sites {
            old_to_new.insert(old, vertices.len());
            vertices.push(source.mesh.vertices()[old]);
            source_vertex_slots.push(Some(old));
        }
        log_detail("compact");
        for (triangle, address) in leaf_triangles.into_iter().zip(leaf_addresses) {
            let tri = triangle.map(|old| new_of(old, &old_to_new));
            push_oriented(&mut triangles, &vertices, tri)?;
            triangle_addresses.push(address);
        }
    }
    log_detail("orient");

    let mesh = MeshState::from_parts(vertices, triangles).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    log_detail("mesh_state");
    Ok(HierarchyLeafMesh {
        mesh,
        triangle_addresses,
        source_vertex_slots,
    })
}

pub fn condense_hierarchy_core(
    source: &MotherGrid,
    parents: &[TriangleAddress],
) -> Result<CoreCondensationTrial, String> {
    let initial_vertices = source.mesh.vertex_count();
    let mut leaf_set = HierarchyLeafSet::from_mother_grid(source)?;
    let parents_condensed = leaf_set.condense_core(parents)?;
    let mesh = rebuild_from_leaf_set(source, &leaf_set)?;
    let child_faces_removed = parents_condensed
        .checked_mul(4)
        .ok_or_else(|| "condensed child face count overflow".to_string())?;
    let vertices_removed = initial_vertices
        .checked_sub(mesh.mesh.vertex_count())
        .ok_or_else(|| "core condensation introduced new vertices".to_string())?;
    let report = CoreCondensationReport {
        parents_condensed,
        child_faces_removed,
        parent_faces_inserted: parents_condensed,
        vertices_removed,
        core_search_states: 0,
    };
    Ok(CoreCondensationTrial {
        leaf_set,
        mesh,
        report,
    })
}

/// Whether the source built the face at `address`: a whole grid builds only
/// its own level, a region may build coarser faces where nothing finer is
/// needed (design B1g).
pub(super) fn source_has_face(source: &MotherGrid, address: TriangleAddress) -> bool {
    match &source.region {
        Some(region) => region.face_slot(address).is_some(),
        None => address.n == source.subdivision,
    }
}

/// The source faces that tile `address`: the face itself if the source
/// built it, else its children's, down to the faces the source built.
pub(super) fn source_faces_under(
    source: &MotherGrid,
    address: TriangleAddress,
) -> Result<Vec<TriangleAddress>, String> {
    if source.region.is_none() {
        return source_descendants(address, source.subdivision);
    }
    let mut faces = Vec::new();
    let mut stack = vec![address];
    while let Some(face) = stack.pop() {
        if source_has_face(source, face) {
            faces.push(face);
            continue;
        }
        if face.n >= source.subdivision {
            return Err(format!("source face {face:?} is outside the region"));
        }
        stack.extend(
            face.children_2_to_1()
                .ok_or_else(|| format!("invalid hierarchy address {face:?}"))?,
        );
    }
    Ok(faces)
}

fn source_descendants(
    address: TriangleAddress,
    source_n: usize,
) -> Result<Vec<TriangleAddress>, String> {
    if address.n == 0 || address.n > source_n || !source_n.is_multiple_of(address.n) {
        return Err(format!(
            "hierarchy address {address:?} does not divide source subdivision {source_n}"
        ));
    }
    let mut frontier = vec![address];
    while frontier.first().is_some_and(|address| address.n < source_n) {
        if frontier[0].n.checked_mul(2).is_none_or(|n| n > source_n) {
            return Err(format!(
                "hierarchy address {address:?} is not a power-of-two ancestor of source subdivision {source_n}"
            ));
        }
        let mut next = Vec::with_capacity(frontier.len() * 4);
        for leaf in frontier {
            next.extend(
                leaf.children_2_to_1()
                    .ok_or_else(|| format!("invalid hierarchy address {leaf:?}"))?,
            );
        }
        frontier = next;
    }
    Ok(frontier)
}

pub(super) fn source_corner_site(
    source: &MotherGrid,
    mut leaf: TriangleAddress,
    corner: usize,
) -> Result<usize, String> {
    if corner >= 3 {
        return Err(format!("invalid hierarchy corner {corner}"));
    }
    while leaf.n < source.subdivision && !source_has_face(source, leaf) {
        let children = leaf
            .children_2_to_1()
            .ok_or_else(|| format!("invalid hierarchy leaf {leaf:?}"))?;
        let child_index = match (leaf.orientation, corner) {
            (crate::mother_grid::TriangleOrientation::Up, 0) => 0,
            (crate::mother_grid::TriangleOrientation::Up, 1) => 1,
            (crate::mother_grid::TriangleOrientation::Up, 2) => 2,
            (crate::mother_grid::TriangleOrientation::Down, 0) => 0,
            (crate::mother_grid::TriangleOrientation::Down, 1) => 2,
            (crate::mother_grid::TriangleOrientation::Down, 2) => 1,
            _ => unreachable!(),
        };
        leaf = children[child_index];
    }
    let slot = source_face_slot(source, leaf)?;
    Ok(source.mesh.triangles()[slot][corner])
}

pub(super) fn source_face_slot(
    source: &MotherGrid,
    address: TriangleAddress,
) -> Result<usize, String> {
    if address.n != source.subdivision && source.region.is_none() {
        return Err(format!(
            "source face address subdivision {} does not match source subdivision {}",
            address.n, source.subdivision
        ));
    }
    let slot = match &source.region {
        Some(region) => region
            .face_slot(address)
            .ok_or_else(|| format!("source face {address:?} is outside the region"))?,
        None => address
            .dense_index(source.subdivision)?
            .checked_add(2)
            .ok_or_else(|| {
                format!("source face slot overflow for hierarchy address {address:?}")
            })?,
    };
    if !source.mesh.is_triangle_live(slot) {
        return Err(format!("source face {slot} for {address:?} is not active"));
    }
    let actual = source
        .triangle_addresses
        .get(slot)
        .and_then(|actual| *actual)
        .ok_or_else(|| format!("source face {slot} has no hierarchy address"))?;
    if actual != address {
        return Err(format!(
            "source face {slot} has address {actual:?}, expected {address:?}"
        ));
    }
    Ok(slot)
}

pub(super) fn uniform_leaf_mesh_to_mother_grid(
    subdivision: usize,
    source: &MotherGrid,
    leaf_mesh: HierarchyLeafMesh,
) -> Result<MotherGrid, String> {
    if leaf_mesh
        .triangle_addresses
        .iter()
        .flatten()
        .any(|address| address.n != subdivision)
    {
        return Err("condensed hierarchy leaves are not a uniform mother level".into());
    }
    let mut addresses = Vec::with_capacity(leaf_mesh.source_vertex_slots.len());
    for (slot, source_slot) in leaf_mesh.source_vertex_slots.iter().copied().enumerate() {
        let address = match source_slot {
            Some(source_slot) => Some(scale_source_vertex_address(
                source
                    .addresses
                    .get(source_slot)
                    .and_then(|address| address.as_ref())
                    .ok_or_else(|| {
                        format!("source vertex {source_slot} has no hierarchy address")
                    })?,
                source.subdivision,
                subdivision,
            )?),
            None => None,
        };
        if slot < 2 && address.is_some() {
            return Err("reserved compact vertex slot unexpectedly has an address".into());
        }
        addresses.push(address);
    }
    if source.region.is_some() {
        return Err("a uniform level of a region is not a mother grid yet".into());
    }
    Ok(MotherGrid {
        subdivision,
        mesh: leaf_mesh.mesh,
        addresses,
        triangle_addresses: leaf_mesh.triangle_addresses,
        region: None,
    })
}

fn scale_source_vertex_address(
    address: &VertexAddress,
    source_n: usize,
    target_n: usize,
) -> Result<VertexAddress, String> {
    if target_n == 0 || !source_n.is_multiple_of(target_n) {
        return Err("source and target subdivisions are not exact hierarchy levels".into());
    }
    let factor = source_n / target_n;
    if !factor.is_power_of_two() {
        return Err("source and target subdivisions are not exact power-of-two levels".into());
    }
    Ok(match address {
        VertexAddress::IcosahedronVertex(vertex) => VertexAddress::IcosahedronVertex(*vertex),
        VertexAddress::IcosahedronEdge { a, b, step, n } => {
            if *n != source_n || !step.is_multiple_of(factor) {
                return Err(format!(
                    "source edge vertex {address:?} is not retained at hierarchy level {target_n}"
                ));
            }
            VertexAddress::IcosahedronEdge {
                a: *a,
                b: *b,
                step: step / factor,
                n: target_n,
            }
        }
        VertexAddress::IcosahedronFace { face, i, j, k, n } => {
            if *n != source_n
                || !i.is_multiple_of(factor)
                || !j.is_multiple_of(factor)
                || !k.is_multiple_of(factor)
            {
                return Err(format!(
                    "source face vertex {address:?} is not retained at hierarchy level {target_n}"
                ));
            }
            VertexAddress::IcosahedronFace {
                face: *face,
                i: i / factor,
                j: j / factor,
                k: k / factor,
                n: target_n,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_custom_parent_covers_all_source_descendants_once() {
        let source = MotherGrid::generate(8).unwrap();
        let parent = MotherGrid::generate(2)
            .unwrap()
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .next()
            .unwrap();
        let mut leaf_set = HierarchyLeafSet::from_mother_grid(&source).unwrap();
        for child in source_descendants(parent, source.subdivision).unwrap() {
            leaf_set.leaves.remove(&child);
        }
        let custom_parents = [parent].into_iter().collect::<BTreeSet<_>>();
        let custom_triangles = [[
            source_corner_site(&source, parent, 0).unwrap(),
            source_corner_site(&source, parent, 1).unwrap(),
            source_corner_site(&source, parent, 2).unwrap(),
        ]];

        let rebuilt = rebuild_from_leaf_set_with_custom_triangles(
            &source,
            &leaf_set,
            &custom_parents,
            &custom_triangles,
        )
        .unwrap();

        assert_eq!(
            rebuilt
                .triangle_addresses
                .iter()
                .filter(|a| a.is_none())
                .count(),
            3
        );
    }

    #[test]
    fn condensing_all_parents_rebuilds_the_uniform_coarse_mesh() {
        for source_n in [2, 4, 8] {
            let source = MotherGrid::generate(source_n).unwrap();
            let expected = MotherGrid::generate(source_n / 2).unwrap();
            let parents = expected
                .triangle_addresses
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();

            let trial = condense_hierarchy_core(&source, &parents).unwrap();
            assert_eq!(trial.report.parents_condensed, parents.len());
            assert_eq!(trial.report.child_faces_removed, parents.len() * 4);
            assert_eq!(trial.report.parent_faces_inserted, parents.len());
            assert_eq!(trial.report.core_search_states, 0);
            assert_eq!(trial.mesh.mesh, expected.mesh);
            assert_eq!(trial.mesh.triangle_addresses, expected.triangle_addresses);

            let uniform =
                uniform_leaf_mesh_to_mother_grid(source_n / 2, &source, trial.mesh).unwrap();
            assert_eq!(uniform, expected);
        }
    }

    #[test]
    fn condense_core_is_atomic_when_a_child_leaf_is_missing() {
        let source = MotherGrid::generate(2).unwrap();
        let mut leaf_set = HierarchyLeafSet::from_mother_grid(&source).unwrap();
        let parent = MotherGrid::generate(1)
            .unwrap()
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .next()
            .unwrap();
        leaf_set
            .leaves
            .remove(&parent.children_2_to_1().unwrap()[0]);
        let before = leaf_set.clone();

        assert!(leaf_set.condense_core(&[parent]).is_err());
        assert_eq!(leaf_set, before);
    }

    /// The set built in one sorted pass is the one built face by face, on a
    /// whole grid and on a region; a core condensed by merging, as one
    /// condensed by removing and inserting, replaces exactly its parents'
    /// children, at every core size from one parent to all of them; and a
    /// merge that finds a child missing leaves the set alone and reports it.
    #[test]
    fn sorted_builds_and_merges_give_the_one_by_one_sets() {
        let source = MotherGrid::generate(8).unwrap();
        let built = HierarchyLeafSet::from_mother_grid(&source).unwrap();
        assert_eq!(
            built,
            HierarchyLeafSet::inserted_one_by_one(&source).unwrap()
        );
        let region = MotherGrid::generate_faces(
            8,
            source
                .triangle_addresses
                .iter()
                .flatten()
                .copied()
                .step_by(3),
        )
        .unwrap();
        assert_eq!(
            HierarchyLeafSet::from_mother_grid(&region).unwrap(),
            HierarchyLeafSet::inserted_one_by_one(&region).unwrap()
        );

        let parents = MotherGrid::generate(4)
            .unwrap()
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        for count in [1, 7, parents.len() / 8, parents.len() / 2, parents.len()] {
            let core = &parents[..count];
            let mut condensed = built.clone();
            assert_eq!(condensed.condense_core(core).unwrap(), count);
            let children = core
                .iter()
                .flat_map(|parent| parent.children_2_to_1().unwrap())
                .collect::<BTreeSet<_>>();
            let expected = built
                .leaves
                .iter()
                .copied()
                .filter(|leaf| !children.contains(leaf))
                .chain(core.iter().copied())
                .collect::<BTreeSet<_>>();
            assert_eq!(condensed.leaves, expected, "{count} parents");
        }

        let mut missing = built.clone();
        missing
            .leaves
            .remove(&parents[0].children_2_to_1().unwrap()[0]);
        let before = missing.clone();
        let error = missing.condense_core(&parents).unwrap_err();
        assert!(error.contains("missing child"), "{error}");
        assert_eq!(missing, before);
    }

    #[test]
    fn mixed_core_rebuild_covers_each_source_face_once() {
        let source = MotherGrid::generate(4).unwrap();
        let parent = MotherGrid::generate(2)
            .unwrap()
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .next()
            .unwrap();

        let trial = condense_hierarchy_core(&source, &[parent]).unwrap();

        assert_eq!(trial.report.parents_condensed, 1);
        assert_eq!(trial.report.child_faces_removed, 4);
        assert_eq!(trial.report.parent_faces_inserted, 1);
        assert_eq!(
            trial.mesh.mesh.triangle_count(),
            source.mesh.triangle_count() - 3
        );
    }
}
