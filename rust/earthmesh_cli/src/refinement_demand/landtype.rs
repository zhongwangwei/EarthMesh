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

use super::class_counts::ClassCounts;
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
        minlat_source: bounds.minlat_source.saturating_add(cells).min(nlats_source),
    }
}

fn periodic_halo_windows(
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    cells: usize,
) -> Vec<AreaJudgeSourceBounds> {
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    let nlats_source = gridnum_perdegree.saturating_mul(180);
    let lat_bounds = (
        bounds.maxlat_source.saturating_sub(cells).max(1),
        bounds.minlat_source.saturating_add(cells).min(nlats_source),
    );
    if cells >= nlons_source {
        return vec![AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: nlons_source,
            maxlat_source: lat_bounds.0,
            minlat_source: lat_bounds.1,
        }];
    }
    let mut windows = vec![AreaJudgeSourceBounds {
        minlon_source: bounds.minlon_source.saturating_sub(cells).max(1),
        maxlon_source: bounds.maxlon_source.saturating_add(cells).min(nlons_source),
        maxlat_source: lat_bounds.0,
        minlat_source: lat_bounds.1,
    }];
    if bounds.minlon_source <= cells {
        let missing = cells - bounds.minlon_source + 1;
        windows.push(AreaJudgeSourceBounds {
            minlon_source: nlons_source - missing + 1,
            maxlon_source: nlons_source,
            maxlat_source: lat_bounds.0,
            minlat_source: lat_bounds.1,
        });
    }
    if bounds.maxlon_source.saturating_add(cells) > nlons_source {
        windows.push(AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: bounds.maxlon_source.saturating_add(cells) - nlons_source,
            maxlat_source: lat_bounds.0,
            minlat_source: lat_bounds.1,
        });
    }
    windows
}

struct PeriodicLandtypeLookup {
    windows: Vec<crate::mkgrd_data_preprocess_source::LandtypeWindow>,
    nlons_source: usize,
}

impl PeriodicLandtypeLookup {
    fn read(
        landtype_file: impl AsRef<Path>,
        gridnum_perdegree: usize,
        bounds: AreaJudgeSourceBounds,
        radius_cells: usize,
    ) -> io::Result<Self> {
        let landtype_file = landtype_file.as_ref();
        let windows = periodic_halo_windows(bounds, gridnum_perdegree, radius_cells)
            .into_iter()
            .map(|halo| read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self {
            windows,
            nlons_source: gridnum_perdegree.saturating_mul(360),
        })
    }

    fn value_at_global(&self, lon_index: isize, lat_index: usize) -> Option<i8> {
        let lon_index = wrap_lon_index(lon_index, self.nlons_source);
        self.windows
            .iter()
            .find_map(|window| window.value_at_global(lon_index, lat_index))
    }

    fn classes_and_planes(&self) -> (Vec<i8>, Vec<u16>) {
        let mut present = vec![false; 256];
        for window in &self.windows {
            for value in &window.values {
                present[*value as isize as usize & 0xff] = true;
            }
        }
        let mut classes = Vec::new();
        for raw in 0..256usize {
            if present[raw] {
                classes.push(raw as u8 as i8);
            }
        }
        let mut plane_of = vec![u16::MAX; 256];
        for (index, class) in classes.iter().enumerate() {
            plane_of[*class as isize as usize & 0xff] = index as u16;
        }
        (classes, plane_of)
    }
}

fn wrap_lon_index(lon_index: isize, nlons_source: usize) -> usize {
    if nlons_source == 0 {
        return lon_index.max(1) as usize;
    }
    (lon_index - 1).rem_euclid(nlons_source as isize) as usize + 1
}

