use std::collections::{BTreeMap, BTreeSet};

use earthmesh_refine_certified::{
    coarsen::{
        condense_hierarchy_core, plan_hierarchy_components, rebuild_from_leaf_set,
        HierarchyFaceKey, HierarchyLeafSet,
    },
    MotherGrid, TriangleAddress, TriangleOrientation, VertexAddress,
};

fn parent_addresses(grid: &MotherGrid) -> Vec<TriangleAddress> {
    let mut parents = grid
        .triangle_addresses
        .iter()
        .flatten()
        .filter_map(|address| address.parent_2_to_1())
        .collect::<Vec<_>>();
    parents.sort_unstable();
    parents.dedup();
    parents
}

fn children(parent: TriangleAddress) -> BTreeSet<HierarchyFaceKey> {
    parent.children_2_to_1().unwrap().into_iter().collect()
}

fn sparse_core_parent(grid: &MotherGrid) -> TriangleAddress {
    parent_addresses(grid)
        .into_iter()
        .find(|parent| {
            parent.base_face == 0
                && parent.i == 0
                && parent.j == 0
                && parent.orientation == TriangleOrientation::Down
        })
        .unwrap()
}

fn parent_touching_icosahedron_vertex(grid: &MotherGrid) -> TriangleAddress {
    for face in grid.mesh.active_triangle_slots() {
        if grid.mesh.triangles()[face].into_iter().any(|site| {
            matches!(
                grid.addresses[site],
                Some(VertexAddress::IcosahedronVertex(_))
            )
        }) {
            return grid.triangle_addresses[face]
                .and_then(TriangleAddress::parent_2_to_1)
                .unwrap();
        }
    }
    panic!("generated mother grid has no parent touching an icosahedron vertex")
}

fn adjacent_parent_pair(grid: &MotherGrid) -> [TriangleAddress; 2] {
    for face in grid.mesh.active_triangle_slots() {
        let parent = grid.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .unwrap();
        for &neighbour in &grid.mesh.neighbours()[face] {
            let other = grid.triangle_addresses[neighbour]
                .and_then(TriangleAddress::parent_2_to_1)
                .unwrap();
            if parent != other {
                return [parent, other];
            }
        }
    }
    panic!("generated mother grid has no adjacent parents");
}

fn triangle_slot(addresses: &[Option<TriangleAddress>], address: TriangleAddress) -> usize {
    addresses
        .iter()
        .position(|&candidate| candidate == Some(address))
        .unwrap()
}

fn parent_corner_sites(grid: &MotherGrid, parent: TriangleAddress) -> BTreeSet<usize> {
    let mut uses = BTreeMap::<usize, usize>::new();
    for child in parent.children_2_to_1().unwrap() {
        let face = triangle_slot(&grid.triangle_addresses, child);
        for site in grid.mesh.triangles()[face] {
            *uses.entry(site).or_default() += 1;
        }
    }
    uses.into_iter()
        .filter_map(|(site, count)| (count == 1).then_some(site))
        .collect()
}

#[test]
fn leaf_set_condenses_core_children_atomically() {
    let grid = MotherGrid::generate(4).unwrap();
    let mut leaf_set = HierarchyLeafSet::from_mother_grid(&grid).unwrap();
    let before = leaf_set.leaves.clone();
    let valid = sparse_core_parent(&grid);
    let invalid = TriangleAddress {
        base_face: 20,
        ..valid
    };

    assert!(leaf_set.condense_core(&[valid, invalid]).is_err());
    assert_eq!(leaf_set.leaves, before);

    assert_eq!(leaf_set.condense_core(&[valid, valid]).unwrap(), 1);
    assert!(leaf_set.leaves.contains(&valid));
    assert!(children(valid)
        .into_iter()
        .all(|child| !leaf_set.leaves.contains(&child)));
}

