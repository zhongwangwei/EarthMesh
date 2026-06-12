use earthmesh_mesh::{
    boundary_closed_curves_fortran_indexed, boundary_connection_fortran_indexed,
    extract_unique_vertices_fortran_indexed, finalize_mask_postproc_data_fortran_indexed,
    renew_mask_postproc_data_fortran_indexed, renew_mask_postproc_domain_triangles_fortran_indexed,
    renew_mask_postproc_opposite_domain_triangles_fortran_indexed, sort_and_reindex_vertices,
    widen_narrow_waterway_fortran_indexed,
};

#[test]
fn extract_unique_vertices_preserves_fortran_placeholder_and_first_seen_order() {
    let center_neighbors = vec![
        vec![1, 1, 1, 1],
        vec![1, 1, 1, 1],
        vec![4, 2, 5, 1],
        vec![2, 4, 6, 1],
        vec![6, 5, 4, 1],
    ];
    let neighbor_counts = vec![0, 0, 3, 3, 2];

    let unique = extract_unique_vertices_fortran_indexed(&center_neighbors, &neighbor_counts, 8)
        .expect("extract unique vertices");

    assert_eq!(unique, vec![1, 4, 2, 5, 6]);
}

#[test]
fn sort_and_reindex_vertices_builds_fortran_old_to_new_mapping() {
    let unique = vec![1, 4, 2, 5, 6];

    let reindexed = sort_and_reindex_vertices(&unique, 8).expect("sort and reindex");

    assert_eq!(reindexed.sorted_vertices, vec![1, 2, 4, 5, 6]);
    assert_eq!(reindexed.vertex_mapping[1], 1);
    assert_eq!(reindexed.vertex_mapping[2], 2);
    assert_eq!(reindexed.vertex_mapping[4], 3);
    assert_eq!(reindexed.vertex_mapping[5], 4);
    assert_eq!(reindexed.vertex_mapping[6], 5);
    assert_eq!(reindexed.vertex_mapping[3], 0);
}

#[test]
fn data_renew_compacts_active_centers_but_keeps_original_center_ids_for_vertices() {
    let active_centers = vec![false, false, true, false, true];
    let center_neighbors = vec![
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![2, 3, 4],
        vec![4, 5, 6],
        vec![2, 6, 5],
    ];
    let center_neighbor_counts = vec![0, 0, 3, 3, 3];

    let renewed = renew_mask_postproc_data_fortran_indexed(
        "tri",
        &active_centers,
        &center_neighbors,
        &center_neighbor_counts,
        6,
    )
    .expect("renew data");

    assert_eq!(renewed.points_next, 3);
    assert_eq!(renewed.bounds_next, 6);
    assert_eq!(renewed.center_neighbors_next[1], vec![1, 1, 1]);
    assert_eq!(renewed.center_neighbors_next[2], vec![2, 3, 4]);
    assert_eq!(renewed.center_neighbors_next[3], vec![2, 6, 5]);
    assert_eq!(renewed.center_neighbor_counts_next, vec![0, 0, 3, 3]);
    assert_eq!(renewed.vertex_neighbor_counts_next[2], 2);
    assert_eq!(renewed.vertex_neighbors_next[2][0..2], [2, 4]);
    assert_eq!(renewed.vertex_neighbor_counts_next[3], 1);
    assert_eq!(renewed.vertex_neighbors_next[3][0], 2);
    assert_eq!(renewed.vertex_neighbor_counts_next[5], 1);
    assert_eq!(renewed.vertex_neighbors_next[5][0], 4);
}

