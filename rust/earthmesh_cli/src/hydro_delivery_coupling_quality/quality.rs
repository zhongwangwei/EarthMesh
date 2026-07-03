use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::*;

/// R7 coupling-quality validator (`earthmesh_quality::coupling`) fed by the mesh +
/// land-type land/ocean signal.
///
/// `land_fraction[i]` in 0..1 (ocean = 1 - land) and 0-based `neighbors[i]` describe
/// each cell. Cells straddling the coast (0 < land < 1) classify as MixedCoast and drive
/// the coastline / orphan / mass-conservation checks — i.e. a meaningful report, not the
/// all-pure degenerate case. River/estuary signals are absent here (land-type only), so
/// the river-outlet checks stay neutral. Returns the full `CouplingQualityReport`.
pub fn landtype_coupling_quality(
    land_fraction: &[f64],
    neighbors: &[Vec<usize>],
) -> earthmesh_quality::coupling::CouplingQualityReport {
    use earthmesh_quality::coupling::{
        build_coupling_map, build_coupling_quality, classify_all, CoupledCellFractions,
        CoupledCellInput, CoupledThresholds,
    };
    let cells: Vec<CoupledCellInput> = land_fraction
        .iter()
        .enumerate()
        .map(|(i, &lf)| {
            let lf = lf.clamp(0.0, 1.0);
            CoupledCellInput {
                fractions: CoupledCellFractions {
                    land_fraction: lf,
                    ocean_fraction: 1.0 - lf,
                    river_fraction: 0.0,
                    wetland_fraction: 0.0,
                    estuary_fraction: 0.0,
                    source_features: vec!["landtype".to_string()],
                    quality_flags: Vec::new(),
                },
                neighbors: neighbors.get(i).cloned().unwrap_or_default(),
                is_estuary: false,
                is_river_mouth: false,
                outlet_ocean_cell: None,
            }
        })
        .collect();
    let th = CoupledThresholds::default();
    let classes = classify_all(&cells, &th);
    let maps = build_coupling_map(&cells, &classes);
    build_coupling_quality(&cells, &classes, &maps, &th)
}

/// Read an EarthMesh gridfile + a global land-type NetCDF, derive each W cell's land
/// fraction (its centre plus its M-corner points sampled against the land-type grid) and
/// its neighbours (W cells that share an M dual triangle), then run
/// [`landtype_coupling_quality`] and write `coupling_quality.json`. The mesh+land-type
/// counterpart of the hydro coupling QA — closes R7 onto the real coupling signal.
pub fn write_coupling_quality_from_gridfile(
    gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    output_json: impl AsRef<Path>,
) -> io::Result<earthmesh_quality::coupling::CouplingQualityReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let lt = read_landtype_data_preprocess_fortran_indexed(landtype_file, gridnum_perdegree)?;
    if lt.lon_i.len() < 3 || lt.lat_i.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "land-type axes too short to derive sampling step",
        ));
    }
    let (nlon, nlat) = (lt.nlons_source, lt.nlats_source);
    let lon0 = lt.lon_i[1];
    let lat0 = lt.lat_i[1];
    let dlon = lt.lon_i[2] - lt.lon_i[1];
    let dlat = lt.lat_i[2] - lt.lat_i[1];
    let sample_land = |lon: f64, lat: f64| -> bool {
        let li = (((lon - lon0) / dlon).round() as i64).rem_euclid(nlon as i64);
        let lj = (((lat - lat0) / dlat).round() as i64).clamp(0, nlat as i64 - 1);
        matches!(
            classify_area_judge_landtype_fortran_indexed(
                lt.landtypes_global[(li + 1) as usize][(lj + 1) as usize]
            ),
            AreaJudgeLandtypeClass::Land
        )
    };
    let is_marker = |p: &LonLatPoint| p.lon == 0.0 && p.lat == 0.0;

    let mut dense_of: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut wi_list: Vec<usize> = Vec::new();
    for (wi, c) in mesh.w_points.iter().enumerate() {
        if wi == 0 || is_marker(c) {
            continue;
        }
        dense_of.insert(wi, wi_list.len());
        wi_list.push(wi);
    }

    let mut land_fraction = Vec::with_capacity(wi_list.len());
    for &wi in &wi_list {
        let center = &mesh.w_points[wi];
        let mut land = usize::from(sample_land(center.lon, center.lat));
        let mut total = 1usize;
        if let Some(corners) = mesh.w_to_m.get(wi) {
            for &mi in corners {
                if mi >= 1 && (mi as usize) < mesh.m_points.len() {
                    let p = &mesh.m_points[mi as usize];
                    if is_marker(p) {
                        continue;
                    }
                    land += usize::from(sample_land(p.lon, p.lat));
                    total += 1;
                }
            }
        }
        land_fraction.push(land as f64 / total as f64);
    }

    let mut nbset: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); wi_list.len()];
    for tri in &mesh.m_to_w {
        let dense: Vec<usize> = tri
            .iter()
            .filter_map(|&w| dense_of.get(&(w as usize)).copied())
            .collect();
        for &a in &dense {
            for &b in &dense {
                if a != b {
                    nbset[a].insert(b);
                }
            }
        }
    }
    let neighbors: Vec<Vec<usize>> = nbset.into_iter().map(|s| s.into_iter().collect()).collect();

    let report = landtype_coupling_quality(&land_fraction, &neighbors);
    crate::ensure_parent_dir(output_json.as_ref())?;
    fs::write(
        output_json,
        earthmesh_quality::coupling::to_coupling_quality_json(&report),
    )?;
    Ok(report)
}

