use earthmesh_core::{
    deg_to_rad, rad_to_deg, EarthmeshConfig, EARTH_RADIUS_METERS, PI2, PI2_R8, PIO180, PIO180_R8,
    PIU180, PIU180_R8, PI_R8,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

#[test]
fn constants_match_fortran_consts_coms_formulas() {
    approx_eq(PIO180, std::f64::consts::PI / 180.0, 1.0e-15);
    approx_eq(PIU180, 180.0 / std::f64::consts::PI, 1.0e-12);
    approx_eq(PI2, 2.0 * std::f64::consts::PI, 1.0e-15);
    approx_eq(PIO180_R8, std::f64::consts::PI / 180.0, 1.0e-15);
    approx_eq(PIU180_R8, 180.0 / std::f64::consts::PI, 1.0e-12);
    approx_eq(PI2_R8, 2.0 * std::f64::consts::PI, 1.0e-15);
    approx_eq(PI_R8, std::f64::consts::PI, 1.0e-15);
}

#[test]
fn angle_helpers_round_trip_degrees_and_radians() {
    approx_eq(deg_to_rad(180.0), std::f64::consts::PI, 1.0e-15);
    approx_eq(rad_to_deg(std::f64::consts::PI / 2.0), 90.0, 1.0e-12);
    approx_eq(rad_to_deg(deg_to_rad(-73.25)), -73.25, 1.0e-12);
}

#[test]
fn default_config_matches_fortran_oname_vars_defaults() {
    let cfg = EarthmeshConfig::default();

    assert_eq!(cfg.experiment_name, "/tmp");
    assert_eq!(cfg.runtype, " ");
    assert_eq!(cfg.nxp, 0);
    assert_eq!(cfg.base_dir, " /tmp");
    assert_eq!(cfg.mesh_type, "/tmp");
    assert_eq!(cfg.mode_grid, "/tmp");
    assert_eq!(cfg.mode_file_description, "/tmp");
    assert_eq!(cfg.mode_file, " /tmp");
    assert!(!cfg.refine);
    assert_eq!(cfg.openmp, 16);
    assert_eq!(cfg.niter, 5000);
    assert_eq!(cfg.gridnum_perdegree, 120);
    approx_eq(cfg.mask_sea_ratio, 0.5, 0.0);
    approx_eq(cfg.beta as f64, 1.2, 1.0e-6);
    approx_eq(cfg.relax as f64, 0.04, 1.0e-7);
    assert!(!cfg.isolated_ocean);
    assert!(!cfg.mask_restart);
    assert_eq!(cfg.mask_domain_type, "/tmp");
    assert_eq!(cfg.landtype_file, "/tmp");
    assert_eq!(cfg.mask_domain_fprefix, "/tmp");
    assert!(cfg.mask_domain_global);
    assert!(!cfg.mask_patch_on);
    assert_eq!(cfg.mask_patch_type, "/tmp");
    assert_eq!(cfg.mask_patch_fprefix, "/tmp");
    assert_eq!(cfg.output_format, "/tmp");
}

#[test]
fn earth_radius_derivatives_are_initialized_from_single_radius() {
    let radii = earthmesh_core::EarthRadii::from_radius_meters(EARTH_RADIUS_METERS);

    approx_eq(radii.radius_meters, EARTH_RADIUS_METERS, 0.0);
    approx_eq(radii.double_radius_meters, EARTH_RADIUS_METERS * 2.0, 0.0);
    approx_eq(
        radii.inverse_radius_meters,
        1.0 / EARTH_RADIUS_METERS,
        1.0e-18,
    );
    approx_eq(
        radii.double_radius_squared_meters,
        (EARTH_RADIUS_METERS * 2.0).powi(2),
        1.0e-3,
    );
    approx_eq(
        radii.radius_over_sqrt_five_meters,
        EARTH_RADIUS_METERS / 5.0_f64.sqrt(),
        1.0e-9,
    );
}

#[test]
fn lonlat_mesh_defaults_match_fortran_lonlatmesh_coms() {
    let mesh = earthmesh_core::LonLatMeshConfig::default();

    assert_eq!(mesh.definition, "center");
    approx_eq(mesh.lon_start, 0.0, 0.0);
    approx_eq(mesh.lon_end, 359.0, 0.0);
    approx_eq(mesh.lon_grid_interval, 0.0625, 0.0);
    assert_eq!(mesh.lon_points, 2880);
    approx_eq(mesh.lat_start, 0.0, 0.0);
    approx_eq(mesh.lat_end, 0.0, 0.0);
    approx_eq(mesh.lat_grid_interval, 0.0, 0.0);
    assert_eq!(mesh.lat_points, 1440);
}

#[test]
fn fvcom_mesh_defaults_match_fortran_fvcommesh_coms() {
    let mesh = earthmesh_core::FvcomMeshConfig::default();

    assert_eq!(mesh.case_name, "CASENAME");
    assert_eq!(mesh.dem_file, "/tmp");
    assert_eq!(mesh.lon_name, "/tmp");
    assert_eq!(mesh.lat_name, "/tmp");
    assert_eq!(mesh.depth_name, "/tmp");
    approx_eq(mesh.min_depth, 1.0, 0.0);
    approx_eq(mesh.max_depth, 300.0, 0.0);
    approx_eq(mesh.limit_slope, 0.02, 1.0e-15);
}

#[test]
fn earthmesh_config_parses_mkgrd_namelist_assignments_like_read_nl() {
    let parsed = EarthmeshConfig::from_mkgrd_namelist(
        r#"
&mkgrd
  NL%EXPNME = 'case_a'
  NL%runtype = 'MAKEGRID_PLOT'
  NL%NXP = 64
  NL%base_dir = '/tmp/earthmesh/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'hex'
  NL%mode_file_description = 'scratch'
  NL%mode_file = '/tmp/input.nc'
  NL%refine = .true.
  NL%openmp = 8
  NL%niter = 5000
  NL%gridnum_perdegree = 240
  NL%mask_sea_ratio = 0.75
  NL%beta = 1.0
  NL%relax = 0.035
  NL%Isolated_Ocean = .true.
  NL%mask_restart = .false.
  NL%mask_domain_global = .false.
  NL%mask_domain_type = 'region'
  NL%landtype_file = '/tmp/landtype.nc'
  NL%mask_domain_fprefix = '/tmp/mask_domain'
  NL%mask_patch_on = .true.
  NL%mask_patch_type = 'patch'
  NL%mask_patch_fprefix = '/tmp/mask_patch'
  NL%output_format = 'MPAS'
/
"#,
    )
    .expect("valid mkgrd namelist");

    assert_eq!(parsed.experiment_name, "case_a");
    assert_eq!(parsed.runtype, "MAKEGRID_PLOT");
    assert_eq!(parsed.nxp, 64);
    assert_eq!(parsed.base_dir, "/tmp/earthmesh/");
    assert_eq!(parsed.file_dir(), "/tmp/earthmesh/case_a/");
    assert_eq!(parsed.mesh_type, "atmosmesh");
    assert_eq!(parsed.mode_grid, "hex");
    assert_eq!(parsed.mode_file_description, "scratch");
    assert_eq!(parsed.mode_file, "/tmp/input.nc");
    assert!(parsed.refine);
    assert_eq!(parsed.openmp, 8);
    assert_eq!(parsed.niter, 5000);
    assert_eq!(parsed.gridnum_perdegree, 240);
    approx_eq(parsed.mask_sea_ratio, 0.75, 0.0);
    approx_eq(parsed.beta as f64, 1.0, 1.0e-6);
    approx_eq(parsed.relax as f64, 0.035, 1.0e-7);
    assert!(parsed.isolated_ocean);
    assert!(!parsed.mask_restart);
    assert!(!parsed.mask_domain_global);
    assert_eq!(parsed.mask_domain_type, "region");
    assert_eq!(parsed.landtype_file, "/tmp/landtype.nc");
    assert_eq!(parsed.mask_domain_fprefix, "/tmp/mask_domain");
    assert!(parsed.mask_patch_on);
    assert_eq!(parsed.mask_patch_type, "patch");
    assert_eq!(parsed.mask_patch_fprefix, "/tmp/mask_patch");
    assert_eq!(parsed.output_format, "MPAS");
}

#[test]
fn earthmesh_config_rejects_invalid_read_nl_gridnum_perdegree() {
    let err = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%gridnum_perdegree = 60\n NL%mesh_type = 'landmesh'\n NL%output_format = 'CoLM'\n/\n",
    )
    .expect_err("gridnum_perdegree must match Fortran read_nl constraints");

    assert!(err.contains("gridnum_perdegree"));
    assert!(err.contains("120"));
    assert!(err.contains("240"));
}

