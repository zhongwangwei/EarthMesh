use earthmesh_cli::{
    bbox_mask_io::write_bbox_mask_netcdf, bbox_mask_io::BBoxMask, bbox_mask_io::BBoxPoint,
    circle_close_mask_io::write_circle_mask_netcdf, circle_close_mask_io::write_close_mask_netcdf,
    circle_close_mask_io::CircleMask, circle_close_mask_io::CloseMask,
    coordinate_types::LonLatPoint,
};
use earthmesh_refine_method_c::MethodCMesh;
use std::{fs, path::PathBuf};

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug)]
struct MethodCAtmosLandLikeCaseResult {
    base_lbx_points: usize,
    added_lbx_points: usize,
    transition_faces: usize,
    spring_nest_iterations: usize,
    max_level: usize,
    spring_nest_passes: usize,
    output_lbx_points: usize,
}

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

fn write_threshold_matrix(
    path: impl AsRef<std::path::Path>,
    var: &str,
    nlon: usize,
    nlat: usize,
    values: &[f64],
) {
    assert_eq!(values.len(), nlon * nlat);
    let mut file =
        earthmesh_cli::create_netcdf_quiet(path.as_ref()).expect("create threshold netcdf");
    file.add_dimension("lon", nlon).expect("lon dim");
    file.add_dimension("lat", nlat).expect("lat dim");
    file.add_variable::<f64>(var, &["lon", "lat"])
        .expect("threshold variable")
        .put_values(values, (.., ..))
        .expect("threshold values");
}

fn run_method_c_atmos_landlike_case(
    root: &PathBuf,
    mesh_type: &str,
    output_format: &str,
    case_name: &str,
) -> MethodCAtmosLandLikeCaseResult {
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

    let namelist = root.join(format!("mkgrd_method_c_{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C specified refine");
    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default Method-C specified refine should use Method-C direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type.trim(), mesh_type);
    assert_eq!(run.runtime_state.config.mode_grid.trim(), "hex");
    assert_eq!(run.runtime_state.config.output_format.trim(), output_format);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(
        run.output.output.exists(),
        "Method-C specified refine output file should exist: {:?}",
        run.output.output
    );
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);

    MethodCAtmosLandLikeCaseResult {
        base_lbx_points: run.gridinit.gridfile.lbx_points,
        added_lbx_points: run.output.lbx_points - run.gridinit.gridfile.lbx_points,
        transition_faces: run.transition_faces,
        spring_nest_iterations: run.spring_nest_iterations,
        max_level: run.max_level,
        spring_nest_passes: run.spring_nest_passes,
        output_lbx_points: run.output.lbx_points,
    }
}

#[test]
fn method_c_hfield_direct_refine_can_use_threshold_source_without_region_masks() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_hfield_threshold_no_regions");
    let threshold_dir = root.join("threshold");
    fs::create_dir_all(&threshold_dir).expect("create threshold dir");
    let src_nlon = 72;
    let src_nlat = 18;
    let hfield_nlon = 36;
    let hfield_nlat = 18;
    let mut values = vec![0.0; src_nlon * src_nlat];
    for i in 0..src_nlon {
        let lon = -180.0 + (i as f64 + 0.5) * 360.0 / src_nlon as f64;
        for j in 0..src_nlat {
            let lat = -90.0 + (j as f64 + 0.5) * 180.0 / src_nlat as f64;
            if (80.0..=150.0).contains(&lon) && (0.0..=50.0).contains(&lat) {
                values[i * src_nlat + j] = if i % 2 == 0 { 10.0 } else { 0.0 };
            }
        }
    }
    write_threshold_matrix(
        threshold_dir.join("lai.nc"),
        "lai",
        src_nlon,
        src_nlat,
        &values,
    );

    let namelist = root.join("mkgrd_method_c_hfield_threshold.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_hfield_threshold'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%threshold_dir='{}'\n  RL%refine_lai_s=.true.\n  RL%th_lai_s=2.0\n/\n&hfield\n  NL%hfield_on=.true.\n  NL%hfield_g=0.2\n  NL%hfield_max_level=1\n  NL%hfield_base_m=100000.0\n  NL%hfield_nlon={hfield_nlon}\n  NL%hfield_nlat={hfield_nlat}\n/\n",
            threshold_dir.display()
        ),
    )
    .expect("write hfield threshold namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 20_000, None)
        .expect("threshold hfield should drive direct Method-C refinement");

    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "threshold hfield should refine the initial mesh"
    );
}

#[test]
fn method_c_hfield_rejects_a_raw_namelist_off_the_stride_three_lattice() {
    let root = temp_root("method_c_hfield_nxp_stride_guard");
    let namelist = root.join("off_stride.nml");
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='off_stride'\n  NL%base_dir='{}/'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&hfield\n  NL%hfield_on=.true.\n/\n",
            root.display()
        ),
    )
    .expect("write off-stride HField namelist");

    let error = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 20_000, None)
        .expect_err("raw HField NXP outside the stride-3 lattice must fail before gridinit");
    assert!(
        error
            .to_string()
            .contains("requires NXP divisible by 3; got 8 (use 9"),
        "unexpected error: {error}"
    );
}

