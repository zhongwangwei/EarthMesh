//! Component planning over a built region (on-demand reverse coarsening,
//! design B1d in `docs/certified_mesh/on_demand_reverse_coarsening.md`).
//!
//! The whole-sphere planner (`plan_hierarchy_components_from_parent_requirements`)
//! sees every parent of a level. A region sees the parents of the base faces it
//! built (R and its frame F); the settled base faces S are never built. Every
//! settled parent can coarsen at every level, so S enters the planning as
//! blocks -- edge-connected groups of settled base faces -- that join the
//! components of the built parents they touch: those components carry an
//! implicit core whose size and smallest address are computed, not listed.

use super::hierarchy_component::{
    ExplicitParentRequirement, HierarchyComponent, HierarchyEdgeKey, ParentRequirement,
};
use crate::mother_grid::{MotherGrid, TriangleAddress, TriangleOrientation};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The settled base faces, in edge-connected blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct SettledRegion {
    base_subdivision: usize,
    block_of: BTreeMap<TriangleAddress, usize>,
    block_faces: Vec<usize>,
    block_first_face: Vec<TriangleAddress>,
    /// For each built base face, its three sides: the unit normal of the
    /// side's great circle and the base face across it.
    built_sides: BTreeMap<TriangleAddress, [([f64; 3], TriangleAddress); 3]>,
    /// Counts of the settled faces' closure at the base level: faces, edges
    /// shared with built faces, Euler characteristic, and the excess of
    /// boundary vertices where the boundary pinches (`sum of b/2 - 1` over
    /// boundary vertices with b boundary edges).
    faces: usize,
    boundary_edges: usize,
    euler: isize,
    pinch_excess: usize,
}

