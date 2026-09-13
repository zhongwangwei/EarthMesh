mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use earthmesh_cli::{
    circle_close_mask_io::{write_circle_mask_netcdf, CircleMask},
    coordinate_types::LonLatPoint,
};

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn stdout_value<'a>(stdout: &'a str, key: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("missing {key} in stdout:\n{stdout}"))
}

fn assert_hex_publication(path: &Path) {
    let mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(path)
        .unwrap_or_else(|error| panic!("read HEX publication {}: {error}", path.display()));
    earthmesh_cli::unstructured_mesh_support::validate_published_cell_degrees(&mesh, "hex")
        .unwrap_or_else(|error| panic!("validate HEX publication {}: {error}", path.display()));
    assert!(
        mesh.n_w_to_m
            .iter()
            .filter(|&&degree| degree > 1)
            .all(|&degree| (5..=7).contains(&degree)),
        "HEX physical W rings must stay 5..=7 in {}",
        path.display()
    );
}

fn write_method_c_lepp_namelist(
    root: &Path,
    case_name: &str,
    backend: &str,
    lepp_config: &str,
) -> PathBuf {
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 1,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_500.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join(format!("{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='{backend}'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=1\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n{lepp_config}\n",
        ),
    )
    .expect("write namelist");
    namelist
}

