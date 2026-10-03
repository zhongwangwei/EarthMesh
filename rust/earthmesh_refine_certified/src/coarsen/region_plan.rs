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

/// The settled base faces, in edge-connected blocks numbered by their
/// smallest face.
#[derive(Debug, Clone, PartialEq)]
pub struct SettledRegion {
    base_subdivision: usize,
    /// The block of each settled face across a side of a built face -- what
    /// `block_across` asks for.
    rim: BTreeMap<TriangleAddress, usize>,
    /// Every settled face, when they were listed (`new`); `by_address` does
    /// not list them.
    listed: Option<BTreeSet<TriangleAddress>>,
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

/// A built face's sides: the unit normal of the great circle through the
/// two corners opposite each corner, and the face across.
fn sides_of(
    corners: [[f64; 3]; 3],
    across: [TriangleAddress; 3],
) -> [([f64; 3], TriangleAddress); 3] {
    std::array::from_fn(|corner| {
        let (a, b) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
        let normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        (
            [normal[0] / length, normal[1] / length, normal[2] / length],
            across[corner],
        )
    })
}

/// Faces a settled region may flood before giving up: every block but the
/// largest is flooded, and a regional run's are small.
const SETTLED_FLOOD_LIMIT: usize = 1 << 24;

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}

/// The level-n faces in address order.
fn faces_in_order(n: usize) -> impl Iterator<Item = TriangleAddress> {
    (0..20u8).flat_map(move |base_face| {
        (0..n).flat_map(move |i| {
            (0..n - i).flat_map(move |j| {
                [TriangleOrientation::Up, TriangleOrientation::Down]
                    .into_iter()
                    .filter(move |&orientation| {
                        orientation == TriangleOrientation::Up || i + j + 1 < n
                    })
                    .map(move |orientation| TriangleAddress {
                        base_face,
                        i,
                        j,
                        n,
                        orientation,
                    })
            })
        })
    })
}

