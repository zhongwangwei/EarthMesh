use earthmesh_mesh::{
    cellwidth_layers_fortran_indexed, distance_layers, dists_on_edge_layers_fortran_indexed,
    find_frac_index_fortran, set_dists_on_edge_global_fortran_indexed, DistanceLayerSpacing,
    GlobalDistanceStep, SetDistsOnEdgeGlobalInput,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn find_frac_index_matches_fortran_ascending_grid() {
    let hit = find_frac_index_fortran(&[-180.0, -90.0, 0.0, 90.0, 180.0], -45.0)
        .expect("point inside ascending grid");

    assert_eq!(hit.index, 2);
    approx_eq(hit.frac, 0.5, 1.0e-15);
}

#[test]
fn find_frac_index_matches_fortran_descending_grid() {
    let hit = find_frac_index_fortran(&[90.0, 45.0, 0.0, -45.0, -90.0], 22.5)
        .expect("point inside descending grid");

    assert_eq!(hit.index, 2);
    approx_eq(hit.frac, 0.5, 1.0e-15);
}

#[test]
fn find_frac_index_clamps_boundary_fraction_like_fortran() {
    let left = find_frac_index_fortran(&[0.0, 10.0, 20.0], 0.0).expect("left boundary");
    assert_eq!(left.index, 1);
    approx_eq(left.frac, 0.0, 1.0e-15);

    let right = find_frac_index_fortran(&[0.0, 10.0, 20.0], 20.0).expect("right boundary");
    assert_eq!(right.index, 2);
    approx_eq(right.frac, 1.0, 1.0e-15);
}

#[test]
fn find_frac_index_rejects_points_outside_grid() {
    assert!(find_frac_index_fortran(&[0.0, 10.0, 20.0], -1.0).is_none());
    assert!(find_frac_index_fortran(&[0.0, 10.0, 20.0], 21.0).is_none());
}

#[test]
fn distance_layers_match_fortran_linear_formula() {
    let layers = distance_layers(4, 100.0, DistanceLayerSpacing::Linear).expect("layers");

    assert_eq!(layers.len(), 4);
    approx_eq(layers[0], 62.5, 1.0e-12);
    approx_eq(layers[1], 75.0, 1.0e-12);
    approx_eq(layers[2], 87.5, 1.0e-12);
    approx_eq(layers[3], 100.0, 1.0e-12);
}

#[test]
fn distance_layers_match_fortran_nonlinear_formulas_at_bounds() {
    let nonlinear1 = distance_layers(4, 100.0, DistanceLayerSpacing::Power).expect("layers");
    let nonlinear2 = distance_layers(4, 100.0, DistanceLayerSpacing::Exponential).expect("layers");
    let nonlinear3 = distance_layers(4, 100.0, DistanceLayerSpacing::Logarithmic).expect("layers");

    approx_eq(nonlinear1[3], 100.0, 1.0e-12);
    approx_eq(nonlinear2[3], 100.0, 1.0e-12);
    approx_eq(nonlinear3[3], 100.0, 1.0e-12);
    assert!(nonlinear1[0] > 50.0 && nonlinear1[0] < 100.0);
    assert!(nonlinear2[0] > 50.0 && nonlinear2[0] < 100.0);
    assert!(nonlinear3[0] > 50.0 && nonlinear3[0] < 100.0);
}

#[test]
fn distance_layers_reject_zero_length_output() {
    assert!(distance_layers(0, 100.0, DistanceLayerSpacing::Linear).is_none());
}

#[test]
fn dists_on_edge_layers_marks_refined_and_first_halo_edges_like_fortran() {
    let initial_dists = vec![100.0; 7];
    let refinement_flags = vec![false, false, true, false, false];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3], vec![3, 4], vec![4]];
    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [4, 5, 6], [6, 0, 0]];
    let cells_on_edge = vec![[0, 0], [0, 0], [0, 0], [0, 0], [2, 3], [2, 3], [3, 4]];
    let dist_layers = vec![40.0, 80.0];

    let updated = dists_on_edge_layers_fortran_indexed(
        1,
        1,
        0,
        1,
        &triangles_on_cell,
        &edges_on_vertex,
        &cells_on_edge,
        &dist_layers,
        &refinement_flags,
        &initial_dists,
    )
    .expect("valid edge layer inputs");

    assert_eq!(updated[0], 100.0);
    assert_eq!(updated[1], 100.0);
    assert_eq!(updated[2], 40.0);
    assert_eq!(updated[3], 40.0);
    assert_eq!(updated[4], 40.0);
    assert_eq!(updated[5], 40.0);
    assert_eq!(updated[6], 80.0);
}

