//! A part of the mother grid built on its own (on-demand reverse coarsening,
//! design B1a in `docs/certified_mesh/on_demand_reverse_coarsening.md`).
//!
//! `MotherGrid::generate(n)` builds all 20 n^2 faces of the level-n lattice.
//! Reverse coarsening needs the finest lattice only where a requirement can be
//! non-zero, so a region builds just the faces it is given -- and they must be
//! the faces `generate` builds: the same corner coordinates bit for bit, the
//! same corner order and orientation, and slots in the same relative order, so
//! that whatever is assembled from a region is numbered as it would have been
//! from the whole grid.

use super::{
    icosahedron_faces, icosahedron_vertices, push_oriented, weighted, MotherGrid, TriangleAddress,
    TriangleOrientation, VertexAddress,
};
use earthmesh_mesh::{normalize_cartesian_to_radius, CartesianPoint, MeshState};
use std::collections::{BTreeMap, BTreeSet};

/// Where `generate` first reaches a lattice vertex: the base face that inserts
/// it and the vertex's `(i, j)` there. A vertex on an icosahedron edge or
/// corner belongs to several base faces; `generate` computes it from the first
/// of them, whose corner order fixes how the weighted sum rounds, and gives it
/// the next free slot -- so origins order slots exactly as `generate` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexOrigin {
    pub face: u8,
    pub i: usize,
    pub j: usize,
}

/// The lookup a region adds to its `MotherGrid`: face slots by address, and
/// the origin of every vertex slot. A whole grid needs neither -- its face
/// slots follow `TriangleAddress::dense_index` and its slots are in origin
/// order by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionIndex {
    face_slots: BTreeMap<TriangleAddress, usize>,
    origins: Vec<Option<VertexOrigin>>,
    outer_boundary: BTreeSet<usize>,
}

impl RegionIndex {
    pub fn face_slot(&self, address: TriangleAddress) -> Option<usize> {
        self.face_slots.get(&address).copied()
    }

    pub fn origin(&self, slot: usize) -> Option<VertexOrigin> {
        self.origins.get(slot).copied().flatten()
    }

    /// Vertex slots on the region's edge: ends of edges that only one built
    /// face has. Their fans are open, so they get no Voronoi cell here.
    pub fn outer_boundary(&self) -> &BTreeSet<usize> {
        &self.outer_boundary
    }
}

/// The origin of the lattice point `(i, j)` of base face `face` at level `n`.
pub fn vertex_origin(n: usize, face: u8, i: usize, j: usize) -> Result<VertexOrigin, String> {
    let faces = icosahedron_faces();
    if face >= 20 || i.checked_add(j).is_none_or(|sum| sum > n) {
        return Err(format!(
            "lattice point ({face}, {i}, {j}) is outside level {n}"
        ));
    }
    let corners = faces[face as usize];
    let weights = [n - i - j, i, j];
    let ends: Vec<(u8, usize)> = corners
        .into_iter()
        .zip(weights)
        .filter(|&(_, weight)| weight > 0)
        .collect();
    if ends.len() == 3 {
        return Ok(VertexOrigin { face, i, j });
    }
    // On an icosahedron edge or corner: the first base face holding every
    // corner with a weight, and the weights in that face's own order.
    let first = (0..20u8)
        .find(|&candidate| {
            ends.iter()
                .all(|(corner, _)| faces[candidate as usize].contains(corner))
        })
        .expect("every icosahedron vertex and edge lies on a base face");
    let mut first_weights = [0usize; 3];
    for (corner, weight) in ends {
        let position = faces[first as usize]
            .iter()
            .position(|&candidate| candidate == corner)
            .expect("the first face holds the corner");
        first_weights[position] = weight;
    }
    Ok(VertexOrigin {
        face: first,
        i: first_weights[1],
        j: first_weights[2],
    })
}