impl SettledRegion {
    /// The base faces of `base` outside `built`, grouped by shared edges --
    /// every one of them listed. The oracle for `by_address`, and what the
    /// sphere's assembly needs.
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
        let mut rim = BTreeMap::new();
        for face in base.mesh.active_triangle_slots() {
            let address = base.triangle_addresses[face].expect("checked above");
            if !built.contains(&address) {
                continue;
            }
            let corners = base.mesh.triangles()[face].map(|site| {
                let point = base.mesh.vertices()[site];
                [point.x, point.y, point.z]
            });
            // `neighbours()[face][k]` lies across the edge opposite corner k.
            let mut across = [address; 3];
            for (corner, slot) in across.iter_mut().enumerate() {
                let neighbour = base.mesh.neighbours()[face][corner];
                *slot = base.triangle_addresses[neighbour]
                    .ok_or_else(|| format!("base face {neighbour} has no address"))?;
                if let Some(&block) = block_of.get(slot) {
                    rim.insert(*slot, block);
                }
            }
            built_sides.insert(address, sides_of(corners, across));
        }
        // Number the blocks by their smallest face.
        let mut order = (0..block_faces.len()).collect::<Vec<_>>();
        order.sort_by_key(|&block| block_first_face[block]);
        let mut renumbered = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            renumbered[old] = new;
        }
        for block in rim.values_mut() {
            *block = renumbered[*block];
        }
        Ok(Self {
            base_subdivision: base.subdivision,
            rim,
            listed: Some(block_of.into_keys().collect()),
            block_faces: order.iter().map(|&block| block_faces[block]).collect(),
            block_first_face: order.iter().map(|&block| block_first_face[block]).collect(),
            built_sides,
            faces: settled_faces,
            boundary_edges,
            euler,
            pinch_excess,
        })
    }

    /// `new` from the built faces alone, for bases too fine to build whole
    /// (design B1r-3); the settled faces are not listed.
    ///
    /// The closure's counts are the sphere's less the built faces' interior,
    /// `chi = 2 - V_int + E_int - F_built`, and its boundary is the built
    /// faces' own. Split at its pinched vertices, every block is a sphere
    /// with holes -- `chi = 2 - loops` -- so there are
    /// `(chi + pinch excess + loops) / 2` blocks, the loops traced around each
    /// boundary vertex through its settled wedges. Floods from the faces
    /// across the built sides then run together until all blocks but one
    /// have closed; the one left -- for a regional run, the rest of the
    /// sphere -- is counted as what remains.
    pub fn by_address(base_n: usize, built: &BTreeSet<TriangleAddress>) -> Result<Self, String> {
        use crate::mother_grid::lattice::{face_corner_origins, faces_across, faces_at_vertex};
        use crate::mother_grid::region::{origin_position, VertexOrigin};
        use std::collections::HashMap;

        if base_n == 0 {
            return Err("base subdivision must be positive".into());
        }
        for face in built {
            face.dense_index(base_n)?;
        }
        let whole = 20usize
            .checked_mul(base_n)
            .and_then(|faces| faces.checked_mul(base_n))
            .ok_or_else(|| "base face count overflows".to_string())?;

        // The built faces' corners and the faces across their sides.
        let mut corners_of = BTreeMap::new();
        let mut across_of = BTreeMap::new();
        for &face in built {
            corners_of.insert(face, face_corner_origins(face)?);
            across_of.insert(face, faces_across(face)?);
        }
        let mut boundary_edges = 0usize;
        let mut shared_sides = 0usize;
        let mut boundary_degree = BTreeMap::<VertexOrigin, usize>::new();
        let mut rim_faces = BTreeSet::new();
        for (face, corners) in &corners_of {
            for (corner, across) in across_of[face].iter().enumerate() {
                if built.contains(across) {
                    shared_sides += 1;
                    continue;
                }
                boundary_edges += 1;
                rim_faces.insert(*across);
                for end in [corners[(corner + 1) % 3], corners[(corner + 2) % 3]] {
                    *boundary_degree.entry(end).or_default() += 1;
                }
            }
        }
        let built_corners = corners_of
            .values()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        let interior_vertices = built_corners
            .iter()
            .filter(|&&origin| {
                faces_at_vertex(base_n, origin)
                    .iter()
                    .all(|face| built.contains(face))
            })
            .count();
        let faces = whole - built.len();
        let euler =
            2 - interior_vertices as isize + (shared_sides / 2) as isize - built.len() as isize;
        let pinch_excess = boundary_degree
            .values()
            .map(|&degree| degree / 2 - 1)
            .sum::<usize>();

        // Boundary loops: around each boundary vertex, the two boundary edges
        // that bound a settled wedge continue each other.
        let mut edge_index = BTreeMap::<(VertexOrigin, VertexOrigin), usize>::new();
        let mut loop_parent = Vec::<usize>::new();
        let mut edge_of = |a: VertexOrigin, b: VertexOrigin, parent: &mut Vec<usize>| {
            *edge_index.entry((a.min(b), a.max(b))).or_insert_with(|| {
                parent.push(parent.len());
                parent.len() - 1
            })
        };
        for &vertex in boundary_degree.keys() {
            // The fan around the vertex in cyclic order: `cycle[k]` is a face
            // and the spoke (edge from the vertex) it shares with the next.
            let fan = faces_at_vertex(base_n, vertex);
            let spokes = fan
                .iter()
                .map(|&face| {
                    let others = face_corner_origins(face)?
                        .into_iter()
                        .filter(|&corner| corner != vertex)
                        .collect::<Vec<_>>();
                    Ok::<_, String>([others[0], others[1]])
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut cycle = vec![(0usize, spokes[0][1])];
            while cycle.len() < fan.len() {
                let (current, out) = *cycle.last().expect("non-empty");
                let next = (0..fan.len())
                    .find(|&other| other != current && spokes[other].contains(&out))
                    .ok_or_else(|| format!("the fan around {vertex:?} does not close"))?;
                let onward = if spokes[next][0] == out {
                    spokes[next][1]
                } else {
                    spokes[next][0]
                };
                cycle.push((next, onward));
            }
            let settled_at =
                |position: usize| !built.contains(&fan[cycle[position % cycle.len()].0]);
            // Spoke k, between cycle[k] and cycle[k + 1], is a boundary edge
            // when one side is built and the other settled.
            let boundary_spokes = (0..cycle.len())
                .filter(|&position| settled_at(position) != settled_at(position + 1))
                .collect::<Vec<_>>();
            // A settled wedge opens at a boundary spoke whose next face is
            // settled and closes at the following boundary spoke.
            for (index, &position) in boundary_spokes.iter().enumerate() {
                if !settled_at(position + 1) {
                    continue;
                }
                let closing = boundary_spokes[(index + 1) % boundary_spokes.len()];
                let left = edge_of(vertex, cycle[position].1, &mut loop_parent);
                let right = edge_of(vertex, cycle[closing].1, &mut loop_parent);
                let (left, right) = (find(&mut loop_parent, left), find(&mut loop_parent, right));
                loop_parent[left] = right;
            }
        }
        if loop_parent.len() != boundary_edges {
            return Err(format!(
                "{} boundary edges traced for {boundary_edges} built sides facing settled faces",
                loop_parent.len()
            ));
        }
        let loops = (0..loop_parent.len())
            .filter(|&edge| find(&mut loop_parent, edge) == edge)
            .count();
        let twice_blocks = euler + pinch_excess as isize + loops as isize;
        if twice_blocks < 0 || twice_blocks % 2 != 0 || (twice_blocks == 0) != (faces == 0) {
            return Err(format!(
                "settled faces with chi {euler}, pinch excess {pinch_excess} and {loops} loops \
                 are not spheres with holes"
            ));
        }
        let block_count = (twice_blocks / 2) as usize;

        // Flood every block but one from the rim, all floods together.
        let mut label = HashMap::<TriangleAddress, usize>::new();
        let mut class_parent = Vec::new();
        let mut pending = Vec::new();
        let mut size = Vec::new();
        let mut first = Vec::new();
        let mut queue = VecDeque::new();
        for &face in &rim_faces {
            let class = class_parent.len();
            class_parent.push(class);
            pending.push(1usize);
            size.push(0usize);
            first.push(face);
            label.insert(face, class);
            queue.push_back(face);
        }
        let mut closed = BTreeSet::new();
        let mut flooded = 0usize;
        while closed.len() + 1 < block_count {
            let Some(face) = queue.pop_front() else {
                break;
            };
            flooded += 1;
            if flooded > SETTLED_FLOOD_LIMIT {
                return Err(format!(
                    "the settled faces form {block_count} blocks, and flooding all but the \
                     largest passes {SETTLED_FLOOD_LIMIT} faces"
                ));
            }
            let class = find(&mut class_parent, label[&face]);
            pending[class] -= 1;
            size[class] += 1;
            first[class] = first[class].min(face);
            for next in faces_across(face)? {
                if built.contains(&next) {
                    continue;
                }
                match label.get(&next) {
                    None => {
                        label.insert(next, class);
                        pending[class] += 1;
                        queue.push_back(next);
                    }
                    Some(&other) => {
                        let other = find(&mut class_parent, other);
                        if other != class {
                            class_parent[other] = class;
                            pending[class] += pending[other];
                            size[class] += size[other];
                            first[class] = first[class].min(first[other]);
                        }
                    }
                }
            }
            if pending[class] == 0 {
                closed.insert(class);
            }
        }
        let mut open_classes = BTreeSet::new();
        for &face in &rim_faces {
            let class = find(&mut class_parent, label[&face]);
            if !closed.contains(&class) {
                open_classes.insert(class);
            }
        }
        let open_blocks =
            usize::from(!open_classes.is_empty() || (faces > 0 && rim_faces.is_empty()));
        if closed.len() + open_blocks != block_count {
            return Err(format!(
                "{} settled blocks closed and {} open, for {block_count} blocks",
                closed.len(),
                open_blocks
            ));
        }
        // Blocks: the closed ones as flooded, the open one as what is left.
        let mut blocks = closed
            .iter()
            .map(|&class| (first[class], size[class], Some(class)))
            .collect::<Vec<_>>();
        if open_blocks == 1 {
            let closed_faces = blocks.iter().map(|block| block.1).sum::<usize>();
            let mut in_closed = BTreeSet::new();
            for (&face, &class) in &label {
                if closed.contains(&find(&mut class_parent, class)) {
                    in_closed.insert(face);
                }
            }
            let smallest = faces_in_order(base_n)
                .find(|face| !built.contains(face) && !in_closed.contains(face))
                .ok_or_else(|| "the open settled block has no face".to_string())?;
            blocks.push((smallest, faces - closed_faces, None));
        }
        blocks.sort_by_key(|block| block.0);
        let mut rim = BTreeMap::new();
        for &face in &rim_faces {
            let class = find(&mut class_parent, label[&face]);
            let owner = closed.contains(&class).then_some(class);
            let block = blocks
                .iter()
                .position(|block| block.2 == owner)
                .ok_or_else(|| format!("rim face {face:?} has no block"))?;
            rim.insert(face, block);
        }
        let mut built_sides = BTreeMap::new();
        for (&face, corners) in &corners_of {
            let mut points = [[0.0; 3]; 3];
            for (point, &origin) in points.iter_mut().zip(corners) {
                let position = origin_position(base_n, origin)?;
                *point = [position.x, position.y, position.z];
            }
            built_sides.insert(face, sides_of(points, across_of[&face]));
        }
        Ok(Self {
            base_subdivision: base_n,
            rim,
            listed: None,
            block_faces: blocks.iter().map(|block| block.1).collect(),
            block_first_face: blocks.iter().map(|block| block.0).collect(),
            built_sides,
            faces,
            boundary_edges,
            euler,
            pinch_excess,
        })
    }

    /// The settled base faces, when they were listed.
    pub fn listed_faces(&self) -> Result<&BTreeSet<TriangleAddress>, String> {
        self.listed.as_ref().ok_or_else(|| {
            "the settled faces were counted, not listed; only a whole base lists them".to_string()
        })
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
            .and_then(|(_, across)| self.rim.get(across).copied())
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

    /// From the built faces alone, the settled region is the whole grid's:
    /// blocks, rim, sides and closure counts, on extents, rings around holes,
    /// bands round the sphere, pinches, random scatters, nothing and
    /// everything.
    #[test]
    fn the_settled_region_by_address_is_the_whole_grids() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let around = |faces: &BTreeSet<TriangleAddress>| {
            let mut grown = faces.clone();
            for &face in faces {
                grown.extend(crate::mother_grid::lattice::faces_around(face).unwrap());
            }
            grown
        };
        let mut cases = 0;
        let mut multi_block = 0;
        let mut pinched = 0;
        for n in [1, 2, 3, 4, 6, 8, 12] {
            let base = MotherGrid::generate(n).unwrap();
            let all = base
                .triangle_addresses
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let mut built_sets = vec![BTreeSet::new(), all.iter().copied().collect()];
            // Disks, and the rings around them that enclose a hole.
            for _ in 0..3 {
                let centre = BTreeSet::from([all[(next() % all.len() as u64) as usize]]);
                let disk = around(&centre);
                let ring = around(&around(&disk))
                    .difference(&disk)
                    .copied()
                    .collect::<BTreeSet<_>>();
                built_sets.push(disk);
                built_sets.push(ring.clone());
                // A ring broken at one vertex: the hole touches the outside
                // there, a pinch between two blocks or within one.
                let mut broken = ring.clone();
                let cut = *ring
                    .iter()
                    .nth((next() % ring.len() as u64) as usize)
                    .unwrap();
                broken.remove(&cut);
                built_sets.push(broken);
            }
            // A band round the sphere: the faces of base faces 5..15.
            built_sets.push(
                all.iter()
                    .copied()
                    .filter(|face| (5..15).contains(&face.base_face))
                    .collect(),
            );
            // Random scatters.
            for fraction in [10, 30, 50, 70, 90] {
                built_sets.push(
                    all.iter()
                        .copied()
                        .filter(|_| next() % 100 < fraction)
                        .collect(),
                );
            }
            for built in built_sets {
                let whole = SettledRegion::new(&base, &built).unwrap();
                let by_address = SettledRegion::by_address(n, &built)
                    .unwrap_or_else(|error| panic!("n {n} built {}: {error}", built.len()));
                assert_eq!(
                    by_address,
                    SettledRegion {
                        listed: None,
                        ..whole.clone()
                    },
                    "n {n} built {built:?}"
                );
                cases += 1;
                multi_block += usize::from(whole.blocks() > 1);
                pinched += usize::from(whole.pinch_excess > 0);
            }
        }
        assert!(
            multi_block >= 15 && pinched >= 10,
            "{cases} cases: {multi_block} with several blocks, {pinched} pinched"
        );
    }

    /// A settled region by address leaves its faces unlisted, and assembly
    /// says so instead of guessing.
    #[test]
    fn a_settled_region_by_address_lists_no_faces() {
        let built = BTreeSet::from([TriangleAddress {
            base_face: 3,
            i: 0,
            j: 0,
            n: 2,
            orientation: TriangleOrientation::Up,
        }]);
        let settled = SettledRegion::by_address(2, &built).unwrap();
        assert!(settled.listed_faces().is_err());
        assert_eq!(settled.blocks(), 1);
        assert_eq!(settled.faces_at_level(4), (80 - 1) * 4);
        let whole = SettledRegion::new(&MotherGrid::generate(2).unwrap(), &built).unwrap();
        assert_eq!(whole.listed_faces().unwrap().len(), 79);
    }
}
