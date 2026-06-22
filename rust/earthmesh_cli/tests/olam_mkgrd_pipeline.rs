use earthmesh_cli::{
    write_bbox_mask_netcdf, write_circle_mask_netcdf, write_close_mask_netcdf, BBoxMask, BBoxPoint,
    CircleMask, CloseMask, LonLatPoint,
};
use earthmesh_mesh::OlamDelaunayMesh;
use std::{fs, path::PathBuf};

static NETCDF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug)]
struct OlamAtmosLandLikeCaseResult {
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

fn run_olam_atmos_landlike_case(
    root: &PathBuf,
    mesh_type: &str,
    output_format: &str,
    case_name: &str,
) -> OlamAtmosLandLikeCaseResult {
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

    let namelist = root.join(format!("mkgrd_olam_{case_name}.nml"));
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='{case_name}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='{mesh_type}'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='{output_format}'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM specified refine");
    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default OLAM specified refine should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type.trim(), mesh_type);
    assert_eq!(run.runtime_state.config.mode_grid.trim(), "hex");
    assert_eq!(run.runtime_state.config.output_format.trim(), output_format);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(
        run.output.output.exists(),
        "OLAM specified refine output file should exist: {:?}",
        run.output.output
    );
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);

    OlamAtmosLandLikeCaseResult {
        base_lbx_points: run.gridinit.gridfile.lbx_points,
        added_lbx_points: run.output.lbx_points - run.gridinit.gridfile.lbx_points,
        transition_faces: run.transition_faces,
        spring_nest_iterations: run.spring_nest_iterations,
        max_level: run.max_level,
        spring_nest_passes: run.spring_nest_passes,
        output_lbx_points: run.output.lbx_points,
    }
}

