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
    threshold::ThresholdSide,
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
