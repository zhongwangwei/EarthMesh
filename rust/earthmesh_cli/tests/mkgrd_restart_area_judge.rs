use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use earthmesh_cli::{
    read_area_judge_grid_netcdf, run_mkgrd_mask_restart_area_judge_namelist,
    write_area_judge_grid_netcdf, write_bbox_mask_netcdf, write_unstructured_mesh_netcdf,
    AreaJudgeGridPayload, BBoxMask, BBoxPoint, LonLatPoint, MkgrdFinalQualityCheckIoPlan,
    MkgrdRefineLoopExecutor, MkgrdRefineLoopStepIoPlan, MkgrdRefineSourceIoPlan,
    MkgrdRestartAreaJudgeOptions, UnstructuredMesh,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

static NETCDF_TEST_LOCK: Mutex<()> = Mutex::new(());

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
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
    let mut file = netcdf::create(path).expect("create global landtype file");
    file.add_dimension("longitude", 360).expect("longitude dim");
    file.add_dimension("latitude", 180).expect("latitude dim");
    let values = vec![1_i8; 360 * 180];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
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

    let final_mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&io_plan.result_gridfile)
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", true)
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", true)
            .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_postproc_source_mesh(),
    )
    .expect("write ocean postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "earthmesh", false)
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
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

    let report = earthmesh_cli::run_mkgrd_mask_restart_area_judge_postproc_namelist(
        &namelist,
        &root,
        7,
        earthmesh_cli::MkgrdRestartAreaJudgePostprocOptions {
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
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
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
        earthmesh_cli::run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    write_unstructured_mesh_netcdf(
        &io_plan.source_gridfile,
        &restart_ocean_postproc_source_mesh(),
    )
    .expect("write ocean postproc source mesh");
    earthmesh_cli::write_contain_netcdf(
        &io_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
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

    let report = earthmesh_cli::run_mkgrd_mask_restart_ocean_inferred_namelist(&namelist, &root, 7)
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

    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
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

    let report = earthmesh_cli::run_mkgrd_mask_restart_area_judge_global_source_namelist(
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
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Land(postproc) => {
            assert_eq!(postproc.final_gridfile.output, io_plan.result_gridfile);
            assert!(postproc.patchtype.output.exists());
        }
        other => panic!("expected land postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&root);
}

#[derive(Default)]
struct RestartHandoffGeometryExecutor {
    refined_steps: Vec<usize>,
}

impl MkgrdRefineLoopExecutor for RestartHandoffGeometryExecutor {
    fn run_source_branch(
        &mut self,
        _step: &MkgrdRefineLoopStepIoPlan,
        _source: &MkgrdRefineSourceIoPlan,
    ) -> std::io::Result<()> {
        panic!("source branches must be handled by the migrated source executor");
    }

    fn run_refine_loop_step(&mut self, step: &MkgrdRefineLoopStepIoPlan) -> std::io::Result<()> {
        self.refined_steps.push(step.step);
        let mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&step.refine_loop_input_gridfile)?;
        earthmesh_cli::write_unstructured_mesh_netcdf(&step.refine_loop_output_gridfile, &mesh)
            .map(|_| ())
    }

    fn run_final_quality_check(
        &mut self,
        _plan: &MkgrdFinalQualityCheckIoPlan,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn restart_area_judge_refine_handoff_runs_migrated_refine_loop_from_restart_state() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_refine_handoff");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = sources.join("IsInDmArea_restart.nc4");
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

    let namelist = root.join("mkgrd_restart_refine_handoff.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_handoff'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let source_grid = earthmesh_cli::MkgrdRefinePrepareSourceGridOptions {
        lon_vertex: &lon_vertex,
        lat_vertex: &lat_vertex,
        lon_i: &lon_i,
        lat_i: &lat_i,
        gridnum_perdegree: 1,
        nlons_source: 6,
        nlats_source: 6,
        first_triangle_id: 1,
    };
    let landtypes_global = vec![vec![1; lat_i.len()]; lon_i.len()];
    let mut geometry = RestartHandoffGeometryExecutor::default();

    let report = earthmesh_cli::run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor(
        &namelist,
        &root,
        earthmesh_cli::MkgrdAreaJudgeRestartRefineLoopOptions {
            restart_input: &restart_input,
            initial_gridfile: &initial_gridfile,
            source_grid,
            landtypes_global: &landtypes_global,
            num_vertex: 1,
            maxlc: 9,
        },
        &mut geometry,
        None,
    )
    .expect("run restart Area_judge handoff into migrated refine loop");

    assert_eq!(geometry.refined_steps, vec![1]);
    assert!(report.restart.refine_write.is_some());
    assert_eq!(report.execution.executed_sources, 1);
    assert_eq!(report.execution.executed_refine_steps, 1);
    assert!(report.prepare.plan.final_result_gridfile.exists());
    assert!(report.prepare.plan.steps[0].sources[0].threshold_outputs[0].exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_handoff_area_judge_restart_grid_into_migrated_refine_loop() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_area_judge_refine_handoff_binary");
    let case_dir = root.join("case_restart_refine_handoff_binary");
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

    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_restart_refine_handoff_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_handoff_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine")
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary restart Area_judge refine handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_sources=1"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_area_selected_cells=16"),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("result/IsInRfArea_grid_cal_NXP0016_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_restart_refine_source_state_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_source_state_atmos_full_mpas");
    let case_dir = root.join("case_restart_refine_source_state_atmos_full_mpas");
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

    let source_state = sources.join("restart_source_state_atmos.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_restart_refine_source_state_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_source_state_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine")
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary restart-refine source-state atmos full MPAS handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas_graph="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/MPASOUT_NXP0009_global.nc4").exists());
    assert!(case_dir
        .join("result/MPASOUT_NXP0009_global.graph.info")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_restart_refine_source_state_earth_reports_patchtype_and_info_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_source_state_earth_binary");
    let case_dir = root.join("case_restart_refine_source_state_earth_binary");
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

    let source_state = sources.join("restart_source_state_earth.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_restart_refine_source_state_earth_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_source_state_earth_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine")
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary restart-refine source-state earth handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "restart-refine compact earth postproc should report patchtype output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_earthmesh_info="),
        "restart-refine compact earth postproc should report earthmesh_info output; stdout={stdout}"
    );
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());
    assert!(case_dir.join("result/earthmesh_info.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_source_state_earth_reports_patchtype_and_info_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_source_state_earth_binary");
    let case_dir = root.join("case_default_restart_refine_source_state_earth_binary");
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

    let source_state = sources.join("restart_source_state_earth.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_source_state_earth_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_source_state_earth_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine source-state earth handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "default restart-refine compact earth postproc should report patchtype output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_earthmesh_info="),
        "default restart-refine compact earth postproc should report earthmesh_info output; stdout={stdout}"
    );
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());
    assert!(case_dir.join("result/earthmesh_info.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_source_state_earth_hex_reports_patchtype_and_info_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_source_state_earth_hex_binary");
    let case_dir = root.join("case_default_restart_refine_source_state_earth_hex_binary");
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

    let source_state = sources.join("restart_source_state_earth_hex.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=6\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_source_state_earth_hex_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_source_state_earth_hex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--mask-postproc-num-vertex")
        .arg("6")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine source-state earth hex handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "default restart-refine compact earth hex postproc should report patchtype output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_earthmesh_info="),
        "default restart-refine compact earth hex postproc should report earthmesh_info output; stdout={stdout}"
    );
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_hex.nc4")
        .exists());
    assert!(case_dir.join("result/earthmesh_info.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_runner_can_execute_restart_refine_compact_source_state_with_final_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_compact_runner");
    let case_dir = root.join("case_restart_refine_compact_runner");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    let restart_seaorland = vec![
        vec![0, 0, 0, 0],
        vec![0, 1, 0, 0],
        vec![0, 1, 0, 0],
        vec![0, 0, 0, 0],
    ];
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
            seaorland_select: Some(restart_seaorland),
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
    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");
    let namelist = root.join("mkgrd_restart_refine_compact_runner.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_compact_runner'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let run = earthmesh_cli::run_mkgrd_restart_refine_compact_source_state_namelist(
        &namelist,
        &root,
        &source_state,
        &initial_gridfile,
        Some(1),
    )
    .expect("run restart-refine compact source-state runner");

    let domain_write = run
        .report
        .restart
        .domain_write
        .as_ref()
        .expect("domain write");
    assert_eq!(run.source_bundle.source_state.maxlc, 9);
    assert_eq!(domain_write.selected_cells, 16);
    assert_eq!(run.report.execution.executed_refine_steps, 1);
    assert_eq!(run.report.execution.executed_sources, 1);
    assert_eq!(run.source_branch_reports().len(), 1);
    let runtime_state = run.runtime_state();
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_restart_refine_compact_runner"
    );
    assert!(runtime_state.refine.is_some());
    assert!(run
        .report
        .execution
        .final_handoff
        .generated_contain
        .is_some());
    assert!(run.report.execution.final_handoff.postproc.is_some());
    assert!(case_dir
        .join("contain/contain_landmesh_domain_NXP0016_tri.nc4")
        .exists());
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_default_restart_refine_source_state_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_source_state_atmos_full_mpas");
    let case_dir = root.join("case_default_restart_refine_source_state_atmos_full_mpas");
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

    let source_state = sources.join("restart_source_state_atmos_full_mpas.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_source_state_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_source_state_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .arg("--mask-postproc-num-vertex")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli default restart-refine source-state atmos full MPAS handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas_graph="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/MPASOUT_NXP0009_global.nc4").exists());
    assert!(case_dir
        .join("result/MPASOUT_NXP0009_global.graph.info")
        .exists());

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
            "&mkgrd\n  NL%EXPNME='case_restart_refine_landtype_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas_graph="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/MPASOUT_NXP0009_global.nc4").exists());
    assert!(case_dir
        .join("result/MPASOUT_NXP0009_global.graph.info")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_handoff_area_judge_restart_grid_into_migrated_refine_loop_from_landtype_file() {
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
            "&mkgrd\n  NL%EXPNME='case_restart_refine_landtype_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_area_selected_cells=16"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("contain/contain_landmesh_domain_NXP0016_tri.nc4")
        .exists());
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_runner_infers_restart_refine_compact_final_postproc_num_vertex_from_persisted_contain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_compact_infer_postproc_boundary");
    let case_dir = root.join("case_restart_refine_compact_infer_postproc_boundary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
    fs::create_dir_all(&sources).expect("create sources");
    let restart_input = case_dir.join("result/IsInDmArea_grid.nc4");
    let restart_seaorland = vec![
        vec![0, 0, 0, 0],
        vec![0, 1, 0, 0],
        vec![0, 1, 0, 0],
        vec![0, 0, 0, 0],
    ];
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
            seaorland_select: Some(restart_seaorland),
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
    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");
    let namelist = root.join("mkgrd_restart_refine_compact_infer_postproc_boundary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_compact_infer_postproc_boundary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");
    let postproc_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_contain_netcdf(
        &postproc_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![2, 2]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let run = earthmesh_cli::run_mkgrd_restart_refine_compact_source_state_namelist(
        &namelist,
        &root,
        &source_state,
        &initial_gridfile,
        None,
    )
    .expect("run restart-refine compact source-state runner with inferred final boundary");

    let generated = run
        .report
        .execution
        .final_handoff
        .generated_contain
        .as_ref()
        .expect("generated final contain");
    assert_eq!(generated.runtime_counts.previous_num_vertex, 1);
    assert!(run.report.execution.final_handoff.postproc.is_some());
    assert!(postproc_plan.result_gridfile.exists());
    assert!(postproc_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_runner_infers_restart_refine_compact_ocean_final_postproc_num_vertex_from_persisted_contain(
) {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_compact_ocean_infer_postproc_boundary");
    let case_dir = root.join("case_restart_refine_compact_ocean_infer_postproc_boundary");
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
    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "0 0 0 0 0 0 0")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");
    let namelist = root.join("mkgrd_restart_refine_compact_ocean_infer_postproc_boundary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_compact_ocean_infer_postproc_boundary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_sea_ratio=0.5\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_sea_ratio=.true.\n  RL%th_sea_ratio=0.2,0.5\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");
    let postproc_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_contain_netcdf(
        &postproc_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![2, 2]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted ocean contain boundary");

    let run = earthmesh_cli::run_mkgrd_restart_refine_compact_source_state_namelist(
        &namelist,
        &root,
        &source_state,
        &initial_gridfile,
        None,
    )
    .expect("run restart-refine compact ocean source-state runner with inferred final boundary");

    let generated = run
        .report
        .execution
        .final_handoff
        .generated_contain
        .as_ref()
        .expect("generated ocean final contain");
    assert_eq!(generated.runtime_counts.previous_num_vertex, 1);
    match run
        .report
        .execution
        .final_handoff
        .postproc
        .expect("ocean final postproc report")
    {
        earthmesh_cli::MkgrdFinalDomainPostprocReport::Ocean(postproc) => {
            assert_eq!(
                postproc.final_gridfile.output,
                postproc_plan.result_gridfile
            );
            assert!(postproc.obc.expect("ocean obc output").output.exists());
            assert!(postproc.obcv2.expect("ocean obcv2 output").output.exists());
        }
        other => panic!("expected ocean postproc report, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_default_restart_refine_source_state_land_uses_source_state_num_vertex_for_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_source_state_land_num_vertex_binary");
    let case_dir = root.join("case_default_restart_refine_source_state_land_num_vertex_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
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
                vec![0, 1, 1, 0],
                vec![0, 1, 1, 0],
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
        .expect("write initial land refine gridfile");
    let source_state = sources.join("restart_source_state_land.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_source_state_land_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_source_state_land_num_vertex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine source-state land handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "source-state num_vertex should request final land contain; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "source-state num_vertex should drive final land postproc; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "final land postproc should report patchtype output; stdout={stdout}"
    );
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    assert!(io_plan.contain_domain.exists());
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_default_restart_refine_source_state_ocean_uses_source_state_num_vertex_for_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_source_state_ocean_num_vertex_binary");
    let case_dir = root.join("case_default_restart_refine_source_state_ocean_num_vertex_binary");
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
    let source_state = sources.join("restart_source_state_ocean.txt");
    let matrix_rows = (0..7)
        .map(|_| "0 0 0 0 0 0 0")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_source_state_ocean_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_source_state_ocean_num_vertex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%mask_sea_ratio=0.5\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_sea_ratio=.true.\n  RL%th_sea_ratio=0.2,0.5\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine source-state ocean handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "source-state num_vertex should request final ocean contain; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "source-state num_vertex should drive final ocean postproc; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_obc="),
        "final ocean postproc should report OBC output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_obcv2="),
        "final ocean postproc should report OBC v2 output; stdout={stdout}"
    );
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    assert!(io_plan.contain_domain.exists());
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.obc_output.clone().unwrap().exists());
    assert!(io_plan.obcv2_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(root);
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
    write_global_landtype_file(&landtype_file);

    let namelist = root.join("mkgrd_default_restart_refine_landtype_ocean_num_vertex.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_ocean_num_vertex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_sea_ratio=0.5\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_sea_ratio=.true.\n  RL%th_sea_ratio=0.2,0.5\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "mode_grid num_vertex should request final ocean contain; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "mode_grid num_vertex should drive final ocean postproc; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_obc="),
        "final ocean postproc should report OBC output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_obcv2="),
        "final ocean postproc should report OBC v2 output; stdout={stdout}"
    );
    let io_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "oceanmesh", false)
            .expect("postproc io plan");
    assert!(io_plan.contain_domain.exists());
    assert!(io_plan.result_gridfile.exists());
    assert!(io_plan.obc_output.clone().unwrap().exists());
    assert!(io_plan.obcv2_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_runner_can_execute_restart_refine_landtype_source_with_final_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_landtype_runner");
    let case_dir = root.join("case_restart_refine_landtype_runner");
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

    let namelist = root.join("mkgrd_restart_refine_landtype_runner.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_landtype_runner'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let run = earthmesh_cli::run_mkgrd_restart_refine_landtype_source_namelist(
        &namelist,
        &root,
        &initial_gridfile,
        Some(1),
        1,
        Some(1),
    )
    .expect("run restart-refine landtype-source runner");

    assert_eq!(run.preprocess.nlons_source, 360);
    assert_eq!(run.preprocess.nlats_source, 180);
    let domain_write = run
        .report
        .restart
        .domain_write
        .as_ref()
        .expect("domain write");
    assert_eq!(domain_write.selected_cells, 16);
    assert_eq!(run.report.execution.executed_refine_steps, 1);
    assert_eq!(run.report.execution.executed_sources, 1);
    assert_eq!(run.source_branch_reports().len(), 1);
    let runtime_state = run.runtime_state();
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_restart_refine_landtype_runner"
    );
    assert!(runtime_state.refine.is_some());
    assert!(run
        .report
        .execution
        .final_handoff
        .generated_contain
        .is_some());
    assert!(run.report.execution.final_handoff.postproc.is_some());
    assert!(case_dir
        .join("contain/contain_landmesh_domain_NXP0016_tri.nc4")
        .exists());
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_runner_infers_restart_refine_landtype_final_postproc_num_vertex_from_persisted_contain()
{
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_restart_refine_landtype_infer_postproc_boundary");
    let case_dir = root.join("case_restart_refine_landtype_infer_postproc_boundary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
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
    let postproc_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_contain_netcdf(
        &postproc_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![2, 2]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let namelist = root.join("mkgrd_restart_refine_landtype_infer_postproc_boundary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart_refine_landtype_infer_postproc_boundary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let run = earthmesh_cli::run_mkgrd_restart_refine_landtype_source_namelist(
        &namelist,
        &root,
        &initial_gridfile,
        Some(1),
        1,
        None,
    )
    .expect("run restart-refine landtype-source runner with inferred final boundary");

    let generated = run
        .report
        .execution
        .final_handoff
        .generated_contain
        .as_ref()
        .expect("generated final contain");
    assert_eq!(generated.runtime_counts.previous_num_vertex, 1);
    assert!(run.report.execution.final_handoff.postproc.is_some());
    assert!(postproc_plan.result_gridfile.exists());
    assert!(postproc_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_default_entry_handoffs_restart_refine_when_restart_state_inputs_are_present() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_handoff_library");
    let case_dir = root.join("case_default_restart_refine_handoff_library");
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

    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_handoff_library.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_handoff_library'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        100,
        0,
        Some(source_state.as_path()),
        Some(initial_gridfile.as_path()),
        None,
        1,
        None,
    )
    .expect("run default restart-refine handoff through library");

    assert_eq!(report.source_branch_reports().len(), 1);
    let runtime_state = report
        .runtime_state()
        .expect("default restart-refine report should expose runtime state");
    assert_eq!(
        runtime_state.config.experiment_name,
        "case_default_restart_refine_handoff_library"
    );
    assert!(
        runtime_state.refine.is_some(),
        "default restart-refine runtime state should preserve refine config"
    );

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::RestartRefineCompact(run) =
        report
    else {
        panic!("expected compact restart-refine report")
    };
    assert_eq!(run.report.execution.executed_refine_steps, 1);
    assert_eq!(run.report.execution.executed_sources, 1);
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("result/IsInRfArea_grid_cal_NXP0016_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_entry_handoffs_restart_refine_when_restart_state_inputs_are_present() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_handoff_binary");
    let case_dir = root.join("case_default_restart_refine_handoff_binary");
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

    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_handoff_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_handoff_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .arg("--restart-refine-initial-gridfile")
        .arg(&initial_gridfile)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_sources=1"),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("result/IsInRfArea_grid_cal_NXP0016_01.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_source_state_reports_inferred_final_postproc_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_source_state_infer_postproc_binary");
    let case_dir = root.join("case_default_restart_refine_source_state_infer_postproc_binary");
    let sources = root.join("sources");
    fs::create_dir_all(case_dir.join("result")).expect("create result dir");
    fs::create_dir_all(case_dir.join("gridfile")).expect("create gridfile dir");
    fs::create_dir_all(case_dir.join("contain")).expect("create contain dir");
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
    let postproc_plan =
        earthmesh_cli::plan_mask_postproc_domain_io(&case_dir, 16, "tri", "landmesh", false)
            .expect("postproc io plan");
    earthmesh_cli::write_contain_netcdf(
        &postproc_plan.contain_domain,
        &earthmesh_cli::ContainMesh {
            ustr_id: vec![vec![0, 0], vec![1, 1]],
            ustr_ii: vec![vec![2, 2]],
            is_in_area_ustr: vec![0, 1],
        },
    )
    .expect("write persisted contain boundary");

    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_source_state_infer_postproc_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_source_state_infer_postproc_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default source-state restart-refine inferred postproc");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restart_refine_source=source_state"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(postproc_plan.result_gridfile.exists());
    assert!(postproc_plan.patchtype_output.clone().unwrap().exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_uses_existing_case_gridfile_when_initial_grid_arg_is_omitted() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_restart_refine_existing_gridfile_binary");
    let case_dir = root.join("case_default_restart_refine_existing_gridfile_binary");
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
            seaorland_select: Some(vec![vec![1; 4]; 4]),
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

    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_default_restart_refine_existing_gridfile_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_existing_gridfile_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine inferred gridfile handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

    let _ = fs::remove_dir_all(&root);
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

    let path = earthmesh_cli::restart_refine_initial_gridfile_path_from_config(&config)
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
fn binary_explicit_restart_refine_uses_existing_case_gridfile_when_initial_grid_arg_is_omitted() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_explicit_restart_refine_existing_gridfile_binary");
    let case_dir = root.join("case_explicit_restart_refine_existing_gridfile_binary");
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
            seaorland_select: Some(vec![vec![1; 4]; 4]),
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

    let source_state = sources.join("restart_source_state.txt");
    let matrix_rows = (0..7)
        .map(|_| "1 1 1 1 1 1 1")
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &source_state,
        format!(
            "gridnum_perdegree=1\nnlons=6\nnlats=6\nfirst_triangle_id=1\nnum_vertex=1\nmaxlc=9\n[is_in_domain]\n{matrix_rows}\n[seaorland]\n{matrix_rows}\n[landtypes_global]\n{matrix_rows}\n"
        ),
    )
    .expect("write restart source state");

    let namelist = root.join("mkgrd_explicit_restart_refine_existing_gridfile_binary.nml");
    let base_dir = format!("{}/", root.display());
    let refine_path = refine_source.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_explicit_restart_refine_existing_gridfile_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-mask-restart-area-judge-refine")
        .arg("--restart-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine inferred gridfile handoff");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());

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

    let handoff = earthmesh_cli::infer_default_restart_refine_handoff_from_config(
        &config, &contents, false, None,
    )
    .expect("infer default restart-refine handoff")
    .expect("landtype handoff should be inferred");

    assert_eq!(handoff.initial_gridfile, initial_gridfile);
    assert_eq!(
        handoff.source,
        earthmesh_cli::MkgrdDefaultRestartRefineSource::LandtypeFile
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn library_derives_selected_land_domain_from_restart_area_judge_payload() {
    let payload = AreaJudgeGridPayload {
        bounds: AreaJudgeSourceBounds {
            minlon_source: 3,
            maxlon_source: 5,
            maxlat_source: 7,
            minlat_source: 8,
        },
        longitude: vec![10.5, 11.5, 12.5],
        latitude: vec![40.5, 39.5],
        is_in_area_select: vec![vec![1, 1], vec![1, 1], vec![1, 1]],
        seaorland_select: Some(vec![vec![1, 0], vec![0, 1], vec![1, 1]]),
    };

    let selected = earthmesh_cli::selected_land_domain_from_area_judge_grid_payload(&payload)
        .expect("derive selected land-domain matrix");

    assert_eq!(selected.minlon_source, 3);
    assert_eq!(selected.maxlat_source, 7);
    assert_eq!(selected.nlons, 3);
    assert_eq!(selected.nlats, 2);
    assert_eq!(selected.seaorland, vec![vec![1, 0], vec![0, 1], vec![1, 1]]);
}

#[test]
fn library_builds_restart_refine_final_postproc_request() {
    let selected = earthmesh_cli::SelectedLandDomainMatrix {
        minlon_source: 3,
        maxlat_source: 7,
        nlons: 2,
        nlats: 2,
        seaorland: vec![vec![1, 0], vec![0, 1]],
    };

    assert!(
        earthmesh_cli::restart_refine_final_postproc_request("landmesh", None, 0.25, None)
            .expect("no requested final postproc")
            .is_none()
    );

    let land = earthmesh_cli::restart_refine_final_postproc_request(
        "landmesh",
        Some(6),
        0.25,
        Some(&selected),
    )
    .expect("build land restart-refine final postproc request")
    .expect("land final postproc request");
    match land {
        earthmesh_cli::MkgrdRestartRefineFinalPostprocRequest::Land(context) => {
            assert_eq!(context.selected_seaorland, selected.seaorland);
            assert_eq!(context.minlon_dm_area, 3);
            assert_eq!(context.maxlat_dm_area, 7);
            assert_eq!(context.nlons_dm_select, 2);
            assert_eq!(context.nlats_dm_select, 2);
        }
        other => panic!("expected land restart-refine postproc request, got {other:?}"),
    }

    let ocean =
        earthmesh_cli::restart_refine_final_postproc_request("oceanmesh", Some(5), 0.25, None)
            .expect("build ocean restart-refine final postproc request")
            .expect("ocean final postproc request");
    assert_eq!(
        ocean,
        earthmesh_cli::MkgrdRestartRefineFinalPostprocRequest::Ocean {
            mask_sea_ratio: 0.25,
            num_vertex: 5,
        }
    );

    let earth = earthmesh_cli::restart_refine_final_postproc_request(
        "earthmesh",
        Some(4),
        0.4,
        Some(&selected),
    )
    .expect("build earth restart-refine final postproc request")
    .expect("earth final postproc request");
    assert_eq!(
        earth,
        earthmesh_cli::MkgrdRestartRefineFinalPostprocRequest::Earth {
            mask_sea_ratio: 0.4,
            minlon_dm_area: 3,
            maxlat_dm_area: 7,
            nlons_dm_select: 2,
            nlats_dm_select: 2,
        }
    );

    let atmos =
        earthmesh_cli::restart_refine_final_postproc_request("atmosmesh", Some(5), 0.25, None)
            .expect("build atmos restart-refine final postproc request")
            .expect("atmos final postproc request");
    assert_eq!(
        atmos,
        earthmesh_cli::MkgrdRestartRefineFinalPostprocRequest::Atmos
    );
}

#[test]
fn library_builds_restart_refine_final_contain_options() {
    let area_grid_file = std::path::Path::new("restart_final_domain_area.nc4");
    let seaorland = vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 0, 1]];
    let lon_vertex = vec![f64::NAN, -180.0, -179.0, -178.0];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0];
    let lon_i = vec![f64::NAN, -179.5, -178.5];
    let lat_i = vec![f64::NAN, 89.5, 88.5];

    assert!(earthmesh_cli::restart_refine_final_contain_options(
        area_grid_file,
        "landmesh",
        None,
        &seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
    )
    .expect("no requested final contain")
    .is_none());

    let options = earthmesh_cli::restart_refine_final_contain_options(
        area_grid_file,
        "oceanmesh",
        Some(8),
        &seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
    )
    .expect("build restart-refine final contain options")
    .expect("requested final contain");
    assert_eq!(options.area_grid_file, area_grid_file);
    assert_eq!(options.mesh_kind, earthmesh_cli::GetContainMeshKind::Ocean);
    assert_eq!(options.num_vertex, 8);
    assert_eq!(options.seaorland[1][1], 1);
    assert_eq!(options.lon_vertex[2], -179.0);
    assert_eq!(options.lat_i[2], 88.5);

    let loc = earthmesh_cli::restart_refine_final_contain_options(
        area_grid_file,
        "LOCmesh",
        Some(9),
        &seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
    )
    .expect("build LOC restart-refine final contain options")
    .expect("requested LOC final contain");
    assert_eq!(loc.mesh_kind, earthmesh_cli::GetContainMeshKind::Loc);
    assert_eq!(loc.num_vertex, 9);

    let earth = earthmesh_cli::restart_refine_final_contain_options(
        area_grid_file,
        "earthmesh",
        Some(10),
        &seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
    )
    .expect("build earth restart-refine final contain options")
    .expect("requested earth final contain");
    assert_eq!(earth.mesh_kind, earthmesh_cli::GetContainMeshKind::Loc);
    assert_eq!(earth.num_vertex, 10);

    let err = earthmesh_cli::restart_refine_final_contain_options(
        area_grid_file,
        "bogusmesh",
        Some(8),
        &seaorland,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
    )
    .expect_err("unsupported restart-refine final contain mesh should fail");
    assert_eq!(
        err.to_string(),
        "restart-refine final contain does not support mesh_type=bogusmesh"
    );
}

#[test]
fn library_derives_selected_land_domain_from_full_source_seaorland() {
    let full = vec![
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
        vec![0, 1, 0, 1],
        vec![0, 0, 0, 1],
    ];

    let selected =
        earthmesh_cli::selected_land_domain_from_full_source_seaorland_fortran_order(&full, 3, 3)
            .expect("derive selected land-domain matrix from full source seaorland");

    assert_eq!(selected.minlon_source, 2);
    assert_eq!(selected.maxlat_source, 1);
    assert_eq!(selected.nlons, 2);
    assert_eq!(selected.nlats, 3);
    assert_eq!(selected.seaorland, vec![vec![1, 0, 1], vec![0, 0, 1]]);

    let ocean_only =
        earthmesh_cli::selected_land_domain_from_full_source_seaorland_fortran_order(&full, 1, 1)
            .expect("derive fallback selected land-domain matrix");
    assert_eq!(ocean_only.minlon_source, 1);
    assert_eq!(ocean_only.maxlat_source, 1);
    assert_eq!(ocean_only.nlons, 1);
    assert_eq!(ocean_only.nlats, 1);
    assert_eq!(ocean_only.seaorland, vec![vec![0]]);
}

#[test]
fn library_derives_mode_grid_num_vertex_and_seaorland_from_landtypes() {
    assert_eq!(earthmesh_cli::mkgrd_mode_grid_num_vertex("tri").unwrap(), 3);
    assert_eq!(earthmesh_cli::mkgrd_mode_grid_num_vertex("hex").unwrap(), 6);
    let err = earthmesh_cli::mkgrd_mode_grid_num_vertex("quad")
        .expect_err("unsupported mode_grid should fail");
    assert_eq!(
        err.to_string(),
        "unsupported NL%mode_grid for migrated landtype-source execution: quad"
    );

    let landtypes = vec![vec![0, 0, 0], vec![0, 4, 0], vec![0, 0, 9]];
    let seaorland = earthmesh_cli::seaorland_from_landtypes_global_fortran_indexed(&landtypes);
    assert_eq!(seaorland, vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 0, 1]]);
}

#[test]
fn library_builds_compact_source_state_global_axes_for_restart_handoffs() {
    let state = earthmesh_cli::MkgrdCompactSourceState {
        gridnum_perdegree: 2,
        nlons_source: 3,
        nlats_source: 2,
        first_triangle_id: 11,
        num_vertex: 6,
        maxlc: 9,
        final_domain_contain: None,
        final_domain_postproc: None,
        calculated_refine: None,
        calculated_bounds: None,
        is_in_domain: vec![vec![0; 3]; 4],
        seaorland: vec![vec![0; 3]; 4],
        landtypes_global: vec![vec![0; 3]; 4],
    };

    let axes = state
        .build_global_source_axes()
        .expect("build compact source-state global axes");
    assert_eq!(axes.gridnum_perdegree, 2);
    assert_eq!(axes.nlons_source, 3);
    assert_eq!(axes.nlats_source, 2);
    assert_eq!(axes.lon_i[1], -179.75);
    assert_eq!(axes.lat_i[2], 89.25);

    let source_grid = axes.refine_prepare_source_grid(state.first_triangle_id);
    assert_eq!(source_grid.gridnum_perdegree, 2);
    assert_eq!(source_grid.nlons_source, 3);
    assert_eq!(source_grid.nlats_source, 2);
    assert_eq!(source_grid.first_triangle_id, 11);
}

#[test]
fn library_builds_restart_refine_options_from_compact_source_state_file() {
    let root = temp_root("compact_restart_refine_source_state_options");
    let source_state_path = root.join("source_state.txt");
    fs::write(
        &source_state_path,
        "\
gridnum_perdegree=1\n\
nlons=2\n\
nlats=2\n\
first_triangle_id=4\n\
num_vertex=3\n\
maxlc=9\n\
[is_in_domain]\n\
0 0 0\n\
0 1 1\n\
0 1 1\n\
[seaorland]\n\
0 0 0\n\
0 1 0\n\
0 1 1\n\
[landtypes_global]\n\
0 0 0\n\
0 5 6\n\
0 7 8\n",
    )
    .expect("write compact source state");

    let bundle = earthmesh_cli::read_mkgrd_compact_restart_refine_source_state(&source_state_path)
        .expect("read compact restart-refine source state");
    let restart_input = root.join("IsInDmArea_grid.nc4");
    let initial_gridfile = root.join("gridfile_initial.nc4");
    let options = bundle.area_judge_restart_refine_loop_options(&restart_input, &initial_gridfile);

    assert_eq!(bundle.source_state.first_triangle_id, 4);
    assert_eq!(bundle.axes.lon_i[1], -179.5);
    assert_eq!(options.restart_input, restart_input.as_path());
    assert_eq!(options.initial_gridfile, initial_gridfile.as_path());
    assert_eq!(options.source_grid.first_triangle_id, 4);
    assert_eq!(options.landtypes_global[2][2], 8);
    assert_eq!(options.num_vertex, 3);
    assert_eq!(options.maxlc, 9);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_builds_restart_area_judge_options_from_global_axes() {
    let axes = earthmesh_cli::build_global_source_axes_fortran_indexed(2, 4, 3)
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
fn library_builds_fortran_indexed_global_source_axes_for_restart_handoffs() {
    let axes = earthmesh_cli::build_global_source_axes_fortran_indexed(2, 4, 3)
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

    let source_grid = axes.refine_prepare_source_grid(7);
    assert_eq!(source_grid.gridnum_perdegree, 2);
    assert_eq!(source_grid.nlons_source, 4);
    assert_eq!(source_grid.nlats_source, 3);
    assert_eq!(source_grid.first_triangle_id, 7);
}

#[test]
fn library_parses_compact_mkgrd_source_state_text() {
    let contents = "\
gridnum_perdegree=1\n\
nlons=2\n\
nlats=2\n\
first_triangle_id=4\n\
num_vertex=3\n\
maxlc=9\n\
calculated_minlon_source=1\n\
calculated_maxlon_source=2\n\
calculated_maxlat_source=1\n\
calculated_minlat_source=2\n\
final_domain_contain=oceanmesh\n\
final_domain_postproc=landmesh\n\
[calculated_refine]\n\
0 0 0\n\
0 1 0\n\
0 0 0\n\
[is_in_domain]\n\
0 0 0\n\
0 1 1\n\
0 1 1\n\
[seaorland]\n\
0 0 0\n\
0 1 0\n\
0 1 1\n\
[landtypes_global]\n\
0 0 0\n\
0 5 6\n\
0 7 8\n";

    let state = earthmesh_cli::parse_mkgrd_compact_source_state(contents)
        .expect("parse compact source-state text");

    assert_eq!(state.gridnum_perdegree, 1);
    assert_eq!(state.nlons_source, 2);
    assert_eq!(state.nlats_source, 2);
    assert_eq!(state.first_triangle_id, 4);
    assert_eq!(state.num_vertex, 3);
    assert_eq!(state.maxlc, 9);
    assert_eq!(
        state.final_domain_contain,
        Some(earthmesh_cli::GetContainMeshKind::Ocean)
    );
    assert_eq!(
        state.final_domain_postproc,
        Some(earthmesh_cli::MkgrdCompactSourceStateFinalPostproc::Land)
    );
    assert_eq!(
        state.calculated_bounds,
        Some(AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        })
    );
    assert_eq!(state.calculated_refine.as_ref().unwrap()[1][1], 1);
    assert_eq!(state.is_in_domain[2][2], 1);
    assert_eq!(state.seaorland[1][2], 0);
    assert_eq!(state.landtypes_global[2][2], 8);
}

#[test]
fn library_extracts_compact_source_state_matrix_in_fortran_postproc_order() {
    let matrix = vec![vec![0, 0, 0], vec![0, 11, 12], vec![0, 21, 22]];

    let selected = earthmesh_cli::compact_source_state_selected_matrix_fortran_order(&matrix, 2, 2)
        .expect("extract selected matrix");

    assert_eq!(selected, vec![vec![11, 12], vec![21, 22]]);
}

#[test]
fn library_builds_compact_source_state_final_land_postproc_request() {
    let mut state = earthmesh_cli::parse_mkgrd_compact_source_state(
        "\
gridnum_perdegree=1\n\
nlons=2\n\
nlats=2\n\
first_triangle_id=4\n\
num_vertex=3\n\
maxlc=9\n\
final_domain_contain=landmesh\n\
final_domain_postproc=landmesh\n\
[is_in_domain]\n\
0 0 0\n\
0 1 1\n\
0 1 1\n\
[seaorland]\n\
0 0 0\n\
0 11 12\n\
0 21 22\n\
[landtypes_global]\n\
0 0 0\n\
0 5 6\n\
0 7 8\n",
    )
    .expect("parse compact source-state text");

    let request = earthmesh_cli::compact_source_state_final_postproc_request(&state)
        .expect("build final postproc request")
        .expect("land postproc request");

    match request {
        earthmesh_cli::MkgrdCompactSourceStateFinalPostprocRequest::Land(context) => {
            assert_eq!(context.selected_seaorland, vec![vec![11, 12], vec![21, 22]]);
            assert_eq!(context.minlon_dm_area, 1);
            assert_eq!(context.maxlat_dm_area, 1);
            assert_eq!(context.nlons_dm_select, 2);
            assert_eq!(context.nlats_dm_select, 2);
        }
        other => panic!("expected land postproc request, got {other:?}"),
    }

    state.final_domain_contain = None;
    let err = earthmesh_cli::compact_source_state_final_postproc_request(&state)
        .expect_err("land postproc without final contain should fail");
    assert_eq!(
        err.to_string(),
        "source-state final_domain_postproc=land requires final_domain_contain"
    );
}

#[test]
fn library_builds_compact_source_state_final_ocean_postproc_request_with_num_vertex() {
    let mut state = earthmesh_cli::parse_mkgrd_compact_source_state(
        "\
gridnum_perdegree=1\n\
nlons=2\n\
nlats=2\n\
first_triangle_id=4\n\
num_vertex=3\n\
maxlc=9\n\
final_domain_contain=oceanmesh\n\
final_domain_postproc=oceanmesh\n\
[is_in_domain]\n\
0 0 0\n\
0 1 1\n\
0 1 1\n\
[seaorland]\n\
0 0 0\n\
0 11 12\n\
0 21 22\n\
[landtypes_global]\n\
0 0 0\n\
0 5 6\n\
0 7 8\n",
    )
    .expect("parse compact source-state text");

    let request = earthmesh_cli::compact_source_state_final_postproc_request(&state)
        .expect("build final postproc request")
        .expect("ocean postproc request");

    match request {
        earthmesh_cli::MkgrdCompactSourceStateFinalPostprocRequest::Ocean { num_vertex } => {
            assert_eq!(num_vertex, 3);
        }
        other => panic!("expected ocean postproc request with num_vertex, got {other:?}"),
    }

    state.final_domain_contain = None;
    let err = earthmesh_cli::compact_source_state_final_postproc_request(&state)
        .expect_err("ocean postproc without final contain should fail");
    assert_eq!(
        err.to_string(),
        "source-state final_domain_postproc=ocean requires final_domain_contain"
    );
}

#[test]
fn library_builds_compact_source_state_final_earth_postproc_request() {
    let state = earthmesh_cli::parse_mkgrd_compact_source_state(
        "\
gridnum_perdegree=1\n\
nlons=2\n\
nlats=2\n\
first_triangle_id=4\n\
num_vertex=3\n\
maxlc=9\n\
final_domain_contain=earthmesh\n\
final_domain_postproc=earthmesh\n\
[is_in_domain]\n\
0 0 0\n\
0 1 1\n\
0 1 1\n\
[seaorland]\n\
0 0 0\n\
0 11 12\n\
0 21 22\n\
[landtypes_global]\n\
0 0 0\n\
0 5 6\n\
0 7 8\n",
    )
    .expect("parse compact source-state earth postproc text");

    let request = earthmesh_cli::compact_source_state_final_postproc_request(&state)
        .expect("build final earth postproc request")
        .expect("earth postproc request");

    match request {
        earthmesh_cli::MkgrdCompactSourceStateFinalPostprocRequest::Earth(context) => {
            assert_eq!(context.minlon_dm_area, 1);
            assert_eq!(context.maxlat_dm_area, 1);
            assert_eq!(context.nlons_dm_select, 2);
            assert_eq!(context.nlats_dm_select, 2);
        }
        other => panic!("expected earth postproc request, got {other:?}"),
    }
}

#[test]
fn library_builds_compact_source_state_final_contain_payload_and_options() {
    let state = earthmesh_cli::parse_mkgrd_compact_source_state(
        "\
gridnum_perdegree=1\n\
nlons=2\n\
nlats=2\n\
first_triangle_id=4\n\
num_vertex=3\n\
maxlc=9\n\
final_domain_contain=oceanmesh\n\
[is_in_domain]\n\
0 0 0\n\
0 1 0\n\
0 1 1\n\
[seaorland]\n\
0 0 0\n\
0 11 12\n\
0 21 22\n\
[landtypes_global]\n\
0 0 0\n\
0 5 6\n\
0 7 8\n",
    )
    .expect("parse compact source-state text");
    let axes = earthmesh_cli::build_global_source_axes_fortran_indexed(1, 2, 2)
        .expect("build source axes");

    let payload = earthmesh_cli::compact_source_state_final_domain_area_payload_fortran_indexed(
        &state, &axes,
    )
    .expect("build final contain area payload")
    .expect("final contain payload");
    assert_eq!(
        payload.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        }
    );
    assert_eq!(payload.longitude, vec![-179.5, -178.5]);
    assert_eq!(payload.latitude, vec![89.5, 88.5]);
    assert_eq!(payload.is_in_area_select, vec![vec![1, 0], vec![1, 1]]);
    assert_eq!(payload.seaorland_select, None);

    let final_domain_area_grid = std::path::Path::new("final_domain_area_grid.nc4");
    let options = earthmesh_cli::compact_source_state_final_contain_options(
        &state,
        &axes,
        final_domain_area_grid,
    )
    .expect("final contain options");
    assert_eq!(options.mesh_kind, earthmesh_cli::GetContainMeshKind::Ocean);
    assert_eq!(options.area_grid_file, final_domain_area_grid);
    assert_eq!(options.seaorland[2][2], 22);
    assert_eq!(options.lon_i[1], -179.5);
    assert_eq!(options.lat_i[2], 88.5);
    assert_eq!(options.num_vertex, 3);
}

#[test]
fn library_builds_data_preprocess_source_state_final_contain_payload_and_options() {
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex: vec![f64::NAN, -180.0, -179.0, -178.0],
        lat_vertex: vec![f64::NAN, 90.0, 89.0, 88.0],
        lon_i: vec![f64::NAN, -179.5, -178.5],
        lat_i: vec![f64::NAN, 89.5, 88.5],
        gridnum_perdegree: 1,
        nlons_source: 2,
        nlats_source: 2,
        first_triangle_id: 4,
        num_vertex: 6,
        sources: Vec::new(),
        is_in_domain: vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 1, 1]],
        seaorland: vec![vec![0, 0, 0], vec![0, 11, 12], vec![0, 21, 22]],
        landtypes_global: vec![vec![0, 0, 0], vec![0, 5, 6], vec![0, 7, 8]],
        maxlc: 9,
    };

    let payload =
        earthmesh_cli::data_preprocess_source_state_final_domain_area_payload_fortran_indexed(
            &state,
        )
        .expect("build final contain area payload");
    assert_eq!(
        payload.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        }
    );
    assert_eq!(payload.longitude, vec![-179.5, -178.5]);
    assert_eq!(payload.latitude, vec![89.5, 88.5]);
    assert_eq!(payload.is_in_area_select, vec![vec![1, 0], vec![1, 1]]);
    assert_eq!(payload.seaorland_select, None);

    let final_domain_area_grid = std::path::Path::new("final_domain_area_grid_from_landtype.nc4");
    let options = earthmesh_cli::data_preprocess_source_state_final_contain_options(
        &state,
        "landmesh",
        final_domain_area_grid,
    )
    .expect("build final contain options")
    .expect("final contain options");
    assert_eq!(options.mesh_kind, earthmesh_cli::GetContainMeshKind::Land);
    assert_eq!(options.area_grid_file, final_domain_area_grid);
    assert_eq!(options.seaorland[2][2], 22);
    assert_eq!(options.lon_i[1], -179.5);
    assert_eq!(options.lat_i[2], 88.5);
    assert_eq!(options.num_vertex, 6);

    let earth_options = earthmesh_cli::data_preprocess_source_state_final_contain_options(
        &state,
        "earthmesh",
        final_domain_area_grid,
    )
    .expect("build earth final contain options")
    .expect("earth final contain options");
    assert_eq!(
        earth_options.mesh_kind,
        earthmesh_cli::GetContainMeshKind::Loc
    );
    assert_eq!(earth_options.area_grid_file, final_domain_area_grid);
    assert_eq!(earth_options.num_vertex, 6);
}