#[test]
fn earthmesh_config_rejects_invalid_read_nl_mesh_output_combo() {
    let err = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%mesh_type = 'landmesh'\n NL%output_format = 'MPAS'\n/\n",
    )
    .expect_err("landmesh should only allow CoLM output like read_nl");

    assert!(err.contains("landmesh"));
    assert!(err.contains("CoLM"));
}

#[test]
fn earthmesh_config_accepts_earthmesh_colm_output_like_read_nl() {
    let parsed = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%mesh_type = 'earthmesh'\n NL%output_format = 'CoLM'\n NL%gridnum_perdegree = 120\n/\n",
    )
    .expect("earthmesh should allow CoLM output like read_nl");

    assert_eq!(parsed.mesh_type, "earthmesh");
    assert_eq!(parsed.output_format, "CoLM");
}

#[test]
fn refine_config_defaults_match_fortran_refine_vars_state_defaults() {
    let cfg = earthmesh_core::RefineConfig::default();

    assert_eq!(cfg.refine_setting, "/tmp");
    assert_eq!(cfg.mask_refine_spc_type, "/tmp");
    assert_eq!(cfg.mask_refine_spc_fprefix, "/tmp");
    assert_eq!(cfg.mask_refine_cal_type, "/tmp");
    assert_eq!(cfg.mask_refine_cal_fprefix, "/tmp");
    assert_eq!(cfg.threshold_dir, "/tmp");
    assert_eq!(cfg.set_dis_type, "/tmp");
    assert_eq!(cfg.mask_refine_ndm, [0; 10]);
    assert_eq!(cfg.max_iter, 0);
    assert_eq!(cfg.max_iter_spc, 0);
    assert_eq!(cfg.max_iter_cal, 0);
    assert_eq!(cfg.halo, [0; 10]);
    assert_eq!(cfg.max_transition_row, [0; 10]);
    assert_eq!(cfg.spring_global_type, 1);
    assert_eq!(cfg.spring_regional_type, 1);
    assert_eq!(cfg.num_rc, 0);
    assert_eq!(cfg.vertex_pretect_layers, 1);
    assert_eq!(cfg.niter_refine, 100);
    assert!(!cfg.niter_refine_specified);
    assert_eq!(cfg.th_num_landtypes, 12);
    approx_eq(cfg.th_area_mainland, 0.6, 0.0);
    assert_eq!(cfg.th_sea_ratio, [0.5, 0.5]);
    assert_eq!(cfg.th_onelayer_lnd, [999.0; 4]);
    assert_eq!(cfg.th_onelayer_ocn, [999.0; 8]);
    assert_eq!(cfg.th_onelayer_atmos, [999.0; 2]);
    assert_eq!(cfg.th_twolayer_lnd, [[999.0; 2]; 10]);
    assert!(!cfg.weak_concav_eliminate);
    assert!(!cfg.is_transition);
    assert!(!cfg.iter_d);
    assert!(!cfg.refine_spc);
    assert!(!cfg.refine_cal);
    assert!(!cfg.refine_num_landtypes);
    assert!(!cfg.refine_area_mainland);
    assert!(!cfg.refine_sea_ratio);
    assert_eq!(cfg.refine_onelayer_lnd, [false; 4]);
    assert_eq!(cfg.refine_onelayer_ocn, [false; 8]);
    assert_eq!(cfg.refine_onelayer_atmos, [false; 2]);
    assert_eq!(cfg.refine_twolayer_lnd, [false; 10]);
    assert_eq!(cfg.exit_loop_step, [false; 10]);
}

