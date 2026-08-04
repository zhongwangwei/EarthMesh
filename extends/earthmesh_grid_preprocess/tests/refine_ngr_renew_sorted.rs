use earthmesh_grid_preprocess::refine_ngr_renew_one_based;
use earthmesh_mesh::LonLatDegrees;

fn ll(lon: f64, lat: f64) -> LonLatDegrees {
    LonLatDegrees::new(lon, lat)
}

#[test]
fn ngr_renew_wrapper_sorts_final_cell_triangle_adjacency_like_get_sort_new() {
    let num_mp = vec![0, 1, 5];
    let num_wp = vec![0, 6, 6];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    triangle_points[1] = ll(-1.0, -1.0);
    triangle_points[2] = ll(2.0, 2.0);
    triangle_points[3] = ll(0.0, 0.0);
    triangle_points[4] = ll(1.0, 0.0);
    triangle_points[5] = ll(0.0, 1.0);
    let mut cell_points = vec![ll(0.0, 0.0); 7];
    for id in 1..=6 {
        cell_points[id] = ll(id as f64, 0.0);
    }
    let mut cells_on_triangle = vec![[0, 0, 0]; 6];
    cells_on_triangle[1] = [1, 2, 3];
    cells_on_triangle[2] = [6, 3, 1];
    cells_on_triangle[3] = [2, 3, 4];
    cells_on_triangle[4] = [4, 3, 5];
    cells_on_triangle[5] = [5, 3, 6];

    let renewed = refine_ngr_renew_one_based(
        2,
        1,
        &num_mp,
        &num_wp,
        &triangle_points,
        &cell_points,
        &cells_on_triangle,
        &[],
        &[],
    )
    .expect("sorted NGR_RENEW wrapper");

    assert_eq!(renewed.n_triangles_on_cell[3], 4);
    assert_eq!(renewed.triangles_on_cell[3], vec![3, 4, 5, 2]);
}
