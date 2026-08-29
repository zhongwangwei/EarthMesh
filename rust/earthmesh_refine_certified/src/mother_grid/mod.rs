use earthmesh_mesh::{
    normalize_cartesian_to_radius, orientation_on_sphere, CartesianPoint, MeshState, Sign,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VertexAddress {
    IcosahedronVertex(u8),
    IcosahedronEdge {
        a: u8,
        b: u8,
        step: usize,
        n: usize,
    },
    IcosahedronFace {
        face: u8,
        i: usize,
        j: usize,
        k: usize,
        n: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriangleOrientation {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TriangleAddress {
    pub base_face: u8,
    pub i: usize,
    pub j: usize,
    pub n: usize,
    pub orientation: TriangleOrientation,
}

impl TriangleAddress {
    pub fn parent_2_to_1(self) -> Option<Self> {
        if self.n < 2 || !self.n.is_multiple_of(2) {
            return None;
        }
        let offset = match self.orientation {
            TriangleOrientation::Up => 1,
            TriangleOrientation::Down => 2,
        };
        let i_numerator = self.i.checked_mul(3)?.checked_add(offset)?;
        let j_numerator = self.j.checked_mul(3)?.checked_add(offset)?;
        let i = i_numerator / 6;
        let j = j_numerator / 6;
        let orientation = if i_numerator % 6 + j_numerator % 6 < 6 {
            TriangleOrientation::Up
        } else {
            TriangleOrientation::Down
        };
        Some(Self {
            base_face: self.base_face,
            i,
            j,
            n: self.n / 2,
            orientation,
        })
    }

    pub fn children_2_to_1(self) -> Option<[Self; 4]> {
        if !self.is_valid() {
            return None;
        }
        let n = self.n.checked_mul(2)?;
        let i = self.i.checked_mul(2)?;
        let j = self.j.checked_mul(2)?;
        let base = |i, j, orientation| Self {
            base_face: self.base_face,
            i,
            j,
            n,
            orientation,
        };
        Some(match self.orientation {
            TriangleOrientation::Up => [
                base(i, j, TriangleOrientation::Up),
                base(i + 1, j, TriangleOrientation::Up),
                base(i, j + 1, TriangleOrientation::Up),
                base(i, j, TriangleOrientation::Down),
            ],
            TriangleOrientation::Down => [
                base(i + 1, j, TriangleOrientation::Down),
                base(i, j + 1, TriangleOrientation::Down),
                base(i + 1, j + 1, TriangleOrientation::Down),
                base(i + 1, j + 1, TriangleOrientation::Up),
            ],
        })
    }

    fn is_valid(self) -> bool {
        if self.base_face >= 20 || self.n == 0 {
            return false;
        }
        let Some(sum) = self.i.checked_add(self.j) else {
            return false;
        };
        match self.orientation {
            TriangleOrientation::Up => sum < self.n,
            TriangleOrientation::Down => sum.checked_add(1).is_some_and(|sum| sum < self.n),
        }
    }

    pub(crate) fn dense_index(self, n: usize) -> Result<usize, String> {
        if self.base_face >= 20 || self.n != n || n == 0 {
            return Err(format!("invalid triangle address {self:?}"));
        }
        if !self.is_valid() {
            return Err(format!("invalid triangle address {self:?}"));
        }
        let row_width = n
            .checked_mul(2)
            .and_then(|width| width.checked_sub(self.i))
            .ok_or_else(|| "triangle dense row overflow".to_string())?;
        let local = self
            .i
            .checked_mul(row_width)
            .and_then(|local| local.checked_add(self.j.checked_mul(2)?))
            .and_then(|local| {
                local.checked_add(match self.orientation {
                    TriangleOrientation::Up => 0,
                    TriangleOrientation::Down => 1,
                })
            })
            .ok_or_else(|| "triangle dense index overflow".to_string())?;
        let per_face = n
            .checked_mul(n)
            .ok_or_else(|| "triangle dense base overflow".to_string())?;
        if local >= per_face {
            return Err(format!(
                "triangle address {self:?} dense local index is out of range"
            ));
        }
        (self.base_face as usize)
            .checked_mul(per_face)
            .and_then(|base| base.checked_add(local))
            .ok_or_else(|| "triangle dense index overflow".to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotherGrid {
    pub subdivision: usize,
    pub mesh: MeshState,
    pub addresses: Vec<Option<VertexAddress>>,
    pub triangle_addresses: Vec<Option<TriangleAddress>>,
}

pub fn analytic_counts(n: usize) -> Option<(usize, usize, usize)> {
    if n == 0 {
        return None;
    }
    let n2 = n.checked_mul(n)?;
    Some((
        10usize.checked_mul(n2)?.checked_add(2)?,
        30usize.checked_mul(n2)?,
        20usize.checked_mul(n2)?,
    ))
}

pub fn mother_cell_count(n: usize) -> Option<usize> {
    analytic_counts(n).map(|(_, _, faces)| faces)
}

impl MotherGrid {
    pub fn generate(n: usize) -> Result<Self, String> {
        if n == 0 {
            return Err("mother subdivision must be positive".into());
        }
        let base = icosahedron_vertices();
        let faces = icosahedron_faces();
        let (vertex_count, _, triangle_count) =
            analytic_counts(n).ok_or_else(|| "mother subdivision is too large".to_string())?;
        let edge_lookup = edge_lookup(&faces);
        let mut vertex_ids = [None; 12];
        let mut edge_ids = vec![None; 30usize.saturating_mul(n + 1)];
        let mut vertices = Vec::with_capacity(vertex_count + 2);
        vertices.resize(2, CartesianPoint::new(0.0, 0.0, 0.0));
        let mut addresses = Vec::with_capacity(vertex_count + 2);
        addresses.resize(2, None);
        let mut triangles = Vec::with_capacity(triangle_count + 2);
        triangles.resize(2, [1usize; 3]);
        let mut triangle_addresses = Vec::with_capacity(triangle_count + 2);
        triangle_addresses.resize(2, None);

        for (face_id, &[a, b, c]) in faces.iter().enumerate() {
            let row_width = n + 1;
            let mut grid = vec![0usize; row_width * row_width];
            for i in 0..=n {
                for j in 0..=n - i {
                    let k = n - i - j;
                    grid[grid_index(i, j, row_width)] = get_or_insert_vertex(
                        face_id as u8,
                        [a, b, c],
                        [k, i, j],
                        n,
                        &base,
                        &edge_lookup,
                        &mut vertex_ids,
                        &mut edge_ids,
                        &mut vertices,
                        &mut addresses,
                    );
                }
            }
            for i in 0..n {
                for j in 0..n - i {
                    push_oriented(
                        &mut triangles,
                        &vertices,
                        [
                            grid[grid_index(i, j, row_width)],
                            grid[grid_index(i + 1, j, row_width)],
                            grid[grid_index(i, j + 1, row_width)],
                        ],
                    )?;
                    triangle_addresses.push(Some(TriangleAddress {
                        base_face: face_id as u8,
                        i,
                        j,
                        n,
                        orientation: TriangleOrientation::Up,
                    }));
                    if i + j < n - 1 {
                        push_oriented(
                            &mut triangles,
                            &vertices,
                            [
                                grid[grid_index(i + 1, j, row_width)],
                                grid[grid_index(i + 1, j + 1, row_width)],
                                grid[grid_index(i, j + 1, row_width)],
                            ],
                        )?;
                        triangle_addresses.push(Some(TriangleAddress {
                            base_face: face_id as u8,
                            i,
                            j,
                            n,
                            orientation: TriangleOrientation::Down,
                        }));
                    }
                }
            }
        }
        let mesh = MeshState::from_parts(vertices, triangles).map_err(|errors| {
            errors
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        Ok(Self {
            subdivision: n,
            mesh,
            addresses,
            triangle_addresses,
        })
    }
}

fn grid_index(i: usize, j: usize, row_width: usize) -> usize {
    i * row_width + j
}

fn edge_lookup(faces: &[[u8; 3]; 20]) -> [[Option<usize>; 12]; 12] {
    let mut lookup = [[None; 12]; 12];
    let mut next = 0usize;
    for &[a, b, c] in faces {
        for (mut x, mut y) in [(a, b), (b, c), (c, a)] {
            if x > y {
                std::mem::swap(&mut x, &mut y);
            }
            let x = x as usize;
            let y = y as usize;
            if lookup[x][y].is_none() {
                lookup[x][y] = Some(next);
                lookup[y][x] = Some(next);
                next += 1;
            }
        }
    }
    debug_assert_eq!(next, 30);
    lookup
}

#[allow(clippy::too_many_arguments)]
fn get_or_insert_vertex(
    face: u8,
    corners: [u8; 3],
    weights: [usize; 3],
    n: usize,
    base: &[CartesianPoint; 12],
    edge_lookup: &[[Option<usize>; 12]; 12],
    vertex_ids: &mut [Option<usize>; 12],
    edge_ids: &mut [Option<usize>],
    vertices: &mut Vec<CartesianPoint>,
    addresses: &mut Vec<Option<VertexAddress>>,
) -> usize {
    if let Some(pos) = weights.iter().position(|&w| w == n) {
        return *vertex_ids[corners[pos] as usize].get_or_insert_with(|| {
            push_vertex(
                VertexAddress::IcosahedronVertex(corners[pos]),
                corners,
                weights,
                base,
                vertices,
                addresses,
            )
        });
    }

    if let Some(zero) = weights.iter().position(|&w| w == 0) {
        let mut ends = [
            (corners[(zero + 1) % 3], weights[(zero + 1) % 3]),
            (corners[(zero + 2) % 3], weights[(zero + 2) % 3]),
        ];
        ends.sort_by_key(|x| x.0);
        let edge = edge_lookup[ends[0].0 as usize][ends[1].0 as usize]
            .expect("icosahedron edge must be indexed");
        let slot = edge * (n + 1) + ends[1].1;
        return *edge_ids[slot].get_or_insert_with(|| {
            push_vertex(
                VertexAddress::IcosahedronEdge {
                    a: ends[0].0,
                    b: ends[1].0,
                    step: ends[1].1,
                    n,
                },
                corners,
                weights,
                base,
                vertices,
                addresses,
            )
        });
    }

    push_vertex(
        VertexAddress::IcosahedronFace {
            face,
            i: weights[1],
            j: weights[2],
            k: weights[0],
            n,
        },
        corners,
        weights,
        base,
        vertices,
        addresses,
    )
}

fn push_vertex(
    address: VertexAddress,
    corners: [u8; 3],
    weights: [usize; 3],
    base: &[CartesianPoint; 12],
    vertices: &mut Vec<CartesianPoint>,
    addresses: &mut Vec<Option<VertexAddress>>,
) -> usize {
    let p = normalize_cartesian_to_radius(
        weighted(
            base[corners[0] as usize],
            weights[0],
            base[corners[1] as usize],
            weights[1],
            base[corners[2] as usize],
            weights[2],
        ),
        1.0,
    )
    .unwrap();
    vertices.push(p);
    addresses.push(Some(address));
    vertices.len() - 1
}

fn weighted(
    a: CartesianPoint,
    aw: usize,
    b: CartesianPoint,
    bw: usize,
    c: CartesianPoint,
    cw: usize,
) -> CartesianPoint {
    let n = (aw + bw + cw) as f64;
    CartesianPoint::new(
        (a.x * aw as f64 + b.x * bw as f64 + c.x * cw as f64) / n,
        (a.y * aw as f64 + b.y * bw as f64 + c.y * cw as f64) / n,
        (a.z * aw as f64 + b.z * bw as f64 + c.z * cw as f64) / n,
    )
}

#[cfg(test)]
fn address(face: u8, corners: [u8; 3], i: usize, j: usize, k: usize, n: usize) -> VertexAddress {
    let weights = [(corners[0], k), (corners[1], i), (corners[2], j)];
    if let Some(&(v, _)) = weights.iter().find(|(_, w)| *w == n) {
        return VertexAddress::IcosahedronVertex(v);
    }
    if let Some(zero) = weights.iter().position(|(_, w)| *w == 0) {
        let mut ends = [weights[(zero + 1) % 3], weights[(zero + 2) % 3]];
        ends.sort_by_key(|x| x.0);
        return VertexAddress::IcosahedronEdge {
            a: ends[0].0,
            b: ends[1].0,
            step: ends[1].1,
            n,
        };
    }
    VertexAddress::IcosahedronFace { face, i, j, k, n }
}

pub(crate) fn push_oriented(
    triangles: &mut Vec<[usize; 3]>,
    vertices: &[CartesianPoint],
    mut tri: [usize; 3],
) -> Result<(), String> {
    match orientation_on_sphere(vertices[tri[0]], vertices[tri[1]], vertices[tri[2]]) {
        Ok(Sign::Positive) => {}
        Ok(Sign::Negative) => tri.swap(1, 2),
        Ok(Sign::Zero) => return Err("degenerate mother-grid triangle".into()),
        Err(e) => return Err(e.to_string()),
    }
    triangles.push(tri);
    Ok(())
}

fn icosahedron_vertices() -> [CartesianPoint; 12] {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let raw = [
        (-1.0, phi, 0.0),
        (1.0, phi, 0.0),
        (-1.0, -phi, 0.0),
        (1.0, -phi, 0.0),
        (0.0, -1.0, phi),
        (0.0, 1.0, phi),
        (0.0, -1.0, -phi),
        (0.0, 1.0, -phi),
        (phi, 0.0, -1.0),
        (phi, 0.0, 1.0),
        (-phi, 0.0, -1.0),
        (-phi, 0.0, 1.0),
    ];
    raw.map(|(x, y, z)| normalize_cartesian_to_radius(CartesianPoint::new(x, y, z), 1.0).unwrap())
}

fn icosahedron_faces() -> [[u8; 3]; 20] {
    [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    fn reference_generate(n: usize) -> MotherGrid {
        let base = icosahedron_vertices();
        let faces = icosahedron_faces();
        let mut ids = BTreeMap::<VertexAddress, usize>::new();
        let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
        let mut addresses = vec![None, None];
        let mut triangles = vec![[1usize; 3]; 2];
        let mut triangle_addresses = vec![None, None];

        for (face_id, &[a, b, c]) in faces.iter().enumerate() {
            let mut grid = BTreeMap::<(usize, usize), usize>::new();
            for i in 0..=n {
                for j in 0..=n - i {
                    let k = n - i - j;
                    let address = address(face_id as u8, [a, b, c], i, j, k, n);
                    let id = *ids.entry(address.clone()).or_insert_with(|| {
                        let p = normalize_cartesian_to_radius(
                            weighted(
                                base[a as usize],
                                k,
                                base[b as usize],
                                i,
                                base[c as usize],
                                j,
                            ),
                            1.0,
                        )
                        .unwrap();
                        vertices.push(p);
                        addresses.push(Some(address));
                        vertices.len() - 1
                    });
                    grid.insert((i, j), id);
                }
            }
            for i in 0..n {
                for j in 0..n - i {
                    push_oriented(
                        &mut triangles,
                        &vertices,
                        [grid[&(i, j)], grid[&(i + 1, j)], grid[&(i, j + 1)]],
                    )
                    .unwrap();
                    triangle_addresses.push(Some(TriangleAddress {
                        base_face: face_id as u8,
                        i,
                        j,
                        n,
                        orientation: TriangleOrientation::Up,
                    }));
                    if i + j < n - 1 {
                        push_oriented(
                            &mut triangles,
                            &vertices,
                            [grid[&(i + 1, j)], grid[&(i + 1, j + 1)], grid[&(i, j + 1)]],
                        )
                        .unwrap();
                        triangle_addresses.push(Some(TriangleAddress {
                            base_face: face_id as u8,
                            i,
                            j,
                            n,
                            orientation: TriangleOrientation::Down,
                        }));
                    }
                }
            }
        }

        MotherGrid {
            subdivision: n,
            mesh: MeshState::from_parts(vertices, triangles).unwrap(),
            addresses,
            triangle_addresses,
        }
    }

    fn triangle_vertices(tri: TriangleAddress) -> [VertexAddress; 3] {
        let [a, b, c] = icosahedron_faces()[tri.base_face as usize];
        let n = tri.n;
        match tri.orientation {
            TriangleOrientation::Up => [
                address(tri.base_face, [a, b, c], tri.i, tri.j, n - tri.i - tri.j, n),
                address(
                    tri.base_face,
                    [a, b, c],
                    tri.i + 1,
                    tri.j,
                    n - tri.i - tri.j - 1,
                    n,
                ),
                address(
                    tri.base_face,
                    [a, b, c],
                    tri.i,
                    tri.j + 1,
                    n - tri.i - tri.j - 1,
                    n,
                ),
            ],
            TriangleOrientation::Down => [
                address(
                    tri.base_face,
                    [a, b, c],
                    tri.i + 1,
                    tri.j,
                    n - tri.i - tri.j - 1,
                    n,
                ),
                address(
                    tri.base_face,
                    [a, b, c],
                    tri.i + 1,
                    tri.j + 1,
                    n - tri.i - tri.j - 2,
                    n,
                ),
                address(
                    tri.base_face,
                    [a, b, c],
                    tri.i,
                    tri.j + 1,
                    n - tri.i - tri.j - 1,
                    n,
                ),
            ],
        }
    }

    #[test]
    fn children_are_exact_parent_inverse_for_powers_of_two() {
        for n in [1, 2, 4, 8] {
            let parents = MotherGrid::generate(n).unwrap();
            let children = MotherGrid::generate(n * 2).unwrap();
            let child_addresses = children
                .triangle_addresses
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut generated = BTreeSet::new();

            for parent in parents.triangle_addresses.iter().flatten().copied() {
                let four = parent.children_2_to_1().unwrap();
                assert_eq!(four.len(), 4);
                for child in four {
                    assert_eq!(child.parent_2_to_1(), Some(parent));
                    assert!(child_addresses.contains(&child), "missing child {child:?}");
                    assert!(generated.insert(child), "duplicate child {child:?}");
                }
            }

            assert_eq!(generated.len(), child_addresses.len());
            assert_eq!(generated, child_addresses);
            assert_eq!(generated.len(), mother_cell_count(n * 2).unwrap());
        }
    }

    #[test]
    fn children_cover_seams_base_faces_and_icosahedron_vertices() {
        let grid = MotherGrid::generate(4).unwrap();
        let child_addresses = MotherGrid::generate(8)
            .unwrap()
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut base_faces = BTreeSet::new();
        let mut vertices = BTreeSet::new();

        for parent in grid.triangle_addresses.iter().flatten().copied() {
            base_faces.insert(parent.base_face);
            let adjacent_vertices = triangle_vertices(parent)
                .into_iter()
                .filter_map(|vertex| match vertex {
                    VertexAddress::IcosahedronVertex(v) => Some(v),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if adjacent_vertices.is_empty() {
                continue;
            }
            vertices.extend(adjacent_vertices);
            for child in parent.children_2_to_1().unwrap() {
                assert_eq!(child.base_face, parent.base_face);
                assert_eq!(child.parent_2_to_1(), Some(parent));
                assert!(child_addresses.contains(&child));
            }
        }

        assert_eq!(base_faces, (0u8..20).collect::<BTreeSet<_>>());
        assert_eq!(vertices, (0u8..12).collect::<BTreeSet<_>>());
    }

    #[test]
    fn invalid_triangle_addresses_have_no_children() {
        for bad in [
            TriangleAddress {
                base_face: 20,
                i: 0,
                j: 0,
                n: 1,
                orientation: TriangleOrientation::Up,
            },
            TriangleAddress {
                base_face: 0,
                i: 0,
                j: 0,
                n: 0,
                orientation: TriangleOrientation::Up,
            },
            TriangleAddress {
                base_face: 0,
                i: 1,
                j: 0,
                n: 1,
                orientation: TriangleOrientation::Up,
            },
            TriangleAddress {
                base_face: 0,
                i: 0,
                j: 0,
                n: 1,
                orientation: TriangleOrientation::Down,
            },
        ] {
            assert_eq!(bad.children_2_to_1(), None);
        }
    }

    #[test]
    fn generate_matches_legacy_btree_ordering() {
        for n in [1, 2, 3, 4] {
            let fast = MotherGrid::generate(n).unwrap();
            let reference = reference_generate(n);
            assert_eq!(fast.addresses, reference.addresses);
            assert_eq!(fast.triangle_addresses, reference.triangle_addresses);
            assert_eq!(fast.mesh.vertices(), reference.mesh.vertices());
            assert_eq!(fast.mesh.triangles(), reference.mesh.triangles());
        }
    }
}
