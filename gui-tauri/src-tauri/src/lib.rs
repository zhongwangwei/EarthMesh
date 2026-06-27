//! EarthMesh Studio — Tauri backend.
//!
//! Thin command layer over `earthmesh_project` (the intent schema). The static
//! webview frontend calls these over Tauri IPC via
//! `window.__TAURI__.core.invoke(...)`.
//!
//! Deliberately hdf5-free: this process only builds/validates the project
//! intent and lowers it to a Fortran namelist. Actual mesh generation is
//! delegated to the discovered engine with the namelist as positional input, so
//! the GUI never links netcdf.

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
            scaffold_project,
            validate_project,
            set_project_metadata,
            preserve_unexposed_project_fields,
            project_summary,
            set_layer_path,
            set_domain_global,
            set_domain_bbox,
            set_quality,
            set_refinement,
            pick_data_file,
            pick_data_folder,
            open_project,
            save_project,
            read_project,
            open_path,
            run_project,
            kill_run,
            mesh_quality,
            mesh_cell_polygons
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