fn periodic_row_counts(
    lookup: &PeriodicLandtypeLookup,
    lat_index: usize,
    lon_from: usize,
    lon_to: usize,
    radius_cells: usize,
    classes: &[i8],
    plane_of: &[u16],
    out: &mut Vec<u32>,
    totals: &mut Vec<u32>,
) {
    let class_count = classes.len();
    let width = lon_to.saturating_sub(lon_from) + 1;
    out.clear();
    out.resize(width * class_count, 0);
    totals.clear();
    totals.resize(width, 0);
    if class_count == 0 {
        return;
    }

    let Some(first) = lookup.windows.first() else {
        return;
    };
    let lat_lo = lat_index
        .saturating_sub(radius_cells)
        .max(first.bounds.maxlat_source);
    let lat_hi = lat_index
        .saturating_add(radius_cells)
        .min(first.bounds.minlat_source);
    if lat_lo > lat_hi {
        return;
    }

    let scan_lo = lon_from as isize - radius_cells as isize;
    let scan_hi = lon_to as isize + radius_cells as isize;
    let scan_width = (scan_hi - scan_lo + 1) as usize;
    let mut column = vec![0u32; scan_width * class_count];
    let mut column_total = vec![0u32; scan_width];
    for logical_lon in scan_lo..=scan_hi {
        let slot = (logical_lon - scan_lo) as usize;
        for lat in lat_lo..=lat_hi {
            let Some(value) = lookup.value_at_global(logical_lon, lat) else {
                continue;
            };
            let plane = plane_of[value as isize as usize & 0xff] as usize;
            column[slot * class_count + plane] += 1;
            column_total[slot] += 1;
        }
    }

    let mut running = vec![0u32; class_count];
    let mut running_total = 0u32;
    let mut covered_lo = scan_lo;
    let mut covered_hi = scan_lo;
    let mut primed = false;
    for lon in lon_from..=lon_to {
        let want_lo = lon as isize - radius_cells as isize;
        let want_hi = lon as isize + radius_cells as isize;
        if !primed {
            for column_index in want_lo..=want_hi {
                let slot = (column_index - scan_lo) as usize;
                for plane in 0..class_count {
                    running[plane] += column[slot * class_count + plane];
                }
                running_total += column_total[slot];
            }
            covered_lo = want_lo;
            covered_hi = want_hi;
            primed = true;
        } else {
            while covered_hi < want_hi {
                covered_hi += 1;
                let slot = (covered_hi - scan_lo) as usize;
                for plane in 0..class_count {
                    running[plane] += column[slot * class_count + plane];
                }
                running_total += column_total[slot];
            }
            while covered_lo < want_lo {
                let slot = (covered_lo - scan_lo) as usize;
                for plane in 0..class_count {
                    running[plane] -= column[slot * class_count + plane];
                }
                running_total -= column_total[slot];
                covered_lo += 1;
            }
        }
        let base = (lon - lon_from) * class_count;
        out[base..base + class_count].copy_from_slice(&running);
        totals[lon - lon_from] = running_total;
    }
}