#[test]
fn refine_config_accepts_fortran_prefix_arrays_for_halo_and_transition_rows() {
    let parsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition=.true.\n RL%HALO=4,4,3\n RL%max_transition_row=5,4,3\n RL%SpringGlobal_type=1\n RL%SpringRegional_type=0\n RL%refine_spc=.true.\n RL%max_iter_spc=2\n/\n",
        "atmosmesh",
        "hex",
    )
    .expect("Fortran namelist prefix arrays should parse");

    assert_eq!(parsed.halo, [0, 4, 4, 3, 0, 0, 0, 0, 0, 0]);
    assert_eq!(parsed.max_transition_row, [0, 5, 4, 3, 0, 0, 0, 0, 0, 0]);
    assert!(!parsed.niter_refine_specified);
}

#[test]
fn refine_config_accepts_fortran_prefix_real_arrays_without_clearing_defaults() {
    let parsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition=.true.\n RL%SpringGlobal_type=1\n RL%SpringRegional_type=0\n RL%refine_spc=.true.\n RL%max_iter_spc=2\n RL%th_k_s_m=300.0\n RL%th_sea_ratio=0.2\n/\n",
        "atmosmesh",
        "hex",
    )
    .expect("Fortran namelist prefix real arrays should parse");

    assert_eq!(parsed.th_twolayer_lnd[0], [300.0, 999.0]);
    assert_eq!(parsed.th_sea_ratio, [0.2, 0.5]);
}