fn write_landtype_file_by_predicate(path: &std::path::Path, is_land: impl Fn(f64, f64) -> bool) {
    let (nlons, nlats) = (360, 180);
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut values = vec![0_i8; nlons * nlats];
    let idx = |lon: usize, lat: usize| lon * nlats + lat;
    for lon_idx in 0..nlons {
        let lon = -180.0 + (lon_idx as f64 + 0.5);
        for lat_idx in 0..nlats {
            let lat = 90.0 - (lat_idx as f64 + 0.5);
            if is_land(lon, lat) {
                values[idx(lon_idx, lat_idx)] = 1;
            }
        }
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

fn write_sparse_landtype_file_for_gridnum(path: &std::path::Path, gridnum_perdegree: usize) {
    let (nlons, nlats) = (360 * gridnum_perdegree, 180 * gridnum_perdegree);
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create sparse landtype file");
    file.add_dimension("longitude", nlons)
        .expect("longitude dim");
    file.add_dimension("latitude", nlats).expect("latitude dim");
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    let strip = vec![1_i8; nlats];
    let lon_zero = 180 * gridnum_perdegree;
    var.put_values(&strip, (lon_zero..lon_zero + 1, ..))
        .expect("write sparse landtype strip");
}

fn non_placeholder_points(points: &[LonLatPoint]) -> Vec<LonLatPoint> {
    points
        .iter()
        .copied()
        .filter(|point| point.lon != 0.0 || point.lat != 0.0)
        .collect()
}

#[test]
fn method_c_atmos_and_surface_constants_match_canonical_defaults() {
    assert_eq!(MethodCMesh::MAX_MROWS_ATMOS, 13);
    assert_eq!(MethodCMesh::MAX_MROWS_SURFACE, 7);
    // (the exact-value assertions above already pin ATMOS > SURFACE)
}

#[test]
fn default_atmos_and_landlike_meshes_use_different_transition_widths() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_transition_width_comparison");

    let atmos = run_method_c_atmos_landlike_case(
        &root,
        "atmosmesh",
        "MPAS",
        "case_method_c_atmos_transition_cmp",
    );
    let land = run_method_c_atmos_landlike_case(
        &root,
        "landmesh",
        "CoLM",
        "case_method_c_land_transition_cmp",
    );

    assert_eq!(atmos.max_level, land.max_level);
    assert_eq!(atmos.spring_nest_passes, land.spring_nest_passes);
    assert_eq!(atmos.spring_nest_iterations, land.spring_nest_iterations);
    assert_eq!(atmos.base_lbx_points, land.base_lbx_points);
    assert!(atmos.transition_faces > 0);
    assert!(land.transition_faces > 0);
    assert!(atmos.output_lbx_points > atmos.base_lbx_points);
    assert!(land.output_lbx_points > land.base_lbx_points);
    assert_eq!(
        atmos.output_lbx_points - atmos.base_lbx_points,
        atmos.added_lbx_points
    );
    assert_eq!(
        land.output_lbx_points - land.base_lbx_points,
        land.added_lbx_points
    );
    assert!(
        atmos.transition_faces > land.transition_faces,
        "Expected atmosmesh to produce more transition faces than landmesh. atmos={}, land={}",
        atmos.transition_faces,
        land.transition_faces
    );
}

#[test]
fn default_atmos_global_specified_refine_uses_method_c_spawn_nest() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_global_refine");
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

    let namelist = root.join("mkgrd_method_c_atmos_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_atmos_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C atmos specified refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default atmos specified refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(run.transition_faces > 0);
    assert_eq!(
        run.output.output,
        root.join("case_method_c_atmos_circle_refine/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "Method-C specified refine should add Voronoi cells: initial={} final={}",
        run.gridinit.gridfile.lbx_points,
        run.output.lbx_points
    );

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C direct output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_atmos_specified_refine_ignores_zero_degree_masks() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_specified_zero_degree");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 0,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_500.0],
        },
    )
    .expect("write zero-degree circle specified refine source");

    let namelist = root.join("mkgrd_method_c_atmos_zero_degree_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_atmos_zero_degree_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("specified degree-zero masks should not create Method-C regions");

    assert!(
        err.to_string()
            .contains("Method-C direct refine found no region sources"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_ngrids_uses_method_c_region() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_ngrids");
    let namelist = root.join("mkgrd_native_grid_ngrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_ngrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native Method-C namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native Method-C ngrids refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native Method-C ngrids refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_atmos_native_method_c_spawn_nest_springs_even_when_mkrefine_global_spring_disabled() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_spring_ignores_mkrefine_flag");
    let namelist = root.join("mkgrd_native_grid_spring_ignores_mkrefine_flag.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_spring_ignores_mkrefine_flag'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=1\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native Method-C spring namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native Method-C spring refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native Method-C spring refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert!(run.spring_nest_passes > 0);
}

#[test]
fn default_atmos_native_method_c_makegrid_plot_uses_canonical_plot_spring_iterations() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_makegrid_plot_spring");
    let namelist = root.join("mkgrd_native_grid_makegrid_plot_spring.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_makegrid_plot_spring'\n  NL%runtype='MAKEGRID_PLOT'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=5000\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native Method-C MAKEGRID_PLOT namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native Method-C MAKEGRID_PLOT refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native Method-C MAKEGRID_PLOT refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 100);
    assert!(run.spring_nest_passes > 0);
}

#[test]
fn default_atmos_native_method_c_ngrids_three_spawns_each_canonical_grid_once() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_ngrids_three_once");
    let namelist = root.join("mkgrd_native_grid_ngrids_three_once.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_ngrids_three_once'\n  NL%base_dir='{base_dir}'\n  NL%NXP=12\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=3\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%ngrdll(3)=1\n  NL%grdrad(3,1)=500000.0\n  NL%grdlat(3,1)=-25.0\n  NL%grdlon(3,1)=-65.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=1\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native atmosphere three-grid namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native atmosphere three-grid refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native atmosphere ngrids should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 2);
    assert_eq!(
        run.regions
            .iter()
            .map(earthmesh_mesh::RefinementRegion::level)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(run.spring_nest_iterations, 5000);
    assert_eq!(
        run.spring_nest_passes, 2,
        "Canonical spawn_nest loops nn=2..NGRIDS and spawns each native grid once"
    );
}

#[test]
fn default_atmos_native_method_c_ngrids_above_six_uses_canonical_maxgrds_not_refine_level_limit() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_ngrids_canonical_maxgrds");
    let namelist = root.join("mkgrd_native_grid_ngrids_canonical_maxgrds.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_ngrids_canonical_maxgrds'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=7\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%ngrdll(3)=1\n  NL%grdrad(3,1)=2400000.0\n  NL%grdlat(3,1)=20.0\n  NL%grdlon(3,1)=110.0\n  NL%ngrdll(4)=1\n  NL%grdrad(4,1)=2300000.0\n  NL%grdlat(4,1)=15.0\n  NL%grdlon(4,1)=105.0\n  NL%ngrdll(5)=1\n  NL%grdrad(5,1)=2200000.0\n  NL%grdlat(5,1)=10.0\n  NL%grdlon(5,1)=100.0\n  NL%ngrdll(6)=1\n  NL%grdrad(6,1)=2100000.0\n  NL%grdlat(6,1)=5.0\n  NL%grdlon(6,1)=95.0\n  NL%ngrdll(7)=1\n  NL%grdrad(7,1)=2000000.0\n  NL%grdlat(7,1)=0.0\n  NL%grdlon(7,1)=90.0\n/\n",
        ),
    )
    .expect("write native atmosphere Canonical maxgrds namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, 1, None,
    )
    .expect_err("max_tris=0 should stop after native ngrids passes Canonical maxgrds validation");

    assert!(
        !err.to_string().contains("max_iter_spc/max_iter_cal"),
        "native Method-C ngrids must not use specified/calculated refine level limit: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_ngrids_does_not_require_refine_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_ngrids_no_refine");
    let namelist = root.join("mkgrd_native_grid_ngrids_no_refine.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_ngrids_no_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere no-refine namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native atmosphere ngrids without refine flag");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native atmosphere ngrids should use Method-C direct path without refine flag");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_atmos_native_method_c_rejects_out_of_range_latitude_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_bad_latitude");
    let namelist = root.join("mkgrd_native_grid_bad_latitude.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_bad_latitude'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=95.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native latitude namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C latitude outside Canonical bounds should fail");

    assert!(
        err.to_string().contains("grdlat"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_radius_larger_than_double_earth_radius_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_large_radius");
    let namelist = root.join("mkgrd_native_grid_large_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_large_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=13000000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write large native radius namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C radius above Canonical erad2 bound should fail");

    assert!(
        err.to_string().contains("grdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_radius_below_canonical_dzxmin() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_small_radius");
    let namelist = root.join("mkgrd_native_grid_small_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_small_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=0.0005\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write small native radius namelist");

    let result = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native Method-C radius below Canonical dzxmin should fail"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("grdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_zero_ngrids_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_zero_ngrids");
    let namelist = root.join("mkgrd_native_grid_zero_ngrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_zero_ngrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=0\n/\n",
        ),
    )
    .expect("write zero native ngrids namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C ngrids below Canonical bound should fail");

    assert!(
        err.to_string().contains("ngrids"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_ngrids_above_canonical_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_many_ngrids");
    let namelist = root.join("mkgrd_native_grid_many_ngrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_many_ngrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=21\n/\n",
        ),
    )
    .expect("write too many native grids namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C ngrids above Canonical maxgrds should fail");

    assert!(
        err.to_string().contains("ngrids"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_gridplot_base_below_canonical_lower_bound() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_bad_gridplot_base");
    let namelist = root.join("mkgrd_native_grid_bad_gridplot_base.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_bad_gridplot_base'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%gridplot_base=1\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native gridplot_base namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C gridplot_base below Canonical lower bound should fail");

    assert!(
        err.to_string().contains("gridplot_base"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_mdomain_above_canonical_upper_bound() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_bad_mdomain");
    let namelist = root.join("mkgrd_native_grid_bad_mdomain.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_bad_mdomain'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=6\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native mdomain namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C mdomain above Canonical upper bound should fail");

    assert!(
        err.to_string().contains("mdomain"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_ngrdll_above_canonical_maxngrdll() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_many_ngrdll");
    let namelist = root.join("mkgrd_native_grid_many_ngrdll.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_many_ngrdll'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=21\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write too many native grid points namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C ngrdll above Canonical maxngrdll should fail");

    assert!(
        err.to_string().contains("ngrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_ngrdll_index_above_canonical_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_ngrdll_bad_index");
    let namelist = root.join("mkgrd_native_grid_ngrdll_bad_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_ngrdll_bad_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(21)=1\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native grid point count with bad index namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C ngrdll index above Canonical maxgrds should fail");

    assert!(
        err.to_string().contains("ngrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_zero_ngrdll_index_like_canonical_array() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_ngrdll_zero_index");
    let namelist = root.join("mkgrd_native_grid_ngrdll_zero_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_ngrdll_zero_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(0)=1\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native grid point count with zero index namelist");

    let result = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native Method-C ngrdll index 0 should fail like a Canonical array"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("ngrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_coordinate_point_index_above_canonical_maxngrdll() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_grdrad_bad_point_index");
    let namelist = root.join("mkgrd_native_grid_grdrad_bad_point_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_grdrad_bad_point_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,21)=2500000.0\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native coordinate with bad point index namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C coordinate point index above Canonical maxngrdll should fail");

    assert!(
        err.to_string().contains("grdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_rejects_global_nxp_not_divisible_by_three_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_bad_nxp");
    let namelist = root.join("mkgrd_native_grid_bad_nxp.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_bad_nxp'\n  NL%base_dir='{base_dir}'\n  NL%NXP=7\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native NXP namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C global NXP not divisible by 3 should fail");

    assert!(err.to_string().contains("NXP"), "unexpected error: {err}");
}

#[test]
fn default_atmos_native_method_c_allows_non_global_domain_nxp_not_divisible_by_three_like_canonical(
) {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_regional_nxp5");
    let namelist = root.join("mkgrd_native_grid_regional_nxp5.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_regional_nxp5'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere regional NXP namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, 1, None,
    )
    .expect_err("max_tris=0 should stop after regional NXP validation passes");

    assert!(
        !err.to_string().contains("NXP must be divisible by 3"),
        "Canonical only applies this NXP check to mdomain < 2 global runs: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_allows_non_global_domain_cartesian_coordinates_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_regional_cartesian");
    let namelist = root.join("mkgrd_native_grid_regional_cartesian.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_regional_cartesian'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=95.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere regional Cartesian namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, 1, None,
    )
    .expect_err("max_tris=0 should stop after regional Cartesian coordinate validation passes");

    assert!(
        !err.to_string().contains("grdlat"),
        "Canonical only applies GRDLAT bounds for mdomain < 2 global runs: {err}"
    );
}

#[test]
fn default_atmos_native_method_c_mdomain_five_overrides_compatibility_global_flag_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_mdomain_five");
    let namelist = root.join("mkgrd_native_grid_mdomain_five.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_mdomain_five'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%deltax=1000000.0\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=5\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=500000.0\n  NL%grdlat(2,1)=-310000.0\n  NL%grdlon(2,1)=10200000.0\n/\n",
        ),
    )
    .expect("write native atmosphere mdomain=5 namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("mdomain=5 should use regional/cartesian native Method-C semantics");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native Method-C mdomain=5 should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert_eq!(run.spring_nest_passes, 1);
    let grid = &run.runtime_state.grid;
    assert!(grid.nma > 0);
    assert!(grid.nwa > 0);
    for im in 1..=grid.nma {
        assert_eq!(
            grid.glonm[im], grid.xem[im],
            "mdomain=5 should keep Canonical cartesian-x M output placeholders"
        );
        assert_eq!(
            grid.glatm[im], grid.yem[im],
            "mdomain=5 should keep Canonical cartesian-y M output placeholders"
        );
    }
    for iw in 1..=grid.nwa {
        assert_eq!(
            grid.glonw[iw], grid.xew[iw],
            "mdomain=5 should keep Canonical cartesian-x W output placeholders"
        );
        assert_eq!(
            grid.glatw[iw], grid.yew[iw],
            "mdomain=5 should keep Canonical cartesian-y W output placeholders"
        );
    }
}

#[test]
fn cartesian_native_method_c_runs_explicit_hfield_in_xy_meters() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_cartesian_explicit_hfield");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create Cartesian hfield sources");
    write_bbox_mask_netcdf(
        sources.join("cartesian_bbox_001.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -2_000_000.0,
                east: 2_000_000.0,
                south: -1_000_000.0,
                north: 1_000_000.0,
            }],
        },
    )
    .expect("write Cartesian bbox hfield source");
    let namelist = root.join("mkgrd_method_c_cartesian_explicit_hfield.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("cartesian_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_cartesian_explicit_hfield'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%deltax=1000000.0\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=5\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=0.0\n  NL%grdlon(2,1)=0.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%num_rc=0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n&hfield\n  NL%hfield_on=.true.\n/\n",
        ),
    )
    .expect("write native atmosphere mdomain=5 hfield namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 100_000, None)
        .expect("explicit hfield should drive Cartesian-XY Method-C refinement");

    assert_eq!(run.max_level, 1);
    assert!(run
        .regions
        .iter()
        .any(|region| matches!(region, earthmesh_mesh::RefinementRegion::Bbox { .. })));
    let cartesian_base = earthmesh_refine_method_c::MethodCMesh::from_cart_hex(18, 1_000_000.0)
        .expect("build Cartesian base mesh");
    assert!(
        run.runtime_state.grid.nma > cartesian_base.nmd,
        "Cartesian-XY hfield should add native M points: base={} final={}",
        cartesian_base.nmd,
        run.runtime_state.grid.nma,
    );
    assert!(run.transition_faces > 0);
}

#[test]
fn cartesian_native_method_c_samples_geographic_threshold_hfield_from_origin() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_cartesian_geographic_threshold_hfield");
    let threshold_dir = root.join("threshold");
    fs::create_dir_all(&threshold_dir).expect("create threshold dir");
    let (nlon, nlat) = (36, 18);
    let mut values = vec![0.0; nlon * nlat];
    for i in 0..nlon {
        let lon = -180.0 + (i as f64 + 0.5) * 360.0 / nlon as f64;
        for j in 0..nlat {
            let lat = -90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64;
            if (100.0..=140.0).contains(&lon) && (10.0..=50.0).contains(&lat) {
                values[i * nlat + j] = 10.0;
            }
        }
    }
    write_threshold_matrix(
        threshold_dir.join("typhoon.nc"),
        "typhoon",
        nlon,
        nlat,
        &values,
    );
    let namelist = root.join("mkgrd_cartesian_geographic_threshold.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='cartesian_geo_threshold'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%deltax=1000000.0\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=5\n  NL%ngrids=1\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%threshold_dir='{}'\n  RL%refine_typhoon_m=.true.\n  RL%th_typhoon_m=2.0\n/\n&hfield\n  NL%hfield_on=.true.\n  NL%hfield_max_level=1\n  NL%hfield_base_m=1000000.0\n  NL%hfield_nlon={nlon}\n  NL%hfield_nlat={nlat}\n  NL%hfield_origin_lon=120.0\n  NL%hfield_origin_lat=30.0\n/\n",
            threshold_dir.display()
        ),
    )
    .expect("write Cartesian geographic threshold namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 100_000, None)
        .expect("geographic threshold hfield should refine Cartesian-XY mesh");

    let cartesian_base = earthmesh_refine_method_c::MethodCMesh::from_cart_hex(18, 1_000_000.0)
        .expect("build Cartesian base mesh");
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert!(run.runtime_state.grid.nma > cartesian_base.nmd);
}

#[test]
fn default_atmos_native_method_c_mdomain_two_does_not_spawn_ngrids_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_native_mdomain_two_no_spawn");
    let namelist = root.join("mkgrd_native_grid_mdomain_two_no_spawn.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_native_grid_mdomain_two_no_spawn'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=2\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=95.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere mdomain=2 namelist");

    let err = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 20_000, None)
        .expect_err("Canonical does not call atmosphere spawn_nest for mdomain=2");

    assert!(
        err.to_string()
            .contains("Method-C specified refine requires NL%refine=.true."),
        "mdomain=2 ngrids must not be treated as native Method-C refinement: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_out_of_range_latitude_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_bad_latitude");
    let namelist = root.join("mkgrd_method_c_surface_bad_latitude.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_bad_latitude'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=95.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native surface latitude namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C surface latitude outside Canonical bounds should fail");

    assert!(
        err.to_string().contains("sfcgrdlat"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_radius_larger_than_double_earth_radius_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_large_radius");
    let namelist = root.join("mkgrd_method_c_surface_large_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_large_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=13000000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write large native surface radius namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C surface radius above Canonical erad2 bound should fail");

    assert!(
        err.to_string().contains("sfcgrdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_radius_below_canonical_dzxmin() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_small_radius");
    let namelist = root.join("mkgrd_method_c_surface_small_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_small_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=0.0005\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write small native surface radius namelist");

    let result = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native Method-C surface radius below Canonical dzxmin should fail"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("sfcgrdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_regional_nsfcgrids_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_regional_nsfc");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create regional surface source dir");
    write_bbox_mask_netcdf(
        sources.join("domain_bbox_000.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: 100.0,
                east: 130.0,
                north: 40.0,
                south: 10.0,
            }],
        },
    )
    .expect("write bbox domain source");
    let namelist = root.join("mkgrd_method_c_surface_regional_nsfc.nml");
    let base_dir = format!("{}/", root.display());
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_regional_nsfc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_prefix}'\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native regional surface namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C surface nsfcgrids should require a global domain like Canonical");

    assert!(
        err.to_string().contains("surface") && err.to_string().contains("global domain"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_regional_inherited_atmos_ngrids_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_regional_atmos");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create regional surface source dir");
    write_bbox_mask_netcdf(
        sources.join("domain_bbox_000.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: 100.0,
                east: 130.0,
                north: 40.0,
                south: 10.0,
            }],
        },
    )
    .expect("write bbox domain source");
    let namelist = root.join("mkgrd_method_c_surface_regional_atmos.nml");
    let base_dir = format!("{}/", root.display());
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_regional_atmos'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_prefix}'\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write native regional inherited-atmos surface namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None)
    .expect_err("native Method-C surface inherited atmosphere grids should require a global domain like Canonical");

    assert!(
        err.to_string().contains("surface") && err.to_string().contains("global domain"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_sfcgridplot_base_below_canonical_lower_bound() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_bad_sfcgridplot_base");
    let namelist = root.join("mkgrd_method_c_surface_bad_sfcgridplot_base.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_bad_sfcgridplot_base'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgridplot_base=0\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native surface gridplot base namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C sfcgridplot_base below Canonical lower bound should fail");

    assert!(
        err.to_string().contains("sfcgridplot_base"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_zero_coordinate_point_index_like_canonical_array() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_sfcgrdrad_zero_point_index");
    let namelist = root.join("mkgrd_method_c_surface_sfcgrdrad_zero_point_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_sfcgrdrad_zero_point_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,0)=500000.0\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native surface coordinate with zero point index namelist");

    let result = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native Method-C coordinate point index 0 should fail like a Canonical array"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("sfcgrdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_nsfcgrdll_index_above_canonical_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_nsfcgrdll_bad_index");
    let namelist = root.join("mkgrd_method_c_surface_nsfcgrdll_bad_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_nsfcgrdll_bad_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(21)=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native surface grid point count with bad index namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C nsfcgrdll index above Canonical maxgrds should fail");

    assert!(
        err.to_string().contains("nsfcgrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_nsfcgrids_above_canonical_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_many_nsfcgrids");
    let namelist = root.join("mkgrd_method_c_surface_many_nsfcgrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_many_nsfcgrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=21\n/\n",
        ),
    )
    .expect("write too many native surface grids namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C nsfcgrids above Canonical maxgrds should fail");

    assert!(
        err.to_string().contains("nsfcgrids"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_nsfcgrdll_above_canonical_maxngrdll() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_many_nsfcgrdll");
    let namelist = root.join("mkgrd_method_c_surface_many_nsfcgrdll.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_many_nsfcgrdll'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=21\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write too many native surface grid points namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C nsfcgrdll above Canonical maxngrdll should fail");

    assert!(
        err.to_string().contains("nsfcgrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_inherits_atmos_ngrids_before_nsfcgrids() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_atmos_then_sfc");
    let namelist = root.join("mkgrd_method_c_surface_native_atmos_then_sfc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_native_atmos_then_sfc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native Method-C surface namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native Method-C surface refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native Method-C surface refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 2);
    assert!(run.regions.iter().all(|region| region.level() == 1));
    assert_eq!(run.max_level, 2);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_method_c_uses_atmos_and_surface_spring_defaults_by_stage() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_stage_spring_defaults");
    let namelist = root.join("mkgrd_method_c_surface_native_stage_spring_defaults.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_native_stage_spring_defaults'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native staged spring namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native staged spring refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native staged spring refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 2);
    assert!(run.regions.iter().all(|region| region.level() == 1));
    assert_eq!(run.spring_nest_iterations, 5000);
    assert_eq!(run.spring_nest_passes, 2);
}

#[test]
fn default_surface_native_method_c_ngrids_without_nsfcgrids_matches_atmos_spawn() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_atmos_only");
    let atmos_namelist = root.join("mkgrd_method_c_atmos_native_only.nml");
    let land_namelist = root.join("mkgrd_method_c_land_native_atmos_only.nml");
    let atmos_base_dir = format!("{}/atmos/", root.display());
    let land_base_dir = format!("{}/land/", root.display());
    fs::write(
        &atmos_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_atmos_native_only'\n  NL%base_dir='{atmos_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native atmosphere-only namelist");
    fs::write(
        &land_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_land_native_atmos_only'\n  NL%base_dir='{land_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native land atmosphere-only namelist");

    let atmos_report =
        earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &atmos_namelist,
            &root,
            20_000,
            0,
            None,
            None,
            1,
            None,
        )
        .expect("run native atmosphere-only atmosphere refine");
    let land_report =
        earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &land_namelist,
            &root,
            20_000,
            0,
            None,
            None,
            1,
            None,
        )
        .expect("run native atmosphere-only surface refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        atmos_run,
    ) = atmos_report
    else {
        panic!("native atmosphere-only atmosmesh should use Method-C direct path");
    };
    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        land_run,
    ) = land_report
    else {
        panic!("native atmosphere-only landmesh should use Method-C direct path");
    };

    assert_eq!(land_run.regions.len(), 1);
    assert_eq!(land_run.max_level, 1);
    assert_eq!(land_run.transition_faces, atmos_run.transition_faces);
    assert_eq!(land_run.spring_nest_passes, atmos_run.spring_nest_passes);
    assert_eq!(
        land_run.spring_nest_iterations,
        atmos_run.spring_nest_iterations
    );
    assert_eq!(land_run.output.lbx_points, atmos_run.output.lbx_points);
    assert_eq!(land_run.output.sjx_points, atmos_run.output.sjx_points);
}

#[test]
fn default_surface_native_method_c_sfcgrid_res_factor_expands_global_surface() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_sfcgrid_res_factor");
    let namelist = root.join("mkgrd_method_c_surface_sfcgrid_res_factor.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_sfcgrid_res_factor'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native surface expansion namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native surface global expansion");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native surface global expansion should use Method-C direct path");
    };
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_iterations, 0);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_method_c_sfcgrid_res_factor_does_not_require_refine_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_sfcgrid_res_no_refine");
    let namelist = root.join("mkgrd_method_c_surface_sfcgrid_res_no_refine.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_sfcgrid_res_no_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native surface expansion no-refine namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native surface global expansion without refine flag");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("surface global expansion should use Method-C surface path without refine flag");
    };
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 0);
    assert_eq!(run.spring_nest_iterations, 0);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_method_c_allows_sfcgrid_res_factor_four_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_sfcgrid_res_four");
    let namelist = root.join("mkgrd_method_c_surface_sfcgrid_res_four.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_sfcgrid_res_four'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=4\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write native surface expansion factor-four namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("native Method-C surface expansion factor four should follow Canonical prime-factor rule");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("surface global expansion factor four should use Method-C surface path");
    };
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 0);
    assert_eq!(run.spring_nest_iterations, 0);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_method_c_rejects_sfcgrid_res_factor_with_prime_factor_other_than_two_or_three_like_canonical(
) {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_bad_sfcgrid_res");
    let namelist = root.join("mkgrd_method_c_surface_bad_sfcgrid_res.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_bad_sfcgrid_res'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=5\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write bad native surface expansion factor namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C surface expansion factor with other prime factors should fail");

    assert!(
        err.to_string().contains("sfcgrid_res_factor"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_rejects_zero_sfcgrid_res_factor_like_canonical() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_zero_sfcgrid_res");
    let namelist = root.join("mkgrd_method_c_surface_zero_sfcgrid_res.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_zero_sfcgrid_res'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=0\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write zero sfcgrid_res_factor namelist");

    let err = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect_err("native Method-C sfcgrid_res_factor=0 should fail like Canonical");

    assert!(
        err.to_string().contains("sfcgrid_res_factor") && err.to_string().contains("positive"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_method_c_nsfcgrids_does_not_require_refine_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_nsfc_no_refine");
    let namelist = root.join("mkgrd_method_c_surface_nsfc_no_refine.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_nsfc_no_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=2500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native surface no-refine namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run native surface nsfcgrids without refine flag");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native surface nsfcgrids should use Method-C direct path without refine flag");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_iterations, 2000);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_method_c_makegrid_plot_uses_canonical_plot_spring_iterations() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_makegrid_plot_spring");
    let namelist = root.join("mkgrd_method_c_surface_makegrid_plot_spring.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_makegrid_plot_spring'\n  NL%runtype='MAKEGRID_PLOT'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=2500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=2000\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native surface MAKEGRID_PLOT namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 200_000, 0, None, None, 1, None,
    )
    .expect("run native surface MAKEGRID_PLOT refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("native surface MAKEGRID_PLOT refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 100);
    assert!(run.spring_nest_passes > 0);
}

#[test]
fn default_surface_native_method_c_expands_surface_before_nsfcgrids_spawn() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_surface_native_expand_then_nsfc");
    let expansion_only = root.join("mkgrd_method_c_surface_expand_only.nml");
    let expansion_then_nest = root.join("mkgrd_method_c_surface_expand_then_nsfc.nml");
    let expansion_base_dir = format!("{}/expand_only/", root.display());
    let nested_base_dir = format!("{}/expand_then_nsfc/", root.display());
    fs::write(
        &expansion_only,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_expand_only'\n  NL%base_dir='{expansion_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write expansion-only surface namelist");
    fs::write(
        &expansion_then_nest,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_surface_expand_then_nsfc'\n  NL%base_dir='{nested_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=1000000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write expansion plus surface nest namelist");

    let expansion_report =
        earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &expansion_only,
            &root,
            20_000,
            0,
            None,
            None,
            1,
            None,
        )
        .expect("run expansion-only surface path");
    let nested_report =
        earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &expansion_then_nest,
            &root,
            20_000,
            0,
            None,
            None,
            1,
            None,
        )
        .expect("run expansion then nsfcgrids surface path");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        expansion_run,
    ) = expansion_report
    else {
        panic!("surface expansion-only should use Method-C direct path");
    };
    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        nested_run,
    ) = nested_report
    else {
        panic!("surface expansion plus nsfcgrids should use Method-C direct path");
    };

    assert!(expansion_run.regions.is_empty());
    assert_eq!(nested_run.regions.len(), 1);
    assert_eq!(nested_run.max_level, 1);
    assert_eq!(nested_run.spring_nest_iterations, 2000);
    assert!(nested_run.output.lbx_points > expansion_run.output.lbx_points);
}

#[test]
fn default_land_global_specified_refine_with_landtype_masks_method_c_output() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_land_global_refine_landtype");
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
    let landtype_file = sources.join("landtype_east.nc");
    write_landtype_file_by_predicate(&landtype_file, |lon, _lat| lon >= 0.0);

    let namelist = root.join("mkgrd_method_c_land_circle_refine_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_land_circle_refine_landtype'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        20_000,
        0,
        None,
        Some(1),
        1,
        None,
    )
    .expect("run default Method-C land specified refine with landtype");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default land specified refine with real landtype should use Method-C direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "landmesh");
    assert!(run.raw_output.is_some());
    assert!(run.landtype_masked_cells.unwrap_or_default() > 0);
    assert_eq!(
        run.output.output,
        root.join("case_method_c_land_circle_refine_landtype/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C land landtype-masked topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
    let final_centers = non_placeholder_points(&refined_mesh.w_points);
    let sampled =
        earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based(
            &landtype_file,
            1,
            &final_centers,
        )
        .expect("sample final land centers");
    assert!(
        sampled.iter().all(|&value| value != 0),
        "landmesh output should contain only land-classified cells"
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_ocean_global_specified_refine_with_landtype_masks_method_c_output() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_ocean_global_refine_landtype");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 1,
            points: vec![LonLatPoint {
                lon: -70.0,
                lat: -20.0,
            }],
            radius_km: vec![2_500.0],
        },
    )
    .expect("write circle specified refine source");
    let landtype_file = sources.join("landtype_east.nc");
    write_landtype_file_by_predicate(&landtype_file, |lon, _lat| lon >= 0.0);

    let namelist = root.join("mkgrd_method_c_ocean_circle_refine_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_ocean_circle_refine_landtype'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        20_000,
        0,
        None,
        Some(1),
        1,
        None,
    )
    .expect("run default Method-C ocean specified refine with landtype");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default ocean specified refine with real landtype should use Method-C direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "oceanmesh");
    assert!(run.raw_output.is_some());
    assert!(run.landtype_masked_cells.unwrap_or_default() > 0);
    assert_eq!(
        run.output.output,
        root.join("case_method_c_ocean_circle_refine_landtype/result/gridfile_NXP0006_tri.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C ocean landtype-masked topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
    let final_centers = non_placeholder_points(&refined_mesh.m_points);
    let sampled =
        earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based(
            &landtype_file,
            1,
            &final_centers,
        )
        .expect("sample final ocean centers");
    assert!(
        sampled.iter().all(|&value| value == 0),
        "oceanmesh output should contain only ocean-classified cells"
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_loc_global_specified_refine_with_landtype_writes_coupled_outputs() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_loc_global_refine_landtype");
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
    let landtype_file = sources.join("landtype_east.nc");
    write_landtype_file_by_predicate(&landtype_file, |lon, _lat| lon >= 0.0);

    let namelist = root.join("mkgrd_method_c_loc_circle_refine_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_loc_circle_refine_landtype'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='LOCmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        20_000,
        0,
        None,
        Some(1),
        1,
        None,
    )
    .expect("run default Method-C LOC specified refine with landtype");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default LOC specified refine with real landtype should use Method-C direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "LOCmesh");
    assert!(run.raw_output.is_some());
    assert_eq!(
        run.output.output,
        root.join("case_method_c_loc_circle_refine_landtype/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());

    let coupled = run
        .coupled_outputs
        .as_ref()
        .expect("LOCmesh Method-C path should write coupled land/ocean/CoLM outputs");
    assert!(coupled.land_output.output.exists());
    assert!(coupled.ocean_output.output.exists());
    assert!(coupled.coupling_csv.exists());
    assert!(coupled.coupling_netcdf.output.exists());
    assert!(coupled.coupling_quality.exists());
    assert!(coupled.manifest.exists());
    assert!(fs::read_to_string(&coupled.manifest)
        .expect("read coupled manifest")
        .contains("coupling_quality_json"));
    assert!(coupled.counts.land > 0);
    assert!(coupled.counts.ocean > 0);
    assert_eq!(
        coupled.coupling_netcdf.rows,
        coupled.counts.land + coupled.counts.ocean
    );

    let land_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &coupled.land_output.output,
    )
    .expect("read coupled land mesh");
    let ocean_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &coupled.ocean_output.output,
    )
    .expect("read coupled ocean mesh");
    let land_topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&land_mesh);
    let ocean_topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&ocean_mesh);
    assert!(
        land_topology.is_consistent(),
        "LOC land subset topology violations: {:?}",
        &land_topology.violations[..land_topology.violations.len().min(8)]
    );
    assert!(
        ocean_topology.is_consistent(),
        "LOC ocean subset topology violations: {:?}",
        &ocean_topology.violations[..ocean_topology.violations.len().min(8)]
    );

    let land_centers = non_placeholder_points(&land_mesh.w_points);
    let ocean_centers = non_placeholder_points(&ocean_mesh.w_points);
    assert!(!land_centers.is_empty());
    assert!(!ocean_centers.is_empty());
    let sampled_land =
        earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based(
            &landtype_file,
            1,
            &land_centers,
        )
        .expect("sample coupled land centers");
    let sampled_ocean =
        earthmesh_cli::mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based(
            &landtype_file,
            1,
            &ocean_centers,
        )
        .expect("sample coupled ocean centers");
    assert!(
        sampled_land.iter().all(|&value| value != 0),
        "LOC land output should contain only land-classified cells"
    );
    assert!(
        sampled_ocean.iter().all(|&value| value == 0),
        "LOC ocean output should contain only ocean-classified cells"
    );

    let class_points = earthmesh_cli::colm_package_io::read_colm_surface_class_points_netcdf(
        &coupled.coupling_netcdf.output,
    )
    .expect("read coupled CoLM surface classes");
    assert_eq!(class_points.len(), coupled.coupling_netcdf.rows);
    assert!(class_points.iter().any(|point| point.code == 1));
    assert!(class_points.iter().any(|point| point.code == 2));

    let _ = (land_topology, ocean_topology, land_mesh, ocean_mesh, run);
}

#[test]
fn default_land_global_specified_refine_without_landtype_uses_method_c_spawn_nest() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_land_global_refine");
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

    let namelist = root.join("mkgrd_method_c_land_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_land_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C land specified refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default land specified refine without landtype should use Method-C direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "landmesh");
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(run.transition_faces > 0);
    assert_eq!(
        run.output.output,
        root.join("case_method_c_land_circle_refine/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C land direct output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_ocean_global_specified_refine_without_landtype_uses_method_c_spawn_nest() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_ocean_global_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 1,
            points: vec![LonLatPoint {
                lon: 140.0,
                lat: 10.0,
            }],
            radius_km: vec![2_500.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("mkgrd_method_c_ocean_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_ocean_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C ocean specified refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default ocean specified refine without landtype should use Method-C direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "oceanmesh");
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(run.transition_faces > 0);
    assert_eq!(
        run.output.output,
        root.join("case_method_c_ocean_circle_refine/result/gridfile_NXP0006_tri.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C ocean direct output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_regional_specified_refine_uses_method_c_and_subsets_domain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_regional_refine_bbox_domain");
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
    write_bbox_mask_netcdf(
        sources.join("domain_bbox_000.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: 100.0,
                east: 130.0,
                north: 40.0,
                south: 10.0,
            }],
        },
    )
    .expect("write bbox domain source");

    let namelist = root.join("mkgrd_method_c_regional_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_regional_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_prefix}'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C regional specified refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default regional specified refine should use Method-C direct path");
    };
    assert!(!run.runtime_state.config.mask_domain_global);
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let points = non_placeholder_points(&refined_mesh.w_points);
    assert!(
        !points.is_empty(),
        "regional Method-C output should keep cells"
    );
    let min_lon = points.iter().map(|p| p.lon).fold(f64::INFINITY, f64::min);
    let max_lon = points
        .iter()
        .map(|p| p.lon)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_lat = points.iter().map(|p| p.lat).fold(f64::INFINITY, f64::min);
    let max_lat = points
        .iter()
        .map(|p| p.lat)
        .fold(f64::NEG_INFINITY, f64::max);

    assert!(
        max_lon - min_lon < 80.0,
        "regional Method-C output should be subset, got lon span {min_lon}..{max_lon}"
    );
    assert!(
        max_lat - min_lat < 80.0,
        "regional Method-C output should be subset, got lat span {min_lat}..{max_lat}"
    );

    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "regional Method-C output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_atmos_close_specified_refine_uses_method_c_polygon_region() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_atmos_close_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_close_mask_netcdf(
        sources.join("refine_close_001_001.nc4"),
        &CloseMask {
            refine_degree: 1,
            points: vec![
                LonLatPoint {
                    lon: 100.0,
                    lat: 15.0,
                },
                LonLatPoint {
                    lon: 130.0,
                    lat: 15.0,
                },
                LonLatPoint {
                    lon: 130.0,
                    lat: 35.0,
                },
                LonLatPoint {
                    lon: 100.0,
                    lat: 35.0,
                },
            ],
        },
    )
    .expect("write close specified refine source");

    let namelist = root.join("mkgrd_method_c_atmos_close_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_close").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_atmos_close_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='close'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C atmos close specified refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default close specified refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "close specified refine should add local child Voronoi cells"
    );

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C close specified refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_land_calculated_refine_uses_method_c_region_source() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_land_calculated_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_cal_bbox_0_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: 105.0,
                east: 125.0,
                north: 35.0,
                south: 15.0,
            }],
        },
    )
    .expect("write calculated refine source");

    let namelist = root.join("mkgrd_method_c_land_calculated_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_cal_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_land_calculated_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C land calculated refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default calculated refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "calculated refine should add local child Voronoi cells"
    );

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C calculated refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_land_calculated_circle_refine_filters_degrees_by_active_max_level() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_land_calculated_circle_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_cal_circle_0_01.nc4"),
        &CircleMask {
            refine_degree: 0,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_000.0],
        },
    )
    .expect("write calculated active circle refine source");
    write_circle_mask_netcdf(
        sources.join("refine_cal_circle_3_01.nc4"),
        &CircleMask {
            refine_degree: 3,
            points: vec![LonLatPoint {
                lon: -70.0,
                lat: -20.0,
            }],
            radius_km: vec![2_000.0],
        },
    )
    .expect("write calculated out-of-range circle refine source");

    let namelist = root.join("mkgrd_method_c_land_calculated_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_cal_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_land_calculated_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_cal_type='circle'\n  RL%mask_refine_cal_fprefix='{refine_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C land calculated circle refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default calculated circle refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.regions[0].level(), 1);
    assert_eq!(run.max_level, 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "calculated circle refine should add local child Voronoi cells"
    );

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C calculated circle refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_land_calculated_close_refine_promotes_zero_degree_to_max_level() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_land_calculated_close_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_close_mask_netcdf(
        sources.join("refine_cal_close_0_01.nc4"),
        &CloseMask {
            refine_degree: 0,
            points: vec![
                LonLatPoint {
                    lon: 100.0,
                    lat: 15.0,
                },
                LonLatPoint {
                    lon: 130.0,
                    lat: 15.0,
                },
                LonLatPoint {
                    lon: 130.0,
                    lat: 35.0,
                },
                LonLatPoint {
                    lon: 100.0,
                    lat: 35.0,
                },
            ],
        },
    )
    .expect("write calculated close refine source");

    let namelist = root.join("mkgrd_method_c_land_calculated_close_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_cal_close").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_land_calculated_close_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_cal_type='close'\n  RL%mask_refine_cal_fprefix='{refine_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_default_restart_handoff::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, 1, None,
    )
    .expect("run default Method-C land calculated close refine");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDefaultRestartRefineRunReport::RefinePipeline(
        run,
    ) = report
    else {
        panic!("default calculated close refine should use Method-C direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.regions[0].level(), 1);
    assert_eq!(run.max_level, 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "calculated close refine should add local child Voronoi cells"
    );

    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C calculated close refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn refine_pipeline_refine_uses_existing_mode_file_as_method_c_source() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_refine_uses_mode_file");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");

    let base_dir = format!("{}/", root.display());
    let gridinit_namelist = root.join("mkgrd_gridinit_source.nml");
    fs::write(
        &gridinit_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_mode_file_source'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n",
        ),
    )
    .expect("write gridinit namelist");
    let gridinit = earthmesh_cli::mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist(
        &gridinit_namelist,
        &root,
        20_000,
    )
    .expect("write initial mode_file grid");

    write_bbox_mask_netcdf(
        sources.join("refine_bbox_001.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: 100.0,
                east: 125.0,
                north: 35.0,
                south: 15.0,
            }],
        },
    )
    .expect("write bbox specified refine source");

    let refine_prefix = sources.join("refine_bbox").display().to_string();
    let mode_file = gridinit.gridfile.output.display().to_string();
    let refine_namelist = root.join("mkgrd_method_c_refine_mode_file.nml");
    fs::write(
        &refine_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_method_c_refine_mode_file'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{mode_file}'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write refine namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&refine_namelist, &root, 20_000, None)
        .expect("refine existing EarthMesh mode_file through Method-C source reconstruction");
    assert!(
        run.output.sjx_points > gridinit.gridfile.sjx_points,
        "mode_file source should be refined"
    );
    assert!(
        run.output.sjx_points < 3_000,
        "direct refine ignored the NXP=6 mode_file source and rebuilt an NXP=16 source"
    );
    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read output mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "Method-C mode_file refined topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
}

