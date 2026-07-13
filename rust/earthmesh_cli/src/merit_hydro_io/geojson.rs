use std::fs;
use std::io::{self, Write};
use std::path::Path;

use super::classify::classify_merit_hydro_window_with_adjacency;
use super::types::{
    MeritHydroGeoJsonLayerWriteReport, MeritHydroWindowReport, MeritMaskThresholds,
};
use crate::{json_escape_string, json_number, require_len};

/// Write native MERIT-Hydro classified windows as combined and split GeoJSON layers.
pub fn write_merit_hydro_mask_geojson_layers(
    windows: &[MeritHydroWindowReport],
    thresholds: MeritMaskThresholds,
    output_dir: impl AsRef<Path>,
    include_surface_masks: bool,
) -> io::Result<MeritHydroGeoJsonLayerWriteReport> {
    let output_dir = output_dir.as_ref().to_path_buf();
    fs::create_dir_all(&output_dir)?;
    let combined_geojson = output_dir.join("merit_masks.geojson");
    let river_geojson = output_dir.join("merit_river_masks.geojson");
    let coast_geojson = output_dir.join("merit_coast_masks.geojson");
    let surface_geojson =
        include_surface_masks.then(|| output_dir.join("merit_surface_masks.geojson"));
    let summary_json = output_dir.join("merit_mask_summary.json");

    let mut combined_features = Vec::new();
    let mut river_features = Vec::new();
    let mut coast_features = Vec::new();
    let mut surface_features = Vec::new();
    let mut mask_counts = std::collections::BTreeMap::<String, usize>::new();
    let coast_adjacency = global_coast_adjacency(windows)?;

    for (window_index, window) in windows.iter().enumerate() {
        let classification = classify_merit_hydro_window_with_adjacency(
            window,
            thresholds,
            &coast_adjacency[window_index],
        )?;
        for lon_index in 0..window.width {
            for lat_index in 0..window.height {
                let offset = lon_index * window.height + lat_index;
                let class = &classification.classes[offset];
                if class == "UNKNOWN" {
                    continue;
                }
                *mask_counts.entry(class.clone()).or_insert(0) += 1;
                if matches!(class.as_str(), "LAND" | "OCEAN") && !include_surface_masks {
                    continue;
                }
                let feature = merit_mask_feature_json(window, lon_index, lat_index, class);
                combined_features.push(feature.clone());
                match class.as_str() {
                    "R2" | "R3" => river_features.push(feature),
                    "COAST_LAND" | "COAST_OCEAN" => coast_features.push(feature),
                    "LAND" | "OCEAN" => surface_features.push(feature),
                    _ => {}
                }
            }
        }
    }

    write_geojson_feature_collection(&combined_geojson, &combined_features)?;
    write_geojson_feature_collection(&river_geojson, &river_features)?;
    write_geojson_feature_collection(&coast_geojson, &coast_features)?;
    if let Some(path) = &surface_geojson {
        write_geojson_feature_collection(path, &surface_features)?;
    }
    write_merit_mask_summary_json(
        &summary_json,
        windows.len(),
        combined_features.len(),
        &mask_counts,
        thresholds,
    )?;

    Ok(MeritHydroGeoJsonLayerWriteReport {
        output_dir,
        combined_geojson,
        river_geojson,
        coast_geojson,
        surface_geojson,
        summary_json,
        window_count: windows.len(),
        combined_feature_count: combined_features.len(),
        river_feature_count: river_features.len(),
        coast_feature_count: coast_features.len(),
        surface_feature_count: surface_features.len(),
        mask_counts,
    })
}