#[test]
fn partial_core_trial_keeps_transition_fine_leaves_and_reuses_fine_vertices() {
    let grid = MotherGrid::generate(4).unwrap();
    let parent = sparse_core_parent(&grid);

    let trial = condense_hierarchy_core(&grid, &[parent]).unwrap();

    assert!(trial.leaf_set.leaves.contains(&parent));
    assert!(children(parent)
        .into_iter()
        .all(|child| !trial.leaf_set.leaves.contains(&child)));
    assert!(trial
        .leaf_set
        .leaves
        .iter()
        .any(|leaf| leaf.n == grid.subdivision));
    assert_eq!(trial.report.parents_condensed, 1);
    assert_eq!(trial.report.child_faces_removed, 4);
    assert_eq!(trial.report.parent_faces_inserted, 1);
    assert_eq!(trial.report.vertices_removed, 0);
    assert_eq!(trial.report.core_search_states, 0);
    assert!(trial.mesh.mesh.open_edge_count() > 0);

    let source_slots = trial
        .mesh
        .mesh
        .active_vertex_slots()
        .map(|slot| trial.mesh.source_vertex_slots[slot].unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        source_slots.iter().copied().collect::<BTreeSet<_>>().len(),
        source_slots.len()
    );
    assert!(source_slots
        .into_iter()
        .all(|source| grid.mesh.is_vertex_live(source)));
}

#[test]
fn full_domain_core_condensation_matches_coarser_mother_grid() {
    let fine = MotherGrid::generate(8).unwrap();
    let coarse = MotherGrid::generate(4).unwrap();
    let requirements = vec![0; fine.mesh.vertices().len()];
    let plan = plan_hierarchy_components(&fine, &requirements, 0, 0).unwrap();
    let parents = plan.components[0].core_parents.clone();

    let trial = condense_hierarchy_core(&fine, &parents).unwrap();

    assert_eq!(trial.report.parents_condensed, parents.len());
    assert_eq!(trial.report.child_faces_removed, parents.len() * 4);
    assert_eq!(trial.report.parent_faces_inserted, parents.len());
    assert_eq!(
        trial.report.vertices_removed,
        fine.mesh.vertex_count() - coarse.mesh.vertex_count()
    );
    assert_eq!(trial.report.core_search_states, 0);
    assert_eq!(trial.mesh.mesh.vertex_count(), coarse.mesh.vertex_count());
    assert_eq!(
        trial.mesh.mesh.triangle_count(),
        coarse.mesh.triangle_count()
    );
    assert_eq!(
        trial.mesh.mesh.open_edge_count(),
        coarse.mesh.open_edge_count()
    );
    assert_eq!(trial.mesh.mesh, coarse.mesh);
    assert_eq!(trial.mesh.triangle_addresses, coarse.triangle_addresses);
}

#[test]
fn adjacent_core_parents_remove_only_their_internal_fine_midpoint() {
    let fine = MotherGrid::generate(4).unwrap();
    let parents = adjacent_parent_pair(&fine);

    let trial = condense_hierarchy_core(&fine, &parents).unwrap();

    assert_eq!(trial.report.parents_condensed, 2);
    assert_eq!(trial.report.vertices_removed, 1);
    let retained = trial
        .mesh
        .source_vertex_slots
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fine.mesh
            .active_vertex_slots()
            .filter(|site| !retained.contains(site))
            .count(),
        1
    );
}

#[test]
fn seam_vertex_parent_materializes_with_corners_mapped_to_fine_sites() {
    let fine = MotherGrid::generate(4).unwrap();
    let parent = parent_touching_icosahedron_vertex(&fine);
    let mut leaf_set = HierarchyLeafSet::from_mother_grid(&fine).unwrap();
    leaf_set.condense_core(&[parent]).unwrap();

    let rebuilt = rebuild_from_leaf_set(&fine, &leaf_set).unwrap();
    let rebuilt_parent_face = triangle_slot(&rebuilt.triangle_addresses, parent);
    let actual = rebuilt.mesh.triangles()[rebuilt_parent_face]
        .into_iter()
        .map(|output| rebuilt.source_vertex_slots[output].unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, parent_corner_sites(&fine, parent));
    assert!(actual.into_iter().any(|site| matches!(
        fine.addresses[site],
        Some(VertexAddress::IcosahedronVertex(_))
    )));
}
