use earthmesh_core::{
    deg_to_rad, rad_to_deg, EarthmeshConfig, EARTH_RADIUS_METERS, PI2, PIO180, PIU180,
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