#[test]
fn top_level_dispatcher_routes_refine_namelist_to_refine_pipeline_report() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_top_level_dispatcher_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_bbox_001.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: 100.0,
                east: 125.0,
                north: 35.0,
                south: 15.0,
            }],
        },
    )
    .expect("write bbox specified refine source");

    let namelist = root.join("mkgrd_top_level_dispatcher_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_level_dispatcher_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_top_level_dispatch::run_mkgrd_top_level_namelist(
        &namelist, &root, 20_000, 0,
    )
    .expect("top-level dispatcher should run refine through Method-C direct");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDispatchRunReport::RefinePipeline(run) =
        report
    else {
        panic!("refine namelist should dispatch to Method-C direct branch");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "atmosmesh");
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "top-level dispatcher should return refined Method-C output"
    );
}

#[test]
fn top_level_dispatcher_compatibility_atmos_mesh_name_routes_to_refine_pipeline_report() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("method_c_top_level_dispatcher_compatibility_atmos_meshname_refine");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("refine_bbox_001.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: 100.0,
                east: 125.0,
                north: 35.0,
                south: 15.0,
            }],
        },
    )
    .expect("write bbox specified refine source");

    let namelist = root.join("mkgrd_top_level_dispatcher_refine_meshname.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_top_level_dispatcher_compatibility_atmos_meshname_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmos'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::mkgrd_top_level_dispatch::run_mkgrd_top_level_namelist(
        &namelist, &root, 20_000, 0,
    )
    .expect("top-level dispatcher should run refine through Method-C direct");

    let earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDispatchRunReport::RefinePipeline(run) =
        report
    else {
        panic!("refine namelist should dispatch to Method-C direct branch");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "atmos");
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "top-level dispatcher should return refined Method-C output"
    );
}

