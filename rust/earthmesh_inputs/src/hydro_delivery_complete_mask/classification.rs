use std::collections::BTreeMap;

use crate::{geojson_feature_nodes, JsonNode};

const RIVER_PRIMARY_MIN_FRACTION: f64 = 0.005;

pub(super) fn cell_mask_priority(class: &str) -> i32 {
    match class {
        "R3" => 30,
        "R2" => 20,
        "COAST" => 10,
        _ => 0,
    }
}

pub(super) fn cell_feature_mask_class(props: Option<&BTreeMap<String, JsonNode>>) -> String {
    let get = |k: &str| props.and_then(|m| m.get(k)).and_then(JsonNode::as_str);
    if let Some(rc) = get("river_class") {
        if rc == "R2" || rc == "R3" {
            let fraction = props
                .and_then(|m| m.get("river_fraction"))
                .and_then(JsonNode::as_f64)
                .unwrap_or(1.0);
            if fraction >= RIVER_PRIMARY_MIN_FRACTION {
                return rc.to_string();
            }
        }
    }
    if get("mask_class") == Some("COAST") {
        return "COAST".into();
    }
    if let Some(mc) = get("mask_class") {
        if mc == "LAND" || mc == "OCEAN" {
            return mc.to_string();
        }
    }
    if let Some(sc) = get("surface_class") {
        if sc == "LAND" || sc == "OCEAN" {
            return sc.to_string();
        }
    }
    "BACKGROUND".into()
}

pub(super) fn surface_class_from_coast(props: Option<&BTreeMap<String, JsonNode>>) -> String {
    let v = props.and_then(|p| {
        p.get("surface_class")
            .or_else(|| p.get("mask_class"))
            .or_else(|| p.get("coast_class"))
            .and_then(JsonNode::as_str)
    });
    match v {
        Some("COAST_LAND") => "LAND".into(),
        Some("COAST_OCEAN") => "OCEAN".into(),
        _ => "BACKGROUND".into(),
    }
}

/// Index sparse overlay features by `cell_id`, keeping the highest-priority per cell
/// (faithful to `_index_best_by_cell`).
pub(super) fn index_best_by_cell(root: &JsonNode) -> BTreeMap<String, &JsonNode> {
    let mut best: BTreeMap<String, (&JsonNode, i32)> = BTreeMap::new();
    for feature in geojson_feature_nodes(root) {
        let props = feature
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let cell_id = props
            .and_then(|p| p.get("cell_id"))
            .and_then(JsonNode::as_str)
            .unwrap_or("")
            .to_string();
        if cell_id.is_empty() {
            continue;
        }
        let prio = cell_mask_priority(&cell_feature_mask_class(props));
        match best.get(&cell_id) {
            Some((_, p)) if *p >= prio => {}
            _ => {
                best.insert(cell_id, (feature, prio));
            }
        }
    }
    best.into_iter().map(|(k, (f, _))| (k, f)).collect()
}
