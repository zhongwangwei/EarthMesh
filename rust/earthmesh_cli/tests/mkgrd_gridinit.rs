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

#[test]
fn run_mkgrd_gridinit_global_converts_existing_mpas_mode_file() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_mpas_mode_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let mode_file = root.join("source_mpas.nc4");
    write_synthetic_mpas_mode_file(&mode_file);

    let namelist = root.join("mkgrd_mpas.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}'\n  NL%mode_file_description='MPAS'\n  NL%refine=.false.\n  NL%niter=0\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
            mode_file.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(&namelist, &root, 100)
        .expect("convert MPAS mode file");

    assert_eq!(report.gridfile.sjx_points, 3);
    assert_eq!(report.gridfile.lbx_points, 3);
    assert_eq!(report.gridfile.dimc, 7);
    let file = netcdf::open(&report.gridfile.output).expect("open converted gridfile");
    let glonm = file
        .variable("GLONM")
        .expect("GLONM")
        .get_values::<f64, _>(..)
        .expect("read GLONM");
    let glatm = file
        .variable("GLATM")
        .expect("GLATM")
        .get_values::<f64, _>(..)
        .expect("read GLATM");
    let glonw = file
        .variable("GLONW")
        .expect("GLONW")
        .get_values::<f64, _>(..)
        .expect("read GLONW");
    for (actual, expected) in glonm.iter().zip([0.0, 10.0, -170.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    for (actual, expected) in glatm.iter().zip([0.0, 20.0, -30.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    for (actual, expected) in glonw.iter().zip([0.0, 40.0, -160.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    assert_eq!(
        file.variable("itab_m%iw")
            .expect("itab_m%iw")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_m%iw"),
        vec![1, 1, 1, 2, 3, 1, 3, 2, 1]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_w%im"),
        vec![1, 0, 0, 0, 0, 0, 0, 2, 3, 1, 1, 0, 0, 0, 3, 2, 1, 1, 0, 0, 0]
    );
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![1, 4, 4]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn run_mkgrd_gridinit_global_converts_existing_fvcom_mode_file() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_fvcom_mode_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let mode_file = root.join("source_fvcom.nc4");
    write_synthetic_fvcom_mode_file(&mode_file);

    let namelist = root.join("mkgrd_fvcom.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_fvcom'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}'\n  NL%mode_file_description='FVCOM'\n  NL%refine=.false.\n  NL%niter=0\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n",
            mode_file.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(&namelist, &root, 100)
        .expect("convert FVCOM mode file");

    assert_eq!(report.gridfile.sjx_points, 3);
    assert_eq!(report.gridfile.lbx_points, 4);
    assert_eq!(report.gridfile.dimc, 7);
    let file = netcdf::open(&report.gridfile.output).expect("open converted gridfile");
    let glonm = file
        .variable("GLONM")
        .expect("GLONM")
        .get_values::<f64, _>(..)
        .expect("read GLONM");
    let glonw = file
        .variable("GLONW")
        .expect("GLONW")
        .get_values::<f64, _>(..)
        .expect("read GLONW");
    for (actual, expected) in glonm.iter().zip([0.0, 170.0, -170.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    for (actual, expected) in glonw.iter().zip([0.0, 10.0, -179.0, 179.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    assert_eq!(
        file.variable("itab_m%iw")
            .expect("itab_m%iw")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_m%iw"),
        vec![1, 1, 1, 2, 3, 4, 4, 3, 2]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_w%im"),
        vec![
            1, 1, 1, 1, 1, 1, 1, //
            2, 3, 1, 1, 1, 1, 1, //
            3, 4, 2, 1, 1, 1, 1, //
            4, 3, 2, 1, 1, 1, 1,
        ]
    );
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![0, 2, 3, 3]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn run_mkgrd_gridinit_global_converts_existing_iap_ocean_mode_file() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_iap_mode_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let mode_file = root.join("source_iap.nc4");
    write_synthetic_iap_ocean_mode_file(&mode_file);

    let namelist = root.join("mkgrd_iap.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_iap'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}'\n  NL%mode_file_description='IAP-Ocean'\n  NL%refine=.false.\n  NL%niter=0\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n",
            mode_file.display()
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(&namelist, &root, 100)
        .expect("convert IAP-Ocean mode file");

    assert_eq!(report.gridfile.sjx_points, 2);
    assert_eq!(report.gridfile.lbx_points, 4);
    assert_eq!(report.gridfile.dimc, 7);
    let file = netcdf::open(&report.gridfile.output).expect("open converted gridfile");
    let glonm = file
        .variable("GLONM")
        .expect("GLONM")
        .get_values::<f64, _>(..)
        .expect("read GLONM");
    let glatm = file
        .variable("GLATM")
        .expect("GLATM")
        .get_values::<f64, _>(..)
        .expect("read GLATM");
    let glonw = file
        .variable("GLONW")
        .expect("GLONW")
        .get_values::<f64, _>(..)
        .expect("read GLONW");
    let glatw = file
        .variable("GLATW")
        .expect("GLATW")
        .get_values::<f64, _>(..)
        .expect("read GLATW");
    for (actual, expected) in glonm.iter().zip([0.0, 45.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    assert_close(glatm[0], 0.0, 1.0e-10);
    assert_close(glatm[1], 35.264389682754654, 1.0e-10);
    for (actual, expected) in glonw.iter().zip([0.0, 0.0, 90.0, 0.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    for (actual, expected) in glatw.iter().zip([0.0, 0.0, 0.0, 90.0]) {
        assert_close(*actual, expected, 1.0e-10);
    }
    assert_eq!(
        file.variable("itab_m%iw")
            .expect("itab_m%iw")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_m%iw"),
        vec![1, 1, 1, 2, 3, 4]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_w%im"),
        vec![
            1, 1, 1, 1, 1, 1, 1, //
            2, 1, 1, 1, 1, 1, 1, //
            2, 1, 1, 1, 1, 1, 1, //
            2, 1, 1, 1, 1, 1, 1,
        ]
    );
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![0, 1, 1, 1]
    );

    let _ = fs::remove_dir_all(&root);
}

fn write_synthetic_mpas_mode_file(path: &std::path::Path) {
    let mut file = netcdf::create(path).expect("create synthetic MPAS mode file");
    file.add_dimension("nVertices", 2).expect("nVertices");
    file.add_dimension("nCells", 2).expect("nCells");
    file.add_dimension("maxEdges", 4).expect("maxEdges");
    file.add_dimension("vertexDegree", 3).expect("vertexDegree");
    {
        let mut var = file
            .add_variable::<f64>("lonVertex", &["nVertices"])
            .expect("lonVertex");
        var.put_values(&[10.0_f64.to_radians(), 190.0_f64.to_radians()], ..)
            .expect("write lonVertex");
    }
    {
        let mut var = file
            .add_variable::<f64>("latVertex", &["nVertices"])
            .expect("latVertex");
        var.put_values(&[20.0_f64.to_radians(), -30.0_f64.to_radians()], ..)
            .expect("write latVertex");
    }
    {
        let mut var = file
            .add_variable::<f64>("lonCell", &["nCells"])
            .expect("lonCell");
        var.put_values(&[40.0_f64.to_radians(), 200.0_f64.to_radians()], ..)
            .expect("write lonCell");
    }
    {
        let mut var = file
            .add_variable::<f64>("latCell", &["nCells"])
            .expect("latCell");
        var.put_values(&[50.0_f64.to_radians(), -60.0_f64.to_radians()], ..)
            .expect("write latCell");
    }
    {
        let mut var = file
            .add_variable::<i32>("cellsOnVertex", &["nVertices", "vertexDegree"])
            .expect("cellsOnVertex");
        var.put_values(&[1, 2, 0, 2, 1, 0], (.., ..))
            .expect("write cellsOnVertex");
    }
    {
        let mut var = file
            .add_variable::<i32>("verticesOnCell", &["nCells", "maxEdges"])
            .expect("verticesOnCell");
        var.put_values(&[1, 2, 0, 0, 2, 1, 0, 0], (.., ..))
            .expect("write verticesOnCell");
    }
    {
        let mut var = file
            .add_variable::<i32>("nEdgesOnCell", &["nCells"])
            .expect("nEdgesOnCell");
        var.put_values(&[4, 4], ..).expect("write nEdgesOnCell");
    }
}

fn write_synthetic_fvcom_mode_file(path: &std::path::Path) {
    let mut file = netcdf::create(path).expect("create synthetic FVCOM mode file");
    file.add_dimension("maxelem", 7).expect("maxelem");
    file.add_dimension("node", 3).expect("node");
    file.add_dimension("nele", 2).expect("nele");
    file.add_dimension("three", 3).expect("three");
    {
        let mut var = file.add_variable::<f64>("lonc", &["nele"]).expect("lonc");
        var.put_values(&[170.0, 190.0], ..).expect("write lonc");
    }
    {
        let mut var = file.add_variable::<f64>("latc", &["nele"]).expect("latc");
        var.put_values(&[20.0, -20.0], ..).expect("write latc");
    }
    {
        let mut var = file.add_variable::<f64>("lon", &["node"]).expect("lon");
        var.put_values(&[10.0, 181.0, -181.0], ..)
            .expect("write lon");
    }
    {
        let mut var = file.add_variable::<f64>("lat", &["node"]).expect("lat");
        var.put_values(&[30.0, 40.0, 50.0], ..).expect("write lat");
    }
    {
        let mut var = file
            .add_variable::<i32>("nv", &["nele", "three"])
            .expect("nv");
        var.put_values(&[1, 2, 3, 3, 2, 1], (.., ..))
            .expect("write nv");
    }
    {
        let mut var = file
            .add_variable::<i32>("nbve", &["node", "maxelem"])
            .expect("nbve");
        var.put_values(
            &[
                1, 2, 0, 0, 0, 0, 0, //
                2, 3, 1, 0, 0, 0, 0, //
                3, 2, 1, 0, 0, 0, 0,
            ],
            (.., ..),
        )
        .expect("write nbve");
    }
    {
        let mut var = file.add_variable::<i32>("ntve", &["node"]).expect("ntve");
        var.put_values(&[2, 3, 3], ..).expect("write ntve");
    }
}

fn write_synthetic_iap_ocean_mode_file(path: &std::path::Path) {
    let mut file = netcdf::create(path).expect("create synthetic IAP-Ocean mode file");
    file.add_dimension("sjx_points", 1).expect("sjx_points");
    file.add_dimension("lbx_points", 3).expect("lbx_points");
    file.add_dimension("dimb", 3).expect("dimb");
    {
        let mut var = file
            .add_variable::<f64>("GLONW", &["lbx_points"])
            .expect("GLONW");
        var.put_values(&[0.0_f64.to_radians(), 90.0_f64.to_radians(), 0.0], ..)
            .expect("write GLONW");
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATW", &["lbx_points"])
            .expect("GLATW");
        var.put_values(&[0.0, 0.0, 90.0_f64.to_radians()], ..)
            .expect("write GLATW");
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_m%im", &["sjx_points", "dimb"])
            .expect("itab_m%im");
        var.put_values(&[1, 2, 3], (.., ..))
            .expect("write itab_m%im");
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
            .expect("itab_m%iw");
        var.put_values(&[1, 2, 3], (.., ..))
            .expect("write itab_m%iw");
    }
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