/// The red-green route, end to end: a project names a circle, the backend
/// splits the triangles it holds, and a gridfile comes out with more cells than
/// the mesh it started from.
///
/// Every link of this chain had its own test before this one, and the chain
/// still did not run: the pipeline refused the backend at the adaptive call
/// site. Without an end-to-end case a wrong `output_mesh` writes a file that
/// opens fine.
#[test]
fn redgreen_backend_refines_a_named_circle_end_to_end() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("redgreen_named_circle");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    // Two levels, so the chaining is exercised rather than just the first
    // round: a level-2 mask carries its own parent halo down to level 1, which
    // is the nesting red-green's `halo` erodes against.
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 2,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("mkgrd_redgreen_circle.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_redgreen_circle'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='red_green'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write red-green namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect("red_green backend should run through the refinement pipeline");

    assert_eq!(run.max_level, 2);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "red-green should refine the initial mesh: {} vs {}",
        run.output.lbx_points,
        run.gridinit.gridfile.lbx_points
    );
    // Not synthesised to make a log line look better: red-green reports no
    // per-cell generations, so nothing was measured off this mesh. The
    // requested depth travels as `max_level`.
    assert_eq!(run.realized_max_level, 0);
    assert_eq!(run.transition_faces, 0);
    // The run record's counts come from the gridfile when there is no Voronoi
    // state to take them from.
    assert!(run.runtime_state.grid.nwa > 0);
    assert!(run.runtime_state.grid.nma > run.runtime_state.grid.nwa);

    // "More cells" is what a wrong `output_mesh` would also produce. This is
    // the assertion that says the file is a mesh.
    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final red-green mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "red-green output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
}

