//! The reduction is tested on demand built by hand, with no data source in
//! sight. That is the point of the split: whatever asked for refinement, the
//! reducer sees the same thing.

use super::*;

/// One degree per cell keeps index arithmetic readable in the assertions.
const PER_DEGREE: usize = 1;

fn window(west: f64, east: f64, south: f64, north: f64) -> RefinementDemand {
    let bounds = source_bounds_for_bbox(west, east, south, north, PER_DEGREE).expect("bounds");
    RefinementDemand::new(bounds, PER_DEGREE).expect("demand")
}

fn mark(demand: &mut RefinementDemand, lon: f64, lat: f64) {
    let point = source_bounds_for_bbox(lon, lon + 0.5, lat - 0.5, lat, PER_DEGREE).expect("point");
    demand.set(point.minlon_source, point.maxlat_source, true);
}

#[test]
fn empty_demand_reduces_to_no_circles() {
    let demand = window(110.0, 120.0, 18.0, 26.0);
    assert!(demand.is_empty());
    let regions = reduce_demand_to_circles(&demand, 1, 200_000.0).expect("reduce");
    assert!(regions.is_empty(), "got {regions:?}");
}

#[test]
fn one_demanded_cell_yields_one_circle_covering_it() {
    let mut demand = window(110.0, 120.0, 18.0, 26.0);
    mark(&mut demand, 114.0, 22.0);
    assert_eq!(demand.demanded_count(), 1);

    let radius_meters = 200_000.0;
    let regions = reduce_demand_to_circles(&demand, 2, radius_meters).expect("reduce");
    assert_eq!(regions.len(), 1, "got {regions:?}");
    let MethodCRefinementRegion::Circle {
        center,
        radius_meters: emitted,
        level,
    } = regions[0]
    else {
        panic!("reduction must emit circles");
    };
    assert_eq!(level, 2);
    assert!((emitted - radius_meters).abs() < 1.0);
    // The circle has to actually contain the cell that asked for it.
    let distance = earthmesh_hfield::great_circle_distance_m(
        center.lon_degrees,
        center.lat_degrees,
        114.0,
        22.0,
    );
    assert!(
        distance < radius_meters,
        "circle at {center:?} is {distance:.0} m from the demanded cell"
    );
}

#[test]
fn demand_far_apart_stays_two_circles() {
    let mut demand = window(100.0, 140.0, 0.0, 40.0);
    mark(&mut demand, 105.0, 35.0);
    mark(&mut demand, 135.0, 5.0);
    let regions = reduce_demand_to_circles(&demand, 1, 200_000.0).expect("reduce");
    assert_eq!(regions.len(), 2, "got {regions:?}");
}

#[test]
fn a_bigger_radius_covers_the_same_demand_with_fewer_circles() {
    // Blocks scale with the radius, so a coarser chain is cheaper. This is the
    // knob a caller turns when the parent generation cannot host small circles.
    let mut demand = window(100.0, 140.0, 0.0, 40.0);
    for degree in 0..30 {
        mark(&mut demand, 105.0 + degree as f64, 5.0 + degree as f64);
    }
    let fine = reduce_demand_to_circles(&demand, 1, 200_000.0).expect("fine");
    let coarse = reduce_demand_to_circles(&demand, 1, 800_000.0).expect("coarse");
    assert!(
        coarse.len() < fine.len(),
        "fine {} vs coarse {}",
        fine.len(),
        coarse.len()
    );
    assert!(!coarse.is_empty());
}

#[test]
fn consecutive_blocks_overlap_by_half_a_radius() {
    // Continuity along a feature is the whole reason for half-radius blocking:
    // neighbouring circles must overlap or a chain leaves gaps between them.
    let mut demand = window(100.0, 140.0, 0.0, 40.0);
    for degree in 0..40 {
        mark(&mut demand, 100.5 + degree as f64, 20.0);
    }
    let radius_meters = 200_000.0;
    let regions = reduce_demand_to_circles(&demand, 1, radius_meters).expect("reduce");
    assert!(regions.len() > 2, "expected a chain, got {regions:?}");
    for pair in regions.windows(2) {
        let (
            MethodCRefinementRegion::Circle { center: left, .. },
            MethodCRefinementRegion::Circle { center: right, .. },
        ) = (&pair[0], &pair[1])
        else {
            panic!("reduction must emit circles");
        };
        let gap = earthmesh_hfield::great_circle_distance_m(
            left.lon_degrees,
            left.lat_degrees,
            right.lon_degrees,
            right.lat_degrees,
        );
        assert!(
            gap < 2.0 * radius_meters,
            "consecutive circles {gap:.0} m apart do not overlap"
        );
    }
}

