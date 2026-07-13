use std::fs;

use earthmesh_core::{GridMemory, IjTabs, ItabM, ItabW};

#[test]
fn gridfile_unstructured_writer_matches_canonical_schema_and_connectivity() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_gridfile_write_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let output = root.join("gridfile_NXP0002_01_hex.nc4");

    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: 20.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 30.0,
                lat: 40.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 50.0,
                lat: 60.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 1]],
        w_to_m: vec![vec![1], vec![2, 1, 1, 1, 1], vec![2, 1, 1, 1, 1, 1]],
        n_w_to_m: vec![1, 5, 6],
    };

    let report =
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&output, &mesh)
            .expect("write unstructured mesh");
    assert_eq!(report.sjx_points, 2);
    assert_eq!(report.lbx_points, 3);
    assert_eq!(report.dimc, 7);

    let file = netcdf::open(&output).expect("open gridfile");
    assert_eq!(file.dimension("sjx_points").expect("sjx dim").len(), 2);
    assert_eq!(file.dimension("lbx_points").expect("lbx dim").len(), 3);
    assert_eq!(file.dimension("dimb").expect("dimb dim").len(), 3);
    assert_eq!(file.dimension("dimc").expect("dimc dim").len(), 7);
    assert_eq!(
        file.variable("itab_m%iw")
            .expect("itab_m%iw")
            .dimensions()
            .iter()
            .map(|dim| dim.name())
            .collect::<Vec<_>>(),
        vec!["sjx_points", "dimb"]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .dimensions()
            .iter()
            .map(|dim| dim.name())
            .collect::<Vec<_>>(),
        vec!["lbx_points", "dimc"]
    );
    assert_eq!(
        file.variable("GLONM")
            .expect("GLONM")
            .get_values::<f64, _>(..)
            .expect("read GLONM"),
        vec![0.0, 10.0]
    );
    assert_eq!(
        file.variable("GLATM")
            .expect("GLATM")
            .get_values::<f64, _>(..)
            .expect("read GLATM"),
        vec![0.0, 20.0]
    );
    assert_eq!(
        file.variable("GLONW")
            .expect("GLONW")
            .get_values::<f64, _>(..)
            .expect("read GLONW"),
        vec![0.0, 30.0, 50.0]
    );
    assert_eq!(
        file.variable("itab_m%iw")
            .expect("itab_m%iw")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_m%iw"),
        vec![1, 1, 1, 2, 3, 1]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_w%im"),
        vec![1, 0, 0, 0, 0, 0, 0, 2, 1, 1, 1, 1, 0, 0, 2, 1, 1, 1, 1, 1, 0]
    );
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![1, 5, 6]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gridfile_mesh_from_state_derives_canonical_n_ngrwm_rule() {
    let mut grid = GridMemory {
        nma: 2,
        nwa: 3,
        glonm: vec![0.0, 10.0],
        glatm: vec![0.0, 20.0],
        glonw: vec![0.0, 30.0, 50.0],
        glatw: vec![0.0, 40.0, 60.0],
        ..GridMemory::default()
    };
    grid.allocate_grid_lonlatmw(2, 0, 3);
    grid.glonm = vec![0.0, 10.0];
    grid.glatm = vec![0.0, 20.0];
    grid.glonw = vec![0.0, 30.0, 50.0];
    grid.glatw = vec![0.0, 40.0, 60.0];

    let tabs = IjTabs {
        m: vec![
            ItabM {
                iw: [1, 1, 1],
                ..ItabM::default()
            },
            ItabM {
                iw: [2, 3, 1],
                ..ItabM::default()
            },
        ],
        v: Vec::new(),
        w: vec![
            ItabW {
                im: [1, 1, 1, 1, 1, 1, 1],
                ..ItabW::default()
            },
            ItabW {
                im: [2, 1, 1, 1, 1, 1, 1],
                ..ItabW::default()
            },
            ItabW {
                im: [2, 1, 1, 1, 1, 2, 1],
                ..ItabW::default()
            },
        ],
    };

    let mesh =
        earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_state(&grid, &tabs)
            .expect("derive gridfile mesh");

    assert_eq!(
        mesh.m_points[1],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 10.0,
            lat: 20.0
        }
    );
    assert_eq!(
        mesh.w_points[2],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: 50.0,
            lat: 60.0
        }
    );
    assert_eq!(mesh.m_to_w, vec![[1, 1, 1], [2, 3, 1]]);
    assert_eq!(mesh.w_to_m[1], vec![2, 1, 1, 1, 1, 1, 1]);
    assert_eq!(mesh.w_to_m[2], vec![2, 1, 1, 1, 1, 2, 1]);
    assert_eq!(mesh.n_w_to_m, vec![1, 5, 6]);
}

