//! The continuous-threshold producer, on a synthetic field.
//!
//! This is the producer every numeric criterion in the catalogue goes through —
//! `sst`, `eke`, `slope`, `dem`, `typhoon`, and bathymetry once it lands — so
//! the cases here are about the comparison and the missing-data rule, not about
//! any one of them.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use earthmesh_cli::refinement_demand::{
    reduce_demand_to_circles, source_bounds_for_bbox, threshold::threshold_demand,
    threshold::threshold_stddev_demand, threshold::ThresholdSide,
};

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "earthmesh_refinement_threshold_{name}_{}_{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

const NLONS: usize = 360;
const NLATS: usize = 180;

/// Write a 1-degree field from a per-cell function of one-based source indices.
fn write_field(path: &Path, var_name: &str, value_at: impl Fn(usize, usize) -> f64) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create threshold file");
    file.add_dimension("longitude", NLONS).expect("lon dim");
    file.add_dimension("latitude", NLATS).expect("lat dim");
    let mut values = Vec::with_capacity(NLONS * NLATS);
    for lon in 0..NLONS {
        for lat in 0..NLATS {
            values.push(value_at(lon + 1, lat + 1));
        }
    }
    let mut var = file
        .add_variable::<f64>(var_name, &["longitude", "latitude"])
        .expect("threshold variable");
    var.put_values(&values, (.., ..)).expect("write field");
}

/// Warm west of `boundary_lon_index`, cold east of it.
fn warm_pool(boundary_lon_index: usize) -> impl Fn(usize, usize) -> f64 {
    move |lon, _lat| if lon < boundary_lon_index { 30.0 } else { 20.0 }
}

