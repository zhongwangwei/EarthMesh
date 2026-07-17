use std::fs;
use std::io::{self, Write};
use std::path::Path;

use super::classify::{classify_merit_river, classify_merit_surface};
use super::types::{
    MeritHydroGeoJsonLayerWriteReport, MeritHydroWindowReport, MeritMaskThresholds,
};
use crate::{json_escape_string, json_number, require_len};

/// Write native MERIT-Hydro classified windows as GeoJSON layers. CLI exports
/// request the detailed split layers; Project execution can skip those duplicate
/// files and losslessly compact consecutive same-class raster cells into disjoint
/// corridor rectangles for the downstream conservative overlay.
pub fn write_merit_hydro_mask_geojson_layers(
    windows: &[MeritHydroWindowReport],
    thresholds: MeritMaskThresholds,
    output_dir: impl AsRef<Path>,
    include_surface_masks: bool,
    write_split_corridor_layers: bool,
) -> io::Result<MeritHydroGeoJsonLayerWriteReport> {
    let output_dir = output_dir.as_ref().to_path_buf();
    fs::create_dir_all(&output_dir)?;
    let combined_geojson = output_dir.join("merit_masks.geojson");
    let river_geojson = output_dir.join("merit_river_masks.geojson");
    let coast_geojson = output_dir.join("merit_coast_masks.geojson");
    let surface_geojson =
        include_surface_masks.then(|| output_dir.join("merit_surface_masks.geojson"));
    let summary_json = output_dir.join("merit_mask_summary.json");

    let mut combined_writer = FeatureCollectionWriter::create(&combined_geojson)?;
    let mut river_writer = write_split_corridor_layers
        .then(|| FeatureCollectionWriter::create(&river_geojson))
        .transpose()?;
    let mut coast_writer = write_split_corridor_layers
        .then(|| FeatureCollectionWriter::create(&coast_geojson))
        .transpose()?;
    let mut surface_writer = surface_geojson
        .as_ref()
        .map(|path| FeatureCollectionWriter::create(path))
        .transpose()?;
    let mut mask_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut river_feature_count = 0;
    let mut coast_feature_count = 0;
    let mut surface_feature_count = 0;
    let coast_adjacency = global_coast_adjacency(windows)?;
    let compact_project_corridors = !write_split_corridor_layers && !include_surface_masks;

    for (window_index, window) in windows.iter().enumerate() {
        let expected = window.width.checked_mul(window.height).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "MERIT-Hydro window shape overflows usize",
            )
        })?;
        for (name, len) in [
            ("upa", window.upa_km2.len()),
            ("elv", window.elv_m.len()),
            ("wth", window.width_m.len()),
            ("landtype_igbp", window.landtype_igbp.len()),
            ("coast adjacency", coast_adjacency[window_index].len()),
        ] {
            require_len(name, len, expected)?;
        }
        if compact_project_corridors {
            for offset in 0..expected {
                if let Some(class) =
                    classify_merit_river(window.width_m[offset], window.upa_km2[offset], thresholds)
                {
                    increment_mask_count(&mut mask_counts, class);
                }
                increment_mask_count(
                    &mut mask_counts,
                    classify_merit_surface(
                        window.landtype_igbp[offset],
                        coast_adjacency[window_index][offset],
                    ),
                );
            }
            for lon_index in 0..window.width {
                for river_pass in [true, false] {
                    let mut run_state: Option<(&'static str, bool, bool)> = None;
                    let mut run_start = 0;
                    for lat_index in 0..=window.height {
                        let state = if lat_index < window.height {
                            let offset = lon_index * window.height + lat_index;
                            if river_pass {
                                classify_merit_river(
                                    window.width_m[offset],
                                    window.upa_km2[offset],
                                    thresholds,
                                )
                                .map(|class| {
                                    (
                                        class,
                                        window.width_m[offset].is_finite()
                                            && window.width_m[offset]
                                                >= thresholds.river_width_refinement_m,
                                        window.upa_km2[offset].is_finite()
                                            && window.upa_km2[offset]
                                                >= thresholds.river_upstream_area_refinement_km2,
                                    )
                                })
                            } else {
                                let surface = classify_merit_surface(
                                    window.landtype_igbp[offset],
                                    coast_adjacency[window_index][offset],
                                );
                                matches!(surface, "COAST_LAND" | "COAST_OCEAN")
                                    .then_some((surface, false, false))
                            }
                        } else {
                            None
                        };
                        if state == run_state {
                            continue;
                        }
                        if let Some((previous, width_triggered, upstream_area_triggered)) =
                            run_state
                        {
                            let feature = merit_mask_run_feature_json(
                                window,
                                lon_index,
                                run_start,
                                lat_index - 1,
                                previous,
                                width_triggered,
                                upstream_area_triggered,
                            );
                            combined_writer.write_feature(&feature)?;
                            if river_pass {
                                river_feature_count += 1;
                            } else {
                                coast_feature_count += 1;
                            }
                        }
                        run_state = state;
                        run_start = lat_index;
                    }
                }
            }
            continue;
        }
        for lon_index in 0..window.width {
            for lat_index in 0..window.height {
                let offset = lon_index * window.height + lat_index;
                if let Some(class) =
                    classify_merit_river(window.width_m[offset], window.upa_km2[offset], thresholds)
                {
                    increment_mask_count(&mut mask_counts, class);
                    let feature = merit_mask_feature_json(window, lon_index, lat_index, class);
                    combined_writer.write_feature(&feature)?;
                    river_feature_count += 1;
                    if let Some(writer) = &mut river_writer {
                        writer.write_feature(&feature)?;
                    }
                }
                let surface = classify_merit_surface(
                    window.landtype_igbp[offset],
                    coast_adjacency[window_index][offset],
                );
                increment_mask_count(&mut mask_counts, surface);
                match surface {
                    "COAST_LAND" | "COAST_OCEAN" => {
                        let feature =
                            merit_mask_feature_json(window, lon_index, lat_index, surface);
                        combined_writer.write_feature(&feature)?;
                        coast_feature_count += 1;
                        if let Some(writer) = &mut coast_writer {
                            writer.write_feature(&feature)?;
                        }
                    }
                    "LAND" | "OCEAN" if include_surface_masks => {
                        let feature =
                            merit_mask_feature_json(window, lon_index, lat_index, surface);
                        combined_writer.write_feature(&feature)?;
                        surface_feature_count += 1;
                        if let Some(writer) = &mut surface_writer {
                            writer.write_feature(&feature)?;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let combined_feature_count = combined_writer.finish()?;
    river_writer
        .map(FeatureCollectionWriter::finish)
        .transpose()?;
    coast_writer
        .map(FeatureCollectionWriter::finish)
        .transpose()?;
    surface_writer
        .map(FeatureCollectionWriter::finish)
        .transpose()?;
    write_merit_mask_summary_json(
        &summary_json,
        windows.len(),
        combined_feature_count,
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
        combined_feature_count,
        river_feature_count,
        coast_feature_count,
        surface_feature_count,
        mask_counts,
    })
}

fn increment_mask_count(mask_counts: &mut std::collections::BTreeMap<String, usize>, class: &str) {
    if class == "UNKNOWN" {
        return;
    }
    if let Some(count) = mask_counts.get_mut(class) {
        *count += 1;
    } else {
        mask_counts.insert(class.to_string(), 1);
    }
}

struct FeatureCollectionWriter {
    file: fs::File,
    feature_count: usize,
}

impl FeatureCollectionWriter {
    fn create(path: &Path) -> io::Result<Self> {
        let mut file = fs::File::create(path)?;
        write!(file, "{{\"type\":\"FeatureCollection\",\"features\":[")?;
        Ok(Self {
            file,
            feature_count: 0,
        })
    }

    fn write_feature(&mut self, feature: &str) -> io::Result<()> {
        if self.feature_count > 0 {
            self.file.write_all(b",")?;
        }
        self.file.write_all(feature.as_bytes())?;
        self.feature_count += 1;
        Ok(())
    }

    fn finish(mut self) -> io::Result<usize> {
        self.file.write_all(b"]}\n")?;
        Ok(self.feature_count)
    }
}

fn global_coast_adjacency(windows: &[MeritHydroWindowReport]) -> io::Result<Vec<Vec<bool>>> {
    let lon_fallback = minimum_axis_step(windows, |window| &window.lon);
    let lat_fallback = minimum_axis_step(windows, |window| &window.lat);
    let mut boundary_surfaces = std::collections::HashMap::<(i64, i64), i8>::new();

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
        for (lon_index, lat_index) in boundary_indices(window.width, window.height) {
            let offset = lon_index * window.height + lat_index;
            let key = coordinate_key(window.lon[lon_index], window.lat[lat_index])?;
            let surface = surface_code(window.landtype_igbp[offset]);
            if let Some(previous) = boundary_surfaces.insert(key, surface) {
                if previous != surface {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "overlapping MERIT-Hydro windows disagree on land/ocean surface",
                    ));
                }
            }
        }
    }

    let adjacency_for_window = |window: &MeritHydroWindowReport| -> io::Result<Vec<bool>> {
        let mut adjacency = local_coast_adjacency(window);
        for (lon_index, lat_index) in boundary_indices(window.width, window.height) {
            let offset = lon_index * window.height + lat_index;
            let surface = surface_code(window.landtype_igbp[offset]);
            if surface == 0 || adjacency[offset] {
                continue;
            }
            let dlon = axis_step(&window.lon, lon_index, lon_fallback)?;
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
                    if boundary_surfaces
                        .get(&key)
                        .is_some_and(|neighbor| *neighbor == -surface)
                    {
                        adjacency[offset] = true;
                        break 'neighbors;
                    }
                }
            }
        }
        Ok(adjacency)
    };
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(windows.len().max(1));
    let chunk_size = windows.len().div_ceil(worker_count);
    std::thread::scope(|scope| {
        let handles = windows
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(&adjacency_for_window)
                        .collect::<io::Result<Vec<_>>>()
                })
            })
            .collect::<Vec<_>>();
        let mut all = Vec::with_capacity(windows.len());
        for handle in handles {
            let chunk = handle
                .join()
                .map_err(|_| io::Error::other("MERIT-Hydro coast adjacency worker panicked"))??;
            all.extend(chunk);
        }
        Ok(all)
    })
}

