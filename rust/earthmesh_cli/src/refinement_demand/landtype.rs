//! Refinement demand read off a land-type raster.
//!
//! Two criteria live here because they read the same source: the coastline,
//! which is a class boundary, and land-cover heterogeneity, which is how many
//! classes crowd into one neighbourhood. Both use the engine's own rule —
//! `landtype != 0` is land, matching `classify_area_judge_landtype_one_based` —
//! so refinement and the carve agree on where the coast is.

use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::class_counts::ClassPrefixSums;
use super::RefinementDemand;
use crate::mkgrd_data_preprocess_source::read_landtype_bbox_window_one_based;

/// Grow a window by `cells` on every side, stopping at the raster's edges.
///
/// The reader rejects bounds past the source dimensions, so a window touching
/// 180 degrees east or the south pole must not ask for a halo beyond them. A
/// clipped halo costs only that the outermost cells are classified against
/// fewer neighbours, which is what "no neighbour there" means anyway.
pub(super) fn halo_within_source(
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    cells: usize,
) -> AreaJudgeSourceBounds {
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    let nlats_source = gridnum_perdegree.saturating_mul(180);
    AreaJudgeSourceBounds {
        minlon_source: bounds.minlon_source.saturating_sub(cells).max(1),
        maxlon_source: (bounds.maxlon_source + cells).min(nlons_source),
        maxlat_source: bounds.maxlat_source.saturating_sub(cells).max(1),
        minlat_source: (bounds.minlat_source + cells).min(nlats_source),
    }
}

/// Mark every source cell that touches the land/sea boundary.
///
/// A cell is demanded when it differs in class from one of its four neighbours.
/// The earlier form of this asked whether a whole block held both classes,
/// which misses a coast running along a block edge: the land block holds no
/// ocean and the ocean block no land, so neither is coastal and a straight
/// aligned coastline yields no circles at all. Marking boundary cells has no
/// such blind spot, and the reduction still decides block coverage.
///
/// The window is read one cell wider than `bounds` where the globe allows, so
/// cells on the window edge are classified against real neighbours rather than
/// against absence.
pub fn coastal_demand(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<RefinementDemand> {
    let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
    let halo = halo_within_source(bounds, gridnum_perdegree, 1);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;
    let is_land = |lon: usize, lat: usize| window.value_at_global(lon, lat).map(|value| value != 0);

    demand.fill_par(|lon, lat| {
        let Some(here) = is_land(lon, lat) else {
            return false;
        };
        [
            is_land(lon.saturating_sub(1), lat),
            is_land(lon + 1, lat),
            is_land(lon, lat.saturating_sub(1)),
            is_land(lon, lat + 1),
        ]
        .into_iter()
        .flatten()
        .any(|neighbour| neighbour != here)
    });
    Ok(demand)
}

/// Mark every source cell whose neighbourhood holds more than `max_classes`
/// distinct land types.
///
/// This is the criterion the project calls `landcover` (`refine_num_landtypes`
/// / `th_num_landtypes`), and it is resolution-dependent by nature: how many
/// classes fall inside a cell depends on how big the cell is. `radius_cells`
/// stands in for that cell size — the caller sets it from the mesh generation
/// being judged, which is what makes the answer mean anything.
pub fn landcover_heterogeneity_demand(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
    radius_cells: usize,
    max_classes: usize,
) -> io::Result<RefinementDemand> {
    if radius_cells == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "land-cover heterogeneity radius must cover at least one cell",
        ));
    }
    let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
    let halo = halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;

    let sums = ClassPrefixSums::build(&window);
    demand.fill_par(|lon, lat| {
        let mut counts = Vec::new();
        sums.counts_at(lon, lat, radius_cells, &mut counts);
        counts.iter().filter(|count| **count > 0).count() > max_classes
    });
    Ok(demand)
}

/// Mark every source cell whose neighbourhood is a mix of land and sea, by
/// fraction rather than by boundary.
///
/// This is the criterion the namelist calls `th_sea_ratio`: refine where the
/// ocean share of a cell falls strictly inside `[low, high]`, because a cell
/// that is neither all sea nor all land is a coastal cell. Like land-cover
/// heterogeneity it is resolution-dependent — shrink the cell and its share
/// moves toward 0 or 1, so fewer cells qualify — which is the difference from
/// [`coastal_demand`]: that one detects the class boundary itself and gives the
/// same answer at every scale.
pub fn sea_ratio_demand(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
    radius_cells: usize,
    low: f64,
    high: f64,
) -> io::Result<RefinementDemand> {
    if radius_cells == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sea-ratio radius must cover at least one cell",
        ));
    }
    if !(low.is_finite() && high.is_finite()) || low >= high {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sea-ratio bounds must satisfy low < high, got [{low}, {high}]"),
        ));
    }
    let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
    let halo = halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;

    let sums = ClassPrefixSums::build(&window);
    let ocean_plane = sums.classes().iter().position(|class| *class == 0);
    demand.fill_par(|lon, lat| {
        let mut counts = Vec::new();
        let total = sums.counts_at(lon, lat, radius_cells, &mut counts) as usize;
        if total == 0 {
            return false;
        }
        let ocean = ocean_plane.map(|index| counts[index] as usize).unwrap_or(0);
        let ratio = ocean as f64 / total as f64;
        ratio > low && ratio < high
    });
    Ok(demand)
}

/// Mark every source cell whose neighbourhood no single land class dominates.
///
/// The namelist calls this `refine_area_mainland`: refine where the largest
/// class covers less than `min_dominant_share` of the land. Resolution-dependent
/// for the same reason as the others — a smaller cell is more likely to sit
/// inside one class.
pub fn dominant_class_demand(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
    radius_cells: usize,
    min_dominant_share: f64,
) -> io::Result<RefinementDemand> {
    if radius_cells == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "dominant-class radius must cover at least one cell",
        ));
    }
    if !min_dominant_share.is_finite() || !(0.0..=1.0).contains(&min_dominant_share) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("dominant class share must be in 0..=1, got {min_dominant_share}"),
        ));
    }
    let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
    let halo = halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;

    let sums = ClassPrefixSums::build(&window);
    let land_planes: Vec<usize> = sums
        .classes()
        .iter()
        .enumerate()
        .filter_map(|(index, class)| (*class != 0).then_some(index))
        .collect();
    demand.fill_par(|lon, lat| {
        let mut counts = Vec::new();
        sums.counts_at(lon, lat, radius_cells, &mut counts);
        let land: usize = land_planes
            .iter()
            .map(|index| counts[*index] as usize)
            .sum();
        if land == 0 {
            return false;
        }
        let dominant = land_planes
            .iter()
            .map(|index| counts[*index] as usize)
            .max()
            .unwrap_or(0);
        (dominant as f64 / land as f64) < min_dominant_share
    });
    Ok(demand)
}
