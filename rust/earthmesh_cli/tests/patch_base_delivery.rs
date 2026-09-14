use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_bbox(path: &Path, west: f64, east: f64, south: f64, north: f64) {
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
    .expect("write bbox mask");
}

fn try_run_final_base(
    namelist: &Path,
    root: &Path,
) -> std::io::Result<earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport> {
    earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_default_with_base_delivery(
        namelist, root, 2000, 7, None, None, 1, None, true,
    )
}

fn run_final_base(
    namelist: &Path,
    root: &Path,
) -> earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport {
    try_run_final_base(namelist, root).expect("run final base delivery")
}

fn gridfile(root: &Path, case_name: &str, nxp: usize, mode_grid: &str) -> PathBuf {
    root.join(case_name)
        .join("gridfile")
        .join(format!("gridfile_NXP{nxp:04}_01_{mode_grid}.nc4"))
}

fn legacy_record_path(gridfile: &Path) -> PathBuf {
    gridfile
        .parent()
        .unwrap()
        .join("final_quality")
        .join(gridfile.file_stem().unwrap())
        .join("legacy_delivery.json")
}

fn delivery_record(gridfile: &Path) -> serde_json::Value {
    let record = legacy_record_path(gridfile);
    serde_json::from_slice(
        &fs::read(&record)
            .unwrap_or_else(|error| panic!("read delivery record {}: {error}", record.display())),
    )
    .expect("parse delivery record")
}

fn assert_no_staging_paths(value: &serde_json::Value) {
    assert!(
        !value.to_string().contains(".earthmesh-delivery-"),
        "delivery record leaked staging path: {value}"
    );
}

fn write_global_patch_namelist(root: &Path, case_name: &str, patch_source: &Path) -> PathBuf {
    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=1\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%landtype_file='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_source}'\n  NL%output_format='MPAS'\n  NL%defer_model_exports=.true.\n/\n",
            patch_source = patch_source.display()
        ),
    )
    .expect("write global patch namelist");
    namelist
}

