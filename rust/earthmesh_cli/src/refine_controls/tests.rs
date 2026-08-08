use crate::method_c_calculated_region_level;
use crate::method_c_spring_iterations;
use crate::native_initial_delaunay_mesh;
use crate::native_spawn_spring_iterations;
use crate::native_spawn_uses_cartesian_xy;
use crate::read_method_c_calculated_circle_refinement_regions;
use crate::read_method_c_circle_refinement_regions;
use crate::read_method_c_close_domain_regions;
use crate::read_method_c_close_refinement_regions;
use crate::read_method_c_domain_region;
use crate::read_method_c_specified_refinement_regions;
use crate::read_native_grid_deltax;
use crate::validate_native_spawn_mdomain;
use earthmesh_mesh::RefinementRegion;
use std::fs;

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_project::CloseBoundaryMode;

#[test]
fn method_c_uses_canonical_spring_defaults_when_niter_refine_is_unspecified() {
    let refine = RefineConfig {
        spring_global_type: 1,
        niter_refine: 100,
        niter_refine_specified: false,
        ..Default::default()
    };

    assert_eq!(
        method_c_spring_iterations(&refine, false).expect("surface iterations"),
        2000
    );
    assert_eq!(
        method_c_spring_iterations(&refine, true).expect("atmos iterations"),
        5000
    );
}

#[test]
fn method_c_respects_explicit_niter_refine_for_fast_or_custom_runs() {
    let refine = RefineConfig {
        spring_global_type: 1,
        niter_refine: 2,
        niter_refine_specified: true,
        ..Default::default()
    };

    assert_eq!(
        method_c_spring_iterations(&refine, false).expect("explicit iterations"),
        2
    );
}

#[test]
fn method_c_accepts_regional_spring_type() {
    let refine = RefineConfig {
        spring_global_type: 0,
        spring_regional_type: 1,
        niter_refine: 2,
        niter_refine_specified: true,
        ..Default::default()
    };

    assert_eq!(
        method_c_spring_iterations(&refine, false).expect("regional spring iterations"),
        2
    );
}

#[test]
fn method_c_skips_spring_when_no_spring_type_is_enabled() {
    let refine = RefineConfig {
        spring_global_type: 0,
        spring_regional_type: 0,
        niter_refine_specified: false,
        ..Default::default()
    };

    assert_eq!(
        method_c_spring_iterations(&refine, true).expect("disabled spring"),
        0
    );
}

#[test]
fn method_c_calculated_refine_level_promotes_zero_and_filters_above_active_max() {
    assert_eq!(method_c_calculated_region_level(0, 3), Some(3));
    assert_eq!(method_c_calculated_region_level(1, 3), Some(1));
    assert_eq!(method_c_calculated_region_level(3, 3), Some(3));
    assert_eq!(method_c_calculated_region_level(4, 3), None);
}

#[test]
fn native_spawn_uses_cartesian_xy_only_for_native_regional_spawn() {
    assert!(native_spawn_uses_cartesian_xy(None, false, true));
    assert!(!native_spawn_uses_cartesian_xy(None, true, true));
    assert!(!native_spawn_uses_cartesian_xy(None, false, false));
    assert!(native_spawn_uses_cartesian_xy(Some(5), true, true));
    assert!(!native_spawn_uses_cartesian_xy(Some(2), true, true));
    assert!(!native_spawn_uses_cartesian_xy(Some(4), true, true));
    assert!(!native_spawn_uses_cartesian_xy(Some(0), false, true));
}

#[test]
fn native_spawn_rejects_unsupported_explicit_mdomain() {
    validate_native_spawn_mdomain(None).expect("default mdomain");
    validate_native_spawn_mdomain(Some(0)).expect("Canonical global spawn");
    validate_native_spawn_mdomain(Some(5)).expect("Canonical cart_hex spawn");

    let err = validate_native_spawn_mdomain(Some(4))
        .expect_err("mdomain=4 uses cart4_hex, not Method-C spawn_nest");
    assert!(
        err.to_string().contains("mdomain=4"),
        "unexpected error: {err}"
    );
}

