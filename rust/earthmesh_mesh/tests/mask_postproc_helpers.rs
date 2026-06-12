use earthmesh_mesh::{extract_unique_vertices_fortran_indexed, sort_and_reindex_vertices};

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
