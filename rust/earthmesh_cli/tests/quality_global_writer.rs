#[test]
fn quality_global_writer_preserves_canonical_schema_with_optional_qbx() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_quality_global_writer_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("quality_global.nc4");

    let quality = earthmesh_cli::quality_global_writer::GlobalQualityMesh {
        sjx: earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
            angle: vec![vec![10.0, 20.0, 30.0], vec![40.0, 50.0, 60.0]],
            extr: [1.0, 6.0],
            eavg: [3.0, 4.0],
            savg: 2.5,
            less: vec![0, 1],
            more: vec![2, 3],
        },
        wbx: earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]],
            angle: vec![vec![11.0, 12.0, 13.0, 14.0, 15.0]],
            extr: [1.0, 5.0],
            eavg: [2.0, 4.0],
            savg: 3.0,
            less: vec![4],
            more: vec![5],
        },
        lbx: earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![6.0, 7.0, 8.0, 9.0, 10.0, 11.0]],
            angle: vec![vec![16.0, 17.0, 18.0, 19.0, 20.0, 21.0]],
            extr: [6.0, 11.0],
            eavg: [7.0, 10.0],
            savg: 8.5,
            less: vec![6],
            more: vec![7],
        },
        qbx: Some(earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0]],
            angle: vec![vec![31.0, 32.0, 33.0, 34.0, 35.0, 36.0, 37.0]],
            extr: [21.0, 27.0],
            eavg: [22.0, 26.0],
            savg: 24.0,
            less: vec![8],
            more: vec![9],
        }),
    };

    let report =
        earthmesh_cli::quality_global_writer::write_quality_global_netcdf(&output, &quality)
            .expect("write quality file");

    assert_eq!(report.output, output);
    assert_eq!(report.num_sjx, 2);
    assert_eq!(report.num_wbx, 1);
    assert_eq!(report.num_lbx, 1);
    assert_eq!(report.num_qbx, 1);

    let file = netcdf::open(&report.output).expect("open quality file");
    assert_eq!(file.dimension("num_sjx").expect("num_sjx").len(), 2);
    assert_eq!(file.dimension("num_wbx").expect("num_wbx").len(), 1);
    assert_eq!(file.dimension("num_lbx").expect("num_lbx").len(), 1);
    assert_eq!(file.dimension("num_qbx").expect("num_qbx").len(), 1);
    assert_eq!(file.dimension("two").expect("two").len(), 2);
    assert_eq!(file.dimension("thr").expect("thr").len(), 3);
    assert_eq!(file.dimension("fiv").expect("fiv").len(), 5);
    assert_eq!(file.dimension("six").expect("six").len(), 6);
    assert_eq!(file.dimension("sev").expect("sev").len(), 7);

    assert_eq!(
        read_f64(&file, "length_sjx"),
        vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
    );
    assert_eq!(
        read_f64(&file, "angle_wbx"),
        vec![11.0, 12.0, 13.0, 14.0, 15.0]
    );
    assert_eq!(read_f64(&file, "Extr_lbx"), vec![6.0, 11.0]);
    assert_eq!(read_f64(&file, "Eavg_qbx"), vec![22.0, 26.0]);
    assert_eq!(read_f64(&file, "Savg_qbx"), vec![24.0]);
    assert_eq!(read_i32(&file, "less_sjx"), vec![0, 1]);
    assert_eq!(read_i32(&file, "more_qbx"), vec![9]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn quality_global_writer_omits_qbx_schema_when_absent() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_quality_global_no_qbx_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("quality_global_no_qbx.nc4");

    let quality = minimal_quality(None);
    earthmesh_cli::quality_global_writer::write_quality_global_netcdf(&output, &quality)
        .expect("write quality file");

    let file = netcdf::open(&output).expect("open quality file");
    assert!(file.dimension("num_qbx").is_none());
    assert!(file.dimension("sev").is_none());
    assert!(file.variable("length_qbx").is_none());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn quality_global_writer_rejects_wrong_class_width() {
    let output = std::env::temp_dir().join("earthmesh_cli_bad_quality_global.nc4");
    let mut quality = minimal_quality(None);
    quality.sjx.length[0] = vec![1.0, 2.0];

    let err = earthmesh_cli::quality_global_writer::write_quality_global_netcdf(&output, &quality)
        .expect_err("wrong sjx width rejected");
    assert!(err.to_string().contains("sjx length row 0 width"));
}

fn minimal_quality(
    qbx: Option<earthmesh_cli::quality_global_writer::QualityClassMetrics>,
) -> earthmesh_cli::quality_global_writer::GlobalQualityMesh {
    earthmesh_cli::quality_global_writer::GlobalQualityMesh {
        sjx: earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![1.0, 2.0, 3.0]],
            angle: vec![vec![4.0, 5.0, 6.0]],
            extr: [1.0, 3.0],
            eavg: [2.0, 2.5],
            savg: 2.0,
            less: vec![0],
            more: vec![1],
        },
        wbx: earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]],
            angle: vec![vec![6.0, 7.0, 8.0, 9.0, 10.0]],
            extr: [1.0, 5.0],
            eavg: [2.0, 4.0],
            savg: 3.0,
            less: vec![0],
            more: vec![1],
        },
        lbx: earthmesh_cli::quality_global_writer::QualityClassMetrics {
            length: vec![vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]],
            angle: vec![vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]],
            extr: [1.0, 6.0],
            eavg: [2.0, 5.0],
            savg: 3.5,
            less: vec![0],
            more: vec![1],
        },
        qbx,
    }
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
