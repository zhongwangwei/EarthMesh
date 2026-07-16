use std::path::PathBuf;

use super::usage;

fn parse_refinement_max_level(value: Option<&String>) -> Result<u8, String> {
    let max_level = value
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| usage("--max-level requires an integer 1..=255"))?;
    if max_level == 0 {
        return Err(usage("--max-level requires an integer 1..=255"));
    }
    Ok(max_level)
}

/// `--coupling-quality-from-mesh <gridfile.nc> <landtype.nc> <out.json>
/// [--gridnum-perdegree N]`: classify each mesh cell's land/ocean fraction from the
/// land-type grid + derive neighbours, then run the R7 coupling-quality validator and
/// write coupling_quality.json (the mesh+land-type counterpart of --hydro-mesh-qa).
pub(crate) fn run_coupling_quality_from_mesh(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut gridnum_perdegree = 1usize;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--gridnum-perdegree" => {
                i += 1;
                gridnum_perdegree = rest
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| usage("--gridnum-perdegree requires an integer"))?;
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --coupling-quality-from-mesh option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--coupling-quality-from-mesh needs <gridfile.nc> <landtype.nc> <out.json>",
        ));
    }
    let report =
        earthmesh_cli::hydro_delivery_coupling_quality::write_coupling_quality_from_gridfile(
            &positional[0],
            &positional[1],
            gridnum_perdegree,
            &positional[2],
        )
        .map_err(|err| format!("coupling quality from mesh: {err}"))?;
    println!("coupling_quality_verdict={}", report.verdict.as_str());
    println!("coupling_quality_land_cells={}", report.total_land_cells);
    println!("coupling_quality_ocean_cells={}", report.total_ocean_cells);
    println!(
        "coupling_quality_mixed_coast={}",
        report.mixed_coastline_cells
    );
    println!("coupling_quality_output={}", positional[2].display());
    Ok(())
}

/// `--plan-refinement-from-hydro <cells.geojson> <plan.json> [--max-level N]
/// [--max-refined-cells N]`: score each cell from the MERIT-Hydro river/coast signal in
/// a per-cell intersection / complete-mask GeoJSON and write an `earthmesh_refinement_plan`
/// target_level map (R8 planner driven by real hydro features).
pub(crate) fn run_plan_refinement_from_hydro(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut max_level: u8 = 3;
    let mut max_refined_cells: Option<usize> = None;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--max-level" => {
                i += 1;
                max_level = parse_refinement_max_level(rest.get(i))?;
            }
            "--max-refined-cells" => {
                i += 1;
                max_refined_cells = Some(
                    rest.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--max-refined-cells requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --plan-refinement-from-hydro option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 2 {
        return Err(usage(
            "--plan-refinement-from-hydro needs <cells.geojson> <plan.json>",
        ));
    }
    let report = earthmesh_cli::hydro_delivery_refine_workflow::plan_refinement_from_hydro_geojson(
        &positional[0],
        &positional[1],
        max_level,
        max_refined_cells,
    )
    .map_err(|err| format!("plan refinement from hydro: {err}"))?;
    println!(
        "refinement_plan_total_cells={}",
        report.target_levels.level.len()
    );
    println!(
        "refinement_plan_cells_refined={}",
        report.budget_used.cells_refined_after
    );
    println!("refinement_plan_output={}", positional[1].display());
    Ok(())
}

/// `--hydro-workflow <cells.geojson> <corridors.geojson> <out_dir> [--classes R2,R3]
/// [--min-fraction F] [--unit-sphere-area] [--domain-bbox W S E N | --domain-geojson P]
/// [--max-level N] [--max-refined-cells N]`: end-to-end hydro chain — overlay cells ×
/// corridors -> intersections -> CoLM coupling CSV + R8 refinement plan + manifest.
pub(crate) fn run_hydro_workflow(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut classes: Vec<String> = vec!["R2".into(), "R3".into()];
    let mut min_fraction = 0.0f64;
    let mut unit_sphere = false;
    let mut domain: Option<Vec<Vec<(f64, f64)>>> = None;
    let mut max_level: u8 = 3;
    let mut max_refined_cells: Option<usize> = None;
    let mut mesh: Option<PathBuf> = None;
    let mut landtype: Option<PathBuf> = None;
    let mut gridnum_perdegree = 1usize;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--mesh" => {
                i += 1;
                mesh = Some(PathBuf::from(
                    rest.get(i)
                        .ok_or_else(|| usage("--mesh requires a value"))?,
                ));
            }
            "--landtype" => {
                i += 1;
                landtype = Some(PathBuf::from(
                    rest.get(i)
                        .ok_or_else(|| usage("--landtype requires a value"))?,
                ));
            }
            "--gridnum-perdegree" => {
                i += 1;
                gridnum_perdegree = rest
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| usage("--gridnum-perdegree requires an integer"))?;
            }
            "--domain-bbox" => {
                let mut v = [0.0; 4];
                for slot in v.iter_mut() {
                    i += 1;
                    *slot = rest
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--domain-bbox needs W S E N"))?;
                }
                domain = Some(vec![vec![
                    (v[0], v[1]),
                    (v[2], v[1]),
                    (v[2], v[3]),
                    (v[0], v[3]),
                ]]);
            }
            "--domain-geojson" => {
                i += 1;
                let path = rest
                    .get(i)
                    .ok_or_else(|| usage("--domain-geojson requires a value"))?;
                domain = Some(
                    earthmesh_cli::hydro_delivery_intersections::read_polygon_outer_rings(path)
                        .map_err(|err| format!("read domain geojson: {err}"))?,
                );
            }
            "--classes" => {
                i += 1;
                classes = rest
                    .get(i)
                    .ok_or_else(|| usage("--classes requires a value"))?
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .collect();
            }
            "--min-fraction" => {
                i += 1;
                min_fraction = rest
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| usage("--min-fraction requires a number"))?;
            }
            "--unit-sphere-area" => unit_sphere = true,
            "--max-level" => {
                i += 1;
                max_level = parse_refinement_max_level(rest.get(i))?;
            }
            "--max-refined-cells" => {
                i += 1;
                max_refined_cells = Some(
                    rest.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--max-refined-cells requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!("unknown --hydro-workflow option: {other}")));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--hydro-workflow needs <cells.geojson> <corridors.geojson> <out_dir>",
        ));
    }
    if mesh.is_some() != landtype.is_some() {
        return Err(usage(
            "--mesh and --landtype must be given together (R7 coupling quality)",
        ));
    }
    let report = earthmesh_cli::hydro_delivery_refine_workflow::run_hydro_workflow(
        &positional[0],
        &positional[1],
        &positional[2],
        &classes,
        min_fraction,
        unit_sphere,
        domain.as_deref(),
        max_level,
        max_refined_cells,
        mesh.as_deref(),
        landtype.as_deref(),
        gridnum_perdegree,
    )
    .map_err(|err| format!("hydro workflow: {err}"))?;
    println!(
        "hydro_workflow_intersection_cells={}",
        report.intersection_cells
    );
    println!("hydro_workflow_coupling_rows={}", report.coupling_rows);
    println!("hydro_workflow_cells_refined={}", report.cells_refined);
    if let Some(verdict) = &report.coupling_quality_verdict {
        println!("hydro_workflow_coupling_quality_verdict={verdict}");
    }
    println!("hydro_workflow_manifest={}", report.manifest_path.display());
    Ok(())
}
