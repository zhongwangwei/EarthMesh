use earthmesh_mesh::{
    arc_length_unit_sphere, connect_on_cell_one_based, edge_distance_angle_one_based,
    edge_id_sort_one_based, lonlat_degrees_to_unit_xyz, order_vertices_on_cell_one_based,
    plane_angle_signed, set_dbx_move_regional_step_one_based, set_weights_on_edge_one_based,
    spring_apply_cell_displacements_one_based, spring_dynamics_global_one_based,
    spring_dynamics_regional_one_based, spring_edge_adjustment_canonical,
    spring_edge_directions_one_based, spring_global_iteration_one_based,
    standardize_vertices_on_cell_rotation_one_based, CartesianPoint, LonLatDegrees,
    RegionalMoveMaskInput,
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
        standardize_vertices_on_cell_rotation_one_based(&vertices_on_cell, &edge_counts)
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
        standardize_vertices_on_cell_rotation_one_based(&vertices_on_cell, &edge_counts).is_none()
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

    let output = connect_on_cell_one_based(
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

    assert!(connect_on_cell_one_based(
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

    let ordered = order_vertices_on_cell_one_based(
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

    assert!(order_vertices_on_cell_one_based(
        &cell_points,
        &vertex_points,
        &vertices_on_cell,
        &n_edges_on_cell,
    )
    .is_none());
}

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

fn magnitude_for_test(point: CartesianPoint) -> f64 {
    (point.x * point.x + point.y * point.y + point.z * point.z).sqrt()
}

#[test]
fn plane_angle_signed_matches_canonical_normal_sign_rule() {
    let origin = CartesianPoint::new(0.0, 0.0, 0.0);
    let east = CartesianPoint::new(1.0, 0.0, 0.0);
    let north = CartesianPoint::new(0.0, 1.0, 0.0);
    let up = CartesianPoint::new(0.0, 0.0, 1.0);

    approx_eq(
        plane_angle_signed(origin, east, north, up).expect("angle"),
        std::f64::consts::FRAC_PI_2,
        1.0e-15,
    );
    approx_eq(
        plane_angle_signed(origin, north, east, up).expect("angle"),
        -std::f64::consts::FRAC_PI_2,
        1.0e-15,
    );
}

#[test]
fn spring_edge_adjustment_matches_canonical_ratio_and_displacement_formula() {
    let point_a = CartesianPoint::new(0.0, 0.0, 0.0);
    let point_b = CartesianPoint::new(10.0, 0.0, 0.0);

    let adjustment = spring_edge_adjustment_canonical(point_a, point_b, 12.0, 8.0, 9.0, 7.0, 6.0)
        .expect("valid spring edge adjustment");

    // twocosphi3 = (8^2 + 9^2 - 10^2)/(8*9) = 45/72
    // twocosphi4 = (7^2 + 6^2 - 10^2)/(7*6) = -15/42
    // ratio = 0.625 - 0.35714285714285715 = 0.26785714285714285
    // distm = 12 / 1.2 * ratio = 2.6785714285714284
    // frac = (distm - 10) / 10 = -0.7321428571428572
    approx_eq(adjustment.distance, 10.0, 1.0e-15);
    approx_eq(adjustment.ratio, 0.26785714285714285, 1.0e-15);
    approx_eq(adjustment.target_distance, 2.6785714285714284, 1.0e-15);
    approx_eq(adjustment.frac_change, -0.7321428571428572, 1.0e-15);
    approx_eq(adjustment.displacement.x, -7.321428571428572, 1.0e-15);
    approx_eq(adjustment.displacement.y, 0.0, 1.0e-15);
    approx_eq(adjustment.displacement.z, 0.0, 1.0e-15);
    approx_eq(
        adjustment.frac_change_squared,
        adjustment.frac_change * adjustment.frac_change,
        1.0e-15,
    );
}

#[test]
fn spring_edge_adjustment_truncates_edge_vector_to_default_real_like_canonical() {
    let point_a = CartesianPoint::new(6_371_000.123_456_789, -12.345_678_901, 99.000_000_1);
    let point_b = CartesianPoint::new(6_371_017.987_654_321, 27.891_234_567, 101.000_000_9);

    let adjustment =
        spring_edge_adjustment_canonical(point_a, point_b, 64.0, 31.25, 29.75, 33.5, 28.125)
            .expect("valid spring edge adjustment");

    // Canonical's dx/dy/dz assignment uses REAL(...) without a kind argument:
    //   dx(iu) = real(xew8(iw2) - xew8(iw1))
    // so each edge-vector component is rounded to default real before the
    // distance and displacement are calculated.
    let dx = (point_b.x - point_a.x) as f32 as f64;
    let dy = (point_b.y - point_a.y) as f32 as f64;
    let dz = (point_b.z - point_a.z) as f32 as f64;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    let twocosphi3 = (31.25_f64.powi(2) + 29.75_f64.powi(2) - distance.powi(2)) / (31.25 * 29.75);
    let twocosphi4 = (33.5_f64.powi(2) + 28.125_f64.powi(2) - distance.powi(2)) / (33.5 * 28.125);
    let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
    let target_distance = 64.0 / 1.2 * ratio;
    let frac_change = (target_distance - distance) / distance;

    approx_eq(adjustment.distance, distance, 1.0e-12);
    approx_eq(adjustment.displacement.x, dx * frac_change, 1.0e-12);
    approx_eq(adjustment.displacement.y, dy * frac_change, 1.0e-12);
    approx_eq(adjustment.displacement.z, dz * frac_change, 1.0e-12);
}

#[test]
fn spring_edge_directions_match_canonical_cell_side_sign_rule() {
    let n_edges_on_cell = vec![0, 0, 2, 2];
    let edges_on_cell = vec![vec![], vec![], vec![2, 3], vec![2, 4]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3], [4, 2], [3, 5]];

    let directions =
        spring_edge_directions_one_based(&n_edges_on_cell, &edges_on_cell, &cells_on_edge, 0.25)
            .expect("valid spring direction inputs");

    assert_eq!(directions[0], Vec::<f64>::new());
    assert_eq!(directions[1], Vec::<f64>::new());
    assert_eq!(directions[2], vec![-0.25, 0.25]);
    assert_eq!(directions[3], vec![0.25, -0.25]);
}

#[test]
fn spring_apply_cell_displacements_accumulates_and_renormalizes_like_canonical() {
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
    ];
    let n_edges_on_cell = vec![0, 0, 2, 0];
    let edges_on_cell = vec![vec![], vec![], vec![2, 3], vec![]];
    let directions = vec![vec![], vec![], vec![0.5, -0.25], vec![]];
    let edge_displacements = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 2.0, 0.0),
    ];

    let updated = spring_apply_cell_displacements_one_based(
        &cell_points,
        &n_edges_on_cell,
        &edges_on_cell,
        &directions,
        &edge_displacements,
        1.0,
    )
    .expect("valid spring displacement inputs");

    // Cell 2 raw update: (1,0,0) + 0.5*(1,0,0) - 0.25*(0,2,0) = (1.5,-0.5,0),
    // then Canonical scales it back onto the sphere.
    approx_eq(updated[2].x, 0.9486832980505138, 1.0e-15);
    approx_eq(updated[2].y, -0.31622776601683794, 1.0e-15);
    approx_eq(updated[2].z, 0.0, 1.0e-15);
    approx_eq(magnitude_for_test(updated[2]), 1.0, 1.0e-15);
    assert_eq!(updated[3], cell_points[3]);
}

