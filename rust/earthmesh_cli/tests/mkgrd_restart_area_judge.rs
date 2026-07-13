use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use earthmesh_cli::{
    area_judge_grid_io::read_area_judge_grid_netcdf,
    area_judge_grid_io::write_area_judge_grid_netcdf, area_judge_grid_io::AreaJudgeGridPayload,
    bbox_mask_io::write_bbox_mask_netcdf, bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
    coordinate_types::LonLatPoint, mkgrd_mask_restart::run_mkgrd_mask_restart_area_judge_namelist,
    mkgrd_restart_types::MkgrdRestartAreaJudgeOptions,
    unstructured_mesh_io::write_unstructured_mesh_netcdf,
    unstructured_mesh_support::UnstructuredMesh,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

static NETCDF_TEST_LOCK: Mutex<()> = Mutex::new(());

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn assert_restart_refine_stdout(stdout: &str, regions: usize, max_level: usize) {
    assert!(
        stdout.contains("mask_restart_action=MethodCRefine"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains(&format!("refine_regions={regions}")),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains(&format!("refine_max_level={max_level}")),
        "stdout={stdout}"
    );
    assert!(stdout.contains("gridfile="), "stdout={stdout}");
    assert!(
        !stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout should not report compatibility restart-refine handoff: {stdout}"
    );
    assert!(
        !stdout.contains("restart_refine_steps="),
        "stdout should not report compatibility restart-refine loop execution: {stdout}"
    );
    assert!(
        !stdout.contains("restart_refine_sources="),
        "stdout should not report compatibility restart-refine source execution: {stdout}"
    );
}

fn assert_method_c_default_refine_stdout(stdout: &str, regions: usize, max_level: usize) {
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains(&format!("refine_regions={regions}")),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains(&format!("refine_max_level={max_level}")),
        "stdout={stdout}"
    );
    assert!(stdout.contains("gridfile="), "stdout={stdout}");
    assert!(
        !stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout should not report compatibility restart-refine handoff: {stdout}"
    );
    assert!(
        !stdout.contains("restart_refine_steps="),
        "stdout should not report compatibility restart-refine loop execution: {stdout}"
    );
    assert!(
        !stdout.contains("restart_refine_source="),
        "stdout should not report compatibility restart-refine source labels: {stdout}"
    );
}

fn small_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let lon_vertex = vec![
        f64::NAN,
        -180.0,
        -179.0,
        -178.0,
        -177.0,
        -176.0,
        -175.0,
        -174.0,
    ];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| -179.5 + idx as f64))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| 89.5 - idx as f64))
        .collect::<Vec<_>>();
    (lon_vertex, lat_vertex, lon_i, lat_i)
}

fn restart_land_postproc_source_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint {
                lon: -176.5,
                lat: 86.5,
            },
            LonLatPoint {
                lon: -178.0,
                lat: 88.3,
            },
        ],
        w_points: vec![
            LonLatPoint {
                lon: -176.9,
                lat: 86.9,
            },
            LonLatPoint {
                lon: -176.1,
                lat: 86.9,
            },
            LonLatPoint {
                lon: -176.5,
                lat: 86.1,
            },
            LonLatPoint {
                lon: -178.8,
                lat: 88.8,
            },
            LonLatPoint {
                lon: -177.2,
                lat: 88.8,
            },
            LonLatPoint {
                lon: -178.0,
                lat: 87.2,
            },
        ],
        m_to_w: vec![[1, 2, 3], [4, 5, 6]],
        w_to_m: vec![
            vec![1, 1],
            vec![1, 1],
            vec![1, 1],
            vec![2, 2],
            vec![2, 2],
            vec![2, 2],
        ],
        n_w_to_m: vec![2, 2, 2, 2, 2, 2],
    }
}

