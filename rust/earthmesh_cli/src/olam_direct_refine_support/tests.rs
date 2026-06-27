use std::fs;

use earthmesh_core::RefineConfig;

use crate::*;

#[test]
fn olam_method_c_uses_fortran_spring_defaults_when_niter_refine_is_unspecified() {
    let refine = RefineConfig {
        spring_global_type: 1,
        niter_refine: 100,
        niter_refine_specified: false,
        ..Default::default()
    };

    assert_eq!(
        olam_method_c_spring_iterations(&refine, false).expect("surface iterations"),
        2000
    );
    assert_eq!(
        olam_method_c_spring_iterations(&refine, true).expect("atmos iterations"),
        5000
    );
}

#[test]
fn olam_method_c_respects_explicit_niter_refine_for_fast_or_custom_runs() {
    let refine = RefineConfig {
        spring_global_type: 1,
        niter_refine: 2,
        niter_refine_specified: true,
        ..Default::default()
    };

    assert_eq!(
        olam_method_c_spring_iterations(&refine, false).expect("explicit iterations"),
        2
    );
}

#[test]
fn olam_method_c_skips_spring_when_global_spring_is_disabled() {
    let refine = RefineConfig {
        spring_global_type: 0,
        niter_refine_specified: false,
        ..Default::default()
    };

    assert_eq!(
        olam_method_c_spring_iterations(&refine, true).expect("disabled spring"),
        0
    );
}

#[test]
fn olam_calculated_refine_level_promotes_zero_and_filters_above_active_max() {
    assert_eq!(olam_calculated_region_level(0, 3), Some(3));
    assert_eq!(olam_calculated_region_level(1, 3), Some(1));
    assert_eq!(olam_calculated_region_level(3, 3), Some(3));
    assert_eq!(olam_calculated_region_level(4, 3), None);
}

#[test]
fn olam_native_method_c_uses_cartesian_xy_only_for_native_regional_spawn() {
    assert!(olam_native_method_c_uses_cartesian_xy(None, false, true));
    assert!(!olam_native_method_c_uses_cartesian_xy(None, true, true));
    assert!(!olam_native_method_c_uses_cartesian_xy(None, false, false));
    assert!(olam_native_method_c_uses_cartesian_xy(Some(5), true, true));
    assert!(!olam_native_method_c_uses_cartesian_xy(Some(2), true, true));
    assert!(!olam_native_method_c_uses_cartesian_xy(Some(4), true, true));
    assert!(!olam_native_method_c_uses_cartesian_xy(
        Some(0),
        false,
        true
    ));
}

#[test]
fn olam_native_method_c_rejects_unported_explicit_mdomain_spawn() {
    validate_olam_native_method_c_spawn_mdomain(None).expect("default mdomain");
    validate_olam_native_method_c_spawn_mdomain(Some(0)).expect("Fortran global spawn");
    validate_olam_native_method_c_spawn_mdomain(Some(5)).expect("Fortran cart_hex spawn");

    let err = validate_olam_native_method_c_spawn_mdomain(Some(4))
        .expect_err("mdomain=4 uses cart4_hex, not Method-C spawn_nest");
    assert!(
        err.to_string().contains("mdomain=4"),
        "unexpected error: {err}"
    );
}

#[test]
fn olam_native_initial_delaunay_mesh_uses_cart_hex_for_mdomain_five() {
    let mesh = olam_native_initial_delaunay_mesh(2, Some(5), 1000.0)
        .expect("mdomain=5 selection")
        .expect("mdomain=5 uses cart_hex");

    assert_eq!(mesh.nmd, 28);
    assert_eq!(mesh.nud, 64);
    assert_eq!(mesh.nwd, 53);
    assert_eq!(mesh.m_points[2].z, 0.0);

    assert!(
        olam_native_initial_delaunay_mesh(2, Some(0), 1000.0)
            .expect("mdomain=0 selection")
            .is_none(),
        "mdomain=0 should keep the existing global-source path"
    );
}

#[test]
fn olam_native_deltax_matches_fortran_default_and_bounds() {
    assert_eq!(
        read_olam_native_deltax("&mkgrd\n/\n").expect("default deltax"),
        1000.0
    );
    assert_eq!(
        read_olam_native_deltax("&mkgrd\n  NL%deltax=2500.0\n/\n").expect("explicit deltax"),
        2500.0
    );

    let err = read_olam_native_deltax("&mkgrd\n  NL%deltax=0.0005\n/\n")
        .expect_err("Fortran rejects deltax below dzxmin");
    assert!(
        err.to_string().contains("DELTAX"),
        "error should identify DELTAX: {err}"
    );
}