#[test]
fn spring_global_iteration_wrapper_matches_manual_helper_composition() {
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
    ];
    let n_edges_on_cell = vec![0, 0, 2, 1, 1];
    let edges_on_cell = vec![vec![], vec![], vec![2, 3], vec![2], vec![3]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3], [4, 2]];
    let edges_on_edge_tri = vec![[0, 0, 0, 0], [0, 0, 0, 0], [2, 2, 2, 2], [3, 3, 3, 3]];
    let dists_on_edge = vec![0.0, 0.0, 2.0, 2.0];

    let output = spring_global_iteration_one_based(
        &cell_points,
        &n_edges_on_cell,
        &edges_on_cell,
        &cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        0.25,
        1.0,
    )
    .expect("valid spring global iteration inputs");

    let adjustment2 = spring_edge_adjustment_canonical(
        cell_points[2],
        cell_points[3],
        dists_on_edge[2],
        std::f64::consts::SQRT_2,
        std::f64::consts::SQRT_2,
        std::f64::consts::SQRT_2,
        std::f64::consts::SQRT_2,
    )
    .expect("edge 2 adjustment");
    let adjustment3 = spring_edge_adjustment_canonical(
        cell_points[4],
        cell_points[2],
        dists_on_edge[3],
        std::f64::consts::SQRT_2,
        std::f64::consts::SQRT_2,
        std::f64::consts::SQRT_2,
        std::f64::consts::SQRT_2,
    )
    .expect("edge 3 adjustment");
    let edge_displacements = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        adjustment2.displacement,
        adjustment3.displacement,
    ];
    let directions =
        spring_edge_directions_one_based(&n_edges_on_cell, &edges_on_cell, &cells_on_edge, 0.25)
            .expect("directions");
    let expected = spring_apply_cell_displacements_one_based(
        &cell_points,
        &n_edges_on_cell,
        &edges_on_cell,
        &directions,
        &edge_displacements,
        1.0,
    )
    .expect("manual application");

    assert_eq!(output.updated_cell_points, expected);
    assert_eq!(output.edge_displacements, edge_displacements);
    approx_eq(
        output.frac_change_squared[2],
        adjustment2.frac_change_squared,
        1.0e-15,
    );
    approx_eq(
        output.frac_change_squared[3],
        adjustment3.frac_change_squared,
        1.0e-15,
    );
}

