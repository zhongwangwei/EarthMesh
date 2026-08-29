use earthmesh_cli::{
    bbox_mask_io::write_bbox_mask_netcdf, bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
    circle_close_mask_io::write_circle_mask_netcdf, circle_close_mask_io::CircleMask,
    coordinate_types::LonLatPoint,
};
use std::{fs, path::Path, path::PathBuf};

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}
#[test]
fn default_dispatcher_runs_atmos_specified_refine_without_landtype_source() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_atmos_specified_refine_passthrough");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let namelist = root.join("mkgrd_default_atmos_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_atmos_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        10_000,
        0,
        None,
        Some(16),
        1,
        None,
    )
    .expect("run default dispatcher");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!(
            "default dispatcher must run Method-C global-source refine for atmos specified refine"
        );
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    let final_grid = root.join("case_default_atmos_refine/result/gridfile_NXP0016_hex.nc4");
    assert!(final_grid.exists());
    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&final_grid)
            .expect("read refined mesh");
    assert!(
        refined_mesh.w_points.len() > run.gridinit.as_ref().unwrap().gridfile.lbx_points,
        "expected refinement to add cells: initial={} final={}",
        run.gridinit.as_ref().unwrap().gridfile.lbx_points,
        refined_mesh.w_points.len()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_dispatcher_uses_synthetic_source_resolution_for_final_global_circle_refine() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_default_atmos_circle_global_spring");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    for degree in 1..=2 {
        write_circle_mask_netcdf(
            sources.join(format!("refine_circle_{degree:03}.nc4")),
            &CircleMask {
                refine_degree: degree,
                points: vec![LonLatPoint {
                    lon: 115.0,
                    lat: 25.0,
                }],
                radius_km: vec![500.0],
            },
        )
        .expect("write circle specified refine source");
    }
    let namelist = root.join("mkgrd_default_atmos_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_default_atmos_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%gridnum_perdegree=120\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,3,3,3,3,3,3\n  RL%max_transition_row=4,4,3,1,1,1,1,1,1\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 200_000, 0, None, None, 1, None,
    )
    .expect("run default dispatcher with synthetic source resolution");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!(
            "default dispatcher must run Method-C global-source refine for atmos specified refine"
        );
    };
    assert_eq!(run.regions.len(), 2);
    assert_eq!(run.max_level, 2);
    assert!(run.transition_faces > 0);
    assert!(root
        .join("case_default_atmos_circle_refine/result/gridfile_NXP0016_hex.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_dispatcher_treats_multipoint_global_circle_source_as_canonical_corridor() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_overlapping_atmos_circle_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    for degree in 1..=3 {
        write_circle_mask_netcdf(
            sources.join(format!("refine_circle_{degree:03}.nc4")),
            &CircleMask {
                refine_degree: degree,
                points: vec![
                    LonLatPoint {
                        lon: 115.0,
                        lat: 25.0,
                    },
                    LonLatPoint {
                        lon: 112.0,
                        lat: 24.0,
                    },
                ],
                radius_km: vec![500.0, 500.0],
            },
        )
        .expect("write overlapping circle specified refine source");
    }
    let namelist = root.join("mkgrd_overlapping_atmos_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_overlapping_atmos_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%gridnum_perdegree=120\n  NL%niter=200\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=3\n  RL%max_iter_cal=0\n  RL%niter_refine=500\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 200_000, 0, None, None, 1, None,
    )
    .expect("Canonical Method-C treats multiple NGR points as one connected corridor grid");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!(
            "default dispatcher must run Method-C global-source refine for atmos specified refine"
        );
    };
    assert_eq!(run.regions.len(), 3);
    assert_eq!(run.max_level, 3);
    assert!(run.transition_faces > 0);
    assert!(root
        .join("case_overlapping_atmos_circle_refine/result/gridfile_NXP0016_hex.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_dispatcher_uses_single_imbeg_for_disjoint_level_two_global_circle_refine_regions() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_disjoint_level_two_atmos_circle_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    for degree in 1..=2 {
        write_circle_mask_netcdf(
            sources.join(format!("refine_circle_{degree:03}.nc4")),
            &CircleMask {
                refine_degree: degree,
                points: vec![
                    LonLatPoint {
                        lon: 115.0,
                        lat: 25.0,
                    },
                    LonLatPoint {
                        lon: 90.0,
                        lat: 25.0,
                    },
                ],
                radius_km: vec![500.0, 500.0],
            },
        )
        .expect("write disjoint level-two circle specified refine source");
    }
    let namelist = root.join("mkgrd_disjoint_level_two_atmos_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_disjoint_level_two_atmos_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%gridnum_perdegree=120\n  NL%niter=200\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=500\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,3,3,0,0,0,0\n  RL%max_transition_row=4,4,3,3,3,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 200_000, 0, None, None, 1, None,
    )
    .expect("Canonical Method-C uses one IMBEG per spawned grid for disjoint circle regions");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!(
            "default dispatcher must run Method-C global-source refine for atmos specified refine"
        );
    };
    assert_eq!(run.regions.len(), 2);
    assert_eq!(run.max_level, 2);
    assert!(run.transition_faces > 0);
    assert!(root
        .join("case_disjoint_level_two_atmos_circle_refine/result/gridfile_NXP0016_hex.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_dispatcher_keeps_single_global_circle_refine_transition_topology_consistent() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_single_atmos_circle_refine_transition");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    for degree in 1..=2 {
        write_circle_mask_netcdf(
            sources.join(format!("refine_circle_{degree:03}.nc4")),
            &CircleMask {
                refine_degree: degree,
                points: vec![LonLatPoint {
                    lon: 115.0,
                    lat: 25.0,
                }],
                radius_km: vec![500.0],
            },
        )
        .expect("write single circle specified refine source");
    }
    let namelist = root.join("mkgrd_single_atmos_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_single_atmos_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%gridnum_perdegree=120\n  NL%niter=200\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=500\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,3,3,0,0,0,0\n  RL%max_transition_row=4,4,3,3,3,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 200_000, 0, None, None, 1, None,
    )
    .expect("run single global circle refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!(
            "default dispatcher must run Method-C global-source refine for atmos specified refine"
        );
    };
    assert_eq!(run.regions.len(), 2);
    assert_eq!(run.max_level, 2);
    assert!(run.transition_faces > 0);
    let final_grid = root.join("case_single_atmos_circle_refine/result/gridfile_NXP0016_hex.nc4");
    assert!(final_grid.exists());
    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&final_grid)
            .expect("read refined mesh");
    let unstructured_topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        unstructured_topology.is_consistent(),
        "unstructured violations: {:?}",
        &unstructured_topology.violations[..unstructured_topology.violations.len().min(8)]
    );
    let cellwidth = vec![100.0; refined_mesh.w_points.len()];
    let mpas = earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based(
        &refined_mesh,
        &cellwidth,
        16,
        1,
    )
    .expect("build MPAS from refined mesh");
    let mpas_topology = earthmesh_cli::mpas_topology::check_mpas_mesh_topology(&mpas);
    assert!(
        mpas_topology.is_consistent(),
        "MPAS violations: {:?}",
        &mpas_topology.violations[..mpas_topology.violations.len().min(8)]
    );
    assert_eq!(mpas_topology.euler_characteristic, 2);
    assert_eq!(mpas_topology.boundary_edges, 0);
    assert!(mpas_topology.is_closed);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_can_run_refine_namelist_through_top_level_passthrough_smoke() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_top_level_refine_passthrough");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let namelist = root.join("mkgrd_binary_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--run-refine-passthrough")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .arg("--source-nlons")
        .arg("6")
        .arg("--source-nlats")
        .arg("6")
        .arg("--source-first-triangle-id")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary refine smoke");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_stack=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("retired_methods"),
        "Method-C direct output must not advertise the compatibility stack: {stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_refine/result/gridfile_NXP0016_hex.nc4")
        .exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn binary_run_refine_source_state_flag_is_removed() {
    let root = temp_root("mkgrd_binary_bad_refine_source_state");
    let namelist = root.join("mkgrd_binary_bad_refine_source_state.nml");
    fs::write(
        &namelist,
        "&mkgrd\n  NL%EXPNME='case_bad_refine_source_state'\n/\n",
    )
    .expect("write namelist");
    let source_state = root.join("bad_source_state.txt");
    fs::write(&source_state, "not a compact source-state\n").expect("write bad source-state");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--run-refine-source-state")
        .arg(&source_state)
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary bad source-state path");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown argument --run-refine-source-state"),
        "{stderr}"
    );

    let _ = fs::remove_dir_all(&root);
}
fn write_global_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon: usize, lat: usize| lon * nlats + lat;
    values[idx(288, 116)] = 2;
    values[idx(289, 114)] = 7;
    values[idx(290, 113)] = 4;
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn write_global_all_land_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create all-land landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let values = vec![2_i8; nlons * nlats];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..))
        .expect("write all-land landtype");
}

