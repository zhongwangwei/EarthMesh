use earthmesh_mesh::{
    arc_length_unit_sphere, area_triangle_reconstruction_error_fortran_indexed,
    cells_on_edge_from_neighbor_cells, centroid_spherical_mesh_fortran_indexed,
    circumcenter_spherical_mesh_fortran_indexed, edge_midpoints_from_cells_fortran_indexed,
    edges_on_edge_tri_fortran_indexed, get_area_production_fortran_indexed,
    get_area_unit_fortran_indexed, get_edge_connectivity_fortran_indexed,
    get_edge_production_fortran_indexed, grid_quality_check_global_fortran_indexed, is_ngrmm,
    lonlat_degrees_to_unit_xyz, next_ccw_edge_candidate_slot, normalize_lon_m180_180,
    normalize_vertex_rotation, order_vertex_arrays_for_vertex, order_vertex_arrays_fortran_indexed,
    order_vertices_on_edge_fortran_indexed, polygon_length_angle_metrics,
    polygon_mesh_quality_fortran_indexed, set_dists_on_edge_global_fortran_indexed,
    shared_cell_for_edge_pair, should_swap_vertices_on_edge,
    spherical_cell_area_from_vertices_unit, spherical_kite_area_unit, spherical_triangle_area_unit,
    spring_dynamics_global_fortran_indexed, springjustment_global_core_fortran_indexed,
    triangle_mesh_quality_fortran_indexed, triangle_neighbors_from_cell_membership_fortran_indexed,
    vertex_cell_position, CartesianPoint, DistanceLayerSpacing, GetAreaUnitInput,
    GlobalDistanceStep, LonLatDegrees, SetDistsOnEdgeGlobalInput, SpringjustmentGlobalCoreInput,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn normalize_lon_matches_mod_grid_preprocess_checklon_single_wrap() {
    approx_eq(normalize_lon_m180_180(181.0), -179.0, 0.0);
    approx_eq(normalize_lon_m180_180(-181.0), 179.0, 0.0);
    approx_eq(normalize_lon_m180_180(180.0), 180.0, 0.0);
    approx_eq(normalize_lon_m180_180(-180.0), -180.0, 0.0);
    // Fortran CheckLon performs one adjustment, not repeated modulo wrapping.
    approx_eq(normalize_lon_m180_180(541.0), 181.0, 0.0);
}

#[test]
fn arc_length_matches_mod_grid_preprocess_equator_quarter_turn() {
    let a = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let b = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(90.0, 0.0));

    approx_eq(arc_length_unit_sphere(a, b), 1.5707962671902518, 1.0e-12);
}

#[test]
fn arc_length_matches_mod_grid_preprocess_one_degree_equator() {
    let a = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let b = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));

    approx_eq(arc_length_unit_sphere(a, b), 0.01745329238890877, 1.0e-12);
}

#[test]
fn arc_length_scales_by_input_radius_like_fortran() {
    let a = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let b = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let scaled_a = earthmesh_mesh::CartesianPoint::new(a.x * 2.0, a.y * 2.0, a.z * 2.0);
    let scaled_b = earthmesh_mesh::CartesianPoint::new(b.x * 2.0, b.y * 2.0, b.z * 2.0);

    approx_eq(
        arc_length_unit_sphere(scaled_a, scaled_b),
        2.0 * arc_length_unit_sphere(a, b),
        1.0e-12,
    );
}

#[test]
fn polygon_length_angle_matches_fortran_octant_triangle() {
    let metrics = earthmesh_mesh::polygon_length_angle_metrics(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(0.0, 90.0),
    ])
    .expect("valid triangle");

    assert_eq!(metrics.angles_degrees.len(), 3);
    assert_eq!(metrics.edge_lengths_meters.len(), 3);

    for angle in metrics.angles_degrees {
        approx_eq(angle, 90.0, 1.0e-5);
    }
    for length in metrics.edge_lengths_meters {
        approx_eq(length, 10_007_902.73061428, 1.0e-3);
    }
}

#[test]
fn polygon_length_angle_rejects_degenerate_polygons() {
    assert!(earthmesh_mesh::polygon_length_angle_metrics(&[]).is_none());
    assert!(
        earthmesh_mesh::polygon_length_angle_metrics(&[LonLatDegrees::new(0.0, 0.0)]).is_none()
    );
    assert!(earthmesh_mesh::polygon_length_angle_metrics(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
    ])
    .is_none());
}

#[test]
fn triangle_mesh_quality_matches_fortran_aggregation_for_single_triangle() {
    let quality = earthmesh_mesh::triangle_mesh_quality(&[[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(0.0, 90.0),
    ]])
    .expect("valid triangle quality");

    approx_eq(quality.extreme_angles_degrees.0, 90.0, 1.0e-5);
    approx_eq(quality.extreme_angles_degrees.1, 90.0, 1.0e-5);
    approx_eq(quality.average_min_max_angles_degrees.0, 90.0, 1.0e-5);
    approx_eq(quality.average_min_max_angles_degrees.1, 90.0, 1.0e-5);
    approx_eq(quality.angle_stddev_degrees, 30.0, 1.0e-5);
    assert_eq!(quality.angle_less_flags, vec![false]);
    assert_eq!(quality.angle_more_flags, vec![true]);
    assert_eq!(quality.cell_metrics.len(), 1);
}

#[test]
fn triangle_mesh_quality_rejects_empty_mesh() {
    assert!(earthmesh_mesh::triangle_mesh_quality(&[]).is_none());
}