fn global_coast_adjacency(windows: &[MeritHydroWindowReport]) -> io::Result<Vec<Vec<bool>>> {
    let lon_fallback = minimum_axis_step(windows, |window| &window.lon);
    let lat_fallback = minimum_axis_step(windows, |window| &window.lat);
    let mut surfaces = std::collections::HashMap::<(i64, i64), i8>::new();

    for window in windows {
        if window.sampling_stride != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "MERIT-Hydro coast export requires native stride 1; got {} for {}",
                    window.sampling_stride, window.tile_name
                ),
            ));
        }
        require_len("longitude", window.lon.len(), window.width)?;
        require_len("latitude", window.lat.len(), window.height)?;
        let expected = window.width.checked_mul(window.height).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "MERIT-Hydro window shape overflows usize",
            )
        })?;
        require_len("landtype_igbp", window.landtype_igbp.len(), expected)?;
        for lon_index in 0..window.width {
            for lat_index in 0..window.height {
                let offset = lon_index * window.height + lat_index;
                let key = coordinate_key(window.lon[lon_index], window.lat[lat_index])?;
                let surface = surface_code(window.landtype_igbp[offset]);
                if let Some(previous) = surfaces.insert(key, surface) {
                    if previous != surface {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "overlapping MERIT-Hydro windows disagree on land/ocean surface",
                        ));
                    }
                }
            }
        }
    }

    windows
        .iter()
        .map(|window| {
            let mut adjacency = vec![false; window.width * window.height];
            for lon_index in 0..window.width {
                let dlon = axis_step(&window.lon, lon_index, lon_fallback)?;
                for lat_index in 0..window.height {
                    let offset = lon_index * window.height + lat_index;
                    let surface = surface_code(window.landtype_igbp[offset]);
                    if surface == 0 {
                        continue;
                    }
                    let dlat = axis_step(&window.lat, lat_index, lat_fallback)?;
                    'neighbors: for dx in -1..=1 {
                        for dy in -1..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let key = coordinate_key(
                                window.lon[lon_index] + f64::from(dx) * dlon,
                                window.lat[lat_index] + f64::from(dy) * dlat,
                            )?;
                            if surfaces
                                .get(&key)
                                .is_some_and(|neighbor| *neighbor == -surface)
                            {
                                adjacency[offset] = true;
                                break 'neighbors;
                            }
                        }
                    }
                }
            }
            Ok(adjacency)
        })
        .collect()
}

fn minimum_axis_step(
    windows: &[MeritHydroWindowReport],
    axis: impl Fn(&MeritHydroWindowReport) -> &[f64],
) -> f64 {
    windows
        .iter()
        .flat_map(|window| axis(window).windows(2))
        .map(|pair| (pair[1] - pair[0]).abs())
        .filter(|step| step.is_finite() && *step > 0.0)
        .reduce(f64::min)
        .unwrap_or(1.0 / 1200.0)
}

fn axis_step(values: &[f64], index: usize, fallback: f64) -> io::Result<f64> {
    let step = if values.len() <= 1 {
        fallback
    } else if index == 0 {
        (values[1] - values[0]).abs()
    } else {
        (values[index] - values[index - 1]).abs()
    };
    if !step.is_finite() || step <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MERIT-Hydro coordinates must have a finite positive native step",
        ));
    }
    Ok(step)
}

fn coordinate_key(lon: f64, lat: f64) -> io::Result<(i64, i64)> {
    if !lon.is_finite() || !lat.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MERIT-Hydro coordinates must be finite",
        ));
    }
    let lon = (lon + 180.0).rem_euclid(360.0) - 180.0;
    Ok(((lon * 1e9).round() as i64, (lat * 1e9).round() as i64))
}

fn surface_code(landtype_igbp: i32) -> i8 {
    if landtype_igbp == 0 || landtype_igbp == 17 {
        -1
    } else if landtype_igbp > 0 {
        1
    } else {
        0
    }
}