#[test]
fn cli_lepp_post_quality_writes_separate_artifacts_without_replacing_canonical_gridfile() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_post_quality_method_c");
    let case_name = "case_lepp_post_quality_method_c";
    let namelist = write_method_c_lepp_namelist(
        &root,
        case_name,
        "method_c",
        "&quality\n  NL%lepp_post_quality=.true.\n  NL%lepp_post_quality_max_insertions=1\n  NL%lepp_post_quality_max_edge_km=1300.0\n/",
    );

    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg(&namelist)
            .arg("--max-tris")
            .arg("20000")
            .arg("--run-refine-passthrough")
            .current_dir(&root),
    )
    .expect("run earthmesh_cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status={:?}\nstdout={stdout}\nstderr={stderr}",
        output.status.code()
    );

    let canonical = root.join(format!("{case_name}/result/gridfile_NXP0006_hex.nc4"));
    assert!(
        canonical.is_file(),
        "missing canonical output: {canonical:?}"
    );
    assert!(stdout.contains(&format!("gridfile={}", canonical.display())));

    assert_eq!(stdout_value(&stdout, "lepp_post_quality_committed"), "1");
    let lepp_gridfile = PathBuf::from(stdout_value(&stdout, "lepp_post_quality_gridfile"));
    let lepp_report = PathBuf::from(stdout_value(&stdout, "lepp_post_quality_report"));
    assert_ne!(lepp_gridfile, canonical);
    assert_eq!(
        lepp_gridfile.file_stem().unwrap(),
        "gridfile_NXP0006_hex_lepp"
    );
    assert!(
        lepp_gridfile.is_file(),
        "missing LEPP gridfile: {lepp_gridfile:?}"
    );
    assert!(
        lepp_report.is_file(),
        "missing LEPP report: {lepp_report:?}"
    );
    assert_eq!(
        lepp_report.extension().and_then(|value| value.to_str()),
        Some("json")
    );
    let canonical_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&canonical)
            .expect("read canonical gridfile");
    let lepp_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&lepp_gridfile)
            .expect("read LEPP gridfile");
    assert_eq!(lepp_mesh.m_points.len(), canonical_mesh.m_points.len() + 2);
    assert_eq!(lepp_mesh.w_points.len(), canonical_mesh.w_points.len() + 1);
    assert_hex_publication(&canonical);
    assert_hex_publication(&lepp_gridfile);

    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lepp_report).expect("read LEPP report"))
            .expect("parse LEPP report");
    assert_eq!(report["committed"], 1);
    assert_eq!(report["canonical_output"], canonical.display().to_string());
    assert_eq!(
        report["optimized_output"],
        lepp_gridfile.display().to_string()
    );
    assert_eq!(report["insertions"].as_array().map(Vec::len), Some(1));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_lepp_post_quality_rejects_non_method_c_backend() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_post_quality_red_green");
    let namelist = write_method_c_lepp_namelist(
        &root,
        "case_lepp_post_quality_red_green",
        "red_green",
        "&quality\n  NL%lepp_post_quality=.true.\n  NL%lepp_post_quality_max_insertions=1\n  NL%lepp_post_quality_max_edge_km=1300.0\n/",
    );

    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg(&namelist)
            .arg("--max-tris")
            .arg("20000")
            .arg("--run-refine-passthrough")
            .current_dir(&root),
    )
    .expect("run earthmesh_cli");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("NL%lepp_post_quality requires NL%refine_backend='method_c'"),
        "stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_lepp_adaptive_hybrid_tri_preserves_single_insertion_and_partial_demand() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_adaptive_hybrid");
    let case_name = "case_lepp_adaptive_hybrid";
    let namelist = write_method_c_lepp_namelist(
        &root,
        case_name,
        "method_c",
        "&method_c\n  NL%algorithm='lepp_delaunay'\n  NL%max_cycles=1\n  NL%maximum_insertions_per_cycle=1\n  NL%maximum_neighbor_size_ratio=10.0\n/",
    );
    let contents = fs::read_to_string(&namelist)
        .unwrap()
        .replace("NL%mode_grid='hex'", "NL%mode_grid='tri'");
    fs::write(&namelist, contents).unwrap();
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg(&namelist)
            .arg("--max-tris")
            .arg("20000")
            .arg("--run-refine-passthrough")
            .current_dir(&root),
    )
    .expect("run earthmesh_cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status={:?}\nstdout={stdout}\nstderr={stderr}",
        output.status.code()
    );

    let gridfile = root.join(format!("{case_name}/result/gridfile_NXP0006_tri.nc4"));
    assert!(gridfile.is_file(), "missing LEPP output: {gridfile:?}");
    assert_eq!(stdout_value(&stdout, "lepp_adaptive_cycles"), "1");
    assert_eq!(stdout_value(&stdout, "refine_spring_iterations"), "1");
    assert!(
        stderr.contains("refinement spring started"),
        "LEPP must consume the common spring controls: {stderr}"
    );
    assert_eq!(
        stdout_value(&stdout, "lepp_adaptive_physical_insertions"),
        "1"
    );
    let report = PathBuf::from(stdout_value(&stdout, "lepp_adaptive_report"));
    let unresolved = PathBuf::from(stdout_value(&stdout, "lepp_adaptive_unresolved_report"));
    assert!(
        report.is_file(),
        "missing AdaptiveHybrid report: {report:?}"
    );
    assert!(
        unresolved.is_file(),
        "missing unresolved-demand report: {unresolved:?}"
    );
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report).expect("read AdaptiveHybrid report"))
            .expect("parse AdaptiveHybrid report");
    assert_eq!(report["algorithm"], "lepp_delaunay");
    assert_eq!(report["mode"], "adaptive_hybrid");
    assert_eq!(report["canonical_method_c_compatible"], false);
    assert_eq!(report["insertions"]["physical"], 1);
    assert_eq!(
        report["resolved_target_semantics"],
        "lepp_resolved_region_targets_v1"
    );
    let targets = report["resolved_targets"].as_array().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0]["region"]["type"], "circle");
    assert!(targets[0]["resolved_target_edge_m"].as_f64().unwrap() > 0.0);
    assert!(
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&gridfile)
            .unwrap()
            .is_none(),
        "partial coverage must not invent a background width"
    );
    assert!(stderr.contains("no background width was supplied"));
    let error = earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
        &gridfile,
        root.join("unavailable"),
        earthmesh_project::ModelFormat::Mpas,
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("missing persisted MPAS width context"));
    assert!(!root.join("unavailable/mesh.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_lepp_resolved_demand_delivers_global_and_regional_mpas() {
    use earthmesh_project::ModelFormat;
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_resolved_delivery");
    let namelist = write_method_c_lepp_namelist(
        &root, "lepp_resolved", "method_c",
        "&method_c\n NL%algorithm='lepp_delaunay'\n NL%max_cycles=1\n NL%maximum_insertions_per_cycle=2\n NL%maximum_neighbor_size_ratio=10.0\n/",
    );
    let contents = fs::read_to_string(&namelist)
        .unwrap()
        .lines()
        .map(|line| {
            if line.contains("RL%mask_refine_spc_type=") {
                " RL%mask_refine_spc_type='bbox'".to_string()
            } else if line.contains("RL%mask_refine_spc_fprefix=") {
                " RL%mask_refine_spc_fprefix='inline:bbox:w=-180,e=180,s=-90,n=90'".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&namelist, contents).unwrap();
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg(&namelist)
            .args(["--max-tris", "20000", "--run-refine-passthrough"])
            .current_dir(&root),
    )
    .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parent = Path::new(stdout_value(&stdout, "gridfile"));
    let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(parent)
        .unwrap()
        .expect("covered LEPP must retain actual resolved width");
    assert_eq!(context.source, "lepp_resolved_region_w_demand_v1");
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(stdout_value(&stdout, "lepp_adaptive_report")).unwrap())
            .unwrap();
    assert_eq!(report["lepp_paths"]["committed"], 2);
    assert_eq!(report["insertions"]["physical"], 1);
    assert_eq!(report["insertions"]["balance"], 1);
    assert_eq!(
        report["resolved_target_semantics"],
        "lepp_resolved_region_targets_v1"
    );
    assert_eq!(report["resolved_target_units"], "m");
    let targets = report["resolved_targets"].as_array().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(
        targets[0]["original_target_edge_m"],
        serde_json::Value::Null
    );
    assert_eq!(targets[0]["source_resolution_m"], serde_json::Value::Null);
    assert_eq!(targets[0]["region"]["type"], "bbox");
    let expected = targets[0]["resolved_target_edge_m"].as_f64().unwrap();
    let base_level_proxy =
        2.0 * std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 30.0 / 2.0;
    assert!((expected - base_level_proxy).abs() > 1000.0);
    assert_eq!(context.density_reference_width_km, expected / 1000.0);
    assert!(context.cellwidth_km.iter().all(|&w| w == expected / 1000.0));
    let regional = root.join("regional.nc4");
    let kept = earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
        parent,
        &regional,
        &earthmesh_cli::coordinate_types::GridRegion::Bbox {
            west: -150.0,
            east: -60.0,
            south: -50.0,
            north: 50.0,
        },
        "hex",
    )
    .unwrap();
    assert!(kept > 0 && kept < context.cellwidth_km.len() - 2);
    let parent_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(parent).unwrap();
    assert_hex_publication(parent);
    let parent_cells = parent_mesh.n_w_to_m.iter().filter(|&&n| n > 1).count();
    let selected_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&regional).unwrap();
    earthmesh_cli::unstructured_mesh_support::validate_published_cell_degrees(
        &selected_mesh,
        "hex",
    )
    .unwrap();
    let selected_context =
        earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&regional)
            .unwrap()
            .unwrap();
    assert_eq!(selected_context.source, context.source);
    assert_eq!(
        selected_context.density_reference_width_km,
        context.density_reference_width_km
    );
    for format in [
        ModelFormat::Mpas,
        ModelFormat::MpasOcean,
        ModelFormat::MpasSimple,
    ] {
        for (scope, grid) in [("global", parent), ("regional", regional.as_path())] {
            let out = root.join(format!("{scope}_{format:?}"));
            let (mesh, graph) = if scope == "global" {
                earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
                    grid, &out, format,
                )
            } else {
                earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
                    grid, parent, &out, format,
                )
            }
            .unwrap();
            assert!(mesh.is_file());
            assert_eq!(graph.is_some(), format != ModelFormat::MpasSimple);
            if let Some(graph) = graph {
                assert!(graph.is_file());
            }
            let file = netcdf::open(&mesh).unwrap();
            assert_eq!(
                file.dimension("nCells").unwrap().len(),
                if scope == "global" {
                    parent_cells
                } else {
                    kept
                }
            );
            let density = file
                .variable("meshDensity")
                .unwrap()
                .get_values::<f64, _>(..)
                .unwrap();
            assert!(density.iter().all(|&d| d == 1.0));
            if format != ModelFormat::MpasSimple {
                let degrees = file
                    .variable("nEdgesOnCell")
                    .unwrap()
                    .get_values::<i32, _>(..)
                    .unwrap();
                assert!(degrees.iter().all(|n| (5..=7).contains(n)));
                let nominal = file
                    .variable("nominalMinDc")
                    .unwrap()
                    .get_values::<f64, _>(..)
                    .unwrap()[0];
                let radius = if format == ModelFormat::MpasOcean {
                    earthmesh_cli::MPAS_OCEAN_SPHERE_RADIUS_METERS
                } else {
                    1.0
                };
                assert!(
                    (nominal - expected / earthmesh_core::EARTH_RADIUS_METERS * radius).abs()
                        < 1.0e-8
                );
            }
        }
    }
    // Altered/imported degree-four parents remain rejected, even if selection is valid.
    let damaged = root.join("damaged_parent.nc4");
    fs::copy(parent, &damaged).unwrap();
    netcdf::append(&damaged)
        .unwrap()
        .variable_mut("n_ngrwm")
        .unwrap()
        .put_value(4_i32, 1)
        .unwrap();
    for format in [
        ModelFormat::Mpas,
        ModelFormat::MpasOcean,
        ModelFormat::MpasSimple,
    ] {
        for scope in ["global", "regional"] {
            let out = root.join(format!("damaged_{scope}_{format:?}"));
            let result = if scope == "global" {
                earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile(
                    &damaged, &out, format,
                )
            } else {
                earthmesh_cli::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
                    &regional, &damaged, &out, format,
                )
            };
            let error = result.unwrap_err();
            assert!(error.to_string().contains("5..=7"), "{error}");
            assert!(!out.join("mesh.nc4").exists());
            assert!(!out.join("graph.info").exists());
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_lepp_post_quality_hex_pair_progress_writes_valid_optimized_output() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_post_quality_hex_pair");
    let case = "case_lepp_post_quality_hex_pair";
    let namelist = write_method_c_lepp_namelist(
        &root,
        case,
        "method_c",
        "&quality\n NL%lepp_post_quality=.true.\n NL%lepp_post_quality_max_insertions=2\n NL%lepp_post_quality_max_edge_km=1300.0\n/",
    );
    let optimized = root.join(format!("{case}/result/gridfile_NXP0006_hex_lepp.nc4"));
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .args([
                namelist.as_os_str(),
                "--max-tris".as_ref(),
                "20000".as_ref(),
                "--run-refine-passthrough".as_ref(),
            ])
            .current_dir(&root),
    )
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status={:?}\nstdout={stdout}\nstderr={stderr}",
        output.status.code()
    );
    assert_eq!(stdout_value(&stdout, "lepp_post_quality_committed"), "2");
    let canonical = root.join(format!("{case}/result/gridfile_NXP0006_hex.nc4"));
    assert_hex_publication(&canonical);
    assert_hex_publication(&optimized);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_lepp_post_quality_hex_zero_progress_rejects_unchanged_optimized_success() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_post_quality_hex_zero_progress");
    let case = "case_lepp_post_quality_hex_zero_progress";
    let namelist = write_method_c_lepp_namelist(
        &root,
        case,
        "method_c",
        "&quality\n NL%lepp_post_quality=.true.\n NL%lepp_post_quality_max_insertions=1\n NL%lepp_post_quality_max_edge_km=0.001\n/",
    );
    let optimized = root.join(format!("{case}/result/gridfile_NXP0006_hex_lepp.nc4"));
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .args([
                namelist.as_os_str(),
                "--max-tris".as_ref(),
                "20000".as_ref(),
                "--run-refine-passthrough".as_ref(),
            ])
            .current_dir(&root),
    )
    .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("committed no insertions"), "{stderr}");
    assert!(stderr.contains("5..=7"), "{stderr}");
    assert!(!optimized.exists());
    let canonical = root.join(format!("{case}/result/gridfile_NXP0006_hex.nc4"));
    assert_hex_publication(&canonical);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_lepp_adaptive_hex_insufficient_budget_rejects_unchanged_publication() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_hex_insufficient_budget");
    let case = "lepp_hex_insufficient_budget";
    let namelist = write_method_c_lepp_namelist(
        &root, case, "method_c",
        "&method_c\n NL%algorithm='lepp_delaunay'\n NL%max_cycles=1\n NL%maximum_insertions_per_cycle=1\n NL%maximum_neighbor_size_ratio=10.0\n/",
    );
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg(&namelist)
            .args(["--max-tris", "20000", "--run-refine-passthrough"])
            .current_dir(&root),
    )
    .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("committed no insertions while demands remain unresolved"),
        "{stderr}"
    );
    assert!(!root
        .join(format!("{case}/result/gridfile_NXP0006_hex.nc4"))
        .exists());
    fs::remove_dir_all(root).unwrap();
}