#[test]
fn polygon_mesh_quality_matches_fortran_thresholds_for_square() {
    let quality = earthmesh_mesh::polygon_mesh_quality(&[vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(90.0, 45.0),
        LonLatDegrees::new(0.0, 45.0),
    ]])
    .expect("valid polygon quality");

    assert_eq!(quality.cell_metrics.len(), 1);
    assert_eq!(quality.angle_less_flags.len(), 1);
    assert_eq!(quality.angle_more_flags.len(), 1);
    assert!(quality.extreme_angles_degrees.0 > 0.0);
    assert!(quality.extreme_angles_degrees.1 > quality.extreme_angles_degrees.0);

    // For four-sided cells, Fortran regular angle is 90 degrees and thresholds are 81/99.
    let expected_stddev = (quality.cell_metrics[0]
        .angles_degrees
        .iter()
        .map(|angle| (angle - 90.0).powi(2))
        .sum::<f64>()
        / 4.0)
        .sqrt();
    approx_eq(quality.angle_stddev_degrees, expected_stddev, 1.0e-12);
    assert_eq!(
        quality.angle_less_flags[0],
        quality.extreme_angles_degrees.0 < 81.0
    );
    assert_eq!(
        quality.angle_more_flags[0],
        quality.extreme_angles_degrees.1 > 99.0
    );
}

#[test]
fn polygon_mesh_quality_rejects_empty_mesh_and_degenerate_cells() {
    assert!(earthmesh_mesh::polygon_mesh_quality(&[]).is_none());
    assert!(earthmesh_mesh::polygon_mesh_quality(&[vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
    ]])
    .is_none());
}

#[test]
fn robust_spherical_area_matches_fortran_equatorial_square() {
    let area = earthmesh_mesh::robust_spherical_area_unit(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
        LonLatDegrees::new(1.0, 1.0),
        LonLatDegrees::new(0.0, 1.0),
    ])
    .expect("valid polygon");

    approx_eq(area, -0.0003046019547268505, 1.0e-15);
}

#[test]
fn robust_spherical_area_matches_fortran_dateline_delta_wrap() {
    let area = earthmesh_mesh::robust_spherical_area_unit(&[
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(-179.0, 0.0),
        LonLatDegrees::new(-179.0, 1.0),
        LonLatDegrees::new(179.0, 1.0),
    ])
    .expect("valid dateline polygon");

    approx_eq(area, -0.000609203909453701, 1.0e-15);
}

#[test]
fn robust_spherical_area_rejects_degenerate_polygons() {
    assert!(earthmesh_mesh::robust_spherical_area_unit(&[]).is_none());
    assert!(earthmesh_mesh::robust_spherical_area_unit(&[LonLatDegrees::new(0.0, 0.0)]).is_none());
    assert!(earthmesh_mesh::robust_spherical_area_unit(&[
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
    ])
    .is_none());
}

#[test]
fn spherical_triangle_area_matches_fortran_octant_triangle() {
    let triangle = [
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(90.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 90.0)),
    ];

    approx_eq(
        spherical_triangle_area_unit(triangle),
        1.5707961479809727,
        1.0e-12,
    );
}

#[test]
fn spherical_triangle_area_matches_fortran_small_right_triangle() {
    let triangle = [
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0)),
    ];

    approx_eq(
        spherical_triangle_area_unit(triangle),
        0.00015231644029306792,
        1.0e-15,
    );
}

#[test]
fn spherical_kite_area_matches_fortran_getarea_two_triangle_sum() {
    let vertex = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let edge1 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let edge2 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let cell = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.5, 0.5));

    approx_eq(
        spherical_kite_area_unit(vertex, edge1, edge2, cell),
        0.00015230773702390324,
        1.0e-15,
    );
}

#[test]
fn spherical_cell_area_fans_vertices_like_fortran_getarea() {
    let vertices = [
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 1.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0)),
    ];

    approx_eq(
        spherical_cell_area_from_vertices_unit(&vertices).expect("valid cell"),
        0.000304609680288118,
        1.0e-15,
    );
}

#[test]
fn spherical_cell_area_rejects_degenerate_cells() {
    assert!(spherical_cell_area_from_vertices_unit(&[]).is_none());
    assert!(spherical_cell_area_from_vertices_unit(&[
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0)),
    ])
    .is_none());
}

#[test]
fn shared_cell_for_edge_pair_matches_fortran_getarea_combinations() {
    assert_eq!(shared_cell_for_edge_pair([12, 30], [7, 12]), Some(12));
    assert_eq!(shared_cell_for_edge_pair([12, 30], [30, 7]), Some(30));
    assert_eq!(shared_cell_for_edge_pair([0, 30], [30, 0]), Some(30));
    assert_eq!(shared_cell_for_edge_pair([4, 9], [5, 10]), None);
}

#[test]
fn vertex_cell_position_matches_fortran_cells_on_vertex_scan() {
    assert_eq!(vertex_cell_position([4, 8, 15], 4), Some(0));
    assert_eq!(vertex_cell_position([4, 8, 15], 8), Some(1));
    assert_eq!(vertex_cell_position([4, 8, 15], 15), Some(2));
    assert_eq!(vertex_cell_position([4, 8, 15], 16), None);
}

#[test]
fn get_area_unit_matches_fortran_indexed_kite_triangle_and_cell_workflow() {
    let zero = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let edge1 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
    let edge2 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
    let cell = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.5, 0.5));

    let vertices = vec![
        zero, // index 0 unused
        zero, // index 1 skipped by Fortran loops
        zero,
        edge1,
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 1.0)),
        edge2,
    ];
    let edge_points = vec![zero, zero, edge1, edge2];
    let cell_points = vec![zero, zero, cell];
    let cells_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
    ];
    let edges_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
    ];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 0], [2, 0]];
    let vertices_on_cell = vec![vec![], vec![], vec![2, 3, 4, 5]];

    let output = get_area_unit_fortran_indexed(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cell_points,
        cells_on_vertex: &cells_on_vertex,
        edges_on_vertex: &edges_on_vertex,
        cells_on_edge: &cells_on_edge,
        vertices_on_cell: &vertices_on_cell,
    })
    .expect("valid Fortran-indexed area input");

    approx_eq(
        output.kite_areas_on_vertex[2][0],
        0.00015230773702390324,
        1.0e-15,
    );
    approx_eq(output.area_triangle[2], 0.00015230773702390324, 1.0e-15);
    approx_eq(output.area_cell[2], 0.000304609680288118, 1.0e-15);
}

