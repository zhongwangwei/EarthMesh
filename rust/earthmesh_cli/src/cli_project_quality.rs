use std::fs;
use std::path::PathBuf;

use super::cli_args::usage;
use super::cli_mkgrd_run::enforce_project_quality_policy;
use super::cli_project_hydro::parse_project;

pub(crate) fn run_project_quality(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let project_path = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--project-quality requires a project YAML/JSON path"))?,
    );
    let gridfile = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--project-quality requires a generated gridfile path"))?,
    );
    let out_dir = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--project-quality requires an output directory"))?,
    );
    if let Some(extra) = args.next() {
        return Err(usage(&format!(
            "unexpected --project-quality argument {extra}"
        )));
    }
    let text = fs::read_to_string(&project_path)
        .map_err(|err| format!("read project {}: {err}", project_path.display()))?;
    let project = parse_project(&project_path, &text)?;
    let report = earthmesh_cli::project_quality::write_project_quality_report(
        &project, &gridfile, &out_dir,
    )?;
    println!("project_quality_verdict={}", report.verdict.as_str());
    println!("project_quality_cells={}", report.geometry.cell_count);
    println!(
        "project_quality_expected_euler={}",
        report
            .topology
            .expected_euler_characteristic
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    enforce_project_quality_policy(project.quality.on_violation, report.verdict)?;
    Ok(())
}
