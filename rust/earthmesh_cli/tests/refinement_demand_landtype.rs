//! The land-type producers, on synthetic rasters where the answer is known.
//!
//! These need no mounted data: 1 degree per cell and a hand-placed coastline
//! make every index in the assertions checkable by hand.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use earthmesh_cli::refinement_demand::{
    landtype::{
        coastal_demand, dominant_class_demand, landcover_heterogeneity_demand, sea_ratio_demand,
    },
    reduce_demand_to_circles, source_bounds_for_bbox,
};
use earthmesh_mesh::MethodCRefinementRegion;

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "earthmesh_refinement_demand_{name}_{}_{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

const NLONS: usize = 360;
const NLATS: usize = 180;

/// Write a 1-degree land-type raster from a per-cell classifier.
///
/// Source index 1 sits at -180 / +90 with latitude running north to south, so
/// `(lon_index, lat_index)` here are the same one-based indices the producers
/// use, minus one for the zero-based NetCDF layout.
fn write_landtype(path: &std::path::Path, class_at: impl Fn(usize, usize) -> i8) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create landtype file");
    file.add_dimension("longitude", NLONS).expect("lon dim");
    file.add_dimension("latitude", NLATS).expect("lat dim");
    let mut values = vec![0_i8; NLONS * NLATS];
    for lon in 0..NLONS {
        for lat in 0..NLATS {
            values[lon * NLATS + lat] = class_at(lon + 1, lat + 1);
        }
    }
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .expect("landtype var");
    var.put_values(&values, (.., ..)).expect("write landtype");
}

/// A meridional coast: everything east of `lon_index` is land.
fn meridional_coast(boundary_lon_index: usize) -> impl Fn(usize, usize) -> i8 {
    move |lon, _lat| i8::from(lon >= boundary_lon_index)
}