#[test]
fn library_writes_data_preprocess_source_state_final_contain_payload_and_options() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("data_preprocess_final_contain_write");
    let final_domain_area_grid = root.join("final_domain_area_grid_from_landtype.nc4");
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex: vec![f64::NAN, -180.0, -179.0, -178.0],
        lat_vertex: vec![f64::NAN, 90.0, 89.0, 88.0],
        lon_i: vec![f64::NAN, -179.5, -178.5],
        lat_i: vec![f64::NAN, 89.5, 88.5],
        gridnum_perdegree: 1,
        nlons_source: 2,
        nlats_source: 2,
        first_triangle_id: 4,
        num_vertex: 6,
        sources: Vec::new(),
        is_in_domain: vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 1, 1]],
        seaorland: vec![vec![0, 0, 0], vec![0, 11, 12], vec![0, 21, 22]],
        landtypes_global: vec![vec![0, 0, 0], vec![0, 5, 6], vec![0, 7, 8]],
        maxlc: 9,
    };

    let options = earthmesh_cli::write_data_preprocess_source_state_final_domain_contain_options(
        &state,
        "landmesh",
        &final_domain_area_grid,
    )
    .expect("write final contain payload and options")
    .expect("final contain options");

    assert_eq!(options.mesh_kind, earthmesh_cli::GetContainMeshKind::Land);
    assert_eq!(options.area_grid_file, final_domain_area_grid.as_path());
    assert_eq!(options.seaorland[2][2], 22);
    assert_eq!(options.lon_i[1], -179.5);
    assert_eq!(options.lat_i[2], 88.5);
    assert_eq!(options.num_vertex, 6);

    let payload =
        read_area_judge_grid_netcdf(&final_domain_area_grid).expect("read written area grid");
    assert_eq!(
        payload.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        }
    );
    assert_eq!(payload.longitude, vec![-179.5, -178.5]);
    assert_eq!(payload.latitude, vec![89.5, 88.5]);
    assert_eq!(payload.is_in_area_select, vec![vec![1, 0], vec![1, 1]]);
    assert_eq!(payload.seaorland_select, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_maps_data_preprocess_final_postproc_request_to_runner_options() {
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex: vec![f64::NAN, -180.0, -179.0, -178.0],
        lat_vertex: vec![f64::NAN, 90.0, 89.0, 88.0],
        lon_i: vec![f64::NAN, -179.5, -178.5],
        lat_i: vec![f64::NAN, 89.5, 88.5],
        gridnum_perdegree: 1,
        nlons_source: 2,
        nlats_source: 2,
        first_triangle_id: 4,
        num_vertex: 6,
        sources: Vec::new(),
        is_in_domain: vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 1, 1]],
        seaorland: vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 0, 1]],
        landtypes_global: vec![vec![0, 0, 0], vec![0, 5, 0], vec![0, 0, 8]],
        maxlc: 9,
    };

    let request =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "landmesh")
            .expect("build land request");
    let options = earthmesh_cli::data_preprocess_source_state_final_postproc_options(
        request.as_ref(),
        &state,
        0.0,
        "CoLM",
    )
    .expect("land postproc options");
    match options {
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::Land(options)) => {
            assert_eq!(options.seaorland, &[vec![1, 0], vec![0, 1]]);
            assert_eq!(options.minlon_dm_area, 1);
            assert_eq!(options.maxlat_dm_area, 1);
            assert_eq!(options.nlons_dm_select, 2);
            assert_eq!(options.nlats_dm_select, 2);
            assert_eq!(options.lon_i[2], -178.5);
        }
        _ => panic!("expected land postproc options"),
    }

    let ocean_request =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "oceanmesh")
            .expect("build ocean request");
    let ocean_options = earthmesh_cli::data_preprocess_source_state_final_postproc_options(
        ocean_request.as_ref(),
        &state,
        0.25,
        "CoLM",
    )
    .expect("ocean postproc options");
    match ocean_options {
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::Ocean(options)) => {
            assert_eq!(options.mask_sea_ratio, 0.25);
            assert_eq!(options.num_vertex, 6);
        }
        _ => panic!("expected ocean postproc options"),
    }

    let earth_request =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "earthmesh")
            .expect("build earth final postproc request");
    let earth_options = earthmesh_cli::data_preprocess_source_state_final_postproc_options(
        earth_request.as_ref(),
        &state,
        0.4,
        "CoLM",
    )
    .expect("earth postproc options");
    match earth_options {
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid(options)) => {
            assert_eq!(options.mask_sea_ratio, 0.4);
            assert_eq!(options.minlon_dm_area, 1);
            assert_eq!(options.maxlat_dm_area, 1);
            assert_eq!(options.nlons_dm_select, 2);
            assert_eq!(options.nlats_dm_select, 2);
            assert_eq!(options.lat_i[2], 88.5);
        }
        _ => panic!("expected earth postproc options"),
    }

    let atmos_request =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "atmosmesh")
            .expect("build atmos final postproc request");
    let atmos_options = earthmesh_cli::data_preprocess_source_state_final_postproc_options(
        atmos_request.as_ref(),
        &state,
        0.0,
        "MPAS-Simple",
    )
    .expect("atmos postproc options");
    match atmos_options {
        Some(earthmesh_cli::MkgrdFinalDomainPostprocOptions::Atmos { output_format }) => {
            assert_eq!(output_format, "MPAS-Simple");
        }
        _ => panic!("expected atmos postproc options"),
    }

    let none = earthmesh_cli::data_preprocess_source_state_final_postproc_options(
        None, &state, 0.0, "CoLM",
    )
    .expect("none postproc options");
    assert!(none.is_none());
}

