use std::fs;
use std::io;
use std::path::Path;

use crate::{JsonNode, JsonParser};

fn manifest_feature_count(node: Option<&JsonNode>) -> i64 {
    node.and_then(JsonNode::as_object)
        .map(|m| {
            m.get("cell_count")
                .or_else(|| m.get("feature_count"))
                .and_then(JsonNode::as_f64)
                .unwrap_or(0.0) as i64
        })
        .unwrap_or(0)
}

fn manifest_retained_triangles(eval_root: &JsonNode) -> [i64; 3] {
    let log = eval_root
        .as_object()
        .and_then(|o| o.get("refinement_log"))
        .and_then(JsonNode::as_object);
    let at = |deg: &str| {
        log.and_then(|l| l.get(deg))
            .and_then(JsonNode::as_object)
            .and_then(|r| r.get("retained_triangles"))
            .and_then(JsonNode::as_f64)
            .unwrap_or(0.0) as i64
    };
    [at("1"), at("2"), at("3")]
}

/// Faithful port of `refinement_package.py::_build_manifest`: assemble the
/// `earthmesh_hydro_coast_delivery_package` manifest (the file QA gates consume) from
/// an eval report + ranking + artifact paths.
fn build_hydro_delivery_manifest_json(
    case_name: &str,
    eval_root: &JsonNode,
    ranking_root: &JsonNode,
    files: &[(String, String)],
    source_files: &[(String, String)],
) -> String {
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let recommended = ranking_root
        .as_object()
        .and_then(|o| o.get("recommended_case"));
    let recommended_json = match recommended {
        Some(JsonNode::String(s)) => format!("\"{}\"", esc(s)),
        _ => "null".to_string(),
    };
    let eval_obj = eval_root.as_object();
    let bg = manifest_feature_count(eval_obj.and_then(|o| o.get("background_cells")));
    let river = manifest_feature_count(eval_obj.and_then(|o| o.get("river_intersections")));
    let coast = manifest_feature_count(eval_obj.and_then(|o| o.get("coast_intersections")));
    let ret = manifest_retained_triangles(eval_root);

    let obj_pairs = |pairs: &[(String, String)]| {
        let mut sorted = pairs.to_vec();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted
            .iter()
            .map(|(k, v)| format!("\"{}\": \"{}\"", esc(k), esc(v)))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "{{\n  \"kind\": \"earthmesh_hydro_coast_delivery_package\",\n  \"case_name\": \"{}\",\n  \
         \"recommended_case\": {},\n  \"files\": {{{}}},\n  \"source_files\": {{{}{}\"comparison_reports\": [], \"failed_reports\": []}},\n  \
         \"metrics\": {{\"background_cell_count\": {}, \"river_overlap_cells\": {}, \"coast_overlap_cells\": {}, \
         \"retained_triangles\": {{\"1\": {}, \"2\": {}, \"3\": {}}}}}\n}}\n",
        esc(case_name),
        recommended_json,
        obj_pairs(files),
        obj_pairs(source_files),
        if source_files.is_empty() { "" } else { ", " },
        bg,
        river,
        coast,
        ret[0],
        ret[1],
        ret[2],
    )
}

/// Faithful port of the manifest-assembly step of `write_refinement_delivery_package`:
/// read the eval + ranking JSON and write the delivery-package manifest. (The full
/// end-to-end orchestration also runs complete-cell-mask + leaflet, which depend on
/// the overlay-geojson writers / viz not yet in Rust.)
pub fn write_hydro_delivery_manifest(
    case_name: &str,
    eval_json: impl AsRef<Path>,
    ranking_json: impl AsRef<Path>,
    output_manifest: impl AsRef<Path>,
    files: &[(String, String)],
    source_files: &[(String, String)],
) -> io::Result<()> {
    let eval_root = JsonParser::new(&fs::read_to_string(eval_json.as_ref())?).parse()?;
    let ranking_root = JsonParser::new(&fs::read_to_string(ranking_json.as_ref())?).parse()?;
    let manifest = build_hydro_delivery_manifest_json(
        case_name,
        &eval_root,
        &ranking_root,
        files,
        source_files,
    );
    if let Some(parent) = output_manifest.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_manifest, manifest)
}