#[test]
fn native_initial_delaunay_mesh_uses_cart_hex_for_mdomain_five() {
    let mesh = native_initial_delaunay_mesh(2, Some(5), 1000.0)
        .expect("mdomain=5 selection")
        .expect("mdomain=5 uses cart_hex");

    assert_eq!(mesh.nmd, 28);
    assert_eq!(mesh.nud, 64);
    assert_eq!(mesh.nwd, 53);
    assert_eq!(mesh.m_points[2].z, 0.0);

    assert!(
        native_initial_delaunay_mesh(2, Some(0), 1000.0)
            .expect("mdomain=0 selection")
            .is_none(),
        "mdomain=0 should keep the existing global-source path"
    );
}

#[test]
fn native_grid_deltax_matches_canonical_default_and_bounds() {
    assert_eq!(
        read_native_grid_deltax("&mkgrd\n/\n").expect("default deltax"),
        1000.0
    );
    assert_eq!(
        read_native_grid_deltax("&mkgrd\n  NL%deltax=2500.0\n/\n").expect("explicit deltax"),
        2500.0
    );

    let err = read_native_grid_deltax("&mkgrd\n  NL%deltax=0.0005\n/\n")
        .expect_err("Canonical rejects deltax below dzxmin");
    assert!(
        err.to_string().contains("DELTAX"),
        "error should identify DELTAX: {err}"
    );
}

#[test]
fn native_spawn_uses_its_own_spring_iteration_policy() {
    let refine = RefineConfig {
        niter_refine: 1,
        niter_refine_specified: true,
        ..Default::default()
    };

    assert_eq!(
        native_spawn_spring_iterations(&refine, true, "MAKEGRID").expect("native atmos iterations"),
        5000
    );
    assert_eq!(
        native_spawn_spring_iterations(&refine, false, "MAKEGRID")
            .expect("native surface iterations"),
        2000
    );
}

#[test]
fn native_spawn_uses_makegrid_plot_iterations() {
    let refine = RefineConfig::default();

    assert_eq!(
        native_spawn_spring_iterations(&refine, true, "MAKEGRID_PLOT")
            .expect("MAKEGRID_PLOT atmos iterations"),
        100
    );
    assert_eq!(
        native_spawn_spring_iterations(&refine, false, "MAKEGRID_PLOT")
            .expect("MAKEGRID_PLOT surface iterations"),
        100
    );
}

#[test]
fn method_c_specified_multipoint_circle_reader_uses_canonical_corridor_with_parent_halo() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_method_c_circle_spc_{}_{}.nml",
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
    read_method_c_circle_refinement_regions(&source, &refine, 2, 16, &mut regions, true)
        .expect("read specified circle refinement regions");

    assert_eq!(regions.len(), 2);
    let RefinementRegion::Corridor {
        points,
        radius_meters,
        level,
    } = &regions[0]
    else {
        panic!("parent halo should preserve Canonical multipoint corridor");
    };
    assert_eq!(*level, 1);
    assert_eq!(points.len(), 2);
    assert_eq!(radius_meters.len(), 2);
    assert!(radius_meters.iter().all(|radius| *radius > 500_000.0));

    let RefinementRegion::Corridor {
        points,
        radius_meters,
        level,
    } = &regions[1]
    else {
        panic!("child source should preserve Canonical multipoint corridor");
    };
    assert_eq!(*level, 2);
    assert_eq!(points.len(), 2);
    assert_eq!(radius_meters, &vec![500_000.0, 500_000.0]);

    let _ = fs::remove_file(source);
}

#[test]
fn hfield_circle_reader_ignores_compatibility_parent_halo() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_hfield_circle_spc_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "circle_num = 1\ncircle_refine = 2\n115.0 25.0 500.0\n",
    )
    .expect("write circle mask source");

    let refine = RefineConfig {
        halo: [0, 4, 0, 0, 0, 0, 0, 0, 0, 0],
        max_transition_row: [0, 4, 0, 0, 0, 0, 0, 0, 0, 0],
        ..Default::default()
    };
    let mut regions = Vec::new();
    read_method_c_circle_refinement_regions(&source, &refine, 2, 16, &mut regions, false)
        .expect("read h-field circle refinement regions");

    assert_eq!(regions.len(), 1);
    let RefinementRegion::Circle {
        radius_meters,
        level,
        ..
    } = regions[0]
    else {
        panic!("h-field should use the raw target circle without parent halo");
    };
    assert_eq!(level, 2);
    assert_eq!(radius_meters, 500_000.0);

    let _ = fs::remove_file(source);
}

