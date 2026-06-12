use earthmesh_mesh::{
    connect_on_cell_fortran_indexed, order_vertices_on_cell_fortran_indexed,
    standardize_vertices_on_cell_rotation_fortran_indexed, CartesianPoint,
};

#[test]
fn standardize_vertices_on_cell_rotation_starts_each_cell_with_min_positive_vertex() {
    let vertices_on_cell = vec![
        vec![],
        vec![9, 8, 7],
        vec![7, 3, 5, 99],
        vec![0, 8, 6, 7],
        vec![4, 9, 2, 8],
    ];
    let edge_counts = vec![0, 3, 3, 4, 0];

    let standardized =
        standardize_vertices_on_cell_rotation_fortran_indexed(&vertices_on_cell, &edge_counts)
            .expect("valid verticesOnCell inputs");

    assert_eq!(standardized[0], vertices_on_cell[0]);
    assert_eq!(standardized[1], vertices_on_cell[1]);
    assert_eq!(standardized[2], vec![3, 5, 7, 99]);
    assert_eq!(standardized[3], vec![6, 7, 0, 8]);
    assert_eq!(standardized[4], vertices_on_cell[4]);
}

#[test]
fn standardize_vertices_on_cell_rotation_rejects_short_edge_counts() {
    let vertices_on_cell = vec![vec![], vec![], vec![7, 3, 5]];
    let edge_counts = vec![0, 0];

    assert!(
        standardize_vertices_on_cell_rotation_fortran_indexed(&vertices_on_cell, &edge_counts)
            .is_none()
    );
}

#[test]
fn connect_on_cell_rebuilds_edges_and_neighbor_cells_from_ordered_vertices() {
    let n_edges_on_cell = vec![0, 0, 3];
    let vertices_on_cell = vec![vec![], vec![], vec![2, 3, 4]];
    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [5, 7, 9], [5, 6, 0], [6, 7, 0]];
    let cells_on_edge = vec![
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
        [2, 10],
        [2, 11],
        [12, 2],
    ];

    let output = connect_on_cell_fortran_indexed(
        &n_edges_on_cell,
        &cells_on_edge,
        &edges_on_vertex,
        &vertices_on_cell,
    )
    .expect("valid ordered cell connectivity");

    assert_eq!(output.edges_on_cell[2], vec![5, 6, 7]);
    assert_eq!(output.cells_on_cell[2], vec![10, 11, 12]);
}

#[test]
fn connect_on_cell_rejects_vertex_pair_without_common_edge() {
    let n_edges_on_cell = vec![0, 0, 3];
    let vertices_on_cell = vec![vec![], vec![], vec![2, 3, 4]];
    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [5, 7, 9], [6, 8, 0], [6, 7, 0]];
    let cells_on_edge = vec![[0, 0]; 10];

    assert!(connect_on_cell_fortran_indexed(
        &n_edges_on_cell,
        &cells_on_edge,
        &edges_on_vertex,
        &vertices_on_cell,
    )
    .is_none());
}

#[test]
fn order_vertices_on_cell_sorts_remaining_vertices_ccw_from_first_vertex() {
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
    ];
    let vertex_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 1.0),
        CartesianPoint::new(0.0, 1.0, 1.0),
        CartesianPoint::new(-1.0, 0.0, 1.0),
        CartesianPoint::new(0.0, -1.0, 1.0),
    ];
    let vertices_on_cell = vec![vec![], vec![], vec![2, 4, 3, 5]];
    let n_edges_on_cell = vec![0, 0, 4];

    let ordered = order_vertices_on_cell_fortran_indexed(
        &cell_points,
        &vertex_points,
        &vertices_on_cell,
        &n_edges_on_cell,
    )
    .expect("valid verticesOnCell ordering inputs");

    assert_eq!(ordered[2], vec![2, 3, 4, 5]);
}

#[test]
fn order_vertices_on_cell_rejects_missing_vertex_coordinates() {
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
    ];
    let vertex_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); 4];
    let vertices_on_cell = vec![vec![], vec![], vec![2, 3, 4]];
    let n_edges_on_cell = vec![0, 0, 3];

    assert!(order_vertices_on_cell_fortran_indexed(
        &cell_points,
        &vertex_points,
        &vertices_on_cell,
        &n_edges_on_cell,
    )
    .is_none());
}