/// The address `generate` records for the vertex at `origin`.
pub fn origin_address(n: usize, origin: VertexOrigin) -> VertexAddress {
    let corners = icosahedron_faces()[origin.face as usize];
    let weights = [n - origin.i - origin.j, origin.i, origin.j];
    if let Some(position) = weights.iter().position(|&weight| weight == n) {
        return VertexAddress::IcosahedronVertex(corners[position]);
    }
    if let Some(zero) = weights.iter().position(|&weight| weight == 0) {
        let mut ends = [
            (corners[(zero + 1) % 3], weights[(zero + 1) % 3]),
            (corners[(zero + 2) % 3], weights[(zero + 2) % 3]),
        ];
        ends.sort_by_key(|end| end.0);
        return VertexAddress::IcosahedronEdge {
            a: ends[0].0,
            b: ends[1].0,
            step: ends[1].1,
            n,
        };
    }
    VertexAddress::IcosahedronFace {
        face: origin.face,
        i: origin.i,
        j: origin.j,
        k: weights[0],
        n,
    }
}

/// The coordinates `generate` computes for the vertex at `origin`: the
/// normalized weighted corners of the origin face, in that face's order.
fn origin_position_on(
    n: usize,
    origin: VertexOrigin,
    base: &[CartesianPoint; 12],
) -> Result<CartesianPoint, String> {
    let corners = icosahedron_faces()[origin.face as usize];
    normalize_cartesian_to_radius(
        weighted(
            base[corners[0] as usize],
            n - origin.i - origin.j,
            base[corners[1] as usize],
            origin.i,
            base[corners[2] as usize],
            origin.j,
        ),
        1.0,
    )
    .map_err(|error| format!("lattice point {origin:?} at level {n}: {error:?}"))
}

/// Where `generate(n)` numbers vertices: for each base face, how many new
/// vertices the faces before it brought, and which of its corners and edges
/// it is the first to reach.
#[derive(Debug, Clone)]
pub struct GlobalNumbering {
    n: usize,
    before_face: [usize; 20],
    /// Per base face: corners a, b, c (weights k, i, j at n) and edges ab
    /// (j = 0), ac (i = 0), bc (k = 0) that are new there.
    new: [[bool; 6]; 20],
}

impl GlobalNumbering {
    pub fn new(n: usize) -> Self {
        let faces = icosahedron_faces();
        let first_with = |corners: &[u8]| {
            (0..20)
                .find(|&face| corners.iter().all(|corner| faces[face].contains(corner)))
                .expect("every icosahedron vertex and edge lies on a base face")
        };
        let mut new = [[false; 6]; 20];
        let mut before_face = [0usize; 20];
        let mut total = 0usize;
        let interior = if n >= 2 { (n - 1) * (n - 2) / 2 } else { 0 };
        for face in 0..20 {
            let [a, b, c] = faces[face];
            new[face] = [
                first_with(&[a]) == face,
                first_with(&[b]) == face,
                first_with(&[c]) == face,
                first_with(&[a, b]) == face,
                first_with(&[a, c]) == face,
                first_with(&[b, c]) == face,
            ];
            before_face[face] = total;
            let flags = new[face];
            total += interior
                + flags[..3].iter().filter(|&&new| new).count()
                + flags[3..].iter().filter(|&&new| new).count() * n.saturating_sub(1);
        }
        Self {
            n,
            before_face,
            new,
        }
    }

    /// The vertex's place among all of `generate(n)`'s vertices (its slot
    /// less the two reserved ones).
    pub fn rank(&self, origin: VertexOrigin) -> usize {
        let n = self.n;
        let [new_a, _, new_c, new_ab, new_ac, new_bc] = self.new[origin.face as usize];
        let (i, j) = (origin.i, origin.j);
        let flag = |value: bool| usize::from(value);
        // Rows before row i: row 0 holds a, the ac edge and c; rows 1..n-1
        // hold an ab-edge point, interior points and a bc-edge point.
        let mut rank = self.before_face[origin.face as usize];
        if i >= 1 {
            rank += flag(new_a) + flag(new_ac) * n.saturating_sub(1) + flag(new_c);
            let rows = (i - 1).min(n.saturating_sub(1));
            rank += rows * (flag(new_ab) + flag(new_bc)) + rows * n.saturating_sub(1)
                - rows * (rows + 1) / 2;
        }
        // Points before (i, j) in row i.
        if j >= 1 {
            if i == 0 {
                rank += flag(new_a) + flag(new_ac) * (j - 1).min(n.saturating_sub(1));
            } else if i < n {
                rank += flag(new_ab) + (j - 1).min(n - i - 1);
            }
        }
        rank
    }
}

