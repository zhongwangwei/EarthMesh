use earthmesh_mesh::{
    arc_length_unit_sphere, area_triangle_reconstruction_error_fortran_indexed,
    cells_on_edge_from_neighbor_cells, get_area_unit_fortran_indexed, is_ngrmm,
    lonlat_degrees_to_unit_xyz, next_ccw_edge_candidate_slot, normalize_lon_m180_180,
    normalize_vertex_rotation, shared_cell_for_edge_pair, should_swap_vertices_on_edge,
    spherical_cell_area_from_vertices_unit, spherical_kite_area_unit, spherical_triangle_area_unit,
    vertex_cell_position, CartesianPoint, GetAreaUnitInput, LonLatDegrees,
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