/// The harp_dv backend, from namelist to gridfile.
///
/// The one test that says the backend exists as far as a user is concerned.
/// Everything below it works on `MeshState`; this is where that becomes a file
/// with a consistent topology, read back rather than trusted.
#[test]
fn harp_dv_backend_refines_a_named_circle_end_to_end() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("redgreen_named_circle");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    // Two levels, so the chaining is exercised rather than just the first
    // round: a level-2 mask carries its own parent halo down to level 1, which
    // is the nesting red-green's `halo` erodes against.
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 2,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("mkgrd_harp_dv_circle.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_harp_dv_circle'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='harp_dv'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=1\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=20\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write harp_dv namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect("harp_dv backend should run through the refinement pipeline");

    assert_eq!(run.max_level, 2);
    assert_eq!(run.spring_nest_iterations, 0);
    assert_eq!(
        run.spring_nest_passes, 0,
        "HARP-DV owns smoothing through transactional site moves"
    );
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "harp_dv should refine the initial mesh: {} vs {}",
        run.output.lbx_points,
        run.gridinit.gridfile.lbx_points
    );
    // HARP-DV does fill per-cell generations -- it carries a depth per site --
    // so unlike red-green there is something measured off this mesh.
    assert!(run.realized_max_level > 0);
    // And no transition band, which is red-green's answer too and for the same
    // reason: it does not build one, so zero reads "not measured from this
    // mesh" rather than "none were needed".
    assert_eq!(run.transition_faces, 0);
    // The run record's counts come from the gridfile when there is no Voronoi
    // state to take them from.
    assert!(run.runtime_state.grid.nwa > 0);
    assert!(run.runtime_state.grid.nma > run.runtime_state.grid.nwa);

    // "More cells" is what a wrong `output_mesh` would also produce. This is
    // the assertion that says the file is a mesh.
    let refined_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .expect("read final harp_dv mesh");
    let topology =
        earthmesh_cli::unstructured_mesh_support::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "harp_dv output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
}