/// The coordinates `generate(n)` gives the vertex at `origin`.
pub(crate) fn origin_position(n: usize, origin: VertexOrigin) -> Result<CartesianPoint, String> {
    origin_position_on(n, origin, &icosahedron_vertices())
}

/// The lattice points at the corners of a level-n face, in the order
/// `generate` passes them to `push_oriented`.
pub(crate) fn face_lattice_corners(address: TriangleAddress) -> [(usize, usize); 3] {
    let (i, j) = (address.i, address.j);
    match address.orientation {
        TriangleOrientation::Up => [(i, j), (i + 1, j), (i, j + 1)],
        TriangleOrientation::Down => [(i + 1, j), (i + 1, j + 1), (i, j + 1)],
    }
}

/// The level-`n` faces inside `ancestors`, faces of levels that divide `n` by
/// a power of two.
pub fn descendant_faces(
    ancestors: impl IntoIterator<Item = TriangleAddress>,
    n: usize,
) -> Result<BTreeSet<TriangleAddress>, String> {
    let mut faces = BTreeSet::new();
    let mut frontier = ancestors.into_iter().collect::<Vec<_>>();
    while let Some(face) = frontier.pop() {
        if face.n == n {
            faces.insert(face);
            continue;
        }
        if face.n == 0 || face.n > n || !n.is_multiple_of(face.n) || !(n / face.n).is_power_of_two()
        {
            return Err(format!("face {face:?} is not an ancestor at level {n}"));
        }
        frontier.extend(
            face.children_2_to_1()
                .ok_or_else(|| format!("face {face:?} is not a lattice face"))?,
        );
    }
    Ok(faces)
}

