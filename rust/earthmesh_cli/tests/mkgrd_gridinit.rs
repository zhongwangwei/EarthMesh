mod support;

use std::fs;
use std::path::{Path, PathBuf};

fn gridinit_temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()))
}

fn write_gridinit_namelist(
    root: &Path,
    case_name: &str,
    mode_grid: &str,
    output_format: &str,
    mode_file: &Path,
    defer_model_exports: bool,
) -> PathBuf {
    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='{mode_grid}'\n  NL%mode_file='{}'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n  NL%defer_model_exports={}\n/\n",
            mode_file.display(),
            if defer_model_exports { ".true." } else { ".false." }
        ),
    )
    .expect("write gridinit namelist");
    namelist
}

fn run_gridinit_cli(root: &Path, namelist: &Path) -> std::process::Output {
    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    support::output(
        std::process::Command::new(exe)
            .arg(namelist)
            .arg("--max-tris")
            .arg("2000")
            .current_dir(root),
    )
    .expect("run earthmesh_cli gridinit binary")
}

fn gridinit_output(root: &Path, case_name: &str, mode_grid: &str) -> PathBuf {
    root.join(case_name)
        .join("gridfile")
        .join(format!("gridfile_NXP0001_01_{mode_grid}.nc4"))
}

fn gridinit_output_nxp(root: &Path, case_name: &str, nxp: usize, mode_grid: &str) -> PathBuf {
    root.join(case_name)
        .join("gridfile")
        .join(format!("gridfile_NXP{nxp:04}_01_{mode_grid}.nc4"))
}

fn stdout_field(stdout: &str, key: &str) -> PathBuf {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("missing {key}: {stdout}"))
}

fn delivery_record(gridfile: &Path) -> serde_json::Value {
    let record_path = legacy_delivery_record_for(gridfile);
    serde_json::from_slice(
        &fs::read(&record_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", record_path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", record_path.display()))
}

fn raw_parent_from_delivery_record(gridfile: &Path) -> PathBuf {
    let record = delivery_record(gridfile);
    let raw_parent = record["auxiliary_artifacts"]["raw_parent"]
        .as_str()
        .unwrap_or_else(|| panic!("missing raw_parent auxiliary artifact in {record}"));
    assert!(
        !raw_parent.contains(".earthmesh-delivery-"),
        "raw_parent must be published path, not staging path: {record}"
    );
    let raw_parent = PathBuf::from(raw_parent);
    assert!(
        raw_parent.is_file(),
        "raw_parent does not exist: {}",
        raw_parent.display()
    );
    raw_parent
}

fn assert_regional_quality_not_closed_sphere(gridfile: &Path) {
    let quality = gridfile
        .parent()
        .unwrap()
        .join("final_quality")
        .join(gridfile.file_stem().unwrap())
        .join("quality_summary.json");
    let quality: serde_json::Value = serde_json::from_slice(
        &fs::read(&quality).unwrap_or_else(|error| panic!("read {}: {error}", quality.display())),
    )
    .expect("parse regional quality summary");
    assert_ne!(
        quality["topology"]["euler_characteristic"]
            .as_i64()
            .expect("numeric Euler characteristic"),
        2,
        "regional base quality must not require closed-sphere χ=2: {quality}"
    );
    assert_eq!(
        quality["topology"].get("expected_euler_characteristic"),
        Some(&serde_json::Value::Null)
    );
    assert!(
        quality["topology"]["boundary_edge_count"]
            .as_u64()
            .expect("boundary count")
            > 0
    );
}

fn write_global_gridinit_namelist_nxp(
    root: &Path,
    case_name: &str,
    nxp: usize,
    mesh_type: &str,
    mode_grid: &str,
    output_format: &str,
    defer_model_exports: bool,
) -> PathBuf {
    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP={nxp}\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='{mode_grid}'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%landtype_file='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n  NL%defer_model_exports={}\n/\n",
            if defer_model_exports { ".true." } else { ".false." }
        ),
    )
    .expect("write global gridinit namelist");
    namelist
}

fn write_landtype_gridinit_namelist(
    root: &Path,
    case_name: &str,
    nxp: usize,
    mesh_type: &str,
    mode_grid: &str,
    output_format: &str,
    landtype: &Path,
    defer_model_exports: bool,
) -> PathBuf {
    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP={nxp}\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='{mode_grid}'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%landtype_file='{}'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n  NL%defer_model_exports={}\n/\n",
            landtype.display(),
            if defer_model_exports { ".true." } else { ".false." }
        ),
    )
    .expect("write landtype gridinit namelist");
    namelist
}

fn write_regional_gridinit_namelist(
    root: &Path,
    case_name: &str,
    nxp: usize,
    mesh_type: &str,
    mode_grid: &str,
    output_format: &str,
    domain_type: &str,
    domain_source: &Path,
    landtype: Option<&Path>,
    defer_model_exports: bool,
) -> PathBuf {
    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    let landtype = landtype
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "none".to_string());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP={nxp}\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='{mode_grid}'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%landtype_file='{landtype}'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='{domain_type}'\n  NL%mask_domain_fprefix='{}'\n  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n  NL%defer_model_exports={}\n/\n",
            domain_source.display(),
            if defer_model_exports { ".true." } else { ".false." }
        ),
    )
    .expect("write regional gridinit namelist");
    namelist
}

fn write_landtype(path: &Path, value: i8) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", 360).expect("longitude dim");
    file.add_dimension("latitude", 180).expect("latitude dim");
    let mut variable = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype variable");
    variable
        .put_values(&vec![value; 360 * 180], (.., ..))
        .expect("write landtype");
}

fn write_bbox_domain(path: &Path, west: f64, east: f64, south: f64, north: f64) {
    earthmesh_cli::bbox_mask_io::write_bbox_mask_netcdf(
        path,
        &earthmesh_cli::bbox_mask_io::BBoxMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::bbox_mask_io::BBoxPoint {
                west,
                east,
                north,
                south,
            }],
        },
    )
    .expect("write bbox domain");
}

fn legacy_delivery_record_for(gridfile: &Path) -> PathBuf {
    gridfile
        .parent()
        .unwrap()
        .join("final_quality")
        .join(gridfile.file_stem().unwrap())
        .join("legacy_delivery.json")
}

fn assert_native_delivery_record(gridfile: &Path) {
    let record = delivery_record(gridfile);
    assert_eq!(record["gridfile"], gridfile.to_str().unwrap());
    assert_eq!(record["model_delivery_status"], "native_only");
    assert!(record["model_artifacts"].as_object().unwrap().is_empty());
    assert!(
        !record.to_string().contains(".earthmesh-delivery-"),
        "delivery record must expose published paths, not staging paths: {record}"
    );
}