#[test]
fn spring_dynamics_global_wrapper_repeats_single_iteration_updates() {
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
    ];
    let n_edges_on_cell = vec![0, 0, 2, 1, 1];
    let edges_on_cell = vec![vec![], vec![], vec![2, 3], vec![2], vec![3]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3], [4, 2]];
    let edges_on_edge_tri = vec![[0, 0, 0, 0], [0, 0, 0, 0], [2, 2, 2, 2], [3, 3, 3, 3]];
    let dists_on_edge = vec![0.0, 0.0, 2.0, 2.0];

    let output = spring_dynamics_global_one_based(
        &cell_points,
        &n_edges_on_cell,
        &edges_on_cell,
        &cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        2,
        0.25,
        1.0,
        100,
    )
    .expect("valid spring dynamics inputs");

    let first = spring_global_iteration_one_based(
        &cell_points,
        &n_edges_on_cell,
        &edges_on_cell,
        &cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        0.25,
        1.0,
    )
    .expect("first iteration");
    let second = spring_global_iteration_one_based(
        &first.updated_cell_points,
        &n_edges_on_cell,
        &edges_on_cell,
        &cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        0.25,
        1.0,
    )
    .expect("second iteration");

    assert_eq!(output.updated_cell_points, second.updated_cell_points);
    assert_eq!(output.last_edge_displacements, second.edge_displacements);
    assert_eq!(output.last_frac_change_squared, second.frac_change_squared);
    assert_eq!(output.diagnostic_max_displacements.len(), 1);
    assert_eq!(output.diagnostic_max_displacements[0].iteration, 1);
    approx_eq(
        output.diagnostic_max_displacements[0].max_displacement,
        first
            .updated_cell_points
            .iter()
            .zip(cell_points.iter())
            .map(|(updated, previous)| {
                magnitude_for_test(CartesianPoint::new(
                    updated.x - previous.x,
                    updated.y - previous.y,
                    updated.z - previous.z,
                ))
            })
            .fold(0.0, f64::max),
        1.0e-15,
    );
}

