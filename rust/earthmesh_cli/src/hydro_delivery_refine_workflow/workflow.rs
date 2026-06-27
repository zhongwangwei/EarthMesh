use std::fs;
use std::io;
use std::path::Path;

use crate::*;

use super::plan::plan_refinement_from_hydro_geojson;

/// End-to-end hydro workflow: cells (from a mesh, e.g. `--mpas-cell-polygons`) ×
/// corridors → per-cell intersection GeoJSON → CoLM coupling CSV + R8 refinement plan,
/// all under `out_dir`, plus a `workflow_manifest.json` listing the artifacts. Chains the
/// migrated overlay / coupling / planner pieces into one command. When both `mesh` (an
/// EarthMesh gridfile) and `landtype` are supplied, also runs the R7 mesh+land-type
/// coupling-quality validator into `coupling_quality.json`.
#[allow(clippy::too_many_arguments)]
pub fn run_hydro_workflow(
    cells_geojson: impl AsRef<Path>,
    corridors_geojson: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    include_classes: &[String],
    min_fraction: f64,
    unit_sphere_area: bool,
    domain: Option<&[Vec<(f64, f64)>]>,
    max_level: u8,
    max_refined_cells: Option<usize>,
    mesh: Option<&Path>,
    landtype: Option<&Path>,
    gridnum_perdegree: usize,
) -> io::Result<HydroWorkflowReport> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    let intersections_path = out_dir.join("intersections.geojson");
    let coupling_csv_path = out_dir.join("colm_coupling.csv");
    let refinement_plan_path = out_dir.join("refinement_plan.json");
    let manifest_path = out_dir.join("workflow_manifest.json");

    let intersection_cells = write_earthmesh_intersection_geojson(
        cells_geojson,
        corridors_geojson,
        &intersections_path,
        include_classes,
        min_fraction,
        unit_sphere_area,
        domain,
    )?;
    let coupling_rows = write_colm_coupling_csv_from_intersections(
        &intersections_path,
        &coupling_csv_path,
        min_fraction,
    )?;
    let report = plan_refinement_from_hydro_geojson(
        &intersections_path,
        &refinement_plan_path,
        max_level,
        max_refined_cells,
    )?;
    let cells_refined = report.budget_used.cells_refined_after;
    let refinement_max_level = report
        .target_levels
        .level
        .iter()
        .copied()
        .max()
        .unwrap_or(0);

    let mut coupling_quality_path = None;
    let mut coupling_quality_verdict = None;
    if let (Some(mesh), Some(landtype)) = (mesh, landtype) {
        let cq_path = out_dir.join("coupling_quality.json");
        let cq = write_coupling_quality_from_gridfile(mesh, landtype, gridnum_perdegree, &cq_path)?;
        coupling_quality_verdict = Some(cq.verdict.as_str().to_string());
        coupling_quality_path = Some(cq_path);
    }

    let cq_verdict_field = coupling_quality_verdict
        .as_ref()
        .map(|v| {
            format!(
                "  \"coupling_quality_verdict\": \"{}\",\n",
                json_escape_string(v)
            )
        })
        .unwrap_or_default();
    let cq_artifact = coupling_quality_path
        .as_ref()
        .map(|p| {
            format!(
                ",\n    \"coupling_quality_json\": \"{}\"",
                json_escape_string(&p.display().to_string())
            )
        })
        .unwrap_or_default();
    let manifest = format!(
        "{{\n  \"kind\": \"earthmesh_hydro_workflow\",\n  \"intersection_cells\": {},\n  \
         \"coupling_rows\": {},\n  \"cells_refined\": {},\n  \"refinement_max_level\": {},\n{}  \
         \"artifacts\": {{\n    \"intersections_geojson\": \"{}\",\n    \
         \"colm_coupling_csv\": \"{}\",\n    \"refinement_plan_json\": \"{}\"{}\n  }}\n}}\n",
        intersection_cells,
        coupling_rows,
        cells_refined,
        refinement_max_level,
        cq_verdict_field,
        json_escape_string(&intersections_path.display().to_string()),
        json_escape_string(&coupling_csv_path.display().to_string()),
        json_escape_string(&refinement_plan_path.display().to_string()),
        cq_artifact,
    );
    fs::write(&manifest_path, manifest)?;

    Ok(HydroWorkflowReport {
        intersection_cells,
        coupling_rows,
        cells_refined,
        refinement_max_level,
        coupling_quality_verdict,
        intersections_path,
        coupling_csv_path,
        refinement_plan_path,
        coupling_quality_path,
        manifest_path,
    })
}