fn assert_no_delivery_staging(root: &Path) {
    fn walk(path: &Path) {
        for entry in fs::read_dir(path)
            .unwrap_or_else(|error| panic!("read dir {}: {error}", path.display()))
        {
            let entry = entry.expect("read dir entry");
            let path = entry.path();
            assert!(
                !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".earthmesh-delivery-")),
                "delivery staging path leaked: {}",
                path.display()
            );
            if path.is_dir() {
                walk(&path);
            }
        }
    }
    if root.exists() {
        walk(root);
    }
}

fn write_clean_ocean_source_gridfile(path: &Path) {
    let mesh = earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 113.6,
                lat: 22.4,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 113.4,
                lat: 22.6,
            },
        ],
        w_points: vec![
            earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 113.0,
                lat: 22.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 114.0,
                lat: 22.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 114.0,
                lat: 23.0,
            },
            earthmesh_cli::coordinate_types::LonLatPoint {
                lon: 113.0,
                lat: 23.0,
            },
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4], [2, 4, 5]],
        w_to_m: vec![vec![1], vec![1, 2], vec![1], vec![1, 2], vec![2]],
        n_w_to_m: vec![0, 2, 1, 2, 1],
    };
    let context = earthmesh_cli::mpas_gridfile_context::MpasGridfileContext {
        cellwidth_km: vec![900.0, 100.0, 125.0, 75.0, 50.0],
        base_nxp: 9,
        step: 3,
        density_reference_width_km: 50.0,
        source: "test_producer".into(),
    };
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_method_c_metadata(
        path,
        &mesh,
        earthmesh_cli::unstructured_mesh_support::MethodCGridfileMetadataSlices {
            mpas: Some(&context),
            m_refine_level: Some(&[0, 0, 1]),
            w_refine_level: Some(&[0, 0, 1, 1, 0]),
            ..Default::default()
        },
    )
    .expect("write clean-ocean source gridfile");
}

fn write_clean_ocean_close(path: &Path) {
    fs::write(
        path,
        "close_num = 4\nclose_refine = 0\n112.0 20.0\n116.0 20.0\n116.0 25.0\n112.0 25.0\n",
    )
    .expect("write clean-ocean close domain");
}

fn write_clean_ocean_namelist(
    root: &Path,
    case_name: &str,
    source: &Path,
    close: &Path,
    landtype: &Path,
    output_format: &str,
    defer_model_exports: bool,
) -> PathBuf {
    write_clean_ocean_namelist_with_close_boundary(
        root,
        case_name,
        source,
        close,
        landtype,
        output_format,
        defer_model_exports,
        None,
    )
}

fn write_clean_ocean_namelist_with_close_boundary(
    root: &Path,
    case_name: &str,
    source: &Path,
    close: &Path,
    landtype: &Path,
    output_format: &str,
    defer_model_exports: bool,
    close_boundary: Option<&str>,
) -> PathBuf {
    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    let close_boundary = close_boundary
        .map(|value| format!("  NL%mask_domain_close_boundary='{value}'\n"))
        .unwrap_or_default();
    fs::write(
        &namelist,
        format!(
            "&mkgrd
  NL%EXPNME='{case_name}'
  NL%base_dir='{base_dir}'
  NL%NXP=7
  NL%mesh_type='oceanmesh'
  NL%mode_grid='tri'
  NL%mode_file='{}'
  NL%mode_file_description='EarthMesh'
  NL%landtype_file='{}'
  NL%gridnum_perdegree=120
  NL%mask_sea_ratio=0.5
  NL%refine=.false.
  NL%niter=0
  NL%beta=1.0
  NL%relax=0.25
  NL%mask_domain_global=.false.
  NL%mask_domain_type='close'
  NL%mask_domain_fprefix='{}'
{close_boundary}  NL%mask_patch_on=.false.
  NL%output_format='{output_format}'
  NL%defer_model_exports={}
/
",
            source.display(),
            landtype.display(),
            close.display(),
            if defer_model_exports {
                ".true."
            } else {
                ".false."
            }
        ),
    )
    .expect("write clean-ocean namelist");
    namelist
}

fn artifact_path(record: &serde_json::Value, section: &str, key: &str) -> PathBuf {
    PathBuf::from(
        record[section][key]
            .as_str()
            .unwrap_or_else(|| panic!("missing {section}.{key} in {record}")),
    )
}