#[test]
fn spring_dynamics_regional_wrapper_moves_only_masked_cells_by_neighbor_average() {
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(1.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 1.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 1.0),
    ];
    let n_edges_on_cell = vec![0, 0, 2, 0, 0];
    let cells_on_cell = vec![vec![], vec![], vec![3, 4], vec![], vec![]];
    let move_mask = vec![false, false, true, false, false];

    let output = spring_dynamics_regional_one_based(
        &cell_points,
        &n_edges_on_cell,
        &cells_on_cell,
        &move_mask,
        1,
        1.0,
        10,
    )
    .expect("valid regional spring inputs");

    assert_eq!(output.calculated_cells, vec![2, 3, 4]);
    assert_eq!(output.moved_cells, vec![2]);
    assert_eq!(output.updated_cell_points[0], cell_points[0]);
    assert_eq!(output.updated_cell_points[1], cell_points[1]);
    assert_eq!(output.updated_cell_points[3], cell_points[3]);
    assert_eq!(output.updated_cell_points[4], cell_points[4]);
    approx_eq(output.updated_cell_points[2].x, 0.0, 1.0e-15);
    approx_eq(
        output.updated_cell_points[2].y,
        std::f64::consts::FRAC_1_SQRT_2,
        1.0e-15,
    );
    approx_eq(
        output.updated_cell_points[2].z,
        std::f64::consts::FRAC_1_SQRT_2,
        1.0e-15,
    );
    assert_eq!(output.diagnostic_max_displacements.len(), 1);
    assert_eq!(output.diagnostic_max_displacements[0].iteration, 1);
    approx_eq(
        output.diagnostic_max_displacements[0].max_displacement,
        magnitude_for_test(CartesianPoint::new(
            output.updated_cell_points[2].x - cell_points[2].x,
            output.updated_cell_points[2].y - cell_points[2].y,
            output.updated_cell_points[2].z - cell_points[2].z,
        )),
        1.0e-15,
    );
}

#[test]
fn set_dbx_move_regional_step_masks_refined_cells_but_freezes_boundary_cells() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 4, 0], [2, 3, 5], [3, 4, 0]];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3], vec![3, 4], vec![2, 4], vec![3]];
    let n_edges_on_cell = vec![0, 0, 2, 2, 2, 1];
    let refined_triangles = vec![false, false, false, true, false];

    let output = set_dbx_move_regional_step_one_based(RegionalMoveMaskInput {
        set_dis: 0,
        refined_triangles: &refined_triangles,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        protected_seed_cells: &[],
        vertex_protect_layers: 0,
    })
    .expect("valid regional move-mask input");

    assert!(output.expanded_refined_triangles[3]);
    assert!(output.boundary_mask[2]);
    assert!(output.boundary_mask[3]);
    assert!(!output.move_mask[2]);
    assert!(!output.move_mask[3]);
    assert!(output.move_mask[5]);
}

#[test]
fn set_dbx_move_regional_step_protects_seed_vertex_cells_when_refinement_touches_them() {
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 4, 0], [2, 3, 5], [3, 4, 0]];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3], vec![3, 4], vec![2, 4], vec![3]];
    let n_edges_on_cell = vec![0, 0, 2, 2, 2, 1];
    let refined_triangles = vec![false, false, false, true, false];

    let output = set_dbx_move_regional_step_one_based(RegionalMoveMaskInput {
        set_dis: 0,
        refined_triangles: &refined_triangles,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        protected_seed_cells: &[5],
        vertex_protect_layers: 1,
    })
    .expect("valid protected regional move-mask input");

    assert!(output.protected_triangles[3]);
    assert!(!output.move_mask[5]);
}

