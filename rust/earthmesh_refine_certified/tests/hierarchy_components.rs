use std::collections::BTreeSet;

use earthmesh_refine_certified::{
    coarsen::{plan_hierarchy_components, HierarchyComponent, HierarchyEdgeKey, ParentRequirement},
    MotherGrid, TriangleAddress,
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

fn sites_for_parent(grid: &MotherGrid, parent: TriangleAddress) -> BTreeSet<usize> {
    grid.mesh
        .active_triangle_slots()
        .filter(|&face| {
            grid.triangle_addresses[face].and_then(|address| address.parent_2_to_1())
                == Some(parent)
        })
        .flat_map(|face| grid.mesh.triangles()[face])
        .collect()
}

fn isolated_parents(grid: &MotherGrid, count: usize) -> Vec<TriangleAddress> {
    let mut picked = Vec::new();
    let mut used_sites = BTreeSet::new();
    for parent in parent_addresses(grid) {
        let sites = sites_for_parent(grid, parent);
        if sites.is_disjoint(&used_sites) {
            used_sites.extend(sites);
            picked.push(parent);
            if picked.len() == count {
                break;
            }
        }
    }
    assert_eq!(picked.len(), count);
    picked
}

fn seam_parent_pair(grid: &MotherGrid) -> [TriangleAddress; 2] {
    for face in grid.mesh.active_triangle_slots() {
        let parent = grid.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .unwrap();
        for &neighbour in &grid.mesh.neighbours()[face] {
            let other = grid.triangle_addresses[neighbour]
                .and_then(TriangleAddress::parent_2_to_1)
                .unwrap();
            if parent != other && parent.base_face != other.base_face {
                return if parent < other {
                    [parent, other]
                } else {
                    [other, parent]
                };
            }
        }
    }
    panic!("generated mother grid has no cross-base-face seam");
}

fn requirement_for(
    requirements: &[ParentRequirement],
    parent: TriangleAddress,
) -> &ParentRequirement {
    requirements
        .iter()
        .find(|requirement| requirement.parent == parent)
        .unwrap()
}

fn boundary_edges(component: &HierarchyComponent) -> &[HierarchyEdgeKey] {
    &component.boundary_edges
}

#[test]
fn parent_requirement_takes_max_and_blocks_overrequired_parent() {
    let grid = MotherGrid::generate(2).unwrap();
    let parent = parent_addresses(&grid)[0];
    let mut required = vec![0; grid.mesh.vertices().len()];
    let hot_site = *sites_for_parent(&grid, parent).iter().next().unwrap();
    required[hot_site] = 2;

    let plan = plan_hierarchy_components(&grid, &required, 1, 0).unwrap();

    assert_eq!(
        requirement_for(&plan.parent_requirements, parent).maximum_required_level,
        2
    );
    assert!(!plan
        .components
        .iter()
        .any(|component| component.parents.contains(&parent)));
}

#[test]
fn disconnected_coarsenable_regions_become_stably_ordered_components() {
    let grid = MotherGrid::generate(2).unwrap();
    let parents = isolated_parents(&grid, 2);
    let mut required = vec![2; grid.mesh.vertices().len()];
    for parent in &parents {
        for site in sites_for_parent(&grid, *parent) {
            required[site] = 0;
        }
    }

    let plan = plan_hierarchy_components(&grid, &required, 1, 0).unwrap();

    assert_eq!(plan.components.len(), 2);
    assert_eq!(plan.components[0].parents, vec![parents[0]]);
    assert_eq!(plan.components[1].parents, vec![parents[1]]);
    assert!(plan.components[0].id < plan.components[1].id);
}

#[test]
fn repeated_component_planning_is_identical() {
    let grid = MotherGrid::generate(2).unwrap();
    let parents = isolated_parents(&grid, 2);
    let mut required = vec![2; grid.mesh.vertices().len()];
    for parent in &parents {
        for site in sites_for_parent(&grid, *parent) {
            required[site] = 0;
        }
    }

    let first = plan_hierarchy_components(&grid, &required, 1, 1).unwrap();
    let second = plan_hierarchy_components(&grid, &required, 1, 1).unwrap();

    assert_eq!(first, second);
}

#[test]
fn seam_crossing_parents_share_one_component_and_canonical_boundary_keys() {
    let grid = MotherGrid::generate(2).unwrap();
    let parents = seam_parent_pair(&grid);
    let mut required = vec![2; grid.mesh.vertices().len()];
    for parent in parents {
        for site in sites_for_parent(&grid, parent) {
            required[site] = 0;
        }
    }

    let plan = plan_hierarchy_components(&grid, &required, 1, 0).unwrap();

    assert_eq!(plan.components.len(), 1);
    assert_eq!(plan.components[0].parents, parents);
    assert!(plan.components[0]
        .boundary_edges
        .iter()
        .all(|&(left, right)| left < right));
    assert_eq!(
        plan.components[0]
            .boundary_edges
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        plan.components[0].boundary_edges.len()
    );
}

#[test]
fn whole_domain_coarsening_is_one_core_component_without_boundary_edges() {
    let grid = MotherGrid::generate(2).unwrap();
    let required = vec![0; grid.mesh.vertices().len()];
    let parents = parent_addresses(&grid);

    let plan = plan_hierarchy_components(&grid, &required, 1, 0).unwrap();

    assert_eq!(plan.components.len(), 1);
    assert_eq!(plan.components[0].parents, parents);
    assert!(boundary_edges(&plan.components[0]).is_empty());
    assert_eq!(plan.components[0].core_parents, plan.components[0].parents);
    assert!(plan.components[0].transition_parents.is_empty());
}

#[test]
fn component_planning_closes_levels_one_through_three() {
    for fine_n in [2, 4, 8] {
        let grid = MotherGrid::generate(fine_n).unwrap();
        let required = vec![0; grid.mesh.vertices().len()];

        let plan = plan_hierarchy_components(&grid, &required, 0, 0).unwrap();

        assert_eq!(
            plan.parent_requirements.len(),
            20 * (fine_n / 2) * (fine_n / 2)
        );
        assert_eq!(plan.components.len(), 1);
        assert_eq!(
            plan.components[0].parents.len(),
            plan.parent_requirements.len()
        );
    }
}

#[test]
fn boundary_parent_is_transition_at_width_zero() {
    let grid = MotherGrid::generate(2).unwrap();
    let parent = isolated_parents(&grid, 1)[0];
    let mut required = vec![2; grid.mesh.vertices().len()];
    for site in sites_for_parent(&grid, parent) {
        required[site] = 0;
    }

    let plan = plan_hierarchy_components(&grid, &required, 1, 0).unwrap();

    assert_eq!(plan.components.len(), 1);
    assert_eq!(plan.components[0].parents, vec![parent]);
    // The contract defines core as distance-to-boundary > width; this parent is distance zero.
    assert!(plan.components[0].core_parents.is_empty());
    assert_eq!(plan.components[0].transition_parents, vec![parent]);
}

#[test]
fn component_planning_rejects_invalid_inputs() {
    let grid = MotherGrid::generate(2).unwrap();
    assert!(plan_hierarchy_components(&grid, &[0; 3], 1, 0).is_err());

    let odd_subdivision = MotherGrid::generate(3).unwrap();
    let required = vec![0; odd_subdivision.mesh.vertices().len()];
    assert!(plan_hierarchy_components(&odd_subdivision, &required, 1, 0).is_err());
}
