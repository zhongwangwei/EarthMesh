use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use earthmesh_hfield::great_circle_distance_m;
use serde_json::Value;

use crate::merit_hydro_io::MeritHydroWindowReport;

const KM_PER_DEGREE: f64 = 111.195;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SurfaceSide {
    Land,
    Ocean,
}

#[derive(Clone, Copy)]
struct CoastRect {
    west: f64,
    east: f64,
    south: f64,
    north: f64,
}

impl CoastRect {
    fn distance_km(self, lon: f64, lat: f64) -> f64 {
        let center = (self.west + self.east) / 2.0;
        let lon = center + wrap_delta(lon - center);
        let nearest_lon = lon.clamp(self.west, self.east);
        let nearest_lat = lat.clamp(self.south, self.north);
        great_circle_distance_m(lon, lat, nearest_lon, nearest_lat) / 1_000.0
    }
}

struct CoastRectIndex {
    rects: Vec<CoastRect>,
    buckets: HashMap<(i32, i32), Vec<usize>>,
    bucket_deg: f64,
    lon_bucket_count: i32,
}

impl CoastRectIndex {
    fn new(rects: Vec<CoastRect>, buffer_km: f64) -> Self {
        let bucket_deg = (buffer_km / KM_PER_DEGREE).clamp(0.05, 5.0);
        let lon_bucket_count = (360.0 / bucket_deg).ceil() as i32;
        let mut buckets = HashMap::<(i32, i32), Vec<usize>>::new();
        let lat_pad = buffer_km / KM_PER_DEGREE;
        for (index, rect) in rects.iter().enumerate() {
            let max_abs_lat = (rect.south.abs().max(rect.north.abs()) + lat_pad).min(89.999);
            let lon_pad = (lat_pad / max_abs_lat.to_radians().cos().max(1.0e-6)).min(180.0);
            let lat_start = lat_bucket(rect.south - lat_pad, bucket_deg);
            let lat_end = lat_bucket(rect.north + lat_pad, bucket_deg);
            let lon_start = ((rect.west - lon_pad + 180.0) / bucket_deg).floor() as i32;
            let lon_end = ((rect.east + lon_pad + 180.0) / bucket_deg).floor() as i32;
            let lon_count = (lon_end - lon_start + 1).min(lon_bucket_count);
            for lat_key in lat_start..=lat_end {
                for offset in 0..lon_count {
                    let lon_key = (lon_start + offset).rem_euclid(lon_bucket_count);
                    buckets.entry((lon_key, lat_key)).or_default().push(index);
                }
            }
        }
        Self {
            rects,
            buckets,
            bucket_deg,
            lon_bucket_count,
        }
    }

    fn nearest_km(&self, lon: f64, lat: f64) -> Option<f64> {
        let lon_key = (((wrap_lon(lon) + 180.0) / self.bucket_deg).floor() as i32)
            .rem_euclid(self.lon_bucket_count);
        let lat_key = lat_bucket(lat, self.bucket_deg);
        self.buckets.get(&(lon_key, lat_key)).and_then(|indices| {
            indices
                .iter()
                .map(|&index| self.rects[index].distance_km(lon, lat))
                .reduce(f64::min)
        })
    }
}

/// Write refinement-only distance-band cells. Native COAST_LAND/OCEAN features
/// remain the coupling truth; this file is merged only into the refinement planner.
pub(crate) fn write_project_coast_refinement_cells(
    cells_geojson: &Path,
    merit_corridors_geojson: &Path,
    windows: &[MeritHydroWindowReport],
    buffer_km: f64,
    include_land: bool,
    include_ocean: bool,
    output: &Path,
) -> io::Result<usize> {
    let cells = read_geojson(cells_geojson)?;
    let merit = read_geojson(merit_corridors_geojson)?;
    let rects = coast_rects(&merit)?;
    let index = CoastRectIndex::new(rects, buffer_km);
    let mut output_features = Vec::new();
    for feature in feature_array(&cells)? {
        let Some((lon, lat)) = feature_center(feature) else {
            continue;
        };
        let Some(side) = sample_surface(windows, lon, lat) else {
            continue;
        };
        if (side == SurfaceSide::Land && !include_land)
            || (side == SurfaceSide::Ocean && !include_ocean)
        {
            continue;
        }
        let Some(distance_km) = index.nearest_km(lon, lat) else {
            continue;
        };
        if distance_km <= 1.0e-9 || distance_km > buffer_km {
            continue;
        }
        let mut feature = feature.clone();
        let properties = feature
            .as_object_mut()
            .and_then(|object| object.get_mut("properties"))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("mesh cell feature must have object properties"))?;
        properties.insert(
            "mask_class".to_string(),
            Value::String(
                match side {
                    SurfaceSide::Land => "COAST_DISTANCE_LAND",
                    SurfaceSide::Ocean => "COAST_DISTANCE_OCEAN",
                }
                .to_string(),
            ),
        );
        properties.insert("coast_distance_km".to_string(), Value::from(distance_km));
        properties.insert("coast_buffer_km".to_string(), Value::from(buffer_km));
        properties.insert("refinement_only".to_string(), Value::Bool(true));
        output_features.push(feature);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_vec(&serde_json::json!({
            "type": "FeatureCollection",
            "features": output_features,
        }))?,
    )?;
    Ok(output_features.len())
}

