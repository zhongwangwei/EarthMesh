use std::fs;

use earthmesh_mesh::{BoundaryClosedCurves, BoundaryConnection, BoundaryOrders};

#[test]
fn obc_writer_preserves_canonical_schema_and_patch_path() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_obc_writer_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    assert_eq!(
        earthmesh_cli::obc_boundary_io::obc_boundary_output_path(&root, false),
        root.join("result/obc.nc4")
    );
    assert_eq!(
        earthmesh_cli::obc_boundary_io::obc_boundary_output_path(&root, true),
        root.join("result/obc_patch.nc4")
    );

    let orders = BoundaryOrders {
        bdy_order: vec![1, 10, 11, 12],
        obc_order: vec![1, 10, 1, 12],
        ibc_order: vec![1, 1, 11, 1],
        rotation_start: Some(2),
    };
    let output = earthmesh_cli::obc_boundary_io::obc_boundary_output_path(&root, true);
    let report = earthmesh_cli::obc_boundary_io::write_obc_boundary_netcdf(&output, &orders)
        .expect("write obc");
    assert_eq!(report.output, output);
    assert_eq!(report.boundary_points, 4);

    let file = netcdf::open(&output).expect("open obc");
    assert_eq!(file.dimension("bdy_num").expect("bdy_num dim").len(), 4);
    assert_eq!(read_i32(&file, "bdy_order"), vec![1_i32, 10, 11, 12]);
    assert_eq!(read_i32(&file, "obc_order"), vec![1_i32, 10, 1, 12]);
    assert_eq!(read_i32(&file, "ibc_order"), vec![1_i32, 1, 11, 1]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn obcv2_writer_preserves_closed_curve_schema_and_patch_path() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_obcv2_writer_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    assert_eq!(
        earthmesh_cli::obc_boundary_io::obcv2_boundary_output_path(&root, false),
        root.join("result/obcv2.nc4")
    );
    assert_eq!(
        earthmesh_cli::obc_boundary_io::obcv2_boundary_output_path(&root, true),
        root.join("result/obcv2_patch.nc4")
    );

    let connection = BoundaryConnection {
        bdy_num_in: 6,
        boundary_order: vec![1, 20, 21, 22, 30, 31],
        boundary_neighbors: vec![vec![1, 1]; 32],
        curves: BoundaryClosedCurves {
            num_closed_curve: 2,
            num_bdy_long: [4, 3, 1],
            close_curves: vec![vec![], vec![20, 21, 22], vec![30, 31]],
            n_close_curve: vec![0, 3, 2],
        },
    };
    let output = earthmesh_cli::obc_boundary_io::obcv2_boundary_output_path(&root, false);
    let report = earthmesh_cli::obc_boundary_io::write_obcv2_boundary_netcdf(&output, &connection)
        .expect("write obcv2");
    assert_eq!(report.output, output);
    assert_eq!(report.longest_curve_slots, 4);
    assert_eq!(report.closed_curves, 2);

    let file = netcdf::open(&output).expect("open obcv2");
    assert_eq!(file.dimension("num1").expect("num1 dim").len(), 4);
    assert_eq!(file.dimension("num2").expect("num2 dim").len(), 2);
    assert_eq!(read_i32(&file, "n_close_curve"), vec![3_i32, 2]);
    assert_eq!(
        read_i32(&file, "close_curve"),
        vec![20_i32, 21, 22, 1, 30, 31, 1, 1]
    );

    let _ = fs::remove_dir_all(&root);
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
