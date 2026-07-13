#[test]
fn cellwidth_writer_preserves_canonical_schema_and_round_trips_reader() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_cellwidth_writer_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("cellwidth_NXP0009_global.nc4");

    let mesh = earthmesh_cli::mesh_metric_writers::CellwidthMesh {
        cell_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 110.0,
                lat: -5.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 120.0,
                lat: 15.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 130.0,
                lat: 25.0,
            },
        ],
        cellwidth: vec![12.0, 24.0, 48.0],
    };

    let report = earthmesh_cli::mesh_metric_writers::write_cellwidth_netcdf(&output, &mesh)
        .expect("write cellwidth file");

    assert_eq!(report.output, output);
    assert_eq!(report.num_dbx, 3);
    let file = netcdf::open(&report.output).expect("open cellwidth file");
    assert_eq!(file.dimension("num_dbx").expect("num_dbx").len(), 3);
    assert_eq!(read_f64(&file, "lonw"), vec![110.0, 120.0, 130.0]);
    assert_eq!(read_f64(&file, "latw"), vec![-5.0, 15.0, 25.0]);
    assert_eq!(read_f64(&file, "cellwidth"), vec![12.0, 24.0, 48.0]);
    assert_eq!(
        earthmesh_cli::mesh_metric_writers::read_cellwidth_netcdf(&report.output)
            .expect("read cellwidth"),
        vec![12.0, 24.0, 48.0]
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cellwidth_writer_rejects_length_mismatch() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_cellwidth.nc4");
    let mesh = earthmesh_cli::mesh_metric_writers::CellwidthMesh {
        cell_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }],
        cellwidth: vec![1.0, 2.0],
    };

    let err = earthmesh_cli::mesh_metric_writers::write_cellwidth_netcdf(&output, &mesh)
        .expect_err("mismatched cellwidth rejected");
    assert!(err.to_string().contains("cellwidth length"));
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