fn one_point_earthmesh_source() -> earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
    earthmesh_cli::unstructured_mesh_support::UnstructuredMesh {
        m_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 1.0 }],
        w_points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 2.0, lat: 3.0 }],
        m_to_w: vec![[1, 1, 1]],
        w_to_m: vec![vec![1, 1, 1, 1, 1, 1, 1]],
        n_w_to_m: vec![1],
    }
}

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

    let report = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &namelist, &root, 100,
    )
    .expect("run Rust mkgrd gridinit path");

    let context =
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&report.gridfile.output)
            .unwrap()
            .expect("newly generated base mesh must preserve its nominal scale");
    assert_eq!(
        context.cellwidth_km,
        vec![7680.0; report.gridfile.lbx_points]
    );
    assert_eq!(context.base_nxp, 1);
    assert_eq!(context.step, 1);
    assert_eq!(context.density_reference_width_km, 7680.0);
    assert_eq!(context.source, "gridinit_uniform_base");
    assert_eq!(report.config.nxp, 1);
    assert_eq!(report.config.mode_grid, "hex");
    assert_eq!(report.workspace_mask.workspace.created_directories.len(), 5);
    assert!(report.workspace_mask.mask_reports.is_empty());
    assert_eq!(report.gridfile.sjx_points, 21);
    assert_eq!(report.gridfile.lbx_points, 13);
    let runtime_state = report
        .runtime_state
        .as_ref()
        .expect("generated gridinit should return Rust-owned runtime state");
    assert_eq!(runtime_state.nxp(), 1);
    assert_eq!(runtime_state.grid.nma, 21);
    assert_eq!(runtime_state.grid.nwa, 13);
    assert_eq!(runtime_state.num_mp_step[0], 21);
    assert_eq!(runtime_state.num_wp_step[0], 13);
    assert_eq!(
        runtime_state.pentagon_indices,
        [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]
    );
    assert_eq!(runtime_state.grid.xem.len(), 22);
    assert_eq!(runtime_state.grid.xew.len(), 14);
    assert_eq!(runtime_state.ijtabs.m.len(), 22);
    assert_eq!(runtime_state.ijtabs.w.len(), 14);
    assert_eq!(
        report.gridfile.output,
        root.join("case_gridinit/gridfile/gridfile_NXP0001_01_hex.nc4")
    );
    assert!(report.gridfile.output.exists());
    assert!(root.join("case_gridinit/result/namelist.save").exists());

    let file = netcdf::open(&report.gridfile.output).expect("open written gridfile");
    assert_eq!(file.dimension("sjx_points").expect("sjx_points").len(), 21);
    assert_eq!(file.dimension("lbx_points").expect("lbx_points").len(), 13);
    for (name, count) in [("earthmesh_m_lineage", 21), ("earthmesh_w_lineage", 13)] {
        assert_eq!(
            file.variable(name)
                .expect("fresh base snapshot lineage")
                .get_values::<i64, _>(..)
                .unwrap(),
            (1..=count).collect::<Vec<i64>>()
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_oceanmesh_missing_landtype_errors_before_gridfile() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_regional_missing_landtype_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let namelist = root.join("mkgrd_missing_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let missing_landtype = root.join("missing_landtype.nc");
    fs::write(
        &namelist,
        format!(
            "&mkgrd
  NL%EXPNME='case_regional_missing_landtype'
  NL%base_dir='{base_dir}'
  NL%NXP=1
  NL%mesh_type='oceanmesh'
  NL%mode_grid='tri'
  NL%mode_file='none'
  NL%mode_file_description='none'
  NL%landtype_file='{}'
  NL%refine=.false.
  NL%niter=0
  NL%mask_domain_global=.false.
  NL%mask_domain_type='bbox'
  NL%mask_domain_fprefix='inline:bbox:w=-180,e=180,s=-90,n=90'
  NL%mask_patch_on=.false.
  NL%output_format='FVCOM'
/
",
            missing_landtype.display()
        ),
    )
    .expect("write namelist");

    let err = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_regional_clip_base_namelist(
        &namelist, &root, 100,
    )
    .expect_err("configured missing landtype must not silently disable ocean carve");

    assert!(
        err.to_string().contains("missing_landtype.nc"),
        "unexpected error: {err}"
    );
    assert!(!root
        .join("case_regional_missing_landtype/gridfile/gridfile_NXP0001_01_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_gridinit_admits_tri_and_hex_native_final_gridfiles() {
    let root = gridinit_temp_root("binary_gridinit_final_admission");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    for (case_name, mode_grid, output_format) in [
        ("case_binary_final_tri", "tri", "FVCOM"),
        ("case_binary_final_hex", "hex", "MPAS"),
    ] {
        let missing_mode_file = root.join(format!("missing_{mode_grid}.nc4"));
        let namelist = write_gridinit_namelist(
            &root,
            case_name,
            mode_grid,
            output_format,
            &missing_mode_file,
            false,
        );

        let output = run_gridinit_cli(&root, &namelist);
        assert!(
            output.status.success(),
            "status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let gridfile = gridinit_output(&root, case_name, mode_grid);
        assert!(
            stdout.contains(&format!("gridfile={}", gridfile.display())),
            "stdout={stdout}"
        );
        assert!(gridfile.exists(), "missing gridfile {}", gridfile.display());
        assert_native_delivery_record(&gridfile);
        assert!(
            !stdout.contains(".earthmesh-delivery-"),
            "stdout must not expose staging paths: {stdout}"
        );

        // An imported source inside the existing workspace must survive a rerun.
        let imported = root.join(case_name).join("gridfile/source.nc4");
        let original = fs::read(&gridfile).unwrap();
        fs::write(&imported, &original).unwrap();
        let namelist =
            write_gridinit_namelist(&root, case_name, mode_grid, output_format, &imported, false);
        let imported_run = run_gridinit_cli(&root, &namelist);
        assert!(
            imported_run.status.success(),
            "{}",
            String::from_utf8_lossy(&imported_run.stderr)
        );
        assert_eq!(fs::read(&imported).unwrap(), original);
        assert_eq!(fs::read(&gridfile).unwrap(), original);
        assert_native_delivery_record(&gridfile);
    }
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_gridinit_failed_import_preserves_native_and_retires_completion() {
    let root = gridinit_temp_root("binary_gridinit_failed_import_rollback");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let case_name = "case_binary_failed_import";
    let generated_namelist = write_gridinit_namelist(
        &root,
        case_name,
        "hex",
        "MPAS",
        &root.join("missing_mode_file.nc4"),
        false,
    );

    let success = run_gridinit_cli(&root, &generated_namelist);
    assert!(
        success.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        success.status.code(),
        String::from_utf8_lossy(&success.stdout),
        String::from_utf8_lossy(&success.stderr)
    );
    let gridfile = gridinit_output(&root, case_name, "hex");
    assert_native_delivery_record(&gridfile);
    let previous_grid = fs::read(&gridfile).expect("snapshot published gridfile");
    let marker = legacy_delivery_record_for(&gridfile);
    assert!(marker.exists(), "success should publish completion marker");

    let invalid_source = root.join("invalid_one_point_earthmesh.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(
        &invalid_source,
        &one_point_earthmesh_source(),
    )
    .expect("write invalid imported EarthMesh source");
    let invalid_namelist =
        write_gridinit_namelist(&root, case_name, "hex", "MPAS", &invalid_source, false);

    let failure = run_gridinit_cli(&root, &invalid_namelist);
    assert!(
        !failure.status.success(),
        "invalid imported final mesh must fail admission\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert_eq!(
        fs::read(&gridfile).expect("read preserved gridfile"),
        previous_grid,
        "failed final admission must preserve the previous native gridfile bytes"
    );
    assert!(
        !marker.exists(),
        "failed final admission must retire the stale completion marker"
    );
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_gridinit_defer_model_exports_still_requires_native_admission() {
    let root = gridinit_temp_root("binary_gridinit_defer_still_admits");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let invalid_source = root.join("invalid_one_point_earthmesh.nc4");
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(
        &invalid_source,
        &one_point_earthmesh_source(),
    )
    .expect("write invalid imported EarthMesh source");
    let namelist = write_gridinit_namelist(
        &root,
        "case_binary_defer_invalid_import",
        "hex",
        "MPAS",
        &invalid_source,
        true,
    );

    let output = run_gridinit_cli(&root, &namelist);
    assert!(
        !output.status.success(),
        "defer_model_exports must not bypass native final admission\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!gridinit_output(&root, "case_binary_defer_invalid_import", "hex").exists());
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_gridinit_rejects_final_base_outside_workdir_without_touching_seeded_outputs(
) {
    let sandbox = gridinit_temp_root("binary_gridinit_outside_file_dir");
    let _ = fs::remove_dir_all(&sandbox);
    let root = sandbox.join("workdir");
    let outside_case = sandbox.join("outside_case");
    fs::create_dir_all(&root).expect("create workdir");
    fs::create_dir_all(outside_case.join("gridfile")).expect("create outside gridfile dir");
    let gridfile = outside_case.join("gridfile/gridfile_NXP0001_01_hex.nc4");
    fs::write(&gridfile, b"old native sentinel").expect("seed outside native");
    let marker = legacy_delivery_record_for(&gridfile);
    fs::create_dir_all(marker.parent().unwrap()).expect("create outside quality dir");
    fs::write(&marker, b"old completion sentinel").expect("seed outside marker");
    let old_grid = fs::read(&gridfile).expect("snapshot outside native");
    let old_marker = fs::read(&marker).expect("snapshot outside marker");

    let namelist = root.join("outside_file_dir.nml");
    let base_dir = format!("{}/", sandbox.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='outside_case'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
            root.display()
        ),
    )
    .expect("write outside file_dir namelist");

    let output = run_gridinit_cli(&root, &namelist);
    assert!(
        !output.status.success(),
        "outside final-base file_dir must be rejected\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&gridfile).unwrap(), old_grid);
    assert_eq!(fs::read(&marker).unwrap(), old_marker);
    assert_no_delivery_staging(&sandbox);

    let _ = fs::remove_dir_all(&sandbox);
}

#[test]
fn earthmesh_cli_binary_gridinit_rejects_namelist_save_aliases_without_touching_inputs_or_native() {
    let sandbox = gridinit_temp_root("binary_gridinit_namelist_save_alias");
    let _ = fs::remove_dir_all(&sandbox);

    for alias_target in ["input", "native"] {
        let root = sandbox.join(alias_target);
        fs::create_dir_all(&root).expect("create alias root");
        let case_name = format!("case_alias_{alias_target}");
        let namelist = write_gridinit_namelist(
            &root,
            &case_name,
            "hex",
            "MPAS",
            &root.join("missing_mode_file.nc4"),
            false,
        );
        let case_dir = root.join(&case_name);
        let save = case_dir.join("result/namelist.save");
        fs::create_dir_all(save.parent().unwrap()).expect("create result dir");

        if alias_target == "input" {
            fs::hard_link(&namelist, &save).expect("alias saved namelist to input namelist");
            let old_input = fs::read(&namelist).expect("snapshot input namelist");

            let output = run_gridinit_cli(&root, &namelist);
            assert!(
                !output.status.success(),
                "namelist.save input alias must be rejected\nstdout={}\nstderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(fs::read(&namelist).unwrap(), old_input);
            assert!(!gridinit_output(&root, &case_name, "hex").exists());
        } else {
            let success = run_gridinit_cli(&root, &namelist);
            assert!(
                success.status.success(),
                "status={:?}\nstdout={}\nstderr={}",
                success.status.code(),
                String::from_utf8_lossy(&success.stdout),
                String::from_utf8_lossy(&success.stderr)
            );
            let gridfile = gridinit_output(&root, &case_name, "hex");
            let old_native = fs::read(&gridfile).expect("snapshot native gridfile");
            fs::remove_file(&save).expect("replace saved namelist with native alias");
            fs::hard_link(&gridfile, &save).expect("alias saved namelist to native gridfile");

            let output = run_gridinit_cli(&root, &namelist);
            assert!(
                !output.status.success(),
                "namelist.save native alias must be rejected\nstdout={}\nstderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(fs::read(&gridfile).unwrap(), old_native);
        }
        assert_no_delivery_staging(&root);
    }

    let _ = fs::remove_dir_all(&sandbox);
}

#[test]
fn earthmesh_cli_binary_regional_base_geometry_clips_publish_native_with_raw_parent() {
    let root = gridinit_temp_root("binary_regional_base_geometry_delivery");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create source dir");

    let bbox = sources.join("domain_bbox.nc4");
    write_bbox_domain(&bbox, -90.0, 90.0, -90.0, 90.0);
    let circle = sources.join("domain_circle.nc4");
    earthmesh_cli::circle_close_mask_io::write_circle_mask_netcdf(
        &circle,
        &earthmesh_cli::circle_close_mask_io::CircleMask {
            refine_degree: 0,
            points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }],
            radius_km: vec![10_000.0],
        },
    )
    .expect("write circle domain");
    let close = sources.join("domain_close.nml");
    fs::write(
        &close,
        "close_num = 4\nclose_refine = 0\n-90.0 -80.0\n90.0 -80.0\n90.0 80.0\n-90.0 80.0\n",
    )
    .expect("write close domain");

    for (case_name, mode_grid, output_format, domain_type, domain) in [
        (
            "case_regional_bbox_tri",
            "tri",
            "FVCOM",
            "bbox",
            bbox.as_path(),
        ),
        (
            "case_regional_circle_hex",
            "hex",
            "MPAS",
            "circle",
            circle.as_path(),
        ),
        (
            "case_regional_close_tri",
            "tri",
            "FVCOM",
            "close",
            close.as_path(),
        ),
    ] {
        let domain_before = fs::read(domain).expect("snapshot domain source");
        let namelist = write_regional_gridinit_namelist(
            &root,
            case_name,
            3,
            "atmosmesh",
            mode_grid,
            output_format,
            domain_type,
            domain,
            None,
            false,
        );
        let output = run_gridinit_cli(&root, &namelist);
        assert!(
            output.status.success(),
            "{case_name}\nstatus={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let gridfile = gridinit_output_nxp(&root, case_name, 3, mode_grid);
        assert!(
            stdout.contains(&format!("gridfile={}", gridfile.display())),
            "stdout={stdout}"
        );
        assert!(
            gridfile.is_file(),
            "missing regional gridfile {}",
            gridfile.display()
        );
        assert_native_delivery_record(&gridfile);
        let raw_parent = raw_parent_from_delivery_record(&gridfile);
        assert_ne!(
            raw_parent, gridfile,
            "regional delivery should retain full raw parent"
        );
        assert_regional_quality_not_closed_sphere(&gridfile);
        let full = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&raw_parent)
            .unwrap();
        let clipped =
            earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&gridfile).unwrap();
        let (full_cells, clipped_cells) = if mode_grid == "tri" {
            (full.m_points.len(), clipped.m_points.len())
        } else {
            (full.w_points.len(), clipped.w_points.len())
        };
        assert!(
            full_cells > clipped_cells,
            "raw_parent must retain the full mother"
        );
        let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&raw_parent)
            .unwrap()
            .expect("mother producer scale");
        assert_eq!(context.base_nxp, 3);
        assert_eq!(
            fs::read(domain).unwrap(),
            domain_before,
            "domain source mutated"
        );
    }
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_regional_base_empty_clip_preserves_native_raw_parent_and_retires_marker() {
    let root = gridinit_temp_root("binary_regional_empty_clip_rollback");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let domain = root.join("domain_bbox.nc4");
    write_bbox_domain(&domain, -90.0, 90.0, -90.0, 90.0);
    let case_name = "case_regional_empty_clip";
    let namelist = write_regional_gridinit_namelist(
        &root,
        case_name,
        3,
        "atmosmesh",
        "tri",
        "FVCOM",
        "bbox",
        &domain,
        None,
        false,
    );
    let success = run_gridinit_cli(&root, &namelist);
    assert!(
        success.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&success.stdout),
        String::from_utf8_lossy(&success.stderr)
    );
    let gridfile = gridinit_output_nxp(&root, case_name, 3, "tri");
    assert_native_delivery_record(&gridfile);
    let raw_parent = raw_parent_from_delivery_record(&gridfile);
    let old_native = fs::read(&gridfile).expect("snapshot regional native");
    let old_raw = fs::read(&raw_parent).expect("snapshot raw parent");
    let marker = legacy_delivery_record_for(&gridfile);
    assert!(marker.exists());

    // File-backed geometry is also an immutable input during workspace setup.
    let saved = root.join(case_name).join("result/namelist.save");
    fs::remove_file(&saved).unwrap();
    fs::hard_link(&domain, &saved).unwrap();
    let domain_bytes = fs::read(&domain).unwrap();
    let alias_failure = run_gridinit_cli(&root, &namelist);
    assert!(!alias_failure.status.success());
    assert_eq!(fs::read(&domain).unwrap(), domain_bytes);
    assert_eq!(fs::read(&gridfile).unwrap(), old_native);
    assert_eq!(fs::read(&raw_parent).unwrap(), old_raw);
    assert!(!marker.exists());
    assert_no_delivery_staging(&root);
    fs::remove_file(&saved).unwrap();
    assert!(run_gridinit_cli(&root, &namelist).status.success());
    assert!(marker.exists());
    let old_native = fs::read(&gridfile).unwrap();
    let old_raw = fs::read(&raw_parent).unwrap();

    write_bbox_domain(&domain, 170.0, 171.0, 80.0, 81.0);
    let failure = run_gridinit_cli(&root, &namelist);
    assert!(
        !failure.status.success(),
        "empty regional clip must fail\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert_eq!(fs::read(&gridfile).unwrap(), old_native);
    assert_eq!(fs::read(&raw_parent).unwrap(), old_raw);
    assert!(
        !marker.exists(),
        "failed empty clip must withdraw readiness marker"
    );
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_regional_base_landtype_carve_publishes_and_empty_rejects() {
    let root = gridinit_temp_root("binary_regional_landtype_carve_delivery");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let landtype = root.join("landtype.nc");
    write_landtype(&landtype, 1);
    let case_name = "case_landtype_carve";

    let global =
        write_global_gridinit_namelist_nxp(&root, case_name, 3, "landmesh", "hex", "CoLM", false);
    let global_success = run_gridinit_cli(&root, &global);
    assert!(
        global_success.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&global_success.stdout),
        String::from_utf8_lossy(&global_success.stderr)
    );
    let global_base = gridinit_output_nxp(&root, case_name, 3, "hex");
    assert_native_delivery_record(&global_base);
    let old_global_base = fs::read(&global_base).expect("snapshot global base native");
    let global_marker = legacy_delivery_record_for(&global_base);
    let old_global_marker = fs::read(&global_marker).expect("snapshot global base marker");

    let namelist = write_landtype_gridinit_namelist(
        &root, case_name, 3, "landmesh", "hex", "CoLM", &landtype, false,
    );
    let success = run_gridinit_cli(&root, &namelist);
    assert!(
        success.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&success.stdout),
        String::from_utf8_lossy(&success.stderr)
    );
    let stdout = String::from_utf8_lossy(&success.stdout);
    let gridfile = stdout_field(&stdout, "gridfile=");
    assert!(gridfile.is_file());
    assert_ne!(
        gridfile, global_base,
        "carved final must not reuse canonical global base path"
    );
    assert_native_delivery_record(&gridfile);
    let raw_parent = raw_parent_from_delivery_record(&gridfile);
    assert!(
        raw_parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "gridfile_NXP0003_clip_raw_hex.nc4"),
        "global landtype carve must publish a raw_parent copy under tmpfile: {}",
        raw_parent.display()
    );
    assert_eq!(fs::read(&global_base).unwrap(), old_global_base);
    assert_eq!(fs::read(&global_marker).unwrap(), old_global_marker);
    let old_native = fs::read(&gridfile).expect("snapshot landtype native");
    let old_raw = fs::read(&raw_parent).expect("snapshot landtype raw parent");
    let marker = legacy_delivery_record_for(&gridfile);
    assert!(marker.exists());

    write_landtype(&landtype, 0);
    let failing = write_landtype_gridinit_namelist(
        &root, case_name, 3, "landmesh", "hex", "CoLM", &landtype, true,
    );
    let failure = run_gridinit_cli(&root, &failing);
    assert!(
        !failure.status.success(),
        "empty landtype carve must fail even when model exports are deferred\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert_eq!(fs::read(&gridfile).unwrap(), old_native);
    assert_eq!(fs::read(&raw_parent).unwrap(), old_raw);
    assert_eq!(fs::read(&global_base).unwrap(), old_global_base);
    assert_eq!(fs::read(&global_marker).unwrap(), old_global_marker);
    assert!(
        !marker.exists(),
        "failed empty carve must withdraw readiness marker"
    );
    // The same handoff handles a TRI ocean clip followed by the opposite mask.
    let ocean_domain = root.join("ocean_bbox.nc4");
    write_bbox_domain(&ocean_domain, -90.0, 90.0, -90.0, 90.0);
    let ocean = write_regional_gridinit_namelist(
        &root,
        "case_ocean_carve",
        3,
        "oceanmesh",
        "tri",
        "FVCOM",
        "bbox",
        &ocean_domain,
        Some(&landtype),
        false,
    );
    let ocean_run = run_gridinit_cli(&root, &ocean);
    assert!(
        ocean_run.status.success(),
        "{}",
        String::from_utf8_lossy(&ocean_run.stderr)
    );
    let ocean_grid = stdout_field(&String::from_utf8_lossy(&ocean_run.stdout), "gridfile=");
    assert_native_delivery_record(&ocean_grid);
    assert_regional_quality_not_closed_sphere(&ocean_grid);
    let ocean_raw = raw_parent_from_delivery_record(&ocean_grid);
    let mother =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&ocean_raw).unwrap();
    let selected =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&ocean_grid).unwrap();
    assert!(mother.m_points.len() > selected.m_points.len());
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_clean_ocean_close_delivers_native_auxiliary_and_fvcom() {
    let root = gridinit_temp_root("binary_clean_ocean_close_delivery");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let source = root.join("source.nc4");
    let close = root.join("domain_close.nml");
    let landtype = root.join("landtype.nc");
    write_clean_ocean_source_gridfile(&source);
    write_clean_ocean_close(&close);
    write_landtype(&landtype, 0);
    let namelist = write_clean_ocean_namelist(
        &root,
        "case_clean_ocean_fvcom",
        &source,
        &close,
        &landtype,
        "FVCOM",
        false,
    );
    let source_before = fs::read(&source).unwrap();
    let close_before = fs::read(&close).unwrap();
    let landtype_before = fs::read(&landtype).unwrap();

    let output = run_gridinit_cli(&root, &namelist);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fvcom_2dm="), "stdout={stdout}");
    let gridfile = stdout_field(&stdout, "gridfile=");
    let record = delivery_record(&gridfile);
    assert_eq!(record["model_delivery_status"], "model_delivered");
    let model_artifacts = record["model_artifacts"].as_object().unwrap();
    assert_eq!(
        model_artifacts.len(),
        1,
        "unexpected model artifacts: {record}"
    );
    assert!(
        model_artifacts.contains_key("fvcom_2dm"),
        "missing fvcom_2dm in {record}"
    );
    for key in ["raw_parent", "obc", "obcv2"] {
        assert!(
            artifact_path(&record, "auxiliary_artifacts", key).is_file(),
            "missing {key}"
        );
    }
    let raw_parent = artifact_path(&record, "auxiliary_artifacts", "raw_parent");
    assert!(
        raw_parent.to_string_lossy().contains("/tmpfile/"),
        "raw_parent should publish under tmpfile: {}",
        raw_parent.display()
    );
    assert_eq!(fs::read(&raw_parent).unwrap(), source_before);
    let fvcom = artifact_path(&record, "model_artifacts", "fvcom_2dm");
    assert!(fvcom.is_file());
    assert_regional_quality_not_closed_sphere(&gridfile);

    let obc = artifact_path(&record, "auxiliary_artifacts", "obc");
    let sidecar_obc =
        earthmesh_cli::obc_boundary_io::read_obc_order_netcdf(&obc).expect("read OBC sidecar");
    assert!(!sidecar_obc.is_empty());
    assert_eq!(
        earthmesh_cli::obc_boundary_io::read_gridfile_obc_order(&gridfile).unwrap(),
        Some(sidecar_obc)
    );

    let final_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&gridfile)
        .expect("read clean-ocean final mesh");
    let delayed = root.join("delayed.2dm");
    let delivered = earthmesh_cli::regional_gridfile_writers::write_fvcom_from_final_gridfile(
        &gridfile, &delayed,
    )
    .expect("write delayed FVCOM from admitted final");
    let fvcom_text = fs::read_to_string(&fvcom).expect("read FVCOM output");
    assert_eq!(
        fvcom_text
            .lines()
            .filter(|line| line.starts_with("E3T "))
            .count(),
        delivered.triangles
    );
    assert_eq!(
        fvcom_text
            .lines()
            .filter(|line| line.starts_with("ND "))
            .count(),
        delivered.nodes
    );
    assert_eq!(
        delivered.triangles,
        final_mesh.m_to_w.len().saturating_sub(2),
        "FVCOM triangle count should match the physical clean-ocean mesh"
    );
    assert_eq!(fs::read_to_string(&delayed).unwrap(), fvcom_text);
    let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&gridfile)
        .unwrap()
        .expect("clean-ocean metadata context");
    assert_eq!(context.source, "test_producer");
    assert_eq!(context.density_reference_width_km, 50.0);
    assert_eq!(fs::read(&source).unwrap(), source_before);
    assert_eq!(fs::read(&close).unwrap(), close_before);
    assert_eq!(fs::read(&landtype).unwrap(), landtype_before);
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn earthmesh_cli_binary_clean_ocean_non_model_exports_keep_native_auxiliary_only() {
    for (
        case_suffix,
        output_format,
        defer_model_exports,
        close_boundary,
        expected_reason,
        expected_aux,
    ) in [
        (
            "fvcom_deferred",
            "FVCOM",
            true,
            None,
            "deferred",
            vec!["raw_parent", "obc", "obcv2"],
        ),
        (
            "mpas_polyline",
            "MPAS",
            false,
            None,
            "no specialized model adapter",
            vec!["raw_parent", "obc", "obcv2"],
        ),
        (
            "fvcom_enclosing_cap",
            "FVCOM",
            false,
            Some("enclosing_cap:margin_km=10,max_radius_deg=80,max_segment_angle_deg=0.25"),
            "no specialized model adapter",
            vec!["raw_parent"],
        ),
    ] {
        let root = gridinit_temp_root(&format!("binary_clean_ocean_native_only_{case_suffix}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp root");
        let source = root.join("source.nc4");
        let close = root.join("domain_close.nml");
        let landtype = root.join("landtype.nc");
        write_clean_ocean_source_gridfile(&source);
        write_clean_ocean_close(&close);
        write_landtype(&landtype, 0);
        let case_name = format!("case_clean_ocean_{case_suffix}");
        let namelist = write_clean_ocean_namelist_with_close_boundary(
            &root,
            &case_name,
            &source,
            &close,
            &landtype,
            output_format,
            defer_model_exports,
            close_boundary,
        );

        let output = run_gridinit_cli(&root, &namelist);
        assert!(
            output.status.success(),
            "stdout={}
stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("fvcom_2dm="), "stdout={stdout}");
        let gridfile = stdout_field(&stdout, "gridfile=");
        let record = delivery_record(&gridfile);
        assert_eq!(record["requested_output_format"], output_format);
        assert_eq!(record["model_delivery_status"], "native_only");
        assert!(record["model_artifacts"].as_object().unwrap().is_empty());
        assert!(
            record["skipped_reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(expected_reason)),
            "unexpected skipped_reason in {record}"
        );
        let auxiliary = record["auxiliary_artifacts"].as_object().unwrap();
        assert_eq!(
            auxiliary.len(),
            expected_aux.len(),
            "unexpected auxiliary artifacts: {record}"
        );
        for key in expected_aux {
            assert!(
                artifact_path(&record, "auxiliary_artifacts", key).is_file(),
                "missing {key}"
            );
        }
        assert!(
            !root.join(&case_name).join("result/fvcom.2dm").exists(),
            "native-only delivery must not leave a canonical FVCOM artifact"
        );
        assert_no_delivery_staging(&root);

        let _ = fs::remove_dir_all(&root);
    }
}

#[test]
fn earthmesh_cli_binary_clean_ocean_failures_preserve_previous_bundle_and_retire_marker() {
    let root = gridinit_temp_root("binary_clean_ocean_failure_rollback");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let source = root.join("source.nc4");
    let close = root.join("domain_close.nml");
    let landtype = root.join("landtype.nc");
    write_clean_ocean_source_gridfile(&source);
    write_clean_ocean_close(&close);
    write_landtype(&landtype, 0);
    let case_name = "case_clean_ocean_failure";
    let namelist =
        write_clean_ocean_namelist(&root, case_name, &source, &close, &landtype, "FVCOM", false);

    let success = run_gridinit_cli(&root, &namelist);
    assert!(
        success.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&success.stdout),
        String::from_utf8_lossy(&success.stderr)
    );
    let gridfile = stdout_field(&String::from_utf8_lossy(&success.stdout), "gridfile=");
    let record = delivery_record(&gridfile);
    let raw_parent = artifact_path(&record, "auxiliary_artifacts", "raw_parent");
    let obc = artifact_path(&record, "auxiliary_artifacts", "obc");
    let obcv2 = artifact_path(&record, "auxiliary_artifacts", "obcv2");
    let fvcom = artifact_path(&record, "model_artifacts", "fvcom_2dm");
    let native_before = fs::read(&gridfile).unwrap();
    let raw_before = fs::read(&raw_parent).unwrap();
    let obc_before = fs::read(&obc).unwrap();
    let obcv2_before = fs::read(&obcv2).unwrap();
    let fvcom_before = fs::read(&fvcom).unwrap();
    let marker = legacy_delivery_record_for(&gridfile);
    assert!(marker.exists());

    let fvcom_backup = fvcom.with_extension("2dm.bak");
    fs::rename(&fvcom, &fvcom_backup).expect("move old FVCOM output aside");
    fs::create_dir(&fvcom).expect("block FVCOM output with directory");
    let sentinel = fvcom.join("sentinel.txt");
    fs::write(&sentinel, b"keep existing directory sentinel")
        .expect("write FVCOM blocker sentinel");
    let blocked = run_gridinit_cli(&root, &namelist);
    assert!(
        !blocked.status.success(),
        "blocked FVCOM output must fail without replacing the bundle\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&blocked.stdout),
        String::from_utf8_lossy(&blocked.stderr)
    );
    assert_eq!(fs::read(&gridfile).unwrap(), native_before);
    assert_eq!(fs::read(&raw_parent).unwrap(), raw_before);
    assert_eq!(fs::read(&obc).unwrap(), obc_before);
    assert_eq!(fs::read(&obcv2).unwrap(), obcv2_before);
    assert_eq!(fs::read(&fvcom_backup).unwrap(), fvcom_before);
    assert_eq!(
        fs::read(&sentinel).unwrap(),
        b"keep existing directory sentinel"
    );
    assert!(
        !marker.exists(),
        "failed FVCOM publication must retire readiness"
    );
    fs::remove_dir_all(&fvcom).expect("remove FVCOM blocker directory");
    fs::rename(&fvcom_backup, &fvcom).expect("restore old FVCOM output");

    assert!(run_gridinit_cli(&root, &namelist).status.success());
    assert!(marker.exists());
    let native_before = fs::read(&gridfile).unwrap();
    let raw_before = fs::read(&raw_parent).unwrap();
    let obc_before = fs::read(&obc).unwrap();
    let obcv2_before = fs::read(&obcv2).unwrap();
    let fvcom_before = fs::read(&fvcom).unwrap();

    fs::write(
        &close,
        "close_num = 2\nclose_refine = 0\n112.0 20.0\n116.0 20.0\n",
    )
    .expect("write invalid clean-ocean close domain");
    let invalid_close = run_gridinit_cli(&root, &namelist);
    assert!(
        !invalid_close.status.success(),
        "invalid close geometry must fail without replacing the bundle\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&invalid_close.stdout),
        String::from_utf8_lossy(&invalid_close.stderr)
    );
    assert_eq!(fs::read(&gridfile).unwrap(), native_before);
    assert_eq!(fs::read(&raw_parent).unwrap(), raw_before);
    assert_eq!(fs::read(&obc).unwrap(), obc_before);
    assert_eq!(fs::read(&obcv2).unwrap(), obcv2_before);
    assert_eq!(fs::read(&fvcom).unwrap(), fvcom_before);
    assert!(
        !marker.exists(),
        "invalid clean-ocean close geometry must retire readiness"
    );
    assert_no_delivery_staging(&root);

    write_clean_ocean_close(&close);
    assert!(run_gridinit_cli(&root, &namelist).status.success());
    assert!(marker.exists());
    let native_before = fs::read(&gridfile).unwrap();
    let raw_before = fs::read(&raw_parent).unwrap();
    let obc_before = fs::read(&obc).unwrap();
    let obcv2_before = fs::read(&obcv2).unwrap();
    let fvcom_before = fs::read(&fvcom).unwrap();

    write_landtype(&landtype, 1);
    let empty =
        write_clean_ocean_namelist(&root, case_name, &source, &close, &landtype, "FVCOM", true);
    let failure = run_gridinit_cli(&root, &empty);
    assert!(
        !failure.status.success(),
        "all-land clean-ocean carve must fail even when FVCOM is deferred\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert_eq!(fs::read(&gridfile).unwrap(), native_before);
    assert_eq!(fs::read(&raw_parent).unwrap(), raw_before);
    assert_eq!(fs::read(&obc).unwrap(), obc_before);
    assert_eq!(fs::read(&obcv2).unwrap(), obcv2_before);
    assert_eq!(fs::read(&fvcom).unwrap(), fvcom_before);
    assert!(
        !marker.exists(),
        "failed clean-ocean carve must retire readiness"
    );
    assert_no_delivery_staging(&root);

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
    let output = support::output(
        std::process::Command::new(exe)
            .arg(&namelist)
            .arg("--max-tris")
            .arg("100")
            .current_dir(&root),
    )
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
    let source_mesh = one_point_earthmesh_source();
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&mode_file, &source_mesh)
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

    let report = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &namelist, &root, 100,
    )
    .expect("copy existing EarthMesh mode file");
    assert!(
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&report.gridfile.output)
            .unwrap()
            .is_none(),
        "imported mesh must not acquire guessed uniform widths"
    );

    assert_eq!(report.gridfile.sjx_points, 1);
    assert_eq!(report.gridfile.lbx_points, 1);
    let runtime_state = report
        .runtime_state
        .as_ref()
        .expect("existing EarthMesh mode_file should return Rust-owned runtime state");
    assert_eq!(runtime_state.grid.nma, 1);
    assert_eq!(runtime_state.grid.nwa, 1);
    assert_eq!(runtime_state.num_mp_step[0], 1);
    assert_eq!(runtime_state.num_wp_step[0], 1);
    assert_eq!(runtime_state.grid.glonm, vec![0.0]);
    assert_eq!(runtime_state.grid.glatm, vec![1.0]);
    assert_eq!(runtime_state.grid.glonw, vec![2.0]);
    assert_eq!(runtime_state.grid.glatw, vec![3.0]);
    assert_eq!(runtime_state.ijtabs.m[0].iw, [1, 1, 1]);
    assert_eq!(runtime_state.ijtabs.w[0].im, [1, 1, 1, 1, 1, 1, 1]);
    assert_eq!(
        report.gridfile.output,
        root.join("case_existing/gridfile/gridfile_NXP0001_01_hex.nc4")
    );
    assert_eq!(
        fs::read(&report.gridfile.output).unwrap(),
        fs::read(&mode_file).unwrap()
    );
    assert!(
        !legacy_delivery_record_for(&report.gridfile.output).exists(),
        "raw gridinit API should not publish a standalone CLI completion record"
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

    let report = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &namelist, &root, 100,
    )
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
        vec![1, 1, 1, 1, 2, 1, 2, 1, 1]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_w%im"),
        vec![
            1, 0, 0, 0, 0, 0, 0, //
            1, 2, 1, 1, 0, 0, 0, //
            2, 1, 1, 1, 0, 0, 0,
        ]
    );
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![1, 2, 2]
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

    let report = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &namelist, &root, 100,
    )
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
        vec![1, 1, 1, 1, 2, 3, 3, 2, 1]
    );
    assert_eq!(
        file.variable("itab_w%im")
            .expect("itab_w%im")
            .get_values::<i32, _>((.., ..))
            .expect("read itab_w%im"),
        vec![
            1, 1, 1, 1, 1, 1, 1, //
            1, 2, 1, 1, 1, 1, 1, //
            2, 1, 1, 1, 1, 1, 1, //
            2, 1, 1, 1, 1, 1, 1,
        ]
    );
    assert_eq!(
        file.variable("n_ngrwm")
            .expect("n_ngrwm")
            .get_values::<i32, _>(..)
            .expect("read n_ngrwm"),
        vec![0, 2, 2, 2]
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

    let report = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &namelist, &root, 100,
    )
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
    let mut file =
        earthmesh_cli::create_netcdf_quiet(path).expect("create synthetic MPAS mode file");
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
        // Standard MPAS is 1-based and uses trailing zero padding beyond the
        // active nEdgesOnCell entries.
        var.put_values(&[2, 2], ..).expect("write nEdgesOnCell");
    }
}