#[test]
fn above_marks_only_the_cells_over_the_threshold() {
    let root = temp_root("above");
    let path = root.join("sst.nc");
    write_field(&path, "sst", warm_pool(291));

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let demand = threshold_demand(&path, "sst", 1, bounds, ThresholdSide::Above, 28.0)
        .expect("threshold demand");

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        assert!(demand.is_demanded(290, lat), "30 C must exceed 28 C");
        assert!(!demand.is_demanded(291, lat), "20 C must not");
    }
    assert!(!demand.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn below_marks_the_other_side() {
    // Bathymetry reads this way: shallow water first, deep water later.
    let root = temp_root("below");
    let path = root.join("sst.nc");
    write_field(&path, "sst", warm_pool(291));

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let demand = threshold_demand(&path, "sst", 1, bounds, ThresholdSide::Below, 28.0)
        .expect("threshold demand");

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        assert!(!demand.is_demanded(290, lat));
        assert!(demand.is_demanded(291, lat));
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_field_nowhere_past_the_threshold_demands_nothing() {
    let root = temp_root("uniform");
    let path = root.join("sst.nc");
    write_field(&path, "sst", |_, _| 20.0);

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let demand = threshold_demand(&path, "sst", 1, bounds, ThresholdSide::Above, 28.0)
        .expect("threshold demand");
    assert!(demand.is_empty());
    assert!(reduce_demand_to_circles(&demand, 1, 200_000.0)
        .expect("reduce")
        .is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn absent_data_is_refused_rather_than_guessed_at() {
    // The engine rejects missing values inside a requested threshold window
    // (`reject_invalid_threshold_values`), and this producer inherits that.
    // Skipping such cells would silently under-refine; treating a fill value as
    // a real number would refine an ocean that nothing asked for.
    let root = temp_root("missing");
    let path = root.join("sst.nc");
    write_field(
        &path,
        "sst",
        |lon, _lat| {
            if lon % 2 == 0 {
                f64::NAN
            } else {
                20.0
            }
        },
    );

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let error = threshold_demand(&path, "sst", 1, bounds, ThresholdSide::Above, 28.0)
        .expect_err("a window with missing data must not answer");
    assert!(
        error.to_string().contains("missing/non-finite"),
        "got {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_non_finite_threshold_is_rejected() {
    let root = temp_root("bad_threshold");
    let path = root.join("sst.nc");
    write_field(&path, "sst", |_, _| 20.0);
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    for threshold in [f64::NAN, f64::INFINITY] {
        assert!(
            threshold_demand(&path, "sst", 1, bounds, ThresholdSide::Above, threshold).is_err(),
            "threshold {threshold} must be rejected"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn two_criteria_over_one_window_union_into_one_chain() {
    // The shape a real project takes: warm water in the west, steep slope in
    // the east, one chain of circles covering both.
    let root = temp_root("union");
    let sst = root.join("sst.nc");
    let slope = root.join("slope.nc");
    write_field(&sst, "sst", |lon, _lat| if lon < 288 { 30.0 } else { 20.0 });
    write_field(
        &slope,
        "slope",
        |lon, _lat| {
            if lon > 294 {
                25.0
            } else {
                2.0
            }
        },
    );

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let mut demand =
        threshold_demand(&sst, "sst", 1, bounds, ThresholdSide::Above, 28.0).expect("sst demand");
    let steep = threshold_demand(&slope, "slope", 1, bounds, ThresholdSide::Above, 15.0)
        .expect("slope demand");
    let (warm_cells, steep_cells) = (demand.demanded_count(), steep.demanded_count());
    demand.union_with(&steep).expect("union");

    assert_eq!(demand.demanded_count(), warm_cells + steep_cells);
    assert!(!reduce_demand_to_circles(&demand, 1, 200_000.0)
        .expect("reduce")
        .is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_flat_field_is_never_rough_however_large_it_is() {
    // The distinction the two halves of a threshold flag draw: a field can sit
    // far above its mean threshold everywhere and still ask nothing of its std
    // threshold. Reading only the mean half collapsed these into one question,
    // and a project asking for refinement where a field is *rough* got a
    // uniform mesh with nothing said about it.
    let root = temp_root("flat");
    let path = root.join("slope.nc");
    write_field(&path, "slope", |_, _| 500.0);
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");

    let above = threshold_demand(&path, "slope", 1, bounds, ThresholdSide::Above, 100.0)
        .expect("mean demand");
    assert!(!above.is_empty(), "500 clears 100 everywhere");

    let rough = threshold_stddev_demand(&path, "slope", 1, bounds, 2, 0.5).expect("std demand");
    assert!(
        rough.is_empty(),
        "a constant field has no variation to demand refinement"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_rough_patch_is_demanded_and_flat_ground_is_not() {
    let root = temp_root("rough");
    let path = root.join("slope.nc");
    // Source index 1 is 180 west, so 110 east is index 291; the window below
    // spans 105 to 113 east. A checkerboard inside it, flat everywhere else.
    write_field(&path, "slope", |lon, lat| {
        if (289..=294).contains(&lon) && (66..=71).contains(&lat) {
            if (lon + lat) % 2 == 0 {
                100.0
            } else {
                0.0
            }
        } else {
            10.0
        }
    });
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");

    let rough = threshold_stddev_demand(&path, "slope", 1, bounds, 1, 20.0).expect("std demand");
    assert!(!rough.is_empty(), "the checkerboard must be demanded");
    assert!(
        rough.is_demanded(291, 68),
        "the middle of the patch must be demanded"
    );
    assert!(
        !rough.is_demanded(bounds.minlon_source, bounds.minlat_source),
        "the flat corner of the window must not be"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_wider_neighbourhood_spreads_the_demand() {
    // Resolution dependence, which is why the radius is a parameter: a coarser
    // pass judges a wider neighbourhood, so one rough spot pulls more of its
    // surroundings in with it. This is what makes the criterion answer
    // differently at each level instead of once for the whole run.
    let root = temp_root("radius");
    let path = root.join("slope.nc");
    write_field(&path, "slope", |lon, lat| {
        if lon == 291 && lat == 68 {
            1000.0
        } else {
            0.0
        }
    });
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");

    let tight = threshold_stddev_demand(&path, "slope", 1, bounds, 1, 1.0).expect("tight");
    let wide = threshold_stddev_demand(&path, "slope", 1, bounds, 3, 1.0).expect("wide");
    assert!(
        wide.demanded_count() > tight.demanded_count(),
        "wide {} must exceed tight {}",
        wide.demanded_count(),
        tight.demanded_count()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_non_finite_std_threshold_is_rejected_too() {
    let root = temp_root("bad_std_threshold");
    let path = root.join("slope.nc");
    write_field(&path, "slope", |_, _| 20.0);
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    for threshold in [f64::NAN, f64::INFINITY] {
        assert!(threshold_stddev_demand(&path, "slope", 1, bounds, 1, threshold).is_err());
    }
    let _ = fs::remove_dir_all(root);
}
