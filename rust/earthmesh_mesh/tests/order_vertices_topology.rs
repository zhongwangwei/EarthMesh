use earthmesh_mesh::{order_vertices_on_cell_by_shared_edges_fortran_indexed, CartesianPoint};

/// Hexagon fixture: cell 2's center on the +z axis, its six ring vertices on a
/// small circle around it. `angles_deg[k]` places vertex id `(k + 1) * 10`;
/// increasing angle = counterclockwise seen from outside (+z above).
fn hexagon_points(angles_deg: [f64; 6]) -> (Vec<CartesianPoint>, Vec<CartesianPoint>) {
    let mut vertex_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); 61];
    for (k, angle) in angles_deg.iter().enumerate() {
        let theta = angle.to_radians();
        vertex_points[(k + 1) * 10] =
            CartesianPoint::new(0.3 * theta.cos(), 0.3 * theta.sin(), 0.95);
    }
    let mut cell_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); 3];
    cell_points[2] = CartesianPoint::new(0.0, 0.0, 1.0);
    (vertex_points, cell_points)
}

fn hexagon_edges() -> Vec<[usize; 3]> {
    let mut edges_on_vertex = vec![[0usize; 3]; 61];
    edges_on_vertex[10] = [1, 6, 0];
    edges_on_vertex[20] = [1, 2, 0];
    edges_on_vertex[30] = [2, 3, 0];
    edges_on_vertex[40] = [3, 4, 0];
    edges_on_vertex[50] = [4, 5, 0];
    edges_on_vertex[60] = [5, 6, 0];
    edges_on_vertex
}

#[test]
fn topology_order_restores_cycle_when_last_two_vertices_are_swapped() {
    let edges_on_vertex = hexagon_edges();
    let vertices_on_cell = vec![vec![], vec![], vec![10, 20, 30, 40, 60, 50, 1]];
    let n_edges_on_cell = vec![0, 0, 6];
    // Geometry agrees with the walk direction 10 -> 20 -> ... -> 60 (CCW).
    let (vertex_points, cell_points) =
        hexagon_points([0.0, 60.0, 120.0, 180.0, 240.0, 300.0]);

    let ordered = order_vertices_on_cell_by_shared_edges_fortran_indexed(
        &vertices_on_cell,
        &n_edges_on_cell,
        &edges_on_vertex,
        &vertex_points,
        &cell_points,
    )
    .expect("topological order");

    assert_eq!(ordered[2], vec![10, 20, 30, 40, 50, 60, 1]);
}

#[test]
fn topology_order_reverses_clockwise_walk_to_ccw() {
    let edges_on_vertex = hexagon_edges();
    let vertices_on_cell = vec![vec![], vec![], vec![10, 20, 30, 40, 60, 50, 1]];
    let n_edges_on_cell = vec![0, 0, 6];
    // Mirror the geometry: the ID-ordered walk 10 -> 20 -> ... -> 60 is now
    // clockwise seen from outside, so the orderer must flip it (keeping the
    // deterministic min-id start vertex in slot 0).
    let (vertex_points, cell_points) =
        hexagon_points([0.0, -60.0, -120.0, -180.0, -240.0, -300.0]);

    let ordered = order_vertices_on_cell_by_shared_edges_fortran_indexed(
        &vertices_on_cell,
        &n_edges_on_cell,
        &edges_on_vertex,
        &vertex_points,
        &cell_points,
    )
    .expect("topological order");

    assert_eq!(ordered[2], vec![10, 60, 50, 40, 30, 20, 1]);
}
