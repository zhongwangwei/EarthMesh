use super::*;
use crate::hfield_refine::{build_composed_hfield_with_report, HfieldRefineOptions};
use crate::refinement_demand::plan::{
    plan_demand_at_scale, plan_demand_at_scale_for_windows, DemandPlanInputs,
};
use crate::refinement_demand::source_bounds_for_bbox;
use crate::GridRegion;
use earthmesh_core::{EarthmeshConfig, RefineConfig, EARTH_RADIUS_METERS};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "earthmesh_threshold_support_{name}_{}_{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn parent_m_for_nlat(nlat: usize) -> f64 {
    std::f64::consts::PI * EARTH_RADIUS_METERS / nlat as f64
}

fn write_numeric(
    path: &Path,
    name: &str,
    nlon: usize,
    nlat: usize,
    value: impl Fn(usize, usize) -> f64,
) {
    let mut file = crate::create_netcdf(path).expect("create numeric file");
    file.add_dimension("longitude", nlon).expect("lon dim");
    file.add_dimension("latitude", nlat).expect("lat dim");
    let mut values = Vec::with_capacity(nlon * nlat);
    for i in 0..nlon {
        for j in 0..nlat {
            values.push(value(i, j));
        }
    }
    file.add_variable::<f64>(name, &["longitude", "latitude"])
        .expect("numeric variable")
        .put_values(&values, (.., ..))
        .expect("write numeric");
}

fn write_landtype(path: &Path, nlon: usize, nlat: usize, value: impl Fn(usize, usize) -> i8) {
    let mut file = crate::create_netcdf(path).expect("create landtype file");
    file.add_dimension("longitude", nlon).expect("lon dim");
    file.add_dimension("latitude", nlat).expect("lat dim");
    let mut values = Vec::with_capacity(nlon * nlat);
    for i in 0..nlon {
        for j in 0..nlat {
            values.push(value(i, j));
        }
    }
    file.add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype variable")
        .put_values(&values, (.., ..))
        .expect("write landtype");
}

fn criterion<'a>(raw: &'a ThresholdSupportDemand, id: &str) -> &'a CriterionSupportDemand {
    raw.criteria
        .iter()
        .find(|criterion| criterion.id == id)
        .unwrap_or_else(|| {
            panic!(
                "missing criterion {id}; got {:?}",
                raw.criteria.iter().map(|c| &c.id).collect::<Vec<_>>()
            )
        })
}

fn support_index_for_source(
    src_i: usize,
    src_j: usize,
    src_nlon: usize,
    src_nlat: usize,
    support_nlon: usize,
    support_nlat: usize,
) -> usize {
    let lon = -180.0 + (src_i as f64 + 0.5) * 360.0 / src_nlon as f64;
    let shifted = wrap_lon(lon + 180.0 / support_nlon as f64);
    let i = (((shifted + 180.0) / 360.0) * support_nlon as f64)
        .floor()
        .clamp(0.0, (support_nlon - 1) as f64) as usize;
    let lat = 90.0 - (src_j as f64 + 0.5) * 180.0 / src_nlat as f64;
    let j = (((lat + 90.0) / 180.0) * support_nlat as f64)
        .floor()
        .clamp(0.0, (support_nlat - 1) as f64) as usize;
    i * support_nlat + j
}

