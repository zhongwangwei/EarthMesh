use crate::hydro_delivery_intersections::{
    write_disjoint_earthmesh_intersection_geojson_with_domain,
    write_earthmesh_intersection_geojson_with_domain, HydroDomainComponent,
};
use crate::write_colm_coupling_csv_from_intersections;
use crate::write_coupling_quality_from_gridfile;
use crate::HydroWorkflowReport;
use crate::{
    geojson_feature_nodes, json_escape_string, read_text_maybe_gzip, JsonNode, JsonParser,
};
use std::fs;
use std::io;
use std::path::Path;

use super::feature_table::{hydro_refine_feature_set, HydroRefinementPolicy};
use super::plan::plan_refinement_from_hydro_geojson_with_policy;

fn write_disabled_refinement_plan(
    intersections_geojson: &Path,
    output_json: &Path,
) -> io::Result<usize> {
    let features = hydro_refine_feature_set(&read_text_maybe_gzip(intersections_geojson)?)?;
    let rows = features
        .table
        .cell_ids
        .iter()
        .enumerate()
        .map(|(cell, cell_id)| {
            format!(
                "    {{\"cell\": {cell}, \"cell_id\": \"{}\", \"target_level\": 0, \"composite_score\": 0, \"why\": \"refinement disabled\"}}",
                json_escape_string(cell_id)
            )
        })
        .collect::<Vec<_>>();
    fs::write(
        output_json,
        format!(
            "{{\n  \"kind\": \"earthmesh_refinement_plan\",\n  \"total_cells\": {},\n  \"cells_refined\": 0,\n  \"max_level\": 0,\n  \"budget_hit\": false,\n  \"level_histogram\": {{\"0\": {}}},\n  \"cells\": [\n{}\n  ]\n}}\n",
            rows.len(),
            rows.len(),
            rows.join(",\n"),
        ),
    )?;
    Ok(rows.len())
}

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
    let domain = domain.map(|rings| {
        rings
            .iter()
            .cloned()
            .map(HydroDomainComponent::shell)
            .collect::<Vec<_>>()
    });
    run_hydro_workflow_with_overlap(
        cells_geojson,
        corridors_geojson,
        out_dir,
        include_classes,
        min_fraction,
        unit_sphere_area,
        domain.as_deref(),
        max_level,
        max_refined_cells,
        mesh,
        landtype,
        gridnum_perdegree,
        true,
        None,
        HydroRefinementPolicy::default(),
    )
}

