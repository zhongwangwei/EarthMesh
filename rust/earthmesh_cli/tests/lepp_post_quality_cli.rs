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

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg(&namelist)
        .arg("--max-tris")
        .arg("20000")
        .arg("--run-refine-passthrough")
        .current_dir(&root)
        .output()
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

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg(&namelist)
        .arg("--max-tris")
        .arg("20000")
        .arg("--run-refine-passthrough")
        .current_dir(&root)
        .output()
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
fn cli_lepp_adaptive_hybrid_is_the_selected_method_c_production_path() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("lepp_adaptive_hybrid");
    let case_name = "case_lepp_adaptive_hybrid";
    let namelist = write_method_c_lepp_namelist(
        &root,
        case_name,
        "method_c",
        "&method_c\n  NL%algorithm='lepp_delaunay'\n  NL%max_cycles=1\n  NL%maximum_insertions_per_cycle=1\n  NL%maximum_neighbor_size_ratio=10.0\n/",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg(&namelist)
        .arg("--max-tris")
        .arg("20000")
        .arg("--run-refine-passthrough")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status={:?}\nstdout={stdout}\nstderr={stderr}",
        output.status.code()
    );

    let gridfile = root.join(format!("{case_name}/result/gridfile_NXP0006_hex.nc4"));
    assert!(gridfile.is_file(), "missing LEPP output: {gridfile:?}");
    assert_eq!(stdout_value(&stdout, "lepp_adaptive_cycles"), "1");
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

    let _ = fs::remove_dir_all(&root);
}