#[test]
fn area_triangle_reconstruction_error_matches_fortran_getarea_summary() {
    let cell_points = vec![
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(90.0, 0.0)),
        lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 90.0)),
    ];
    let cells_on_vertex = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [2, 3, 4]];
    let exact = spherical_triangle_area_unit([cell_points[2], cell_points[3], cell_points[4]]);
    let area_triangle = vec![0.0, 0.0, exact, exact * 1.1];

    let summary = area_triangle_reconstruction_error_fortran_indexed(
        &area_triangle,
        &cell_points,
        &cells_on_vertex,
    )
    .expect("valid reconstruction summary");

    approx_eq(summary.max_relative, 0.1, 1.0e-12);
    approx_eq(summary.avg_relative, 0.05, 1.0e-12);
}

#[test]
fn is_ngrmm_matches_fortran_opposite_vertex_codes() {
    assert_eq!(is_ngrmm([1, 2, 3], [1, 2, 9]), Some(3));
    assert_eq!(is_ngrmm([1, 2, 3], [1, 8, 3]), Some(2));
    assert_eq!(is_ngrmm([1, 2, 3], [7, 2, 3]), Some(1));
    assert_eq!(is_ngrmm([1, 2, 3], [7, 8, 3]), None);
}

#[test]
fn cells_on_edge_from_neighbor_cells_matches_fortran_getedge_mapping() {
    assert_eq!(
        cells_on_edge_from_neighbor_cells([3, 1, 2], [1, 3, 9]),
        Some([1, 3])
    );
    assert_eq!(
        cells_on_edge_from_neighbor_cells([3, 2, 1], [1, 3, 9]),
        Some([1, 3])
    );
    assert_eq!(
        cells_on_edge_from_neighbor_cells([1, 2, 3], [9, 2, 3]),
        Some([2, 3])
    );
    assert_eq!(
        cells_on_edge_from_neighbor_cells([1, 2, 3], [7, 8, 3]),
        None
    );
}

#[test]
fn should_swap_vertices_on_edge_matches_fortran_cross_product_rule() {
    assert!(!should_swap_vertices_on_edge(
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 1.0),
    ));
    assert!(should_swap_vertices_on_edge(
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, -1.0),
    ));
    assert!(!should_swap_vertices_on_edge(
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(-179.0, 1.0),
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(179.0, 1.0),
    ));
}

#[test]
fn normalize_vertex_rotation_matches_fortran_min_cell_rotation() {
    assert_eq!(
        normalize_vertex_rotation([7, 3, 5], [70, 30, 50]),
        ([3, 5, 7], [30, 50, 70])
    );
    assert_eq!(
        normalize_vertex_rotation([7, 5, 3], [70, 50, 30]),
        ([3, 7, 5], [30, 70, 50])
    );
    assert_eq!(
        normalize_vertex_rotation([0, 4, 6], [0, 40, 60]),
        ([4, 6, 0], [40, 60, 0])
    );
    assert_eq!(
        normalize_vertex_rotation([0, 0, 0], [10, 20, 30]),
        ([0, 0, 0], [10, 20, 30])
    );
}

#[test]
fn next_ccw_edge_candidate_slot_matches_fortran_min_positive_angle_rule() {
    let vertex = CartesianPoint::new(0.0, 0.0, 1.0);
    let reference_edge = CartesianPoint::new(1.0, 0.0, 1.0);
    let candidates = [
        CartesianPoint::new(0.0, -1.0, 1.0),
        CartesianPoint::new(0.2, 0.2, 1.0),
        CartesianPoint::new(0.0, 1.0, 1.0),
    ];

    assert_eq!(
        next_ccw_edge_candidate_slot(vertex, reference_edge, &candidates),
        Some(1)
    );
}

#[test]
fn next_ccw_edge_candidate_slot_rejects_clockwise_or_degenerate_candidates() {
    let vertex = CartesianPoint::new(0.0, 0.0, 1.0);
    let reference_edge = CartesianPoint::new(1.0, 0.0, 1.0);
    let candidates = [
        CartesianPoint::new(0.0, -1.0, 1.0),
        CartesianPoint::new(2.0, 0.0, 1.0),
    ];

    assert_eq!(
        next_ccw_edge_candidate_slot(vertex, reference_edge, &candidates),
        None
    );
}

#[test]
fn order_vertex_arrays_for_vertex_matches_fortran_edge_sort_and_cell_rebuild() {
    let mut edge_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); 13];
    edge_points[10] = CartesianPoint::new(1.0, 0.0, 1.0);
    edge_points[11] = CartesianPoint::new(0.0, 1.0, 1.0);
    edge_points[12] = CartesianPoint::new(0.2, 0.2, 1.0);

    let mut vertices_on_edge = vec![[0usize, 0usize]; 13];
    vertices_on_edge[10] = [2, 99];
    vertices_on_edge[11] = [2, 99];
    vertices_on_edge[12] = [99, 2];

    let mut cells_on_edge = vec![[0usize, 0usize]; 13];
    cells_on_edge[10] = [100, 200];
    cells_on_edge[11] = [110, 210];
    cells_on_edge[12] = [120, 220];

    let ordered = order_vertex_arrays_for_vertex(
        2,
        CartesianPoint::new(0.0, 0.0, 1.0),
        [10, 11, 12],
        &edge_points,
        &vertices_on_edge,
        &cells_on_edge,
    )
    .expect("valid vertex ordering");

    assert_eq!(ordered.edges_on_vertex, [10, 12, 11]);
    assert_eq!(ordered.cells_on_vertex, [100, 220, 110]);
}