/// Project workflow with independent river/coast refinement demand. The
/// intersection/delivery artifacts keep every included class; only the HField
/// score is switched.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_project_hydro_workflow(
    cells_geojson: impl AsRef<Path>,
    corridors_geojson: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    include_classes: &[String],
    min_fraction: f64,
    unit_sphere_area: bool,
    domain: Option<&[HydroDomainComponent]>,
    max_level: u8,
    max_refined_cells: Option<usize>,
    mesh: Option<&Path>,
    landtype: Option<&Path>,
    gridnum_perdegree: usize,
    same_class_overlap_possible: bool,
    supplemental_refinement_geojson: Option<&Path>,
    refinement_policy: HydroRefinementPolicy,
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
        same_class_overlap_possible,
        supplemental_refinement_geojson,
        refinement_policy,
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
    domain: Option<&[HydroDomainComponent]>,
    max_level: u8,
    max_refined_cells: Option<usize>,
    mesh: Option<&Path>,
    landtype: Option<&Path>,
    gridnum_perdegree: usize,
    same_class_overlap_possible: bool,
    supplemental_refinement_geojson: Option<&Path>,
    refinement_policy: HydroRefinementPolicy,
) -> io::Result<HydroWorkflowReport> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;
    let intersections_path = out_dir.join("intersections.geojson");
    let coupling_csv_path = out_dir.join("colm_coupling.csv");
    let refinement_plan_path = out_dir.join("refinement_plan.json");
    let manifest_path = out_dir.join("workflow_manifest.json");

    let intersection_cells = if same_class_overlap_possible {
        write_earthmesh_intersection_geojson_with_domain(
            cells_geojson,
            corridors_geojson,
            &intersections_path,
            include_classes,
            min_fraction,
            unit_sphere_area,
            domain,
        )?
    } else {
        write_disjoint_earthmesh_intersection_geojson_with_domain(
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
    let refinement_inputs_path = out_dir.join("refinement_inputs.geojson");
    let refinement_source_path = if let Some(supplemental) = supplemental_refinement_geojson {
        merge_refinement_features(&intersections_path, supplemental, &refinement_inputs_path)?;
        refinement_inputs_path.clone()
    } else {
        intersections_path.clone()
    };
    let (cells_refined, refinement_max_level) = if max_level == 0 {
        write_disabled_refinement_plan(&refinement_source_path, &refinement_plan_path)?;
        (0, 0)
    } else {
        let report = plan_refinement_from_hydro_geojson_with_policy(
            &refinement_source_path,
            &refinement_plan_path,
            max_level,
            max_refined_cells,
            refinement_policy,
        )?;
        (
            report.budget_used.cells_refined_after,
            report
                .target_levels
                .level
                .iter()
                .copied()
                .max()
                .unwrap_or(0),
        )
    };

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
         \"refinement_source_geojson\": \"{}\",\n    \"colm_coupling_csv\": \"{}\",\n    \"refinement_plan_json\": \"{}\"{}\n  }}\n}}\n",
        intersection_cells,
        coupling_rows,
        estuary_coupling_rows,
        cells_refined,
        refinement_max_level,
        cq_verdict_field,
        json_escape_string(&intersections_path.display().to_string()),
        json_escape_string(&refinement_source_path.display().to_string()),
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
        refinement_source_path,
        coupling_csv_path,
        refinement_plan_path,
        coupling_quality_path,
        manifest_path,
    })
}