#[test]
fn refine_config_parses_mkrefine_namelist_and_derives_specified_setting() {
    let parsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        r#"
&mkrefine
  RL%weak_concav_eliminate = .true.
  RL%Istransition = .true.
  RL%iterD = .true.
  RL%halo = 1, 2, 3, 4, 5, 6, 7, 8, 9
  RL%max_transition_row = 9, 8, 7, 6, 5, 4, 3, 2, 1
  RL%SpringGlobal_type = 0
  RL%SpringRegional_type = 2
  RL%num_rc = 3
  RL%set_dis_type = 'nonlinear2'
  RL%vertex_pretect_layers = 4
  RL%niter_refine = 80
  RL%refine_spc = .true.
  RL%refine_cal = .false.
  RL%max_iter_spc = 2
  RL%mask_refine_spc_type = 'bbox'
  RL%mask_refine_spc_fprefix = '/tmp/refine_spc'
/
"#,
        "landmesh",
        "tri",
    )
    .expect("valid mkrefine namelist");

    assert!(parsed.weak_concav_eliminate);
    assert!(parsed.is_transition);
    assert!(parsed.iter_d);
    assert_eq!(parsed.halo, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(parsed.max_transition_row, [0, 9, 8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(parsed.spring_global_type, 0);
    assert_eq!(parsed.spring_regional_type, 2);
    assert_eq!(parsed.num_rc, 3);
    assert_eq!(parsed.set_dis_type, "nonlinear2");
    assert_eq!(parsed.vertex_pretect_layers, 4);
    assert_eq!(parsed.niter_refine, 80);
    assert!(parsed.niter_refine_specified);
    assert!(parsed.refine_spc);
    assert!(!parsed.refine_cal);
    assert_eq!(parsed.max_iter_spc, 2);
    assert_eq!(parsed.refine_setting, "specified");
    assert_eq!(parsed.mask_refine_spc_type, "bbox");
    assert_eq!(parsed.mask_refine_spc_fprefix, "/tmp/refine_spc");
}

#[test]
fn refine_config_forces_spring_types_to_zero_when_transition_disabled_for_tri() {
    let parsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition = .false.\n RL%SpringGlobal_type = 1\n RL%SpringRegional_type = 2\n RL%refine_spc = .true.\n RL%max_iter_spc = 1\n/\n",
        "landmesh",
        "tri",
    )
    .expect("tri grid can disable transition");

    assert_eq!(parsed.spring_global_type, 0);
    assert_eq!(parsed.spring_regional_type, 0);
    assert_eq!(parsed.refine_setting, "specified");
}

#[test]
fn refine_config_rejects_invalid_core_read_nl_refine_combinations() {
    let both_springs = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition = .true.\n RL%SpringGlobal_type = 1\n RL%SpringRegional_type = 1\n RL%refine_spc = .true.\n RL%max_iter_spc = 1\n/\n",
        "landmesh",
        "tri",
    )
    .expect_err("read_nl allows only one spring type larger than zero");
    assert!(both_springs.contains("only one"));

    let no_refine_mode = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition = .true.\n RL%SpringGlobal_type = 0\n RL%SpringRegional_type = 0\n RL%refine_spc = .false.\n RL%refine_cal = .false.\n/\n",
        "landmesh",
        "tri",
    )
    .expect_err("refine=true requires specified or calculated refinement");
    assert!(no_refine_mode.contains("refine_spc"));
    assert!(no_refine_mode.contains("refine_cal"));

    let atmos_cal = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition = .true.\n RL%SpringGlobal_type = 0\n RL%SpringRegional_type = 0\n RL%refine_cal = .true.\n RL%max_iter_cal = 1\n/\n",
        "atmosmesh",
        "tri",
    )
    .expect_err("atmosmesh cannot use refine_cal like read_nl");
    assert!(atmos_cal.contains("atmosmesh"));
    assert!(atmos_cal.contains("refine_cal"));
}

