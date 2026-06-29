use std::fs;
use std::io;
use std::path::Path;

use crate::HydroMeshQaReport;

use super::evaluate::evaluate_hydro_mesh_qa;
use super::json::hydro_mesh_qa_report_json;

/// Faithful port of `qa_gates.py::write_hydro_mesh_qa_report`.
pub fn write_hydro_mesh_qa_report(
    delivery_manifest_json: impl AsRef<Path>,
    output_json: impl AsRef<Path>,
    colm_summary_json: Option<&Path>,
    min_river_cells: i64,
    min_coast_cells: i64,
) -> io::Result<HydroMeshQaReport> {
    let report = evaluate_hydro_mesh_qa(
        delivery_manifest_json,
        colm_summary_json,
        min_river_cells,
        min_coast_cells,
    )?;
    crate::ensure_parent_dir(output_json.as_ref())?;
    fs::write(output_json, hydro_mesh_qa_report_json(&report))?;
    Ok(report)
}