#[test]
fn harp_trace_failure_does_not_write_final_gridfile() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("harp_trace_existing_target_failure");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 1,
            points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }],
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("mkgrd_harp_trace_existing_target_failure.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_harp_trace_existing_target_failure'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='harp_dv'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n&harp_dv\n  NL%max_cycles=1\n/\n",
        ),
    )
    .expect("write HARP trace failure namelist");

    let trace_target = root.join("harp_trace.jsonl");
    fs::write(&trace_target, "sentinel\n").expect("write existing trace sentinel");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
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
        .env("EARTHMESH_HARP_TRACE_JSONL", &trace_target)
        .current_dir(&root)
        .output()
        .expect("run HARP trace failure case");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("HARP trace target already exists"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&trace_target).expect("read trace sentinel"),
        "sentinel\n"
    );
    let result = root.join("case_harp_trace_existing_target_failure/result");
    assert!(!result.join("gridfile_NXP0006_hex.nc4").exists());
    assert!(!result.join("harp_dv_conservative_remap.csv").exists());
}

#[test]
fn harp_dv_backend_consumes_adaptive_landcover_criteria_without_named_regions() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("harp_dv_landcover_adaptive");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_mask_netcdf(
        sources.join("domain_bbox_000.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -6.0,
                east: 6.0,
                north: 6.0,
                south: -6.0,
            }],
        },
    )
    .expect("write bbox domain source");
    let landtype = root.join("landtype.nc");
    write_sparse_landtype_file_for_gridnum(&landtype, 120);

    let namelist = root.join("mkgrd_harp_dv_landcover.nml");
    let base_dir = format!("{}/", root.display());
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd
  NL%EXPNME='case_harp_dv_landcover'
  NL%base_dir='{base_dir}'
  NL%NXP=6
  NL%mesh_type='landmesh'
  NL%mode_grid='hex'
  NL%mode_file='none'
  NL%mode_file_description='none'
  NL%refine=.true.
  NL%refine_backend='harp_dv'
  NL%niter=0
  NL%beta=1.0
  NL%relax=0.25
  NL%gridnum_perdegree=120
  NL%landtype_file='{}'
  NL%mask_domain_global=.false.
  NL%mask_domain_type='bbox'
  NL%mask_domain_fprefix='{domain_prefix}'
  NL%mask_patch_on=.false.
  NL%output_format='CoLM'
