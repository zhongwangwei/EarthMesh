use earthmesh_core::{deg_to_rad, rad_to_deg, EarthmeshConfig, EARTH_RADIUS_METERS, PI2, PIO180, PIU180};

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
    approx_eq(radii.inverse_radius_meters, 1.0 / EARTH_RADIUS_METERS, 1.0e-18);
    approx_eq(radii.double_radius_squared_meters, (EARTH_RADIUS_METERS * 2.0).powi(2), 1.0e-3);
    approx_eq(radii.radius_over_sqrt_five_meters, EARTH_RADIUS_METERS / 5.0_f64.sqrt(), 1.0e-9);
}
