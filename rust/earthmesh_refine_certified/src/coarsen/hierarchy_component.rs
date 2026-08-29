use crate::mother_grid::{MotherGrid, TriangleAddress};
use std::collections::VecDeque;

/// Canonical shared parent edge, stored with the smaller face address first.
pub type HierarchyEdgeKey = (TriangleAddress, TriangleAddress);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentRequirement {
    pub parent: TriangleAddress,
    pub maximum_required_level: usize,
    pub can_coarsen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyComponent {
    pub id: u64,
    pub parents: Vec<TriangleAddress>,
    pub boundary_edges: Vec<HierarchyEdgeKey>,
    pub core_parents: Vec<TriangleAddress>,
    pub transition_parents: Vec<TriangleAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyComponentPlan {
    pub parent_requirements: Vec<ParentRequirement>,
    pub components: Vec<HierarchyComponent>,
}

#[derive(Debug, Clone, Copy)]
struct ParentAggregate {
    address: Option<TriangleAddress>,
    child_count: u8,
    maximum_required_level: usize,
    neighbours: [usize; 3],
    degree: u8,
}

impl Default for ParentAggregate {
    fn default() -> Self {
        Self {
            address: None,
            child_count: 0,
            maximum_required_level: 0,
            neighbours: [usize::MAX; 3],
            degree: 0,
        }
    }
}

/// Plan one exact 2:1 parent layer without rebuilding or searching the mesh.
///
/// A parent's requirement is the maximum over sites touched by its four child
/// faces. Core parents have graph distance strictly greater than
/// `transition_ring_width` from the component boundary.
pub fn plan_hierarchy_components(
    grid: &MotherGrid,
    required_levels: &[usize],
    coarse_level: usize,
    transition_ring_width: usize,
) -> Result<HierarchyComponentPlan, String> {
    let fine_n = grid.subdivision;
    if required_levels.len() != grid.mesh.vertices().len() {
        return Err("required level slots must match mesh vertices".into());
    }
    if fine_n < 2 {
        return Ok(HierarchyComponentPlan {
            parent_requirements: Vec::new(),
            components: Vec::new(),
        });
    }
    if !fine_n.is_multiple_of(2) {
        return Err("hierarchy component planning requires an even subdivision".into());
    }

    let coarse_n = fine_n / 2;
    let parent_count = 20usize
        .checked_mul(coarse_n)
        .and_then(|count| count.checked_mul(coarse_n))
        .ok_or_else(|| "parent face count overflow".to_string())?;
    let mut parents = vec![ParentAggregate::default(); parent_count];
    let mut face_parents = vec![usize::MAX; grid.mesh.triangles().len()];

    for face in grid.mesh.active_triangle_slots() {
        let child = grid
            .triangle_addresses
            .get(face)
            .and_then(|address| *address)
            .ok_or_else(|| format!("active face {face} has no hierarchy address"))?;
        let parent = child
            .parent_2_to_1()
            .ok_or_else(|| format!("active face {face} has no 2-to-1 parent"))?;
        if parent.n != coarse_n {
            return Err(format!(
                "active face {face} parent subdivision {} does not match {coarse_n}",
                parent.n
            ));
        }
        let dense = parent.dense_index(coarse_n)?;
        let aggregate = &mut parents[dense];
        if let Some(existing) = aggregate.address {
            if existing != parent {
                return Err("parent dense index collision".into());
            }
        } else {
            aggregate.address = Some(parent);
        }
        aggregate.child_count = aggregate
            .child_count
            .checked_add(1)
            .ok_or_else(|| "parent child count overflow".to_string())?;
        let face_max = grid.mesh.triangles()[face]
            .into_iter()
            .map(|site| required_levels[site])
            .max()
            .unwrap_or(0);
        aggregate.maximum_required_level = aggregate.maximum_required_level.max(face_max);
        face_parents[face] = dense;
    }

    for (dense, parent) in parents.iter().enumerate() {
        if parent.address.is_none() {
            return Err(format!("parent face {dense} has no active children"));
        }
        if parent.child_count != 4 {
            return Err(format!(
                "parent face {dense} has {} active children, expected 4",
                parent.child_count
            ));
        }
    }

    for face in grid.mesh.active_triangle_slots() {
        let left = face_parents[face];
        if left == usize::MAX {
            continue;
        }
        for &neighbour in &grid.mesh.neighbours()[face] {
            if neighbour == 0 || !grid.mesh.is_triangle_live(neighbour) {
                continue;
            }
            let right = face_parents[neighbour];
            if right != usize::MAX && right != left {
                push_neighbour(&mut parents[left], right, left)?;
            }
        }
    }

    for (dense, parent) in parents.iter().enumerate() {
        if parent.degree != 3 {
            return Err(format!(
                "parent face {dense} has {} parent neighbours, expected 3",
                parent.degree
            ));
        }
    }

    let eligible = parents
        .iter()
        .map(|parent| parent.maximum_required_level <= coarse_level)
        .collect::<Vec<_>>();
    let parent_requirements = parents
        .iter()
        .map(|parent| ParentRequirement {
            parent: parent.address.expect("validated parent has address"),
            maximum_required_level: parent.maximum_required_level,
            can_coarsen: parent.maximum_required_level <= coarse_level,
        })
        .collect::<Vec<_>>();

    let mut component_by_parent = vec![usize::MAX; parent_count];
    let mut component_count = 0usize;
    let mut queue = VecDeque::new();
    for seed in 0..parent_count {
        if !eligible[seed] || component_by_parent[seed] != usize::MAX {
            continue;
        }
        component_by_parent[seed] = component_count;
        queue.push_back(seed);
        while let Some(current) = queue.pop_front() {
            for neighbour in parent_neighbours(&parents[current]) {
                if eligible[neighbour] && component_by_parent[neighbour] == usize::MAX {
                    component_by_parent[neighbour] = component_count;
                    queue.push_back(neighbour);
                }
            }
        }
        component_count += 1;
    }

    let mut components = (0..component_count)
        .map(|id| HierarchyComponent {
            id: id as u64,
            parents: Vec::new(),
            boundary_edges: Vec::new(),
            core_parents: Vec::new(),
            transition_parents: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut distances = vec![usize::MAX; parent_count];
    queue.clear();

    for dense in 0..parent_count {
        if !eligible[dense] {
            continue;
        }
        let component = component_by_parent[dense];
        let address = parents[dense]
            .address
            .expect("validated parent has address");
        components[component].parents.push(address);
        for neighbour in parent_neighbours(&parents[dense]) {
            if !eligible[neighbour] {
                components[component].boundary_edges.push(canonical_edge(
                    address,
                    parents[neighbour]
                        .address
                        .expect("validated neighbour has address"),
                ));
                if distances[dense] == usize::MAX {
                    distances[dense] = 0;
                    queue.push_back(dense);
                }
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        let next_distance = distances[current].saturating_add(1);
        for neighbour in parent_neighbours(&parents[current]) {
            if eligible[neighbour] && distances[neighbour] == usize::MAX {
                distances[neighbour] = next_distance;
                queue.push_back(neighbour);
            }
        }
    }

    for dense in 0..parent_count {
        if !eligible[dense] {
            continue;
        }
        let component = component_by_parent[dense];
        let address = parents[dense]
            .address
            .expect("validated parent has address");
        if distances[dense] <= transition_ring_width {
            components[component].transition_parents.push(address);
        } else {
            components[component].core_parents.push(address);
        }
    }
    Ok(HierarchyComponentPlan {
        parent_requirements,
        components,
    })
}

fn push_neighbour(
    parent: &mut ParentAggregate,
    neighbour: usize,
    dense: usize,
) -> Result<(), String> {
    if parent.neighbours[..parent.degree as usize].contains(&neighbour) {
        return Ok(());
    }
    if parent.degree as usize == parent.neighbours.len() {
        return Err(format!("parent face {dense} has more than 3 neighbours"));
    }
    parent.neighbours[parent.degree as usize] = neighbour;
    parent.degree += 1;
    Ok(())
}

fn parent_neighbours(parent: &ParentAggregate) -> impl Iterator<Item = usize> + '_ {
    parent.neighbours[..parent.degree as usize].iter().copied()
}

fn canonical_edge(left: TriangleAddress, right: TriangleAddress) -> HierarchyEdgeKey {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MotherGrid;

    #[test]
    fn n_less_than_two_returns_empty_plan() {
        let grid = MotherGrid::generate(1).unwrap();
        let plan =
            plan_hierarchy_components(&grid, &vec![0; grid.mesh.vertices().len()], 0, 1).unwrap();
        assert!(plan.parent_requirements.is_empty());
        assert!(plan.components.is_empty());
    }

    #[test]
    fn global_eligible_grid_is_one_core_component() {
        let grid = MotherGrid::generate(2).unwrap();
        let plan =
            plan_hierarchy_components(&grid, &vec![0; grid.mesh.vertices().len()], 0, 1).unwrap();
        assert_eq!(plan.parent_requirements.len(), 20);
        assert_eq!(plan.components.len(), 1);
        assert_eq!(plan.components[0].parents.len(), 20);
        assert_eq!(plan.components[0].core_parents.len(), 20);
        assert!(plan.components[0].transition_parents.is_empty());
        assert!(plan.components[0].boundary_edges.is_empty());
    }

    #[test]
    fn odd_subdivision_is_rejected() {
        let grid = MotherGrid::generate(3).unwrap();
        assert!(
            plan_hierarchy_components(&grid, &vec![0; grid.mesh.vertices().len()], 0, 1)
                .unwrap_err()
                .contains("even subdivision")
        );
    }
}