#[test]
fn write_gridfile_from_state_uses_canonical_output_name() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_gridfile_from_state_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);

    let grid = GridMemory {
        nma: 1,
        nwa: 1,
        glonm: vec![0.0],
        glatm: vec![0.0],
        glonw: vec![0.0],
        glatw: vec![0.0],
        ..GridMemory::default()
    };
    let tabs = IjTabs {
        m: vec![ItabM {
            iw: [1, 1, 1],
            ..ItabM::default()
        }],
        v: Vec::new(),
        w: vec![ItabW {
            im: [1, 1, 1, 1, 1, 1, 1],
            ..ItabW::default()
        }],
    };

    let report =
        earthmesh_cli::mode_file_io::write_gridfile_from_state(&root, 64, 3, "hex", &grid, &tabs)
            .expect("write gridfile from state");

    assert_eq!(
        report.output,
        root.join("gridfile/gridfile_NXP0064_03_hex.nc4")
    );
    assert!(report.output.exists());
    assert_eq!(report.sjx_points, 1);
    assert_eq!(report.lbx_points, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gridfile_mesh_from_one_based_state_uses_one_based_slots() {
    let grid = GridMemory {
        nma: 2,
        nwa: 3,
        glonm: vec![-999.0, 0.0, 10.0],
        glatm: vec![-999.0, 0.0, 20.0],
        glonw: vec![-999.0, 0.0, 30.0, 50.0],
        glatw: vec![-999.0, 0.0, 40.0, 60.0],
        ..GridMemory::default()
    };
    let tabs = IjTabs {
        m: vec![
            ItabM {
                iw: [9, 9, 9],
                ..ItabM::default()
            },
            ItabM {
                iw: [1, 1, 1],
                ..ItabM::default()
            },
            ItabM {
                iw: [2, 3, 1],
                ..ItabM::default()
            },
        ],
        v: Vec::new(),
        w: vec![
            ItabW {
                im: [9, 9, 9, 9, 9, 9, 9],
                ..ItabW::default()
            },
            ItabW {
                im: [1, 1, 1, 1, 1, 1, 1],
                ..ItabW::default()
            },
            ItabW {
                im: [2, 1, 1, 1, 1, 1, 1],
                ..ItabW::default()
            },
            ItabW {
                im: [2, 1, 1, 1, 1, 2, 1],
                ..ItabW::default()
            },
        ],
    };

    let mesh = earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &grid, &tabs,
    )
    .expect("derive one-based gridfile mesh");

    assert_eq!(
        mesh.m_points,
        vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: 20.0
            },
        ]
    );
    assert_eq!(
        mesh.w_points,
        vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 30.0,
                lat: 40.0
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 50.0,
                lat: 60.0
            },
        ]
    );
    assert_eq!(mesh.m_to_w, vec![[1, 1, 1], [2, 3, 1]]);
    assert_eq!(mesh.w_to_m[0], vec![1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(mesh.w_to_m[2], vec![2, 1, 1, 1, 1, 2, 1]);
    assert_eq!(mesh.n_w_to_m, vec![1, 5, 6]);
}

#[test]
fn gridfile_mesh_from_one_based_state_rejects_missing_one_based_tail() {
    let grid = GridMemory {
        nma: 2,
        nwa: 1,
        glonm: vec![0.0, 10.0],
        glatm: vec![0.0, 20.0],
        glonw: vec![0.0, 30.0],
        glatw: vec![0.0, 40.0],
        ..GridMemory::default()
    };
    let tabs = IjTabs {
        m: vec![ItabM::default(), ItabM::default()],
        v: Vec::new(),
        w: vec![ItabW::default(), ItabW::default()],
    };

    let err = earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &grid, &tabs,
    )
    .expect_err("missing one-based nma slot should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn write_gridfile_from_one_based_state_uses_canonical_output_name() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_gridfile_one_based_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);

    let grid = GridMemory {
        nma: 1,
        nwa: 1,
        glonm: vec![-999.0, 0.0],
        glatm: vec![-999.0, 0.0],
        glonw: vec![-999.0, 0.0],
        glatw: vec![-999.0, 0.0],
        ..GridMemory::default()
    };
    let tabs = IjTabs {
        m: vec![
            ItabM {
                iw: [9, 9, 9],
                ..ItabM::default()
            },
            ItabM {
                iw: [1, 1, 1],
                ..ItabM::default()
            },
        ],
        v: Vec::new(),
        w: vec![
            ItabW {
                im: [9, 9, 9, 9, 9, 9, 9],
                ..ItabW::default()
            },
            ItabW {
                im: [1, 1, 1, 1, 1, 1, 1],
                ..ItabW::default()
            },
        ],
    };

    let report = earthmesh_cli::mode_file_io::write_gridfile_from_one_based_state(
        &root, 64, 3, "hex", &grid, &tabs,
    )
    .expect("write one-based gridfile from state");

    assert_eq!(
        report.output,
        root.join("gridfile/gridfile_NXP0064_03_hex.nc4")
    );
    assert!(report.output.exists());
    assert_eq!(report.sjx_points, 1);
    assert_eq!(report.lbx_points, 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unstructured_mesh_reader_round_trips_compatibility_gridfile_schema() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_unstructured_reader_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("gridfile.nc4");
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: -1.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 20.0,
                lat: 2.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 30.0,
                lat: 3.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 3]],
        w_to_m: vec![vec![1, 1, 1, 1, 1, 1, 1], vec![1, 2, 1], vec![2, 1, 1]],
        n_w_to_m: vec![1, 3, 3],
    };

    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&output, &mesh)
        .expect("write mesh");
    let read_back = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&output)
        .expect("read mesh");

    assert_eq!(read_back, mesh);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gridfile_writer_round_trips_full_method_c_metadata() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_gridfile_levels_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let output = root.join("gridfile.nc4");
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 10.0,
                lat: -1.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 20.0,
                lat: 2.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 30.0,
                lat: 3.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [1, 2, 3]],
        w_to_m: vec![vec![1, 1, 1, 1, 1, 1, 1], vec![1, 2, 1], vec![2, 1, 1]],
        n_w_to_m: vec![1, 3, 3],
    };
    let m_levels = [0, 2];
    let m_orig = [0, 1];
    let m_ngr = [0, 7];
    let w_levels = [0, 1, 2];
    let w_orig = [0, 1, 1];
    let w_ngr = [0, 8, 9];

    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        &output,
        &mesh,
        earthmesh_cli::unstructured_mesh_support::MethodCGridfileMetadataSlices {
            m_refine_level: Some(&m_levels),
            m_refine_level_orig: Some(&m_orig),
            m_ngr: Some(&m_ngr),
            w_refine_level: Some(&w_levels),
            w_refine_level_orig: Some(&w_orig),
            w_ngr: Some(&w_ngr),
        },
    )
    .expect("write mesh");
    let read_back = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&output)
        .expect("read quality mesh");

    assert_eq!(read_back.m_refine_level, m_levels.to_vec());
    assert_eq!(read_back.m_refine_level_orig, m_orig.to_vec());
    assert_eq!(read_back.m_ngr, m_ngr.to_vec());
    assert_eq!(read_back.w_refine_level, w_levels.to_vec());
    assert_eq!(read_back.w_refine_level_orig, w_orig.to_vec());
    assert_eq!(read_back.w_ngr, w_ngr.to_vec());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn regional_clip_preserves_method_c_metadata_after_inserted_placeholder() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_regional_levels_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let input = root.join("global.nc4");
    let output = root.join("regional.nc4");
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.2 },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.1, lat: 0.1 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.3, lat: 0.1 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.3 },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    };

    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        &input,
        &mesh,
        earthmesh_cli::unstructured_mesh_support::MethodCGridfileMetadataSlices {
            m_refine_level: Some(&[0, 5]),
            m_refine_level_orig: Some(&[0, 3]),
            m_ngr: Some(&[0, 6]),
            w_refine_level: Some(&[0, 7, 8, 9]),
            w_refine_level_orig: Some(&[0, 4, 5, 6]),
            w_ngr: Some(&[0, 10, 11, 12]),
        },
    )
    .expect("write input");
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        &input,
        &output,
        &earthmesh_cli::coordinate_types::GridRegion::Bbox {
            west: -1.0,
            east: 1.0,
            north: 1.0,
            south: -1.0,
        },
        "tri",
    )
    .expect("regional clip");

    assert_eq!(kept, 1);
    let clipped = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&output)
        .expect("read clipped");
    assert_eq!(clipped.m_refine_level, vec![0, 0, 5]);
    assert_eq!(clipped.m_refine_level_orig, vec![0, 0, 3]);
    assert_eq!(clipped.m_ngr, vec![0, 0, 6]);
    assert_eq!(clipped.w_refine_level, vec![0, 0, 7, 8, 9]);
    assert_eq!(clipped.w_refine_level_orig, vec![0, 0, 4, 5, 6]);
    assert_eq!(clipped.w_ngr, vec![0, 0, 10, 11, 12]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn regional_clip_rejects_mismatched_explicit_refine_levels() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_regional_bad_levels_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let input = root.join("global.nc4");
    let output = root.join("regional.nc4");
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.2 },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.1, lat: 0.1 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.3, lat: 0.1 },
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.2, lat: 0.3 },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    };

    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&input, &mesh)
        .expect("write input");
    let err = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile_with_refine_levels(
        &input,
        &output,
        &earthmesh_cli::coordinate_types::GridRegion::Bbox {
            west: -1.0,
            east: 1.0,
            north: 1.0,
            south: -1.0,
        },
        "tri",
        Some(&[0, 1, 2, 3]),
        Some(&[0, 1, 2, 3]),
    )
    .expect_err("bad metadata lengths must fail");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("earthmesh_m_refine_level"));

    let _ = std::fs::remove_dir_all(&root);
}
