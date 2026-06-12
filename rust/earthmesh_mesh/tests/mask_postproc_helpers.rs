use earthmesh_mesh::{
    extract_unique_vertices_fortran_indexed, renew_mask_postproc_data_fortran_indexed,
    sort_and_reindex_vertices,
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
