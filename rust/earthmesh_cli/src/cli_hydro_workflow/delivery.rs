use std::path::PathBuf;

use super::usage;

fn next_arg(rest: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    rest.get(*index)
        .cloned()
        .ok_or_else(|| usage(&format!("{flag} requires a value")))
}

fn next_path_arg(rest: &[String], index: &mut usize, flag: &str) -> Result<PathBuf, String> {
    next_arg(rest, index, flag).map(PathBuf::from)
}

/// `--hydro-delivery-manifest --case-name <n> --eval-json <e> --ranking-json <r>
/// --output-json <m> [--file role=path ...] [--source role=path ...]`:
/// assemble the delivery-package manifest (port of refinement_package.py::_build_manifest).
pub(crate) fn run_hydro_delivery_manifest(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut case_name = String::new();
    let mut eval_json: Option<PathBuf> = None;
    let mut ranking_json: Option<PathBuf> = None;
    let mut output_json: Option<PathBuf> = None;
    let mut files: Vec<(String, String)> = Vec::new();
    let mut source_files: Vec<(String, String)> = Vec::new();
    let mut i = 0usize;
    let split_kv = |s: &str| -> Result<(String, String), String> {
        s.split_once('=')
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .ok_or_else(|| usage("expected role=path"))
    };
    while i < rest.len() {
        match rest[i].as_str() {
            "--case-name" => case_name = next_arg(&rest, &mut i, "flag")?,
            "--eval-json" => eval_json = Some(next_path_arg(&rest, &mut i, "flag")?),
            "--ranking-json" => ranking_json = Some(next_path_arg(&rest, &mut i, "flag")?),
            "--output-json" => output_json = Some(next_path_arg(&rest, &mut i, "flag")?),
            "--file" => files.push(split_kv(&next_arg(&rest, &mut i, "flag")?)?),
            "--source" => source_files.push(split_kv(&next_arg(&rest, &mut i, "flag")?)?),
            other => {
                return Err(usage(&format!(
                    "unknown --hydro-delivery-manifest option: {other}"
                )));
            }
        }
        i += 1;
    }
    let eval_json =
        eval_json.ok_or_else(|| usage("--hydro-delivery-manifest requires --eval-json"))?;
    let ranking_json =
        ranking_json.ok_or_else(|| usage("--hydro-delivery-manifest requires --ranking-json"))?;
    let output_json =
        output_json.ok_or_else(|| usage("--hydro-delivery-manifest requires --output-json"))?;
    earthmesh_cli::hydro_delivery_manifest::write_hydro_delivery_manifest(
        &case_name,
        &eval_json,
        &ranking_json,
        &output_json,
        &files,
        &source_files,
    )
    .map_err(|err| format!("delivery manifest: {err}"))?;
    println!("hydro_delivery_manifest_output={}", output_json.display());
    Ok(())
}

fn parse_int_csv(value: &str) -> Result<Vec<i64>, String> {
    let parsed: Vec<i64> = value
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().parse::<i64>())
        .collect::<Result<_, _>>()
        .map_err(|_| usage("expected comma-separated integers"))?;
    if parsed.is_empty() {
        return Err(usage("expected at least one integer value"));
    }
    Ok(parsed)
}

/// `--hydro-sweep-recipes --river-geojson <r> --coast-geojson <c> --output-dir <d>
/// [--r2-caps 40,60,80] [--coast-caps 10,20,40] [--r3-cap 19]`:
/// write composite close-mask recipes for an R2 x COAST sweep (port of refinement_sweep.py).
pub(crate) fn run_hydro_sweep_recipes(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut river: Option<String> = None;
    let mut coast: Option<String> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut r2_caps = vec![40i64, 60, 80];
    let mut coast_caps = vec![10i64, 20, 40];
    let mut r3_cap = 19i64;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--river-geojson" => river = Some(next_arg(&rest, &mut i, "--river-geojson")?),
            "--coast-geojson" => coast = Some(next_arg(&rest, &mut i, "--coast-geojson")?),
            "--output-dir" => output_dir = Some(next_path_arg(&rest, &mut i, "--output-dir")?),
            "--r2-caps" => r2_caps = parse_int_csv(&next_arg(&rest, &mut i, "--r2-caps")?)?,
            "--coast-caps" => {
                coast_caps = parse_int_csv(&next_arg(&rest, &mut i, "--coast-caps")?)?
            }
            "--r3-cap" => {
                r3_cap = next_arg(&rest, &mut i, "--r3-cap")?
                    .parse()
                    .map_err(|_| usage("--r3-cap requires an integer"))?
            }
            other => {
                return Err(usage(&format!(
                    "unknown --hydro-sweep-recipes option: {other}"
                )));
            }
        }
        i += 1;
    }
    let river = river.ok_or_else(|| usage("--hydro-sweep-recipes requires --river-geojson"))?;
    let coast = coast.ok_or_else(|| usage("--hydro-sweep-recipes requires --coast-geojson"))?;
    let output_dir =
        output_dir.ok_or_else(|| usage("--hydro-sweep-recipes requires --output-dir"))?;
    let count = earthmesh_cli::hydro_sweep::write_sweep_recipes(
        &output_dir,
        &river,
        &coast,
        r2_caps,
        coast_caps,
        r3_cap,
    )
    .map_err(|err| format!("sweep recipes: {err}"))?;
    println!("hydro_sweep_cases={count}");
    println!("hydro_sweep_output_dir={}", output_dir.display());
    Ok(())
}

