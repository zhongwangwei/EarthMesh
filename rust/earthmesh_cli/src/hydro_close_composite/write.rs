use crate::default_hydro_close_class_refine;
use crate::json_node_to_f64;
use crate::json_node_to_usize;
use crate::json_string_usize_map;
use crate::json_usize_f64_map_node;
use crate::read_hydro_close_mask_specs;
use crate::write_hydro_close_mask_specs;
use crate::HydroCloseMaskNmlOptions;
use crate::HydroCloseMaskSpec;
use crate::HydroCompositeCloseMaskComponentSummary;
use crate::HydroCompositeCloseMaskNmlWriteReport;
use crate::JsonNode;
use crate::JsonParser;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use super::cap::apply_composite_refine_degree_cap;
use super::summary::hydro_composite_close_mask_summary_json;

/// Compose a JSON recipe of multiple hydro/coast sources into one close-mask NML set.
pub fn write_hydro_composite_close_mask_nmls(
    recipe_json: impl AsRef<Path>,
    output_prefix: impl AsRef<Path>,
    summary_json: Option<impl AsRef<Path>>,
) -> io::Result<HydroCompositeCloseMaskNmlWriteReport> {
    let recipe_text = fs::read_to_string(recipe_json.as_ref())?;
    let recipe = JsonParser::new(&recipe_text).parse()?;
    let recipe_object = recipe.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "composite close-mask recipe must be a JSON object",
        )
    })?;
    let components = recipe_object
        .get("components")
        .and_then(JsonNode::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "composite close-mask recipe requires a components array",
            )
        })?;
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "composite close-mask recipe requires a non-empty components array",
        ));
    }

    let mut tagged_specs = Vec::<(String, HydroCloseMaskSpec)>::new();
    let mut component_summaries = Vec::new();
    for (index, component) in components.iter().enumerate() {
        let component = component.as_object().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "each component must be an object",
            )
        })?;
        let component_name = component
            .get("name")
            .and_then(JsonNode::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("component_{}", index + 1));
        let input_geojson = component
            .get("input_geojson")
            .and_then(JsonNode::as_str)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "each component requires input_geojson",
                )
            })?
            .to_string();
        let default_class_refine = default_hydro_close_class_refine();
        let class_refine =
            json_string_usize_map(component.get("class_refine"), Some(&default_class_refine))?;
        let max_rings_by_class = json_string_usize_map(component.get("max_rings_by_class"), None)?;
        let max_rings_per_class = component
            .get("max_rings_per_class")
            .map(json_node_to_usize)
            .transpose()?;
        let cumulative_refine = component
            .get("cumulative_refine")
            .and_then(JsonNode::as_bool)
            .unwrap_or(true);
        let min_ring_separation_deg = component
            .get("min_ring_separation_deg")
            .map(json_node_to_f64)
            .transpose()?
            .unwrap_or(0.0);
        let buffer_deg_by_refine_degree =
            json_usize_f64_map_node(component.get("buffer_deg_by_refine_degree"), None)?;
        let simplify_tolerance_deg = component
            .get("simplify_tolerance_deg")
            .map(json_node_to_f64)
            .transpose()?
            .unwrap_or(0.0);
        let dissolve_overlapping_envelopes = component
            .get("dissolve_overlapping_envelopes")
            .and_then(JsonNode::as_bool)
            .unwrap_or(false);
        let specs = read_hydro_close_mask_specs(
            &input_geojson,
            HydroCloseMaskNmlOptions {
                class_refine: class_refine.clone(),
                max_rings_per_class,
                max_rings_by_class: max_rings_by_class.clone(),
                max_masks_per_refine_degree: None,
                min_ring_separation_deg,
                buffer_deg_by_refine_degree,
                simplify_tolerance_deg,
                dissolve_overlapping_envelopes,
                cumulative_refine,
            },
        )?;
        tagged_specs.extend(
            specs
                .iter()
                .cloned()
                .map(|spec| (component_name.clone(), spec)),
        );
        component_summaries.push(HydroCompositeCloseMaskComponentSummary {
            name: component_name,
            input_geojson,
            files_selected: specs.len(),
            class_refine,
            max_rings_by_class,
            max_rings_per_class,
            dissolve_overlapping_envelopes,
        });
    }

    let max_masks_per_refine_degree = recipe_object
        .get("max_masks_per_refine_degree")
        .map(json_node_to_usize)
        .transpose()?
        .or(Some(999));
    let capped_specs = apply_composite_refine_degree_cap(tagged_specs, max_masks_per_refine_degree);
    let mut sorted_specs = capped_specs;
    sorted_specs.sort_by(|left, right| {
        left.1
            .river_class
            .cmp(&right.1.river_class)
            .then_with(|| left.1.refine_degree.cmp(&right.1.refine_degree))
            .then_with(|| left.0.cmp(&right.0))
            .then_with(|| {
                left.1
                    .source_feature_index
                    .cmp(&right.1.source_feature_index)
            })
            .then_with(|| left.1.ring_index.cmp(&right.1.ring_index))
    });
    let write_report = write_hydro_close_mask_specs(
        output_prefix.as_ref(),
        &sorted_specs
            .iter()
            .map(|(_, spec)| spec.clone())
            .collect::<Vec<_>>(),
    )?;

    let mut counts_by_component = BTreeMap::<String, usize>::new();
    let mut counts_by_class_degree = BTreeMap::<String, usize>::new();
    for (component_name, spec) in &sorted_specs {
        *counts_by_component
            .entry(component_name.clone())
            .or_insert(0) += 1;
        *counts_by_class_degree
            .entry(format!("{}_d{}", spec.river_class, spec.refine_degree))
            .or_insert(0) += 1;
    }

    let summary_json = summary_json.map(|path| path.as_ref().to_path_buf());
    let report = HydroCompositeCloseMaskNmlWriteReport {
        output_prefix: write_report.output_prefix,
        files: write_report.files,
        counts_by_component,
        counts_by_class_degree,
        components: component_summaries,
        summary_json,
    };
    if let Some(path) = &report.summary_json {
        crate::ensure_parent_dir(path)?;
        fs::write(path, hydro_composite_close_mask_summary_json(&report))?;
    }
    Ok(report)
}