#[test]
fn edge_distance_angle_matches_canonical_meridian_edge_case() {
    let vertex1 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let vertex2 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let cell1 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(-1.0, 0.5));
    let cell2 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.5));
    let edge = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.5));

    let vertices = vec![vertex1, vertex1, vertex1, vertex2];
    let cells = vec![cell1, cell1, cell1, cell2];
    let edge_points = vec![edge, edge, edge];
    let vertices_on_edge = vec![[0, 0], [0, 0], [2, 3]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3]];
    let lat_vertex = vec![0.0, 0.0, 0.0, 1.0];
    let lon_edge = vec![0.0, 0.0, 0.0];
    let lat_edge = vec![0.0, 0.0, 0.5];

    let output = edge_distance_angle_one_based(
        &vertices,
        &cells,
        &edge_points,
        &vertices_on_edge,
        &cells_on_edge,
        &lat_vertex,
        &lon_edge,
        &lat_edge,
    )
    .expect("valid edge metric inputs");

    approx_eq(
        output.dv_edge[2],
        arc_length_unit_sphere(vertex1, vertex2),
        1.0e-15,
    );
    approx_eq(
        output.dc_edge[2],
        arc_length_unit_sphere(cell1, cell2),
        1.0e-15,
    );
    approx_eq(output.angle_edge[2], 0.0, 1.0e-12);
}

#[test]
fn edge_distance_angle_rejects_bad_connectivity() {
    let point = CartesianPoint::new(1.0, 0.0, 0.0);
    let vertices = vec![point, point, point];
    let cells = vec![point, point, point];
    let edge_points = vec![point, point, point];
    let vertices_on_edge = vec![[0, 0], [0, 0], [2, 99]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 2]];
    let coords = vec![0.0, 0.0, 0.0];

    assert!(edge_distance_angle_one_based(
        &vertices,
        &cells,
        &edge_points,
        &vertices_on_edge,
        &cells_on_edge,
        &coords,
        &coords,
        &coords,
    )
    .is_none());
}

#[test]
fn edge_id_sort_reorders_edges_to_match_canonical_cells_and_rebuilds_edges_on_vertex() {
    let cells_on_edge_canonical = vec![[0, 0], [0, 0], [10, 20], [30, 40]];
    let cells_on_edge = vec![[0, 0], [0, 0], [30, 40], [10, 20]];
    let vertices_on_edge = vec![[0, 0], [0, 0], [4, 5], [2, 3]];
    let edge_points = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(30.0, 40.0),
        LonLatDegrees::new(10.0, 20.0),
    ];

    let output = edge_id_sort_one_based(
        6,
        &cells_on_edge_canonical,
        &cells_on_edge,
        &vertices_on_edge,
        &edge_points,
    )
    .expect("sortable edge ids");

    assert_eq!(output.cells_on_edge[2], [10, 20]);
    assert_eq!(output.cells_on_edge[3], [30, 40]);
    assert_eq!(output.vertices_on_edge[2], [2, 3]);
    assert_eq!(output.vertices_on_edge[3], [4, 5]);
    assert_eq!(output.edge_points[2], LonLatDegrees::new(10.0, 20.0));
    assert_eq!(output.edge_points[3], LonLatDegrees::new(30.0, 40.0));
    assert_eq!(output.edges_on_vertex[2], [2, 0, 0]);
    assert_eq!(output.edges_on_vertex[3], [2, 0, 0]);
    assert_eq!(output.edges_on_vertex[4], [3, 0, 0]);
    assert_eq!(output.edges_on_vertex[5], [3, 0, 0]);
}

#[test]
fn edge_id_sort_rejects_missing_canonical_match() {
    let cells_on_edge_canonical = vec![[0, 0], [0, 0], [10, 20]];
    let cells_on_edge = vec![[0, 0], [0, 0], [30, 40]];
    let vertices_on_edge = vec![[0, 0], [0, 0], [2, 3]];
    let edge_points = vec![LonLatDegrees::new(0.0, 0.0); 3];

    assert!(edge_id_sort_one_based(
        4,
        &cells_on_edge_canonical,
        &cells_on_edge,
        &vertices_on_edge,
        &edge_points,
    )
    .is_none());
}