#[test]
fn refine_config_parses_threshold_switches_and_values_for_locmesh() {
    let parsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        r#"
&mkrefine
  RL%Istransition = .true.
  RL%SpringGlobal_type = 0
  RL%SpringRegional_type = 0
  RL%refine_cal = .true.
  RL%max_iter_cal = 3
  RL%threshold_dir = '/tmp/threshold'
  RL%mask_refine_cal_type = 'circle'
  RL%mask_refine_cal_fprefix = '/tmp/refine_cal'
  RL%refine_num_landtypes = .true.
  RL%refine_lai_m = .true.
  RL%refine_k_s_m = .true.
  RL%th_num_landtypes = 9
  RL%th_lai_m = 0.25
  RL%th_k_s_m = 1.1, 2.2
  RL%refine_sea_ratio = .true.
  RL%refine_sst_m = .true.
  RL%th_sea_ratio = 0.2, 0.8
  RL%th_sst_m = 0.4
  RL%refine_typhoon_s = .true.
  RL%th_typhoon_s = 0.6
/
"#,
        "LOCmesh",
        "tri",
    )
    .expect("LOCmesh can combine land/ocean/atmos threshold criteria");

    assert!(parsed.refine_cal);
    assert_eq!(parsed.refine_setting, "calculate");
    assert_eq!(parsed.max_iter_cal, 3);
    assert_eq!(parsed.threshold_dir, "/tmp/threshold");
    assert_eq!(parsed.mask_refine_cal_type, "circle");
    assert_eq!(parsed.mask_refine_cal_fprefix, "/tmp/refine_cal");
    assert!(parsed.refine_num_landtypes);
    assert_eq!(parsed.th_num_landtypes, 9);
    assert!(parsed.refine_onelayer_lnd[0]);
    approx_eq(parsed.th_onelayer_lnd[0], 0.25, 0.0);
    assert!(parsed.refine_twolayer_lnd[0]);
    assert_eq!(parsed.th_twolayer_lnd[0], [1.1, 2.2]);
    assert!(parsed.refine_sea_ratio);
    assert_eq!(parsed.th_sea_ratio, [0.2, 0.8]);
    assert!(parsed.refine_onelayer_ocn[0]);
    approx_eq(parsed.th_onelayer_ocn[0], 0.4, 0.0);
    assert!(parsed.refine_onelayer_atmos[1]);
    approx_eq(parsed.th_onelayer_atmos[1], 0.6, 0.0);
}

#[test]
fn refine_config_rejects_calculate_mode_without_mesh_specific_threshold_switches() {
    let err = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition = .true.\n RL%SpringGlobal_type = 0\n RL%SpringRegional_type = 0\n RL%refine_cal = .true.\n RL%max_iter_cal = 1\n/\n",
        "oceanmesh",
        "tri",
    )
    .expect_err("calculate mode needs an ocean threshold switch like read_nl");

    assert!(err.contains("refine_sea_ratio"));
    assert!(err.contains("refine_onelayer_Ocn"));
}

#[test]
fn refine_config_rejects_enabled_threshold_switch_with_missing_threshold_value() {
    let err = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition = .true.\n RL%SpringGlobal_type = 0\n RL%SpringRegional_type = 0\n RL%refine_cal = .true.\n RL%max_iter_cal = 1\n RL%refine_lai_m = .true.\n/\n",
        "landmesh",
        "tri",
    )
    .expect_err("enabled land one-layer switch needs non-999 threshold");

    assert!(err.contains("refine_onelayer_Lnd"));
    assert!(err.contains("th_onelayer_Lnd"));
}

#[test]
fn earthmesh_config_builds_non_destructive_read_nl_workspace_plan() {
    let cfg = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='case_b'\n NL%base_dir='/tmp/earthmesh/'\n NL%nxp=16\n NL%mesh_type='landmesh'\n NL%output_format='CoLM'\n NL%refine=.true.\n NL%mask_domain_global=.false.\n NL%mask_domain_type='bbox'\n NL%mask_domain_fprefix='/tmp/domain'\n NL%mask_patch_on=.true.\n NL%mask_patch_type='circle'\n NL%mask_patch_fprefix='/tmp/patch'\n/\n",
    )
    .expect("valid mkgrd namelist");
    let refine = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition=.true.\n RL%SpringGlobal_type=0\n RL%SpringRegional_type=0\n RL%refine_spc=.true.\n RL%max_iter_spc=2\n RL%mask_refine_spc_type='close'\n RL%mask_refine_spc_fprefix='/tmp/refine'\n/\n",
        "landmesh",
        "tri",
    )
    .expect("valid specified refine namelist");

    let plan = cfg.read_nl_workspace_plan(Some(&refine));

    assert_eq!(plan.file_dir, "/tmp/earthmesh/case_b/");
    assert!(plan.remove_existing_file_dir);
    assert!(plan.remove_filelists);
    assert_eq!(
        plan.namelist_save_path,
        "/tmp/earthmesh/case_b/result/namelist.save"
    );
    assert_eq!(
        plan.directories_to_create,
        vec![
            "/tmp/earthmesh/case_b/contain/",
            "/tmp/earthmesh/case_b/gridfile/",
            "/tmp/earthmesh/case_b/patchtype/",
            "/tmp/earthmesh/case_b/result/",
            "/tmp/earthmesh/case_b/tmpfile/",
            "/tmp/earthmesh/case_b/threshold/",
        ]
    );
    assert_eq!(
        plan.mask_operations,
        vec![
            earthmesh_core::MaskOperation::new("mask_domain", "bbox", "/tmp/domain"),
            earthmesh_core::MaskOperation::new("mask_patch", "circle", "/tmp/patch"),
            earthmesh_core::MaskOperation::new("mask_refine", "close", "/tmp/refine"),
        ]
    );
}

