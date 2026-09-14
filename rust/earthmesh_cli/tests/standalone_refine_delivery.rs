mod support;

use earthmesh_cli::{
    bbox_mask_io::{write_bbox_mask_netcdf, BBoxMask, BBoxPoint},
    circle_close_mask_io::{write_circle_mask_netcdf, CircleMask},
    coordinate_types::LonLatPoint,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "earthmesh_standalone_refine_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn run_cli(root: &Path, namelist: &Path, extra: &[&str]) -> std::process::Output {
    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let mut command = std::process::Command::new(exe);
    command.arg(namelist).arg("--max-tris").arg("200000");
    for arg in extra {
        command.arg(arg);
    }
    support::output(command.current_dir(root)).expect("run earthmesh_cli")
}

fn stdout_field(stdout: &str, key: &str) -> PathBuf {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("missing {key}: {stdout}"))
}

fn marker_path(root: &Path, case: &str) -> PathBuf {
    root.join(case)
        .join("result/final_quality/refinement/legacy_delivery.json")
}

fn delivery_record(root: &Path, case: &str) -> serde_json::Value {
    let marker = marker_path(root, case);
    serde_json::from_slice(&fs::read(&marker).unwrap_or_else(|error| {
        panic!(
            "read standalone refine marker {}: {error}",
            marker.display()
        )
    }))
    .unwrap_or_else(|error| {
        panic!(
            "parse standalone refine marker {}: {error}",
            marker.display()
        )
    })
}

fn assert_no_delivery_staging(root: &Path) {
    fn walk(path: &Path) {
        for entry in
            fs::read_dir(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        {
            let path = entry.expect("dir entry").path();
            assert!(
                !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".earthmesh-delivery-")),
                "delivery staging leaked: {}",
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

fn artifact_bytes(paths: &[&Path]) -> BTreeMap<PathBuf, Vec<u8>> {
    paths
        .iter()
        .map(|path| {
            (
                (*path).to_path_buf(),
                fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
            )
        })
        .collect()
}

fn assert_artifacts_unchanged(before: &BTreeMap<PathBuf, Vec<u8>>) {
    for (path, bytes) in before {
        assert_eq!(
            fs::read(path).unwrap_or_else(|e| panic!("read {} after rerun: {e}", path.display())),
            *bytes,
            "artifact changed: {}",
            path.display()
        );
    }
}

fn write_threshold_matrix(path: &Path, var: &str, nlon: usize, nlat: usize, values: &[f64]) {
    assert_eq!(values.len(), nlon * nlat);
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create threshold file");
    file.add_dimension("lon", nlon).expect("lon dim");
    file.add_dimension("lat", nlat).expect("lat dim");
    file.add_variable::<f64>(var, &["lon", "lat"])
        .expect("threshold variable")
        .put_values(values, (.., ..))
        .expect("write threshold values");
}

fn write_refine_circle(prefix: &Path) -> PathBuf {
    let path = prefix.with_file_name(format!(
        "{}001.nc4",
        prefix.file_name().unwrap().to_string_lossy()
    ));
    write_circle_mask_netcdf(
        &path,
        &CircleMask {
            refine_degree: 1,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_500.0],
        },
    )
    .expect("write refine circle");
    path
}

fn write_domain_bbox(prefix: &Path) -> PathBuf {
    let path = prefix.with_file_name(format!(
        "{}001.nc4",
        prefix.file_name().unwrap().to_string_lossy()
    ));
    write_bbox_mask_netcdf(
        &path,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: 80.0,
                east: 170.0,
                south: -10.0,
                north: 60.0,
            }],
        },
    )
    .expect("write domain bbox");
    path
}

fn write_close_domain(path: &Path) {
    fs::write(
        path,
        "close_num = 4\nclose_refine = 0\n80.0 -10.0\n170.0 -10.0\n170.0 60.0\n80.0 60.0\n",
    )
    .expect("write close domain");
}

fn write_all_ocean_landtype(path: &Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create all-ocean landtype");
    file.add_dimension("longitude", 360).expect("longitude dim");
    file.add_dimension("latitude", 180).expect("latitude dim");
    file.add_variable::<i8>("landtype", &["latitude", "longitude"])
        .expect("landtype variable")
        .put_values(&vec![0_i8; 360 * 180], (.., ..))
        .expect("write all-ocean landtype");
}