fn write_global_ocean_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create ocean landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let values = vec![0_i8; nlons * nlats];
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..))
        .expect("write ocean landtype");
}
fn write_global_sparse_land_landtype_file(path: &Path, nlons: usize, nlats: usize) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create sparse landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon_canonical: usize, lat_canonical: usize| {
        (lon_canonical - 1) * nlats + lat_canonical - 1
    };
    for (lon, lat) in [
        (289, 117),
        (290, 115),
        (290, 116),
        (290, 117),
        (291, 114),
        (291, 115),
        (291, 116),
        (291, 117),
    ] {
        values[idx(lon, lat)] = 1;
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..))
        .expect("write sparse landtype");
}

#[test]
fn binary_can_run_refine_namelist_with_landtype_source_without_source_state_file() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_state");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let landtype_file = root.join("landtype_binary.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype-source path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_landtype_masked_cells="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_source_state/result/gridfile_NXP0016_hex.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_atmos_full_mpas_reports_mesh_and_graph_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_atmos_full_mpas");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let landtype_file = root.join("landtype_binary_atmos_full_mpas.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_atmos_full_mpas.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_atmos_full_mpas'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary full-MPAS landtype-source path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_landtype_atmos_full_mpas/result/gridfile_NXP0016_hex.nc4")
        .exists());
}