fn crosses_periodic_lon_halo(
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    radius_cells: usize,
) -> bool {
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    bounds.minlon_source <= radius_cells
        || bounds.maxlon_source.saturating_add(radius_cells) > nlons_source
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
    let window = PeriodicLandtypeLookup::read(landtype_file, gridnum_perdegree, bounds, 1)?;
    let is_land = |lon: isize, lat: usize| window.value_at_global(lon, lat).map(|value| value != 0);

    demand.fill_par(|lon, lat| {
        let lon = lon as isize;
        let Some(here) = is_land(lon, lat) else {
            return false;
        };
        [
            is_land(lon - 1, lat),
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
    if crosses_periodic_lon_halo(bounds, gridnum_perdegree, radius_cells) {
        let window =
            PeriodicLandtypeLookup::read(landtype_file, gridnum_perdegree, bounds, radius_cells)?;
        let (classes, plane_of) = window.classes_and_planes();
        let class_count = classes.len();
        demand.fill_rows_par(|lat, lon_from, lon_to, row| {
            let (mut cells, mut totals) = (Vec::new(), Vec::new());
            periodic_row_counts(
                &window,
                lat,
                lon_from,
                lon_to,
                radius_cells,
                &classes,
                &plane_of,
                &mut cells,
                &mut totals,
            );
            row.clear();
            for index in 0..totals.len() {
                let base = index * class_count;
                let present = cells[base..base + class_count]
                    .iter()
                    .filter(|count| **count > 0)
                    .count();
                row.push(present > max_classes);
            }
        });
        return Ok(demand);
    }
    let halo = halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;

    let counts = ClassCounts::build(&window);
    let class_count = counts.classes().len();
    demand.fill_rows_par(|lat, lon_from, lon_to, row| {
        let (mut cells, mut totals) = (Vec::new(), Vec::new());
        counts.row_counts(lat, lon_from, lon_to, radius_cells, &mut cells, &mut totals);
        row.clear();
        for index in 0..totals.len() {
            let base = index * class_count;
            let present = cells[base..base + class_count]
                .iter()
                .filter(|count| **count > 0)
                .count();
            row.push(present > max_classes);
        }
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
    if crosses_periodic_lon_halo(bounds, gridnum_perdegree, radius_cells) {
        let window =
            PeriodicLandtypeLookup::read(landtype_file, gridnum_perdegree, bounds, radius_cells)?;
        let (classes, plane_of) = window.classes_and_planes();
        let class_count = classes.len();
        let ocean_plane = classes.iter().position(|class| *class == 0);
        demand.fill_rows_par(|lat, lon_from, lon_to, row| {
            let (mut cells, mut totals) = (Vec::new(), Vec::new());
            periodic_row_counts(
                &window,
                lat,
                lon_from,
                lon_to,
                radius_cells,
                &classes,
                &plane_of,
                &mut cells,
                &mut totals,
            );
            row.clear();
            for (index, total) in totals.iter().enumerate() {
                if *total == 0 {
                    row.push(false);
                    continue;
                }
                let ocean = ocean_plane
                    .map(|plane| cells[index * class_count + plane] as usize)
                    .unwrap_or(0);
                let ratio = ocean as f64 / *total as f64;
                row.push(ratio > low && ratio < high);
            }
        });
        return Ok(demand);
    }
    let halo = halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;

    let counts = ClassCounts::build(&window);
    let class_count = counts.classes().len();
    let ocean_plane = counts.classes().iter().position(|class| *class == 0);
    demand.fill_rows_par(|lat, lon_from, lon_to, row| {
        let (mut cells, mut totals) = (Vec::new(), Vec::new());
        counts.row_counts(lat, lon_from, lon_to, radius_cells, &mut cells, &mut totals);
        row.clear();
        for (index, total) in totals.iter().enumerate() {
            if *total == 0 {
                row.push(false);
                continue;
            }
            let ocean = ocean_plane
                .map(|plane| cells[index * class_count + plane] as usize)
                .unwrap_or(0);
            let ratio = ocean as f64 / *total as f64;
            row.push(ratio > low && ratio < high);
        }
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
    if crosses_periodic_lon_halo(bounds, gridnum_perdegree, radius_cells) {
        let window =
            PeriodicLandtypeLookup::read(landtype_file, gridnum_perdegree, bounds, radius_cells)?;
        let (classes, plane_of) = window.classes_and_planes();
        let class_count = classes.len();
        let land_planes: Vec<usize> = classes
            .iter()
            .enumerate()
            .filter_map(|(index, class)| (*class != 0).then_some(index))
            .collect();
        demand.fill_rows_par(|lat, lon_from, lon_to, row| {
            let (mut cells, mut totals) = (Vec::new(), Vec::new());
            periodic_row_counts(
                &window,
                lat,
                lon_from,
                lon_to,
                radius_cells,
                &classes,
                &plane_of,
                &mut cells,
                &mut totals,
            );
            row.clear();
            for index in 0..totals.len() {
                let base = index * class_count;
                let land: usize = land_planes
                    .iter()
                    .map(|plane| cells[base + plane] as usize)
                    .sum();
                if land == 0 {
                    row.push(false);
                    continue;
                }
                let dominant = land_planes
                    .iter()
                    .map(|plane| cells[base + plane] as usize)
                    .max()
                    .unwrap_or(0);
                row.push((dominant as f64 / land as f64) < min_dominant_share);
            }
        });
        return Ok(demand);
    }
    let halo = halo_within_source(bounds, gridnum_perdegree, radius_cells);
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, halo)?;

    let counts = ClassCounts::build(&window);
    let class_count = counts.classes().len();
    let land_planes: Vec<usize> = counts
        .classes()
        .iter()
        .enumerate()
        .filter_map(|(index, class)| (*class != 0).then_some(index))
        .collect();
    demand.fill_rows_par(|lat, lon_from, lon_to, row| {
        let (mut cells, mut totals) = (Vec::new(), Vec::new());
        counts.row_counts(lat, lon_from, lon_to, radius_cells, &mut cells, &mut totals);
        row.clear();
        for index in 0..totals.len() {
            let base = index * class_count;
            let land: usize = land_planes
                .iter()
                .map(|plane| cells[base + plane] as usize)
                .sum();
            if land == 0 {
                row.push(false);
                continue;
            }
            let dominant = land_planes
                .iter()
                .map(|plane| cells[base + plane] as usize)
                .max()
                .unwrap_or(0);
            row.push((dominant as f64 / land as f64) < min_dominant_share);
        }
    });
    Ok(demand)
}
