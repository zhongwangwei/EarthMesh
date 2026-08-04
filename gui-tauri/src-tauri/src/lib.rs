//! EarthMesh Studio — Tauri backend.
//!
//! Thin command layer over `earthmesh_project` (the intent schema). The static
//! webview frontend calls these over Tauri IPC via
//! `window.__TAURI__.core.invoke(...)`.
//!
//! Deliberately hdf5-free: this process only builds/validates the project
//! intent. Actual lowering and mesh generation are delegated to the discovered
//! CLI through its authoritative `--project` workflow, so every quality policy,
//! refinement source, and hydro mode has one implementation and the GUI never
//! links netcdf.

mod auto_refine;
mod dto;
mod engine;
mod file_commands;
mod mesh_outputs;
mod mesh_paths;
mod mesh_process;
mod mesh_runner;
mod project_commands;
mod project_edits;
mod project_queries;
mod quality;

use crate::file_commands::*;
use crate::mesh_outputs::*;
use crate::mesh_process::*;
use crate::mesh_runner::*;
use crate::project_commands::*;
use crate::project_edits::*;
use crate::project_queries::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_criteria,
            project_capabilities,
            scaffold_project,
            validate_project,
            set_project_metadata,
            preserve_unexposed_project_fields,
            project_summary,
            set_layer_path,
            set_threshold_value,
            set_threshold_criterion,
            set_hydro_refinement,
            autofill_data_layers_from_folder,
            set_project_target,
            set_target_cell,
            set_domain_global,
            set_domain_bbox,
            set_domain_shapefile,
            set_domain_close,
            set_close_boundary,
            set_quality,
            set_refinement,
            set_specified_refinement,
            set_adaptive_refinement,
            set_hfield_refinement,
            set_expert,
            pick_data_file,
            pick_data_folder,
            open_project,
            save_project,
            save_map_png,
            read_project,
            open_path,
            run_project,
            kill_run,
            mesh_quality,
            mesh_cell_polygons,
            mesh_merit_cells,
            shapefile_boundary_geojson
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