fn merge_refinement_features(base: &Path, supplemental: &Path, output: &Path) -> io::Result<()> {
    let mut features = Vec::new();
    for path in [base, supplemental] {
        let root = JsonParser::new(&read_text_maybe_gzip(path)?).parse()?;
        features.extend(
            geojson_feature_nodes(&root)
                .into_iter()
                .map(crate::json_node_to_string),
        );
    }
    fs::write(
        output,
        format!(
            "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}\n",
            features.join(",")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_coast_refinement_keeps_coast_coupling_output() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hydro_policy_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let corridors = root.join("corridors.geojson");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"coast-cell"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &corridors,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"mask_class":"COAST_LAND"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}]}"#,
        )
        .unwrap();

        let report = run_project_hydro_workflow(
            &cells,
            &corridors,
            root.join("workflow"),
            &["COAST_LAND".to_string()],
            0.0,
            false,
            None,
            3,
            None,
            None,
            None,
            1,
            false,
            None,
            HydroRefinementPolicy {
                river_width: true,
                river_upstream_area: true,
                legacy_river_classes: true,
                coast_land: false,
                coast_ocean: false,
            },
        )
        .unwrap();
        assert_eq!(report.intersection_cells, 1);
        assert_eq!(report.cells_refined, 0);
        assert!(fs::read_to_string(report.intersections_path)
            .unwrap()
            .contains("COAST_LAND"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coast_distance_supplement_refines_without_adding_coupling_rows() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hydro_coast_distance_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let corridors = root.join("corridors.geojson");
        let supplemental = root.join("coast_refinement_cells.geojson");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{"cell_id":"core","center_lon":0.5,"center_lat":0.5},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
              {"type":"Feature","properties":{"cell_id":"buffer","center_lon":1.5,"center_lat":0.5},"geometry":{"type":"Polygon","coordinates":[[[1,0],[2,0],[2,1],[1,1],[1,0]]]}}
            ]}"#,
        )
        .unwrap();
        fs::write(
            &corridors,
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{"mask_class":"COAST_LAND"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0.9,0],[0.9,1],[0,1],[0,0]]]}},
              {"type":"Feature","properties":{"mask_class":"R3"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0.9,0],[0.9,1],[0,1],[0,0]]]}}
            ]}"#,
        )
        .unwrap();
        fs::write(
            &supplemental,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"buffer","center_lon":1.5,"center_lat":0.5,"mask_class":"COAST_DISTANCE_LAND","refinement_only":true},"geometry":{"type":"Polygon","coordinates":[[[1,0],[2,0],[2,1],[1,1],[1,0]]]}}]}"#,
        )
        .unwrap();

        let base = run_project_hydro_workflow(
            &cells,
            &corridors,
            root.join("base"),
            &["COAST_LAND".to_string(), "R3".to_string()],
            0.0,
            false,
            None,
            5,
            None,
            None,
            None,
            1,
            false,
            None,
            HydroRefinementPolicy::default(),
        )
        .unwrap();
        let report = run_project_hydro_workflow(
            &cells,
            &corridors,
            root.join("supplemented"),
            &["COAST_LAND".to_string(), "R3".to_string()],
            0.0,
            false,
            None,
            5,
            None,
            None,
            None,
            1,
            false,
            Some(&supplemental),
            HydroRefinementPolicy::default(),
        )
        .unwrap();
        assert_eq!(report.intersection_cells, base.intersection_cells);
        assert_eq!(report.coupling_rows, base.coupling_rows);
        assert!(report.coupling_rows > 0);
        assert_eq!(
            fs::read(&report.coupling_csv_path).unwrap(),
            fs::read(&base.coupling_csv_path).unwrap()
        );
        assert_eq!(base.cells_refined, 1);
        assert_eq!(report.cells_refined, 2);
        assert_ne!(report.refinement_source_path, report.intersections_path);
        assert!(!fs::read_to_string(&report.intersections_path)
            .unwrap()
            .contains("COAST_DISTANCE"));
        assert!(!fs::read_to_string(&report.coupling_csv_path)
            .unwrap()
            .contains("COAST_DISTANCE"));
        assert!(fs::read_to_string(&report.refinement_source_path)
            .unwrap()
            .contains("COAST_DISTANCE_LAND"));
        let plan = fs::read_to_string(&report.refinement_plan_path).unwrap();
        assert!(plan.contains(r#""cell_id": "core", "target_level": 5"#));
        assert!(plan.contains(r#""cell_id": "buffer", "target_level": 5"#));
        let target = crate::hydro_refinement_adapter::load_hydro_target_field(
            &report.refinement_source_path,
            &report.refinement_plan_path,
            1_000_000.0,
            0.2,
            36,
            18,
        )
        .unwrap();
        assert_eq!(target.summary.refined_rows, 2);
        assert_eq!(target.summary.max_level, 5);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_river_options_filter_refinement_without_removing_coupling() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hydro_independent_river_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let corridors = root.join("corridors.geojson");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"river","center_lon":0.5,"center_lat":0.5},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &corridors,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"mask_class":"R3","river_width_triggered":false,"river_upstream_area_triggered":true},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#,
        )
        .unwrap();
        let width_only = run_project_hydro_workflow(
            &cells,
            &corridors,
            root.join("width"),
            &["R3".to_string()],
            0.0,
            false,
            None,
            3,
            None,
            None,
            None,
            1,
            false,
            None,
            HydroRefinementPolicy {
                river_width: true,
                river_upstream_area: false,
                legacy_river_classes: false,
                coast_land: false,
                coast_ocean: false,
            },
        )
        .unwrap();
        let upstream_only = run_project_hydro_workflow(
            &cells,
            &corridors,
            root.join("upstream"),
            &["R3".to_string()],
            0.0,
            false,
            None,
            3,
            None,
            None,
            None,
            1,
            false,
            None,
            HydroRefinementPolicy {
                river_width: false,
                river_upstream_area: true,
                legacy_river_classes: false,
                coast_land: false,
                coast_ocean: false,
            },
        )
        .unwrap();
        assert_eq!(width_only.coupling_rows, upstream_only.coupling_rows);
        assert!(width_only.coupling_rows > 0);
        assert_eq!(width_only.cells_refined, 0);
        assert_eq!(upstream_only.cells_refined, 1);
        let _ = fs::remove_dir_all(root);
    }
}