#[test]
fn earthmesh_config_workspace_plan_preserves_mask_restart_short_circuit() {
    let cfg = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='restart_case'\n NL%base_dir='/tmp/earthmesh/'\n NL%nxp=16\n NL%mesh_type='oceanmesh'\n NL%output_format='FVCOM'\n NL%mask_restart=.true.\n NL%mask_patch_on=.true.\n NL%mask_patch_type='bbox'\n NL%mask_patch_fprefix='/tmp/restart_patch'\n/\n",
    )
    .expect("valid restart namelist");

    let plan = cfg.read_nl_workspace_plan(None);

    assert_eq!(plan.file_dir, "/tmp/earthmesh/restart_case/");
    assert!(!plan.remove_existing_file_dir);
    assert!(!plan.remove_filelists);
    assert!(plan.directories_to_create.is_empty());
    assert_eq!(
        plan.namelist_save_path,
        "/tmp/earthmesh/restart_case/result/namelist.save"
    );
    assert_eq!(
        plan.mask_operations,
        vec![earthmesh_core::MaskOperation::new(
            "mask_patch",
            "bbox",
            "/tmp/restart_patch"
        )]
    );
}

#[test]
fn grid_memory_allocators_match_mem_grid_zero_initialization() {
    let mut grid = earthmesh_core::GridMemory::default();

    grid.allocate_xyzem(3);
    assert_eq!(grid.xem, vec![0.0; 3]);
    assert_eq!(grid.yem, vec![0.0; 3]);
    assert_eq!(grid.zem, vec![0.0; 3]);

    grid.allocate_xyzew(2);
    assert_eq!(grid.xew, vec![0.0; 2]);
    assert_eq!(grid.yew, vec![0.0; 2]);
    assert_eq!(grid.zew, vec![0.0; 2]);

    grid.allocate_grid_lonlatmw(3, 99, 2);
    assert_eq!(grid.glatm, vec![0.0; 3]);
    assert_eq!(grid.glonm, vec![0.0; 3]);
    assert_eq!(grid.glatw, vec![0.0; 2]);
    assert_eq!(grid.glonw, vec![0.0; 2]);
}

#[test]
fn ijtab_allocators_match_mem_ijtabs_defaults() {
    assert_eq!(earthmesh_core::MLOOPS, 7);
    assert_eq!(
        earthmesh_core::NLOOPS_M,
        earthmesh_core::MLOOPS + earthmesh_core::MAX_REMOTE
    );
    assert_eq!(earthmesh_core::JTM_VADJ, 7);
    assert_eq!(earthmesh_core::JTU_WALL, 7);

    let tabs = earthmesh_core::IjTabs::allocate(2, 1, 1);

    assert_eq!(tabs.m.len(), 2);
    assert_eq!(tabs.v.len(), 1);
    assert_eq!(tabs.w.len(), 1);
    assert_eq!(tabs.m[0].loop_flags, vec![false; earthmesh_core::MLOOPS]);
    assert_eq!(tabs.m[0].npoly, 0);
    assert_eq!(tabs.m[0].imp, 1);
    assert_eq!(tabs.m[0].imglobe, 1);
    assert_eq!(tabs.m[0].mrlm, 0);
    assert_eq!(tabs.m[0].iv, [1; 3]);
    assert_eq!(tabs.v[0].ivp, 1);
    assert_eq!(tabs.v[0].irank, -1);
    assert_eq!(tabs.v[0].im, [1; 6]);
    assert_eq!(tabs.w[0].iwp, 1);
    assert_eq!(tabs.w[0].irank, -1);
    assert_eq!(tabs.w[0].dirv, [0.0; 7]);
}