#[test]
fn method_c_calculated_multipoint_circle_reader_uses_canonical_corridor() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_method_c_circle_cal_{}_{}.nml",
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
    read_method_c_calculated_circle_refinement_regions(&source, 3, &mut regions)
        .expect("read calculated circle refinement regions");

    assert_eq!(regions.len(), 1);
    let RefinementRegion::Corridor {
        points,
        radius_meters,
        level,
    } = &regions[0]
    else {
        panic!("calculated multipoint circle source should produce Canonical corridor");
    };
    assert_eq!(*level, 3);
    assert_eq!(points.len(), 2);
    assert_eq!(radius_meters, &vec![500_000.0, 400_000.0]);

    let _ = fs::remove_file(source);
}

#[test]
fn method_c_close_mask_reader_repeats_first_point_for_canonical_ngrdll() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_method_c_close_mask_{}_{}.nml",
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
    read_method_c_close_refinement_regions(
        &source,
        1,
        &CloseBoundaryMode::Polyline,
        &mut regions,
        &RefineConfig::default(),
        21,
        true,
    )
    .expect("read close refinement regions");

    let RefinementRegion::Polygon { points, level } = &regions[0] else {
        panic!("close mask should produce Method-C polygon region");
    };
    assert_eq!(*level, 1);
    assert_eq!(points.len(), 5);
    assert_eq!(points.first(), points.last());

    let _ = fs::remove_file(source);
}

#[test]
fn method_c_close_mask_reader_applies_spherical_chaikin_before_polygon_membership() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_method_c_close_smooth_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "close_num = 4\nclose_refine = 1\n100.0 10.0\n102.0 10.0\n102.0 12.0\n100.0 12.0\n",
    )
    .expect("write close mask source");

    let mut regions = Vec::new();
    read_method_c_close_refinement_regions(
        &source,
        1,
        &CloseBoundaryMode::SphericalChaikin {
            iterations: 1,
            max_segment_angle_deg: 0.5,
        },
        &mut regions,
        &RefineConfig::default(),
        21,
        true,
    )
    .expect("smooth close refinement regions");

    let RefinementRegion::Polygon { points, level } = &regions[0] else {
        panic!("smooth close mask should remain an Method-C polygon");
    };
    assert_eq!(*level, 1);
    assert!(points.len() > 5);
    assert_eq!(points.first(), points.last());

    let _ = fs::remove_file(source);
}

#[test]
fn method_c_close_domain_reader_can_emit_an_enclosing_circle() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_method_c_close_cap_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "close_num = 4\nclose_refine = 0\n100.0 10.0\n102.0 10.0\n102.0 12.0\n100.0 12.0\n",
    )
    .expect("write close domain source");

    let mut regions = Vec::new();
    read_method_c_close_domain_regions(
        &source,
        &CloseBoundaryMode::EnclosingCap {
            margin_km: 10.0,
            max_radius_deg: 80.0,
            max_segment_angle_deg: 0.25,
        },
        &mut regions,
    )
    .expect("fit close domain enclosing cap");

    let crate::GridRegion::Circle {
        lon,
        lat,
        radius_km,
    } = &regions[0]
    else {
        panic!("enclosing cap should reuse GridRegion::Circle");
    };
    assert!((100.0..=102.0).contains(lon));
    assert!((10.0..=12.0).contains(lat));
    assert!(*radius_km > 10.0);

    let _ = fs::remove_file(source);
}