#[test]
fn data_finial_compacts_centers_and_vertices_using_compact_center_ids() {
    let active_centers = vec![false, false, true, false, true];
    let center_coordinates = vec![
        [0.0, 0.0],
        [10.0, 10.0],
        [20.0, 20.5],
        [30.0, 30.5],
        [40.0, 40.5],
    ];
    let vertex_coordinates = vec![
        [0.0, 0.0],
        [1.0, 1.0],
        [2.0, 2.5],
        [3.0, 3.5],
        [4.0, 4.5],
        [5.0, 5.5],
        [6.0, 6.5],
    ];
    let center_neighbors = vec![
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![2, 3, 4],
        vec![4, 5, 6],
        vec![2, 6, 5],
    ];
    let center_neighbor_counts = vec![0, 0, 3, 3, 3];

    let final_data = finalize_mask_postproc_data_fortran_indexed(
        "tri",
        &active_centers,
        &center_coordinates,
        &vertex_coordinates,
        &center_neighbors,
        &center_neighbor_counts,
        6,
    )
    .expect("finalize data");

    assert_eq!(final_data.points_final, 3);
    assert_eq!(final_data.bounds_final, 6);
    assert_eq!(final_data.center_coordinates_final[1], [0.0, 0.0]);
    assert_eq!(final_data.center_coordinates_final[2], [20.0, 20.5]);
    assert_eq!(final_data.center_coordinates_final[3], [40.0, 40.5]);
    assert_eq!(final_data.vertex_coordinates_final[1], [0.0, 0.0]);
    assert_eq!(final_data.vertex_coordinates_final[2], [2.0, 2.5]);
    assert_eq!(final_data.vertex_coordinates_final[3], [3.0, 3.5]);
    assert_eq!(final_data.vertex_coordinates_final[4], [4.0, 4.5]);
    assert_eq!(final_data.vertex_coordinates_final[5], [5.0, 5.5]);
    assert_eq!(final_data.vertex_coordinates_final[6], [6.0, 6.5]);
    assert_eq!(final_data.center_neighbors_final[2], vec![2, 3, 4]);
    assert_eq!(final_data.center_neighbors_final[3], vec![2, 6, 5]);
    assert_eq!(final_data.center_neighbor_counts_final, vec![0, 0, 3, 3]);
    assert_eq!(
        final_data.vertex_neighbor_counts_final,
        vec![0, 0, 2, 1, 1, 1, 1]
    );
    assert_eq!(final_data.vertex_neighbors_final[2][0..2], [2, 3]);
    assert_eq!(final_data.vertex_neighbors_final[3][0], 2);
    assert_eq!(final_data.vertex_neighbors_final[5][0], 3);
}

#[test]
fn domain_triangle_renew_deletes_solid_boundary_triangles_and_refills_one_missing_vertex() {
    let mut is_in_domain = vec![0, -1, 1, 1, 1, -1, -1];
    let original_vertex_neighbors = vec![
        vec![1, 1, 1, 1, 1, 1, 1],
        vec![1, 1, 1, 1, 1, 1, 1],
        vec![2, 3, 4, 1, 1, 1, 1],
        vec![2, 4, 6, 1, 1, 1, 1],
        vec![2, 3, 4, 6, 1, 1, 1],
        vec![5, 6, 1, 1, 1, 1, 1],
    ];
    let renewed_vertex_neighbors = vec![
        vec![1, 1, 1, 1, 1, 1, 1],
        vec![1, 1, 1, 1, 1, 1, 1],
        vec![2, 1, 1, 1, 1, 1, 1],
        vec![2, 1, 1, 1, 1, 1, 1],
        vec![2, 1, 1, 1, 1, 1, 1],
        vec![5, 1, 1, 1, 1, 1, 1],
    ];
    let original_counts = vec![0, 0, 3, 3, 4, 2];
    let renewed_counts = vec![0, 0, 1, 1, 1, 1];
    let mut points_new = 4;

    renew_mask_postproc_domain_triangles_fortran_indexed(
        &mut is_in_domain,
        &original_vertex_neighbors,
        &renewed_vertex_neighbors,
        &original_counts,
        &renewed_counts,
        &mut points_new,
    )
    .expect("renew domain triangles");

    assert_eq!(is_in_domain, vec![0, -1, -1, 1, 1, 1, 1]);
    assert_eq!(points_new, 4);
}