fn local_coast_adjacency(window: &MeritHydroWindowReport) -> Vec<bool> {
    let mut adjacency = vec![false; window.width * window.height];
    const FORWARD_NEIGHBORS: [(isize, isize); 4] = [(0, 1), (1, -1), (1, 0), (1, 1)];
    for lon_index in 0..window.width {
        for lat_index in 0..window.height {
            let offset = lon_index * window.height + lat_index;
            let surface = surface_code(window.landtype_igbp[offset]);
            if surface == 0 {
                continue;
            }
            for (dx, dy) in FORWARD_NEIGHBORS {
                let next_lon = lon_index as isize + dx;
                let next_lat = lat_index as isize + dy;
                if next_lon < 0
                    || next_lon >= window.width as isize
                    || next_lat < 0
                    || next_lat >= window.height as isize
                {
                    continue;
                }
                let neighbor_offset = next_lon as usize * window.height + next_lat as usize;
                if surface == -surface_code(window.landtype_igbp[neighbor_offset]) {
                    adjacency[offset] = true;
                    adjacency[neighbor_offset] = true;
                }
            }
        }
    }
    adjacency
}

fn boundary_indices(width: usize, height: usize) -> Vec<(usize, usize)> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut indices = Vec::with_capacity(2 * width + 2 * height.saturating_sub(2));
    for lon_index in 0..width {
        indices.push((lon_index, 0));
        if height > 1 {
            indices.push((lon_index, height - 1));
        }
    }
    for lat_index in 1..height.saturating_sub(1) {
        indices.push((0, lat_index));
        if width > 1 {
            indices.push((width - 1, lat_index));
        }
    }
    indices
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

