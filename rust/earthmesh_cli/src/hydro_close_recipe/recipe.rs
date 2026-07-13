use crate::json_escape_string;
use crate::json_number;
use crate::json_string_array;
use crate::json_usize_f64_map;
use crate::json_usize_map;
use crate::HydroCloseRefinementRecipeOptions;
use crate::HydroCloseRefinementRecipeWriteReport;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

/// Default hydro class-to-refine-degree mapping used by the current hydro workflow.
pub fn default_hydro_close_class_refine() -> BTreeMap<String, usize> {
    BTreeMap::from([("R2".to_string(), 1_usize), ("R3".to_string(), 2_usize)])
}

/// Write the close-refinement recipe consumed by the hydro mesh tools.
pub fn write_hydro_close_refinement_recipe_json(
    output_json: impl AsRef<Path>,
    options: HydroCloseRefinementRecipeOptions,
) -> io::Result<HydroCloseRefinementRecipeWriteReport> {
    let Some(max_iter_spc) = options.class_refine.values().copied().max() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hydro close recipe requires at least one class refine mapping",
        ));
    };
    if !options.simplify_tolerance_deg.is_finite() || options.simplify_tolerance_deg < 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "simplify_tolerance_deg must be a finite non-negative number",
        ));
    }
    for (degree, buffer) in &options.buffer_deg_by_refine_degree {
        if *degree == 0 || !buffer.is_finite() || *buffer < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "buffer_deg_by_refine_degree requires positive degrees and finite non-negative buffers",
            ));
        }
    }
    let output_json = output_json.as_ref().to_path_buf();
    crate::ensure_parent_dir(&output_json)?;

    let mut close_mask_command = vec![
        "earthmesh_cli".to_string(),
        "--hydro-close-mask-nmls".to_string(),
        options.input_geojson.display().to_string(),
        options.output_prefix.display().to_string(),
    ];
    if !options.class_refine.is_empty() {
        close_mask_command.push("--class-refine".to_string());
        for (class, degree) in &options.class_refine {
            close_mask_command.push(format!("{class}={degree}"));
        }
    }
    if !options.buffer_deg_by_refine_degree.is_empty() {
        close_mask_command.push("--buffer-deg-by-refine-degree".to_string());
        for (degree, buffer_deg) in &options.buffer_deg_by_refine_degree {
            close_mask_command.push(format!("{degree}={}", json_number(*buffer_deg)));
        }
    }
    if options.simplify_tolerance_deg > 0.0 {
        close_mask_command.push("--simplify-tolerance-deg".to_string());
        close_mask_command.push(json_number(options.simplify_tolerance_deg));
    }

    let smoke_run_command = options
        .example_namelist
        .as_ref()
        .map(|namelist| vec!["./mkgrd.x".to_string(), namelist.clone()]);
    let notes = vec![
        "Buffers are mesh-generation envelopes, not CoLM river-area estimates.".to_string(),
        "Use cumulative close masks for nested refinement unless deliberately testing non-cumulative behavior.".to_string(),
    ];

    let mut text = String::new();
    text.push_str("{\"kind\":\"earthmesh_hydro_close_refinement_recipe\"");
    text.push_str(",\"input_geojson\":\"");
    text.push_str(&json_escape_string(
        &options.input_geojson.display().to_string(),
    ));
    text.push('"');
    text.push_str(",\"output_prefix\":\"");
    text.push_str(&json_escape_string(
        &options.output_prefix.display().to_string(),
    ));
    text.push('"');
    text.push_str(",\"class_refine\":");
    text.push_str(&json_usize_map(&options.class_refine));
    text.push_str(",\"buffer_deg_by_refine_degree\":");
    text.push_str(&json_usize_f64_map(&options.buffer_deg_by_refine_degree));
    text.push_str(",\"simplify_tolerance_deg\":");
    text.push_str(&json_number(options.simplify_tolerance_deg));
    text.push_str(",\"close_mask_command\":");
    text.push_str(&json_string_array(&close_mask_command));
    text.push_str(",\"earthmesh_namelist_overrides\":{");
    text.push_str("\"RL%mask_refine_spc_fprefix\":\"'");
    text.push_str(&json_escape_string(
        &options.output_prefix.display().to_string(),
    ));
    text.push_str("'\",\"RL%mask_refine_spc_type\":\"'close'\"");
    text.push_str(",\"RL%max_iter_spc\":\"");
    text.push_str(&max_iter_spc.to_string());
    text.push_str("\",\"RL%refine_spc\":\".TRUE.\"}");
    text.push_str(",\"notes\":");
    text.push_str(&json_string_array(&notes));
    if let Some(command) = &smoke_run_command {
        text.push_str(",\"smoke_run_command\":");
        text.push_str(&json_string_array(command));
    }
    text.push_str("}\n");

    fs::write(&output_json, text)?;
    Ok(HydroCloseRefinementRecipeWriteReport {
        output_json,
        max_iter_spc,
        class_count: options.class_refine.len(),
        buffer_count: options.buffer_deg_by_refine_degree.len(),
    })
}
