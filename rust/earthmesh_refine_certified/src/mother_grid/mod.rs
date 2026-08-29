use earthmesh_mesh::{
    normalize_cartesian_to_radius, orientation_on_sphere, CartesianPoint, MeshState, Sign,
};
use std::collections::BTreeMap;

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
                            [grid[&(i + 1, j)], grid[&(i + 1, j + 1)], grid[&(i, j + 1)]],
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

fn push_oriented(
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
