use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::{
    geojson_feature_nodes, geometry_outer_rings, json_node_to_string, read_text_maybe_gzip,
    JsonNode, JsonParser,
};

use super::classification::{
    cell_feature_mask_class, cell_mask_priority, index_best_by_cell, surface_class_from_coast,
};

/// Faithful port of `cell_mask_merge.py::write_complete_cell_mask_geojson`: every
/// background cell annotated with surface_class (max-area LAND/OCEAN overlay) and the
/// dominant mask_class (R3>R2>COAST>LAND/OCEAN/BACKGROUND), merging sparse river/coast
/// overlay properties. Uses `intersection_area` (no shapely).
pub fn write_complete_cell_mask_geojson(
    background_geojson: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    river_geojson: Option<&Path>,
    coast_geojson: Option<&Path>,
    surface_geojson: Option<&Path>,
) -> io::Result<usize> {
    use earthmesh_geometry::{intersection_area, Point};
    let read =
        |p: &Path| -> io::Result<JsonNode> { JsonParser::new(&read_text_maybe_gzip(p)?).parse() };
    let background = read(background_geojson.as_ref())?;
    let river = river_geojson.map(read).transpose()?;
    let coast = coast_geojson.map(read).transpose()?;
    let surface = surface_geojson.map(read).transpose()?;

    let river_by_cell = river.as_ref().map(index_best_by_cell).unwrap_or_default();
    let coast_by_cell = coast.as_ref().map(index_best_by_cell).unwrap_or_default();

    let mut surface_polys: Vec<(String, Vec<Vec<Point>>)> = Vec::new();
    if let Some(surface) = &surface {
        for feature in geojson_feature_nodes(surface) {
            let obj = feature.as_object();
            let props = obj
                .and_then(|o| o.get("properties"))
                .and_then(JsonNode::as_object);
            let class = props
                .and_then(|p| {
                    p.get("surface_class")
                        .or_else(|| p.get("mask_class"))
                        .and_then(JsonNode::as_str)
                })
                .unwrap_or("");
            if class != "LAND" && class != "OCEAN" {
                continue;
            }
            if let Some(geom) = obj.and_then(|o| o.get("geometry")) {
                let rings = geometry_outer_rings(geom);
                if !rings.is_empty() {
                    surface_polys.push((class.to_string(), rings));
                }
            }
        }
    }

    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let mut out_features = Vec::new();
    for base in geojson_feature_nodes(&background) {
        let base_obj = base.as_object();
        let base_props = base_obj
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let cell_id = match base_props
            .and_then(|p| p.get("cell_id"))
            .and_then(JsonNode::as_str)
        {
            Some(c) => c.to_string(),
            None => continue,
        };
        let Some(geom) = base_obj.and_then(|o| o.get("geometry")) else {
            continue;
        };
        let cell_rings = geometry_outer_rings(geom);

        let mut surface_class = "BACKGROUND".to_string();
        if !surface_polys.is_empty() && !cell_rings.is_empty() {
            let mut best_area = 0.0;
            for (class, rings) in &surface_polys {
                let mut area = 0.0;
                for cr in &cell_rings {
                    for sr in rings {
                        area += intersection_area(cr, sr);
                    }
                }
                if area > best_area {
                    best_area = area;
                    surface_class = class.clone();
                }
            }
        }

        let river = river_by_cell.get(&cell_id).copied();
        let coast = coast_by_cell.get(&cell_id).copied();
        let base_class = if surface_class == "LAND" || surface_class == "OCEAN" {
            surface_class.clone()
        } else {
            "BACKGROUND".to_string()
        };
        let mut candidates: Vec<String> = vec![base_class];
        if coast.is_some() {
            candidates.push("COAST".into());
        }
        if let Some(r) = river {
            candidates.push(cell_feature_mask_class(
                r.as_object()
                    .and_then(|o| o.get("properties"))
                    .and_then(JsonNode::as_object),
            ));
        }
        let mask_class = candidates
            .iter()
            .cloned()
            .reduce(|a, b| {
                if cell_mask_priority(&b) > cell_mask_priority(&a) {
                    b
                } else {
                    a
                }
            })
            .unwrap_or_else(|| "BACKGROUND".into());

        let mut props: BTreeMap<String, String> = BTreeMap::new();
        if let Some(bp) = base_props {
            for (k, v) in bp {
                props.insert(k.clone(), json_node_to_string(v));
            }
        }
        let mut sources: Vec<&str> = Vec::new();
        let mut surface_class_final = surface_class.clone();
        if surface_class_final != "LAND" && surface_class_final != "OCEAN" {
            surface_class_final = surface_class_from_coast(
                coast
                    .and_then(|c| c.as_object())
                    .and_then(|o| o.get("properties"))
                    .and_then(JsonNode::as_object),
            );
        }
        if surface_class_final == "LAND" || surface_class_final == "OCEAN" {
            props.insert(
                "surface_class".into(),
                format!("\"{}\"", surface_class_final),
            );
            sources.push("surface");
        }
        for (name, overlay) in [("coast", coast), ("river", river)] {
            let Some(ov) = overlay else { continue };
            if let Some(op) = ov
                .as_object()
                .and_then(|o| o.get("properties"))
                .and_then(JsonNode::as_object)
            {
                for (k, v) in op {
                    if k != "mask_class" && k != "surface_class" {
                        props.insert(k.clone(), json_node_to_string(v));
                    }
                }
            }
            sources.push(name);
        }
        let prio = cell_mask_priority(&mask_class);
        props.insert("mask_class".into(), format!("\"{}\"", esc(&mask_class)));
        props.insert(
            "hydro_mask_class".into(),
            format!("\"{}\"", esc(&mask_class)),
        );
        props.insert("mask_priority".into(), prio.to_string());
        props.insert(
            "mask_source".into(),
            format!(
                "\"{}\"",
                if sources.is_empty() {
                    "background".to_string()
                } else {
                    sources.join("+")
                }
            ),
        );
        props.insert(
            "is_hydro_masked".into(),
            (mask_class == "COAST" || mask_class == "R2" || mask_class == "R3").to_string(),
        );

        let body = props
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