/// `--hydro-sweep-rank <report1.json> [report2.json ...] --output-json <out.json>
/// [--max-background-cells N]`: rank refinement-eval reports (port of refinement_sweep.py).
pub(crate) fn run_hydro_sweep_rank(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut reports: Vec<PathBuf> = Vec::new();
    let mut output_json: Option<PathBuf> = None;
    let mut max_background: Option<i64> = None;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--output-json" => {
                output_json = Some(next_path_arg(&rest, &mut i, "--output-json")?);
            }
            "--max-background-cells" => {
                max_background = Some(
                    next_arg(&rest, &mut i, "--max-background-cells")?
                        .parse()
                        .map_err(|_| usage("--max-background-cells requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-sweep-rank option: {other}"
                )));
            }
            other => reports.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if reports.is_empty() {
        return Err(usage(
            "--hydro-sweep-rank requires at least one report path",
        ));
    }
    let output_json =
        output_json.ok_or_else(|| usage("--hydro-sweep-rank requires --output-json"))?;
    let recommended =
        earthmesh_cli::hydro_sweep::write_sweep_ranking(&reports, &output_json, max_background)
            .map_err(|err| format!("sweep ranking: {err}"))?;
    println!("hydro_sweep_recommended={recommended}");
    println!("hydro_sweep_ranking_output={}", output_json.display());
    Ok(())
}

/// `--hydro-refinement-eval <background.geojson> <intersections.geojson> <out.json>
/// [--coast-intersections-geojson <g>] [--log-path <l>] [--file-area-m2]`:
/// summarize hydro-refinement cells + river/coast overlaps (port of refinement_eval.py).
pub(crate) fn run_hydro_refinement_eval(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut coast: Option<PathBuf> = None;
    let mut log_path: Option<PathBuf> = None;
    let mut unit_sphere = true;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--coast-intersections-geojson" => {
                coast = Some(next_path_arg(
                    &rest,
                    &mut i,
                    "--coast-intersections-geojson",
                )?);
            }
            "--log-path" => {
                log_path = Some(next_path_arg(&rest, &mut i, "--log-path")?);
            }
            "--file-area-m2" => unit_sphere = false,
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-refinement-eval option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--hydro-refinement-eval needs <background.geojson> <intersections.geojson> <out.json>",
        ));
    }
    earthmesh_cli::hydro_refinement_eval::write_refinement_eval_json(
        &positional[0],
        &positional[1],
        &positional[2],
        coast.as_deref(),
        log_path.as_deref(),
        unit_sphere,
    )
    .map_err(|err| format!("refinement eval: {err}"))?;
    println!("hydro_refinement_eval_output={}", positional[2].display());
    Ok(())
}

/// `--hydro-mesh-qa --delivery-manifest <m.json> --output-json <out.json>
/// [--colm-summary-json <s.json>] [--min-river-cells N] [--min-coast-cells N]`:
/// evaluate delivery-package QA gates (Rust port of util/hydro_mesh/qa_gates.py).
pub(crate) fn run_hydro_mesh_qa(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut delivery_manifest: Option<PathBuf> = None;
    let mut output_json: Option<PathBuf> = None;
    let mut colm_summary: Option<PathBuf> = None;
    let mut min_river: i64 = 1;
    let mut min_coast: i64 = 1;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--delivery-manifest" => {
                delivery_manifest = Some(next_path_arg(&rest, &mut i, "--delivery-manifest")?);
            }
            "--output-json" => {
                output_json = Some(next_path_arg(&rest, &mut i, "--output-json")?);
            }
            "--colm-summary-json" => {
                colm_summary = Some(next_path_arg(&rest, &mut i, "--colm-summary-json")?);
            }
            "--min-river-cells" => {
                min_river = next_arg(&rest, &mut i, "--min-river-cells")?
                    .parse()
                    .map_err(|_| usage("--min-river-cells requires an integer"))?;
            }
            "--min-coast-cells" => {
                min_coast = next_arg(&rest, &mut i, "--min-coast-cells")?
                    .parse()
                    .map_err(|_| usage("--min-coast-cells requires an integer"))?;
            }
            other => return Err(usage(&format!("unknown --hydro-mesh-qa option: {other}"))),
        }
        i += 1;
    }
    let delivery_manifest =
        delivery_manifest.ok_or_else(|| usage("--hydro-mesh-qa requires --delivery-manifest"))?;
    let output_json = output_json.ok_or_else(|| usage("--hydro-mesh-qa requires --output-json"))?;
    let report = earthmesh_cli::hydro_delivery_qa::write_hydro_mesh_qa_report(
        &delivery_manifest,
        &output_json,
        colm_summary.as_deref(),
        min_river,
        min_coast,
    )
    .map_err(|err| format!("hydro mesh qa: {err}"))?;
    println!("hydro_mesh_qa_status={}", report.status);
    println!("hydro_mesh_qa_output={}", output_json.display());
    Ok(())
}
