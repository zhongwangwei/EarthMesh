use std::fs;

use earthmesh_core::{GridMemory, IjTabs, ItabM, ItabW};

#[test]
fn gridfile_unstructured_writer_matches_fortran_schema_and_connectivity() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_gridfile_write_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let output = root.join("gridfile_NXP0002_01_hex.nc4");

    let mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 10.0,
                lat: 20.0,
            },
        ],
        w_points: vec![
            earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::LonLatPoint {
                lon: 30.0,
                lat: 40.0,
            },
            earthmesh_cli::LonLatPoint {
                lon: 50.0,
                lat: 60.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 1]],
        w_to_m: vec![vec![1], vec![2, 1, 1, 1, 1], vec![2, 1, 1, 1, 1, 1]],
        n_w_to_m: vec![1, 5, 6],
    };

    let report = earthmesh_cli::write_unstructured_mesh_netcdf(&output, &mesh)
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
fn gridfile_mesh_from_state_derives_fortran_n_ngrwm_rule() {
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

    let mesh = earthmesh_cli::gridfile_mesh_from_state(&grid, &tabs).expect("derive gridfile mesh");

    assert_eq!(
        mesh.m_points[1],
        earthmesh_cli::LonLatPoint {
            lon: 10.0,
            lat: 20.0
        }
    );
    assert_eq!(
        mesh.w_points[2],
        earthmesh_cli::LonLatPoint {
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
fn write_gridfile_from_state_uses_fortran_output_name() {
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

    let report = earthmesh_cli::write_gridfile_from_state(&root, 64, 3, "hex", &grid, &tabs)
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
