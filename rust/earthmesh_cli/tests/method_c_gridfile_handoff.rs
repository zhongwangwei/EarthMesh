use earthmesh_cli::{
    mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state,
    mode_file_io::write_gridfile_from_one_based_state,
    unstructured_mesh_io::read_unstructured_mesh_netcdf,
};
use earthmesh_core::{GridMemory, IjTabs};

fn temp_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn method_c_one_based_state() -> (GridMemory, IjTabs) {
    let mut grid = GridMemory {
        nma: 7,
        nua: 0,
        nva: 0,
        nwa: 3,
        mma: 7,
        mua: 0,
        mva: 0,
        mwa: 3,
        ..GridMemory::default()
    };
    grid.allocate_grid_lonlatmw(grid.nma + 1, grid.nva + 1, grid.nwa + 1);

    for im in 1..=grid.nma {
        grid.glonm[im] = im as f32;
        grid.glatm[im] = (im as f32) * 2.0;
    }
    for iw in 1..=grid.nwa {
        grid.glonw[iw] = (iw as f32) * 10.0;
        grid.glatw[iw] = (iw as f32) * 20.0;
    }

    let mut tabs = IjTabs::allocate(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for im in 1..=grid.nma {
        tabs.m[im].iw = [1, 2, 3];
    }

    tabs.w[1].npoly = 0;
    tabs.w[1].im = [1, 1, 1, 1, 1, 1, 1];

    tabs.w[2].npoly = 7;
    tabs.w[2].im = [1, 2, 3, 4, 5, 6, 7];

    tabs.w[3].npoly = 3;
    tabs.w[3].im = [2, 4, 6, 7, 7, 7, 7];

    (grid, tabs)
}

#[test]
fn one_based_gridfile_handoff_preserves_explicit_method_c_w_npoly() {
    let (grid, tabs) = method_c_one_based_state();

    let mesh = gridfile_mesh_from_one_based_state(&grid, &tabs).expect("build compact mesh");
    assert_eq!(mesh.n_w_to_m, vec![1, 7, 3]);
    assert_eq!(mesh.w_to_m[1], vec![1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(mesh.w_to_m[2], vec![2, 4, 6, 7, 7, 7, 7]);

    let root = temp_root("method_c_gridfile_handoff");
    let report = write_gridfile_from_one_based_state(&root, 66, 1, "method_c", &grid, &tabs)
        .expect("write compact gridfile");
    let round_trip = read_unstructured_mesh_netcdf(&report.output).expect("read compact gridfile");

    assert_eq!(round_trip.n_w_to_m, vec![1, 7, 3]);

    let file = netcdf::open(&report.output).expect("open compact gridfile");
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![1, 7, 3]
    );

    let _ = std::fs::remove_dir_all(&root);
}
