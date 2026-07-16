use crate::hydro_delivery_intersections::write_disjoint_earthmesh_intersection_geojson;
use crate::write_colm_coupling_csv_from_intersections;
use crate::write_coupling_quality_from_gridfile;
use crate::write_earthmesh_intersection_geojson;
use crate::HydroWorkflowReport;
use crate::{
    geojson_feature_nodes, json_escape_string, read_text_maybe_gzip, JsonNode, JsonParser,
};
use std::fs;
use std::io;
use std::path::Path;

use super::plan::plan_refinement_from_hydro_geojson;

/// End-to-end hydro workflow: cells (from a mesh, e.g. `--mpas-cell-polygons`) ×
/// corridors → conservative spherical intersection GeoJSON → CoLM coupling CSV +
/// R8 refinement plan, all under `out_dir`, plus a `workflow_manifest.json` listing
/// the artifacts. When both `mesh` (an EarthMesh gridfile) and `landtype` are supplied,
/// also runs the R7 mesh+land-type coupling-quality validator into
/// `coupling_quality.json`.
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
    run_hydro_workflow_with_overlap(
        cells_geojson,
        corridors_geojson,
        out_dir,
        include_classes,
        min_fraction,
        unit_sphere_area,
        domain,
        max_level,
        max_refined_cells,
        mesh,
        landtype,
        gridnum_perdegree,
        true,
    )
}

/// Project-only fast path for a corridor layer whose same-class polygon interiors
/// are known to be disjoint (the native MERIT-Hydro raster classification).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_disjoint_hydro_workflow(
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
    run_hydro_workflow_with_overlap(
        cells_geojson,
        corridors_geojson,
        out_dir,
        include_classes,
        min_fraction,
        unit_sphere_area,
        domain,
        max_level,
        max_refined_cells,
        mesh,
        landtype,
        gridnum_perdegree,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_hydro_workflow_with_overlap(
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
    same_class_overlap_possible: bool,
) -> io::Result<HydroWorkflowReport> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    let intersections_path = out_dir.join("intersections.geojson");
    let coupling_csv_path = out_dir.join("colm_coupling.csv");
    let refinement_plan_path = out_dir.join("refinement_plan.json");
    let manifest_path = out_dir.join("workflow_manifest.json");

    let intersection_cells = if same_class_overlap_possible {
        write_earthmesh_intersection_geojson(
            cells_geojson,
            corridors_geojson,
            &intersections_path,
            include_classes,
            min_fraction,
            unit_sphere_area,
            domain,
        )?
    } else {
        write_disjoint_earthmesh_intersection_geojson(
            cells_geojson,
            corridors_geojson,
            &intersections_path,
            include_classes,
            min_fraction,
            unit_sphere_area,
            domain,
        )?
    };
    let coupling_rows = write_colm_coupling_csv_from_intersections(
        &intersections_path,
        &coupling_csv_path,
        min_fraction,
    )?;
    let intersections = JsonParser::new(&read_text_maybe_gzip(&intersections_path)?).parse()?;
    let estuary_coupling_rows = geojson_feature_nodes(&intersections)
        .into_iter()
        .filter(|feature| {
            let properties = feature
                .as_object()
                .and_then(|object| object.get("properties"))
                .and_then(JsonNode::as_object);
            properties
                .and_then(|props| props.get("is_estuary"))
                .is_some_and(|value| matches!(value, JsonNode::Bool(true)))
                && properties
                    .and_then(|props| props.get("estuary_fraction"))
                    .and_then(JsonNode::as_f64)
                    .is_some_and(|fraction| fraction > 0.0)
        })
        .count();
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
        "{{\n  \"kind\": \"earthmesh_hydro_workflow\",\n  \"overlay_semantics\": \"cell_local_lambert_azimuthal_equal_area_conservative\",\n  \"production_coupling\": true,\n  \"intersection_cells\": {},\n  \
         \"coupling_rows\": {},\n  \"estuary_coupling_rows\": {},\n  \"cells_refined\": {},\n  \"refinement_max_level\": {},\n{}  \
         \"artifacts\": {{\n    \"intersections_geojson\": \"{}\",\n    \
         \"colm_coupling_csv\": \"{}\",\n    \"refinement_plan_json\": \"{}\"{}\n  }}\n}}\n",
        intersection_cells,
        coupling_rows,
        estuary_coupling_rows,
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
        estuary_coupling_rows,
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