fn wrap_lon(lon: f64) -> f64 {
    let mut wrapped = (lon + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped == -180.0 && lon > 0.0 {
        wrapped = 180.0;
    }
    wrapped
}

fn support0_domain() -> GridRegion {
    GridRegion::Bbox {
        west: -180.1,
        east: -179.9,
        south: -67.6,
        north: -67.4,
    }
}

#[test]
fn cap_uses_the_configured_threshold_depth_not_adapter_outer_depth() {
    let mut refine = RefineConfig {
        max_iter_cal: 2,
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;

    assert_eq!(threshold_level_cap(&refine, "landmesh", 1).unwrap(), 1);
    assert_eq!(threshold_level_cap(&refine, "landmesh", 5).unwrap(), 2);

    refine.max_iter_cal = 0;
    assert!(threshold_level_cap(&refine, "landmesh", 5).is_err());
}

#[test]
fn enabled_thresholds_reject_impossible_support_counts_before_source_io() {
    let mut refine = RefineConfig {
        max_iter_cal: 5,
        threshold_dir: "/definitely/not/read".into(),
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;
    let err = match evaluate_threshold_support(&refine, "landmesh", None, 1.0, None) {
        Ok(_) => panic!("tiny parent scale must trip the support cap before opening sources"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("finer than the support lattice"),
        "{err}"
    );
}

#[test]
fn numeric_catalogue_fields_share_support_semantics_across_mesh_aliases() {
    let root = temp_root("numeric_aliases");
    for name in ["lai", "sst", "typhoon"] {
        write_numeric(&root.join(format!("{name}.nc")), name, 16, 8, |i, j| {
            if support_index_for_source(i, j, 16, 8, 8, 4) == 0 && (i + j) % 2 == 0 {
                10.0
            } else {
                0.0
            }
        });
    }

    for (name, configure) in [
        ("lai", configure_lai_thresholds as fn(&mut RefineConfig)),
        ("sst", configure_sst_thresholds as fn(&mut RefineConfig)),
        (
            "typhoon",
            configure_typhoon_thresholds as fn(&mut RefineConfig),
        ),
    ] {
        let mut baseline: Option<Vec<(String, usize, usize, usize)>> = None;
        for mesh_type in [
            "landmesh",
            "oceanmesh",
            "atmos",
            "atmosmesh",
            "LOCmesh",
            "earthmesh",
        ] {
            let mut refine = RefineConfig {
                max_iter_cal: 1,
                threshold_dir: root.display().to_string(),
                ..RefineConfig::default()
            };
            configure(&mut refine);
            let raw =
                evaluate_threshold_support(&refine, mesh_type, None, parent_m_for_nlat(4), None)
                    .unwrap_or_else(|err| panic!("{name} on {mesh_type}: {err}"));
            let got = raw
                .criteria
                .iter()
                .map(|criterion| {
                    (
                        criterion.id.clone(),
                        criterion.hits.iter().filter(|&&hit| hit).count(),
                        criterion.source_samples,
                        criterion.empty_supports,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                got.iter().map(|row| row.0.clone()).collect::<Vec<_>>(),
                vec![format!("{name}_mean"), format!("{name}_std")]
            );
            assert!(got
                .iter()
                .all(|(_, hits, samples, empty)| *hits > 0 && *samples == 16 * 8 && *empty == 0));
            if let Some(baseline) = &baseline {
                assert_eq!(&got, baseline, "{name} differs on {mesh_type}");
            } else {
                baseline = Some(got);
            }
        }
    }
    let _ = fs::remove_dir_all(root);
}

fn configure_lai_thresholds(refine: &mut RefineConfig) {
    refine.refine_onelayer_lnd[0] = true;
    refine.refine_onelayer_lnd[1] = true;
    refine.th_onelayer_lnd[0] = 1.0;
    refine.th_onelayer_lnd[1] = 1.0;
}

fn configure_sst_thresholds(refine: &mut RefineConfig) {
    refine.refine_onelayer_ocn[0] = true;
    refine.refine_onelayer_ocn[1] = true;
    refine.th_onelayer_ocn[0] = 1.0;
    refine.th_onelayer_ocn[1] = 1.0;
}

fn configure_typhoon_thresholds(refine: &mut RefineConfig) {
    refine.refine_onelayer_atmos[0] = true;
    refine.refine_onelayer_atmos[1] = true;
    refine.th_onelayer_atmos[0] = 1.0;
    refine.th_onelayer_atmos[1] = 1.0;
}

#[test]
fn mean_uses_support_average_not_the_center_source_value() {
    let root = temp_root("mean_support");
    let lai = root.join("lai.nc");
    write_numeric(&lai, "lai", 16, 8, |i, j| {
        if support_index_for_source(i, j, 16, 8, 8, 4) == 0 {
            if i % 2 == 0 {
                0.0
            } else {
                20.0
            }
        } else {
            0.0
        }
    });
    let mut refine = RefineConfig {
        max_iter_cal: 1,
        threshold_dir: root.display().to_string(),
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;
    refine.th_onelayer_lnd[0] = 9.9;

    let raw = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(4), None)
        .expect("support demand");

    let mean = criterion(&raw, "lai_mean");
    assert!(mean.hits[0], "support 0 has hand-enumerated mean 10");
    assert_eq!(mean.source_samples, 16 * 8);
    refine.th_onelayer_lnd[0] = 10.1;
    let raw = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(4), None)
        .expect("support demand above known mean");
    assert!(!criterion(&raw, "lai_mean").hits[0]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn std_levels_are_independent_and_union_keeps_coarse_and_fine_hits() {
    let root = temp_root("std_levels");
    let lai = root.join("lai.nc");
    // North: 0/100 latitude halves give coarse std=50, fine std=0.
    // South: eight hot samples give coarse std≈33, fine std=50.
    write_numeric(&lai, "lai", 32, 16, |i, j| {
        if (4..8).contains(&j) || ((14..16).contains(&i) && (8..12).contains(&j)) {
            100.0
        } else {
            0.0
        }
    });
    let mut refine = RefineConfig {
        refine_cal: true,
        max_iter_cal: 2,
        threshold_dir: root.display().to_string(),
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[1] = true;
    refine.th_onelayer_lnd[1] = 45.0;

    let coarse = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(2), None)
        .expect("coarse support");
    let fine = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(4), None)
        .expect("fine support");

    assert!(criterion(&coarse, "lai_std").hits[1]);
    assert!(!criterion(&coarse, "lai_std").hits[4]);
    assert!(criterion(&fine, "lai_std").hits[17]);
    assert!(criterion(&fine, "lai_std")
        .hits
        .iter()
        .enumerate()
        .all(|(i, &hit)| i % 4 < 2 || !hit));

    let options = HfieldRefineOptions {
        g: 10.0,
        nlon: 8,
        nlat: 4,
        base_m: Some(parent_m_for_nlat(2)),
        max_level: Some(2),
        ..HfieldRefineOptions::default()
    };
    let (field, _report) = build_composed_hfield_with_report(
        &[],
        &refine,
        "landmesh",
        None,
        parent_m_for_nlat(2),
        &options,
        2,
        None,
    )
    .expect("hfield composition");
    let base = parent_m_for_nlat(2);
    assert!(field
        .values()
        .iter()
        .any(|&value| (value - base / 2.0).abs() < 1e-6));
    assert!(field
        .values()
        .iter()
        .any(|&value| (value - base / 4.0).abs() < 1e-6));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn fine_only_std_can_trigger_below_a_quiet_parent_support() {
    let root = temp_root("fine_only_std");
    let lai = root.join("lai.nc");
    write_numeric(&lai, "lai", 16, 8, |i, _j| if i == 4 { 100.0 } else { 0.0 });
    let mut refine = RefineConfig {
        max_iter_cal: 2,
        threshold_dir: root.display().to_string(),
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[1] = true;
    refine.th_onelayer_lnd[1] = 45.0;

    let coarse = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(2), None)
        .expect("coarse support");
    let fine = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(4), None)
        .expect("fine support");

    assert!(!criterion(&coarse, "lai_std").hits.iter().any(|&hit| hit));
    assert!(criterion(&fine, "lai_std").hits.iter().any(|&hit| hit));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn negative_mean_hits_nonempty_supports_but_empty_supports_do_not_trigger() {
    let root = temp_root("negative_empty");
    let lai = root.join("lai.nc");
    write_numeric(&lai, "lai", 8, 4, |_i, _j| -5.0);
    let mut refine = RefineConfig {
        max_iter_cal: 1,
        threshold_dir: root.display().to_string(),
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;
    refine.th_onelayer_lnd[0] = -10.0;

    let raw = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(8), None)
        .expect("support demand");
    let lai = criterion(&raw, "lai_mean");

    assert!(lai.empty_supports > 0);
    assert_eq!(lai.source_samples, 8 * 4);
    assert_eq!(
        lai.hits.iter().filter(|&&hit| hit).count(),
        lai.source_samples
    );
    assert!(lai.hits.iter().filter(|&&hit| hit).count() < raw.eligible_supports);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn maxlc_samples_are_excluded_before_categorical_denominators() {
    let root = temp_root("maxlc_denominator");
    let land = root.join("landtype.nc");
    write_landtype(&land, 16, 8, |i, j| {
        if support_index_for_source(i, j, 16, 8, 8, 4) != 0 {
            1
        } else if i == 15 && j == 6 {
            0
        } else if i == 15 && j == 7 {
            1
        } else {
            9
        }
    });
    let refine = RefineConfig {
        max_iter_cal: 1,
        refine_sea_ratio: true,
        th_sea_ratio: [0.4, 0.6],
        ..RefineConfig::default()
    };

    let raw = evaluate_threshold_support(
        &refine,
        "oceanmesh",
        Some(&land),
        parent_m_for_nlat(4),
        Some(&support0_domain()),
    )
    .expect("support demand");
    let sea = criterion(&raw, "sea_ratio");

    assert_eq!(
        sea.source_samples, 2,
        "one ocean + one valid land after maxlc removal"
    );
    assert_eq!(sea.hits.iter().filter(|&&hit| hit).count(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dominant_share_uses_valid_land_denominator_not_ocean_or_maxlc() {
    let root = temp_root("dominant_denominator");
    let land = root.join("landtype.nc");
    write_landtype(&land, 16, 8, |i, j| {
        if support_index_for_source(i, j, 16, 8, 8, 4) != 0 {
            2
        } else if (i, j) == (15, 6) || (i, j) == (15, 7) {
            1
        } else if (i, j) == (0, 6) {
            0
        } else {
            9
        }
    });
    let refine = RefineConfig {
        max_iter_cal: 1,
        refine_area_mainland: true,
        th_area_mainland: 0.75,
        ..RefineConfig::default()
    };

    let raw = evaluate_threshold_support(
        &refine,
        "landmesh",
        Some(&land),
        parent_m_for_nlat(4),
        Some(&support0_domain()),
    )
    .expect("support demand");
    let dominant = criterion(&raw, "area_mainland");

    assert_eq!(
        dominant.source_samples, 3,
        "two land + one ocean after maxlc removal"
    );
    assert_eq!(dominant.hits.iter().filter(|&&hit| hit).count(), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_domain_window_judges_exactly_what_the_whole_lattice_does() {
    let root = temp_root("window_oracle");
    let land = root.join("landtype.nc");
    let lai = root.join("lai.nc");
    let hash = |i: usize, j: usize, salt: usize| {
        (i.wrapping_mul(2_654_435_761) ^ j.wrapping_mul(40_503) ^ salt).wrapping_mul(97) % 1000
    };
    write_landtype(&land, 288, 144, |i, j| match hash(i, j, 1) % 10 {
        0..=2 => 0,
        3 => 17,
        k => k as i8,
    });
    write_numeric(&lai, "lai", 144, 72, |i, j| hash(i, j, 7) as f64 / 400.0);
    let mut refine = RefineConfig {
        max_iter_cal: 2,
        threshold_dir: root.display().to_string(),
        refine_num_landtypes: true,
        th_num_landtypes: 3,
        refine_sea_ratio: true,
        th_sea_ratio: [0.2, 0.6],
        ..RefineConfig::default()
    };
    configure_lai_thresholds(&mut refine);
    let star = |lon: f64, lat: f64, radius: f64, vertices: usize| GridRegion::Close {
        points: (0..vertices)
            .map(|k| {
                let t = k as f64 / vertices as f64 * std::f64::consts::TAU;
                let r = radius * (0.55 + 0.45 * (5.0 * t).sin().abs());
                crate::LonLatPoint {
                    lon: lon + r * t.cos() / lat.to_radians().cos(),
                    lat: lat + r * t.sin(),
                }
            })
            .collect(),
    };
    let dateline = GridRegion::Bbox {
        west: 170.0,
        east: -160.0,
        south: -20.0,
        north: 15.0,
    };
    let polar = GridRegion::Circle {
        lon: 30.0,
        lat: 75.0,
        radius_km: 1500.0,
    };
    let domains = [
        dateline.clone(),
        polar.clone(),
        star(100.0, 40.0, 12.0, 80),
        GridRegion::Any(vec![dateline, polar]),
    ];
    for domain in &domains {
        for (mesh, nlat) in [("landmesh", 36), ("oceanmesh", 72)] {
            let judge = |full: bool| {
                FULL_WINDOW.with(|cell| cell.set(full));
                let raw = evaluate_threshold_support(
                    &refine,
                    mesh,
                    Some(&land),
                    parent_m_for_nlat(nlat),
                    Some(domain),
                );
                FULL_WINDOW.with(|cell| cell.set(false));
                raw.expect("support demand")
            };
            let (windowed, whole) = (judge(false), judge(true));
            assert!(windowed.window.len() < whole.window.len() / 2, "{domain:?}");
            assert!(windowed.eligible_supports > 0);
            assert_eq!(windowed.eligible_supports, whole.eligible_supports);
            assert_eq!(windowed.criteria.len(), whole.criteria.len());
            let mut hits = 0;
            for (w, f) in windowed.criteria.iter().zip(&whole.criteria) {
                assert_eq!(w.id, f.id);
                assert_eq!(w.source_samples, f.source_samples, "{} {domain:?}", w.id);
                assert_eq!(w.empty_supports, f.empty_supports, "{}", w.id);
                assert_eq!(w.singleton_supports, f.singleton_supports, "{}", w.id);
                let project = |raw: &ThresholdSupportDemand, hits: &[bool]| {
                    raw.project_hfield(hits, 2 * nlat, nlat).unwrap()
                };
                let projected = project(&windowed, &w.hits);
                assert_eq!(projected, project(&whole, &f.hits), "{} {domain:?}", w.id);
                hits += projected.iter().filter(|&&hit| hit).count();
                let bounds = source_bounds_for_bbox(-180.0, 180.0, -90.0, 90.0, 1).unwrap();
                let a = windowed.project_source(&w.hits, bounds, 1).unwrap();
                let b = whole.project_source(&f.hits, bounds, 1).unwrap();
                assert_eq!(a.words, b.words, "{} source {domain:?}", w.id);
            }
            assert!(hits > 0, "the oracle compares something: {domain:?}");
        }
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_regional_domain_is_judged_finer_than_the_global_support_cap() {
    let refine = RefineConfig {
        max_iter_cal: 1,
        refine_sea_ratio: true,
        th_sea_ratio: [0.2, 0.6],
        ..RefineConfig::default()
    };
    let root = temp_root("fine_window");
    let land = root.join("landtype.nc");
    write_landtype(&land, 360, 180, |i, _| if i % 2 == 0 { 0 } else { 3 });
    let domain = GridRegion::Bbox {
        west: 99.0,
        east: 101.0,
        south: 38.0,
        north: 40.0,
    };
    // 2 km supports: the globe would be 10,000 x 20,000.
    let raw = evaluate_threshold_support(&refine, "landmesh", Some(&land), 2_000.0, Some(&domain))
        .expect("a regional window fits");
    assert!(raw.nlon * raw.nlat > MAX_SUPPORTS);
    assert!(raw.window.len() < 100_000, "{:?}", raw.window);
    assert!(raw.eligible_supports > 10_000);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn latitude_bands_judge_exactly_what_one_pass_does() {
    let root = temp_root("band_oracle");
    let land = root.join("landtype.nc");
    let lai = root.join("lai.nc");
    let hash = |i: usize, j: usize, salt: usize| {
        (i.wrapping_mul(2_654_435_761) ^ j.wrapping_mul(40_503) ^ salt).wrapping_mul(97) % 1000
    };
    write_landtype(&land, 288, 144, |i, j| match hash(i, j, 3) % 10 {
        0..=2 => 0,
        3 => 17,
        k => k as i8,
    });
    write_numeric(&lai, "lai", 144, 72, |i, j| hash(i, j, 11) as f64 / 400.0);
    let mut refine = RefineConfig {
        max_iter_cal: 2,
        threshold_dir: root.display().to_string(),
        refine_num_landtypes: true,
        th_num_landtypes: 3,
        refine_sea_ratio: true,
        th_sea_ratio: [0.2, 0.6],
        ..RefineConfig::default()
    };
    configure_lai_thresholds(&mut refine);
    let regional = GridRegion::Circle {
        lon: 175.0,
        lat: -30.0,
        radius_km: 2500.0,
    };
    for domain in [None, Some(&regional)] {
        for mesh in ["landmesh", "oceanmesh"] {
            let judge = |band: Option<usize>| {
                BAND_SUPPORTS.with(|cell| cell.set(band));
                let raw = evaluate_threshold_support(
                    &refine,
                    mesh,
                    Some(&land),
                    parent_m_for_nlat(36),
                    domain,
                );
                BAND_SUPPORTS.with(|cell| cell.set(None));
                raw.expect("support demand")
            };
            let (whole, banded) = (judge(None), judge(Some(50)));
            assert_eq!(banded.window, whole.window);
            assert_eq!(banded.eligible_supports, whole.eligible_supports);
            assert_eq!(banded.criteria.len(), whole.criteria.len());
            let mut hits = 0;
            for (b, w) in banded.criteria.iter().zip(&whole.criteria) {
                assert_eq!(b.id, w.id);
                assert_eq!(b.hits, w.hits, "{} {mesh} {domain:?}", b.id);
                assert_eq!(b.source_samples, w.source_samples, "{}", b.id);
                assert_eq!(b.empty_supports, w.empty_supports, "{}", b.id);
                assert_eq!(b.singleton_supports, w.singleton_supports, "{}", b.id);
                hits += b.hits.iter().filter(|&&hit| hit).count();
            }
            assert!(hits > 0, "the oracle compares something");
        }
    }
    BAND_SUPPORTS.with(|cell| cell.set(Some(100)));
    let bands = support_bands(LatticeWindow::full(72, 36));
    BAND_SUPPORTS.with(|cell| cell.set(None));
    assert_eq!(bands.len(), 36, "one row of 72 supports per band");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_support_keeps_shrinking_below_the_composition_hfield_bin() {
    assert_eq!(
        support_dimensions(parent_m_for_nlat(360)).unwrap(),
        (720, 360)
    );
    assert_eq!(
        support_dimensions(parent_m_for_nlat(720)).unwrap(),
        (1440, 720)
    );

    let root = temp_root("below_hfield_bin");
    let lai = root.join("lai.nc");
    write_numeric(&lai, "lai", 8, 4, |_i, _j| 1.0);
    let mut refine = RefineConfig {
        max_iter_cal: 2,
        threshold_dir: root.display().to_string(),
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;
    refine.th_onelayer_lnd[0] = 0.5;

    let raw = evaluate_threshold_support(&refine, "landmesh", None, parent_m_for_nlat(8), None)
        .expect("fine support");
    assert_eq!((raw.nlon, raw.nlat), (16, 8));
    let projected = raw
        .project_hfield(&criterion(&raw, "lai_mean").hits, 4, 2)
        .expect("project to coarse composition HField");
    assert!(projected.iter().all(|&hit| hit));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dateline_support_projects_to_both_ends_of_a_source_window() {
    let raw = ThresholdSupportDemand {
        nlon: 8,
        nlat: 4,
        parent_m: parent_m_for_nlat(4),
        longitude_shift: 22.5,
        eligible_supports: 32,
        window: LatticeWindow::full(8, 4),
        criteria: Vec::new(),
    };
    let mut hits = vec![false; raw.nlon * raw.nlat];
    hits[0] = true;
    let bounds = source_bounds_for_bbox(-180.0, 180.0, -90.0, 90.0, 1).unwrap();

    let demand = raw
        .project_source(&hits, bounds, 1)
        .expect("source projection");

    assert!(demand.is_demanded(1, 180), "wrapped west half");
    assert!(demand.is_demanded(360, 180), "wrapped east half");
}

#[test]
fn hfield_and_source_projection_match_positive_area_intersection_oracles() {
    let raw = ThresholdSupportDemand {
        nlon: 8,
        nlat: 4,
        parent_m: parent_m_for_nlat(4),
        longitude_shift: 22.5,
        eligible_supports: 32,
        window: LatticeWindow::full(8, 4),
        criteria: Vec::new(),
    };

    for hit in 0..raw.nlon * raw.nlat {
        let mut hits = vec![false; raw.nlon * raw.nlat];
        hits[hit] = true;
        for (dst_nlon, dst_nlat) in [(16, 8), (13, 7), (5, 3)] {
            let actual = raw
                .project_hfield(&hits, dst_nlon, dst_nlat)
                .expect("hfield projection");
            let expected =
                exhaustive_project_bool(&hits, raw.nlon, raw.nlat, dst_nlon, dst_nlat, true);
            assert_eq!(actual, expected, "hit {hit} to {dst_nlon}x{dst_nlat}");
        }
    }

    let mut hits = vec![false; raw.nlon * raw.nlat];
    hits[0] = true;
    hits[raw.nlat - 1] = true;
    hits[(raw.nlon - 1) * raw.nlat] = true;
    let bounds = source_bounds_for_bbox(-180.0, -90.0, -90.0, 0.0, 1).unwrap();
    let source = raw
        .project_source(&hits, bounds, 1)
        .expect("cropped source projection");
    let expected_source = exhaustive_project_bool(&hits, raw.nlon, raw.nlat, 360, 180, true);
    for lon in bounds.minlon_source..=bounds.maxlon_source {
        for lat in bounds.maxlat_source..=bounds.minlat_source {
            let index = (lon - 1) * 180 + (180 - lat);
            assert_eq!(
                source.is_demanded(lon, lat),
                expected_source[index],
                "cropped source {lon},{lat}"
            );
        }
    }
}

#[test]
fn plan_entry_and_hfield_report_share_raw_support_for_mean_std_and_landcover() {
    let root = temp_root("entry_conformance");
    let lai = root.join("lai.nc");
    let land = root.join("landtype.nc");
    write_numeric(&lai, "lai", 16, 8, |i, j| {
        if support_index_for_source(i, j, 16, 8, 8, 4) == 0 && (i + j) % 2 == 0 {
            10.0
        } else {
            0.0
        }
    });
    write_landtype(&land, 16, 8, |i, j| {
        if i == 8 && j == 0 {
            9
        } else if support_index_for_source(i, j, 16, 8, 8, 4) == 0 && (i + j) % 2 == 0 {
            2
        } else {
            1
        }
    });
    let mut refine = RefineConfig {
        refine_cal: true,
        max_iter_cal: 2,
        threshold_dir: root.display().to_string(),
        refine_num_landtypes: true,
        th_num_landtypes: 1,
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;
    refine.refine_onelayer_lnd[1] = true;
    refine.th_onelayer_lnd[0] = 1.0;
    refine.th_onelayer_lnd[1] = 1.0;
    let bounds = source_bounds_for_bbox(-180.0, 180.0, -90.0, 90.0, 1).unwrap();
    let inputs = DemandPlanInputs {
        bounds,
        gridnum_perdegree: 1,
        landtype_file: Some(&land),
        mesh_type: "landmesh",
        refine_coastline: false,
        domain_region: None,
        coastal_cache: Default::default(),
    };
    let plan = plan_demand_at_scale(&refine, &inputs, 1, parent_m_for_nlat(4)).expect("plan");
    let config = EarthmeshConfig {
        landtype_file: land.display().to_string(),
        ..EarthmeshConfig::default()
    };
    let options = HfieldRefineOptions {
        nlon: 8,
        nlat: 4,
        base_m: Some(parent_m_for_nlat(4)),
        max_level: Some(1),
        ..HfieldRefineOptions::default()
    };
    let (field, report) = build_composed_hfield_with_report(
        &[],
        &refine,
        "landmesh",
        Some(&config),
        parent_m_for_nlat(4),
        &options,
        2,
        None,
    )
    .expect("hfield report");

    let plan_support = sorted_support_reports(plan.raw_support.clone());
    let hfield_support = sorted_support_reports(
        report["criteria"]
            .as_array()
            .expect("criteria")
            .iter()
            .map(|row| row["raw_support"].clone())
            .collect(),
    );
    assert_eq!(
        support_ids(&plan_support),
        vec!["lai_mean", "lai_std", "landcover"]
    );
    assert_eq!(hfield_support, plan_support);
    assert!(plan_support
        .iter()
        .all(|row| row["hit_supports"].as_u64().unwrap_or(0) > 0));
    assert!(field
        .values()
        .iter()
        .any(|&value| value < parent_m_for_nlat(4)));

    let mut no_io = refine.clone();
    no_io.threshold_dir = "/definitely/not/read".into();
    let above_cap = plan_demand_at_scale(&no_io, &inputs, 3, parent_m_for_nlat(16))
        .expect("above-cap threshold level must not open sources");
    assert!(above_cap.raw_support.is_empty());
    assert!(above_cap.demand.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn window_planning_shares_support_and_matches_per_window_plans_cell_for_cell() {
    // The per-window entry is the oracle: sharing one support evaluation across
    // windows must not change a single demanded cell, count or support report.
    let root = temp_root("window_support_sharing");
    let lai = root.join("lai.nc");
    let land = root.join("landtype.nc");
    write_numeric(&lai, "lai", 16, 8, |i, j| {
        if (i * 3 + j) % 5 == 0 {
            10.0
        } else {
            0.0
        }
    });
    // Source resolution, so the coastline producer can read it too.
    write_landtype(&land, 360, 180, |i, j| match (i / 7 + 2 * (j / 5)) % 4 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 5,
    });
    let mut refine = RefineConfig {
        refine_cal: true,
        max_iter_cal: 3,
        threshold_dir: root.display().to_string(),
        refine_num_landtypes: true,
        th_num_landtypes: 1,
        ..RefineConfig::default()
    };
    refine.refine_onelayer_lnd[0] = true;
    refine.refine_onelayer_lnd[1] = true;
    refine.th_onelayer_lnd[0] = 1.0;
    refine.th_onelayer_lnd[1] = 1.0;
    let windows = [
        (-180.0, 0.0, -90.0, 0.0),
        (0.0, 180.0, -90.0, 0.0),
        (-180.0, 0.0, 0.0, 90.0),
        (0.0, 180.0, 0.0, 90.0),
    ];
    for refine_coastline in [false, true] {
        let inputs = windows
            .iter()
            .map(|&(west, east, south, north)| DemandPlanInputs {
                bounds: source_bounds_for_bbox(west, east, south, north, 1).unwrap(),
                gridnum_perdegree: 1,
                landtype_file: Some(&land),
                mesh_type: "landmesh",
                refine_coastline,
                domain_region: None,
                coastal_cache: Default::default(),
            })
            .collect::<Vec<_>>();
        for (level, parent_m) in [(1, parent_m_for_nlat(4)), (2, parent_m_for_nlat(8))] {
            let mut shared = Vec::new();
            plan_demand_at_scale_for_windows(&refine, &inputs, level, parent_m, |plan| {
                shared.push(plan);
                Ok(())
            })
            .expect("shared window plans");
            assert_eq!(shared.len(), inputs.len());
            let mut demanded = 0;
            for (input, shared) in inputs.iter().zip(&shared) {
                // The oracle recomputes the coastline; the shared plan reads
                // it from the cache the first level filled.
                let fresh = DemandPlanInputs {
                    coastal_cache: Default::default(),
                    ..input.clone()
                };
                let oracle =
                    plan_demand_at_scale(&refine, &fresh, level, parent_m).expect("oracle");
                assert_eq!(shared.level, oracle.level);
                assert_eq!(shared.demand, oracle.demand, "{:?}", input.bounds);
                assert_eq!(shared.contributions, oracle.contributions);
                assert_eq!(shared.raw_support, oracle.raw_support);
                demanded += oracle.demand.demanded_count();
            }
            assert!(
                demanded > 0,
                "fixture must demand something at level {level}"
            );
        }
    }
    let _ = fs::remove_dir_all(root);
}

fn sorted_support_reports(mut values: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    values.sort_by(|left, right| left["criterion"].as_str().cmp(&right["criterion"].as_str()));
    values
}

fn support_ids(values: &[serde_json::Value]) -> Vec<String> {
    values
        .iter()
        .map(|value| {
            value["criterion"]
                .as_str()
                .expect("criterion id")
                .to_string()
        })
        .collect::<Vec<_>>()
}

fn exhaustive_project_bool(
    hits: &[bool],
    src_nlon: usize,
    src_nlat: usize,
    dst_nlon: usize,
    dst_nlat: usize,
    shifted_lon: bool,
) -> Vec<bool> {
    let mut out = vec![false; dst_nlon * dst_nlat];
    for src_i in 0..src_nlon {
        for src_j in 0..src_nlat {
            if !hits[src_i * src_nlat + src_j] {
                continue;
            }
            for dst_i in 0..dst_nlon {
                if !intervals_overlap(src_i, src_nlon, dst_i, dst_nlon, shifted_lon) {
                    continue;
                }
                for dst_j in 0..dst_nlat {
                    if intervals_overlap(src_j, src_nlat, dst_j, dst_nlat, false) {
                        out[dst_i * dst_nlat + dst_j] = true;
                    }
                }
            }
        }
    }
    out
}

fn intervals_overlap(
    src_i: usize,
    src_n: usize,
    dst_i: usize,
    dst_n: usize,
    shifted: bool,
) -> bool {
    let (src_lo, src_hi) = if shifted {
        (
            ((2 * src_i) as f64 - 1.0) / (2 * src_n) as f64,
            ((2 * src_i) as f64 + 1.0) / (2 * src_n) as f64,
        )
    } else {
        (
            src_i as f64 / src_n as f64,
            (src_i + 1) as f64 / src_n as f64,
        )
    };
    let dst_lo = dst_i as f64 / dst_n as f64;
    let dst_hi = (dst_i + 1) as f64 / dst_n as f64;
    if src_lo < 0.0 {
        dst_lo < src_hi || dst_hi > src_lo + 1.0
    } else if src_hi > 1.0 {
        dst_hi > src_lo || dst_lo < src_hi - 1.0
    } else {
        dst_lo < src_hi && dst_hi > src_lo
    }
}