fn read_geojson(path: &Path) -> io::Result<Value> {
    serde_json::from_slice(&fs::read(path)?).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid GeoJSON {}: {error}", path.display()),
        )
    })
}

fn feature_array(root: &Value) -> io::Result<&Vec<Value>> {
    root.get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GeoJSON must contain a features array"))
}

fn feature_center(feature: &Value) -> Option<(f64, f64)> {
    let properties = feature.get("properties")?.as_object()?;
    Some((
        properties.get("center_lon")?.as_f64()?,
        properties.get("center_lat")?.as_f64()?,
    ))
}

fn coast_rects(root: &Value) -> io::Result<Vec<CoastRect>> {
    let mut rects = Vec::new();
    for feature in feature_array(root)? {
        let class = feature
            .pointer("/properties/mask_class")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !matches!(class, "COAST_LAND" | "COAST_OCEAN") {
            continue;
        }
        let ring = feature
            .pointer("/geometry/coordinates/0")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("MERIT coast feature must be a Polygon"))?;
        let Some(first_lon) = ring
            .first()
            .and_then(Value::as_array)
            .and_then(|point| point.first())
            .and_then(Value::as_f64)
        else {
            continue;
        };
        let mut west = f64::INFINITY;
        let mut east = f64::NEG_INFINITY;
        let mut south = f64::INFINITY;
        let mut north = f64::NEG_INFINITY;
        for point in ring.iter().filter_map(Value::as_array) {
            let (Some(lon), Some(lat)) = (
                point.first().and_then(Value::as_f64),
                point.get(1).and_then(Value::as_f64),
            ) else {
                continue;
            };
            let lon = first_lon + wrap_delta(lon - first_lon);
            west = west.min(lon);
            east = east.max(lon);
            south = south.min(lat);
            north = north.max(lat);
        }
        if [west, east, south, north]
            .iter()
            .all(|value| value.is_finite())
        {
            rects.push(CoastRect {
                west,
                east,
                south,
                north,
            });
        }
    }
    Ok(rects)
}

fn sample_surface(windows: &[MeritHydroWindowReport], lon: f64, lat: f64) -> Option<SurfaceSide> {
    for window in windows {
        let Some(lon_index) = nearest_axis_index(&window.lon, lon, true) else {
            continue;
        };
        let Some(lat_index) = nearest_axis_index(&window.lat, lat, false) else {
            continue;
        };
        let value = *window
            .landtype_igbp
            .get(lon_index * window.height + lat_index)?;
        if value == 0 || value == 17 {
            return Some(SurfaceSide::Ocean);
        }
        if value > 0 {
            return Some(SurfaceSide::Land);
        }
    }
    None
}

fn nearest_axis_index(axis: &[f64], value: f64, longitude: bool) -> Option<usize> {
    let first = *axis.first()?;
    if axis.len() == 1 {
        let delta = if longitude {
            wrap_delta(value - first).abs()
        } else {
            (value - first).abs()
        };
        return (delta <= 3.0 / 7_200.0 + 1.0e-9).then_some(0);
    }
    let last = *axis.last()?;
    let center = (first + last) / 2.0;
    let value = if longitude {
        center + wrap_delta(value - center)
    } else {
        value
    };
    let step = (last - first) / (axis.len() - 1) as f64;
    if !step.is_finite() || step == 0.0 {
        return None;
    }
    let index = ((value - first) / step).round();
    if index < 0.0 || index >= axis.len() as f64 {
        return None;
    }
    let index = index as usize;
    ((value - axis[index]).abs() <= step.abs() / 2.0 + 1.0e-9).then_some(index)
}