#[test]
fn close_boundary_engine_specs_drive_domain_and_specified_dispatch() {
    let source = std::env::temp_dir().join(format!(
        "earthmesh_cli_method_c_close_dispatch_{}_{}.nml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ));
    fs::write(
        &source,
        "close_num = 4\nclose_refine = 1\n100.0 10.0\n102.0 10.0\n102.0 12.0\n100.0 12.0\n",
    )
    .expect("write close mask source");

    let domain = EarthmeshConfig {
        mask_domain_global: false,
        mask_domain_type: "close".to_string(),
        mask_domain_fprefix: source.to_string_lossy().into_owned(),
        mask_domain_close_boundary:
            "enclosing_cap:margin_km=10,max_radius_deg=80,max_segment_angle_deg=0.25".to_string(),
        ..Default::default()
    };
    assert!(matches!(
        read_method_c_domain_region(&domain).expect("domain dispatch"),
        Some(crate::GridRegion::Circle { .. })
    ));

    let refine = RefineConfig {
        mask_refine_spc_type: "close".to_string(),
        mask_refine_spc_fprefix: source.to_string_lossy().into_owned(),
        mask_refine_spc_close_boundary: "spherical_chaikin:iterations=1,max_segment_angle_deg=0.5"
            .to_string(),
        ..Default::default()
    };
    let regions = read_method_c_specified_refinement_regions(&refine, 1, 40, false)
        .expect("specified dispatch");
    let RefinementRegion::Polygon { points, .. } = &regions[0] else {
        panic!("specified smooth close must remain polygon");
    };
    assert!(points.len() > 5);

    let _ = fs::remove_file(source);
}

#[test]
fn an_inline_circle_chain_becomes_one_region_per_member() {
    // A coastline reduced to point+radius demand arrives here as one string.
    // Each member must survive as its own region; the alternative — a chain
    // silently collapsing to one circle — would refine a fraction of the coast
    // and still report success.
    let refine = RefineConfig {
        mask_refine_spc_type: "circle".to_string(),
        mask_refine_spc_fprefix:
            "inline:circles:lon=114,lat=22,radius_km=200;lon=118,lat=24,radius_km=200".to_string(),
        ..Default::default()
    };
    let regions =
        read_method_c_specified_refinement_regions(&refine, 1, 40, false).expect("chain dispatch");
    assert_eq!(regions.len(), 2, "got {regions:?}");
    for region in &regions {
        let RefinementRegion::Circle {
            radius_meters,
            level,
            ..
        } = region
        else {
            panic!("chain members must stay circles");
        };
        assert!((radius_meters - 200_000.0).abs() < 1.0);
        assert_eq!(*level, 1);
    }
}

#[test]
fn an_inline_circle_chain_merges_members_that_coincide() {
    // Half-radius blocking makes consecutive circles overlap on purpose; the
    // dispatch already folds duplicates so the seed pass does not see the same
    // shape twice.
    let refine = RefineConfig {
        mask_refine_spc_type: "circle".to_string(),
        mask_refine_spc_fprefix:
            "inline:circles:lon=114,lat=22,radius_km=200;lon=114,lat=22,radius_km=200".to_string(),
        ..Default::default()
    };
    let regions =
        read_method_c_specified_refinement_regions(&refine, 1, 40, false).expect("chain dispatch");
    assert_eq!(regions.len(), 1, "got {regions:?}");
}

#[test]
fn a_circle_chain_is_not_a_regional_domain() {
    let domain = EarthmeshConfig {
        mask_domain_global: false,
        mask_domain_type: "circle".to_string(),
        mask_domain_fprefix: "inline:circles:lon=114,lat=22,radius_km=200".to_string(),
        ..Default::default()
    };
    let error = read_method_c_domain_region(&domain).expect_err("chain is not a domain");
    assert!(
        error.to_string().contains("refinement source"),
        "got {error}"
    );
}

#[test]
fn a_malformed_circle_chain_is_rejected() {
    // Namelists are hand-written too, so the chain parser is the only guard
    // between a typo and a region that silently refines something else.
    for (prefix, expected) in [
        ("inline:circles:", "must not be empty"),
        ("inline:circles:lon=114,lat=22", "positive radius_km"),
        (
            "inline:circles:lon=114,lat=22,radius_km=0",
            "positive radius_km",
        ),
        (
            "inline:circles:lon=114,lat=22,radius_km=inf",
            "positive radius_km",
        ),
        (
            "inline:circles:lon=114,lat=22,radius_km=abc",
            "invalid inline circle number",
        ),
        (
            "inline:circles:lon=114,lat=22,depth=3",
            "unsupported inline circle key",
        ),
    ] {
        let refine = RefineConfig {
            mask_refine_spc_type: "circle".to_string(),
            mask_refine_spc_fprefix: prefix.to_string(),
            ..Default::default()
        };
        let error =
            read_method_c_specified_refinement_regions(&refine, 1, 40, false).expect_err(prefix);
        assert!(
            error.to_string().contains(expected),
            "{prefix}: got {error}"
        );
    }
}
