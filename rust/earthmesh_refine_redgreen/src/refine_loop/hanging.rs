//! Closing hanging vertices a classic round left behind.
//!
//! Two refined blocks close enough for their transition rows to meet can each
//! split an edge of the same triangle. The rows split that triangle once, on
//! one edge, so the other edge's new vertex hangs on it -- and the triangle
//! across the first edge, cut by the other block's row on a different edge,
//! keeps the first edge whole. Either way an edge is used by one triangle on
//! one side and by two half-edges through a new vertex on the other: the mesh
//! has a hole the width of a zero-area sliver. A global slope run at 100 km
//! left ten such edges in western Tibet, where the steep supports sit a block
//! apart.
//!
//! The closure is the green split itself: a triangle `(a, b, c)` whose edge
//! `ab` carries a hanging vertex `m` becomes `(a, m, c)` and `(m, b, c)`. It
//! repeats until no open edge has a vertex to close it; the angle repair that
//! follows every round takes the new triangles into its window.

use std::collections::HashMap;

use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, xyz_points_to_lonlat_degrees, CartesianPoint};

use super::RedGreenMesh;

/// Split every triangle with a hanging vertex on an edge, until none is left
/// or none can be closed. Returns the number of splits; the new triangles are
/// appended and inherit their parent's refinement depth.
pub fn close_hanging_vertices(mesh: &mut RedGreenMesh) -> usize {
    let mut splits = 0;
    // A sweep splits every triangle with a hanging vertex once; a triangle
    // with two takes two sweeps. Each split closes a hanging vertex and opens
    // none, so the sweeps end; the bound guards a misread geometry test.
    for _ in 0..8 {
        let found = find_hanging(mesh);
        if found.is_empty() {
            break;
        }
        for (triangle, slot, hanging) in found {
            split(mesh, triangle, slot, hanging);
            splits += 1;
        }
    }
    splits
}

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn unit(mesh: &RedGreenMesh, cell: usize) -> CartesianPoint {
    lonlat_degrees_to_unit_xyz(mesh.cell_points[cell])
}

fn angle(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let dot = (a.x * b.x + a.y * b.y + a.z * b.z).clamp(-1.0, 1.0);
    dot.acos()
}

/// Each triangle with a hanging vertex, once: the slot `k` of its open edge
/// `(v[k], v[k + 1])` and the vertex hanging on that edge.
fn find_hanging(mesh: &RedGreenMesh) -> Vec<(usize, usize, usize)> {
    let mut uses: HashMap<(usize, usize), usize> = HashMap::new();
    for corners in mesh.cells_on_triangle.iter().skip(2) {
        for k in 0..3 {
            *uses
                .entry(edge(corners[k], corners[(k + 1) % 3]))
                .or_default() += 1;
        }
    }
    // Vertices joined to each vertex by an open edge.
    let mut open_neighbours: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&(a, b), &count) in &uses {
        if count == 1 {
            open_neighbours.entry(a).or_default().push(b);
            open_neighbours.entry(b).or_default().push(a);
        }
    }
    let mut found = Vec::new();
    if open_neighbours.is_empty() {
        return found;
    }
    for (triangle, corners) in mesh.cells_on_triangle.iter().enumerate().skip(2) {
        for k in 0..3 {
            let (a, b) = (corners[k], corners[(k + 1) % 3]);
            if uses.get(&edge(a, b)) != Some(&1) {
                continue;
            }
            let (Some(from_a), Some(from_b)) = (open_neighbours.get(&a), open_neighbours.get(&b))
            else {
                continue;
            };
            let (pa, pb) = (unit(mesh, a), unit(mesh, b));
            let length = angle(pa, pb);
            // On the edge: the detour through `m` is no longer than the edge.
            let hanging = from_a
                .iter()
                .filter(|&&m| m != b && m != corners[(k + 2) % 3] && from_b.contains(&m))
                .copied()
                .find(|&m| {
                    let pm = unit(mesh, m);
                    angle(pa, pm) + angle(pm, pb) <= length * (1.0 + 1.0e-6)
                });
            if let Some(m) = hanging {
                found.push((triangle, k, m));
                break;
            }
        }
    }
    found
}

fn centre(mesh: &RedGreenMesh, corners: [usize; 3]) -> earthmesh_mesh::LonLatDegrees {
    let [a, b, c] = corners.map(|cell| unit(mesh, cell));
    let (x, y, z) = (a.x + b.x + c.x, a.y + b.y + c.y, a.z + b.z + c.z);
    let length = (x * x + y * y + z * z).sqrt();
    xyz_points_to_lonlat_degrees(&[CartesianPoint {
        x: x / length,
        y: y / length,
        z: z / length,
    }])[0]
}