#[test]
fn delaunay_memory_allocators_match_mem_delaunay_defaults() {
    let mut memory = earthmesh_core::DelaunayMemory::default();

    memory.allocate_itabsd(2, 1, 1);

    assert_eq!(memory.md.len(), 2);
    assert_eq!(memory.ud.len(), 1);
    assert_eq!(memory.wd.len(), 1);
    assert_eq!(memory.xemd, vec![0.0; 2]);
    assert_eq!(memory.yemd, vec![0.0; 2]);
    assert_eq!(memory.zemd, vec![0.0; 2]);
    assert_eq!(memory.md[0].loop_flags, [false; earthmesh_core::MLOOPS]);
    assert_eq!(memory.md[0].npoly, 0);
    assert_eq!(memory.md[0].imp, 1);
    assert_eq!(memory.md[0].mrlm, 0);
    assert_eq!(memory.md[0].mrlm_orig, 0);
    assert_eq!(memory.md[0].ngr, 0);
    assert_eq!(memory.md[0].im, [1; 7]);
    assert_eq!(memory.md[0].iu, [1; 7]);
    assert_eq!(memory.md[0].iw, [1; 7]);
    assert_eq!(memory.ud[0].loop_flags, [false; earthmesh_core::MLOOPS]);
    assert_eq!(memory.ud[0].iup, 1);
    assert_eq!(memory.ud[0].mrlu, 0);
    assert_eq!(memory.ud[0].im, [1; 2]);
    assert_eq!(memory.ud[0].iu, [1; 12]);
    assert_eq!(memory.ud[0].iw, [1; 6]);
    assert_eq!(memory.wd[0].loop_flags, [false; earthmesh_core::MLOOPS]);
    assert_eq!(memory.wd[0].iwp, 1);
    assert_eq!(memory.wd[0].mrlw, 0);
    assert_eq!(memory.wd[0].mrlw_orig, 0);
    assert_eq!(memory.wd[0].mrow, 0);
    assert_eq!(memory.wd[0].ngr, 0);
    assert_eq!(memory.wd[0].im, [1; 3]);
    assert_eq!(memory.wd[0].iu, [1; 3]);
    assert_eq!(memory.wd[0].iw, [1; 9]);
}

#[test]
fn delaunay_memory_copy_and_original_buffers_match_fortran_initial_state() {
    let memory = earthmesh_core::DelaunayMemory::default();

    assert_eq!(memory.nmd_copy, 0);
    assert_eq!(memory.nud_copy, 0);
    assert_eq!(memory.nwd_copy, 0);
    assert!(memory.md_copy.is_empty());
    assert!(memory.ud_copy.is_empty());
    assert!(memory.wd_copy.is_empty());
    assert!(memory.xemd_copy.is_empty());
    assert!(memory.yemd_copy.is_empty());
    assert!(memory.zemd_copy.is_empty());
    assert!(memory.iwdorig.is_empty());
    assert!(memory.iwdorig_temp.is_empty());
}

#[test]
fn runtime_state_wires_configs_and_legacy_memories_without_fortran_globals() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='state_case'\n NL%nxp=42\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%output_format='CoLM'\n NL%refine=.true.\n/\n",
    )
    .expect("valid mkgrd config");
    let refine = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%refine_spc=.true.\n RL%max_iter_spc=1\n/\n",
        &config.mesh_type,
        &config.mode_grid,
    )
    .expect("valid refine config");

    let mut state = earthmesh_core::EarthmeshRuntimeState::new(config.clone())
        .with_refine_config(refine.clone());
    state.allocate_legacy_memories(earthmesh_core::LegacyMemoryShape {
        nma: 3,
        nua: 4,
        nva: 5,
        nwa: 6,
        mma: 7,
        mua: 8,
        mva: 9,
        mwa: 10,
    });

    assert_eq!(state.config.experiment_name, "state_case");
    assert_eq!(state.refine.as_ref().unwrap().refine_setting, "specified");
    assert_eq!(state.step, 1);
    assert_eq!(state.nxp(), 42);
    assert_eq!(state.num_mp_step, [1; 10]);
    assert_eq!(state.num_wp_step, [1; 10]);
    assert_eq!(state.grid.nma, 3);
    assert_eq!(state.grid.nua, 4);
    assert_eq!(state.grid.nva, 5);
    assert_eq!(state.grid.nwa, 6);
    assert_eq!(state.grid.xem, vec![0.0; 7]);
    assert_eq!(state.grid.xew, vec![0.0; 10]);
    assert_eq!(state.ijtabs.m.len(), 7);
    assert_eq!(state.ijtabs.v.len(), 9);
    assert_eq!(state.ijtabs.w.len(), 10);
    assert_eq!(state.delaunay.md.len(), 7);
    assert_eq!(state.delaunay.ud.len(), 8);
    assert_eq!(state.delaunay.wd.len(), 10);
    approx_eq(
        state.radii.double_radius_meters,
        earthmesh_core::EARTH_RADIUS_METERS * 2.0,
        0.0,
    );
}

#[test]
fn runtime_state_try_nxp_rejects_uninitialized_or_negative_nxp() {
    let zero = earthmesh_core::EarthmeshRuntimeState::new(EarthmeshConfig::default());
    assert!(zero.try_nxp().is_err());

    let mut negative_config = EarthmeshConfig::default();
    negative_config.nxp = -1;
    let negative = earthmesh_core::EarthmeshRuntimeState::new(negative_config);
    assert!(negative.try_nxp().is_err());
}