#[test]
fn binary_default_entry_runs_landtype_refine_through_refine_pipeline_path() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_default_landtype_source_state");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let landtype_file = root.join("landtype_default_binary.nc");
    write_global_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_default_landtype_source_state.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_default_landtype_source_state'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary default landtype-source path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(root
        .join("case_binary_default_landtype_source_state/result/gridfile_NXP0016_hex.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_can_run_calculated_refine_thresholds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_calculated");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("calref_01.nc4"),
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
    .expect("write calculated refine mask source");
    let threshold_inputs = root.join("threshold_inputs");
    fs::create_dir_all(&threshold_inputs).expect("create threshold inputs");
    {
        let mut file = earthmesh_cli::create_netcdf_quiet(threshold_inputs.join("lai.nc"))
            .expect("create lai input");
        file.add_dimension("lon", 360).expect("lon dim");
        file.add_dimension("lat", 180).expect("lat dim");
        let mut values = vec![1.0_f64; 360 * 180];
        let idx = |lon: usize, lat: usize| lon * 180 + lat;
        values[idx(280, 60)] = 10.0;
        values[idx(281, 60)] = 10.0;
        values[idx(280, 61)] = 10.0;
        values[idx(281, 61)] = 10.0;
        let mut var = file
            .add_variable::<f64>("lai", &["lon", "lat"])
            .expect("lai var");
        var.put_values(&values, (.., ..)).expect("write lai values");
    }
    let landtype_file = root.join("landtype_calculated.nc");
    write_global_all_land_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_calculated.nml");
    let base_dir = format!("{}/", root.display());
    let calref_prefix = sources.join("calref_").display().to_string();
    let threshold_dir = threshold_inputs.display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_calculated'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{calref_prefix}'\n  RL%threshold_dir='{threshold_dir}'\n  RL%refine_lai_m=.true.\n  RL%th_lai_m=5.0\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype calculated path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains("refine_stack=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("retired_methods"),
        "hfield/Method-C path must not advertise the compatibility stack: {stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_landtype_masked_cells="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_source_calculated/result/gridfile_NXP0016_tri.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_runs_ocean_final_domain_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_ocean_final_postproc");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let landtype_file = root.join("landtype_ocean_final.nc");
    write_global_ocean_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_ocean_final.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_ocean_final'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype ocean final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_landtype_masked_cells="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_source_ocean_final/result/gridfile_NXP0016_tri.nc4")
        .exists());
}

#[test]
fn binary_landtype_source_runs_land_final_domain_postproc() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("mkgrd_binary_landtype_source_land_final_postproc");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified refine source");
    let landtype_file = root.join("landtype_land_final.nc");
    write_global_sparse_land_landtype_file(&landtype_file, 360, 180);
    let namelist = root.join("mkgrd_binary_landtype_land_final.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let landtype_text = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_binary_landtype_source_land_final'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{}/missing_mode_file.nc4'\n  NL%mode_file_description='EarthMesh'\n  NL%landtype_file='{landtype_text}'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            root.display()
        ),
    )
    .expect("write namelist");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = std::process::Command::new(exe)
        .arg(&namelist)
        .arg("--max-tris")
        .arg("200000")
        .arg("--run-refine-landtype-source")
        .arg("--source-gridnum-perdegree")
        .arg("1")
        .current_dir(&root)
        .output()
        .expect("run earthmesh_cli binary landtype land final postproc path");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("refine_source=refine_pipeline"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("refine_regions=1"), "stdout={stdout}");
    assert!(stdout.contains("refine_max_level=1"), "stdout={stdout}");
    assert!(
        stdout.contains("refine_landtype_masked_cells="),
        "stdout={stdout}"
    );
    assert!(root
        .join("case_binary_landtype_source_land_final/result/gridfile_NXP0016_hex.nc4")
        .exists());
}