#[test]
fn set_weights_on_edge_matches_canonical_two_cell_stencil() {
    let area_cell = vec![0.0, 0.0, 1.0, 1.0];
    let angle_edge = vec![0.0; 8];
    let dc_edge = vec![1.0; 8];
    let dv_edge = vec![1.0; 8];
    let mut kite_areas_on_vertex = vec![[0.0; 3]; 6];
    kite_areas_on_vertex[2][0] = 0.25;
    kite_areas_on_vertex[3][0] = 0.25;
    kite_areas_on_vertex[4][0] = 0.50;
    kite_areas_on_vertex[2][1] = 0.20;
    kite_areas_on_vertex[3][1] = 0.30;
    kite_areas_on_vertex[5][0] = 0.50;

    let edges_on_cell = vec![vec![], vec![], vec![2, 4, 5], vec![2, 6, 7]];
    let cells_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 0],
        [2, 3, 0],
        [2, 0, 0],
        [3, 0, 0],
    ];
    let cells_on_edge = vec![
        [0, 0],
        [0, 0],
        [2, 3],
        [0, 0],
        [2, 10],
        [2, 11],
        [3, 12],
        [3, 13],
    ];
    let vertices_on_cell = vec![vec![], vec![], vec![3, 2, 4], vec![2, 3, 5]];
    let vertices_on_edge = vec![
        [0, 0],
        [0, 0],
        [2, 3],
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
    ];
    let n_edges_on_cell = vec![0, 0, 3, 3];

    let output = set_weights_on_edge_one_based(
        &area_cell,
        &angle_edge,
        &dc_edge,
        &dv_edge,
        &kite_areas_on_vertex,
        &edges_on_cell,
        &cells_on_vertex,
        &cells_on_edge,
        &vertices_on_cell,
        &vertices_on_edge,
        &n_edges_on_cell,
    )
    .expect("valid set_weightsOnEdge inputs");

    assert_eq!(output.n_edges_on_edge[2], 4);
    assert_eq!(output.edges_on_edge[2], vec![4, 5, 6, 7]);
    approx_eq(output.weights_on_edge[2][0], 0.25, 1.0e-15);
    approx_eq(output.weights_on_edge[2][1], 0.0, 1.0e-15);
    approx_eq(output.weights_on_edge[2][2], -0.30, 1.0e-15);
    approx_eq(output.weights_on_edge[2][3], 0.0, 1.0e-15);
    approx_eq(output.error_segment[2], 0.05, 1.0e-15);
}

#[test]
fn set_weights_on_edge_rejects_missing_cell_in_vertex_kite_lookup() {
    let area_cell = vec![0.0, 0.0, 1.0, 1.0];
    let angle_edge = vec![0.0; 3];
    let dc_edge = vec![1.0; 3];
    let dv_edge = vec![1.0; 3];
    let kite_areas_on_vertex = vec![[0.0; 3]; 4];
    let edges_on_cell = vec![vec![], vec![], vec![2], vec![2]];
    let cells_on_vertex = vec![[0, 0, 0]; 4];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3]];
    let vertices_on_cell = vec![vec![], vec![], vec![2], vec![3]];
    let vertices_on_edge = vec![[0, 0], [0, 0], [2, 3]];
    let n_edges_on_cell = vec![0, 0, 1, 1];

    assert!(set_weights_on_edge_one_based(
        &area_cell,
        &angle_edge,
        &dc_edge,
        &dv_edge,
        &kite_areas_on_vertex,
        &edges_on_cell,
        &cells_on_vertex,
        &cells_on_edge,
        &vertices_on_cell,
        &vertices_on_edge,
        &n_edges_on_cell,
    )
    .is_none());
}