fn restart_hex_postproc_source_mesh() -> UnstructuredMesh {
    let mut mesh = restart_land_postproc_source_mesh();
    for idx in 0..6 {
        let triangle_id = i32::try_from(mesh.m_points.len() + 1).expect("triangle id fits i32");
        let lon = -178.8 + (idx % 3) as f64 * 0.5;
        let lat = 88.7 - (idx / 3) as f64 * 0.5;
        mesh.m_points.push(LonLatPoint { lon, lat });
        let first_vertex = i32::try_from(mesh.w_points.len() + 1).expect("vertex id fits i32");
        mesh.w_points.push(LonLatPoint {
            lon: lon - 0.12,
            lat: lat - 0.12,
        });
        mesh.w_points.push(LonLatPoint {
            lon: lon + 0.12,
            lat: lat - 0.12,
        });
        mesh.w_points.push(LonLatPoint {
            lon,
            lat: lat + 0.12,
        });
        mesh.m_to_w
            .push([first_vertex, first_vertex + 1, first_vertex + 2]);
        mesh.w_to_m.push(vec![triangle_id, triangle_id]);
        mesh.w_to_m.push(vec![triangle_id, triangle_id]);
        mesh.w_to_m.push(vec![triangle_id, triangle_id]);
        mesh.n_w_to_m.push(2);
        mesh.n_w_to_m.push(2);
        mesh.n_w_to_m.push(2);
    }
    mesh
}

fn restart_ocean_postproc_source_mesh() -> UnstructuredMesh {
    let mut m_points = vec![
        LonLatPoint {
            lon: -176.0,
            lat: 86.0
        };
        8
    ];
    for point in m_points.iter_mut().take(5).skip(1) {
        point.lon = -178.0;
        point.lat = 88.0;
    }
    let mut w_points = vec![
        LonLatPoint {
            lon: -176.0,
            lat: 86.0
        };
        14
    ];
    for vertex_id in [2_usize, 3, 4, 5, 6, 10, 11, 12, 13] {
        w_points[vertex_id - 1] = LonLatPoint {
            lon: -178.8 + (vertex_id % 3) as f64 * 0.7,
            lat: 88.8 - (vertex_id % 2) as f64 * 0.8,
        };
    }
    let mut m_to_w = vec![[1, 1, 1]; 8];
    m_to_w[1] = [2, 10, 11];
    m_to_w[2] = [10, 11, 3];
    m_to_w[3] = [11, 12, 4];
    m_to_w[4] = [12, 13, 5];
    m_to_w[5] = [13, 10, 6];
    let mut w_to_m = vec![vec![1; 7]; 14];
    w_to_m[1] = vec![2, 1, 1, 1, 1, 1, 1];
    w_to_m[2] = vec![3, 1, 1, 1, 1, 1, 1];
    w_to_m[3] = vec![4, 1, 1, 1, 1, 1, 1];
    w_to_m[4] = vec![5, 1, 1, 1, 1, 1, 1];
    w_to_m[5] = vec![6, 1, 1, 1, 1, 1, 1];
    w_to_m[9] = vec![2, 3, 6, 7, 1, 1, 1];
    w_to_m[10] = vec![2, 3, 6, 7, 1, 1, 1];
    w_to_m[11] = vec![3, 4, 6, 7, 1, 1, 1];
    w_to_m[12] = vec![4, 5, 6, 7, 1, 1, 1];
    let n_w_to_m = vec![5; 14];
    UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    }
}

fn restart_atmos_mpas_full_source_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.2, lat: 0.2 },
            LonLatPoint { lon: 0.8, lat: 0.2 },
            LonLatPoint { lon: 0.2, lat: 0.8 },
            LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 3, 5],
            [2, 4, 5],
            [3, 4, 5],
        ],
        w_to_m: vec![
            vec![1],
            vec![1],
            vec![2, 3, 4],
            vec![2, 3, 5],
            vec![2, 4, 5],
            vec![3, 4, 5],
        ],
        n_w_to_m: vec![1, 1, 3, 3, 3, 3],
    }
}