#[test]
fn runtime_state_records_real_mesh_counts_for_fortran_steps() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='count_case'\n NL%nxp=4\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%output_format='CoLM'\n/\n",
    )
    .expect("valid mkgrd config");
    let mut state = earthmesh_core::EarthmeshRuntimeState::new(config);

    state
        .record_mesh_counts_for_step(1, 21, 13)
        .expect("record first Fortran step");

    assert_eq!(state.step, 1);
    assert_eq!(state.num_mp_step[0], 21);
    assert_eq!(state.num_wp_step[0], 13);
    assert!(state.record_mesh_counts_for_step(0, 1, 1).is_err());
    assert!(state.record_mesh_counts_for_step(11, 1, 1).is_err());
}

#[test]
fn runtime_state_records_legacy_num_vertex_boundary() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='num_vertex_case'\n NL%nxp=4\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='oceanmesh'\n NL%mode_grid='hex'\n NL%output_format='FVCOM'\n/\n",
    )
    .expect("valid mkgrd config");
    let mut state = earthmesh_core::EarthmeshRuntimeState::new(config);

    assert_eq!(state.num_vertex, 0);
    state
        .record_num_vertex(6)
        .expect("record legacy num_vertex boundary");

    assert_eq!(state.num_vertex, 6);
    assert!(state.record_num_vertex(0).is_err());
}

#[test]
fn runtime_state_records_legacy_scalar_defaults_from_consts_coms_and_mkgrd() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='scalar_case'\n NL%nxp=4\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%output_format='CoLM'\n/\n",
    )
    .expect("valid mkgrd config");
    let state = earthmesh_core::EarthmeshRuntimeState::new(config);

    assert_eq!(state.scalars.rinit, 0.0);
    assert_eq!(state.scalars.rinit8, 0.0);
    assert_eq!(state.scalars.iunit, 10);
    assert_eq!(state.scalars.io6, 6);
    assert_eq!(state.scalars.num_center, 1);
}

#[test]
fn runtime_state_derives_num_center_from_previous_step_wp_count() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='num_center_case'\n NL%nxp=4\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%output_format='CoLM'\n/\n",
    )
    .expect("valid mkgrd config");
    let mut state = earthmesh_core::EarthmeshRuntimeState::new(config);
    state
        .record_mesh_counts_for_step(1, 21, 13)
        .expect("record previous step W count");

    state
        .record_num_center_from_previous_step(2)
        .expect("record Get_Contain num_center handoff");

    assert_eq!(state.step, 2);
    assert_eq!(state.scalars.num_center, 13);
    assert!(state.record_num_center_from_previous_step(0).is_err());
    assert!(state.record_num_center_from_previous_step(11).is_err());
}

#[test]
fn runtime_state_records_legacy_impent_pentagon_indices() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='impent_case'\n NL%nxp=4\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%output_format='CoLM'\n/\n",
    )
    .expect("valid mkgrd config");
    let mut state = earthmesh_core::EarthmeshRuntimeState::new(config);

    assert_eq!(state.pentagon_indices, [0; 12]);
    state
        .record_pentagon_indices_from_icosahedron([2, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610])
        .expect("record icosahedron impent handoff");

    assert_eq!(state.pentagon_indices[0], 2);
    assert_eq!(state.pentagon_indices[11], 610);
    assert!(state
        .record_pentagon_indices_from_icosahedron([0; 12])
        .is_err());
}

#[test]
fn runtime_state_records_data_preprocess_source_grid_globals() {
    let config = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd\n NL%expnme='source_grid_case'\n NL%nxp=4\n NL%base_dir='/tmp/earthmesh/'\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%gridnum_perdegree=120\n NL%output_format='CoLM'\n/\n",
    )
    .expect("valid mkgrd config");
    let mut state = earthmesh_core::EarthmeshRuntimeState::new(config);

    assert_eq!(state.source_grid.nlons_source, 0);
    assert_eq!(state.source_grid.nlats_source, 0);
    assert_eq!(state.source_grid.maxlc, 0);

    state
        .record_data_preprocess_source_grid(43)
        .expect("record data_preprocess global source-grid dimensions and max land class");

    assert_eq!(state.source_grid.nlons_source, 43_200);
    assert_eq!(state.source_grid.nlats_source, 21_600);
    assert_eq!(state.source_grid.maxlc, 43);

    let mut invalid = state.clone();
    invalid.config.gridnum_perdegree = 0;
    assert!(invalid.record_data_preprocess_source_grid(1).is_err());
}
