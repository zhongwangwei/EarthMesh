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
