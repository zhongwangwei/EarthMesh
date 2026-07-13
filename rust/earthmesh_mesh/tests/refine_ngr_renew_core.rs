use earthmesh_mesh::{refine_ngr_renew_core_one_based, LonLatDegrees};

fn ll(lon: f64, lat: f64) -> LonLatDegrees {
    LonLatDegrees::new(lon, lat)
}

#[test]
fn ngr_renew_core_deduplicates_new_vertices_compacts_triangles_and_maps_boundaries() {
    let num_mp = vec![0, 3, 6];
    let num_wp = vec![0, 4, 7];
    let mut triangle_points = vec![ll(0.0, 0.0); 7];
    triangle_points[1] = ll(-1.0, -1.0);
    triangle_points[2] = ll(1.0, 1.0);
    triangle_points[3] = ll(2.0, 2.0);
    triangle_points[4] = ll(4.0, 4.0);
    triangle_points[5] = ll(5.0, 5.0);
    triangle_points[6] = ll(6.0, 6.0);
    let mut cell_points = vec![ll(0.0, 0.0); 8];
    for id in 1..=4 {
        cell_points[id] = ll(id as f64, 0.0);
    }
    cell_points[5] = ll(10.0, 1.0);
    cell_points[6] = ll(10.0, 1.0);
    cell_points[7] = ll(11.0, 2.0);
    let mut cells_on_triangle = vec![[0, 0, 0]; 7];
    cells_on_triangle[1] = [1, 2, 3];
    cells_on_triangle[2] = [2, 3, 4];
    cells_on_triangle[3] = [3, 4, 2];
    cells_on_triangle[4] = [2, 5, 6];
    cells_on_triangle[5] = [1, 1, 1];
    cells_on_triangle[6] = [4, 6, 7];
    let bdy_refine = vec![5, 6, 7];
    let bdy_refine_tran = vec![6, 7];

    let renewed = refine_ngr_renew_core_one_based(
        2,
        1,
        &num_mp,
        &num_wp,
        &triangle_points,
        &cell_points,
        &cells_on_triangle,
        &bdy_refine,
        &bdy_refine_tran,
    )
    .expect("NGR_RENEW pure core");

    assert_eq!(renewed.num_dbx, 6);
    assert_eq!(renewed.vertex_mapping[5], 5);
    assert_eq!(renewed.vertex_mapping[6], 5);
    assert_eq!(renewed.vertex_mapping[7], 6);
    assert_eq!(renewed.num_sjx, 5);
    assert_eq!(renewed.cells_on_triangle[4], [2, 5, 5]);
    assert_eq!(renewed.cells_on_triangle[5], [4, 5, 6]);
    assert_eq!(renewed.triangle_points[5], ll(6.0, 6.0));
    assert_eq!(renewed.boundary_refine, vec![5, 5, 6]);
    assert_eq!(renewed.boundary_refine_transition, vec![5, 6]);
    assert_eq!(renewed.n_triangles_on_cell[5], 3);
    assert_eq!(renewed.triangles_on_cell[5], vec![4, 4, 5]);
}

#[test]
fn ngr_renew_core_rejects_triangle_vertex_without_mapping() {
    let num_mp = vec![0, 1, 2];
    let num_wp = vec![0, 2, 2];
    let triangle_points = vec![ll(0.0, 0.0); 3];
    let cell_points = vec![ll(0.0, 0.0); 3];
    let cells_on_triangle = vec![[0, 0, 0], [1, 2, 2], [2, 2, 9]];

    let err = refine_ngr_renew_core_one_based(
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
    .expect_err("triangle vertex must have a final mapping");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