#[test]
fn opposite_domain_triangle_renew_refills_two_opposed_missing_triangles() {
    let mut is_in_domain = vec![0, -1, -1, 1, 1, -1, 1, 1];
    let vertex_neighbors = vec![
        vec![1, 1, 1, 1, 1, 1],
        vec![1, 1, 1, 1, 1, 1],
        vec![2, 3, 4, 5, 6, 7],
    ];
    let original_counts = vec![0, 0, 6];
    let renewed_counts = vec![0, 0, 4];
    let mut points_new = 5;

    renew_mask_postproc_opposite_domain_triangles_fortran_indexed(
        &mut is_in_domain,
        &vertex_neighbors,
        &original_counts,
        &renewed_counts,
        &mut points_new,
    )
    .expect("renew opposite domain triangles");

    assert_eq!(is_in_domain, vec![0, -1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(points_new, 7);
}

#[test]
fn narrow_waterway_widen_activates_cells_around_duplicate_boundary_neighbor() {
    let mut is_in_domain = vec![0, -1, 1, 1, 1, 1, -1, -1, -1, -1, -1];
    let vertex_neighbors = vec![
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![9, 10, 1],
        vec![1, 1, 1],
        vec![1, 1, 1],
    ];
    let center_neighbors_new = vec![
        vec![1, 1, 1],
        vec![1, 1, 1],
        vec![3, 5, 1],
        vec![3, 5, 1],
        vec![3, 6, 1],
        vec![3, 7, 1],
    ];
    let vertex_neighbor_counts = vec![0, 0, 0, 3, 0, 3, 3, 3];
    let vertex_neighbor_counts_new = vec![0, 0, 0, 1, 0, 1, 1, 1];
    let center_neighbor_counts_new = vec![0, 0, 2, 2, 2, 2];

    widen_narrow_waterway_fortran_indexed(
        &mut is_in_domain,
        &vertex_neighbors,
        &center_neighbors_new,
        &vertex_neighbor_counts,
        &vertex_neighbor_counts_new,
        &center_neighbor_counts_new,
    )
    .expect("widen narrow waterway");

    assert_eq!(is_in_domain[9], 1);
    assert_eq!(is_in_domain[10], 1);
}

#[test]
fn boundary_closed_curves_preserve_fortran_walk_order_and_longest_metadata() {
    let boundary_order = vec![1, 10, 11, 12, 20, 21, 22, 23];
    let mut boundary_neighbors = vec![vec![1, 1]; 24];
    boundary_neighbors[10] = vec![11, 12];
    boundary_neighbors[11] = vec![12, 10];
    boundary_neighbors[12] = vec![10, 11];
    boundary_neighbors[20] = vec![21, 23];
    boundary_neighbors[21] = vec![22, 20];
    boundary_neighbors[22] = vec![23, 21];
    boundary_neighbors[23] = vec![20, 22];

    let curves = boundary_closed_curves_fortran_indexed(&boundary_order, &boundary_neighbors)
        .expect("closed curves");

    assert_eq!(curves.num_closed_curve, 2);
    assert_eq!(curves.num_bdy_long, [5, 1, 2]);
    assert_eq!(curves.close_curves[1], vec![10, 11, 12]);
    assert_eq!(curves.close_curves[2], vec![20, 21, 22, 23]);
    assert_eq!(curves.n_close_curve, vec![0, 3, 4]);
}

#[test]
fn boundary_connection_builds_boundary_graph_and_closed_curve_from_center_edges() {
    let center_neighbors_new = vec![
        vec![1, 1],
        vec![1, 1],
        vec![10, 11],
        vec![11, 12],
        vec![12, 13],
        vec![13, 10],
    ];
    let center_neighbor_counts_new = vec![0, 0, 2, 2, 2, 2];
    let mut vertex_neighbor_counts = vec![0; 14];
    let mut vertex_neighbor_counts_new = vec![0; 14];
    for vertex_id in 10..=13 {
        vertex_neighbor_counts[vertex_id] = 3;
        vertex_neighbor_counts_new[vertex_id] = 1;
    }

    let boundary = boundary_connection_fortran_indexed(
        &center_neighbors_new,
        &center_neighbor_counts_new,
        &vertex_neighbor_counts,
        &vertex_neighbor_counts_new,
    )
    .expect("boundary connection");

    assert_eq!(boundary.bdy_num_in, 5);
    assert_eq!(boundary.boundary_order, vec![1, 10, 11, 12, 13]);
    assert_eq!(boundary.boundary_neighbors[10], vec![11, 13]);
    assert_eq!(boundary.boundary_neighbors[11], vec![10, 12]);
    assert_eq!(boundary.curves.num_closed_curve, 1);
    assert_eq!(boundary.curves.num_bdy_long, [5, 1, 1]);
    assert_eq!(boundary.curves.close_curves[1], vec![10, 11, 12, 13]);
}
