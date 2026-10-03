use std::collections::BTreeMap;

use crate::hydro_workflow_types::{HydroBackgroundSummary, HydroIntersectionSummary};
use crate::{geojson_feature_nodes, JsonNode, HYDRO_EARTH_RADIUS_M};

/// `statistics.median`: middle for odd n, mean of the two middle for even n.
fn slice_median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn round12(value: f64) -> f64 {
    (value * 1e12).round() / 1e12
}

/// Faithful port of `refinement_eval.py::summarize_background_cells`.
pub(super) fn summarize_background_cells(
    root: &JsonNode,
    unit_sphere_area: bool,
) -> HydroBackgroundSummary {
    let features = geojson_feature_nodes(root);
    let mut sizes_km = Vec::new();
    for feature in &features {
        let props = feature
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let Some(props) = props else { continue };
        let area = if let Some(a) = props
            .get("normalized_cell_area_m2")
            .and_then(JsonNode::as_f64)
        {
            Some(a)
        } else {
            props
                .get("source_areaCell")
                .and_then(JsonNode::as_f64)
                .map(|s| {
                    if unit_sphere_area {
                        s * HYDRO_EARTH_RADIUS_M * HYDRO_EARTH_RADIUS_M
                    } else {
                        s
                    }
                })
        };
        if let Some(a) = area {
            if a > 0.0 {
                sizes_km.push(a.sqrt() / 1000.0);
            }
        }
    }
    let mut summary = HydroBackgroundSummary {
        cell_count: features.len(),
        ..Default::default()
    };
    if !sizes_km.is_empty() {
        let min = sizes_km.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = sizes_km.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        summary.size_km_min = Some(round12(min));
        summary.size_km_median = Some(round12(slice_median(&mut sizes_km)));
        summary.size_km_max = Some(round12(max));
    }
    summary
}

fn summarize_intersection_generic(
    root: &JsonNode,
    class_keys: &[&str],
    fraction_keys: &[&str],
    area_keys: &[&str],
) -> HydroIntersectionSummary {
    let features = geojson_feature_nodes(root);
    let mut class_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut fractions = Vec::new();
    let mut area_sum = 0.0;
    for feature in &features {
        let Some(props) = feature
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object)
        else {
            continue;
        };
        let class = class_keys
            .iter()
            .find_map(|k| {
                props
                    .get(*k)
                    .and_then(JsonNode::as_str)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("");
        if !class.is_empty() {
            *class_counts.entry(class.to_string()).or_insert(0) += 1;
        }
        for fk in fraction_keys {
            if let Some(v) = props.get(*fk).and_then(JsonNode::as_f64) {
                fractions.push(v);
                break;
            }
        }
        for ak in area_keys {
            if let Some(v) = props.get(*ak).and_then(JsonNode::as_f64) {
                area_sum += v;
                break;
            }
        }
    }
    let mut summary = HydroIntersectionSummary {
        feature_count: features.len(),
        class_counts,
        ..Default::default()
    };
    if !fractions.is_empty() {
        let min = fractions.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = fractions.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        summary.fraction_min = Some(round12(min));
        summary.fraction_median = Some(round12(slice_median(&mut fractions)));
        summary.fraction_max = Some(round12(max));
    }
    if area_sum > 0.0 {
        summary.area_sum = Some(round12(area_sum));
    }
    summary
}

/// Faithful port of `refinement_eval.py::summarize_intersections` (river).
pub(super) fn summarize_river_intersections(root: &JsonNode) -> HydroIntersectionSummary {
    summarize_intersection_generic(
        root,
        &["river_class"],
        &["river_fraction"],
        &["estimated_river_area_m2"],
    )
}

/// Faithful port of `refinement_eval.py::summarize_coast_intersections`.
pub(super) fn summarize_coast_intersections(root: &JsonNode) -> HydroIntersectionSummary {
    summarize_intersection_generic(
        root,
        &["mask_class", "overlap_class", "coast_class"],
        &["coastal_fraction", "coast_fraction"],
        &["estimated_coastal_area_m2", "estimated_coast_area_m2"],
    )
}