#[test]
fn several_criteria_union_into_one_reduction() {
    // Coast, SST and slope do not each get their own chain; they add up to one
    // demand and one set of circles, the same way the h-field takes a min.
    let bounds = source_bounds_for_bbox(100.0, 140.0, 0.0, 40.0, PER_DEGREE).expect("bounds");
    let mut coast = RefinementDemand::new(bounds, PER_DEGREE).expect("coast");
    mark(&mut coast, 105.0, 35.0);
    let mut warm_water = RefinementDemand::new(bounds, PER_DEGREE).expect("sst");
    mark(&mut warm_water, 135.0, 5.0);

    coast.union_with(&warm_water).expect("union");
    assert_eq!(coast.demanded_count(), 2);
    assert_eq!(
        reduce_demand_to_circles(&coast, 1, 200_000.0)
            .expect("reduce")
            .len(),
        2
    );
}

#[test]
fn demands_over_different_windows_do_not_union() {
    let mut here = window(100.0, 140.0, 0.0, 40.0);
    let there = window(0.0, 40.0, 0.0, 40.0);
    let error = here.union_with(&there).expect_err("windows differ");
    assert!(error.to_string().contains("share a window"), "got {error}");
}

#[test]
fn every_demanded_cell_lands_inside_some_circle() {
    // The property the block-edge gap used to break. Blocks are half a radius
    // across, so the farthest a demanded cell can sit from its block centre is
    // about 0.35 of a radius -- coverage holds by construction, for any demand
    // whatever produced it.
    let mut demand = window(100.0, 140.0, 0.0, 40.0);
    let cells = [
        (105.0, 35.0),
        (106.0, 35.0),
        (120.0, 20.0),
        (139.0, 1.0),
        (100.0, 39.0),
    ];
    for (lon, lat) in cells {
        mark(&mut demand, lon, lat);
    }
    let radius_meters = 200_000.0;
    let regions = reduce_demand_to_circles(&demand, 1, radius_meters).expect("reduce");
    for (lon, lat) in cells {
        let covered = regions.iter().any(|region| {
            let MethodCRefinementRegion::Circle {
                center,
                radius_meters,
                ..
            } = region
            else {
                panic!("reduction must emit circles");
            };
            earthmesh_hfield::great_circle_distance_m(
                center.lon_degrees,
                center.lat_degrees,
                lon,
                lat,
            ) < *radius_meters
        });
        assert!(covered, "demand at {lon},{lat} is outside every circle");
    }
}

#[test]
fn an_unusable_radius_is_rejected() {
    let demand = window(110.0, 120.0, 18.0, 26.0);
    for radius in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert!(
            reduce_demand_to_circles(&demand, 1, radius).is_err(),
            "radius {radius} must be rejected"
        );
    }
}

#[test]
fn the_materializable_floor_stays_four_tenths_of_a_base_cell() {
    // Measured against spawn_nest, not derived from the ring count; see the
    // note on materializable_radius_meters.
    assert!((materializable_radius_meters(381_000.0) - 152_400.0).abs() < 1.0);
}

#[test]
fn bounds_at_the_rasters_extremes_stay_inside_it() {
    // floor-plus-one puts exactly 180 east and exactly -90 one past the last
    // cell; a window ending there must not claim a column the raster has not
    // got, or the reader rejects the whole request.
    let bounds = source_bounds_for_bbox(178.0, 180.0, -90.0, -88.0, 4).expect("edge window");
    assert_eq!(bounds.maxlon_source, 360 * 4);
    assert_eq!(bounds.minlat_source, 180 * 4);
    assert!(RefinementDemand::new(bounds, 4).is_ok());
}

#[test]
fn bounds_must_describe_a_real_window() {
    assert!(source_bounds_for_bbox(120.0, 110.0, 18.0, 26.0, 1).is_err());
    assert!(source_bounds_for_bbox(110.0, 120.0, 26.0, 18.0, 1).is_err());
    assert!(source_bounds_for_bbox(110.0, 120.0, 18.0, 26.0, 0).is_err());
    assert!(source_bounds_for_bbox(f64::NAN, 120.0, 18.0, 26.0, 1).is_err());
}

/// A window whose cell count is not a multiple of 64, so the last word is
/// partly padding — the only case the packed form can get wrong.
fn ragged_window() -> RefinementDemand {
    let demand = window(0.0, 5.0, 0.0, 3.0);
    assert!(
        !demand.bounds_cell_count().is_multiple_of(64),
        "this window must leave padding bits or it tests nothing: {} cells",
        demand.bounds_cell_count()
    );
    demand
}