#[test]
fn order_vertex_arrays_fortran_indexed_processes_vertices_from_two() {
    let zero = CartesianPoint::new(0.0, 0.0, 0.0);
    let vertex_points = vec![zero, zero, CartesianPoint::new(0.0, 0.0, 1.0)];

    let mut edge_points = vec![zero; 13];
    edge_points[10] = CartesianPoint::new(1.0, 0.0, 1.0);
    edge_points[11] = CartesianPoint::new(0.0, 1.0, 1.0);
    edge_points[12] = CartesianPoint::new(0.2, 0.2, 1.0);

    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [10, 11, 12]];
    let mut vertices_on_edge = vec![[0usize, 0usize]; 13];
    vertices_on_edge[10] = [2, 99];
    vertices_on_edge[11] = [2, 99];
    vertices_on_edge[12] = [99, 2];

    let mut cells_on_edge = vec![[0usize, 0usize]; 13];
    cells_on_edge[10] = [100, 200];
    cells_on_edge[11] = [110, 210];
    cells_on_edge[12] = [120, 220];

    let output = order_vertex_arrays_fortran_indexed(
        &vertex_points,
        &edge_points,
        &edges_on_vertex,
        &vertices_on_edge,
        &cells_on_edge,
    )
    .expect("valid Fortran-indexed ordering input");

    assert_eq!(output.edges_on_vertex[1], [0, 0, 0]);
    assert_eq!(output.cells_on_vertex[1], [0, 0, 0]);
    assert_eq!(output.edges_on_vertex[2], [10, 12, 11]);
    assert_eq!(output.cells_on_vertex[2], [100, 220, 110]);
}

#[test]
fn order_vertices_on_edge_fortran_indexed_matches_getsort_vertices_on_edge() {
    let points = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 1.0),
        LonLatDegrees::new(0.0, -1.0),
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(179.0, 1.0),
    ];
    let cells = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(-179.0, 1.0),
    ];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3], [2, 3], [4, 5]];
    let vertices_on_edge = vec![[0, 0], [9, 9], [2, 3], [2, 4], [5, 6]];

    let ordered =
        order_vertices_on_edge_fortran_indexed(&points, &cells, &cells_on_edge, &vertices_on_edge)
            .expect("valid Fortran-indexed edge sorting input");

    assert_eq!(ordered[1], [9, 9]);
    assert_eq!(ordered[2], [2, 3]);
    assert_eq!(ordered[3], [4, 2]);
    assert_eq!(ordered[4], [5, 6]);
}

#[test]
fn get_edge_connectivity_fortran_indexed_matches_getedge_reuse_and_cell_mapping() {
    let cells_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [10, 11, 12],
        [10, 11, 13],
        [10, 12, 13],
        [11, 12, 13],
    ];
    let triangle_neighbors = vec![
        [0, 0, 0],
        [0, 0, 0],
        [3, 4, 5],
        [2, 4, 5],
        [2, 3, 5],
        [2, 3, 4],
    ];

    let connectivity = get_edge_connectivity_fortran_indexed(&triangle_neighbors, &cells_on_vertex)
        .expect("valid Fortran-indexed GetEdge input");

    assert_eq!(connectivity.edges_on_vertex[2], [2, 3, 4]);
    assert_eq!(connectivity.edges_on_vertex[3][0], 2);
    assert_eq!(connectivity.vertices_on_edge[2], [2, 3]);
    assert_eq!(connectivity.cells_on_edge[2], [10, 11]);
    assert_eq!(connectivity.cells_on_edge[3], [10, 12]);
    assert_eq!(connectivity.cells_on_edge[4], [11, 12]);
    assert!(connectivity.vertices_on_edge.len() > 7);
}

#[test]
fn triangle_neighbors_from_cell_membership_matches_set_ngrmm_slots() {
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [10, 11, 12],
        [10, 11, 13],
        [10, 12, 13],
        [11, 12, 13],
    ];
    let mut triangles_on_cell = vec![vec![]; 14];
    triangles_on_cell[10] = vec![2, 3, 4];
    triangles_on_cell[11] = vec![2, 3, 5];
    triangles_on_cell[12] = vec![2, 4, 5];
    triangles_on_cell[13] = vec![3, 4, 5];
    let mut triangle_counts_on_cell = vec![0usize; 14];
    triangle_counts_on_cell[10] = 3;
    triangle_counts_on_cell[11] = 3;
    triangle_counts_on_cell[12] = 3;
    triangle_counts_on_cell[13] = 3;

    let triangle_neighbors = triangle_neighbors_from_cell_membership_fortran_indexed(
        &cells_on_triangle,
        &triangles_on_cell,
        &triangle_counts_on_cell,
    )
    .expect("valid set_ngrmm inputs");

    assert_eq!(triangle_neighbors[2], [5, 4, 3]);
    assert_eq!(triangle_neighbors[3], [5, 4, 2]);
    assert_eq!(triangle_neighbors[4], [5, 3, 2]);
    assert_eq!(triangle_neighbors[5], [4, 3, 2]);
}

