use std::fs;

use earthmesh_cli::{
    read_close_mesh_netcdf, run_refine_array_length_calculation_fortran_indexed, LonLatPoint,
};

fn point(lon: f64, lat: f64) -> LonLatPoint {
    LonLatPoint { lon, lat }
}

#[test]
fn array_length_calculation_adapter_computes_halo_and_writes_close_meshes() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_array_calc_adapter_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let sjx_points = 9;
    let lbx_points = 13;
    let mut mrl = vec![0; sjx_points + 1];
    let mut triangle_neighbors = vec![vec![1, 1, 1]; sjx_points + 1];
    let mut cells_on_triangle = vec![[0, 0, 0]; sjx_points + 1];
    let mut triangles_on_cell = vec![Vec::<usize>::new(); lbx_points + 1];
    let mut edge_counts = vec![0usize; lbx_points + 1];
    let mut wp = vec![point(0.0, 0.0); lbx_points + 1];

    for triangle in 2..=9 {
        mrl[triangle] = 1;
    }
    for refined in [6, 7, 8, 9] {
        mrl[refined] = 4;
    }

    triangle_neighbors[2] = vec![6, 3, 5];
    triangle_neighbors[3] = vec![7, 4, 2];
    triangle_neighbors[4] = vec![8, 5, 3];
    triangle_neighbors[5] = vec![9, 2, 4];
    cells_on_triangle[2] = [10, 11, 99];
    cells_on_triangle[3] = [11, 12, 99];
    cells_on_triangle[4] = [12, 13, 99];
    cells_on_triangle[5] = [13, 10, 99];
    cells_on_triangle[6] = [10, 11, 90];
    cells_on_triangle[7] = [11, 12, 91];
    cells_on_triangle[8] = [12, 13, 92];
    cells_on_triangle[9] = [13, 10, 93];

    triangles_on_cell[10] = vec![5, 2, 6];
    triangles_on_cell[11] = vec![2, 3, 7];
    triangles_on_cell[12] = vec![3, 4, 8];
    triangles_on_cell[13] = vec![4, 5, 9];
    for cell in 10..=13 {
        edge_counts[cell] = triangles_on_cell[cell].len();
        wp[cell] = point(100.0 + cell as f64, 20.0 + cell as f64);
    }

    let report = run_refine_array_length_calculation_fortran_indexed(
        &root,
        4,
        1,
        1,
        9,
        sjx_points,
        lbx_points,
        &mrl,
        &triangle_neighbors,
        &cells_on_triangle,
        &triangles_on_cell,
        &edge_counts,
        0,
        &wp,
    )
    .expect("compute Array_length_calculation and write close mesh side effects");

    assert_eq!(
        report.calculation.halo.boundary_refine,
        vec![10, 11, 12, 13]
    );
    assert_eq!(
        report.calculation.halo.boundary_refine_transition,
        Vec::<usize>::new()
    );
    assert_eq!(report.calculation.halo.num_transition_row_triangles, 4);
    assert_eq!(report.calculation.boundary.curves.num_closed_curve, 1);
    assert_eq!(report.close_meshes.mask_patch_ndm, 1);
    assert_eq!(
        report.close_meshes.outputs[0].output,
        root.join("tmpfile/mask_patch_close_4_001.nc4")
    );
    assert_eq!(
        read_close_mesh_netcdf(root.join("tmpfile/mask_patch_close_4_001.nc4")).unwrap(),
        vec![wp[10], wp[11], wp[12], wp[13]]
    );

    let _ = fs::remove_dir_all(&root);
}