#[test]
fn global_patch_base_publishes_admitted_native_and_patch_cache_without_mutating_source() {
    let root = temp_root("global_patch_base_delivery");
    let patch_source = root.join("patch_source.nc4");
    write_bbox(&patch_source, -10.0, 10.0, -10.0, 10.0);
    let source_before = fs::read(&patch_source).expect("read patch source before");
    let namelist = write_global_patch_namelist(&root, "global_patch", &patch_source);

    let report = run_final_base(&namelist, &root);
    let expected_grid = gridfile(&root, "global_patch", 1, "hex");
    let expected_patch = root.join("global_patch/tmpfile/mask_patch_bbox_0_01.nc4");
    match report {
        earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
            earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDispatchRunReport::Gridinit(report),
        ) => {
            assert_eq!(report.gridfile.output, expected_grid);
            assert_eq!(report.workspace_mask.mask_counts.mask_patch_ndm[0], 1);
            assert_eq!(
                report.workspace_mask.mask_reports[0].outputs,
                vec![expected_patch.clone()]
            );
            let saved = root.join("global_patch/result/namelist.save");
            assert_eq!(
                report.workspace_mask.workspace.copied_namelist_to,
                Some(saved.clone())
            );
            assert!(saved.is_file(), "published namelist.save missing");
        }
        other => panic!("unexpected report: {other:?}"),
    }
    assert!(expected_grid.is_file(), "published native missing");
    assert!(expected_patch.is_file(), "published patch cache missing");
    assert_eq!(fs::read(&patch_source).unwrap(), source_before);

    let record = delivery_record(&expected_grid);
    assert_eq!(record["gridfile"], expected_grid.to_str().unwrap());
    assert_eq!(record["patch_preprocessing_only"], true);
    assert_eq!(record["patch_applied_to_geometry"], false);
    assert_eq!(
        record["auxiliary_artifacts"]["patch_cache_00"],
        expected_patch.to_str().unwrap()
    );
    assert_no_staging_paths(&record);

    let quality = expected_grid
        .parent()
        .unwrap()
        .join("final_quality")
        .join(expected_grid.file_stem().unwrap())
        .join("quality_summary.json");
    let quality: serde_json::Value =
        serde_json::from_slice(&fs::read(&quality).unwrap()).expect("parse quality summary");
    assert_eq!(quality["topology"]["expected_euler_characteristic"], 2);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn regional_patch_base_publishes_raw_parent_and_patch_cache() {
    let root = temp_root("regional_patch_base_delivery");
    let domain_source = root.join("domain.nc4");
    let patch_source = root.join("patch.nc4");
    write_bbox(&domain_source, -120.0, 120.0, -60.0, 60.0);
    write_bbox(&patch_source, -10.0, 10.0, -10.0, 10.0);
    let namelist = root.join("regional_patch.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='regional_patch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=3\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%landtype_file='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_source}'\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{patch_source}'\n  NL%output_format='MPAS'\n  NL%defer_model_exports=.true.\n/\n",
            domain_source = domain_source.display(),
            patch_source = patch_source.display()
        ),
    )
    .expect("write namelist");

    let report = run_final_base(&namelist, &root);
    let expected_grid = gridfile(&root, "regional_patch", 3, "hex");
    let expected_raw = root.join("regional_patch/tmpfile/gridfile_NXP0003_clip_raw_hex.nc4");
    let expected_patch = root.join("regional_patch/tmpfile/mask_patch_bbox_0_01.nc4");
    match report {
        earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
            earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDispatchRunReport::Gridinit(report),
        ) => {
            assert_eq!(report.gridfile.output, expected_grid);
            assert_eq!(report.raw_output.expect("raw parent").output, expected_raw);
            assert_eq!(report.workspace_mask.mask_counts.mask_patch_ndm[0], 1);
            assert_eq!(
                report.workspace_mask.mask_reports[0].outputs,
                vec![expected_patch.clone()]
            );
            let saved = root.join("regional_patch/result/namelist.save");
            assert_eq!(
                report.workspace_mask.workspace.copied_namelist_to,
                Some(saved.clone())
            );
            assert!(saved.is_file(), "published namelist.save missing");
        }
        other => panic!("unexpected report: {other:?}"),
    }
    assert!(expected_grid.is_file(), "published regional native missing");
    assert!(expected_raw.is_file(), "published raw parent missing");
    assert!(expected_patch.is_file(), "published patch cache missing");

    let record = delivery_record(&expected_grid);
    assert_eq!(
        record["auxiliary_artifacts"]["raw_parent"],
        expected_raw.to_str().unwrap()
    );
    assert_eq!(
        record["auxiliary_artifacts"]["patch_cache_00"],
        expected_patch.to_str().unwrap()
    );
    assert_no_staging_paths(&record);
    let quality = expected_grid
        .parent()
        .unwrap()
        .join("final_quality")
        .join(expected_grid.file_stem().unwrap())
        .join("quality_summary.json");
    let quality: serde_json::Value =
        serde_json::from_slice(&fs::read(&quality).unwrap()).expect("parse quality summary");
    assert_eq!(
        quality["topology"].get("expected_euler_characteristic"),
        Some(&serde_json::Value::Null)
    );

    let native_before = fs::read(&expected_grid).unwrap();
    let config = fs::read_to_string(&namelist).unwrap().replace(
        domain_source.to_str().unwrap(),
        root.join("missing/domain.nc4").to_str().unwrap(),
    );
    fs::write(&namelist, config).unwrap();
    assert!(try_run_final_base(&namelist, &root).is_err());
    assert_eq!(fs::read(&expected_grid).unwrap(), native_before);
    assert!(!legacy_record_path(&expected_grid).exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn blocked_patch_cache_output_rerun_withdraws_marker_without_replacing_native() {
    let root = temp_root("blocked_patch_output_delivery");
    let patch_source = root.join("patch_source.nc4");
    write_bbox(&patch_source, -10.0, 10.0, -10.0, 10.0);
    let namelist = write_global_patch_namelist(&root, "blocked_patch", &patch_source);
    run_final_base(&namelist, &root);

    let expected_grid = gridfile(&root, "blocked_patch", 1, "hex");
    let expected_patch = root.join("blocked_patch/tmpfile/mask_patch_bbox_0_01.nc4");
    let marker = legacy_record_path(&expected_grid);
    let grid_before = fs::read(&expected_grid).expect("read first native");
    assert!(marker.is_file(), "first run should publish marker");
    fs::remove_file(&expected_patch).expect("remove previous patch cache");
    fs::create_dir(&expected_patch).expect("block patch cache destination with directory");

    let error = try_run_final_base(&namelist, &root).expect_err("blocked patch output must fail");
    assert!(
        error.to_string().contains("directory") || error.to_string().contains("regular file"),
        "unexpected error: {error}"
    );
    assert_eq!(
        fs::read(&expected_grid).expect("read native after failed rerun"),
        grid_before
    );
    assert!(
        !marker.exists(),
        "readiness marker must stay withdrawn after failed rerun"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_patch_source_rerun_withdraws_previous_marker() {
    let root = temp_root("missing_patch_source_delivery");
    let sources = root.join("sources");
    fs::create_dir(&sources).unwrap();
    let patch_source = sources.join("patch_source.nc4");
    write_bbox(&patch_source, -10.0, 10.0, -10.0, 10.0);
    let namelist = write_global_patch_namelist(&root, "missing_patch", &patch_source);
    run_final_base(&namelist, &root);

    let expected_grid = gridfile(&root, "missing_patch", 1, "hex");
    let marker = legacy_record_path(&expected_grid);
    let grid_before = fs::read(&expected_grid).expect("read first native");
    assert!(marker.is_file(), "first run should publish marker");
    fs::remove_dir_all(&sources).expect("remove patch source directory");

    let error = try_run_final_base(&namelist, &root).expect_err("missing patch source must fail");
    assert!(
        error.to_string().contains("No such file")
            || error.to_string().contains("no mask sources")
            || error.kind() == std::io::ErrorKind::NotFound,
        "unexpected error: {error}"
    );
    assert_eq!(
        fs::read(&expected_grid).expect("read native after failed rerun"),
        grid_before
    );
    assert!(
        !marker.exists(),
        "readiness marker must be withdrawn before source discovery failure"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn patch_source_must_not_alias_published_patch_cache() {
    let root = temp_root("patch_source_output_alias_delivery");
    let case_dir = root.join("alias_patch/tmpfile");
    fs::create_dir_all(&case_dir).expect("create case tmpfile");
    let patch_source = case_dir.join("mask_patch_bbox_0_01.nc4");
    write_bbox(&patch_source, -10.0, 10.0, -10.0, 10.0);
    let source_before = fs::read(&patch_source).expect("read alias source before");
    let namelist = write_global_patch_namelist(&root, "alias_patch", &patch_source);

    let error =
        try_run_final_base(&namelist, &root).expect_err("patch source/output alias must fail");
    assert!(
        error.to_string().contains("same file")
            || error.to_string().contains("aliases")
            || error.to_string().contains("distinct"),
        "unexpected error: {error}"
    );
    assert_eq!(
        fs::read(&patch_source).expect("read alias source after"),
        source_before
    );
    let expected_grid = gridfile(&root, "alias_patch", 1, "hex");
    assert!(!legacy_record_path(&expected_grid).exists());

    let _ = fs::remove_dir_all(root);
}