fn merit_mask_run_feature_json(
    window: &MeritHydroWindowReport,
    lon_index: usize,
    start_lat_index: usize,
    end_lat_index: usize,
    mask_class: &str,
    river_width_triggered: bool,
    river_upstream_area_triggered: bool,
) -> String {
    let lon = window.lon[lon_index];
    let dlon = merit_cell_delta(&window.lon, lon_index);
    let lon0 = lon - dlon / 2.0;
    let lon1 = lon + dlon / 2.0;
    let start_lat = window.lat[start_lat_index];
    let end_lat = window.lat[end_lat_index];
    let start_delta = merit_cell_delta(&window.lat, start_lat_index);
    let end_delta = merit_cell_delta(&window.lat, end_lat_index);
    let lat0 = (start_lat - start_delta / 2.0).min(end_lat - end_delta / 2.0);
    let lat1 = (start_lat + start_delta / 2.0).max(end_lat + end_delta / 2.0);
    let stem = window
        .tile
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| window.tile_name.trim_end_matches(".nc").to_string());
    let feature_id = format!("{stem}:{lon_index}:{start_lat_index}-{end_lat_index}:{mask_class}");
    format!(
        "{{\"type\":\"Feature\",\"geometry\":{{\"type\":\"Polygon\",\"coordinates\":[[[{},{}],[{},{}],[{},{}],[{},{}],[{},{}]]]}},\"properties\":{{\"feature_id\":\"{}\",\"mask_class\":\"{}\",\"river_upstream_area_triggered\":{},\"river_width_triggered\":{},\"source\":\"MERIT-Hydro\",\"source_cell_count\":{},\"tile\":\"{}\"}}}}",
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
        json_escape_string(&feature_id),
        json_escape_string(mask_class),
        river_upstream_area_triggered,
        river_width_triggered,
        end_lat_index - start_lat_index + 1,
        json_escape_string(&window.tile_name),
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
        "}},\"thresholds\":{{\"r2_upa_km2\":{},\"r2_width_m\":{},\"r3_upa_km2\":{},\"r3_width_m\":{},\"river_upstream_area_refinement_km2\":{},\"river_width_refinement_m\":{}}},\"tile_count\":{}}}\n",
        json_number(thresholds.r2_upa_km2),
        json_number(thresholds.r2_width_m),
        json_number(thresholds.r3_upa_km2),
        json_number(thresholds.r3_width_m),
        json_number(thresholds.river_upstream_area_refinement_km2),
        json_number(thresholds.river_width_refinement_m),
        tile_count,
    ));
    fs::write(path, text)
}