fn write_global_landtype_file(path: &std::path::Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create global landtype file");
    file.add_dimension("longitude", 360).expect("longitude dim");
    file.add_dimension("latitude", 180).expect("latitude dim");
    let values = vec![1_i8; 360 * 180];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn write_global_ocean_landtype_file(path: &std::path::Path) {
    let mut file =
        earthmesh_cli::create_netcdf_quiet(path).expect("create global ocean landtype file");
    file.add_dimension("longitude", 360).expect("longitude dim");
    file.add_dimension("latitude", 180).expect("latitude dim");
    let values = vec![0_i8; 360 * 180];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..))
        .expect("write ocean landtype");
}

#[test]
fn mask_restart_area_judge_runner_restores_patches_and_rewrites_domain_grid() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge");
    let case_dir = root.join("case_restart_patch");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let source = root.join("patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_restart_patch.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_patch'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report = run_mkgrd_mask_restart_area_judge_namelist(
        &namelist,
        &root,
        7,
        MkgrdRestartAreaJudgeOptions {
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
        },
    )
    .expect("run restart Area_judge continuation");

    assert_eq!(report.plan.remask.step, 8);
    assert_eq!(report.workspace_mask.mask_counts.mask_patch_ndm[0], 1);
    assert_eq!(report.area.domain.numpatch, 4);
    assert_eq!(report.area_write.output, restart_input);
    assert_eq!(report.area_write.selected_cells, 4);
    assert!(report.refine_write.is_none());

    let rewritten = read_area_judge_grid_netcdf(case_dir.join("result/IsInDmArea_grid.nc4"))
        .expect("read rewritten grid");
    assert_eq!(rewritten.is_in_area_select, vec![vec![1, 1], vec![1, 1]]);
    assert_eq!(
        rewritten.seaorland_select,
        Some(vec![vec![0, 0], vec![0, 0]])
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_mask_restart_area_judge_continuation_branch() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_binary");
    let case_dir = root.join("case_restart_area_judge_binary");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let source = root.join("patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_restart_area_judge_binary.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart Area_judge path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=ContinueMkgrd"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_area_selected_cells=4"),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_restart_area_judge_binary/tmpfile/mask_patch_bbox_0_01.nc4")
        .exists());
    let rewritten = read_area_judge_grid_netcdf(&restart_input).expect("read rewritten grid");
    assert_eq!(
        rewritten.seaorland_select,
        Some(vec![vec![0, 0], vec![0, 0]])
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_mask_restart_area_judge_with_configured_global_source_dims() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_configured_binary");
    let case_dir = root.join("case_restart_area_judge_configured_binary");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-179.9875, -179.97916666666666],
            latitude: vec![89.9875, 89.97916666666667],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let source = root.join("patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -179.95,
                north: 90.0,
                south: 89.95,
            }],
        },
    )
    .expect("write patch source");

    let namelist = root.join("mkgrd_restart_area_judge_configured_binary.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_configured_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary configured mask_restart Area_judge path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=ContinueMkgrd"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_area_selected_cells=4"),
        "stdout={stdout}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_mask_restart_area_judge_can_generate_land_final_postproc_gridfile() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_postproc_binary");
    let case_dir = root.join("case_restart_area_judge_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir, 16, "tri", "landmesh", false,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");

    let namelist = root.join("mkgrd_restart_area_judge_postproc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart Area_judge final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        io_plan.contain_domain.exists(),
        "missing contain file {}",
        io_plan.contain_domain.display()
    );
    assert!(
        io_plan.result_gridfile.exists(),
        "missing final gridfile {}",
        io_plan.result_gridfile.display()
    );
    assert!(
        io_plan.patchtype_output.clone().unwrap().exists(),
        "missing patchtype file"
    );

    let final_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &io_plan.result_gridfile,
    )
    .expect("read final postproc gridfile");
    assert!(
        !final_mesh.m_points.is_empty(),
        "final postproc gridfile should retain selected land cells"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_mask_restart_area_judge_can_generate_ocean_final_postproc_gridfile() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_ocean_postproc_binary");
    let case_dir = root.join("case_restart_area_judge_ocean_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");
    let source = root.join("ocean_patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write ocean patch source");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir,
        16,
        "tri",
        "oceanmesh",
        true,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_postproc_source_mesh(),
    )
    .expect("write ocean postproc source mesh");

    let namelist = root.join("mkgrd_restart_area_judge_ocean_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_ocean_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart Area_judge ocean final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_obc="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_obcv2="),
        "stdout={stdout}"
    );
    assert!(io_plan.contain_domain.exists());
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.obc_output.clone().unwrap().exists());
    assert!(io_plan.obcv2_output.clone().unwrap().exists());

    let rewritten = read_area_judge_grid_netcdf(&restart_input).expect("read rewritten grid");
    assert_eq!(
        rewritten.seaorland_select,
        Some(vec![vec![0, 0], vec![0, 0]])
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_mask_restart_area_judge_ocean_without_persisted_contain_remains_area_only() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_ocean_missing_postproc_boundary_binary");
    let case_dir = root.join("case_restart_area_judge_ocean_missing_postproc_boundary");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");
    let source = root.join("ocean_patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write ocean patch source");

    let namelist = root.join("mkgrd_restart_area_judge_ocean_missing_postproc_boundary.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_ocean_missing_postproc_boundary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart Area_judge ocean area-only path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_area_selected_cells=4"),
        "stdout={stdout}"
    );
    assert!(!stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        !stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_mask_restart_area_judge_ocean_inferrs_final_postproc_boundary_from_persisted_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_ocean_infer_postproc_binary");
    let case_dir = root.join("case_restart_area_judge_ocean_infer_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");
    let source = root.join("ocean_patch_source.nc4");
    write_bbox_mask_netcdf(
        &source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write ocean patch source");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir,
        16,
        "tri",
        "oceanmesh",
        true,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_postproc_source_mesh(),
    )
    .expect("write ocean postproc source mesh");
    earthmesh_cli::contain_io::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::contain_io::ContainMesh {
            ustr_id: vec![vec![0, 0, 1], vec![1, 0, 1], vec![1, 0, 1], vec![0, 0, 1]],
            ustr_ii: vec![vec![2, 2, 0]],
            is_in_area_ustr: vec![0, 1, 1, -1],
        },
    )
    .expect("write persisted ocean contain boundary");

    let namelist = root.join("mkgrd_restart_area_judge_ocean_infer_postproc.nml");
    let base_dir = format!("{}/", root.display());
    let source_path = source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_ocean_infer_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.true.\n  NL%mask_patch_type='bbox'\n  NL%mask_patch_fprefix='{source_path}'\n  NL%mask_sea_ratio=0.5\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .current_dir(&root)
        .output()
        .expect(
            "run earthmesh_cli binary mask_restart Area_judge ocean inferred final postproc path",
        );

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_obc="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_obcv2="),
        "stdout={stdout}"
    );
    assert!(io_plan.contain_domain.exists());
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.obc_output.clone().unwrap().exists());
    assert!(io_plan.obcv2_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_mask_restart_area_judge_can_generate_earth_final_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_earth_postproc_binary");
    let case_dir = root.join("case_restart_area_judge_earth_postproc");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 0], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir,
        16,
        "tri",
        "earthmesh",
        false,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_postproc_source_mesh(),
    )
    .expect("write earth postproc source mesh");

    let namelist = root.join("mkgrd_restart_area_judge_earth_postproc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_earth_postproc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge")
        .arg("--mask-restart-max-iter")
        .arg("7")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary mask_restart Area_judge earth final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mask_restart_contain="), "stdout={stdout}");
    assert!(
        stdout.contains("mask_restart_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_postproc_earthmesh_info="),
        "stdout={stdout}"
    );
    assert!(io_plan.contain_domain.exists());
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.patchtype_output.clone().unwrap().exists());
    assert!(case_dir.join("result/earthmesh_info.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_mask_restart_area_judge_postproc_runner_generates_land_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_postproc_library");
    let case_dir = root.join("case_restart_area_judge_postproc_library");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir, 16, "tri", "landmesh", false,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");

    let namelist = root.join("mkgrd_restart_area_judge_postproc_library.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_postproc_library'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let report =
        earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_area_judge_postproc_namelist(
            &namelist,
            &root,
            7,
            earthmesh_cli::mkgrd_restart_types::MkgrdRestartAreaJudgePostprocOptions {
                area_judge: MkgrdRestartAreaJudgeOptions {
                    lon_vertex: &lon_vertex,
                    lat_vertex: &lat_vertex,
                    lon_i: &lon_i,
                    lat_i: &lat_i,
                    gridnum_perdegree: 1,
                    nlons_source: 6,
                    nlats_source: 6,
                },
                num_vertex: 1,
            },
        )
        .expect("run library mask_restart Area_judge final postproc path");

    assert_eq!(report.restart.area_write.output, restart_input);
    assert_eq!(report.contain.output, io_plan.contain_domain);
    match report.postproc {
        earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }
    assert!(io_plan.result_gridfile.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_mask_restart_area_judge_configured_global_source_runner_infers_dims() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_configured_global_source_library");
    let case_dir = root.join("case_restart_area_judge_configured_global_source_library");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-179.9875, -179.97916666666666],
            latitude: vec![89.9875, 89.97916666666667],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir, 16, "tri", "landmesh", false,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");

    let namelist = root.join("mkgrd_restart_area_judge_configured_global_source_library.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_configured_global_source_library'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report =
        earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
            &namelist, &root, 7, None,
        )
        .expect("run configured global-source Area_judge postproc runner");

    assert_eq!(report.restart.area_write.output, restart_input);
    assert_eq!(report.restart.area_write.selected_cells, 4);
    assert!(report.postproc.is_none());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_mask_restart_ocean_runner_can_infer_persisted_num_vertex_without_options() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_mask_restart_ocean_infer_postproc_boundary");
    let case_dir = root.join("case_mask_restart_ocean_infer_postproc_boundary");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile dir");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir,
        16,
        "tri",
        "oceanmesh",
        false,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_postproc_source_mesh(),
    )
    .expect("write ocean postproc source mesh");
    earthmesh_cli::contain_io::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::contain_io::ContainMesh {
            ustr_id: vec![
                vec![0, 0, 1],
                vec![0, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![1, 0, 1],
                vec![0, 0, 1],
                vec![0, 0, 1],
            ],
            ustr_ii: vec![vec![0, 0, 0]],
            is_in_area_ustr: vec![0, -1, 1, 1, 1, 1, -1, -1],
        },
    )
    .expect("write persisted ocean contain boundary");

    let namelist = root.join("mkgrd_mask_restart_ocean_infer_postproc_boundary.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_mask_restart_ocean_infer_postproc_boundary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_sea_ratio=0.5\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_ocean_inferred_namelist(
        &namelist, &root, 7,
    )
    .expect("run direct ocean mask_restart postproc with inferred boundary");

    assert_eq!(
        report.postproc.final_gridfile.output,
        io_plan.result_gridfile
    );
    assert!(report
        .postproc
        .obc
        .expect("ocean obc output")
        .output
        .exists());
    assert!(report
        .postproc
        .obcv2
        .expect("ocean obcv2 output")
        .output
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_mask_restart_area_judge_global_source_runner_builds_axes_and_runs_optional_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_global_source_library");
    let case_dir = root.join("case_restart_area_judge_global_source_library");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("patchtype")).expect("create patchtype dir");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 2,
                maxlon_source: 3,
                maxlat_source: 2,
                minlat_source: 3,
            },
            longitude: vec![-178.5, -177.5],
            latitude: vec![88.5, 87.5],
            is_in_area_select: vec![vec![1, 1], vec![1, 1]],
            seaorland_select: Some(vec![vec![1, 1], vec![1, 1]]),
        },
    )
    .expect("write restart domain");

    let io_plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &case_dir, 16, "tri", "landmesh", false,
    )
    .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_land_postproc_source_mesh(),
    )
    .expect("write postproc source mesh");

    let namelist = root.join("mkgrd_restart_area_judge_global_source_library.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_area_judge_global_source_library'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_mask_restart::run_mkgrd_mask_restart_area_judge_global_source_namelist(
        &namelist,
        &root,
        7,
        1,
        6,
        6,
        Some(1),
    )
    .expect("run global-source Area_judge postproc runner");

    assert_eq!(report.restart.area_write.output, restart_input);
    let runtime_counts = report
        .final_domain_contain_runtime_counts()
        .expect("global-source Area_judge report should expose final contain runtime counts");
    assert_eq!(runtime_counts.previous_num_vertex, 1);
    let runtime_state = report.runtime_state();
    assert_eq!(
        runtime_state.num_mp_step[0], 2,
        "direct global-source Area_judge report should expose final Get_Contain(0) cell-count writeback"
    );
    assert_eq!(
        runtime_state.num_wp_step[0], 6,
        "direct global-source Area_judge report should expose final Get_Contain(0) vertex-count writeback"
    );
    let postproc = report.postproc.expect("postproc report");
    assert_eq!(postproc.contain.output, io_plan.contain_domain);
    match postproc.postproc {
        earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_restart_refine_landtype_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_landtype_atmos_full_mpas");
    let case_dir = root.join("case_restart_refine_landtype_atmos_full_mpas");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![vec![1; 4]; 4]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("refine_01.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.5,
                north: 89.5,
                south: 86.5,
            }],
        },
    )
    .expect("write specified refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_atmos_mpas_full_source_mesh())
        .expect("write initial atmos gridfile");
    let landtype_file = sources.join("landtype_atmos_full_mpas.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_restart_refine_landtype_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_landtype_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine-landtype-source")
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary restart-refine landtype atmos full MPAS handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_restart_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0009_hex.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_handoff_area_judge_restart_grid_into_refine_pipeline_refine_from_landtype_file() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_refine_landtype_binary");
    let case_dir = root.join("case_restart_refine_landtype_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![
                vec![0, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 0, 0, 0],
            ]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_land_postproc_source_mesh())
        .expect("write initial refine gridfile");
    let landtype_file = sources.join("landtype.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_restart_refine_landtype_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_landtype_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine-landtype-source")
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary restart Area_judge refine landtype handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_restart_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_landtype_ocean_uses_mode_grid_num_vertex_for_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_landtype_ocean_num_vertex_binary");
    let case_dir = root.join("case_default_restart_refine_landtype_ocean_num_vertex_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![vec![0; 4]; 4]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_ocean_postproc_source_mesh())
        .expect("write initial ocean refine gridfile");
    let landtype_file = sources.join("landtype_ocean.nc");
    write_global_ocean_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_landtype_ocean_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_ocean_num_vertex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_sea_ratio=0.5\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_sea_ratio=.true.\n  RL%th_sea_ratio=0.2,0.5\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine landtype ocean handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_method_c_default_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_builds_standard_restart_refine_initial_gridfile_path() {
    let root = temp_root("mkgrd_restart_refine_initial_gridfile_path_api");
    let namelist = root.join("mkgrd_restart_refine_initial_gridfile_path_api.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_initial_gridfile_path_api'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n/\n"
        ),
    )
    .expect("write namelist");
    let contents = fs::read_to_string(&namelist).expect("read namelist");
    let config =
        earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents).expect("parse config");

    let path = earthmesh_cli::mkgrd_default_restart_handoff::restart_refine_initial_gridfile_path_from_config(&config)
        .expect("infer restart-refine initial gridfile path");

    assert_eq!(
        path,
        root.join(
            "case_restart_refine_initial_gridfile_path_api/gridfile/gridfile_NXP0016_01_tri.nc4"
        )
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_classifies_default_restart_refine_landtype_handoff_with_existing_gridfile() {
    let root = temp_root("mkgrd_default_restart_refine_landtype_handoff_api");
    let case_dir = root.join("case_default_restart_refine_landtype_handoff_api");
    fs::create_dir_all(case_dir.join("gridfile")).expect("create gridfile dir");
    let initial_gridfile = case_dir.join("gridfile/gridfile_NXP0016_01_tri.nc4");
    fs::write(&initial_gridfile, b"existing grid placeholder").expect("write initial gridfile");

    let namelist = root.join("mkgrd_default_restart_refine_landtype_handoff_api.nml");
    let base_dir = format!("{}/", root.display());
    let contents = format!(
        "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_handoff_api'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%landtype_file='landtype.nc'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n/\n"
    );
    fs::write(&namelist, &contents).expect("write namelist");
    let config =
        earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents).expect("parse config");

    let handoff =
        earthmesh_cli::mkgrd_default_restart_handoff::infer_default_restart_refine_handoff_from_config(&config, &contents, None)
            .expect("infer default restart-refine handoff")
            .expect("landtype handoff should be inferred");

    assert_eq!(handoff.initial_gridfile, initial_gridfile);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_builds_restart_area_judge_options_from_global_axes() {
    let axes = earthmesh_cli::global_source_axes::build_global_source_axes_one_based(2, 4, 3)
        .expect("build global axes");

    let options = axes.restart_area_judge_options();
    assert_eq!(options.gridnum_perdegree, 2);
    assert_eq!(options.nlons_source, 4);
    assert_eq!(options.nlats_source, 3);
    assert_eq!(options.lon_vertex[1], -180.0);
    assert_eq!(options.lon_i[1], -179.75);
    assert_eq!(options.lat_i[3], 88.75);
}

#[test]
fn library_builds_one_based_global_source_axes_for_restart_handoffs() {
    let axes = earthmesh_cli::global_source_axes::build_global_source_axes_one_based(2, 4, 3)
        .expect("build reusable source axes");

    assert!(axes.lon_vertex[0].is_nan());
    assert!(axes.lat_vertex[0].is_nan());
    assert!(axes.lon_i[0].is_nan());
    assert!(axes.lat_i[0].is_nan());
    assert_eq!(
        &axes.lon_vertex[1..],
        &[-180.0, -179.5, -179.0, -178.5, -178.0]
    );
    assert_eq!(&axes.lat_vertex[1..], &[90.0, 89.5, 89.0, 88.5]);
    assert_eq!(&axes.lon_i[1..], &[-179.75, -179.25, -178.75, -178.25]);
    assert_eq!(&axes.lat_i[1..], &[89.75, 89.25, 88.75]);
}

#[test]
fn binary_default_restart_refine_landtype_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_landtype_atmos_full_mpas");
    let case_dir = root.join("case_default_restart_refine_landtype_atmos_full_mpas");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![vec![1; 4]; 4]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("refine_01.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.5,
                north: 89.5,
                south: 86.5,
            }],
        },
    )
    .expect("write specified refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_atmos_mpas_full_source_mesh())
        .expect("write initial atmos gridfile");
    let landtype_file = sources.join("landtype_default_atmos_full_mpas.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_landtype_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli default restart-refine landtype atmos full MPAS handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_method_c_default_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0009_hex.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_entry_handoffs_restart_refine_from_landtype_file_when_initial_grid_is_present() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_landtype_binary");
    let case_dir = root.join("case_default_restart_refine_landtype_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![
                vec![0, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 0, 0, 0],
            ]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_land_postproc_source_mesh())
        .expect("write initial refine gridfile");
    let landtype_file = sources.join("landtype.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_landtype_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine landtype handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_method_c_default_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_earth_reports_patchtype_and_info_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_earth_binary");
    let case_dir = root.join("case_default_restart_refine_earth_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![
                vec![0, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 0, 0, 0],
            ]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_land_postproc_source_mesh())
        .expect("write initial refine gridfile");
    let landtype_file = sources.join("landtype.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_earth_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_earth_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine earth handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_method_c_default_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_landtype_earth_hex_reports_patchtype_and_info_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_landtype_earth_hex_binary");
    let case_dir = root.join("case_default_restart_refine_landtype_earth_hex_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![
                vec![0, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 0, 0, 0],
            ]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let initial_gridfile = sources.join("gridfile_initial.nc4");
    write_unstructured_mesh_netcdf(&initial_gridfile, &restart_hex_postproc_source_mesh())
        .expect("write initial refine gridfile");
    let landtype_file = sources.join("landtype.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_landtype_earth_hex_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_earth_hex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--mask-postproc-num-vertex")
        .arg("6")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine landtype earth hex handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_method_c_default_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_hex.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_landtype_uses_existing_case_gridfile_when_initial_grid_arg_is_omitted(
) {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_landtype_existing_gridfile_binary");
    let case_dir = root.join("case_default_restart_refine_landtype_existing_gridfile_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("gridfile")).expect("create gridfile dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![
                vec![0, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 0, 0, 0],
            ]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let existing_gridfile = case_dir.join("gridfile/gridfile_NXP0016_01_tri.nc4");
    write_unstructured_mesh_netcdf(&existing_gridfile, &restart_land_postproc_source_mesh())
        .expect("write existing case gridfile");
    let landtype_file = sources.join("landtype.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_landtype_existing_gridfile_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_existing_gridfile_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect(
            "run earthmesh_cli binary default restart-refine landtype inferred gridfile handoff",
        );

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_method_c_default_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_explicit_restart_refine_landtype_uses_existing_case_gridfile_when_initial_grid_arg_is_omitted(
) {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_explicit_restart_refine_landtype_existing_gridfile_binary");
    let case_dir = root.join("case_explicit_restart_refine_landtype_existing_gridfile_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("gridfile")).expect("create gridfile dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    write_area_judge_grid_netcdf(
        &restart_input,
        &AreaJudgeGridPayload {
            bounds: AreaJudgeSourceBounds {
                minlon_source: 1,
                maxlon_source: 4,
                maxlat_source: 1,
                minlat_source: 4,
            },
            longitude: vec![-179.5, -178.5, -177.5, -176.5],
            latitude: vec![89.5, 88.5, 87.5, 86.5],
            is_in_area_select: vec![vec![1; 4]; 4],
            seaorland_select: Some(vec![
                vec![0, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 1, 0, 0],
                vec![0, 0, 0, 0],
            ]),
        },
    )
    .expect("write restart domain");
    let refine_source = sources.join("cal_refine.nc4");
    write_bbox_mask_netcdf(
        &refine_source,
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -180.0,
                east: -176.0,
                north: 90.0,
                south: 86.0,
            }],
        },
    )
    .expect("write calculated refine source");
    let existing_gridfile = case_dir.join("gridfile/gridfile_NXP0016_01_tri.nc4");
    write_unstructured_mesh_netcdf(&existing_gridfile, &restart_land_postproc_source_mesh())
        .expect("write existing case gridfile");
    let landtype_file = sources.join("landtype.nc");
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_explicit_restart_refine_landtype_existing_gridfile_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_explicit_restart_refine_landtype_existing_gridfile_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect(
            "run earthmesh_cli binary explicit restart-refine landtype inferred gridfile handoff",
        );

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_restart_refine_stdout(&stdout, 1, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}
