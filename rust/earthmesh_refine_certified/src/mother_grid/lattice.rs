//! The level-n lattice by address alone (design B1r-1): the faces around a
//! vertex, a face's neighbours, and the face holding a point -- without
//! building the grid. A regional run on a fine base cannot build its whole
//! base grid (a 1 km base has about a billion faces), so whatever it needs of
//! the base it computes here.

use super::region::{origin_address, vertex_origin, VertexOrigin};
use super::{
    icosahedron_faces, icosahedron_vertices, TriangleAddress, TriangleOrientation, VertexAddress,
};
use std::collections::BTreeSet;

/// The lattice point `(i, j)` of base face `face` in the frame of every base
/// face that holds it: one face for an interior point, two on an icosahedron
/// edge, five at an icosahedron vertex.
fn frames(n: usize, origin: VertexOrigin) -> Vec<(u8, usize, usize)> {
    let faces = icosahedron_faces();
    match origin_address(n, origin) {
        VertexAddress::IcosahedronFace { face, i, j, .. } => vec![(face, i, j)],
        VertexAddress::IcosahedronEdge { a, b, step, .. } => (0..20u8)
            .filter(|&face| faces[face as usize].contains(&a) && faces[face as usize].contains(&b))
            .map(|face| {
                let mut weights = [0usize; 3];
                for (position, &corner) in faces[face as usize].iter().enumerate() {
                    if corner == a {
                        weights[position] = n - step;
                    } else if corner == b {
                        weights[position] = step;
                    }
                }
                (face, weights[1], weights[2])
            })
            .collect(),
        VertexAddress::IcosahedronVertex(vertex) => (0..20u8)
            .filter_map(|face| {
                let position = faces[face as usize]
                    .iter()
                    .position(|&corner| corner == vertex)?;
                Some(match position {
                    0 => (face, 0, 0),
                    1 => (face, n, 0),
                    _ => (face, 0, n),
                })
            })
            .collect(),
    }
}

/// The level-n faces with a corner at the lattice vertex `origin`.
pub fn faces_at_vertex(n: usize, origin: VertexOrigin) -> Vec<TriangleAddress> {
    let mut faces = BTreeSet::new();
    for (face, i, j) in frames(n, origin) {
        let (i, j) = (i as isize, j as isize);
        let candidates = [
            (i, j, TriangleOrientation::Up),
            (i - 1, j, TriangleOrientation::Up),
            (i, j - 1, TriangleOrientation::Up),
            (i - 1, j, TriangleOrientation::Down),
            (i, j - 1, TriangleOrientation::Down),
            (i - 1, j - 1, TriangleOrientation::Down),
        ];
        for (a, b, orientation) in candidates {
            if a < 0 || b < 0 {
                continue;
            }
            let address = TriangleAddress {
                base_face: face,
                i: a as usize,
                j: b as usize,
                n,
                orientation,
            };
            if address.dense_index(n).is_ok() {
                faces.insert(address);
            }
        }
    }
    faces.into_iter().collect()
}

/// The origins of a level-n face's corners, in lattice order.
pub fn face_corner_origins(address: TriangleAddress) -> Result<[VertexOrigin; 3], String> {
    let mut corners = [VertexOrigin {
        face: 0,
        i: 0,
        j: 0,
    }; 3];
    for (corner, (i, j)) in corners
        .iter_mut()
        .zip(super::region::face_lattice_corners(address))
    {
        *corner = vertex_origin(address.n, address.base_face, i, j)?;
    }
    Ok(corners)
}

/// Faces sharing a corner with `address` (itself excluded).
pub fn faces_around(address: TriangleAddress) -> Result<Vec<TriangleAddress>, String> {
    let mut around = BTreeSet::new();
    for corner in face_corner_origins(address)? {
        around.extend(faces_at_vertex(address.n, corner));
    }
    around.remove(&address);
    Ok(around.into_iter().collect())
}