impl SettledRegion {
    /// The base faces of `base` outside `built`, grouped by shared edges.
    pub fn new(base: &MotherGrid, built: &BTreeSet<TriangleAddress>) -> Result<Self, String> {
        if base.region.is_some() {
            return Err("settled faces are drawn on a whole base mother".into());
        }
        let mut block_of = BTreeMap::new();
        let mut block_faces = Vec::new();
        let mut block_first_face = Vec::new();
        for start in base.mesh.active_triangle_slots() {
            let address = base.triangle_addresses[start]
                .ok_or_else(|| format!("base face {start} has no address"))?;
            if built.contains(&address) || block_of.contains_key(&address) {
                continue;
            }
            let block = block_faces.len();
            block_of.insert(address, block);
            let mut first = address;
            let mut count = 0usize;
            let mut queue = VecDeque::from([start]);
            while let Some(face) = queue.pop_front() {
                count += 1;
                let face_address = base.triangle_addresses[face].expect("checked above");
                first = first.min(face_address);
                for neighbour in base.mesh.neighbours()[face] {
                    if neighbour == 0 || !base.mesh.is_triangle_live(neighbour) {
                        continue;
                    }
                    let neighbour_address = base.triangle_addresses[neighbour]
                        .ok_or_else(|| format!("base face {neighbour} has no address"))?;
                    if built.contains(&neighbour_address)
                        || block_of.contains_key(&neighbour_address)
                    {
                        continue;
                    }
                    block_of.insert(neighbour_address, block);
                    queue.push_back(neighbour);
                }
            }
            block_faces.push(count);
            block_first_face.push(first);
        }
        // The closure of the settled faces at the base level.
        let mut vertices = BTreeSet::new();
        let mut edges = BTreeMap::<(usize, usize), usize>::new();
        let mut settled_faces = 0usize;
        for face in base.mesh.active_triangle_slots() {
            let address = base.triangle_addresses[face].expect("checked above");
            if built.contains(&address) {
                continue;
            }
            settled_faces += 1;
            let corners = base.mesh.triangles()[face];
            vertices.extend(corners);
            for side in 0..3 {
                let (a, b) = (corners[side], corners[(side + 1) % 3]);
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut boundary_degree = BTreeMap::<usize, usize>::new();
        let mut boundary_edges = 0usize;
        for (&(a, b), &count) in &edges {
            if count == 1 {
                boundary_edges += 1;
                *boundary_degree.entry(a).or_default() += 1;
                *boundary_degree.entry(b).or_default() += 1;
            }
        }
        let pinch_excess = boundary_degree
            .values()
            .map(|&degree| degree / 2 - 1)
            .sum::<usize>();
        let euler = vertices.len() as isize - edges.len() as isize + settled_faces as isize;
        let mut built_sides = BTreeMap::new();
        for face in base.mesh.active_triangle_slots() {
            let address = base.triangle_addresses[face].expect("checked above");
            if !built.contains(&address) {
                continue;
            }
            let corners = base.mesh.triangles()[face].map(|site| {
                let point = base.mesh.vertices()[site];
                [point.x, point.y, point.z]
            });
            let mut sides = [([0.0; 3], address); 3];
            for (corner, side) in sides.iter_mut().enumerate() {
                // `neighbours()[face][k]` lies across the edge opposite corner k.
                let (a, b) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                let normal = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                let length =
                    (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
                let across = base.mesh.neighbours()[face][corner];
                *side = (
                    [normal[0] / length, normal[1] / length, normal[2] / length],
                    base.triangle_addresses[across]
                        .ok_or_else(|| format!("base face {across} has no address"))?,
                );
            }
            built_sides.insert(address, sides);
        }
        Ok(Self {
            base_subdivision: base.subdivision,
            block_of,
            block_faces,
            block_first_face,
            built_sides,
            faces: settled_faces,
            boundary_edges,
            euler,
            pinch_excess,
        })
    }

    /// The settled base faces.
    pub fn faces(&self) -> impl Iterator<Item = TriangleAddress> + '_ {
        self.block_of.keys().copied()
    }

    /// Settled faces at level `n`.
    pub fn faces_at_level(&self, n: usize) -> usize {
        let ratio = n / self.base_subdivision;
        self.faces * ratio * ratio
    }

    /// Lattice vertices at level `n` strictly inside the settled region --
    /// those no built face has. With r = n / base, the closure has
    /// `F r^2` faces and `B r` boundary edges, so `V = chi + (F r^2 + B r) / 2`,
    /// of which `B r - P` lie on the boundary.
    pub fn interior_vertices_at_level(&self, n: usize) -> usize {
        if self.faces == 0 {
            return 0;
        }
        let ratio = n / self.base_subdivision;
        let twice = self.faces * ratio * ratio + self.boundary_edges * ratio;
        let all = self.euler + (twice / 2) as isize;
        (all - (self.boundary_edges * ratio) as isize + self.pinch_excess as isize) as usize
    }

    /// The settled block across the edge `(a, b)` of a built face whose base
    /// face is `ancestor`: the edge lies on one side of the base face -- both
    /// ends on that side's great circle -- and the block holds the base face
    /// across it.
    fn block_across(&self, ancestor: TriangleAddress, a: [f64; 3], b: [f64; 3]) -> Option<usize> {
        let on = |normal: [f64; 3], point: [f64; 3]| {
            (normal[0] * point[0] + normal[1] * point[1] + normal[2] * point[2]).abs() <= 1.0e-12
        };
        self.built_sides
            .get(&ancestor)?
            .iter()
            .find(|(normal, _)| on(*normal, a) && on(*normal, b))
            .and_then(|(_, across)| self.block_of.get(across).copied())
    }

    pub fn blocks(&self) -> usize {
        self.block_faces.len()
    }

    /// Level-`n` faces in `block`.
    fn faces_at(&self, block: usize, n: usize) -> usize {
        let ratio = n / self.base_subdivision;
        self.block_faces[block] * ratio * ratio
    }

    /// The smallest level-`n` face address in `block`: the smallest
    /// descendant of its smallest base face (descendants keep the order of
    /// their ancestors).
    fn first_face_at(&self, block: usize, n: usize) -> TriangleAddress {
        smallest_descendant(self.block_first_face[block], n)
    }
}

/// The level-`base_n` ancestor of `face`.
fn base_ancestor(mut face: TriangleAddress, base_n: usize) -> Option<TriangleAddress> {
    while face.n > base_n {
        face = face.parent_2_to_1()?;
    }
    (face.n == base_n).then_some(face)
}

/// The smallest level-`n` descendant of `face`: an up face's lattice patch
/// starts at its own corner, a down face's at the bottom of its first column.
fn smallest_descendant(face: TriangleAddress, n: usize) -> TriangleAddress {
    let ratio = n / face.n;
    match face.orientation {
        TriangleOrientation::Up => TriangleAddress {
            i: face.i * ratio,
            j: face.j * ratio,
            n,
            ..face
        },
        TriangleOrientation::Down => TriangleAddress {
            i: face.i * ratio,
            j: face.j * ratio + ratio - 1,
            n,
            ..face
        },
    }
}

/// The settled part of a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplicitCore {
    pub blocks: Vec<usize>,
    /// Settled parents: all core, none listed.
    pub parents: usize,
    /// The smallest settled parent address.
    pub first_parent: TriangleAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionComponent {
    pub component: HierarchyComponent,
    pub implicit: Option<ImplicitCore>,
}

impl RegionComponent {
    /// Core parents, listed and settled.
    pub fn core_parent_count(&self) -> usize {
        self.component.core_parents.len() + self.implicit.as_ref().map_or(0, |core| core.parents)
    }

    /// Parents, listed and settled.
    pub fn parent_count(&self) -> usize {
        self.component.parents.len() + self.implicit.as_ref().map_or(0, |core| core.parents)
    }

    /// The smallest parent address, listed or settled.
    pub fn first_parent(&self) -> Option<TriangleAddress> {
        let listed = self.component.parents.first().copied();
        let settled = self.implicit.as_ref().map(|core| core.first_parent);
        match (listed, settled) {
            (Some(listed), Some(settled)) => Some(listed.min(settled)),
            (listed, settled) => listed.or(settled),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionComponentPlan {
    /// Requirements of the listed parents, in address order.
    pub parent_requirements: Vec<ParentRequirement>,
    pub components: Vec<RegionComponent>,
}

struct RegionParent {
    address: TriangleAddress,
    child_count: u8,
    neighbours: Vec<usize>,
    settled_blocks: BTreeSet<usize>,
}

/// The parents of a built level grid, their listed neighbours (in the order
/// the whole-sphere planner finds them), and the settled blocks across the
/// region's edge.
fn region_parent_graph(
    level_grid: &MotherGrid,
    settled: &SettledRegion,
) -> Result<Vec<RegionParent>, String> {
    let fine_n = level_grid.subdivision;
    if !fine_n.is_multiple_of(2) {
        return Err("hierarchy component planning requires an even subdivision".into());
    }
    let coarse_n = fine_n / 2;
    let mut parent_of_face = vec![usize::MAX; level_grid.mesh.triangles().len()];
    let mut index_of = BTreeMap::new();
    for face in level_grid.mesh.active_triangle_slots() {
        let child = level_grid.triangle_addresses[face]
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
        index_of.insert(parent, usize::MAX);
        parent_of_face[face] = 0;
    }
    let mut parents = index_of
        .keys()
        .map(|&address| RegionParent {
            address,
            child_count: 0,
            neighbours: Vec::new(),
            settled_blocks: BTreeSet::new(),
        })
        .collect::<Vec<_>>();
    for (index, value) in index_of.values_mut().enumerate() {
        *value = index;
    }
    for face in level_grid.mesh.active_triangle_slots() {
        let parent = level_grid.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .expect("checked above");
        let index = index_of[&parent];
        parents[index].child_count += 1;
        parent_of_face[face] = index;
    }
    if let Some(parent) = parents.iter().find(|parent| parent.child_count != 4) {
        return Err(format!(
            "parent face {:?} has {} built children, expected 4",
            parent.address, parent.child_count
        ));
    }
    for face in level_grid.mesh.active_triangle_slots() {
        let left = parent_of_face[face];
        for (side, &neighbour) in level_grid.mesh.neighbours()[face].iter().enumerate() {
            if neighbour == 0 || !level_grid.mesh.is_triangle_live(neighbour) {
                // Across the region's edge: the settled base face there.
                let corners = level_grid.mesh.triangles()[face];
                let (a, b) = (corners[(side + 1) % 3], corners[(side + 2) % 3]);
                let block = settled_block_across(level_grid, settled, face, a, b)?;
                parents[left].settled_blocks.insert(block);
                continue;
            }
            let right = parent_of_face[neighbour];
            if right != left && !parents[left].neighbours.contains(&right) {
                if parents[left].neighbours.len() == 3 {
                    return Err(format!(
                        "parent face {:?} has more than 3 neighbours",
                        parents[left].address
                    ));
                }
                parents[left].neighbours.push(right);
            }
        }
    }
    for parent in &parents {
        let degree = parent.neighbours.len() + parent.settled_blocks.len();
        if parent.settled_blocks.is_empty() && degree != 3 {
            return Err(format!(
                "parent face {:?} has {} parent neighbours, expected 3",
                parent.address, degree
            ));
        }
    }
    Ok(parents)
}

/// The settled block on the other side of the edge `(a, b)` of a built face
/// whose neighbour there was not built.
fn settled_block_across(
    level_grid: &MotherGrid,
    settled: &SettledRegion,
    face: usize,
    a: usize,
    b: usize,
) -> Result<usize, String> {
    let address = level_grid.triangle_addresses[face].expect("built faces have addresses");
    let ancestor = base_ancestor(address, settled.base_subdivision)
        .ok_or_else(|| format!("face {address:?} has no base face"))?;
    let point = |site: usize| {
        let point = level_grid.mesh.vertices()[site];
        [point.x, point.y, point.z]
    };
    settled
        .block_across(ancestor, point(a), point(b))
        .ok_or_else(|| format!("face {address:?} has an open edge that faces no settled face"))
}

/// `plan_hierarchy_components_from_parent_requirements` over a built region:
/// the same components, parents, transition rings and boundary edges for the
/// built parents, with the settled blocks each component reaches folded into
/// its implicit core. With nothing settled it is that planner.
pub fn plan_region_components(
    level_grid: &MotherGrid,
    requirements: &[ExplicitParentRequirement],
    settled: &SettledRegion,
    coarse_level: usize,
    transition_ring_width: usize,
) -> Result<RegionComponentPlan, String> {
    let parents = region_parent_graph(level_grid, settled)?;
    if requirements.len() != parents.len() {
        return Err(format!(
            "{} parent requirements for {} built parents",
            requirements.len(),
            parents.len()
        ));
    }
    let mut eligible = Vec::with_capacity(parents.len());
    let mut parent_requirements = Vec::with_capacity(parents.len());
    for (parent, requirement) in parents.iter().zip(requirements) {
        if requirement.parent != parent.address {
            return Err(format!(
                "parent requirement {:?} is out of order (expected {:?})",
                requirement.parent, parent.address
            ));
        }
        let can_coarsen =
            requirement.available && requirement.maximum_required_level <= coarse_level;
        eligible.push(can_coarsen);
        parent_requirements.push(ParentRequirement {
            parent: parent.address,
            maximum_required_level: requirement.maximum_required_level,
            can_coarsen,
        });
    }
    if let Some(parent) = parents
        .iter()
        .zip(&eligible)
        .find(|(parent, &eligible)| !eligible && !parent.settled_blocks.is_empty())
    {
        return Err(format!(
            "parent {:?} touches the settled region but cannot coarsen",
            parent.0.address
        ));
    }

    // Components: built parents joined through their neighbours and through
    // the settled blocks they touch.
    let parent_count = parents.len();
    let mut parents_by_block = vec![Vec::new(); settled.blocks()];
    for (index, parent) in parents.iter().enumerate() {
        for &block in &parent.settled_blocks {
            parents_by_block[block].push(index);
        }
    }
    let mut component_by_parent = vec![usize::MAX; parent_count];
    let mut component_by_block = vec![usize::MAX; settled.blocks()];
    let mut component_count = 0usize;
    let mut queue = VecDeque::new();
    for seed in 0..parent_count {
        if !eligible[seed] || component_by_parent[seed] != usize::MAX {
            continue;
        }
        component_by_parent[seed] = component_count;
        queue.push_back(seed);
        while let Some(current) = queue.pop_front() {
            let through_blocks = parents[current]
                .settled_blocks
                .iter()
                .copied()
                .filter(|&block| component_by_block[block] == usize::MAX)
                .collect::<Vec<_>>();
            for block in through_blocks {
                component_by_block[block] = component_count;
                for &next in &parents_by_block[block] {
                    if component_by_parent[next] == usize::MAX {
                        component_by_parent[next] = component_count;
                        queue.push_back(next);
                    }
                }
            }
            for &neighbour in &parents[current].neighbours {
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
    // Transition rings: distance to a parent that cannot coarsen, counted
    // through built parents only -- settled parents are far beyond any ring.
    let mut distances = vec![usize::MAX; parent_count];
    queue.clear();
    for index in 0..parent_count {
        if !eligible[index] {
            continue;
        }
        let component = component_by_parent[index];
        components[component].parents.push(parents[index].address);
        for &neighbour in &parents[index].neighbours {
            if !eligible[neighbour] {
                components[component].boundary_edges.push(canonical_edge(
                    parents[index].address,
                    parents[neighbour].address,
                ));
                if distances[index] == usize::MAX {
                    distances[index] = 0;
                    queue.push_back(index);
                }
            }
        }
    }
    while let Some(current) = queue.pop_front() {
        let next_distance = distances[current].saturating_add(1);
        for &neighbour in &parents[current].neighbours {
            if eligible[neighbour] && distances[neighbour] == usize::MAX {
                distances[neighbour] = next_distance;
                queue.push_back(neighbour);
            }
        }
    }
    for index in 0..parent_count {
        if !eligible[index] {
            continue;
        }
        let component = component_by_parent[index];
        if distances[index] <= transition_ring_width {
            if !parents[index].settled_blocks.is_empty() {
                return Err(format!(
                    "transition parent {:?} touches the settled region",
                    parents[index].address
                ));
            }
            components[component]
                .transition_parents
                .push(parents[index].address);
        } else {
            components[component]
                .core_parents
                .push(parents[index].address);
        }
    }

    let coarse_n = level_grid.subdivision / 2;
    let mut implicit = vec![None::<ImplicitCore>; component_count];
    for (block, &component) in component_by_block.iter().enumerate() {
        if component == usize::MAX {
            continue;
        }
        let first = settled.first_face_at(block, coarse_n);
        let core = implicit[component].get_or_insert(ImplicitCore {
            blocks: Vec::new(),
            parents: 0,
            first_parent: first,
        });
        core.blocks.push(block);
        core.parents += settled.faces_at(block, coarse_n);
        core.first_parent = core.first_parent.min(first);
    }
    Ok(RegionComponentPlan {
        parent_requirements,
        components: components
            .into_iter()
            .zip(implicit)
            .map(|(component, implicit)| RegionComponent {
                component,
                implicit,
            })
            .collect(),
    })
}

/// `sort_components` for region components: the same keys, with settled
/// parents counted in the core and in the parent lists. Settled parents
/// require level zero, and two components' parent lists, being disjoint,
/// differ at their first entries.
pub fn sort_region_components(
    components: &mut [RegionComponent],
    requirements: &[ParentRequirement],
    coarse_level: usize,
) {
    let margin = |component: &RegionComponent| {
        let maximum = component
            .component
            .parents
            .iter()
            .filter_map(|parent| {
                requirements
                    .binary_search_by(|requirement| requirement.parent.cmp(parent))
                    .ok()
            })
            .map(|index| requirements[index].maximum_required_level)
            .chain(component.implicit.as_ref().map(|_| 0))
            .max()
            .unwrap_or(coarse_level);
        coarse_level.saturating_sub(maximum)
    };
    components.sort_by(|left, right| {
        right
            .core_parent_count()
            .cmp(&left.core_parent_count())
            .then_with(|| {
                left.component
                    .boundary_edges
                    .len()
                    .cmp(&right.component.boundary_edges.len())
            })
            .then_with(|| {
                right
                    .component
                    .transition_parents
                    .is_empty()
                    .cmp(&left.component.transition_parents.is_empty())
            })
            .then_with(|| {
                left.component
                    .transition_parents
                    .len()
                    .cmp(&right.component.transition_parents.len())
            })
            .then_with(|| margin(right).cmp(&margin(left)))
            .then_with(|| left.first_parent().cmp(&right.first_parent()))
    });
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
    use crate::coarsen::plan_hierarchy_components_from_parent_requirements;
    use crate::mother_grid::region::descendant_faces;

    /// Requirements of every parent of `grid`: the maximum of `levels` over
    /// the parent's four children's corners, all available.
    fn requirements(grid: &MotherGrid, levels: &[usize]) -> Vec<ExplicitParentRequirement> {
        let mut by_parent = BTreeMap::new();
        for face in grid.mesh.active_triangle_slots() {
            let parent = grid.triangle_addresses[face]
                .unwrap()
                .parent_2_to_1()
                .unwrap();
            let level = grid.mesh.triangles()[face]
                .iter()
                .map(|&site| levels[site])
                .max()
                .unwrap();
            let entry = by_parent.entry(parent).or_insert(0);
            *entry = (*entry).max(level);
        }
        by_parent
            .into_iter()
            .map(
                |(parent, maximum_required_level)| ExplicitParentRequirement {
                    parent,
                    maximum_required_level,
                    available: true,
                },
            )
            .collect()
    }

    /// The interior vertex count matches a count over the whole grid.
    #[test]
    fn settled_interior_vertices_are_counted_exactly() {
        let base_n = 3;
        let base = MotherGrid::generate(base_n).unwrap();
        let faces = base
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        for (step, take) in [(2, 60), (3, 40), (1, 25), (5, 30)] {
            let built = faces
                .iter()
                .copied()
                .step_by(step)
                .take(take)
                .collect::<BTreeSet<_>>();
            let settled = SettledRegion::new(&base, &built).unwrap();
            for n in [3, 6, 12] {
                let grid = MotherGrid::generate(n).unwrap();
                let mut touched_by_built = vec![false; grid.mesh.vertices().len()];
                let mut settled_faces = 0;
                for face in grid.mesh.active_triangle_slots() {
                    let ancestor =
                        base_ancestor(grid.triangle_addresses[face].unwrap(), base_n).unwrap();
                    if built.contains(&ancestor) {
                        for site in grid.mesh.triangles()[face] {
                            touched_by_built[site] = true;
                        }
                    } else {
                        settled_faces += 1;
                    }
                }
                let interior = grid
                    .mesh
                    .active_vertex_slots()
                    .filter(|&site| !touched_by_built[site])
                    .count();
                assert_eq!(
                    settled.faces_at_level(n),
                    settled_faces,
                    "step {step}, n {n}"
                );
                assert_eq!(
                    settled.interior_vertices_at_level(n),
                    interior,
                    "step {step}, n {n}"
                );
            }
        }
    }

    #[test]
    fn smallest_descendants_match_a_search() {
        for base in MotherGrid::generate(3)
            .unwrap()
            .triangle_addresses
            .into_iter()
            .flatten()
        {
            for n in [3, 6, 12, 24] {
                let searched = *descendant_faces([base], n).unwrap().first().unwrap();
                assert_eq!(smallest_descendant(base, n), searched, "{base:?} at {n}");
            }
        }
    }

    /// With nothing settled the region planner is the whole-sphere planner.
    #[test]
    fn with_nothing_settled_the_plan_is_the_whole_plan() {
        let n = 12;
        let grid = MotherGrid::generate(n).unwrap();
        let mut levels = vec![0; grid.mesh.vertices().len()];
        for (site, level) in levels.iter_mut().enumerate().skip(2).step_by(37) {
            *level = 1 + site % 2;
        }
        let requirements = requirements(&grid, &levels);
        let base = MotherGrid::generate(n / 4).unwrap();
        let everything = base.triangle_addresses.iter().flatten().copied().collect();
        let settled = SettledRegion::new(&base, &everything).unwrap();
        assert_eq!(settled.blocks(), 0);
        for (coarse_level, rings) in [(0, 1), (1, 1), (1, 2)] {
            let whole = plan_hierarchy_components_from_parent_requirements(
                &grid,
                &requirements,
                coarse_level,
                rings,
            )
            .unwrap();
            let region =
                plan_region_components(&grid, &requirements, &settled, coarse_level, rings)
                    .unwrap();
            assert_eq!(region.parent_requirements, whole.parent_requirements);
            assert_eq!(
                region
                    .components
                    .iter()
                    .map(|c| c.component.clone())
                    .collect::<Vec<_>>(),
                whole.components
            );
            assert!(region.components.iter().all(|c| c.implicit.is_none()));
        }
    }

    /// With settled faces, every component of the whole plan appears with its
    /// built parents listed (same transition and core split, same boundary
    /// edges) and its settled parents counted.
    #[test]
    fn settled_parents_are_counted_not_listed() {
        let (base_n, fine_n) = (4, 16);
        let whole_grid = MotherGrid::generate(fine_n).unwrap();
        let base = MotherGrid::generate(base_n).unwrap();
        // Demand around two sites far apart.
        let mut levels = vec![0; whole_grid.mesh.vertices().len()];
        levels[40] = 2;
        levels[1500] = 1;
        let whole_requirements = requirements(&whole_grid, &levels);
        // Built: the base faces within two vertex rings of the demand.
        let demand_faces = whole_grid
            .mesh
            .active_triangle_slots()
            .filter(|&face| {
                whole_grid.mesh.triangles()[face]
                    .iter()
                    .any(|&site| levels[site] > 0)
            })
            .map(|face| {
                base_ancestor(whole_grid.triangle_addresses[face].unwrap(), base_n).unwrap()
            })
            .collect::<BTreeSet<_>>();
        let mut built = demand_faces;
        for _ in 0..2 {
            let ring = base
                .mesh
                .active_triangle_slots()
                .filter(|&face| {
                    let corners = base.mesh.triangles()[face];
                    base.mesh.active_triangle_slots().any(|other| {
                        built.contains(&base.triangle_addresses[other].unwrap())
                            && base.mesh.triangles()[other]
                                .iter()
                                .any(|v| corners.contains(v))
                    })
                })
                .map(|face| base.triangle_addresses[face].unwrap())
                .collect::<Vec<_>>();
            built.extend(ring);
        }
        let settled = SettledRegion::new(&base, &built).unwrap();
        assert!(settled.blocks() > 0);
        let region_grid = MotherGrid::generate_faces(
            fine_n,
            descendant_faces(built.iter().copied(), fine_n).unwrap(),
        )
        .unwrap();
        let region_requirements = whole_requirements
            .iter()
            .copied()
            .filter(|requirement| {
                built.contains(&base_ancestor(requirement.parent, base_n).unwrap())
            })
            .collect::<Vec<_>>();
        for (coarse_level, rings) in [(0, 1), (1, 1)] {
            let whole = plan_hierarchy_components_from_parent_requirements(
                &whole_grid,
                &whole_requirements,
                coarse_level,
                rings,
            )
            .unwrap();
            let region = plan_region_components(
                &region_grid,
                &region_requirements,
                &settled,
                coarse_level,
                rings,
            )
            .unwrap();
            let listed = |parents: &[TriangleAddress]| {
                parents
                    .iter()
                    .copied()
                    .filter(|parent| built.contains(&base_ancestor(*parent, base_n).unwrap()))
                    .collect::<Vec<_>>()
            };
            let mut matched = 0;
            for whole_component in &whole.components {
                let Some(region_component) = region.components.iter().find(|candidate| {
                    candidate.first_parent() == whole_component.parents.first().copied()
                }) else {
                    assert!(listed(&whole_component.parents).is_empty());
                    continue;
                };
                matched += 1;
                assert_eq!(
                    region_component.component.parents,
                    listed(&whole_component.parents)
                );
                assert_eq!(
                    region_component.component.transition_parents,
                    whole_component.transition_parents
                );
                assert_eq!(
                    region_component.component.core_parents,
                    listed(&whole_component.core_parents)
                );
                assert_eq!(
                    region_component.component.boundary_edges,
                    whole_component.boundary_edges
                );
                assert_eq!(
                    region_component.parent_count(),
                    whole_component.parents.len()
                );
                assert_eq!(
                    region_component.core_parent_count(),
                    whole_component.core_parents.len()
                );
            }
            assert_eq!(matched, region.components.len());
            assert!(region.components.iter().any(|c| c.implicit.is_some()));

            // Sorted, the region components follow the whole ones.
            let mut whole_sorted = whole.components.clone();
            crate::coarsen::scheduler::sort_components_for_test(
                &mut whole_sorted,
                &whole.parent_requirements,
                coarse_level,
            );
            let mut region_sorted = region.components.clone();
            sort_region_components(
                &mut region_sorted,
                &region.parent_requirements,
                coarse_level,
            );
            let whole_order = whole_sorted
                .iter()
                .filter(|component| !listed(&component.parents).is_empty())
                .map(|component| component.parents.first().copied())
                .collect::<Vec<_>>();
            let region_order = region_sorted
                .iter()
                .map(RegionComponent::first_parent)
                .collect::<Vec<_>>();
            assert_eq!(region_order, whole_order);
        }
    }
}
