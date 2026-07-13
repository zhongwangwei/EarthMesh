use crate::classify_area_judge_landtype_one_based;
use crate::geojson_feature_nodes;
use crate::geometry_outer_rings;
use crate::json_node_to_string;
use crate::read_text_maybe_gzip;
use crate::read_unstructured_mesh_netcdf;
use crate::sample_landtype_values_for_points_one_based;
use crate::AreaJudgeLandtypeClass;
use crate::JsonNode;
use crate::JsonParser;
use crate::LonLatPoint;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

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
    let has_two_placeholders = |points: &[LonLatPoint]| {
        points.len() > 2
            && points[0].lon == 0.0
            && points[0].lat == 0.0
            && points[1].lon == 0.0
            && points[1].lat == 0.0
    };
    let w_has_two_placeholders = has_two_placeholders(&mesh.w_points);
    let m_has_two_placeholders = has_two_placeholders(&mesh.m_points);

    let mut dense_of: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut wi_list: Vec<usize> = Vec::new();
    for wi in 0..mesh.w_points.len() {
        if wi == 0 {
            continue;
        }
        let Some(canonical_id) =
            crate::unstructured_mesh_support::mesh_canonical_id_for_row(wi, w_has_two_placeholders)
        else {
            continue;
        };
        dense_of.insert(canonical_id as usize, wi_list.len());
        wi_list.push(wi);
    }

    let mut sample_points = Vec::new();
    let mut sample_ranges = Vec::with_capacity(wi_list.len());
    for &wi in &wi_list {
        let start = sample_points.len();
        let center = &mesh.w_points[wi];
        sample_points.push(*center);
        if let Some(corners) = mesh.w_to_m.get(wi) {
            for &mi in corners {
                if let Some(m_row) = crate::unstructured_mesh_support::mesh_row_for_canonical_id(
                    mi,
                    mesh.m_points.len(),
                    m_has_two_placeholders,
                ) {
                    sample_points.push(mesh.m_points[m_row]);
                }
            }
        }
        sample_ranges.push(start..sample_points.len());
    }
    let sampled = sample_landtype_values_for_points_one_based(
        landtype_file,
        gridnum_perdegree,
        &sample_points,
    )?;
    let land_fraction = sample_ranges
        .into_iter()
        .map(|range| {
            let total = range.len();
            let land = sampled[range]
                .iter()
                .filter(|&&value| {
                    matches!(
                        classify_area_judge_landtype_one_based(value),
                        AreaJudgeLandtypeClass::Land
                    )
                })
                .count();
            land as f64 / total as f64
        })
        .collect::<Vec<_>>();

    let mut nbset: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); wi_list.len()];
    for tri in &mesh.m_to_w {
        let dense: Vec<usize> = tri
            .iter()
            .filter_map(|&w_id| usize::try_from(w_id).ok())
            .filter_map(|w_id| dense_of.get(&w_id).copied())
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
    let root = JsonParser::new(&read_text_maybe_gzip(cell_geojson.as_ref())?).parse()?;
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    struct PendingFeature {
        geometry: String,
        properties: BTreeMap<String, String>,
        samples: std::ops::Range<usize>,
    }
    let mut sample_points = Vec::new();
    let mut pending = Vec::new();
    for feature in geojson_feature_nodes(&root) {
        let obj = feature.as_object();
        let Some(geom) = obj.and_then(|o| o.get("geometry")) else {
            continue;
        };
        let props = obj
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let sample_start = sample_points.len();
        if let Some(p) = props {
            if let (Some(lon), Some(lat)) = (
                p.get("center_lon").and_then(JsonNode::as_f64),
                p.get("center_lat").and_then(JsonNode::as_f64),
            ) {
                sample_points.push(LonLatPoint { lon, lat });
            }
        }
        for ring in geometry_outer_rings(geom) {
            for p in ring {
                if p.x.is_finite() && p.y.is_finite() {
                    sample_points.push(LonLatPoint { lon: p.x, lat: p.y });
                }
            }
        }
        if sample_points.len() == sample_start {
            continue;
        }
        let mut out_props = BTreeMap::new();
        if let Some(p) = props {
            for (k, v) in p {
                out_props.insert(k.clone(), json_node_to_string(v));
            }
        }
        pending.push(PendingFeature {
            geometry: json_node_to_string(geom),
            properties: out_props,
            samples: sample_start..sample_points.len(),
        });
    }
    let sampled = sample_landtype_values_for_points_one_based(
        landtype_file,
        gridnum_perdegree,
        &sample_points,
    )?;
    let mut out_features = Vec::with_capacity(pending.len());
    for pending in pending {
        let sample_count = pending.samples.len();
        let land = sampled[pending.samples]
            .iter()
            .filter(|&&value| {
                matches!(
                    classify_area_judge_landtype_one_based(value),
                    AreaJudgeLandtypeClass::Land
                )
            })
            .count();
        let land_fraction = land as f64 / sample_count as f64;
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

        let mut out_props = pending.properties;
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
            pending.geometry, body
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
