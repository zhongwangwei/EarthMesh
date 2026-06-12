use earthmesh_mesh::{
    extract_unique_vertices_fortran_indexed, finalize_mask_postproc_data_fortran_indexed,
    renew_mask_postproc_data_fortran_indexed, sort_and_reindex_vertices,
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