/// The faces across `address`'s sides: entry k lies across the side opposite
/// corner k (corners in lattice order), as `neighbours()` lists them.
pub fn faces_across(address: TriangleAddress) -> Result<[TriangleAddress; 3], String> {
    let corners = face_corner_origins(address)?;
    let mut across = [None; 3];
    for candidate in faces_around(address)? {
        let theirs = face_corner_origins(candidate)?;
        for (corner, slot) in across.iter_mut().enumerate() {
            let (a, b) = (corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
            if theirs.contains(&a) && theirs.contains(&b) {
                *slot = Some(candidate);
            }
        }
    }
    let [Some(first), Some(second), Some(third)] = across else {
        return Err(format!("face {address:?} lacks a face across a side"));
    };
    Ok([first, second, third])
}

/// Which sides of `face` -- a face of a level dividing `n` by a power of two
/// -- hold the level-`n` lattice point `origin`; side k lies opposite corner
/// k, corners in lattice order. Exact: integer arithmetic in the face's own
/// frame, whatever the scale.
pub fn sides_holding(
    face: TriangleAddress,
    n: usize,
    origin: VertexOrigin,
) -> Result<[bool; 3], String> {
    if face.n == 0 || n < face.n || !n.is_multiple_of(face.n) {
        return Err(format!("level {n} does not refine face {face:?}"));
    }
    let ratio = (n / face.n) as i64;
    let corners = super::region::face_lattice_corners(face)
        .map(|(i, j)| (i as i64 * ratio, j as i64 * ratio));
    let mut held = [false; 3];
    for (frame, i, j) in frames(n, origin) {
        if frame != face.base_face {
            continue;
        }
        let point = (i as i64, j as i64);
        for (side, slot) in held.iter_mut().enumerate() {
            let (start, end) = (corners[(side + 1) % 3], corners[(side + 2) % 3]);
            let direction = (end.0 - start.0, end.1 - start.1);
            let offset = (point.0 - start.0, point.1 - start.1);
            let cross = direction.0 * offset.1 - direction.1 * offset.0;
            let along = direction.0 * offset.0 + direction.1 * offset.1;
            let length = direction.0 * direction.0 + direction.1 * direction.1;
            if cross == 0 && (0..=length).contains(&along) {
                *slot = true;
            }
        }
    }
    Ok(held)
}

/// Faces sharing an edge with `address`: the three that share two corners.
pub fn edge_neighbours(address: TriangleAddress) -> Result<Vec<TriangleAddress>, String> {
    let corners = face_corner_origins(address)?;
    let mut neighbours = Vec::with_capacity(3);
    for candidate in faces_around(address)? {
        let shared = face_corner_origins(candidate)?
            .iter()
            .filter(|origin| corners.contains(origin))
            .count();
        if shared == 2 {
            neighbours.push(candidate);
        }
    }
    Ok(neighbours)
}

/// The level-n face holding the unit vector `point`. The lattice is the
/// radial projection of each base face's planar barycentric grid, so the
/// planar barycentric coordinates of the point's projection place it exactly
/// (up to rounding on shared edges, where either face is returned).
pub fn locate(n: usize, point: [f64; 3]) -> Option<TriangleAddress> {
    let base = icosahedron_vertices().map(|vertex| [vertex.x, vertex.y, vertex.z]);
    let faces = icosahedron_faces();
    let mut best = None::<(f64, u8, [f64; 3])>;
    for (face, &[a, b, c]) in faces.iter().enumerate() {
        let (a, b, c) = (base[a as usize], base[b as usize], base[c as usize]);
        // Solve point = alpha a + beta b + gamma c.
        let det = triple(a, b, c);
        if det.abs() < 1.0e-300 {
            continue;
        }
        let weights = [
            triple(point, b, c) / det,
            triple(a, point, c) / det,
            triple(a, b, point) / det,
        ];
        let sum = weights[0] + weights[1] + weights[2];
        if sum <= 0.0 {
            continue;
        }
        let weights = weights.map(|weight| weight / sum);
        // The face whose least weight is largest holds the point.
        let least = weights[0].min(weights[1]).min(weights[2]);
        if best.is_none_or(|(score, _, _)| least > score) {
            best = Some((least, face as u8, weights));
        }
    }
    let (_, face, weights) = best?;
    let (i_real, j_real) = (weights[1] * n as f64, weights[2] * n as f64);
    let mut i = (i_real.floor().max(0.0) as usize).min(n - 1);
    let mut j = (j_real.floor().max(0.0) as usize).min(n - 1);
    if i + j > n - 1 {
        // On the far edge: step back inside.
        let excess = i + j - (n - 1);
        if i >= excess {
            i -= excess;
        } else {
            j -= excess - i;
            i = 0;
        }
    }
    let (fi, fj) = (i_real - i as f64, j_real - j as f64);
    let orientation = if fi + fj <= 1.0 || i + j + 1 >= n {
        TriangleOrientation::Up
    } else {
        TriangleOrientation::Down
    };
    Some(TriangleAddress {
        base_face: face,
        i,
        j,
        n,
        orientation,
    })
}

/// The cap around a base face: its corners' normalized centroid and the
/// largest angle to a corner.
pub fn face_cap(address: TriangleAddress) -> Result<([f64; 3], f64), String> {
    let corners = face_corner_points(address)?;
    let sum = [
        corners[0][0] + corners[1][0] + corners[2][0],
        corners[0][1] + corners[1][1] + corners[2][1],
        corners[0][2] + corners[1][2] + corners[2][2],
    ];
    let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    let center = [sum[0] / length, sum[1] / length, sum[2] / length];
    let radius = corners
        .iter()
        .map(|&corner| arc(center, corner))
        .fold(0.0, f64::max);
    Ok((center, radius))
}

/// Longitude-latitude boxes, in degrees `[west, east, south, north]`, that
/// cover `faces`: each face's cap marked on 0.01-degree longitude bins and
/// read back as runs, so no box crosses the dateline, over the faces'
/// latitude range. The window a data layer is read in to serve a field on
/// these faces; a cap over a pole takes every longitude.
pub fn lon_lat_boxes(
    faces: impl IntoIterator<Item = TriangleAddress>,
) -> Result<Vec<[f64; 4]>, String> {
    const BINS: usize = 36_000;
    let width = 360.0 / BINS as f64;
    let mut marked = vec![false; BINS];
    let (mut south, mut north) = (90.0f64, -90.0f64);
    for face in faces {
        let (centre, radius) = face_cap(face)?;
        let lat = centre[2].clamp(-1.0, 1.0).asin();
        let lon = centre[1].atan2(centre[0]);
        south = south.min((lat - radius).to_degrees().max(-90.0));
        north = north.max((lat + radius).to_degrees().min(90.0));
        if lat.abs() + radius >= std::f64::consts::FRAC_PI_2 {
            marked.fill(true);
            continue;
        }
        // The cap's half-width in longitude at its centre's latitude.
        let half = (radius.sin() / lat.cos()).min(1.0).asin();
        let bin = |angle: f64| ((angle.to_degrees() + 180.0) / width).floor() as isize;
        for at in bin(lon - half)..=bin(lon + half) {
            marked[at.rem_euclid(BINS as isize) as usize] = true;
        }
    }
    let mut boxes = Vec::new();
    let mut at = 0;
    while at < BINS {
        if !marked[at] {
            at += 1;
            continue;
        }
        let start = at;
        while at < BINS && marked[at] {
            at += 1;
        }
        boxes.push([
            -180.0 + start as f64 * width,
            -180.0 + at as f64 * width,
            south,
            north,
        ]);
    }
    Ok(boxes)
}

/// A face's corners on the unit sphere, in lattice order -- `generate`'s
/// order, at `generate`'s positions -- so sums over them round as the built
/// grid's do.
pub fn face_corner_points(address: TriangleAddress) -> Result<[[f64; 3]; 3], String> {
    let mut corners = [[0.0; 3]; 3];
    for (corner, origin) in corners.iter_mut().zip(face_corner_origins(address)?) {
        let point = super::region::origin_position(address.n, origin)?;
        *corner = [point.x, point.y, point.z];
    }
    Ok(corners)
}

/// A bound on the longest edge of the level-n lattice (and so on every face's
/// cap radius): exact on small bases, scanned; on large ones, sampled where
/// it lies (`sampled_longest_lattice_edge`).
pub fn longest_edge(n: usize) -> Result<f64, String> {
    if n <= 300 {
        scanned_longest_lattice_edge(n)
    } else {
        sampled_longest_lattice_edge(n)
    }
}

fn longest_face_edge(address: TriangleAddress) -> Result<f64, String> {
    let corners = face_corner_points(address)?;
    Ok((0..3)
        .map(|side| arc(corners[side], corners[(side + 1) % 3]))
        .fold(0.0, f64::max))
}

fn lattice_faces_of(
    n: usize,
    base_face: u8,
    rows: std::ops::Range<usize>,
    columns: std::ops::Range<usize>,
) -> impl Iterator<Item = TriangleAddress> {
    rows.flat_map(move |i| {
        columns
            .clone()
            .filter(move |&j| i + j < n)
            .flat_map(move |j| {
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
}

fn scanned_longest_lattice_edge(n: usize) -> Result<f64, String> {
    let mut longest = 0.0f64;
    for base_face in 0..20u8 {
        for face in lattice_faces_of(n, base_face, 0..n, 0..n) {
            longest = longest.max(longest_face_edge(face)?);
        }
    }
    Ok(longest)
}

/// The twenty base faces are congruent, and the radial projection stretches
/// a planar lattice edge most where the plane is nearest the centre -- at a
/// base face's middle -- so the edges around base face 0's middle bound the
/// rest, with a hair of margin for the rounding that differs face to face.
fn sampled_longest_lattice_edge(n: usize) -> Result<f64, String> {
    let middle = n / 3;
    let window = middle.saturating_sub(3)..(middle + 4).min(n);
    let mut longest = 0.0f64;
    for face in lattice_faces_of(n, 0, window.clone(), window) {
        longest = longest.max(longest_face_edge(face)?);
    }
    Ok(longest * (1.0 + 1.0e-6))
}

fn arc(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        .clamp(-1.0, 1.0)
        .acos()
}

fn triple(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mother_grid::MotherGrid;
    use std::collections::BTreeMap;

    /// Every corner of the faces lies in a box; faces across the dateline
    /// give two boxes, each inside -180..180; a face at a pole takes every
    /// longitude.
    #[test]
    fn lon_lat_boxes_cover_their_faces() {
        let lon_lat = |point: [f64; 3]| {
            (
                point[1].atan2(point[0]).to_degrees(),
                point[2].clamp(-1.0, 1.0).asin().to_degrees(),
            )
        };
        let unit = |lon: f64, lat: f64| {
            let (lon, lat) = (f64::to_radians(lon), f64::to_radians(lat));
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
        };
        for (lon, lat, expected_boxes) in [(102.7, 25.0, 1), (179.95, -10.0, 2), (0.0, 89.9, 1)] {
            let n = 240;
            let centre = locate(n, unit(lon, lat)).unwrap();
            let mut faces = BTreeSet::from([centre]);
            faces.extend(faces_around(centre).unwrap());
            let boxes = lon_lat_boxes(faces.iter().copied()).unwrap();
            assert_eq!(boxes.len(), expected_boxes, "{boxes:?}");
            for b in &boxes {
                assert!(
                    -180.0 <= b[0] && b[0] < b[1] && b[1] <= 180.0 && b[2] < b[3],
                    "{b:?}"
                );
            }
            for &face in &faces {
                for corner in face_corner_points(face).unwrap() {
                    let (lon, lat) = lon_lat(corner);
                    assert!(
                        boxes
                            .iter()
                            .any(|b| b[0] <= lon && lon <= b[1] && b[2] <= lat && lat <= b[3]),
                        "corner {lon},{lat} outside {boxes:?}"
                    );
                }
            }
            if lat > 89.0 {
                assert_eq!(boxes[0][0..2], [-180.0, 180.0]);
            }
        }
    }

    /// Faces around every vertex and every face's edge neighbours agree with
    /// the whole grid's incidence.
    #[test]
    fn adjacency_by_address_is_the_grids() {
        for n in [1, 2, 3, 5] {
            let grid = MotherGrid::generate(n).unwrap();
            let mut at_vertex = BTreeMap::<usize, BTreeSet<TriangleAddress>>::new();
            for face in grid.mesh.active_triangle_slots() {
                for site in grid.mesh.triangles()[face] {
                    at_vertex
                        .entry(site)
                        .or_default()
                        .insert(grid.triangle_addresses[face].unwrap());
                }
            }
            for (site, expected) in at_vertex {
                let address = grid.addresses[site].clone().unwrap();
                let origin = (0..20u8)
                    .flat_map(|face| {
                        (0..=n).flat_map(move |i| (0..=n - i).map(move |j| (face, i, j)))
                    })
                    .map(|(face, i, j)| vertex_origin(n, face, i, j).unwrap())
                    .find(|&origin| origin_address(n, origin) == address)
                    .unwrap();
                let found = faces_at_vertex(n, origin)
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                assert_eq!(found, expected, "n {n} vertex {address:?}");
            }
            for face in grid.mesh.active_triangle_slots() {
                let address = grid.triangle_addresses[face].unwrap();
                let expected = grid.mesh.neighbours()[face]
                    .iter()
                    .map(|&neighbour| grid.triangle_addresses[neighbour].unwrap())
                    .collect::<BTreeSet<_>>();
                let found = edge_neighbours(address)
                    .unwrap()
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                assert_eq!(found, expected, "n {n} face {address:?}");
                let across = grid.mesh.neighbours()[face]
                    .map(|neighbour| grid.triangle_addresses[neighbour].unwrap());
                assert_eq!(
                    faces_across(address).unwrap(),
                    across,
                    "n {n} face {address:?}"
                );
            }
        }
    }

    /// Points inside a face are located in it.
    #[test]
    fn points_are_located_in_their_faces() {
        for n in [1, 2, 4, 7, 40] {
            let grid = MotherGrid::generate(n).unwrap();
            for face in grid.mesh.active_triangle_slots().step_by(7) {
                let corners = grid.mesh.triangles()[face].map(|site| {
                    let point = grid.mesh.vertices()[site];
                    [point.x, point.y, point.z]
                });
                for weights in [[1.0, 1.0, 1.0], [3.0, 1.0, 1.0], [1.0, 4.0, 2.0]] {
                    let mut point = [0.0; 3];
                    for (corner, weight) in corners.iter().zip(weights) {
                        for axis in 0..3 {
                            point[axis] += corner[axis] * weight;
                        }
                    }
                    let length =
                        (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
                    let point = point.map(|value| value / length);
                    assert_eq!(
                        locate(n, point),
                        grid.triangle_addresses[face],
                        "n {n} face {face} weights {weights:?}"
                    );
                }
            }
        }
    }

    /// The sampled bound on the longest lattice edge holds, and is tight.
    #[test]
    fn the_sampled_longest_edge_bounds_every_edge() {
        for n in [1, 2, 5, 12, 40, 128, 301] {
            let scanned = scanned_longest_lattice_edge(n).unwrap();
            let sampled = sampled_longest_lattice_edge(n).unwrap();
            assert!(
                sampled >= scanned && sampled <= scanned * (1.0 + 2.0e-6),
                "n {n}: sampled {sampled} scanned {scanned}"
            );
        }
    }

    /// A lattice point lies on a coarser face's side exactly when it is one
    /// of that side's lattice points: every point of every side, seen from
    /// every face that has it, and no other point of the face.
    #[test]
    fn sides_hold_their_lattice_points() {
        for (coarse, levels) in [(1, 2), (2, 1), (3, 2)] {
            let n = coarse << levels;
            let ratio = n / coarse;
            let grid = MotherGrid::generate(coarse).unwrap();
            for face in grid.triangle_addresses.iter().flatten().copied() {
                let corners = super::super::region::face_lattice_corners(face)
                    .map(|(i, j)| (i * ratio, j * ratio));
                for side in 0..3 {
                    let (start, end) = (corners[(side + 1) % 3], corners[(side + 2) % 3]);
                    for step in 0..=ratio {
                        let i = (start.0 * (ratio - step) + end.0 * step) / ratio;
                        let j = (start.1 * (ratio - step) + end.1 * step) / ratio;
                        let origin = vertex_origin(n, face.base_face, i, j).unwrap();
                        assert!(
                            sides_holding(face, n, origin).unwrap()[side],
                            "{face:?} side {side} step {step}"
                        );
                    }
                }
                // Points strictly inside hold no side.
                for i in 0..=n {
                    for j in 0..=n - i {
                        let (a, b) = (corners[0], corners[1]);
                        let c = corners[2];
                        let inside = {
                            let area = |p: (usize, usize), q: (usize, usize), r: (usize, usize)| {
                                (q.0 as i64 - p.0 as i64) * (r.1 as i64 - p.1 as i64)
                                    - (q.1 as i64 - p.1 as i64) * (r.0 as i64 - p.0 as i64)
                            };
                            let total = area(a, b, c);
                            let w = [area((i, j), b, c), area(a, (i, j), c), area(a, b, (i, j))];
                            w.iter().all(|&w| w.signum() == total.signum() && w != 0)
                        };
                        if inside {
                            let origin = vertex_origin(n, face.base_face, i, j).unwrap();
                            assert_eq!(sides_holding(face, n, origin).unwrap(), [false; 3]);
                        }
                    }
                }
            }
        }
    }
}
