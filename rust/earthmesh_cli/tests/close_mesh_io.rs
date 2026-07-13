use std::fs;

use earthmesh_cli::{
    close_mesh_io::read_close_mesh_netcdf, close_mesh_io::write_close_mesh_netcdf,
    coordinate_types::LonLatPoint,
};

#[test]
fn close_mesh_reader_writer_match_mod_file_preprocess_schema_without_refine_var() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_close_mesh_io_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let output = root.join("mask_patch_close_1_001.nc4");

    let points = vec![
        LonLatPoint {
            lon: 113.1,
            lat: 22.2,
        },
        LonLatPoint {
            lon: 114.3,
            lat: 22.8,
        },
        LonLatPoint {
            lon: 113.9,
            lat: 23.4,
        },
    ];
    write_close_mesh_netcdf(&output, &points).expect("write close_Mesh_Save schema");

    let file = netcdf::open(&output).expect("open close mesh");
    assert_eq!(file.dimension("close_num").unwrap().len(), 3);
    assert_eq!(file.dimension("two").unwrap().len(), 2);
    assert!(file.variable("close_refine").is_none());
    assert_eq!(
        file.variable("close_points")
            .unwrap()
            .get_values::<f64, _>((.., ..))
            .unwrap(),
        vec![113.1, 22.2, 114.3, 22.8, 113.9, 23.4]
    );

    let read_back = read_close_mesh_netcdf(&output).expect("read close_Mesh_Read schema");
    assert_eq!(read_back, points);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn close_mesh_writer_rejects_empty_or_nonfinite_points() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_close_mesh_io_invalid_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let empty_err = write_close_mesh_netcdf(root.join("empty.nc4"), &[])
        .expect_err("empty close mesh should be rejected");
    assert_eq!(empty_err.kind(), std::io::ErrorKind::InvalidInput);

    let bad_err = write_close_mesh_netcdf(
        root.join("bad.nc4"),
        &[LonLatPoint {
            lon: f64::NAN,
            lat: 0.0,
        }],
    )
    .expect_err("non-finite close mesh point should be rejected");
    assert_eq!(bad_err.kind(), std::io::ErrorKind::InvalidInput);

    let _ = fs::remove_dir_all(&root);
}