fn lat_bucket(lat: f64, bucket_deg: f64) -> i32 {
    ((lat.clamp(-90.0, 90.0) + 90.0) / bucket_deg).floor() as i32
}

fn wrap_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

fn wrap_delta(delta: f64) -> f64 {
    (delta + 180.0).rem_euclid(360.0) - 180.0
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn window(name: &str, lon: f64, landtype: i32) -> MeritHydroWindowReport {
        MeritHydroWindowReport {
            tile: PathBuf::from(name),
            tile_name: name.to_string(),
            lon: vec![lon],
            lat: vec![0.0],
            width: 1,
            height: 1,
            sampling_stride: 1,
            dir: Vec::new(),
            upa_km2: vec![0.0],
            elv_m: vec![0.0],
            width_m: vec![0.0],
            landtype_igbp: vec![landtype],
        }
    }

    #[test]
    fn physical_coast_buffer_is_refinement_only_and_respects_surface_switches() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh-coast-buffer-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let merit = root.join("merit.geojson");
        let output = root.join("coast-refine.geojson");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{"cell_id":"land","center_lon":0.2,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[0.19,-0.01],[0.21,-0.01],[0.21,0.01],[0.19,0.01],[0.19,-0.01]]]}},
              {"type":"Feature","properties":{"cell_id":"ocean","center_lon":-0.2,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-0.21,-0.01],[-0.19,-0.01],[-0.19,0.01],[-0.21,0.01],[-0.21,-0.01]]]}},
              {"type":"Feature","properties":{"cell_id":"far","center_lon":2,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[1.99,-0.01],[2.01,-0.01],[2.01,0.01],[1.99,0.01],[1.99,-0.01]]]}}
            ]}"#,
        )
        .unwrap();
        fs::write(
            &merit,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"mask_class":"COAST_LAND"},"geometry":{"type":"Polygon","coordinates":[[[-0.01,-0.01],[0.01,-0.01],[0.01,0.01],[-0.01,0.01],[-0.01,-0.01]]]}}]}"#,
        )
        .unwrap();
        let windows = vec![
            window("land.nc", 0.2, 1),
            window("ocean.nc", -0.2, 0),
            window("far.nc", 2.0, 1),
        ];

        let count = write_project_coast_refinement_cells(
            &cells, &merit, &windows, 30.0, true, false, &output,
        )
        .unwrap();
        assert_eq!(count, 1);
        let text = fs::read_to_string(&output).unwrap();
        assert!(text.contains("COAST_DISTANCE_LAND"));
        assert!(!text.contains("COAST_DISTANCE_OCEAN"));
        assert!(!text.contains("coastal_fraction"));

        let count = write_project_coast_refinement_cells(
            &cells, &merit, &windows, 30.0, true, true, &output,
        )
        .unwrap();
        assert_eq!(count, 2);
        assert!(fs::read_to_string(&output)
            .unwrap()
            .contains("COAST_DISTANCE_OCEAN"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coast_distance_index_wraps_across_the_antimeridian() {
        let index = CoastRectIndex::new(
            vec![CoastRect {
                west: 179.98,
                east: 180.02,
                south: -0.01,
                north: 0.01,
            }],
            30.0,
        );
        let distance = index.nearest_km(-179.9, 0.0).unwrap();
        assert!(distance > 0.0 && distance < 30.0, "distance={distance}");
    }

    #[test]
    fn coast_distance_uses_physical_longitude_scale_at_high_latitude() {
        let equator = CoastRect {
            west: -0.01,
            east: 0.01,
            south: -0.01,
            north: 0.01,
        }
        .distance_km(0.2, 0.0);
        let high_latitude = CoastRect {
            west: -0.01,
            east: 0.01,
            south: 79.99,
            north: 80.01,
        }
        .distance_km(0.2, 80.0);
        assert!(high_latitude < equator / 4.0);
    }

    #[test]
    #[ignore = "requires the mounted native MERIT-Hydro dataset"]
    fn real_merit_tiles_drive_coast_distance_across_the_antimeridian() {
        let merit_root = std::env::var_os("EARTHMESH_REAL_MERIT_ROOT")
            .map(PathBuf::from)
            .expect("EARTHMESH_REAL_MERIT_ROOT must point to native MERIT-Hydro tiles");
        let root = std::env::temp_dir().join(format!(
            "earthmesh-real-merit-antimeridian-coast-distance-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let bboxes = [
            crate::MeritLonLatBbox {
                west: 179.5,
                south: 51.84,
                east: 180.0,
                north: 51.96,
            },
            crate::MeritLonLatBbox {
                west: -180.0,
                south: 51.84,
                east: -177.5,
                north: 51.96,
            },
        ];
        let mut windows = Vec::new();
        for bbox in bboxes {
            let tiles = crate::merit_tile_selection::select_merit_hydro_tiles(&merit_root, bbox)
                .expect("select real MERIT tiles");
            assert_eq!(tiles.len(), 1, "each antimeridian half must use one tile");
            windows.push(
                crate::merit_hydro_io::read_merit_hydro_window(&tiles[0], bbox, 1)
                    .expect("read native MERIT window"),
            );
        }
        assert_ne!(windows[0].tile, windows[1].tile);

        let merit = crate::merit_hydro_io::write_merit_hydro_mask_geojson_layers(
            &windows,
            crate::merit_hydro_io::MeritMaskThresholds::default(),
            root.join("merit"),
            false,
            false,
        )
        .expect("classify real coast across two tiles");
        let corridors = fs::read_to_string(&merit.combined_geojson).unwrap();
        assert!(corridors.contains("n50e175.nc"));
        assert!(corridors.contains("n50w180.nc"));

        let mut cells = Vec::new();
        for (window_index, window) in windows.iter().enumerate() {
            for lon_index in (0..window.width).step_by(60) {
                for lat_index in (0..window.height).step_by(30) {
                    let lon = window.lon[lon_index];
                    let lat = window.lat[lat_index];
                    cells.push(serde_json::json!({
                        "type": "Feature",
                        "properties": {
                            "cell_id": format!("{window_index}:{lon_index}:{lat_index}"),
                            "center_lon": lon,
                            "center_lat": lat,
                        },
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [[
                                [lon - 0.0001, lat - 0.0001],
                                [lon + 0.0001, lat - 0.0001],
                                [lon + 0.0001, lat + 0.0001],
                                [lon - 0.0001, lat + 0.0001],
                                [lon - 0.0001, lat - 0.0001],
                            ]],
                        },
                    }));
                }
            }
        }
        let cells_path = root.join("cells.geojson");
        fs::write(
            &cells_path,
            serde_json::to_vec(&serde_json::json!({
                "type": "FeatureCollection",
                "features": cells,
            }))
            .unwrap(),
        )
        .unwrap();
        let coast_distance_path = root.join("coast_refinement_cells.geojson");
        let coast_distance_count = write_project_coast_refinement_cells(
            &cells_path,
            &merit.combined_geojson,
            &windows,
            50.0,
            true,
            true,
            &coast_distance_path,
        )
        .expect("build distance-band cells from real MERIT coast");
        assert!(coast_distance_count > 0);
        let coast_distance: Value =
            serde_json::from_slice(&fs::read(&coast_distance_path).unwrap()).unwrap();
        let centers = feature_array(&coast_distance)
            .unwrap()
            .iter()
            .filter_map(feature_center)
            .collect::<Vec<_>>();
        assert!(centers.iter().any(|(lon, _)| *lon > 0.0));
        assert!(centers.iter().any(|(lon, _)| *lon < 0.0));

        let report = crate::hydro_delivery_refine_workflow::run_project_hydro_workflow(
            &cells_path,
            &merit.combined_geojson,
            root.join("workflow"),
            &["COAST_LAND".to_string(), "COAST_OCEAN".to_string()],
            0.0,
            false,
            None,
            3,
            None,
            None,
            None,
            1,
            false,
            Some(&coast_distance_path),
            crate::hydro_delivery_refine_workflow::HydroRefinementPolicy {
                river_width: false,
                river_upstream_area: false,
                legacy_river_classes: false,
                coast_land: true,
                coast_ocean: true,
            },
        )
        .expect("plan real cross-tile coast-distance refinement");
        assert!(report.cells_refined >= coast_distance_count);
        assert!(fs::read_to_string(&report.refinement_plan_path)
            .unwrap()
            .contains(r#""target_level": 3"#));
        let target = crate::hydro_refinement_adapter::load_hydro_target_field(
            &report.refinement_source_path,
            &report.refinement_plan_path,
            1_000_000.0,
            0.2,
            72,
            36,
        )
        .expect("load the real cross-tile coast-distance plan into HField");
        assert_eq!(target.summary.refined_rows, report.cells_refined);
        let _ = fs::remove_dir_all(root);
    }
}