fn write_synthetic_fvcom_mode_file(path: &std::path::Path) {
    let mut file =
        earthmesh_cli::create_netcdf_quiet(path).expect("create synthetic FVCOM mode file");
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
                2, 1, 0, 0, 0, 0, 0, //
                2, 1, 0, 0, 0, 0, 0,
            ],
            (.., ..),
        )
        .expect("write nbve");
    }
    {
        let mut var = file.add_variable::<i32>("ntve", &["node"]).expect("ntve");
        var.put_values(&[2, 2, 2], ..).expect("write ntve");
    }
}

fn write_synthetic_iap_ocean_mode_file(path: &std::path::Path) {
    let mut file =
        earthmesh_cli::create_netcdf_quiet(path).expect("create synthetic IAP-Ocean mode file");
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
fn run_mkgrd_gridinit_global_matches_canonical_nxp64_gridfile_fixture() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root");
    let canonical = repo_root.join(
        "cases/ATMOS_hex_N64_refine2_global_LOM67_251027/gridfile/gridfile_NXP0064_01_hex.nc4",
    );
    if !canonical.exists() {
        eprintln!("skip: missing canonical fixture {canonical:?}");
        return;
    }

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

    let report = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &namelist, &root, 100_000,
    )
    .expect("run Rust NXP64 mkgrd gridinit path");
    assert_eq!(report.gridfile.sjx_points, 81921);
    assert_eq!(report.gridfile.lbx_points, 40963);

    let produced = netcdf::open(&report.gridfile.output).expect("open produced gridfile");
    let expected = netcdf::open(&canonical).expect("open canonical gridfile");
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
        let expected_latitudes = match var_name {
            "GLONM" => Some("GLATM"),
            "GLONW" => Some("GLATW"),
            _ => None,
        }
        .map(|latitude_name| {
            expected
                .variable(latitude_name)
                .expect("expected latitude variable")
                .get_values::<f64, _>(..)
                .expect("read expected latitude variable")
        });
        let expected_values = expected
            .variable(var_name)
            .expect("expected variable")
            .get_values::<f64, _>(..)
            .expect("read expected variable");
        for index in [0usize, 1, 2, 3, 4, actual.len() / 2, actual.len() - 1] {
            let tolerance = if var_name.starts_with("GLO") && expected_values[index].abs() > 179.9 {
                5.0e-4
            } else {
                2.0e-4
            };
            let longitude_is_defined = expected_latitudes
                .as_ref()
                .is_none_or(|latitudes| latitudes[index].abs() < 89.999);
            if var_name.starts_with("GLA") || (var_name.starts_with("GLO") && longitude_is_defined)
            {
                assert_close(actual[index], expected_values[index], tolerance);
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