impl MotherGrid {
    /// The faces `generate(n)` builds at `faces`, and only those: the same
    /// corner coordinates, addresses, corner order and orientation, with
    /// vertex and face slots in the relative order `generate` gives them.
    /// Built from every face it is `generate(n)` itself.
    pub fn generate_faces(
        n: usize,
        faces: impl IntoIterator<Item = TriangleAddress>,
    ) -> Result<Self, String> {
        if n == 0 {
            return Err("mother subdivision must be positive".into());
        }
        let faces = faces.into_iter().collect::<BTreeSet<_>>();
        if let Some(face) = faces
            .iter()
            .find(|face| face.n != n || face.dense_index(n).is_err())
        {
            return Err(format!("face {face:?} is not a level-{n} lattice face"));
        }
        let mut origins = BTreeSet::new();
        let mut face_origins = Vec::with_capacity(faces.len());
        for &face in &faces {
            let mut corners = [VertexOrigin {
                face: 0,
                i: 0,
                j: 0,
            }; 3];
            for (corner, (i, j)) in corners.iter_mut().zip(face_lattice_corners(face)) {
                *corner = vertex_origin(n, face.base_face, i, j)?;
            }
            origins.extend(corners);
            face_origins.push((face, corners));
        }

        let base = icosahedron_vertices();
        let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
        let mut addresses = vec![None, None];
        let mut slot_origins = vec![None, None];
        let mut slot_of = BTreeMap::new();
        for origin in origins {
            slot_of.insert(origin, vertices.len());
            vertices.push(origin_position_on(n, origin, &base)?);
            addresses.push(Some(origin_address(n, origin)));
            slot_origins.push(Some(origin));
        }

        let mut triangles = vec![[1usize; 3]; 2];
        let mut triangle_addresses = vec![None, None];
        let mut face_slots = BTreeMap::new();
        for (face, corners) in face_origins {
            face_slots.insert(face, triangles.len());
            push_oriented(
                &mut triangles,
                &vertices,
                corners.map(|origin| slot_of[&origin]),
            )?;
            triangle_addresses.push(Some(face));
        }
        let mesh = MeshState::from_parts(vertices, triangles).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        let mut outer_boundary = BTreeSet::new();
        for face in mesh.active_triangle_slots() {
            let corners = mesh.triangles()[face];
            for (corner, &neighbour) in mesh.neighbours()[face].iter().enumerate() {
                if neighbour == 0 {
                    outer_boundary.insert(corners[(corner + 1) % 3]);
                    outer_boundary.insert(corners[(corner + 2) % 3]);
                }
            }
        }
        Ok(Self {
            subdivision: n,
            mesh,
            addresses,
            triangle_addresses,
            region: Some(Box::new(RegionIndex {
                face_slots,
                origins: slot_origins,
                outer_boundary,
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_face(n: usize) -> Vec<TriangleAddress> {
        MotherGrid::generate(n)
            .unwrap()
            .triangle_addresses
            .into_iter()
            .flatten()
            .collect()
    }

    /// Built from every face, a region is the whole grid, slot for slot.
    #[test]
    fn a_region_of_every_face_is_the_whole_grid() {
        for n in [1, 2, 3, 4, 5, 8, 12] {
            let whole = MotherGrid::generate(n).unwrap();
            let region = MotherGrid::generate_faces(n, every_face(n)).unwrap();
            assert_eq!(region.mesh, whole.mesh, "n = {n}");
            assert_eq!(region.addresses, whole.addresses, "n = {n}");
            assert_eq!(
                region.triangle_addresses, whole.triangle_addresses,
                "n = {n}"
            );
        }
    }

    /// Any subset of faces gets the whole grid's corners bit for bit, in the
    /// same order, with slots in the same relative order.
    #[test]
    fn a_region_matches_the_whole_grid_face_by_face() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for n in [2, 3, 6, 8] {
            let whole = MotherGrid::generate(n).unwrap();
            let faces = every_face(n);
            for _ in 0..6 {
                let subset = faces
                    .iter()
                    .copied()
                    .filter(|_| next() % 5 == 0)
                    .collect::<Vec<_>>();
                if subset.is_empty() {
                    continue;
                }
                let region = MotherGrid::generate_faces(n, subset.iter().copied()).unwrap();
                let mut whole_slot_of = BTreeMap::new();
                for face in &subset {
                    let whole_face = face.dense_index(n).unwrap() + 2;
                    let region_face = region.region.as_ref().unwrap().face_slot(*face).unwrap();
                    let whole_corners = whole.mesh.triangles()[whole_face];
                    let region_corners = region.mesh.triangles()[region_face];
                    for (w, r) in whole_corners.into_iter().zip(region_corners) {
                        assert_eq!(whole.addresses[w], region.addresses[r], "{face:?}");
                        let (wp, rp) = (whole.mesh.vertices()[w], region.mesh.vertices()[r]);
                        assert_eq!(
                            (wp.x.to_bits(), wp.y.to_bits(), wp.z.to_bits()),
                            (rp.x.to_bits(), rp.y.to_bits(), rp.z.to_bits()),
                            "{face:?}"
                        );
                        whole_slot_of.insert(r, w);
                    }
                }
                // Region slots ascend with the whole grid's slots.
                let whole_order = whole_slot_of.values().copied().collect::<Vec<_>>();
                assert!(
                    whole_order.windows(2).all(|pair| pair[0] < pair[1]),
                    "n = {n}"
                );
            }
        }
    }

    /// The leaf machinery runs on a region as on the whole grid: condensing
    /// the same parents and rebuilding gives the same faces, corner for corner
    /// and in the same order, wherever the region has them.
    #[test]
    fn a_region_condenses_and_rebuilds_as_the_whole_grid() {
        use crate::coarsen::{rebuild_from_leaf_set, HierarchyLeafSet};
        let (coarse, n) = (2, 8);
        let bases = every_face(coarse);
        let inside = bases
            .iter()
            .copied()
            .step_by(3)
            .take(14)
            .collect::<Vec<_>>();
        let whole = MotherGrid::generate(n).unwrap();
        let region =
            MotherGrid::generate_faces(n, descendant_faces(inside.clone(), n).unwrap()).unwrap();
        // Condense nine level-4 parents under the region's first two bases.
        let mut level4 = descendant_faces(inside[..2].iter().copied(), 4)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();
        level4.truncate(9);
        let rebuilt = |grid: &MotherGrid| {
            let mut leaves = HierarchyLeafSet::from_mother_grid(grid).unwrap();
            leaves.condense_core(&level4).unwrap();
            rebuild_from_leaf_set(grid, &leaves).unwrap()
        };
        let (whole_leaves, region_leaves) = (rebuilt(&whole), rebuilt(&region));
        let corners =
            |grid: &MotherGrid, leaves: &crate::coarsen::HierarchyLeafMesh, face: usize| {
                leaves.mesh.triangles()[face].map(|compact| {
                    let source = leaves.source_vertex_slots[compact].unwrap();
                    let point = grid.mesh.vertices()[source];
                    (
                        grid.addresses[source].clone(),
                        [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()],
                    )
                })
            };
        let mut whole_faces = whole_leaves
            .mesh
            .active_triangle_slots()
            .filter(|&face| {
                let address = whole_leaves.triangle_addresses[face].unwrap();
                region.face_slot(address).is_some() || level4.contains(&address)
            })
            .collect::<Vec<_>>()
            .into_iter();
        for face in region_leaves.mesh.active_triangle_slots() {
            let whole_face = whole_faces
                .next()
                .expect("the whole grid has every region face");
            assert_eq!(
                region_leaves.triangle_addresses[face],
                whole_leaves.triangle_addresses[whole_face]
            );
            assert_eq!(
                corners(&region, &region_leaves, face),
                corners(&whole, &whole_leaves, whole_face)
            );
        }
        assert!(whole_faces.next().is_none());
    }

    /// The outer boundary is every built vertex that some unbuilt face of the
    /// whole grid touches; a region of every face has none.
    #[test]
    fn the_outer_boundary_is_where_unbuilt_faces_touch() {
        let n = 6;
        let whole = MotherGrid::generate(n).unwrap();
        let everything = MotherGrid::generate_faces(n, every_face(n)).unwrap();
        assert!(everything
            .region
            .as_ref()
            .unwrap()
            .outer_boundary()
            .is_empty());
        let built = every_face(n)
            .into_iter()
            .step_by(2)
            .take(150)
            .collect::<BTreeSet<_>>();
        let region = MotherGrid::generate_faces(n, built.iter().copied()).unwrap();
        let mut touched_by_unbuilt = BTreeSet::new();
        for face in whole.mesh.active_triangle_slots() {
            if !built.contains(&whole.triangle_addresses[face].unwrap()) {
                for site in whole.mesh.triangles()[face] {
                    touched_by_unbuilt.insert(whole.addresses[site].clone().unwrap());
                }
            }
        }
        let outer = region
            .region
            .as_ref()
            .unwrap()
            .outer_boundary()
            .iter()
            .map(|&slot| region.addresses[slot].clone().unwrap())
            .collect::<BTreeSet<_>>();
        let built_vertices = region
            .mesh
            .active_vertex_slots()
            .map(|slot| region.addresses[slot].clone().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            outer,
            built_vertices
                .intersection(&touched_by_unbuilt)
                .cloned()
                .collect()
        );
    }

    /// Ranks are slots: every vertex of `generate(n)` at its slot less two.
    #[test]
    fn global_ranks_are_generate_slots() {
        for n in [1, 2, 3, 5, 8] {
            let whole = MotherGrid::generate(n).unwrap();
            let numbering = GlobalNumbering::new(n);
            for face in 0..20u8 {
                for i in 0..=n {
                    for j in 0..=n - i {
                        let origin = vertex_origin(n, face, i, j).unwrap();
                        let address = origin_address(n, origin);
                        let slot = whole
                            .addresses
                            .iter()
                            .position(|candidate| candidate.as_ref() == Some(&address))
                            .unwrap();
                        assert_eq!(numbering.rank(origin), slot - 2, "n {n} {origin:?}");
                    }
                }
            }
        }
    }

    /// Every lattice point of every base face names the origin `generate`
    /// gives its slot: one origin per vertex, in slot order.
    #[test]
    fn origins_order_vertices_as_generate_numbers_them() {
        for n in [1, 2, 3, 7] {
            let whole = MotherGrid::generate(n).unwrap();
            let mut origin_of_slot = BTreeMap::new();
            for face in 0..20u8 {
                for i in 0..=n {
                    for j in 0..=n - i {
                        let origin = vertex_origin(n, face, i, j).unwrap();
                        let address = origin_address(n, origin);
                        let slot = whole
                            .addresses
                            .iter()
                            .position(|candidate| candidate.as_ref() == Some(&address))
                            .unwrap();
                        assert_eq!(*origin_of_slot.entry(slot).or_insert(origin), origin);
                    }
                }
            }
            let origins = origin_of_slot.values().copied().collect::<Vec<_>>();
            assert_eq!(origins.len(), whole.mesh.vertex_count(), "n = {n}");
            assert!(origins.windows(2).all(|pair| pair[0] < pair[1]), "n = {n}");
        }
    }
}
