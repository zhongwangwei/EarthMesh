use crate::{json_escape_string, json_usize_map, HydroCompositeCloseMaskNmlWriteReport};

pub(super) fn hydro_composite_close_mask_summary_json(
    report: &HydroCompositeCloseMaskNmlWriteReport,
) -> String {
    let mut text = String::from("{\"kind\":\"earthmesh_composite_close_mask_summary\"");
    text.push_str(",\"output_prefix\":\"");
    text.push_str(&json_escape_string(
        &report.output_prefix.display().to_string(),
    ));
    text.push('"');
    text.push_str(",\"files_written\":");
    text.push_str(&report.files.len().to_string());
    text.push_str(",\"files\":[");
    for (index, path) in report.files.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push('"');
        text.push_str(&json_escape_string(&path.display().to_string()));
        text.push('"');
    }
    text.push(']');
    text.push_str(",\"counts_by_component\":");
    text.push_str(&json_usize_map(&report.counts_by_component));
    text.push_str(",\"counts_by_class_degree\":");
    text.push_str(&json_usize_map(&report.counts_by_class_degree));
    text.push_str(",\"components\":[");
    for (index, component) in report.components.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str("{\"name\":\"");
        text.push_str(&json_escape_string(&component.name));
        text.push_str("\",\"input_geojson\":\"");
        text.push_str(&json_escape_string(&component.input_geojson));
        text.push_str("\",\"files_selected\":");
        text.push_str(&component.files_selected.to_string());
        text.push_str(",\"class_refine\":");
        text.push_str(&json_usize_map(&component.class_refine));
        text.push_str(",\"max_rings_by_class\":");
        text.push_str(&json_usize_map(&component.max_rings_by_class));
        text.push_str(",\"max_rings_per_class\":");
        match component.max_rings_per_class {
            Some(value) => text.push_str(&value.to_string()),
            None => text.push_str("null"),
        }
        text.push_str(",\"dissolve_overlapping_envelopes\":");
        text.push_str(if component.dissolve_overlapping_envelopes {
            "true"
        } else {
            "false"
        });
        text.push('}');
    }
    text.push_str("]}\n");
    text
}
