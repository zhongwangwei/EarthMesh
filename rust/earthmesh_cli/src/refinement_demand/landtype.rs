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

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        for lon in bounds.minlon_source..=bounds.maxlon_source {
            let Some(here) = is_land(lon, lat) else {
                continue;
            };
            let neighbours = [
                is_land(lon.saturating_sub(1), lat),
                is_land(lon + 1, lat),
                is_land(lon, lat.saturating_sub(1)),
                is_land(lon, lat + 1),
            ];
            if neighbours
                .into_iter()
                .flatten()
                .any(|neighbour| neighbour != here)
            {
                demand.set(lon, lat, true);
            }
        }
    }
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

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        for lon in bounds.minlon_source..=bounds.maxlon_source {
            let mut seen: Vec<i8> = Vec::new();
            for neighbour_lat in lat.saturating_sub(radius_cells)..=lat + radius_cells {
                for neighbour_lon in lon.saturating_sub(radius_cells)..=lon + radius_cells {
                    let Some(value) = window.value_at_global(neighbour_lon, neighbour_lat) else {
                        continue;
                    };
                    if !seen.contains(&value) {
                        seen.push(value);
                    }
                }
            }
            if seen.len() > max_classes {
                demand.set(lon, lat, true);
            }
        }
    }
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

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        for lon in bounds.minlon_source..=bounds.maxlon_source {
            let (mut ocean, mut total) = (0usize, 0usize);
            for neighbour_lat in lat.saturating_sub(radius_cells)..=lat + radius_cells {
                for neighbour_lon in lon.saturating_sub(radius_cells)..=lon + radius_cells {
                    let Some(value) = window.value_at_global(neighbour_lon, neighbour_lat) else {
                        continue;
                    };
                    total += 1;
                    if value == 0 {
                        ocean += 1;
                    }
                }
            }
            if total == 0 {
                continue;
            }
            let ratio = ocean as f64 / total as f64;
            if ratio > low && ratio < high {
                demand.set(lon, lat, true);
            }
        }
    }
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

    for lat in bounds.maxlat_source..=bounds.minlat_source {
        for lon in bounds.minlon_source..=bounds.maxlon_source {
            let mut counts: Vec<(i8, usize)> = Vec::new();
            let mut land = 0usize;
            for neighbour_lat in lat.saturating_sub(radius_cells)..=lat + radius_cells {
                for neighbour_lon in lon.saturating_sub(radius_cells)..=lon + radius_cells {
                    let Some(value) = window.value_at_global(neighbour_lon, neighbour_lat) else {
                        continue;
                    };
                    if value == 0 {
                        continue;
                    }
                    land += 1;
                    match counts.iter_mut().find(|(class, _)| *class == value) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((value, 1)),
                    }
                }
            }
            if land == 0 {
                continue;
            }
            let dominant = counts.iter().map(|(_, count)| *count).max().unwrap_or(0);
            if (dominant as f64 / land as f64) < min_dominant_share {
                demand.set(lon, lat, true);
            }
        }
    }
    Ok(demand)
}