#[test]
fn edges_on_edge_tri_matches_fortran_endpoint_cyclic_neighbors() {
    let vertices_on_edge = vec![
        [0, 0],
        [0, 0],
        [2, 3],
        [2, 4],
        [2, 5],
        [3, 4],
        [3, 5],
        [4, 5],
    ];
    let edges_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [2, 5, 6],
        [3, 5, 7],
        [4, 6, 7],
    ];

    let edges_on_edge_tri = edges_on_edge_tri_fortran_indexed(&vertices_on_edge, &edges_on_vertex)
        .expect("valid set_edgesOnEdge_tri inputs");

    assert_eq!(edges_on_edge_tri[2], [3, 4, 5, 6]);
    assert_eq!(edges_on_edge_tri[3], [4, 2, 5, 7]);
    assert_eq!(edges_on_edge_tri[7], [3, 5, 4, 6]);
}

#[test]
fn edge_midpoints_from_cells_fortran_indexed_matches_getedge_optional_vp() {
    let cells = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(179.0, 0.0),
        LonLatDegrees::new(-179.0, 0.0),
    ];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3], [4, 5]];

    let midpoints = edge_midpoints_from_cells_fortran_indexed(&cells_on_edge, &cells)
        .expect("valid Fortran-indexed midpoint input");

    approx_eq(midpoints[2].lon_degrees, 45.0, 1.0e-12);
    approx_eq(midpoints[2].lat_degrees, 0.0, 1.0e-12);
    approx_eq(midpoints[3].lon_degrees.abs(), 180.0, 1.0e-12);
    approx_eq(midpoints[3].lat_degrees, 0.0, 1.0e-12);
}

#[test]
fn get_edge_production_wrapper_matches_manual_getedge_pipeline() {
    let cells_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [10, 11, 12],
        [10, 11, 13],
        [10, 12, 13],
        [11, 12, 13],
    ];
    let triangle_neighbors = vec![
        [0, 0, 0],
        [0, 0, 0],
        [3, 4, 5],
        [2, 4, 5],
        [2, 3, 5],
        [2, 3, 4],
    ];
    let triangle_lonlat = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.2, 0.2),
        LonLatDegrees::new(0.8, 0.2),
        LonLatDegrees::new(0.2, 0.8),
        LonLatDegrees::new(0.8, 0.8),
    ];
    let mut cell_lonlat = vec![LonLatDegrees::new(0.0, 0.0); 14];
    cell_lonlat[10] = LonLatDegrees::new(0.0, 0.0);
    cell_lonlat[11] = LonLatDegrees::new(1.0, 0.0);
    cell_lonlat[12] = LonLatDegrees::new(0.0, 1.0);
    cell_lonlat[13] = LonLatDegrees::new(1.0, 1.0);

    let output = get_edge_production_fortran_indexed(
        &triangle_neighbors,
        &cells_on_vertex,
        &triangle_lonlat,
        &cell_lonlat,
    )
    .expect("valid production GetEdge input");

    let connectivity = get_edge_connectivity_fortran_indexed(&triangle_neighbors, &cells_on_vertex)
        .expect("valid connectivity");
    let expected_vertices_on_edge = order_vertices_on_edge_fortran_indexed(
        &triangle_lonlat,
        &cell_lonlat,
        &connectivity.cells_on_edge,
        &connectivity.vertices_on_edge,
    )
    .expect("sorted verticesOnEdge");
    let expected_edge_points =
        edge_midpoints_from_cells_fortran_indexed(&connectivity.cells_on_edge, &cell_lonlat)
            .expect("edge midpoint output");
    let triangle_points = triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let edge_points_cartesian = expected_edge_points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let expected_ordered = order_vertex_arrays_fortran_indexed(
        &triangle_points,
        &edge_points_cartesian,
        &connectivity.edges_on_vertex,
        &expected_vertices_on_edge,
        &connectivity.cells_on_edge,
    )
    .expect("ordered vertex arrays");

    assert_eq!(output.cells_on_edge, connectivity.cells_on_edge);
    assert_eq!(output.vertices_on_edge, expected_vertices_on_edge);
    assert_eq!(output.edge_points, expected_edge_points);
    assert_eq!(output.edges_on_vertex, expected_ordered.edges_on_vertex);
    assert_eq!(output.cells_on_vertex, expected_ordered.cells_on_vertex);
}