fn split(mesh: &mut RedGreenMesh, triangle: usize, slot: usize, m: usize) {
    let corners = mesh.cells_on_triangle[triangle];
    let (a, b, c) = (
        corners[slot],
        corners[(slot + 1) % 3],
        corners[(slot + 2) % 3],
    );
    // Same orientation as the parent: a -> m -> c and m -> b -> c.
    let first = [a, m, c];
    let second = [m, b, c];
    let added = mesh.cells_on_triangle.len();
    mesh.cells_on_triangle[triangle] = first;
    mesh.cells_on_triangle.push(second);
    mesh.triangle_points[triangle] = centre(mesh, first);
    let second_centre = centre(mesh, second);
    if mesh.triangle_points.len() == added {
        mesh.triangle_points.push(second_centre);
    } else {
        mesh.triangle_points.resize(added + 1, second_centre);
        mesh.triangle_points[added] = second_centre;
    }
    if !mesh.refinement_levels.is_empty() {
        let depth = mesh.refinement_levels.get(triangle).copied().unwrap_or(0);
        mesh.refinement_levels.resize(added + 1, depth);
        mesh.refinement_levels[added] = depth;
    }
    // `b` leaves the parent's row for the new triangle's; `m` and `c` gain it.
    let rows = &mut mesh.triangles_on_cell;
    if let Some(row) = rows.get_mut(b) {
        for entry in row.iter_mut() {
            if *entry == triangle {
                *entry = added;
            }
        }
    }
    for cell in [m, c] {
        rows[cell].push(added);
        mesh.n_triangles_on_cell[cell] += 1;
    }
    rows[m].push(triangle);
    mesh.n_triangles_on_cell[m] += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_mesh::LonLatDegrees;

    /// Two triangles sharing edge 2-3, one of them already cut at its
    /// midpoint 5 into two halves: the whole triangle across is closed.
    #[test]
    fn a_vertex_hanging_on_a_whole_edge_splits_the_triangle_across() {
        let p = |lon: f64, lat: f64| LonLatDegrees::new(lon, lat);
        // Cells 1.. (0 placeholder): 2 and 3 share the edge, 4 is the far
        // corner on the whole side, 6 on the cut side, 5 the midpoint.
        let cell_points = vec![
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(0.5, 1.0),
            p(0.5, 0.0),
            p(0.5, -1.0),
        ];
        // Triangles 0 and 1 are placeholders; 3 and 4 are the cut side.
        let cells_on_triangle = vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [3, 5, 6], [5, 2, 6]];
        let mut triangles_on_cell = vec![Vec::new(); 7];
        for (triangle, corners) in cells_on_triangle.iter().enumerate().skip(2) {
            for &cell in corners {
                triangles_on_cell[cell].push(triangle);
            }
        }
        let n_triangles_on_cell = triangles_on_cell.iter().map(Vec::len).collect();
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![p(0.0, 0.0); 5],
            cell_points,
            cells_on_triangle,
            triangles_on_cell,
            n_triangles_on_cell,
            green_parents: Vec::new(),
            refinement_levels: vec![0, 0, 1, 1, 1],
        };
        assert_eq!(close_hanging_vertices(&mut mesh), 1);
        assert_eq!(mesh.cells_on_triangle[2], [2, 5, 4]);
        assert_eq!(mesh.cells_on_triangle[5], [5, 3, 4]);
        assert_eq!(mesh.refinement_levels[5], 1);
        // Every edge is now used twice or lies on the outside of this patch.
        let mut uses: HashMap<(usize, usize), usize> = HashMap::new();
        for corners in mesh.cells_on_triangle.iter().skip(2) {
            for k in 0..3 {
                *uses
                    .entry(edge(corners[k], corners[(k + 1) % 3]))
                    .or_default() += 1;
            }
        }
        assert_eq!(uses[&edge(2, 5)], 2);
        assert_eq!(uses[&edge(5, 3)], 2);
        assert!(!uses.contains_key(&edge(2, 3)));
        assert_eq!(mesh.n_triangles_on_cell[5], 4);
        assert!(mesh.triangles_on_cell[3].contains(&5));
        assert!(!mesh.triangles_on_cell[3].contains(&2));
        assert_eq!(
            close_hanging_vertices(&mut mesh),
            0,
            "nothing hangs any more"
        );
    }
}