fn write_method_c_namelist(
    root: &Path,
    case: &str,
    refine_prefix: &Path,
    domain_prefix: Option<&Path>,
    output_format: &str,
) -> PathBuf {
    let path = root.join(format!("{case}.nml"));
    let base_dir = format!("{}/", root.display());
    let domain = domain_prefix
        .map(|prefix| format!("  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{}'\n", prefix.display()))
        .unwrap_or_else(|| "  NL%mask_domain_global=.true.\n".to_string());
    fs::write(
        &path,
        format!(
            "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n{domain}  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{}'\n/\n",
            refine_prefix.display()
        ),
    )
    .expect("write Method-C namelist");
    path
}

fn write_clean_ocean_refine_namelist(
    root: &Path,
    case: &str,
    refine_prefix: &Path,
    close: &Path,
    landtype: &Path,
) -> PathBuf {
    let path = root.join(format!("{case}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &path,
        format!(
            "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{}'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='close'\n  NL%mask_domain_fprefix='{}'\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{}'\n/\n",
            landtype.display(),
            close.display(),
            refine_prefix.display()
        ),
    )
    .expect("write clean-ocean refine namelist");
    path
}

fn write_redgreen_namelist(root: &Path, case: &str, refine_prefix: &Path) -> PathBuf {
    let path = root.join(format!("{case}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &path,
        format!(
            "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='red_green'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{}'\n/\n",
            refine_prefix.display()
        ),
    )
    .expect("write RedGreen namelist");
    path
}

fn cmrc_namelist(root: &Path, case: &str) -> String {
    format!(
        "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{}/'\n  NL%NXP=3\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='certified'\n  NL%mask_domain_global=.true.\n  NL%landtype_file='none'\n/\n&certified\n  NL%mode='safe_mother_only'\n  NL%delivery='hex'\n  NL%maximum_level=2\n  NL%maximum_cells=1000\n  NL%gradation_rings_per_level=3\n  NL%search_budget=100\n/\n",
        root.display()
    )
}

fn write_hfield_mpas_namelist(root: &Path, case: &str, threshold_dir: &Path) -> PathBuf {
    let path = root.join(format!("{case}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &path,
        format!(
            "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%threshold_dir='{}'\n  RL%refine_lai_s=.true.\n  RL%th_lai_s=2.0\n/\n&hfield\n  NL%hfield_on=.true.\n  NL%hfield_g=0.2\n  NL%hfield_max_level=1\n  NL%hfield_nlon=36\n  NL%hfield_nlat=18\n/\n",
            threshold_dir.display()
        ),
    )
    .expect("write HField MPAS namelist");
    path
}

fn write_cartesian_refine_namelist(root: &Path, case: &str) -> PathBuf {
    let path = root.join(format!("{case}.nml"));
    let base_dir = format!("{}/", root.display());
    fs::write(
        &path,
        format!(
            "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%deltax=1000000.0\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=5\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=0.0\n  NL%grdlon(2,1)=0.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=0\n/\n"
        ),
    )
    .expect("write Cartesian refine namelist");
    path
}

fn assert_completion_record(
    root: &Path,
    case: &str,
    gridfile: &Path,
    expected_euler: serde_json::Value,
) -> serde_json::Value {
    let record = delivery_record(root, case);
    assert_eq!(record["kind"], "earthmesh_legacy_delivery");
    assert_eq!(record["gridfile"], gridfile.display().to_string());
    assert!(
        !record.to_string().contains(".earthmesh-delivery-"),
        "staged path leaked: {record}"
    );
    for section in ["model_artifacts", "auxiliary_artifacts"] {
        if let Some(artifacts) = record[section].as_object() {
            for path in artifacts.values().filter_map(|value| value.as_str()) {
                let path = PathBuf::from(path);
                if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    let json = fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
                    assert!(
                        !json.contains(".earthmesh-delivery-"),
                        "published JSON artifact contains staging path {}: {json}",
                        path.display()
                    );
                }
            }
        }
    }
    assert!(
        record["final_quality"]["report"]
            .as_str()
            .is_some_and(|p| p.ends_with("/final_quality/refinement/quality_summary.json")),
        "stable final quality path missing in {record}"
    );
    let quality_path = PathBuf::from(record["final_quality"]["report"].as_str().unwrap());
    let quality: serde_json::Value =
        serde_json::from_slice(&fs::read(&quality_path).unwrap()).unwrap();
    assert_eq!(
        quality["topology"]["expected_euler_characteristic"],
        expected_euler
    );
    assert!(gridfile.is_file());
    record
}

#[test]
fn cli_default_method_c_regional_refine_writes_stable_completion_and_keeps_inputs() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("method_c_default_regional");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let refine_prefix = sources.join("refine_circle_");
    let domain_prefix = sources.join("domain_bbox_");
    let refine_mask = write_refine_circle(&refine_prefix);
    let domain_mask = write_domain_bbox(&domain_prefix);
    let case = "case_method_c_default_regional";
    let namelist =
        write_method_c_namelist(&root, case, &refine_prefix, Some(&domain_prefix), "MPAS");
    let before_inputs = artifact_bytes(&[
        namelist.as_path(),
        refine_mask.as_path(),
        domain_mask.as_path(),
    ]);

    let output = run_cli(&root, &namelist, &[]);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    let gridfile = stdout_field(&stdout, "gridfile=");
    let record = assert_completion_record(&root, case, &gridfile, serde_json::Value::Null);
    assert_eq!(record["model_delivery_status"], "native_only");
    assert!(record["model_artifacts"].as_object().unwrap().is_empty());
    assert_artifacts_unchanged(&before_inputs);
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_passthrough_redgreen_global_refine_writes_completion_with_closed_sphere_quality() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("redgreen_passthrough_global");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let refine_prefix = sources.join("refine_circle_");
    write_refine_circle(&refine_prefix);
    let case = "case_redgreen_passthrough_global";
    let namelist = write_redgreen_namelist(&root, case, &refine_prefix);

    let output = run_cli(
        &root,
        &namelist,
        &[
            "--run-refine-passthrough",
            "--source-gridnum-perdegree",
            "1",
            "--source-nlons",
            "6",
            "--source-nlats",
            "6",
            "--source-first-triangle-id",
            "1",
        ],
    );
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    let gridfile = stdout_field(&stdout, "gridfile=");
    let record = assert_completion_record(&root, case, &gridfile, serde_json::json!(2));
    assert_eq!(record["model_delivery_status"], "native_only");
    assert!(record["model_artifacts"].as_object().unwrap().is_empty());
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_hfield_mpas_refine_delivers_model_outputs_with_persisted_width_context() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("hfield_mpas_delivery");
    let threshold_dir = root.join("threshold");
    fs::create_dir_all(&threshold_dir).expect("create threshold dir");
    let src_nlon = 72;
    let src_nlat = 18;
    let mut values = vec![0.0; src_nlon * src_nlat];
    for i in 0..src_nlon {
        let lon = -180.0 + (i as f64 + 0.5) * 360.0 / src_nlon as f64;
        for j in 0..src_nlat {
            let lat = 90.0 - (j as f64 + 0.5) * 180.0 / src_nlat as f64;
            if (80.0..=150.0).contains(&lon) && (0.0..=50.0).contains(&lat) {
                values[i * src_nlat + j] = if i % 2 == 0 { 10.0 } else { 0.0 };
            }
        }
    }
    let threshold = threshold_dir.join("lai.nc");
    write_threshold_matrix(&threshold, "lai", src_nlon, src_nlat, &values);
    let case = "case_hfield_mpas_delivery";
    let namelist = write_hfield_mpas_namelist(&root, case, &threshold_dir);

    let output = run_cli(&root, &namelist, &[]);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    let gridfile = stdout_field(&stdout, "gridfile=");
    let record = assert_completion_record(&root, case, &gridfile, serde_json::json!(2));
    assert_eq!(record["model_delivery_status"], "model_delivered");
    let model = record["model_artifacts"].as_object().unwrap();
    assert!(
        model.values().any(|value| value
            .as_str()
            .is_some_and(|path| path.ends_with("/graph.info"))),
        "missing MPAS graph artifact in {record}"
    );
    assert!(
        model
            .values()
            .any(|value| value.as_str().is_some_and(|path| path.ends_with(".nc4"))),
        "missing MPAS mesh artifact in {record}"
    );
    for path in model.values().filter_map(|value| value.as_str()) {
        assert!(Path::new(path).is_file(), "missing model artifact {path}");
    }
    let hfield = earthmesh_cli::hfield_gridfile_context::read_hfield_gridfile_context(&gridfile)
        .unwrap()
        .expect("admitted HField grid keeps demand context");
    assert_eq!((hfield.field.nlon(), hfield.field.nlat()), (36, 18));
    let widths = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(&gridfile)
        .unwrap()
        .expect("admitted HField grid keeps MPAS width context");
    assert_eq!(widths.source, "method_c_hfield_quantized_w_demand_v1");
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_clean_ocean_refined_tri_delivers_fvcom_with_embedded_obc() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("clean_ocean_refined_fvcom");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    let refine_prefix = sources.join("refine_circle_");
    write_refine_circle(&refine_prefix);
    let close = root.join("domain_close.nml");
    write_close_domain(&close);
    let landtype = root.join("all_ocean.nc");
    write_all_ocean_landtype(&landtype);
    let case = "case_clean_ocean_refined_fvcom";
    let namelist =
        write_clean_ocean_refine_namelist(&root, case, &refine_prefix, &close, &landtype);

    let output = run_cli(
        &root,
        &namelist,
        &[
            "--run-refine-landtype-source",
            "--source-gridnum-perdegree",
            "1",
        ],
    );
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    let gridfile = stdout_field(&stdout, "gridfile=");
    let record = assert_completion_record(&root, case, &gridfile, serde_json::Value::Null);
    assert_eq!(record["model_delivery_status"], "model_delivered");
    assert!(
        record["model_artifacts"]
            .as_object()
            .unwrap()
            .values()
            .any(|value| value.as_str().is_some_and(|path| path.ends_with(".2dm"))),
        "missing FVCOM model artifact in {record}"
    );
    let aux = record["auxiliary_artifacts"]
        .as_object()
        .expect("auxiliary artifacts");
    let obc = aux
        .values()
        .filter_map(|value| value.as_str())
        .map(PathBuf::from)
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("obc"))
                && path.extension().and_then(|ext| ext.to_str()) == Some("nc4")
        })
        .expect("published OBC auxiliary");
    let sidecar_obc = earthmesh_cli::obc_boundary_io::read_obc_order_netcdf(&obc)
        .expect("read published OBC sidecar");
    assert!(!sidecar_obc.is_empty());
    assert_eq!(
        earthmesh_cli::obc_boundary_io::read_gridfile_obc_order(&gridfile).unwrap(),
        Some(sidecar_obc)
    );
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_cartesian_refine_retains_native_output_without_readiness_marker() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("cartesian_refine_no_readiness");
    let case = "case_cartesian_refine_no_readiness";
    let namelist = write_cartesian_refine_namelist(&root, case);

    let output = run_cli(&root, &namelist, &[]);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(stderr.contains("Cartesian"), "stderr={stderr}");
    let gridfile = stdout_field(&stdout, "gridfile=");
    assert!(gridfile.is_file());
    assert!(
        !marker_path(&root, case).exists(),
        "Cartesian standalone refine must not publish spherical readiness"
    );
    let native_before = fs::read(&gridfile).unwrap();
    let backup = gridfile.with_extension("nc4.bak");
    fs::rename(&gridfile, &backup).expect("move Cartesian native aside");
    fs::create_dir(&gridfile).expect("block Cartesian native path with directory");
    let sentinel = gridfile.join("sentinel.txt");
    fs::write(&sentinel, b"keep Cartesian blocker").expect("write Cartesian blocker");
    let blocked = run_cli(&root, &namelist, &[]);
    assert!(
        !blocked.status.success(),
        "blocked Cartesian native output should fail\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&blocked.stdout),
        String::from_utf8_lossy(&blocked.stderr)
    );
    assert_eq!(fs::read(&backup).unwrap(), native_before);
    assert_eq!(fs::read(&sentinel).unwrap(), b"keep Cartesian blocker");
    assert!(
        !marker_path(&root, case).exists(),
        "failed Cartesian rerun must not publish readiness"
    );
    fs::remove_dir_all(&gridfile).expect("remove Cartesian blocker directory");
    fs::rename(&backup, &gridfile).expect("restore Cartesian native");
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn raw_refine_pipeline_api_remains_unchecked_without_completion_record() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("raw_api_unchecked");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let refine_prefix = sources.join("refine_circle_");
    write_refine_circle(&refine_prefix);
    let case = "case_raw_api_unchecked";
    let namelist = write_method_c_namelist(&root, case, &refine_prefix, None, "CoLM");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 20_000, None)
        .expect("raw refine pipeline still runs");
    assert!(run.output.output.exists());
    assert!(
        !marker_path(&root, case).exists(),
        "raw public refine API must not create standalone delivery record"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_refine_failed_rerun_with_bad_refine_source_preserves_prior_native_and_withdraws_marker() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("method_c_invalid_source_rollback");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let refine_prefix = sources.join("refine_circle_");
    let refine_mask = write_refine_circle(&refine_prefix);
    let case = "case_method_c_invalid_source_rollback";
    let namelist = write_method_c_namelist(&root, case, &refine_prefix, None, "CoLM");

    let first = run_cli(&root, &namelist, &[]);
    assert!(
        first.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let gridfile = stdout_field(&String::from_utf8_lossy(&first.stdout), "gridfile=");
    let marker = marker_path(&root, case);
    let before = artifact_bytes(&[
        gridfile.as_path(),
        namelist.as_path(),
        refine_mask.as_path(),
    ]);
    assert!(marker.exists());

    let bad_namelist = root.join("bad_same_case_missing_refine_source.nml");
    let bad_prefix = sources.join("missing_refine_circle_");
    let bad_contents = fs::read_to_string(&namelist).unwrap().replace(
        &refine_prefix.display().to_string(),
        &bad_prefix.display().to_string(),
    );
    fs::write(&bad_namelist, bad_contents).unwrap();
    let failure = run_cli(&root, &bad_namelist, &[]);
    assert!(
        !failure.status.success(),
        "bad refine source should fail\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert_artifacts_unchanged(&before);
    assert!(
        !marker.exists(),
        "failed valid-config refine attempt must withdraw readiness"
    );
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_cmrc_rejects_local_update_path_that_aliases_readiness_marker_without_retiring_input() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("cmrc_local_update_marker_alias");
    let case = "case_cmrc_local_update_marker_alias";
    let namelist = root.join("cmrc_marker_alias.nml");
    fs::write(&namelist, cmrc_namelist(&root, case)).unwrap();
    let marker = marker_path(&root, case);
    fs::create_dir_all(marker.parent().unwrap()).expect("create marker parent");
    let sentinel = br#"{"kind":"external-local-update-sentinel"}"#;
    fs::write(&marker, sentinel).expect("seed marker-alias input sentinel");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = support::output(
        std::process::Command::new(exe)
            .arg(&namelist)
            .arg("--max-tris")
            .arg("200000")
            .env("EARTHMESH_CMRC_LOCAL_UPDATE", &marker)
            .current_dir(&root),
    )
    .expect("run CMRC marker-alias local update request");
    assert!(
        !output.status.success(),
        "marker-alias local update input must be rejected\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&marker).unwrap(), sentinel);
    assert_no_delivery_staging(&root);

    // The request's baseline meshes are immutable inputs too, not only the
    // request JSON itself. Reject aliases before withdrawing those source bytes.
    let request_path = root.join("local_update.json");
    for field in ["source_gridfile", "source_mpas"] {
        let mut request = serde_json::json!({
            "source_gridfile": namelist,
            "source_mpas": namelist,
            "target_cell": 0,
            "updates": [],
        });
        request[field] = serde_json::json!(marker);
        fs::write(&request_path, request.to_string()).unwrap();
        let output = support::output(
            std::process::Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
                .arg(&namelist)
                .env("EARTHMESH_CMRC_LOCAL_UPDATE", &request_path)
                .current_dir(&root),
        )
        .unwrap();
        assert!(!output.status.success());
        assert_eq!(fs::read(&marker).unwrap(), sentinel, "{field} was retired");
        assert_no_delivery_staging(&root);
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_cmrc_mpas_completion_keeps_certificate_ready_and_model_outputs() {
    let _guard = NETCDF_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let root = temp_root("cmrc_mpas_delivery");
    let case = "case_cmrc_mpas_delivery";
    let namelist = root.join("cmrc_mpas.nml");
    fs::write(&namelist, cmrc_namelist(&root, case)).unwrap();

    let output = run_cli(&root, &namelist, &[]);
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let gridfile = stdout_field(&String::from_utf8_lossy(&output.stdout), "gridfile=");
    let record = assert_completion_record(&root, case, &gridfile, serde_json::json!(2));
    assert_eq!(record["model_delivery_status"], "model_delivered");
    for path in record["model_artifacts"].as_object().unwrap().values() {
        let path = PathBuf::from(path.as_str().expect("model path"));
        assert!(path.is_file(), "missing model artifact {}", path.display());
    }
    let model_text = record["model_artifacts"].to_string();
    assert!(
        model_text.contains("MPASOUT_NXP0003_global.nc4"),
        "{record}"
    );
    assert!(
        model_text.contains("MPASOUT_NXP0003_global.graph.info"),
        "{record}"
    );
    let aux = record["auxiliary_artifacts"]
        .as_object()
        .expect("auxiliary artifacts");
    for path in aux.values() {
        let path = PathBuf::from(path.as_str().expect("aux path"));
        assert!(
            path.exists(),
            "missing auxiliary artifact {}",
            path.display()
        );
    }
    let aux_text = serde_json::to_string(aux).unwrap();
    let resources: serde_json::Value = serde_json::from_slice(
        &fs::read(aux["result/certified_resources.json"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    for (field, key) in [
        ("manifest", "result/certified_manifest.json"),
        ("certificate", "result/certified_certificate.json"),
    ] {
        assert_eq!(
            resources["artifact_bytes"][field].as_u64(),
            Some(fs::metadata(aux[key].as_str().unwrap()).unwrap().len())
        );
    }
    for required in [
        "certified_certificate.json",
        "certified_manifest.json",
        "certified_resources.json",
        "certified_ready",
    ] {
        assert!(
            aux_text.contains(required),
            "missing {required} in {record}"
        );
    }

    let graph = record["model_artifacts"]
        .as_object()
        .unwrap()
        .values()
        .find_map(|value| {
            let path = value.as_str()?;
            path.ends_with(".graph.info").then(|| PathBuf::from(path))
        })
        .expect("graph artifact path");
    let marker = marker_path(&root, case);
    let mut protected_paths = vec![gridfile.clone(), namelist.clone()];
    protected_paths.extend(
        record["model_artifacts"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|value| value.as_str())
            .map(PathBuf::from)
            .filter(|path| path != &graph),
    );
    protected_paths.extend(
        aux.values()
            .filter_map(|value| value.as_str())
            .map(PathBuf::from),
    );
    let protected_refs = protected_paths
        .iter()
        .map(PathBuf::as_path)
        .collect::<Vec<_>>();
    let protected = artifact_bytes(&protected_refs);
    assert!(marker.exists());
    let graph_before = fs::read(&graph).unwrap();
    let graph_backup = graph.with_file_name("blocked_graph_backup.graph.info");
    fs::rename(&graph, &graph_backup).expect("move old graph output aside");
    fs::create_dir(&graph).expect("block graph output with directory");
    let sentinel = graph.join("sentinel.txt");
    fs::write(&sentinel, b"keep graph blocker").expect("write graph blocker sentinel");
    let failure = run_cli(&root, &namelist, &[]);
    assert!(
        !failure.status.success(),
        "blocked graph output should fail without replacing prior bundle\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert_artifacts_unchanged(&protected);
    assert!(
        !marker.exists(),
        "failed valid-config CMRC attempt must withdraw readiness"
    );
    assert_eq!(fs::read(&graph_backup).unwrap(), graph_before);
    assert_eq!(fs::read(&sentinel).unwrap(), b"keep graph blocker");
    fs::remove_dir_all(&graph).expect("remove graph blocker directory");
    fs::rename(&graph_backup, &graph).expect("restore old graph output");
    assert_no_delivery_staging(&root);

    let _ = fs::remove_dir_all(&root);
}