pub fn write_landtype_cell_mask_geojson(
    cell_geojson: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    output_geojson: impl AsRef<Path>,
) -> io::Result<usize> {
    let lt = read_landtype_data_preprocess_fortran_indexed(landtype_file, gridnum_perdegree)?;
    if lt.lon_i.len() < 3 || lt.lat_i.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "land-type axes too short to derive sampling step",
        ));
    }
    let (nlon, nlat) = (lt.nlons_source, lt.nlats_source);
    let lon0 = lt.lon_i[1];
    let lat0 = lt.lat_i[1];
    let dlon = lt.lon_i[2] - lt.lon_i[1];
    let dlat = lt.lat_i[2] - lt.lat_i[1];
    let sample_land = |lon: f64, lat: f64| -> bool {
        let li = (((lon - lon0) / dlon).round() as i64).rem_euclid(nlon as i64);
        let lj = (((lat - lat0) / dlat).round() as i64).clamp(0, nlat as i64 - 1);
        matches!(
            classify_area_judge_landtype_fortran_indexed(
                lt.landtypes_global[(li + 1) as usize][(lj + 1) as usize]
            ),
            AreaJudgeLandtypeClass::Land
        )
    };

    let root = JsonParser::new(&read_text_maybe_gzip(cell_geojson.as_ref())?).parse()?;
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let mut out_features = Vec::new();
    for feature in geojson_feature_nodes(&root) {
        let obj = feature.as_object();
        let Some(geom) = obj.and_then(|o| o.get("geometry")) else {
            continue;
        };
        let props = obj
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let mut samples = Vec::new();
        if let Some(p) = props {
            if let (Some(lon), Some(lat)) = (
                p.get("center_lon").and_then(JsonNode::as_f64),
                p.get("center_lat").and_then(JsonNode::as_f64),
            ) {
                samples.push((lon, lat));
            }
        }
        for ring in geometry_outer_rings(geom) {
            for p in ring {
                if p.x.is_finite() && p.y.is_finite() {
                    samples.push((p.x, p.y));
                }
            }
        }
        if samples.is_empty() {
            continue;
        }
        let land = samples
            .iter()
            .filter(|&&(lon, lat)| sample_land(lon, lat))
            .count();
        let land_fraction = land as f64 / samples.len() as f64;
        let ocean_fraction = 1.0 - land_fraction;
        let surface_class = if land_fraction >= ocean_fraction {
            "LAND"
        } else {
            "OCEAN"
        };
        let mask_class = if land_fraction > 0.0 && ocean_fraction > 0.0 {
            "COAST"
        } else {
            surface_class
        };

        let mut out_props: BTreeMap<String, String> = BTreeMap::new();
        if let Some(p) = props {
            for (k, v) in p {
                out_props.insert(k.clone(), json_node_to_string(v));
            }
        }
        out_props.insert(
            "surface_class".into(),
            format!("\"{}\"", esc(surface_class)),
        );
        out_props.insert("mask_class".into(), format!("\"{}\"", esc(mask_class)));
        out_props.insert(
            "hydro_mask_class".into(),
            format!("\"{}\"", esc(mask_class)),
        );
        out_props.insert("land_fraction".into(), format!("{land_fraction}"));
        out_props.insert("ocean_fraction".into(), format!("{ocean_fraction}"));
        out_props.insert(
            "coastal_fraction".into(),
            format!("{}", land_fraction.min(ocean_fraction)),
        );
        out_props.insert("has_coast".into(), (mask_class == "COAST").to_string());
        out_props.insert("mask_source".into(), "\"landtype_fraction\"".to_string());

        let body = out_props
            .iter()
            .map(|(k, v)| format!("\"{}\": {}", esc(k), v))
            .collect::<Vec<_>>()
            .join(", ");
        out_features.push(format!(
            "    {{\"type\": \"Feature\", \"geometry\": {}, \"properties\": {{{}}}}}",
            json_node_to_string(geom),
            body
        ));
    }

    let out = format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        out_features.join(",\n")
    );
    crate::ensure_parent_dir(output_geojson.as_ref())?;
    fs::write(output_geojson, out)?;
    Ok(out_features.len())
}
