use std::fs;

#[test]
fn run_mkgrd_gridinit_global_namelist_writes_initial_gridfile() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_gridinit_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let namelist = root.join("mkgrd.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_gridinit'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(&namelist, &root, 100)
        .expect("run Rust mkgrd gridinit path");

    assert_eq!(report.config.nxp, 1);
    assert_eq!(report.config.mode_grid, "hex");
    assert_eq!(report.workspace_mask.workspace.created_directories.len(), 5);
    assert!(report.workspace_mask.mask_reports.is_empty());
    assert_eq!(report.gridfile.sjx_points, 21);
    assert_eq!(report.gridfile.lbx_points, 13);
    assert_eq!(
        report.gridfile.output,
        root.join("case_gridinit/gridfile/gridfile_NXP0001_01_hex.nc4")
    );
    assert!(report.gridfile.output.exists());
    assert!(root.join("case_gridinit/result/namelist.save").exists());

    let file = netcdf::open(&report.gridfile.output).expect("open written gridfile");
    assert_eq!(file.dimension("sjx_points").expect("sjx_points").len(), 21);
    assert_eq!(file.dimension("lbx_points").expect("lbx_points").len(), 13);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_runs_gridinit_namelist() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_binary_gridinit_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let namelist = root.join("mkgrd.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("100")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gridfile="), "stdout={stdout}");
    assert!(stdout.contains("sjx_points=21"), "stdout={stdout}");
    assert!(root
        .join("case_binary/gridfile/gridfile_NXP0001_01_hex.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn run_mkgrd_gridinit_global_copies_existing_earthmesh_mode_file() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_existing_mode_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let mode_file = root.join("source_mode.nc4");
    let source_mesh = earthmesh_cli::UnstructuredMesh {
        m_points: vec![earthmesh_cli::LonLatPoint { lon: 0.0, lat: 1.0 }],
        w_points: vec![earthmesh_cli::LonLatPoint { lon: 2.0, lat: 3.0 }],
        m_to_w: vec![[1, 1, 1]],
        w_to_m: vec![vec![1, 1, 1, 1, 1, 1, 1]],
        n_w_to_m: vec![1],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&mode_file, &source_mesh)
        .expect("write source EarthMesh mode file");

    let namelist = root.join("mkgrd_existing.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_existing'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.false.\n  NL%niter=0\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
            mode_file.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(&namelist, &root, 100)
        .expect("copy existing EarthMesh mode file");

    assert_eq!(report.gridfile.sjx_points, 1);
    assert_eq!(report.gridfile.lbx_points, 1);
    assert_eq!(
        report.gridfile.output,
        root.join("case_existing/gridfile/gridfile_NXP0001_01_hex.nc4")
    );
    assert_eq!(
        fs::read(&report.gridfile.output).unwrap(),
        fs::read(&mode_file).unwrap()
    );

    let _ = fs::remove_dir_all(&root);
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual} expected={expected} tolerance={tolerance}"
    );
}

#[test]
#[ignore = "NXP64 full Rust gridinit parity writes a large gridfile and takes about two minutes"]
fn run_mkgrd_gridinit_global_matches_fortran_nxp64_gridfile_fixture() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let reference = repo_root.join(
        "cases/ATMOS_hex_N64_refine2_global_LOM67_251027/gridfile/gridfile_NXP0064_01_hex.nc4",
    );
    assert!(
        reference.exists(),
        "missing reference fixture {reference:?}"
    );

    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_nxp64_gridinit_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let namelist = root.join("mkgrd_n64.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_n64'\n  NL%base_dir='{base_dir}'\n  NL%NXP=64\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.false.\n  NL%niter=5000\n  NL%beta=1.0\n  NL%relax=0.035\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
            root.display()
        ),
    )
    .expect("write NXP64 namelist");

    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(&namelist, &root, 100)
        .expect("run Rust NXP64 mkgrd gridinit path");
    assert_eq!(report.gridfile.sjx_points, 81921);
    assert_eq!(report.gridfile.lbx_points, 40963);

    let produced = netcdf::open(&report.gridfile.output).expect("open produced gridfile");
    let expected = netcdf::open(&reference).expect("open reference gridfile");
    assert_eq!(
        produced
            .dimension("sjx_points")
            .expect("produced sjx")
            .len(),
        expected
            .dimension("sjx_points")
            .expect("expected sjx")
            .len()
    );
    assert_eq!(
        produced
            .dimension("lbx_points")
            .expect("produced lbx")
            .len(),
        expected
            .dimension("lbx_points")
            .expect("expected lbx")
            .len()
    );

    for var_name in ["GLONM", "GLATM", "GLONW", "GLATW"] {
        let actual = produced
            .variable(var_name)
            .expect("produced variable")
            .get_values::<f64, _>(..)
            .expect("read produced variable");
        let expected = expected
            .variable(var_name)
            .expect("expected variable")
            .get_values::<f64, _>(..)
            .expect("read expected variable");
        for index in [0usize, 1, 2, 3, 4, actual.len() / 2, actual.len() - 1] {
            let tolerance = if var_name.starts_with("GLO") && expected[index].abs() > 179.9 {
                5.0e-4
            } else {
                2.0e-4
            };
            if var_name.starts_with("GLO") && expected[index].abs() < 89.999 {
                assert_close(actual[index], expected[index], tolerance);
            } else if var_name.starts_with("GLA") {
                assert_close(actual[index], expected[index], tolerance);
            }
        }
    }

    let produced_m = produced
        .variable("itab_m%iw")
        .expect("produced itab_m%iw")
        .get_values::<i32, _>((.., ..))
        .expect("read produced itab_m%iw");
    let expected_m = expected
        .variable("itab_m%iw")
        .expect("expected itab_m%iw")
        .get_values::<i32, _>((.., ..))
        .expect("read expected itab_m%iw");
    assert_eq!(&produced_m[0..6], &expected_m[0..6]);

    let produced_n = produced
        .variable("n_ngrwm")
        .expect("produced n_ngrwm")
        .get_values::<i32, _>(..)
        .expect("read produced n_ngrwm");
    let expected_n = expected
        .variable("n_ngrwm")
        .expect("expected n_ngrwm")
        .get_values::<i32, _>(..)
        .expect("read expected n_ngrwm");
    assert_eq!(&produced_n[0..5], &expected_n[0..5]);

    let _ = fs::remove_dir_all(&root);
}