#[test]
fn a_coast_marks_the_cells_on_both_of_its_sides() {
    let root = temp_root("coast_cells");
    let path = root.join("landtype.nc");
    // -180 + 290 - 1 = 109 degrees east.
    write_landtype(&path, meridional_coast(291));

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let demand = coastal_demand(&path, 1, bounds).expect("coastal demand");

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        assert!(
            demand.is_demanded(290, lat),
            "the ocean cell against the coast must be demanded"
        );
        assert!(
            demand.is_demanded(291, lat),
            "the land cell against the coast must be demanded"
        );
        assert!(
            !demand.is_demanded(287, lat),
            "open ocean three cells away must not be demanded"
        );
        assert!(
            !demand.is_demanded(294, lat),
            "inland three cells away must not be demanded"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_coast_on_a_block_edge_still_yields_circles() {
    // The gap this producer exists to close. Blocking the raster and asking
    // "does this block hold both land and ocean" misses a coast that runs
    // along a block edge: the land block holds no ocean, the ocean block no
    // land, so neither looks coastal and the chain comes out empty. Marking
    // boundary cells cannot miss it -- the cells are in both blocks.
    let root = temp_root("coast_block_edge");
    let path = root.join("landtype.nc");

    // Blocks are half a radius across. At 1 degree per cell a 222 km radius
    // gives 1-degree blocks, so a coast on an integer degree is exactly on a
    // block edge.
    let meters_per_degree = std::f64::consts::PI * 6_371_229.0 / 180.0;
    let radius_meters = 2.0 * meters_per_degree;
    write_landtype(&path, meridional_coast(291));

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let demand = coastal_demand(&path, 1, bounds).expect("coastal demand");
    let regions = reduce_demand_to_circles(&demand, 1, radius_meters).expect("reduce");

    assert!(
        !regions.is_empty(),
        "an aligned coast must still produce a chain"
    );
    // One circle per demanded cell means the blocks here are a single cell
    // across, and a single cell holds a single class -- so the rule this
    // replaced ("does the block hold both land and ocean") could never fire on
    // this raster, whatever the coastline looked like.
    assert_eq!(
        regions.len(),
        demand.demanded_count(),
        "blocks must be one cell across for this to test what it claims"
    );
    // And the chain has to run the whole length of the coast, not just part.
    let latitudes: Vec<f64> = regions
        .iter()
        .map(|region| {
            let MethodCRefinementRegion::Circle { center, .. } = region else {
                panic!("reduction must emit circles");
            };
            center.lat_degrees
        })
        .collect();
    let span = latitudes
        .iter()
        .fold(f64::NEG_INFINITY, |max, value| max.max(*value))
        - latitudes
            .iter()
            .fold(f64::INFINITY, |min, value| min.min(*value));
    assert!(span > 6.0, "chain spans only {span} degrees of the coast");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn open_ocean_demands_nothing() {
    let root = temp_root("open_ocean");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let demand = coastal_demand(&path, 1, bounds).expect("coastal demand");
    assert!(demand.is_empty());
    assert!(reduce_demand_to_circles(&demand, 1, 200_000.0)
        .expect("reduce")
        .is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn land_cover_heterogeneity_follows_the_cell_size_it_is_asked_about() {
    // The point of this criterion: whether a neighbourhood is heterogeneous
    // depends on how big the neighbourhood is. A stripe pattern with a period
    // of four cells is uniform to a one-cell look and mixed to a wider one, so
    // the radius the caller passes -- which stands in for the mesh cell being
    // judged -- decides the answer. That is exactly why this cannot be settled
    // once on a raster whose resolution has nothing to do with the mesh.
    let root = temp_root("landcover");
    let path = root.join("landtype.nc");
    write_landtype(&path, |lon, _lat| ((lon / 4) % 3) as i8 + 1);

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let narrow =
        landcover_heterogeneity_demand(&path, 1, bounds, 1, 1).expect("narrow heterogeneity");
    let wide = landcover_heterogeneity_demand(&path, 1, bounds, 5, 1).expect("wide heterogeneity");

    assert!(
        wide.demanded_count() > narrow.demanded_count(),
        "narrow {} vs wide {}",
        narrow.demanded_count(),
        wide.demanded_count()
    );
    // With stripes four cells wide, a five-cell radius always spans a change.
    assert_eq!(wide.demanded_count(), wide.bounds_cell_count());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_zero_radius_heterogeneity_request_is_rejected() {
    let root = temp_root("landcover_radius");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 1);
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    assert!(landcover_heterogeneity_demand(&path, 1, bounds, 0, 1).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_window_on_the_rasters_edge_still_reads() {
    // The halo these producers read must stop at the source dimensions. The
    // reader rejects bounds past them, so an unclipped halo turns a window
    // touching 180 east or the south pole into a hard error rather than a
    // coastline.
    let root = temp_root("raster_edge");
    let path = root.join("landtype.nc");
    write_landtype(&path, meridional_coast(NLONS));

    for (label, west, east, south, north) in [
        ("east edge", 178.0, 180.0, 18.0, 26.0),
        ("west edge", -180.0, -178.0, 18.0, 26.0),
        ("south pole", 100.0, 108.0, -90.0, -88.0),
        ("north pole", 100.0, 108.0, 88.0, 90.0),
    ] {
        let bounds = source_bounds_for_bbox(west, east, south, north, 1).expect(label);
        coastal_demand(&path, 1, bounds).unwrap_or_else(|error| panic!("{label}: {error}"));
        landcover_heterogeneity_demand(&path, 1, bounds, 3, 1)
            .unwrap_or_else(|error| panic!("{label} heterogeneity: {error}"));
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn the_sea_ratio_criterion_narrows_as_the_cell_shrinks() {
    // The difference from `coastal_demand`: this asks what fraction of a cell
    // is sea, so shrinking the cell pushes the fraction toward 0 or 1 and fewer
    // cells qualify. A class-boundary detector gives the same answer at every
    // scale; this one is why the namelist's coastal criterion is written as a
    // ratio in the first place.
    let root = temp_root("sea_ratio");
    let path = root.join("landtype.nc");
    write_landtype(&path, meridional_coast(291));

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let wide = sea_ratio_demand(&path, 1, bounds, 4, 0.4, 0.6).expect("wide");
    let narrow = sea_ratio_demand(&path, 1, bounds, 1, 0.4, 0.6).expect("narrow");
    assert!(
        narrow.demanded_count() < wide.demanded_count(),
        "narrow {} vs wide {}",
        narrow.demanded_count(),
        wide.demanded_count()
    );
    assert!(!wide.is_empty(), "a coast must qualify at some scale");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn open_ocean_and_deep_inland_are_never_a_sea_ratio_mix() {
    let root = temp_root("sea_ratio_uniform");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 0);
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    assert!(sea_ratio_demand(&path, 1, bounds, 2, 0.4, 0.6)
        .expect("all sea")
        .is_empty());

    write_landtype(&path, |_, _| 1);
    assert!(sea_ratio_demand(&path, 1, bounds, 2, 0.4, 0.6)
        .expect("all land")
        .is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn the_dominant_class_criterion_fires_only_where_no_class_rules() {
    let root = temp_root("dominant");
    let path = root.join("landtype.nc");
    // Three classes in equal stripes: no class holds 75% of any wide window.
    write_landtype(&path, |lon, _lat| ((lon / 2) % 3) as i8 + 1);

    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");
    let mixed = dominant_class_demand(&path, 1, bounds, 4, 0.75).expect("mixed");
    assert_eq!(mixed.demanded_count(), mixed.bounds_cell_count());

    // One class everywhere: it holds all of it, so nothing qualifies.
    write_landtype(&path, |_, _| 5);
    let uniform = dominant_class_demand(&path, 1, bounds, 4, 0.75).expect("uniform");
    assert!(uniform.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn the_new_criteria_reject_the_requests_they_cannot_answer() {
    let root = temp_root("criteria_bounds");
    let path = root.join("landtype.nc");
    write_landtype(&path, |_, _| 1);
    let bounds = source_bounds_for_bbox(105.0, 113.0, 18.0, 26.0, 1).expect("bounds");

    assert!(sea_ratio_demand(&path, 1, bounds, 0, 0.4, 0.6).is_err());
    assert!(sea_ratio_demand(&path, 1, bounds, 2, 0.6, 0.4).is_err());
    assert!(sea_ratio_demand(&path, 1, bounds, 2, f64::NAN, 0.6).is_err());
    assert!(dominant_class_demand(&path, 1, bounds, 0, 0.75).is_err());
    assert!(dominant_class_demand(&path, 1, bounds, 2, 1.5).is_err());
    let _ = fs::remove_dir_all(root);
}
