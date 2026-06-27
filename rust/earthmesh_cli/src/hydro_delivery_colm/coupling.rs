use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::{
    format_coupling_number, geojson_feature_nodes, read_text_maybe_gzip, JsonNode, JsonParser,
};

/// CoLM coupling CSV fields, in the order of `util/hydro_mesh/colm_coupling.py`'s
/// `COUPLING_FIELDS`. `cell_id` is index 0, `river_class` index 2 (the sort keys).
const COLM_COUPLING_FIELDS: &[&str] = &[
    "cell_id",
    "cell_index",
    "river_class",
    "river_fraction",
    "estimated_river_area_m2",
    "normalized_cell_area_m2",
    "center_lon",
    "center_lat",
    "domain_clip_applied",
    "area_normalization",
];

fn coupling_prop_string(props: &BTreeMap<String, JsonNode>, key: &str) -> String {
    match props.get(key) {
        Some(JsonNode::String(s)) => s.clone(),
        Some(JsonNode::Number(n)) => format_coupling_number(*n),
        Some(JsonNode::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// Faithful Rust port of `util/hydro_mesh/colm_coupling.py::intersections_to_coupling_rows`:
/// read an EarthMesh cell×river intersection GeoJSON FeatureCollection and assemble CoLM
/// coupling rows. A feature is kept only if `river_fraction >= min_fraction` and both
/// `cell_id` and `river_class` are non-empty; rows are sorted by `(cell_id, river_class)`.
/// Pure parsing (the cell×mask overlay that produced the intersections is
/// `earthmesh_geometry::overlay_cell`); no shapely.
pub fn colm_coupling_rows_from_intersections(
    geojson_text: &str,
    min_fraction: f64,
) -> io::Result<Vec<Vec<String>>> {
    if !(0.0..=1.0).contains(&min_fraction) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "min_fraction must be between 0 and 1",
        ));
    }
    let root = JsonParser::new(geojson_text).parse()?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for feature in geojson_feature_nodes(&root) {
        let Some(props) = feature
            .as_object()
            .and_then(|object| object.get("properties"))
            .and_then(JsonNode::as_object)
        else {
            continue;
        };
        // _as_float: number, or a numeric string, else 0.0.
        let river_fraction = match props.get("river_fraction") {
            Some(JsonNode::Number(n)) => *n,
            Some(JsonNode::String(s)) => s.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        };
        if river_fraction < min_fraction {
            continue;
        }
        let cell_id = coupling_prop_string(props, "cell_id");
        let river_class = coupling_prop_string(props, "river_class");
        if cell_id.is_empty() || river_class.is_empty() {
            continue;
        }
        let row: Vec<String> = COLM_COUPLING_FIELDS
            .iter()
            .map(|&field| match field {
                "cell_id" => cell_id.clone(),
                "river_class" => river_class.clone(),
                "river_fraction" => format_coupling_number(river_fraction),
                other => coupling_prop_string(props, other),
            })
            .collect();
        rows.push(row);
    }
    rows.sort_by(|a, b| (a[0].as_str(), a[2].as_str()).cmp(&(b[0].as_str(), b[2].as_str())));
    Ok(rows)
}

/// Faithful Rust port of `colm_coupling.py::write_colm_coupling_csv`: intersection
/// GeoJSON -> CoLM coupling CSV. Returns the row count.
pub fn write_colm_coupling_csv_from_intersections(
    input_geojson: impl AsRef<Path>,
    output_csv: impl AsRef<Path>,
    min_fraction: f64,
) -> io::Result<usize> {
    let text = read_text_maybe_gzip(input_geojson.as_ref())?;
    let rows = colm_coupling_rows_from_intersections(&text, min_fraction)?;
    let mut out = String::new();
    out.push_str(&COLM_COUPLING_FIELDS.join(","));
    out.push('\n');
    for row in &rows {
        out.push_str(&row.join(","));
        out.push('\n');
    }
    if let Some(parent) = output_csv.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_csv, out)?;
    Ok(rows.len())
}