#[test]
fn library_builds_data_preprocess_source_state_final_postproc_request() {
    let state = earthmesh_cli::MkgrdDataPreprocessSourceState {
        lon_vertex: vec![f64::NAN, -180.0, -179.0, -178.0],
        lat_vertex: vec![f64::NAN, 90.0, 89.0, 88.0],
        lon_i: vec![f64::NAN, -179.5, -178.5],
        lat_i: vec![f64::NAN, 89.5, 88.5],
        gridnum_perdegree: 1,
        nlons_source: 2,
        nlats_source: 2,
        first_triangle_id: 4,
        num_vertex: 6,
        sources: Vec::new(),
        is_in_domain: vec![vec![0, 0, 0], vec![0, 1, 0], vec![0, 1, 1]],
        seaorland: vec![vec![0, 0, 0], vec![0, 0, 1], vec![0, 0, 1]],
        landtypes_global: vec![vec![0, 0, 0], vec![0, 5, 6], vec![0, 7, 8]],
        maxlc: 9,
    };

    let land =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "landmesh")
            .expect("build land final postproc request")
            .expect("land postproc request");
    match land {
        earthmesh_cli::MkgrdDataPreprocessSourceStateFinalPostprocRequest::Land(context) => {
            assert_eq!(context.selected_seaorland, vec![vec![1], vec![1]]);
            assert_eq!(context.minlon_dm_area, 1);
            assert_eq!(context.maxlat_dm_area, 2);
            assert_eq!(context.nlons_dm_select, 2);
            assert_eq!(context.nlats_dm_select, 1);
        }
        other => panic!("expected land postproc request, got {other:?}"),
    }

    let ocean =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "oceanmesh")
            .expect("build ocean final postproc request")
            .expect("ocean postproc request");
    assert_eq!(
        ocean,
        earthmesh_cli::MkgrdDataPreprocessSourceStateFinalPostprocRequest::Ocean { num_vertex: 6 }
    );

    let earth =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "earthmesh")
            .expect("build earth final postproc request")
            .expect("earth postproc request");
    assert_eq!(
        earth,
        earthmesh_cli::MkgrdDataPreprocessSourceStateFinalPostprocRequest::Earth(
            earthmesh_cli::MkgrdDataPreprocessSourceStateEarthPostprocContext {
                minlon_dm_area: 1,
                maxlat_dm_area: 2,
                nlons_dm_select: 2,
                nlats_dm_select: 1,
            }
        )
    );

    let atmos =
        earthmesh_cli::data_preprocess_source_state_final_postproc_request(&state, "atmosmesh")
            .expect("build atmos final postproc request")
            .expect("atmos postproc request");
    assert_eq!(
        atmos,
        earthmesh_cli::MkgrdDataPreprocessSourceStateFinalPostprocRequest::Atmos
    );
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
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_mpas_graph="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/MPASOUT_NXP0009_global.nc4").exists());
    assert!(case_dir
        .join("result/MPASOUT_NXP0009_global.graph.info")
        .exists());

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
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_final_contain="),
        "landtype land default num_vertex should drive final Get_Contain without a manual postprocess boundary; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "restart-refine land postproc should report the patchtype artifact for CoLM handoff; stdout={stdout}"
    );
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());

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
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_earth_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "restart-refine earth postproc should report patchtype output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_earthmesh_info="),
        "restart-refine earth postproc should report earthmesh_info output; stdout={stdout}"
    );
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());
    assert!(case_dir.join("result/earthmesh_info.nc4").exists());

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
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_earth_hex_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%mask_sea_ratio=0.4\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_patchtype="),
        "restart-refine landtype earth hex postproc should report patchtype output; stdout={stdout}"
    );
    assert!(
        stdout.contains("restart_refine_final_postproc_earthmesh_info="),
        "restart-refine landtype earth hex postproc should report earthmesh_info output; stdout={stdout}"
    );
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_hex.nc4")
        .exists());
    assert!(case_dir.join("result/earthmesh_info.nc4").exists());

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
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_landtype_existing_gridfile_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());

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
            "&mkgrd\n  NL%EXPNME='case_explicit_restart_refine_landtype_existing_gridfile_binary'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_path}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=1\n/\n"
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
    assert!(
        stdout.contains("restart_refine_source=landtype_file"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("mask_restart_action=RefineHandoff"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("restart_refine_steps=1"), "stdout={stdout}");
    assert!(
        stdout.contains("restart_refine_final_postproc_gridfile="),
        "stdout={stdout}"
    );
    assert!(case_dir.join("result/gridfile_NXP0016_tri.nc4").exists());
    assert!(case_dir
        .join("patchtype/patchtype_NXP0016_tri.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_default_restart_refine_requires_source_state_or_landtype_file() {
    let root = temp_root("mkgrd_default_restart_refine_missing_source");
    let namelist = root.join("mkgrd_default_restart_refine_missing_source.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_restart_refine_missing_source'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%gridnum_perdegree=120\n  NL%refine=.true.\n  NL%mask_restart=.true.\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n/\n"
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--restart-refine-initial-gridfile")
        .arg(root.join("gridfile_initial.nc4"))
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default restart-refine missing source");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "default restart-refine handoff requires --restart-refine-source-state or NL%landtype_file"
        ),
        "stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&root);
}