/
&mkrefine
  RL%Istransition=.true.
  RL%SpringGlobal_type=0
  RL%SpringRegional_type=0
  RL%refine_spc=.false.
  RL%refine_cal=.true.
  RL%max_iter_spc=0
  RL%max_iter_cal=1
  RL%niter_refine=0
  RL%num_rc=1
  RL%set_dis_type='linear'
  RL%halo=3,3,3,0,0,0,0,0,0
  RL%max_transition_row=3,3,3,0,0,0,0,0,0
  RL%mask_refine_cal_fprefix='/tmp'
  RL%refine_num_landtypes=.true.
  RL%th_num_landtypes=1
/
&harp_dv
  NL%max_cycles=1
/
&adaptive
  NL%adaptive_on=.true.
  NL%adaptive_max_level=1
  NL%adaptive_base_m=800000.0
  NL%adaptive_coastline=.false.
/
",
            landtype.display()
        ),
    )
    .expect("write harp_dv landcover namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect("HARP-DV should consume adaptive landcover criteria");
    let harp = run.harp_dv_run.expect("HARP-DV report");
    assert_eq!(
        harp.cycles_completed, 1,
        "&harp_dv controls must reach the backend"
    );
    assert!(
        harp.transactions_committed > 0,
        "adaptive landcover criteria must do work: {harp:?}"
    );
    assert!(harp.active_adaptive_sites >= harp.active_leaf_sites);
    assert_eq!(
        harp.leaf_degree_4
            + harp.leaf_degree_5
            + harp.leaf_degree_6
            + harp.leaf_degree_7
            + harp.leaf_degree_other,
        harp.active_leaf_sites
    );
    assert!(harp.leaf_birth_cycle_min <= harp.leaf_birth_cycle_max);
    assert!(harp.leaf_target_scale_min_m <= harp.leaf_target_scale_max_m);
    assert!(harp.interior_leaf_sites <= harp.active_leaf_sites);
    assert!(harp.leaf_target_scale_measured <= harp.active_leaf_sites);
    assert!(harp.lineage_unknown_adaptive_sites <= harp.active_adaptive_sites);
    assert!(
        harp.angles_below_40_at_interior_leaf_vertices <= harp.angles_below_40_at_leaf_vertices
    );
    assert!(
        harp.angles_above_80_at_interior_leaf_vertices <= harp.angles_above_80_at_leaf_vertices
    );
    assert!(
        harp.violating_triangles_touching_interior_leaf <= harp.violating_triangles_touching_leaf
    );
    assert_eq!(harp.d4_leaf_retirement_candidates, 0);
    assert!(harp.d4_leaf_retirement_fully_acceptable <= harp.d4_leaf_retirement_quality_improving);
    assert!(harp.d4_leaf_retirement_fully_acceptable <= harp.d4_leaf_retirement_candidates);
    assert!(
        run.realized_region_halvings > 0.0,
        "adaptive circles must reach the achieved-resolution measurement"
    );
    assert!(run.output.output.exists());
    let adaptive_report = run
        .output
        .output
        .parent()
        .expect("result dir")
        .join("adaptive_refinement.json");
    assert!(
        adaptive_report.exists(),
        "HARP-DV adaptive criteria must leave the same demand report as the other consumers"
    );
    assert!(
        fs::read_to_string(adaptive_report)
            .expect("adaptive report")
            .contains("\"base_m\":800000"),
        "explicit adaptive_base_m must reach the HARP-DV target/report"
    );
}