#[test]
fn olam_native_method_c_ignores_mkrefine_niter_refine_like_fortran_spawn_nest() {
    let refine = RefineConfig {
        niter_refine: 1,
        niter_refine_specified: true,
        ..Default::default()
    };

    assert_eq!(
        olam_native_method_c_spring_iterations(&refine, true, "MAKEGRID")
            .expect("native atmos iterations"),
        5000
    );
    assert_eq!(
        olam_native_method_c_spring_iterations(&refine, false, "MAKEGRID")
            .expect("native surface iterations"),
        2000
    );
}

#[test]
fn olam_native_method_c_uses_makegrid_plot_iterations_like_fortran_spawn_nest() {
    let refine = RefineConfig::default();

    assert_eq!(
        olam_native_method_c_spring_iterations(&refine, true, "MAKEGRID_PLOT")
            .expect("MAKEGRID_PLOT atmos iterations"),
        100
    );
    assert_eq!(
        olam_native_method_c_spring_iterations(&refine, false, "MAKEGRID_PLOT")
            .expect("MAKEGRID_PLOT surface iterations"),
        100
    );
}

#[test]
fn olam_specified_multipoint_circle_reader_uses_fortran_corridor_with_parent_halo() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_olam_circle_spc_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "circle_num = 2\ncircle_refine = 2\n115.0 25.0 500.0\n90.0 25.0 500.0\n",
    )
    .expect("write circle mask source");

    let refine = RefineConfig {
        halo: [0, 4, 0, 0, 0, 0, 0, 0, 0, 0],
        max_transition_row: [0, 4, 0, 0, 0, 0, 0, 0, 0, 0],
        ..Default::default()
    };
    let mut regions = Vec::new();
    read_olam_circle_refinement_regions(&source, &refine, 2, 16, &mut regions)
        .expect("read specified circle refinement regions");

    assert_eq!(regions.len(), 2);
    let OlamRefinementRegion::Corridor {
        points,
        radius_meters,
        level,
    } = &regions[0]
    else {
        panic!("parent halo should preserve Fortran multipoint corridor");
    };
    assert_eq!(*level, 1);
    assert_eq!(points.len(), 2);
    assert_eq!(radius_meters.len(), 2);
    assert!(radius_meters.iter().all(|radius| *radius > 500_000.0));

    let OlamRefinementRegion::Corridor {
        points,
        radius_meters,
        level,
    } = &regions[1]
    else {
        panic!("child source should preserve Fortran multipoint corridor");
    };
    assert_eq!(*level, 2);
    assert_eq!(points.len(), 2);
    assert_eq!(radius_meters, &vec![500_000.0, 500_000.0]);

    let _ = fs::remove_file(source);
}

#[test]
fn olam_calculated_multipoint_circle_reader_uses_fortran_corridor() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_olam_circle_cal_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "circle_num = 2\ncircle_refine = 0\n115.0 25.0 500.0\n90.0 25.0 400.0\n",
    )
    .expect("write calculated circle mask source");

    let mut regions = Vec::new();
    read_olam_calculated_circle_refinement_regions(&source, 3, &mut regions)
        .expect("read calculated circle refinement regions");

    assert_eq!(regions.len(), 1);
    let OlamRefinementRegion::Corridor {
        points,
        radius_meters,
        level,
    } = &regions[0]
    else {
        panic!("calculated multipoint circle source should produce Fortran corridor");
    };
    assert_eq!(*level, 3);
    assert_eq!(points.len(), 2);
    assert_eq!(radius_meters, &vec![500_000.0, 400_000.0]);

    let _ = fs::remove_file(source);
}

#[test]
fn olam_close_mask_reader_repeats_first_point_for_fortran_ngrdll() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_olam_close_mask_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "close_num = 4\nclose_refine = 1\n100.0 15.0\n130.0 15.0\n130.0 35.0\n100.0 35.0\n",
    )
    .expect("write close mask source");

    let mut regions = Vec::new();
    read_olam_close_refinement_regions(&source, 1, &mut regions)
        .expect("read close refinement regions");

    let OlamRefinementRegion::Polygon { points, level } = &regions[0] else {
        panic!("close mask should produce OLAM polygon region");
    };
    assert_eq!(*level, 1);
    assert_eq!(points.len(), 5);
    assert_eq!(points.first(), points.last());

    let _ = fs::remove_file(source);
}