fn merit_mask_feature_json(
    window: &MeritHydroWindowReport,
    lon_index: usize,
    lat_index: usize,
    mask_class: &str,
) -> String {
    let offset = lon_index * window.height + lat_index;
    let lon = window.lon[lon_index];
    let lat = window.lat[lat_index];
    let dlon = merit_cell_delta(&window.lon, lon_index);
    let dlat = merit_cell_delta(&window.lat, lat_index);
    let lon0 = lon - dlon / 2.0;
    let lon1 = lon + dlon / 2.0;
    let lat0 = lat - dlat / 2.0;
    let lat1 = lat + dlat / 2.0;
    let stem = window
        .tile
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| window.tile_name.trim_end_matches(".nc").to_string());
    let feature_id = format!("{stem}:{lon_index}:{lat_index}:{mask_class}");
    format!(
        "{{\"type\":\"Feature\",\"geometry\":{{\"type\":\"Polygon\",\"coordinates\":[[[{},{}],[{},{}],[{},{}],[{},{}],[{},{}]]]}},\"properties\":{{\"elevation_m\":{},\"feature_id\":\"{}\",\"landtype_igbp\":{},\"mask_class\":\"{}\",\"source\":\"MERIT-Hydro\",\"tile\":\"{}\",\"upstream_area_km2\":{},\"width_m\":{}}}}}",
        json_number(lon0),
        json_number(lat0),
        json_number(lon1),
        json_number(lat0),
        json_number(lon1),
        json_number(lat1),
        json_number(lon0),
        json_number(lat1),
        json_number(lon0),
        json_number(lat0),
        json_number(window.elv_m[offset]),
        json_escape_string(&feature_id),
        window.landtype_igbp[offset],
        json_escape_string(mask_class),
        json_escape_string(&window.tile_name),
        json_number(window.upa_km2[offset]),
        json_number(window.width_m[offset]),
    )
}

fn merit_cell_delta(values: &[f64], index: usize) -> f64 {
    if values.len() <= 1 {
        return 0.000_833_333_333_333_333_4;
    }
    if index == 0 {
        (values[1] - values[0]).abs()
    } else {
        (values[index] - values[index - 1]).abs()
    }
}

fn write_geojson_feature_collection(path: &Path, features: &[String]) -> io::Result<()> {
    let mut handle = fs::File::create(path)?;
    write!(handle, "{{\"type\":\"FeatureCollection\",\"features\":[")?;
    for (index, feature) in features.iter().enumerate() {
        if index > 0 {
            write!(handle, ",")?;
        }
        write!(handle, "{feature}")?;
    }
    writeln!(handle, "]}}")?;
    Ok(())
}

fn write_merit_mask_summary_json(
    path: &Path,
    tile_count: usize,
    feature_count: usize,
    mask_counts: &std::collections::BTreeMap<String, usize>,
    thresholds: MeritMaskThresholds,
) -> io::Result<()> {
    let river_count =
        mask_counts.get("R2").copied().unwrap_or(0) + mask_counts.get("R3").copied().unwrap_or(0);
    let coast_count = mask_counts.get("COAST_LAND").copied().unwrap_or(0)
        + mask_counts.get("COAST_OCEAN").copied().unwrap_or(0);
    let hydro_coast_score = if feature_count > 0 {
        (river_count + coast_count) as f64 / feature_count as f64
    } else {
        0.0
    };
    let mut text = format!(
        "{{\"feature_count\":{},\"hydro_coast_score\":{},\"mask_counts\":{{",
        feature_count,
        json_number(hydro_coast_score)
    );
    for (index, (class, count)) in mask_counts.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push('"');
        text.push_str(&json_escape_string(class));
        text.push_str("\":");
        text.push_str(&count.to_string());
    }
    text.push_str(&format!(
        "}},\"thresholds\":{{\"r2_upa_km2\":{},\"r2_width_m\":{},\"r3_upa_km2\":{},\"r3_width_m\":{}}},\"tile_count\":{}}}\n",
        json_number(thresholds.r2_upa_km2),
        json_number(thresholds.r2_width_m),
        json_number(thresholds.r3_upa_km2),
        json_number(thresholds.r3_width_m),
        tile_count,
    ));
    fs::write(path, text)
}