/// A request red-green cannot serve is refused, not served with less.
///
/// The marking comes from named regions; an h-field's target levels are simply
/// not read on this route. Ignoring them would produce a mesh that is valid,
/// passes its quality checks, and is not what the project asked for.
#[test]
fn redgreen_backend_refuses_a_request_it_would_have_to_ignore() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("redgreen_hfield_refusal");
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
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("redgreen_hfield.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='redgreen_hfield'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='red_green'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n&hfield\n  NL%hfield_on=.true.\n  NL%hfield_max_level=1\n/\n",
        ),
    )
    .expect("write red-green h-field namelist");

    let error = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 20_000, None)
        .expect_err("red_green must not silently drop an h-field");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
    assert!(
        error.to_string().contains("does not serve an h-field"),
        "unexpected error: {error}"
    );
}

/// Method-C's adaptive section does not stop a red-green run.
///
/// `&adaptive` is one of Method-C's two ways of turning criteria into
/// refinement, and the project layer defaults it on for any refining run. A
/// backend that has no reader for it must treat it as not its business rather
/// than refuse -- refusing would fail every red-green project that carries the
/// default, which is all of them.
#[test]
fn redgreen_backend_is_not_stopped_by_method_cs_adaptive_section() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("redgreen_with_adaptive_section");
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
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("mkgrd_redgreen_adaptive.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_redgreen_adaptive'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='red_green'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n&adaptive\n  NL%adaptive_on=.true.\n  NL%adaptive_coastline=.true.\n/\n",
        ),
    )
    .expect("write red-green namelist carrying the adaptive default");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect("the adaptive default must not refuse a red-green run");

    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "and the named circle must still refine: {} vs {}",
        run.output.lbx_points,
        run.gridinit.gridfile.lbx_points
    );
}

/// Criteria with nowhere to go are refused, not walked into the region reader.
///
/// `refine_cal` means "a criterion decides where to refine". Both routes that
/// read a criterion in-process -- the h-field and point+radius -- are
/// Method-C's; red-green refines named regions. Mask *files* it serves fine,
/// because a mask file is a named region by another name. With the prefix left
/// at the engine's `/tmp` sentinel there is no file and no reader, and the
/// calculated-region reader would go looking anyway and fail with a message
/// about Method-C and a path nobody typed.
#[test]
fn redgreen_backend_refuses_criteria_it_has_no_reader_for() {
    let root = temp_root("redgreen_refine_cal_no_masks");
    let namelist = root.join("redgreen_refine_cal.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='redgreen_refine_cal'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='red_green'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=0\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=3\n  RL%mask_refine_cal_fprefix='/tmp'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n/\n",
        ),
    )
    .expect("write red-green calculated-criteria namelist");

    let error = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect_err("criteria with no reader must be refused");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
    assert!(
        error
            .to_string()
            .contains("no reader for calculated criteria"),
        "unexpected error: {error}"
    );
}

/// Red-green refuses to run without its transition rows.
///
/// `RL%Istransition = .false.` is a setting the engine accepts for
/// `mode_grid = 'tri'` alone, and Method-C produces a closed mesh under it. For
/// red-green those rows *are* the closure step -- the green half of red-green --
/// so without them a single level already comes out with hanging nodes: 345
/// triangle edges with no neighbouring triangle, on the shipped atmosphere
/// example switched to tri.
#[test]
fn redgreen_backend_refuses_to_run_without_its_transition_rows() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("redgreen_no_transition");
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
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("redgreen_no_transition.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='redgreen_no_transition'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='red_green'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.false.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write red-green no-transition namelist");

    let error = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect_err("a mesh with hanging nodes must not be written");
    assert!(
        error.to_string().contains("requires RL%Istransition"),
        "unexpected error: {error}"
    );
}

/// What a harp_dv mesh actually measures, put through the quality gate.
///
/// The reason the pipeline was wired before the remaining tuning: section
/// 13.3's improvement gate needs weights, and the only honest source for them
/// is what a real mesh scores. Nothing before this point could produce a
/// number -- the backend worked on `MeshState` and never reached a file.
///
/// This asserts the two things that decide whether the mesh is usable at all,
/// and prints the rest so the numbers are visible when the gate is next
/// touched.
#[test]
fn harp_dv_output_passes_the_mesh_quality_gate() {
    let root = temp_root("harp_dv_quality");
    let sources = root.join("sources");
    std::fs::create_dir_all(&sources).expect("create sources");
    write_circle_mask_netcdf(
        sources.join("refine_circle_001.nc4"),
        &CircleMask {
            refine_degree: 2,
            points: vec![LonLatPoint {
                lon: 115.0,
                lat: 25.0,
            }],
            radius_km: vec![2_000.0],
        },
    )
    .expect("write circle specified refine source");

    let namelist = root.join("mkgrd_harp_dv_quality.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    std::fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_harp_dv_quality'\n  NL%base_dir='{base_dir}'\n  NL%NXP=21\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='harp_dv'\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=0\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=3,3,3,0,0,0,0,0,0\n  RL%max_transition_row=3,3,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write harp_dv namelist");

    let run = earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 200_000, None)
        .expect("harp_dv backend should run");

    let mesh = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
        .expect("read harp_dv gridfile mesh points");
    let input = earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile_hex(&mesh)
        .expect("hex quality input");
    let report =
        earthmesh_quality::compute(&input, &earthmesh_quality::QualityThresholds::default());
    let harp = run.harp_dv_run.as_ref().expect("HARP-DV report");

    println!(
        "harp_dv quality: cells {}, min angle {:.2} deg, max angle {:.2} deg, area ratio {:.3}, \
         edge km mean {:.1}",
        report.geometry.cell_count,
        report.geometry.min_angle_deg,
        report.geometry.max_angle_deg,
        report.geometry.cell_area_ratio,
        report.geometry.edge_length_km.mean,
    );

    // A mesh with a degenerate cell is not a coarser answer to the same
    // question; it is one a solver cannot use.
    assert!(
        report.geometry.min_angle_deg > 0.0,
        "a cell has a zero or inverted angle: {:?}",
        report.geometry
    );
    // Topology is the claim the whole transaction layer rests on. If any of
    // these fails, every hard gate above it was measuring the wrong thing.
    assert_eq!(report.topology.connected_component_count, 1);
    assert_eq!(report.topology.non_manifold_vertex_fan_count, 0);
    assert_eq!(report.topology.invalid_vertex_index_count, 0);
    assert_eq!(report.topology.euler_characteristic_mismatch_count, 0);
    assert!(harp.quality_optimiser_moves > 0);
    assert!(harp.triangle_eta_min.is_finite() && harp.triangle_eta_min > 0.0);
    assert!(harp.triangle_eta_p1 >= harp.triangle_eta_min);
}

/// The native spawn refuses a route it would otherwise swallow -- and only then.
///
/// It sits at the head of Method-C's branch chain and never consults `&adaptive`
/// or `&hfield`, so a namelist carrying both used to get the native mesh in
/// silence: measured at NXP 6, `&nsfcgrids` with and without `&adaptive`
/// produced bit-identical 435-cell meshes, exit 0, and not one line of adaptive
/// output.
///
/// The pair is not always a swallow, which is the part worth pinning. With
/// `refine_spc` on, the native spawn stands down and the h-field branch runs --
/// that is how Cartesian-XY serves `&ngrids` and an h-field together. A guard
/// keyed on "native regions are configured" rather than on the branch's own
/// condition refuses that too, and 64 tests said so.
#[test]
fn native_grids_refuse_a_route_they_would_swallow_and_not_one_they_share() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("native_route_composition");
    let base_dir = format!("{}/", root.display());

    let namelist_with = |name: &str, refine_spc: &str, extra: &str| {
        let path = root.join(format!("{name}.nml"));
        fs::write(
            &path,
            format!(
                "&mkgrd\n  NL%EXPNME='{name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  \
                 NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  \
                 NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  \
                 NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  \
                 NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  \
                 NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  \
                 NL%sfcgrdrad(1,1)=2500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  \
                 NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  \
                 RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  \
                 RL%refine_spc={refine_spc}\n  RL%refine_cal=.false.\n/\n{extra}",
            ),
        )
        .expect("write namelist");
        path
    };

    // Native alone: the spawn owns the run and nothing is refused.
    let alone = namelist_with("native_alone", ".false.", "");
    earthmesh_cli::run_refine_pipeline_namelist(&alone, &root, 20_000, None)
        .expect("native grids alone");

    // Native plus a route it never reads: refused by name rather than dropped.
    let with_adaptive = namelist_with(
        "native_with_adaptive",
        ".false.",
        "&adaptive\n  NL%adaptive_on = .true.\n  NL%adaptive_max_level = 2\n/\n",
    );
    let error = earthmesh_cli::run_refine_pipeline_namelist(&with_adaptive, &root, 20_000, None)
        .expect_err("refused");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported, "{error}");
    assert!(error.to_string().contains("&adaptive"), "{error}");
    assert!(
        error.to_string().contains("nothing composes them"),
        "{error}"
    );
}
