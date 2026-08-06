use earthmesh_mesh::LonLatDegrees;
use earthmesh_refine_redgreen::get_sort_new_one_based;

fn ll(lon: f64, lat: f64) -> LonLatDegrees {
    LonLatDegrees::new(lon, lat)
}

#[test]
fn get_sort_new_walks_adjacent_triangles_from_first_degree_one_entry() {
    let num_dbx = 2;
    let n_triangles_on_cell = vec![0, 0, 3];
    let mut cells_on_triangle = vec![[0, 0, 0]; 6];
    cells_on_triangle[3] = [10, 11, 12];
    cells_on_triangle[4] = [12, 11, 13];
    cells_on_triangle[5] = [13, 11, 14];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    triangle_points[5] = ll(0.0, 0.0);
    triangle_points[4] = ll(0.0, 1.0);
    triangle_points[3] = ll(1.0, 0.0);
    let mut triangles_on_cell = vec![vec![], vec![], vec![5, 3, 4]];

    get_sort_new_one_based(
        num_dbx,
        &n_triangles_on_cell,
        &cells_on_triangle,
        &triangle_points,
        &mut triangles_on_cell,
    )
    .expect("sort adjacent triangles like GetSortNew");

    assert_eq!(triangles_on_cell[2], vec![5, 4, 3]);
}

#[test]
fn get_sort_new_appends_unreachable_triangles_like_canonical_warning_fallback() {
    let num_dbx = 2;
    let n_triangles_on_cell = vec![0, 0, 3];
    let mut cells_on_triangle = vec![[0, 0, 0]; 7];
    cells_on_triangle[3] = [10, 11, 12];
    cells_on_triangle[4] = [12, 11, 13];
    cells_on_triangle[6] = [20, 21, 22];
    let mut triangle_points = vec![ll(0.0, 0.0); 7];
    triangle_points[3] = ll(0.0, 0.0);
    triangle_points[4] = ll(0.0, 1.0);
    triangle_points[6] = ll(1.0, 0.0);
    let mut triangles_on_cell = vec![vec![], vec![], vec![3, 4, 6]];

    get_sort_new_one_based(
        num_dbx,
        &n_triangles_on_cell,
        &cells_on_triangle,
        &triangle_points,
        &mut triangles_on_cell,
    )
    .expect("disconnected fallback preserves remaining input order");

    assert_eq!(triangles_on_cell[2], vec![3, 4, 6]);
}

#[test]
fn get_sort_new_reverses_clockwise_area_to_counterclockwise_order() {
    let num_dbx = 2;
    let n_triangles_on_cell = vec![0, 0, 3];
    let mut cells_on_triangle = vec![[0, 0, 0]; 6];
    cells_on_triangle[3] = [10, 11, 12];
    cells_on_triangle[4] = [12, 11, 13];
    cells_on_triangle[5] = [13, 11, 14];
    let mut triangle_points = vec![ll(0.0, 0.0); 6];
    triangle_points[3] = ll(0.0, 0.0);
    triangle_points[4] = ll(1.0, 0.0);
    triangle_points[5] = ll(0.0, 1.0);
    let mut triangles_on_cell = vec![vec![], vec![], vec![3, 4, 5]];

    get_sort_new_one_based(
        num_dbx,
        &n_triangles_on_cell,
        &cells_on_triangle,
        &triangle_points,
        &mut triangles_on_cell,
    )
    .expect("clockwise order is reversed");

    assert_eq!(triangles_on_cell[2], vec![5, 4, 3]);
}
