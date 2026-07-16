use std::io;

use super::types::{
    MeritHydroMaskClassificationReport, MeritHydroWindowReport, MeritMaskThresholds,
};
use crate::require_len;

/// Classify a native MERIT-Hydro window into river/coast/surface mask classes.
pub fn classify_merit_hydro_window(
    window: &MeritHydroWindowReport,
    thresholds: MeritMaskThresholds,
) -> io::Result<MeritHydroMaskClassificationReport> {
    let adjacency = (0..window.width)
        .flat_map(|lon_index| {
            (0..window.height).map(move |lat_index| {
                merit_cell_adjacent_to_other_surface(window, lon_index, lat_index)
            })
        })
        .collect::<Vec<_>>();
    classify_merit_hydro_window_with_adjacency(window, thresholds, &adjacency)
}

pub(super) fn classify_merit_hydro_window_with_adjacency(
    window: &MeritHydroWindowReport,
    thresholds: MeritMaskThresholds,
    adjacency: &[bool],
) -> io::Result<MeritHydroMaskClassificationReport> {
    if window.sampling_stride != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "MERIT-Hydro coast classification requires native stride 1; got {}",
                window.sampling_stride
            ),
        ));
    }
    let expected = window.width.checked_mul(window.height).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "MERIT-Hydro window shape overflows usize",
        )
    })?;
    for (name, len) in [
        ("dir", window.dir.len()),
        ("upa", window.upa_km2.len()),
        ("elv", window.elv_m.len()),
        ("wth", window.width_m.len()),
        ("landtype_igbp", window.landtype_igbp.len()),
        ("coast adjacency", adjacency.len()),
    ] {
        require_len(name, len, expected)?;
    }

    let mut classes = Vec::with_capacity(expected);
    let mut report = MeritHydroMaskClassificationReport {
        classes: Vec::new(),
        r3_cells: 0,
        r2_cells: 0,
        coast_land_cells: 0,
        coast_ocean_cells: 0,
        land_cells: 0,
        ocean_cells: 0,
        unknown_cells: 0,
    };

    for lon_index in 0..window.width {
        for lat_index in 0..window.height {
            let offset = lon_index * window.height + lat_index;
            let class = classify_merit_cell(
                window.width_m[offset],
                window.upa_km2[offset],
                window.landtype_igbp[offset],
                adjacency[offset],
                thresholds,
            );
            match class {
                "R3" => report.r3_cells += 1,
                "R2" => report.r2_cells += 1,
                "COAST_LAND" => report.coast_land_cells += 1,
                "COAST_OCEAN" => report.coast_ocean_cells += 1,
                "LAND" => report.land_cells += 1,
                "OCEAN" => report.ocean_cells += 1,
                "UNKNOWN" => report.unknown_cells += 1,
                _ => unreachable!("MERIT-Hydro classifier returns a closed class set"),
            }
            classes.push(class.to_string());
        }
    }
    report.classes = classes;
    Ok(report)
}

pub(super) fn classify_merit_cell(
    width_m: f64,
    upa_km2: f64,
    landtype_igbp: i32,
    adjacent_to_other_surface: bool,
    thresholds: MeritMaskThresholds,
) -> &'static str {
    if width_m.is_finite() && upa_km2.is_finite() {
        if width_m >= thresholds.r3_width_m || upa_km2 >= thresholds.r3_upa_km2 {
            return "R3";
        }
        if width_m >= thresholds.r2_width_m || upa_km2 >= thresholds.r2_upa_km2 {
            return "R2";
        }
    }
    if is_merit_ocean_landtype(landtype_igbp) {
        if adjacent_to_other_surface {
            "COAST_OCEAN"
        } else {
            "OCEAN"
        }
    } else if landtype_igbp > 0 {
        if adjacent_to_other_surface {
            "COAST_LAND"
        } else {
            "LAND"
        }
    } else {
        "UNKNOWN"
    }
}

fn merit_cell_adjacent_to_other_surface(
    window: &MeritHydroWindowReport,
    lon_index: usize,
    lat_index: usize,
) -> bool {
    let offset = lon_index * window.height + lat_index;
    let cell_ocean = is_merit_ocean_landtype(window.landtype_igbp[offset]);
    let cell_land = window.landtype_igbp[offset] > 0 && !cell_ocean;
    if !cell_ocean && !cell_land {
        return false;
    }
    let wrap_lon = merit_window_wraps_longitude(window);
    let lat_min = lat_index.saturating_sub(1);
    let lat_max = (lat_index + 1).min(window.height.saturating_sub(1));
    for dx in -1_isize..=1 {
        let candidate_lon = lon_index as isize + dx;
        if !wrap_lon && (candidate_lon < 0 || candidate_lon >= window.width as isize) {
            continue;
        }
        let ni = candidate_lon.rem_euclid(window.width as isize) as usize;
        for nj in lat_min..=lat_max {
            if ni == lon_index && nj == lat_index {
                continue;
            }
            let neighbor = window.landtype_igbp[ni * window.height + nj];
            let neighbor_ocean = is_merit_ocean_landtype(neighbor);
            let neighbor_land = neighbor > 0 && !neighbor_ocean;
            if (cell_land && neighbor_ocean) || (cell_ocean && neighbor_land) {
                return true;
            }
        }
    }
    false
}

fn merit_window_wraps_longitude(window: &MeritHydroWindowReport) -> bool {
    if window.lon.len() < 2 {
        return false;
    }
    let first = window.lon[0];
    let last = *window.lon.last().unwrap_or(&first);
    let step = (window.lon[1] - window.lon[0]).abs();
    first.is_finite()
        && last.is_finite()
        && step.is_finite()
        && (last - first).abs() + step >= 359.0
}

fn is_merit_ocean_landtype(value: i32) -> bool {
    value == 0 || value == 17
}
