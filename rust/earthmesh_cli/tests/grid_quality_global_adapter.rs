#[test]
fn grid_quality_global_adapter_writes_mesh_output_to_canonical_quality_schema() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_grid_quality_global_adapter_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("quality_from_grid.nc4");

    let grid_quality = earthmesh_mesh::GridQualityGlobalOutput {
        edge_class_counts: earthmesh_mesh::PolygonEdgeClassCounts {
            pentagons: 1,
            hexagons: 1,
            heptagons: 1,
            less_than_five: 0,
            greater_than_seven: 0,
        },
        triangle: earthmesh_mesh::TriangleMeshQualityCanonicalOutput {
            length_cache: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [3.0, 4.0, 5.0]],
            angle_cache: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [50.0, 60.0, 70.0]],
            extreme_angles_degrees: (50.0, 70.0),
            average_min_max_angles_degrees: (50.0, 70.0),
            angle_stddev_degrees: 8.16,
            angle_less_flags: vec![false, false, true],
            angle_more_flags: vec![false, false, false],
        },
        pentagon: Some(earthmesh_mesh::PolygonMeshQualityCanonicalOutput {
            length_cache: vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]],
            angle_cache: vec![vec![100.0, 105.0, 108.0, 110.0, 115.0]],
            extreme_angles_degrees: (100.0, 115.0),
            average_min_max_angles_degrees: (100.0, 115.0),
            angle_stddev_degrees: 5.0,
            angle_less_flags: vec![false],
            angle_more_flags: vec![true],
        }),
        hexagon: Some(earthmesh_mesh::PolygonMeshQualityCanonicalOutput {
            length_cache: vec![vec![6.0, 7.0, 8.0, 9.0, 10.0, 11.0]],
            angle_cache: vec![vec![110.0, 115.0, 120.0, 125.0, 130.0, 135.0]],
            extreme_angles_degrees: (110.0, 135.0),
            average_min_max_angles_degrees: (110.0, 135.0),
            angle_stddev_degrees: 6.0,
            angle_less_flags: vec![false],
            angle_more_flags: vec![true],
        }),
        heptagon: Some(earthmesh_mesh::PolygonMeshQualityCanonicalOutput {
            length_cache: vec![vec![12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0]],
            angle_cache: vec![vec![120.0, 125.0, 128.0, 130.0, 132.0, 135.0, 138.0]],
            extreme_angles_degrees: (120.0, 138.0),
            average_min_max_angles_degrees: (120.0, 138.0),
            angle_stddev_degrees: 7.0,
            angle_less_flags: vec![false],
            angle_more_flags: vec![true],
        }),
    };

    let report = earthmesh_cli::grid_quality_pipeline::write_grid_quality_global_netcdf(
        &output,
        &grid_quality,
    )
    .expect("write grid quality output");

    assert_eq!(report.output, output);
    assert_eq!(report.num_sjx, 3);
    assert_eq!(report.num_wbx, 1);
    assert_eq!(report.num_lbx, 1);
    assert_eq!(report.num_qbx, 1);

    let file = netcdf::open(&report.output).expect("open quality file");
    assert_eq!(
        read_f64(&file, "length_sjx"),
        vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 4.0, 5.0]
    );
    assert_eq!(read_f64(&file, "Extr_wbx"), vec![100.0, 115.0]);
    assert_eq!(read_f64(&file, "Savg_lbx"), vec![6.0]);
    assert_eq!(read_i32(&file, "less_sjx"), vec![0, 0, 1]);
    assert_eq!(read_i32(&file, "more_qbx"), vec![1]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn grid_quality_global_adapter_omits_absent_heptagon_quality() {
    let mesh = earthmesh_cli::grid_quality_pipeline::global_quality_mesh_from_grid_quality(
        &earthmesh_mesh::GridQualityGlobalOutput {
            edge_class_counts: earthmesh_mesh::PolygonEdgeClassCounts {
                pentagons: 0,
                hexagons: 0,
                heptagons: 0,
                less_than_five: 0,
                greater_than_seven: 0,
            },
            triangle: earthmesh_mesh::TriangleMeshQualityCanonicalOutput {
                length_cache: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [3.0, 4.0, 5.0]],
                angle_cache: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [50.0, 60.0, 70.0]],
                extreme_angles_degrees: (50.0, 70.0),
                average_min_max_angles_degrees: (50.0, 70.0),
                angle_stddev_degrees: 8.16,
                angle_less_flags: vec![false, false, false],
                angle_more_flags: vec![false, false, true],
            },
            pentagon: None,
            hexagon: None,
            heptagon: None,
        },
    );

    assert_eq!(mesh.sjx.more, vec![0, 0, 1]);
    assert_eq!(mesh.wbx.length.len(), 0);
    assert_eq!(mesh.lbx.angle.len(), 0);
    assert!(mesh.qbx.is_none());
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn read_i32(file: &netcdf::File, name: &str) -> Vec<i32> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<i32, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
