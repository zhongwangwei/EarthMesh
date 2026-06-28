use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::{
    geojson_feature_nodes, json_escape_string, read_text_maybe_gzip, JsonNode, JsonParser,
    HYDRO_EARTH_RADIUS_M,
};

use super::geometry::geometry_outer_rings;
use super::json::json_node_to_string;

fn feature_river_class(props: Option<&BTreeMap<String, JsonNode>>) -> String {
    props
        .and_then(|p| {
            p.get("river_class")
                .and_then(JsonNode::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| p.get("mask_class").and_then(JsonNode::as_str))
        })
        .unwrap_or("")
        .to_string()
}

/// Faithful port of `earthmesh_intersection.py::earthmesh_cells_to_corridor_intersections`
/// (single-Polygon / MultiPolygon outer rings; no holes, no domain clip). Per class, the
/// per-cell overlap area is the SUM of `intersection_area(cell, corridor)` clamped to the
/// cell area — exact when same-class corridors are disjoint (river reaches), which is the
/// realistic case; Python unions same-class corridors first (matters only when they overlap).
#[allow(clippy::too_many_arguments)]
pub fn write_earthmesh_intersection_geojson(
    cell_geojson: impl AsRef<Path>,
    corridor_geojson: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    include_classes: &[String],
    min_fraction: f64,
    unit_sphere_area: bool,
    domain: Option<&[Vec<(f64, f64)>]>,
) -> io::Result<usize> {
    use earthmesh_geometry::{
        clip_convex_polygon, polygon_area, polygon_intersection_pieces, polygon_union_area, Point,
    };
    let domain_polys: Option<Vec<Vec<Point>>> = domain.map(|polys| {
        polys
            .iter()
            .map(|ring| ring.iter().map(|&(x, y)| Point::new(x, y)).collect())
            .collect()
    });
    if !(0.0..=1.0).contains(&min_fraction) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "min_fraction must be between 0 and 1",
        ));
    }
    let cells_root = JsonParser::new(&read_text_maybe_gzip(cell_geojson.as_ref())?).parse()?;
    let corridors_root =
        JsonParser::new(&read_text_maybe_gzip(corridor_geojson.as_ref())?).parse()?;
    let included: std::collections::BTreeSet<&str> =
        include_classes.iter().map(|s| s.as_str()).collect();

    let mut class_rings: BTreeMap<String, Vec<Vec<earthmesh_geometry::Point>>> = BTreeMap::new();
    for feature in geojson_feature_nodes(&corridors_root) {
        let obj = feature.as_object();
        let class = feature_river_class(
            obj.and_then(|o| o.get("properties"))
                .and_then(JsonNode::as_object),
        );
        if class.is_empty() || !included.contains(class.as_str()) {
            continue;
        }
        if let Some(geom) = obj.and_then(|o| o.get("geometry")) {
            for ring in geometry_outer_rings(geom) {
                if ring.len() < 3 {
                    continue;
                }
                match &domain_polys {
                    Some(dpolys) => {
                        for dpoly in dpolys {
                            for piece in polygon_intersection_pieces(&ring, dpoly) {
                                class_rings.entry(class.clone()).or_default().push(piece);
                            }
                        }
                    }
                    None => class_rings.entry(class.clone()).or_default().push(ring),
                }
            }
        }
    }

    let mut features = Vec::new();
    for cell in geojson_feature_nodes(&cells_root) {
        let cell_obj = cell.as_object();
        let Some(geom) = cell_obj.and_then(|o| o.get("geometry")) else {
            continue;
        };
        let cell_rings = geometry_outer_rings(geom);
        if cell_rings.is_empty() {
            continue;
        }
        let cell_area: f64 = cell_rings.iter().map(|r| polygon_area(r)).sum();
        if cell_area <= 0.0 {
            continue;
        }
        let cell_props = cell_obj
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let cell_id = cell_props
            .and_then(|p| p.get("cell_id"))
            .map(|n| match n {
                JsonNode::String(s) => s.clone(),
                other => json_node_to_string(other),
            })
            .unwrap_or_default();
        let source_area = cell_props
            .and_then(|p| p.get("source_areaCell"))
            .and_then(JsonNode::as_f64);

        for (class, rings) in &class_rings {
            let mut clipped: Vec<Vec<earthmesh_geometry::Point>> = Vec::new();
            for cr in &cell_rings {
                for corridor in rings {
                    let piece = clip_convex_polygon(corridor, cr);
                    if piece.len() >= 3 {
                        clipped.push(piece);
                    }
                }
            }
            let inter = polygon_union_area(&clipped).min(cell_area);
            if inter <= 0.0 {
                continue;
            }
            let fraction = inter / cell_area;
            if fraction < min_fraction {
                continue;
            }
            let mut props: BTreeMap<String, String> = BTreeMap::new();
            if let Some(cp) = cell_props {
                for (k, v) in cp {
                    props.insert(k.clone(), json_node_to_string(v));
                }
            }
            props.insert(
                "cell_id".into(),
                format!("\"{}\"", json_escape_string(&cell_id)),
            );
            props.insert("grid_kind".into(), "\"earthmesh_cell_preview\"".into());
            props.insert(
                "corridor_source_geometry".into(),
                "\"earthmesh_cell_intersection_preview\"".into(),
            );
            props.insert("cell_area_deg2".into(), format!("{cell_area}"));
            props.insert("intersection_area_deg2".into(), format!("{inter}"));
            props.insert(
                "overlap_class".into(),
                format!("\"{}\"", json_escape_string(class)),
            );
            props.insert("overlap_fraction".into(), format!("{fraction}"));
            props.insert(
                "domain_clip_applied".into(),
                if domain_polys.is_some() {
                    "true".into()
                } else {
                    "false".into()
                },
            );
            if class.to_ascii_uppercase().starts_with('R') {
                props.insert(
                    "river_class".into(),
                    format!("\"{}\"", json_escape_string(class)),
                );
                props.insert("river_fraction".into(), format!("{fraction}"));
                if let Some(sa) = source_area {
                    props.insert(
                        "source_estimated_river_area".into(),
                        format!("{}", sa * fraction),
                    );
                    if unit_sphere_area {
                        let norm = sa * HYDRO_EARTH_RADIUS_M * HYDRO_EARTH_RADIUS_M;
                        props.insert(
                            "area_normalization".into(),
                            "\"unit_sphere_area_to_m2\"".into(),
                        );
                        props.insert("normalized_cell_area_m2".into(), format!("{norm}"));
                        props.insert(
                            "estimated_river_area_m2".into(),
                            format!("{}", norm * fraction),
                        );
                    }
                }
            } else {
                props.insert(
                    "mask_class".into(),
                    format!("\"{}\"", json_escape_string(class)),
                );
                if class == "COAST" || class.starts_with("COAST_") {
                    props.insert("coastal_fraction".into(), format!("{fraction}"));
                }
            }
            let body = props
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", json_escape_string(k), v))
                .collect::<Vec<_>>()
                .join(", ");
            features.push(format!(
                "    {{\"type\": \"Feature\", \"geometry\": {}, \"properties\": {{{}}}}}",
                json_node_to_string(geom),
                body
            ));
        }
    }

    let out = format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        features.join(",\n")
    );
    if let Some(parent) = output_geojson.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_geojson, out)?;
    Ok(features.len())
}