#[test]
fn springjustment_global_core_matches_manual_migrated_pipeline() {
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [10, 11, 12],
        [10, 11, 13],
        [10, 12, 13],
        [11, 12, 13],
    ];
    let mut triangles_on_cell = vec![Vec::<usize>::new(); 14];
    triangles_on_cell[10] = vec![2, 3, 4];
    triangles_on_cell[11] = vec![2, 3, 5];
    triangles_on_cell[12] = vec![2, 4, 5];
    triangles_on_cell[13] = vec![3, 4, 5];
    let mut n_edges_on_cell = vec![0usize; 14];
    n_edges_on_cell[10] = 3;
    n_edges_on_cell[11] = 3;
    n_edges_on_cell[12] = 3;
    n_edges_on_cell[13] = 3;
    let triangle_lonlat = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.2, 0.2),
        LonLatDegrees::new(0.8, 0.2),
        LonLatDegrees::new(0.2, 0.8),
        LonLatDegrees::new(0.8, 0.8),
    ];
    let mut cell_lonlat = vec![LonLatDegrees::new(0.0, 0.0); 14];
    cell_lonlat[10] = LonLatDegrees::new(0.0, 0.0);
    cell_lonlat[11] = LonLatDegrees::new(1.0, 0.0);
    cell_lonlat[12] = LonLatDegrees::new(0.0, 1.0);
    cell_lonlat[13] = LonLatDegrees::new(1.0, 1.0);

    let output = springjustment_global_core_fortran_indexed(SpringjustmentGlobalCoreInput {
        triangle_lonlat: &triangle_lonlat,
        cell_lonlat: &cell_lonlat,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        base_dists_on_edge: 2.0,
        base_cellwidth: None,
        distance_num_rc: 0,
        distance_spacing: DistanceLayerSpacing::Linear,
        distance_steps: &[],
        niter_refine: 1,
        relax: 0.25,
        radius: 1.0,
        diagnostic_every: 100,
    })
    .expect("valid springjustment global core input");

    let triangle_neighbors = triangle_neighbors_from_cell_membership_fortran_indexed(
        &cells_on_triangle,
        &triangles_on_cell,
        &n_edges_on_cell,
    )
    .expect("triangle neighbors");
    let edge_output = get_edge_production_fortran_indexed(
        &triangle_neighbors,
        &cells_on_triangle,
        &triangle_lonlat,
        &cell_lonlat,
    )
    .expect("edge production");
    let cell_connectivity = earthmesh_mesh::connect_on_cell_fortran_indexed(
        &n_edges_on_cell,
        &edge_output.cells_on_edge,
        &edge_output.edges_on_vertex,
        &triangles_on_cell,
    )
    .expect("cell connectivity");
    let edges_on_edge_tri = edges_on_edge_tri_fortran_indexed(
        &edge_output.vertices_on_edge,
        &edge_output.edges_on_vertex,
    )
    .expect("edges on edge tri");
    let dists_on_edge = vec![2.0; edge_output.cells_on_edge.len()];
    let cell_points = cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let spring_output = spring_dynamics_global_fortran_indexed(
        &cell_points,
        &n_edges_on_cell,
        &cell_connectivity.edges_on_cell,
        &edge_output.cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        1,
        0.25,
        1.0,
        100,
    )
    .expect("spring dynamics");
    let expected_cell_lonlat = spring_output
        .updated_cell_points
        .iter()
        .copied()
        .map(earthmesh_mesh::xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let centroid_lonlat =
        centroid_spherical_mesh_fortran_indexed(&expected_cell_lonlat, &cells_on_triangle)
            .expect("centroids");
    let centroid_cartesian = centroid_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let circumcenters = circumcenter_spherical_mesh_fortran_indexed(
        &centroid_cartesian,
        &spring_output.updated_cell_points,
        &cells_on_triangle,
    )
    .expect("circumcenters");
    let expected_triangle_lonlat = circumcenters
        .iter()
        .copied()
        .map(earthmesh_mesh::xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();

    assert_eq!(output.triangle_neighbors, triangle_neighbors);
    assert_eq!(output.cells_on_edge, edge_output.cells_on_edge);
    assert_eq!(output.vertices_on_edge, edge_output.vertices_on_edge);
    assert_eq!(output.edges_on_cell, cell_connectivity.edges_on_cell);
    assert_eq!(output.dists_on_edge, dists_on_edge);
    assert_eq!(output.updated_cell_lonlat, expected_cell_lonlat);
    assert_eq!(output.updated_triangle_lonlat, expected_triangle_lonlat);
    assert_eq!(
        output.spring.diagnostic_max_displacements,
        spring_output.diagnostic_max_displacements
    );
}

#[test]
fn springjustment_global_core_wires_distance_step_updates() {
    let cells_on_triangle = vec![
        [0, 0, 0],
        [0, 0, 0],
        [10, 11, 12],
        [10, 11, 13],
        [10, 12, 13],
        [11, 12, 13],
    ];
    let mut triangles_on_cell = vec![Vec::<usize>::new(); 14];
    triangles_on_cell[10] = vec![2, 3, 4];
    triangles_on_cell[11] = vec![2, 3, 5];
    triangles_on_cell[12] = vec![2, 4, 5];
    triangles_on_cell[13] = vec![3, 4, 5];
    let mut n_edges_on_cell = vec![0usize; 14];
    n_edges_on_cell[10] = 3;
    n_edges_on_cell[11] = 3;
    n_edges_on_cell[12] = 3;
    n_edges_on_cell[13] = 3;
    let triangle_lonlat = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.2, 0.2),
        LonLatDegrees::new(0.8, 0.2),
        LonLatDegrees::new(0.2, 0.8),
        LonLatDegrees::new(0.8, 0.8),
    ];
    let mut cell_lonlat = vec![LonLatDegrees::new(0.0, 0.0); 14];
    cell_lonlat[10] = LonLatDegrees::new(0.0, 0.0);
    cell_lonlat[11] = LonLatDegrees::new(1.0, 0.0);
    cell_lonlat[12] = LonLatDegrees::new(0.0, 1.0);
    cell_lonlat[13] = LonLatDegrees::new(1.0, 1.0);
    let refinement_flags = vec![false, false, true, false, false, false];
    let distance_steps = vec![GlobalDistanceStep {
        active: true,
        halo: 1,
        refinement_flags: &refinement_flags,
        num_vertex_in: 1,
        num_center_in: 1,
    }];

    let output = springjustment_global_core_fortran_indexed(SpringjustmentGlobalCoreInput {
        triangle_lonlat: &triangle_lonlat,
        cell_lonlat: &cell_lonlat,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        base_dists_on_edge: 100.0,
        base_cellwidth: Some(200.0),
        distance_num_rc: 0,
        distance_spacing: DistanceLayerSpacing::Linear,
        distance_steps: &distance_steps,
        niter_refine: 0,
        relax: 0.25,
        radius: 1.0,
        diagnostic_every: 100,
    })
    .expect("valid springjustment global distance input");

    let expected_distance = set_dists_on_edge_global_fortran_indexed(SetDistsOnEdgeGlobalInput {
        base_dists_on_edge: 100.0,
        base_cellwidth: Some(200.0),
        num_rc: 0,
        spacing: DistanceLayerSpacing::Linear,
        triangles_on_cell: &triangles_on_cell,
        cells_on_triangle: Some(&cells_on_triangle),
        edges_on_vertex: &output.edges_on_vertex,
        cells_on_edge: &output.cells_on_edge,
        steps: &distance_steps,
    })
    .expect("manual distance update");

    assert_eq!(output.dists_on_edge, expected_distance.dists_on_edge);
    assert_eq!(output.cellwidth, expected_distance.cellwidth);
}