#[test]
fn dists_on_edge_layers_rejects_short_dist_layer_array() {
    let initial_dists = vec![100.0; 3];
    let refinement_flags = vec![false, false, true];
    let triangles_on_cell = vec![vec![], vec![], vec![2]];
    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [2, 0, 0]];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 2]];

    assert!(dists_on_edge_layers_fortran_indexed(
        1,
        1,
        0,
        1,
        &triangles_on_cell,
        &edges_on_vertex,
        &cells_on_edge,
        &[40.0],
        &refinement_flags,
        &initial_dists,
    )
    .is_none());
}

#[test]
fn cellwidth_layers_marks_refined_and_first_halo_cells_like_fortran() {
    let initial_cellwidth = vec![100.0; 7];
    let refinement_flags = vec![false, false, true, false];
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3], vec![2], vec![2], vec![], vec![]];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [2, 5, 6]];
    let dist_layers = vec![40.0];

    let updated = cellwidth_layers_fortran_indexed(
        1,
        1,
        0,
        1,
        &cells_on_triangle,
        &triangles_on_cell,
        &dist_layers,
        &refinement_flags,
        &initial_cellwidth,
    )
    .expect("valid cellwidth layer inputs");

    assert_eq!(updated[0], 100.0);
    assert_eq!(updated[1], 100.0);
    assert_eq!(updated[2], 20.0);
    assert_eq!(updated[3], 20.0);
    assert_eq!(updated[4], 20.0);
    assert_eq!(updated[5], 40.0);
    assert_eq!(updated[6], 40.0);
}

#[test]
fn cellwidth_layers_rejects_short_dist_layer_array() {
    let initial_cellwidth = vec![100.0; 3];
    let refinement_flags = vec![false, false, true];
    let triangles_on_cell = vec![vec![], vec![], vec![2]];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 2, 2]];

    assert!(cellwidth_layers_fortran_indexed(
        1,
        1,
        0,
        1,
        &cells_on_triangle,
        &triangles_on_cell,
        &[],
        &refinement_flags,
        &initial_cellwidth,
    )
    .is_none());
}

#[test]
fn set_dists_on_edge_global_wrapper_matches_manual_layer_sequence() {
    let triangles_on_cell = vec![vec![], vec![], vec![2, 3], vec![3, 4], vec![4]];
    let cells_on_triangle = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [2, 3, 4], [3, 4, 4]];
    let edges_on_vertex = vec![[0, 0, 0], [0, 0, 0], [2, 3, 4], [4, 5, 6], [6, 0, 0]];
    let cells_on_edge = vec![[0, 0], [0, 0], [0, 0], [0, 0], [2, 3], [2, 3], [3, 4]];
    let refinement_flags = vec![false, false, true, false, false];
    let steps = vec![GlobalDistanceStep {
        active: true,
        halo: 1,
        refinement_flags: &refinement_flags,
        num_vertex_in: 1,
        num_center_in: 1,
    }];

    let output = set_dists_on_edge_global_fortran_indexed(SetDistsOnEdgeGlobalInput {
        base_dists_on_edge: 100.0,
        base_cellwidth: Some(200.0),
        num_rc: 0,
        spacing: DistanceLayerSpacing::Linear,
        triangles_on_cell: &triangles_on_cell,
        cells_on_triangle: Some(&cells_on_triangle),
        edges_on_vertex: &edges_on_vertex,
        cells_on_edge: &cells_on_edge,
        steps: &steps,
    })
    .expect("valid set_distsOnEdge_global input");

    let edge_layers = distance_layers(2, 100.0, DistanceLayerSpacing::Linear).expect("edge layers");
    let expected_edges = dists_on_edge_layers_fortran_indexed(
        1,
        1,
        0,
        1,
        &triangles_on_cell,
        &edges_on_vertex,
        &cells_on_edge,
        &edge_layers,
        &refinement_flags,
        &vec![100.0; cells_on_edge.len()],
    )
    .expect("manual edge layers");
    let cell_layers =
        distance_layers(1, 200.0, DistanceLayerSpacing::Linear).expect("cellwidth layers");
    let expected_cellwidth = cellwidth_layers_fortran_indexed(
        1,
        1,
        0,
        1,
        &cells_on_triangle,
        &triangles_on_cell,
        &cell_layers,
        &refinement_flags,
        &vec![200.0; triangles_on_cell.len()],
    )
    .expect("manual cellwidth layers");

    assert_eq!(output.dists_on_edge, expected_edges);
    assert_eq!(
        output.cellwidth.expect("cellwidth output"),
        expected_cellwidth
    );
}