#[test]
fn the_packed_form_counts_only_real_cells() {
    let mut demand = ragged_window();
    let cells = demand.bounds_cell_count();
    assert_eq!(demand.demanded_count(), 0);
    assert!(demand.is_empty());

    let bounds = demand.bounds();
    for lon in bounds.minlon_source..=bounds.maxlon_source {
        for lat in bounds.maxlat_source..=bounds.minlat_source {
            demand.set(lon, lat, true);
        }
    }
    // Padding bits must stay clear, or this would exceed the window.
    assert_eq!(demand.demanded_count(), cells);
    assert!(!demand.is_empty());
}

#[test]
fn clearing_a_cell_leaves_its_neighbours_alone() {
    // Every cell shares a word with 63 others; a careless clear takes them out.
    let mut demand = ragged_window();
    let bounds = demand.bounds();
    for lon in bounds.minlon_source..=bounds.maxlon_source {
        for lat in bounds.maxlat_source..=bounds.minlat_source {
            demand.set(lon, lat, true);
        }
    }
    let cells = demand.bounds_cell_count();
    demand.set(bounds.minlon_source + 2, bounds.maxlat_source + 1, false);

    assert_eq!(demand.demanded_count(), cells - 1);
    assert!(!demand.is_demanded(bounds.minlon_source + 2, bounds.maxlat_source + 1));
    assert!(demand.is_demanded(bounds.minlon_source + 1, bounds.maxlat_source + 1));
    assert!(demand.is_demanded(bounds.minlon_source + 3, bounds.maxlat_source + 1));
}

#[test]
fn equality_still_means_the_same_cells_are_demanded() {
    // Derived PartialEq compares the padding bits too, so anything that left
    // one set would make two identical demands compare unequal.
    let mut left = ragged_window();
    let mut right = ragged_window();
    let bounds = left.bounds();
    assert_eq!(left, right);

    left.set(bounds.minlon_source, bounds.maxlat_source, true);
    assert_ne!(left, right);
    right.set(bounds.minlon_source, bounds.maxlat_source, true);
    assert_eq!(left, right);

    // Setting then clearing must return to the original state exactly.
    left.set(bounds.maxlon_source, bounds.minlat_source, true);
    left.set(bounds.maxlon_source, bounds.minlat_source, false);
    assert_eq!(left, right);
}

#[test]
fn a_union_is_the_bitwise_or_of_the_two() {
    let mut left = ragged_window();
    let mut right = ragged_window();
    let bounds = left.bounds();
    left.set(bounds.minlon_source, bounds.maxlat_source, true);
    right.set(bounds.maxlon_source, bounds.minlat_source, true);

    left.union_with(&right).expect("union");
    assert_eq!(left.demanded_count(), 2);
    assert!(left.is_demanded(bounds.minlon_source, bounds.maxlat_source));
    assert!(left.is_demanded(bounds.maxlon_source, bounds.minlat_source));
}

#[test]
fn a_parallel_fill_lands_where_the_serial_one_would() {
    // Rows are handed to different threads, so a mistake in the row-to-bit
    // arithmetic shows up as cells marked in the wrong place rather than as a
    // crash. Compared against `set` over the same predicate.
    let mut parallel = window(0.0, 17.0, 0.0, 11.0);
    let mut serial = window(0.0, 17.0, 0.0, 11.0);
    let bounds = parallel.bounds();
    // Something that varies in both axes and is not symmetric, so a transposed
    // or off-by-one index cannot pass by accident.
    let decide = |lon: usize, lat: usize| (lon * 3 + lat * 7) % 5 < 2;

    parallel.fill_par(decide);
    for lat in bounds.maxlat_source..=bounds.minlat_source {
        for lon in bounds.minlon_source..=bounds.maxlon_source {
            serial.set(lon, lat, decide(lon, lat));
        }
    }

    assert_eq!(parallel.demanded_count(), serial.demanded_count());
    assert_eq!(parallel, serial);
    assert!(
        parallel.demanded_count() > 0,
        "the predicate must mark some"
    );
}

#[test]
fn a_parallel_fill_only_adds_to_what_is_already_there() {
    // `fill_par` ORs into the bitset, as `set(.., true)` does; a criterion that
    // fills after another has run must not wipe it.
    let mut demand = window(0.0, 17.0, 0.0, 11.0);
    let bounds = demand.bounds();
    demand.set(bounds.minlon_source, bounds.maxlat_source, true);
    demand.fill_par(|lon, _| lon == bounds.maxlon_source);

    assert!(demand.is_demanded(bounds.minlon_source, bounds.maxlat_source));
    assert!(demand.is_demanded(bounds.maxlon_source, bounds.maxlat_source));
}