fn write_landtype_file_by_predicate(path: &std::path::Path, is_land: impl Fn(f64, f64) -> bool) {
    let (nlons, nlats) = (360, 180);
    let mut file = netcdf::create(path).expect("create landtype file");
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

fn non_placeholder_points(points: &[LonLatPoint]) -> Vec<LonLatPoint> {
    points
        .iter()
        .copied()
        .filter(|point| point.lon != 0.0 || point.lat != 0.0)
        .collect()
}

#[test]
fn method_c_atmos_and_surface_constants_match_fortran_defaults() {
    assert_eq!(OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS, 13);
    assert_eq!(OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE, 7);
    assert!(
        OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS > OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
    );
}

#[test]
fn default_atmos_and_landlike_meshes_use_different_transition_widths() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_transition_width_comparison");

    let atmos =
        run_olam_atmos_landlike_case(&root, "atmosmesh", "MPAS", "case_olam_atmos_transition_cmp");
    let land =
        run_olam_atmos_landlike_case(&root, "landmesh", "CoLM", "case_olam_land_transition_cmp");

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
fn default_atmos_global_specified_refine_uses_olam_spawn_nest() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_global_refine");
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

    let namelist = root.join("mkgrd_olam_atmos_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_atmos_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM atmos specified refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default atmos specified refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(run.transition_faces > 0);
    assert!(run.source_branch_reports().is_empty());
    assert_eq!(
        run.output.output,
        root.join("case_olam_atmos_circle_refine/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "OLAM specified refine should add Voronoi cells: initial={} final={}",
        run.gridinit.gridfile.lbx_points,
        run.output.lbx_points
    );

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM direct output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_atmos_specified_refine_ignores_zero_degree_masks() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_specified_zero_degree");
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

    let namelist = root.join("mkgrd_olam_atmos_zero_degree_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_atmos_zero_degree_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("specified degree-zero masks should not create OLAM regions");

    assert!(
        err.to_string()
            .contains("OLAM direct refine found no region sources"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_ngrids_uses_method_c_region() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_ngrids");
    let namelist = root.join("mkgrd_olam_native_ngrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_ngrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native OLAM namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native OLAM ngrids refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native OLAM ngrids refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_atmos_native_olam_spawn_nest_springs_even_when_mkrefine_global_spring_disabled() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_spring_ignores_mkrefine_flag");
    let namelist = root.join("mkgrd_olam_native_spring_ignores_mkrefine_flag.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_spring_ignores_mkrefine_flag'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=1\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native OLAM spring namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native OLAM spring refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native OLAM spring refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert!(run.spring_nest_passes > 0);
}

#[test]
fn default_atmos_native_olam_makegrid_plot_uses_fortran_plot_spring_iterations() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_makegrid_plot_spring");
    let namelist = root.join("mkgrd_olam_native_makegrid_plot_spring.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_makegrid_plot_spring'\n  NL%runtype='MAKEGRID_PLOT'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=5000\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native OLAM MAKEGRID_PLOT namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native OLAM MAKEGRID_PLOT refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native OLAM MAKEGRID_PLOT refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 100);
    assert!(run.spring_nest_passes > 0);
}

#[test]
fn default_atmos_native_olam_ngrids_three_spawns_each_fortran_grid_once() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_ngrids_three_once");
    let namelist = root.join("mkgrd_olam_native_ngrids_three_once.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_ngrids_three_once'\n  NL%base_dir='{base_dir}'\n  NL%NXP=12\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=3\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%ngrdll(3)=1\n  NL%grdrad(3,1)=500000.0\n  NL%grdlat(3,1)=-25.0\n  NL%grdlon(3,1)=-65.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=1\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native atmosphere three-grid namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native atmosphere three-grid refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native atmosphere ngrids should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 2);
    assert_eq!(
        run.regions
            .iter()
            .map(earthmesh_mesh::OlamRefinementRegion::level)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(run.spring_nest_iterations, 5000);
    assert_eq!(
        run.spring_nest_passes, 2,
        "Fortran spawn_nest loops nn=2..NGRIDS and spawns each native grid once"
    );
}

#[test]
fn default_atmos_native_olam_ngrids_above_six_uses_fortran_maxgrds_not_refine_level_limit() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_ngrids_fortran_maxgrds");
    let namelist = root.join("mkgrd_olam_native_ngrids_fortran_maxgrds.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_ngrids_fortran_maxgrds'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=7\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%ngrdll(3)=1\n  NL%grdrad(3,1)=2400000.0\n  NL%grdlat(3,1)=20.0\n  NL%grdlon(3,1)=110.0\n  NL%ngrdll(4)=1\n  NL%grdrad(4,1)=2300000.0\n  NL%grdlat(4,1)=15.0\n  NL%grdlon(4,1)=105.0\n  NL%ngrdll(5)=1\n  NL%grdrad(5,1)=2200000.0\n  NL%grdlat(5,1)=10.0\n  NL%grdlon(5,1)=100.0\n  NL%ngrdll(6)=1\n  NL%grdrad(6,1)=2100000.0\n  NL%grdlat(6,1)=5.0\n  NL%grdlon(6,1)=95.0\n  NL%ngrdll(7)=1\n  NL%grdrad(7,1)=2000000.0\n  NL%grdlat(7,1)=0.0\n  NL%grdlon(7,1)=90.0\n/\n",
        ),
    )
    .expect("write native atmosphere Fortran maxgrds namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, None, 1, None,
    )
    .expect_err("max_tris=0 should stop after native ngrids passes Fortran maxgrds validation");

    assert!(
        !err.to_string().contains("max_iter_spc/max_iter_cal"),
        "native OLAM ngrids must not use specified/calculated refine level limit: {err}"
    );
}

#[test]
fn default_atmos_native_olam_ngrids_does_not_require_refine_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_ngrids_no_refine");
    let namelist = root.join("mkgrd_olam_native_ngrids_no_refine.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_ngrids_no_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere no-refine namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native atmosphere ngrids without refine flag");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native atmosphere ngrids should use OLAM direct path without refine flag");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_atmos_native_olam_rejects_out_of_range_latitude_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_bad_latitude");
    let namelist = root.join("mkgrd_olam_native_bad_latitude.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_bad_latitude'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=95.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native latitude namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM latitude outside Fortran bounds should fail");

    assert!(
        err.to_string().contains("grdlat"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_radius_larger_than_double_earth_radius_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_large_radius");
    let namelist = root.join("mkgrd_olam_native_large_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_large_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=13000000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write large native radius namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM radius above Fortran erad2 bound should fail");

    assert!(
        err.to_string().contains("grdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_radius_below_fortran_dzxmin() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_small_radius");
    let namelist = root.join("mkgrd_olam_native_small_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_small_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=0.0005\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write small native radius namelist");

    let result = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native OLAM radius below Fortran dzxmin should fail"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("grdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_zero_ngrids_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_zero_ngrids");
    let namelist = root.join("mkgrd_olam_native_zero_ngrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_zero_ngrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=0\n/\n",
        ),
    )
    .expect("write zero native ngrids namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM ngrids below Fortran bound should fail");

    assert!(
        err.to_string().contains("ngrids"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_ngrids_above_fortran_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_many_ngrids");
    let namelist = root.join("mkgrd_olam_native_many_ngrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_many_ngrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=21\n/\n",
        ),
    )
    .expect("write too many native grids namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM ngrids above Fortran maxgrds should fail");

    assert!(
        err.to_string().contains("ngrids"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_gridplot_base_below_fortran_lower_bound() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_bad_gridplot_base");
    let namelist = root.join("mkgrd_olam_native_bad_gridplot_base.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_bad_gridplot_base'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%gridplot_base=1\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native gridplot_base namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM gridplot_base below Fortran lower bound should fail");

    assert!(
        err.to_string().contains("gridplot_base"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_mdomain_above_fortran_upper_bound() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_bad_mdomain");
    let namelist = root.join("mkgrd_olam_native_bad_mdomain.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_bad_mdomain'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=6\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native mdomain namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM mdomain above Fortran upper bound should fail");

    assert!(
        err.to_string().contains("mdomain"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_ngrdll_above_fortran_maxngrdll() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_many_ngrdll");
    let namelist = root.join("mkgrd_olam_native_many_ngrdll.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_many_ngrdll'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=21\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write too many native grid points namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM ngrdll above Fortran maxngrdll should fail");

    assert!(
        err.to_string().contains("ngrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_ngrdll_index_above_fortran_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_ngrdll_bad_index");
    let namelist = root.join("mkgrd_olam_native_ngrdll_bad_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_ngrdll_bad_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(21)=1\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native grid point count with bad index namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM ngrdll index above Fortran maxgrds should fail");

    assert!(
        err.to_string().contains("ngrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_zero_ngrdll_index_like_fortran_array() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_ngrdll_zero_index");
    let namelist = root.join("mkgrd_olam_native_ngrdll_zero_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_ngrdll_zero_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(0)=1\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native grid point count with zero index namelist");

    let result = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native OLAM ngrdll index 0 should fail like a Fortran array"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("ngrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_coordinate_point_index_above_fortran_maxngrdll() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_grdrad_bad_point_index");
    let namelist = root.join("mkgrd_olam_native_grdrad_bad_point_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_grdrad_bad_point_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,21)=2500000.0\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native coordinate with bad point index namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM coordinate point index above Fortran maxngrdll should fail");

    assert!(
        err.to_string().contains("grdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_atmos_native_olam_rejects_global_nxp_not_divisible_by_three_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_bad_nxp");
    let namelist = root.join("mkgrd_olam_native_bad_nxp.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_bad_nxp'\n  NL%base_dir='{base_dir}'\n  NL%NXP=7\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native NXP namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM global NXP not divisible by 3 should fail");

    assert!(err.to_string().contains("NXP"), "unexpected error: {err}");
}

#[test]
fn default_atmos_native_olam_allows_non_global_domain_nxp_not_divisible_by_three_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_regional_nxp5");
    let namelist = root.join("mkgrd_olam_native_regional_nxp5.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_regional_nxp5'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere regional NXP namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, None, 1, None,
    )
    .expect_err("max_tris=0 should stop after regional NXP validation passes");

    assert!(
        !err.to_string().contains("NXP must be divisible by 3"),
        "Fortran only applies this NXP check to mdomain < 2 global runs: {err}"
    );
}

#[test]
fn default_atmos_native_olam_allows_non_global_domain_cartesian_coordinates_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_regional_cartesian");
    let namelist = root.join("mkgrd_olam_native_regional_cartesian.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_regional_cartesian'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=95.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere regional Cartesian namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, None, 1, None,
    )
    .expect_err("max_tris=0 should stop after regional Cartesian coordinate validation passes");

    assert!(
        !err.to_string().contains("grdlat"),
        "Fortran only applies GRDLAT bounds for mdomain < 2 global runs: {err}"
    );
}

#[test]
fn default_atmos_native_olam_mdomain_five_overrides_legacy_global_flag_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_mdomain_five");
    let namelist = root.join("mkgrd_olam_native_mdomain_five.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_mdomain_five'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%deltax=1000000.0\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=5\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=500000.0\n  NL%grdlat(2,1)=-310000.0\n  NL%grdlon(2,1)=10200000.0\n/\n",
        ),
    )
    .expect("write native atmosphere mdomain=5 namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 0, 0, None, None, None, 1, None,
    )
    .expect("mdomain=5 should use regional/cartesian native OLAM Method-C semantics");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native OLAM mdomain=5 should use OLAM direct path");
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
            "mdomain=5 should keep Fortran cartesian-x M output placeholders"
        );
        assert_eq!(
            grid.glatm[im], grid.yem[im],
            "mdomain=5 should keep Fortran cartesian-y M output placeholders"
        );
    }
    for iw in 1..=grid.nwa {
        assert_eq!(
            grid.glonw[iw], grid.xew[iw],
            "mdomain=5 should keep Fortran cartesian-x W output placeholders"
        );
        assert_eq!(
            grid.glatw[iw], grid.yew[iw],
            "mdomain=5 should keep Fortran cartesian-y W output placeholders"
        );
    }
}

#[test]
fn default_atmos_native_olam_mdomain_two_does_not_spawn_ngrids_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_native_mdomain_two_no_spawn");
    let namelist = root.join("mkgrd_olam_native_mdomain_two_no_spawn.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_native_mdomain_two_no_spawn'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%mdomain=2\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=95.0\n  NL%grdlon(2,1)=115.0\n/\n",
        ),
    )
    .expect("write native atmosphere mdomain=2 namelist");

    let err = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
        &namelist, &root, 20_000, None,
    )
    .expect_err("Fortran does not call atmosphere spawn_nest for mdomain=2");

    assert!(
        err.to_string()
            .contains("OLAM specified refine requires NL%refine=.true."),
        "mdomain=2 ngrids must not be treated as native Method-C refinement: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_out_of_range_latitude_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_bad_latitude");
    let namelist = root.join("mkgrd_olam_surface_bad_latitude.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_bad_latitude'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=95.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native surface latitude namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM surface latitude outside Fortran bounds should fail");

    assert!(
        err.to_string().contains("sfcgrdlat"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_radius_larger_than_double_earth_radius_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_large_radius");
    let namelist = root.join("mkgrd_olam_surface_large_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_large_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=13000000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write large native surface radius namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM surface radius above Fortran erad2 bound should fail");

    assert!(
        err.to_string().contains("sfcgrdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_radius_below_fortran_dzxmin() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_small_radius");
    let namelist = root.join("mkgrd_olam_surface_small_radius.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_small_radius'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=0.0005\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write small native surface radius namelist");

    let result = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native OLAM surface radius below Fortran dzxmin should fail"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("sfcgrdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_regional_nsfcgrids_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_regional_nsfc");
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
    let namelist = root.join("mkgrd_olam_surface_regional_nsfc.nml");
    let base_dir = format!("{}/", root.display());
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_regional_nsfc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_prefix}'\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native regional surface namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM surface nsfcgrids should require a global domain like Fortran");

    assert!(
        err.to_string().contains("surface") && err.to_string().contains("global domain"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_regional_inherited_atmos_ngrids_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_regional_atmos");
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
    let namelist = root.join("mkgrd_olam_surface_regional_atmos.nml");
    let base_dir = format!("{}/", root.display());
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_regional_atmos'\n  NL%base_dir='{base_dir}'\n  NL%NXP=5\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_prefix}'\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write native regional inherited-atmos surface namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM surface inherited atmosphere grids should require a global domain like Fortran");

    assert!(
        err.to_string().contains("surface") && err.to_string().contains("global domain"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_sfcgridplot_base_below_fortran_lower_bound() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_bad_sfcgridplot_base");
    let namelist = root.join("mkgrd_olam_surface_bad_sfcgridplot_base.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_bad_sfcgridplot_base'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgridplot_base=0\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write bad native surface gridplot base namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM sfcgridplot_base below Fortran lower bound should fail");

    assert!(
        err.to_string().contains("sfcgridplot_base"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_zero_coordinate_point_index_like_fortran_array() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_sfcgrdrad_zero_point_index");
    let namelist = root.join("mkgrd_olam_surface_sfcgrdrad_zero_point_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_sfcgrdrad_zero_point_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,0)=500000.0\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native surface coordinate with zero point index namelist");

    let result = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    );
    assert!(
        result.is_err(),
        "native OLAM coordinate point index 0 should fail like a Fortran array"
    );
    let err = result.unwrap_err();

    assert!(
        err.to_string().contains("sfcgrdrad"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_nsfcgrdll_index_above_fortran_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_nsfcgrdll_bad_index");
    let namelist = root.join("mkgrd_olam_surface_nsfcgrdll_bad_index.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_nsfcgrdll_bad_index'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(21)=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native surface grid point count with bad index namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM nsfcgrdll index above Fortran maxgrds should fail");

    assert!(
        err.to_string().contains("nsfcgrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_nsfcgrids_above_fortran_maxgrds() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_many_nsfcgrids");
    let namelist = root.join("mkgrd_olam_surface_many_nsfcgrids.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_many_nsfcgrids'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=21\n/\n",
        ),
    )
    .expect("write too many native surface grids namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM nsfcgrids above Fortran maxgrds should fail");

    assert!(
        err.to_string().contains("nsfcgrids"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_nsfcgrdll_above_fortran_maxngrdll() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_many_nsfcgrdll");
    let namelist = root.join("mkgrd_olam_surface_many_nsfcgrdll.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_many_nsfcgrdll'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=21\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write too many native surface grid points namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM nsfcgrdll above Fortran maxngrdll should fail");

    assert!(
        err.to_string().contains("nsfcgrdll"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_inherits_atmos_ngrids_before_nsfcgrids() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_atmos_then_sfc");
    let namelist = root.join("mkgrd_olam_surface_native_atmos_then_sfc.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_native_atmos_then_sfc'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native OLAM surface namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native OLAM surface refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native OLAM surface refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 2);
    assert!(run.regions.iter().all(|region| region.level() == 1));
    assert_eq!(run.max_level, 2);
    assert_eq!(run.spring_nest_iterations, 5000);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_olam_uses_atmos_and_surface_spring_defaults_by_stage() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_stage_spring_defaults");
    let namelist = root.join("mkgrd_olam_surface_native_stage_spring_defaults.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_native_stage_spring_defaults'\n  NL%base_dir='{base_dir}'\n  NL%NXP=9\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native staged spring namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native staged spring refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native staged spring refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 2);
    assert!(run.regions.iter().all(|region| region.level() == 1));
    assert_eq!(run.spring_nest_iterations, 5000);
    assert_eq!(run.spring_nest_passes, 2);
}

#[test]
fn default_surface_native_olam_ngrids_without_nsfcgrids_matches_atmos_spawn() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_atmos_only");
    let atmos_namelist = root.join("mkgrd_olam_atmos_native_only.nml");
    let land_namelist = root.join("mkgrd_olam_land_native_atmos_only.nml");
    let atmos_base_dir = format!("{}/atmos/", root.display());
    let land_base_dir = format!("{}/land/", root.display());
    fs::write(
        &atmos_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_atmos_native_only'\n  NL%base_dir='{atmos_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native atmosphere-only namelist");
    fs::write(
        &land_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_land_native_atmos_only'\n  NL%base_dir='{land_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native land atmosphere-only namelist");

    let atmos_report =
        earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &atmos_namelist,
            &root,
            20_000,
            0,
            None,
            None,
            None,
            1,
            None,
        )
        .expect("run native atmosphere-only atmosphere refine");
    let land_report =
        earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &land_namelist,
            &root,
            20_000,
            0,
            None,
            None,
            None,
            1,
            None,
        )
        .expect("run native atmosphere-only surface refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(
        atmos_run,
    ) = atmos_report
    else {
        panic!("native atmosphere-only atmosmesh should use OLAM direct path");
    };
    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(land_run) =
        land_report
    else {
        panic!("native atmosphere-only landmesh should use OLAM direct path");
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
fn default_surface_native_olam_sfcgrid_res_factor_expands_global_surface() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_sfcgrid_res_factor");
    let namelist = root.join("mkgrd_olam_surface_sfcgrid_res_factor.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_sfcgrid_res_factor'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native surface expansion namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native surface global expansion");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native surface global expansion should use OLAM direct path");
    };
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_iterations, 0);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_olam_sfcgrid_res_factor_does_not_require_refine_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_sfcgrid_res_no_refine");
    let namelist = root.join("mkgrd_olam_surface_sfcgrid_res_no_refine.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_sfcgrid_res_no_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native surface expansion no-refine namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native surface global expansion without refine flag");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("surface global expansion should use OLAM surface path without refine flag");
    };
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 0);
    assert_eq!(run.spring_nest_iterations, 0);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_olam_allows_sfcgrid_res_factor_four_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_sfcgrid_res_four");
    let namelist = root.join("mkgrd_olam_surface_sfcgrid_res_four.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_sfcgrid_res_four'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=4\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write native surface expansion factor-four namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("native OLAM surface expansion factor four should follow Fortran prime-factor rule");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("surface global expansion factor four should use OLAM surface path");
    };
    assert!(run.regions.is_empty());
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 0);
    assert_eq!(run.spring_nest_iterations, 0);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_olam_rejects_sfcgrid_res_factor_with_prime_factor_other_than_two_or_three_like_fortran(
) {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_bad_sfcgrid_res");
    let namelist = root.join("mkgrd_olam_surface_bad_sfcgrid_res.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_bad_sfcgrid_res'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=5\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write bad native surface expansion factor namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM surface expansion factor with other prime factors should fail");

    assert!(
        err.to_string().contains("sfcgrid_res_factor"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_rejects_zero_sfcgrid_res_factor_like_fortran() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_zero_sfcgrid_res");
    let namelist = root.join("mkgrd_olam_surface_zero_sfcgrid_res.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_zero_sfcgrid_res'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=0\n  NL%nsfcgrids=0\n/\n",
        ),
    )
    .expect("write zero sfcgrid_res_factor namelist");

    let err = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect_err("native OLAM sfcgrid_res_factor=0 should fail like Fortran");

    assert!(
        err.to_string().contains("sfcgrid_res_factor") && err.to_string().contains("positive"),
        "unexpected error: {err}"
    );
}

#[test]
fn default_surface_native_olam_nsfcgrids_does_not_require_refine_flag() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_nsfc_no_refine");
    let namelist = root.join("mkgrd_olam_surface_nsfc_no_refine.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_nsfc_no_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=2500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n",
        ),
    )
    .expect("write native surface no-refine namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run native surface nsfcgrids without refine flag");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native surface nsfcgrids should use OLAM direct path without refine flag");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_iterations, 2000);
    assert!(run.output.lbx_points > run.gridinit.gridfile.lbx_points);
}

#[test]
fn default_surface_native_olam_makegrid_plot_uses_fortran_plot_spring_iterations() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_makegrid_plot_spring");
    let namelist = root.join("mkgrd_olam_surface_makegrid_plot_spring.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_makegrid_plot_spring'\n  NL%runtype='MAKEGRID_PLOT'\n  NL%base_dir='{base_dir}'\n  NL%NXP=18\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=2500000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=2000\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write native surface MAKEGRID_PLOT namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 200_000, 0, None, None, None, 1, None,
    )
    .expect("run native surface MAKEGRID_PLOT refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("native surface MAKEGRID_PLOT refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.spring_nest_iterations, 100);
    assert!(run.spring_nest_passes > 0);
}

#[test]
fn default_surface_native_olam_expands_surface_before_nsfcgrids_spawn() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_surface_native_expand_then_nsfc");
    let expansion_only = root.join("mkgrd_olam_surface_expand_only.nml");
    let expansion_then_nest = root.join("mkgrd_olam_surface_expand_then_nsfc.nml");
    let expansion_base_dir = format!("{}/expand_only/", root.display());
    let nested_base_dir = format!("{}/expand_then_nsfc/", root.display());
    fs::write(
        &expansion_only,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_expand_only'\n  NL%base_dir='{expansion_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write expansion-only surface namelist");
    fs::write(
        &expansion_then_nest,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_surface_expand_then_nsfc'\n  NL%base_dir='{nested_base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n  NL%sfcgrid_res_factor=2\n  NL%nsfcgrids=1\n  NL%nsfcgrdll(1)=1\n  NL%sfcgrdrad(1,1)=1000000.0\n  NL%sfcgrdlat(1,1)=25.0\n  NL%sfcgrdlon(1,1)=115.0\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n",
        ),
    )
    .expect("write expansion plus surface nest namelist");

    let expansion_report =
        earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &expansion_only,
            &root,
            20_000,
            0,
            None,
            None,
            None,
            1,
            None,
        )
        .expect("run expansion-only surface path");
    let nested_report =
        earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &expansion_then_nest,
            &root,
            20_000,
            0,
            None,
            None,
            None,
            1,
            None,
        )
        .expect("run expansion then nsfcgrids surface path");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(
        expansion_run,
    ) = expansion_report
    else {
        panic!("surface expansion-only should use OLAM direct path");
    };
    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(
        nested_run,
    ) = nested_report
    else {
        panic!("surface expansion plus nsfcgrids should use OLAM direct path");
    };

    assert!(expansion_run.regions.is_empty());
    assert_eq!(nested_run.regions.len(), 1);
    assert_eq!(nested_run.max_level, 1);
    assert_eq!(nested_run.spring_nest_iterations, 2000);
    assert!(nested_run.output.lbx_points > expansion_run.output.lbx_points);
}

#[test]
fn default_land_global_specified_refine_with_landtype_masks_olam_output() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_land_global_refine_landtype");
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

    let namelist = root.join("mkgrd_olam_land_circle_refine_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_land_circle_refine_landtype'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        20_000,
        0,
        None,
        None,
        Some(1),
        1,
        None,
    )
    .expect("run default OLAM land specified refine with landtype");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default land specified refine with real landtype should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "landmesh");
    assert!(run.raw_output.is_some());
    assert!(run.landtype_masked_cells.unwrap_or_default() > 0);
    assert_eq!(
        run.output.output,
        root.join("case_olam_land_circle_refine_landtype/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM land landtype-masked topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
    let final_centers = non_placeholder_points(&refined_mesh.w_points);
    let sampled = earthmesh_cli::sample_landtype_values_for_points_fortran_indexed(
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
fn default_ocean_global_specified_refine_with_landtype_masks_olam_output() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_ocean_global_refine_landtype");
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

    let namelist = root.join("mkgrd_olam_ocean_circle_refine_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_ocean_circle_refine_landtype'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        20_000,
        0,
        None,
        None,
        Some(1),
        1,
        None,
    )
    .expect("run default OLAM ocean specified refine with landtype");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default ocean specified refine with real landtype should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "oceanmesh");
    assert!(run.raw_output.is_some());
    assert!(run.landtype_masked_cells.unwrap_or_default() > 0);
    assert_eq!(
        run.output.output,
        root.join("case_olam_ocean_circle_refine_landtype/result/gridfile_NXP0006_tri.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM ocean landtype-masked topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
    let final_centers = non_placeholder_points(&refined_mesh.m_points);
    let sampled = earthmesh_cli::sample_landtype_values_for_points_fortran_indexed(
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
    let root = temp_root("olam_loc_global_refine_landtype");
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

    let namelist = root.join("mkgrd_olam_loc_circle_refine_landtype.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let landtype_path = landtype_file.display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_loc_circle_refine_landtype'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='LOCmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype_path}'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist,
        &root,
        20_000,
        0,
        None,
        None,
        Some(1),
        1,
        None,
    )
    .expect("run default OLAM LOC specified refine with landtype");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default LOC specified refine with real landtype should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "LOCmesh");
    assert!(run.raw_output.is_some());
    assert_eq!(
        run.output.output,
        root.join("case_olam_loc_circle_refine_landtype/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());

    let coupled = run
        .coupled_outputs
        .as_ref()
        .expect("LOCmesh OLAM path should write coupled land/ocean/CoLM outputs");
    assert!(coupled.land_output.output.exists());
    assert!(coupled.ocean_output.output.exists());
    assert!(coupled.coupling_csv.exists());
    assert!(coupled.coupling_netcdf.output.exists());
    assert!(coupled.manifest.exists());
    assert!(coupled.counts.land > 0);
    assert!(coupled.counts.ocean > 0);
    assert_eq!(
        coupled.coupling_netcdf.rows,
        coupled.counts.land + coupled.counts.ocean
    );

    let land_mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&coupled.land_output.output)
        .expect("read coupled land mesh");
    let ocean_mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&coupled.ocean_output.output)
        .expect("read coupled ocean mesh");
    let land_topology = earthmesh_cli::check_unstructured_mesh_topology(&land_mesh);
    let ocean_topology = earthmesh_cli::check_unstructured_mesh_topology(&ocean_mesh);
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
    let sampled_land = earthmesh_cli::sample_landtype_values_for_points_fortran_indexed(
        &landtype_file,
        1,
        &land_centers,
    )
    .expect("sample coupled land centers");
    let sampled_ocean = earthmesh_cli::sample_landtype_values_for_points_fortran_indexed(
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

    let class_points =
        earthmesh_cli::read_colm_surface_class_points_netcdf(&coupled.coupling_netcdf.output)
            .expect("read coupled CoLM surface classes");
    assert_eq!(class_points.len(), coupled.coupling_netcdf.rows);
    assert!(class_points.iter().any(|point| point.code == 1));
    assert!(class_points.iter().any(|point| point.code == 2));

    let _ = (land_topology, ocean_topology, land_mesh, ocean_mesh, run);
}

#[test]
fn default_land_global_specified_refine_without_landtype_uses_olam_spawn_nest() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_land_global_refine");
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

    let namelist = root.join("mkgrd_olam_land_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_land_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM land specified refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default land specified refine without landtype should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "landmesh");
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(run.transition_faces > 0);
    assert_eq!(
        run.output.output,
        root.join("case_olam_land_circle_refine/result/gridfile_NXP0006_hex.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM land direct output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_ocean_global_specified_refine_without_landtype_uses_olam_spawn_nest() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_ocean_global_refine");
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

    let namelist = root.join("mkgrd_olam_ocean_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_ocean_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='FVCOM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM ocean specified refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default ocean specified refine without landtype should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "oceanmesh");
    assert_eq!(run.regions.len(), 1);
    assert_eq!(run.max_level, 1);
    assert_eq!(run.spring_nest_passes, 1);
    assert_eq!(run.spring_nest_iterations, 2);
    assert!(run.transition_faces > 0);
    assert_eq!(
        run.output.output,
        root.join("case_olam_ocean_circle_refine/result/gridfile_NXP0006_tri.nc4")
    );
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM ocean direct output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_regional_specified_refine_uses_olam_and_subsets_domain() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_regional_refine_bbox_domain");
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

    let namelist = root.join("mkgrd_olam_regional_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_circle").display().to_string();
    let domain_prefix = sources.join("domain_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_regional_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='{domain_prefix}'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='circle'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM regional specified refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default regional specified refine should use OLAM direct path");
    };
    assert_eq!(run.runtime_state.config.mask_domain_global, false);
    assert!(run.output.output.exists());

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let points = non_placeholder_points(&refined_mesh.w_points);
    assert!(!points.is_empty(), "regional OLAM output should keep cells");
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
        "regional OLAM output should be subset, got lon span {min_lon}..{max_lon}"
    );
    assert!(
        max_lat - min_lat < 80.0,
        "regional OLAM output should be subset, got lat span {min_lat}..{max_lat}"
    );

    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "regional OLAM output topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_atmos_close_specified_refine_uses_olam_polygon_region() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_atmos_close_refine");
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

    let namelist = root.join("mkgrd_olam_atmos_close_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_close").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_atmos_close_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='close'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM atmos close specified refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default close specified refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "close specified refine should add local child Voronoi cells"
    );

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM close specified refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_land_calculated_refine_uses_olam_region_source() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_land_calculated_refine");
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

    let namelist = root.join("mkgrd_olam_land_calculated_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_cal_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_land_calculated_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_cal_type='bbox'\n  RL%mask_refine_cal_fprefix='{refine_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM land calculated refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default calculated refine should use OLAM direct path");
    };
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "calculated refine should add local child Voronoi cells"
    );

    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM calculated refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_land_calculated_circle_refine_filters_degrees_by_active_max_level() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_land_calculated_circle_refine");
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

    let namelist = root.join("mkgrd_olam_land_calculated_circle_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_cal_circle").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_land_calculated_circle_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_cal_type='circle'\n  RL%mask_refine_cal_fprefix='{refine_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM land calculated circle refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default calculated circle refine should use OLAM direct path");
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
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM calculated circle refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn default_land_calculated_close_refine_promotes_zero_degree_to_max_level() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_land_calculated_close_refine");
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

    let namelist = root.join("mkgrd_olam_land_calculated_close_refine.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_cal_close").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_land_calculated_close_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_cal_type='close'\n  RL%mask_refine_cal_fprefix='{refine_prefix}'\n  RL%refine_num_landtypes=.true.\n  RL%th_num_landtypes=0\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &namelist, &root, 20_000, 0, None, None, None, 1, None,
    )
    .expect("run default OLAM land calculated close refine");

    let earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource(run) =
        report
    else {
        panic!("default calculated close refine should use OLAM direct path");
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
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read final mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM calculated close refine topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );

    let _ = (topology, refined_mesh, run);
}

#[test]
fn olam_direct_refine_uses_existing_mode_file_as_olam_source() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_refine_uses_mode_file");
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
    let gridinit =
        earthmesh_cli::run_mkgrd_gridinit_global_namelist(&gridinit_namelist, &root, 20_000)
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
    let refine_namelist = root.join("mkgrd_olam_refine_mode_file.nml");
    fs::write(
        &refine_namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_olam_refine_mode_file'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='{mode_file}'\n  NL%mode_file_description='EarthMesh'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write refine namelist");

    let run = earthmesh_cli::run_mkgrd_olam_specified_refine_global_source_namelist(
        &refine_namelist,
        &root,
        20_000,
        None,
    )
    .expect("refine existing EarthMesh mode_file through OLAM source reconstruction");
    assert!(
        run.output.sjx_points > gridinit.gridfile.sjx_points,
        "mode_file source should be refined"
    );
    assert!(
        run.output.sjx_points < 3_000,
        "direct refine ignored the NXP=6 mode_file source and rebuilt an NXP=16 source"
    );
    let refined_mesh =
        earthmesh_cli::read_unstructured_mesh_netcdf(&run.output.output).expect("read output mesh");
    let topology = earthmesh_cli::check_unstructured_mesh_topology(&refined_mesh);
    assert!(
        topology.is_consistent(),
        "OLAM mode_file refined topology violations: {:?}",
        &topology.violations[..topology.violations.len().min(8)]
    );
}

#[test]
fn legacy_atmos_specified_refine_api_routes_to_olam_direct_report() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_legacy_atmos_specified_refine_api");
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

    let namelist = root.join("mkgrd_legacy_atmos_specified_refine_api.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_legacy_atmos_specified_refine_api'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report: earthmesh_cli::MkgrdOlamSpecifiedRefineRunReport =
        earthmesh_cli::run_mkgrd_atmos_specified_refine_global_source_namelist(
            &namelist, &root, 20_000, 120, 360, 180, 1,
        )
        .expect("legacy atmosphere specified refine API should route to OLAM direct");

    assert_eq!(report.runtime_state.config.mesh_type, "atmosmesh");
    assert_eq!(report.regions.len(), 1);
    assert_eq!(report.max_level, 1);
    assert!(report.output.output.exists());
    assert!(
        report.output.lbx_points > report.gridinit.gridfile.lbx_points,
        "OLAM direct API path should add refined cells"
    );
}

#[test]
fn top_level_dispatcher_routes_refine_namelist_to_olam_direct_report() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_top_level_dispatcher_refine");
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

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 20_000, 0)
        .expect("top-level dispatcher should run refine through OLAM direct");

    let earthmesh_cli::MkgrdTopLevelDispatchRunReport::OlamRefineGlobalSource(run) = report else {
        panic!("refine namelist should dispatch to OLAM direct branch");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "atmosmesh");
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "top-level dispatcher should return refined OLAM output"
    );
}

#[test]
fn legacy_atmos_mesh_name_legacy_api_routes_to_olam_direct_report() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_legacy_atmos_meshname_api");
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

    let namelist = root.join("mkgrd_legacy_atmos_specified_refine_api_meshname.nml");
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_bbox").display().to_string();
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_legacy_atmos_meshname_specified_refine_api'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmos'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report: earthmesh_cli::MkgrdOlamSpecifiedRefineRunReport =
        earthmesh_cli::run_mkgrd_atmos_specified_refine_global_source_namelist(
            &namelist, &root, 20_000, 120, 360, 180, 1,
        )
        .expect("legacy atmosphere specified refine API should route to OLAM direct");

    assert_eq!(report.runtime_state.config.mesh_type, "atmos");
    assert_eq!(report.regions.len(), 1);
    assert_eq!(report.max_level, 1);
    assert!(report.output.output.exists());
    assert!(
        report.output.lbx_points > report.gridinit.gridfile.lbx_points,
        "OLAM direct API path should add refined cells"
    );
}

#[test]
fn top_level_dispatcher_legacy_atmos_mesh_name_routes_to_olam_direct_report() {
    let _guard = NETCDF_TEST_LOCK.lock().expect("lock netcdf test guard");
    let root = temp_root("olam_top_level_dispatcher_legacy_atmos_meshname_refine");
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
            "&mkgrd\n  NL%EXPNME='case_top_level_dispatcher_legacy_atmos_meshname_refine'\n  NL%base_dir='{base_dir}'\n  NL%NXP=6\n  NL%mesh_type='atmos'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%niter_refine=2\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%halo=4,4,3,0,0,0,0,0,0\n  RL%max_transition_row=4,4,3,0,0,0,0,0,0\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
        ),
    )
    .expect("write namelist");

    let report = earthmesh_cli::run_mkgrd_top_level_namelist(&namelist, &root, 20_000, 0)
        .expect("top-level dispatcher should run refine through OLAM direct");

    let earthmesh_cli::MkgrdTopLevelDispatchRunReport::OlamRefineGlobalSource(run) = report else {
        panic!("refine namelist should dispatch to OLAM direct branch");
    };
    assert_eq!(run.runtime_state.config.mesh_type, "atmos");
    assert_eq!(run.regions.len(), 1);
    assert!(run.output.output.exists());
    assert!(
        run.output.lbx_points > run.gridinit.gridfile.lbx_points,
        "top-level dispatcher should return refined OLAM output"
    );
}