#[test]
fn triangle_mesh_quality_fortran_indexed_updates_adjusted_triangles_and_reuses_cache() {
    let cell_points = vec![
        LonLatDegrees::new(-999.0, -999.0),
        LonLatDegrees::new(-999.0, -999.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(0.0, 90.0),
        LonLatDegrees::new(180.0, 0.0),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [2, 4, 5]];
    let adjust_flags = vec![false, false, true, false];
    let length_cache = vec![[0.0; 3], [0.0; 3], [0.0; 3], [1.0, 2.0, 3.0]];
    let angle_cache = vec![[0.0; 3], [0.0; 3], [0.0; 3], [50.0, 60.0, 70.0]];

    let output = triangle_mesh_quality_fortran_indexed(
        &cell_points,
        &cells_on_triangle,
        &adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .expect("valid Fortran-indexed triangle quality inputs");

    assert_eq!(output.length_cache[3], [1.0, 2.0, 3.0]);
    assert_eq!(output.angle_cache[3], [50.0, 60.0, 70.0]);
    for angle in output.angle_cache[2] {
        approx_eq(angle, 90.0, 1.0e-5);
    }
    assert!(!output.angle_less_flags[2]);
    assert!(output.angle_more_flags[2]);
    assert!(!output.angle_less_flags[3]);
    assert!(!output.angle_more_flags[3]);
    approx_eq(output.extreme_angles_degrees.0, 50.0, 1.0e-12);
    approx_eq(output.extreme_angles_degrees.1, 90.0, 1.0e-5);
    approx_eq(output.average_min_max_angles_degrees.0, 70.0, 1.0e-5);
    approx_eq(output.average_min_max_angles_degrees.1, 80.0, 1.0e-5);
}

#[test]
fn triangle_mesh_quality_fortran_indexed_rejects_mismatched_cache_lengths() {
    let cell_points = vec![LonLatDegrees::new(0.0, 0.0); 5];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];
    let adjust_flags = vec![false, false, false];
    let length_cache = vec![[0.0; 3]; 2];
    let angle_cache = vec![[0.0; 3]; 3];

    assert!(triangle_mesh_quality_fortran_indexed(
        &cell_points,
        &cells_on_triangle,
        &adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .is_none());
}

#[test]
fn polygon_mesh_quality_fortran_indexed_filters_cells_and_reuses_compact_cache() {
    let cell_points = vec![LonLatDegrees::new(0.0, 0.0); 8];
    let cells_on_polygon = vec![
        vec![],
        vec![],
        vec![2, 3, 4, 5],
        vec![2, 3, 4],
        vec![3, 4, 5, 6],
    ];
    let polygon_edge_counts = vec![0, 0, 4, 3, 4];
    let adjust_flags = vec![false; 5];
    let length_cache = vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]];
    let angle_cache = vec![
        vec![80.0, 90.0, 100.0, 110.0],
        vec![85.0, 90.0, 95.0, 100.0],
    ];

    let output = polygon_mesh_quality_fortran_indexed(
        4,
        &cell_points,
        &cells_on_polygon,
        &polygon_edge_counts,
        &adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .expect("valid compact polygon quality cache");

    assert_eq!(output.length_cache, length_cache);
    assert_eq!(output.angle_cache, angle_cache);
    assert!(output.angle_less_flags[0]);
    assert!(output.angle_more_flags[0]);
    assert!(!output.angle_less_flags[1]);
    assert!(output.angle_more_flags[1]);
    approx_eq(output.extreme_angles_degrees.0, 80.0, 1.0e-12);
    approx_eq(output.extreme_angles_degrees.1, 110.0, 1.0e-12);
    approx_eq(output.average_min_max_angles_degrees.0, 82.5, 1.0e-12);
    approx_eq(output.average_min_max_angles_degrees.1, 105.0, 1.0e-12);
}

#[test]
fn polygon_mesh_quality_fortran_indexed_updates_adjusted_compact_cache() {
    let cell_points = vec![
        LonLatDegrees::new(-999.0, -999.0),
        LonLatDegrees::new(-999.0, -999.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(60.0, 0.0),
        LonLatDegrees::new(60.0, 30.0),
        LonLatDegrees::new(0.0, 30.0),
    ];
    let cells_on_polygon = vec![vec![], vec![], vec![2, 3, 4, 5]];
    let polygon_edge_counts = vec![0, 0, 4];
    let adjust_flags = vec![false, false, true];
    let length_cache = vec![vec![0.0; 4]];
    let angle_cache = vec![vec![0.0; 4]];

    let output = polygon_mesh_quality_fortran_indexed(
        4,
        &cell_points,
        &cells_on_polygon,
        &polygon_edge_counts,
        &adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .expect("valid adjusted polygon quality input");
    let expected = polygon_length_angle_metrics(&[
        cell_points[2],
        cell_points[3],
        cell_points[4],
        cell_points[5],
    ])
    .expect("polygon metrics");

    for (actual, expected) in output.angle_cache[0].iter().zip(expected.angles_degrees) {
        approx_eq(*actual, expected, 1.0e-12);
    }
    for (actual, expected) in output.length_cache[0]
        .iter()
        .zip(expected.edge_lengths_meters)
    {
        approx_eq(*actual, expected, 1.0e-9);
    }
}

#[test]
fn polygon_mesh_quality_fortran_indexed_rejects_bad_compact_cache_length() {
    let cell_points = vec![LonLatDegrees::new(0.0, 0.0); 6];
    let cells_on_polygon = vec![vec![], vec![], vec![2, 3, 4, 5]];
    let polygon_edge_counts = vec![0, 0, 4];
    let adjust_flags = vec![false, false, false];
    let length_cache: Vec<Vec<f64>> = Vec::new();
    let angle_cache: Vec<Vec<f64>> = Vec::new();

    assert!(polygon_mesh_quality_fortran_indexed(
        4,
        &cell_points,
        &cells_on_polygon,
        &polygon_edge_counts,
        &adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .is_none());
}

fn ring_lonlat(center_lon: f64, center_lat: f64, radius: f64, count: usize) -> Vec<LonLatDegrees> {
    (0..count)
        .map(|index| {
            let theta = std::f64::consts::TAU * index as f64 / count as f64;
            LonLatDegrees::new(
                center_lon + radius * theta.cos(),
                center_lat + radius * theta.sin(),
            )
        })
        .collect()
}

#[test]
fn grid_quality_global_wrapper_matches_direct_quality_calls() {
    let triangle_cell_points = vec![
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(1.0, 0.0),
        LonLatDegrees::new(0.0, 1.0),
        LonLatDegrees::new(1.0, 1.0),
    ];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [3, 5, 4]];

    let mut polygon_points = vec![LonLatDegrees::new(0.0, 0.0); 20];
    for (slot, point) in (2..=6).zip(ring_lonlat(10.0, 0.0, 0.25, 5)) {
        polygon_points[slot] = point;
    }
    for (slot, point) in (7..=12).zip(ring_lonlat(20.0, 0.0, 0.25, 6)) {
        polygon_points[slot] = point;
    }
    for (slot, point) in (13..=19).zip(ring_lonlat(30.0, 0.0, 0.25, 7)) {
        polygon_points[slot] = point;
    }
    let cells_on_polygon = vec![
        vec![],
        vec![],
        vec![2, 3, 4, 5, 6],
        vec![7, 8, 9, 10, 11, 12],
        vec![13, 14, 15, 16, 17, 18, 19],
    ];
    let polygon_edge_counts = vec![0, 0, 5, 6, 7];

    let output = grid_quality_check_global_fortran_indexed(
        &triangle_cell_points,
        &cells_on_triangle,
        &polygon_points,
        &cells_on_polygon,
        &polygon_edge_counts,
    )
    .expect("valid global quality inputs");

    let triangle_adjust = vec![true; cells_on_triangle.len()];
    let triangle_lengths = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle_angles = vec![[0.0; 3]; cells_on_triangle.len()];
    let expected_triangle = triangle_mesh_quality_fortran_indexed(
        &triangle_cell_points,
        &cells_on_triangle,
        &triangle_adjust,
        &triangle_lengths,
        &triangle_angles,
    )
    .expect("direct triangle quality");
    let polygon_adjust = vec![true; cells_on_polygon.len()];
    let expected_pentagon = polygon_mesh_quality_fortran_indexed(
        5,
        &polygon_points,
        &cells_on_polygon,
        &polygon_edge_counts,
        &polygon_adjust,
        &[vec![0.0; 5]],
        &[vec![0.0; 5]],
    )
    .expect("direct pentagon quality");

    assert_eq!(output.edge_class_counts.pentagons, 1);
    assert_eq!(output.edge_class_counts.hexagons, 1);
    assert_eq!(output.edge_class_counts.heptagons, 1);
    assert_eq!(output.edge_class_counts.less_than_five, 0);
    assert_eq!(output.edge_class_counts.greater_than_seven, 0);
    assert_eq!(output.triangle.length_cache, expected_triangle.length_cache);
    assert_eq!(
        output.triangle.angle_less_flags,
        expected_triangle.angle_less_flags
    );
    assert_eq!(
        output.pentagon.expect("pentagon quality").angle_cache,
        expected_pentagon.angle_cache
    );
    assert!(output.hexagon.is_some());
    assert!(output.heptagon.is_some());
}

#[test]
fn get_area_production_wrapper_includes_reconstruction_error_summary() {
    let zero = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let vertex = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
    let cell2 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.5, 0.0));
    let cell3 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.5));
    let cell4 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.5, 0.5));
    let edge2 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.25, 0.0));
    let edge3 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.25, 0.5));
    let edge4 = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.5, 0.25));

    let vertices = vec![zero, zero, vertex];
    let edge_points = vec![zero, zero, edge2, edge3, edge4];
    let cell_points = vec![zero, zero, cell2, cell3, cell4];
    let cells_on_vertex = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];
    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 3], [3, 4], [4, 2]];
    let vertices_on_cell = vec![vec![], vec![]];

    let output = get_area_production_fortran_indexed(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cell_points,
        cells_on_vertex: &cells_on_vertex,
        edges_on_vertex: &edges_on_vertex,
        cells_on_edge: &cells_on_edge,
        vertices_on_cell: &vertices_on_cell,
    })
    .expect("valid production GetArea input");

    let expected_error = area_triangle_reconstruction_error_fortran_indexed(
        &output.unit.area_triangle,
        &cell_points,
        &cells_on_vertex,
    )
    .expect("direct reconstruction summary");

    assert!(output.unit.area_triangle[2] > 0.0);
    approx_eq(
        output.reconstruction_error.max_relative,
        expected_error.max_relative,
        1.0e-15,
    );
    approx_eq(
        output.reconstruction_error.avg_relative,
        expected_error.avg_relative,
        1.0e-15,
    );
}
