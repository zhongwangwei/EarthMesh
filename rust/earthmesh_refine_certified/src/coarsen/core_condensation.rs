use crate::mother_grid::{push_oriented, MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::{CartesianPoint, MeshState};
use std::collections::BTreeSet;

/// Exact hierarchy leaves. Each leaf is a source face or one of its ancestors.
pub type HierarchyFaceKey = TriangleAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyLeafSet {
    pub leaves: BTreeSet<HierarchyFaceKey>,
}

impl HierarchyLeafSet {
    pub fn from_mother_grid(grid: &MotherGrid) -> Result<Self, String> {
        let mut leaves = BTreeSet::new();
        for face in grid.mesh.active_triangle_slots() {
            let address = grid
                .triangle_addresses
                .get(face)
                .and_then(|address| *address)
                .ok_or_else(|| format!("active face {face} has no hierarchy address"))?;
            if address.n != grid.subdivision {
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

    pub fn condense_core(&mut self, parents: &[TriangleAddress]) -> Result<usize, String> {
        let unique = parents.iter().copied().collect::<BTreeSet<_>>();
        let mut trial = self.leaves.clone();
        for parent in &unique {
            let children = parent
                .children_2_to_1()
                .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?;
            for child in children {
                if !trial.remove(&child) {
                    return Err(format!(
                        "parent {parent:?} is not a complete core leaf patch; missing child {child:?}"
                    ));
                }
            }
            trial.insert(*parent);
        }
        self.leaves = trial;
        Ok(unique.len())
    }
}

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
    let source_n = source.subdivision;
    if source_n == 0 {
        return Err("source mother subdivision must be positive".into());
    }

    let source_faces = source.mesh.triangles().len();
    let mut covered = vec![false; source_faces];
    let mut leaf_triangles = Vec::<[usize; 3]>::new();
    let mut leaf_addresses = Vec::<Option<TriangleAddress>>::new();

    for &parent in custom_parents {
        for child in source_descendants(parent, source_n)? {
            let slot = source_face_slot(source, child)?;
            if std::mem::replace(&mut covered[slot], true) {
                return Err(format!("source face {slot} is covered more than once"));
            }
        }
    }

    for &leaf in &leaf_set.leaves {
        if leaf.n == source_n {
            let slot = source_face_slot(source, leaf)?;
            if std::mem::replace(&mut covered[slot], true) {
                return Err(format!("source face {slot} is covered more than once"));
            }
            leaf_triangles.push(source.mesh.triangles()[slot]);
            leaf_addresses.push(Some(leaf));
            continue;
        }
        let mut corner_counts = std::collections::BTreeMap::<usize, usize>::new();
        for child in source_descendants(leaf, source_n)? {
            let slot = source_face_slot(source, child)?;
            if std::mem::replace(&mut covered[slot], true) {
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

    for face in source.mesh.active_triangle_slots() {
        if !covered[face] {
            return Err(format!(
                "active source face {face} is not covered by the hierarchy leaves"
            ));
        }
    }

    let mut used_sites = vec![false; source.mesh.vertices().len()];
    for site in leaf_triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
    {
        if !source.mesh.is_vertex_live(site) {
            return Err(format!("hierarchy leaf uses inactive source site {site}"));
        }
        used_sites[site] = true;
    }
    let mut old_to_new = vec![None; source.mesh.vertices().len()];
    let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
    let mut source_vertex_slots = vec![None, None];
    for old in source.mesh.active_vertex_slots() {
        if used_sites[old] {
            old_to_new[old] = Some(vertices.len());
            vertices.push(source.mesh.vertices()[old]);
            source_vertex_slots.push(Some(old));
        }
    }

    let mut triangles = vec![[1usize; 3]; 2];
    let mut triangle_addresses = vec![None, None];
    for (triangle, address) in leaf_triangles.into_iter().zip(leaf_addresses) {
        let tri = triangle.map(|old| old_to_new[old].expect("used source site was compacted"));
        push_oriented(&mut triangles, &vertices, tri)?;
        triangle_addresses.push(address);
    }

    let mesh = MeshState::from_parts(vertices, triangles).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
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

fn source_corner_site(
    source: &MotherGrid,
    mut leaf: TriangleAddress,
    corner: usize,
) -> Result<usize, String> {
    if corner >= 3 {
        return Err(format!("invalid hierarchy corner {corner}"));
    }
    while leaf.n < source.subdivision {
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
    if address.n != source.subdivision {
        return Err(format!(
            "source face address subdivision {} does not match source subdivision {}",
            address.n, source.subdivision
        ));
    }
    let slot = address
        .dense_index(source.subdivision)?
        .checked_add(2)
        .ok_or_else(|| format!("source face slot overflow for hierarchy address {address:?}"))?;
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
    Ok(MotherGrid {
        subdivision,
        mesh: leaf_mesh.mesh,
        addresses,
        triangle_addresses: leaf_mesh.triangle_addresses,
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
