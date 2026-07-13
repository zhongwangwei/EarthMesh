use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use earthmesh_project::ProjectConfig;

use super::cli_args::usage;

pub(crate) fn run_project_hydro_postprocess(
    mut args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let project_path =
        PathBuf::from(args.next().ok_or_else(|| {
            usage("--project-hydro-postprocess requires a project YAML/JSON path")
        })?);
    let gridfile =
        PathBuf::from(args.next().ok_or_else(|| {
            usage("--project-hydro-postprocess requires a generated gridfile path")
        })?);
    let out_dir = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--project-hydro-postprocess requires an output directory"))?,
    );
    let source_namelist = args.next().map(PathBuf::from);
    let refinement_parent_gridfile = args.next().map(PathBuf::from);
    if let Some(extra) = args.next() {
        return Err(usage(&format!(
            "unexpected --project-hydro-postprocess argument {extra}"
        )));
    }

    let text = fs::read_to_string(&project_path)
        .map_err(|err| format!("read project {}: {err}", project_path.display()))?;
    let project = parse_project(&project_path, &text)?;
    let source_namelist = source_namelist
        .or_else(|| infer_source_namelist(&project_path))
        .ok_or_else(|| {
            "project hydro closed loop requires the compiled engine namelist (pass it as the fourth argument)"
                .to_string()
        })?;
    let refinement_parent_gridfile = refinement_parent_gridfile.unwrap_or_else(|| gridfile.clone());
    let workdir = env::current_dir().map_err(|err| format!("resolve hydro workdir: {err}"))?;
    let report = earthmesh_cli::project_hydro_closed_loop::run_project_hydro_closed_loop(
        &project,
        &project_path,
        &source_namelist,
        &gridfile,
        &refinement_parent_gridfile,
        &out_dir,
        &workdir,
        200_000,
        None,
    )
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "project does not configure hydro_coast".to_string())?;

    if project.quality.on_violation == earthmesh_project::ViolationPolicy::Block
        && (report.final_quality_verdict == earthmesh_quality::QualityLevel::Fail
            || report.final_coupling_quality_verdict.as_deref() == Some("fail"))
    {
        return Err(format!(
            "project quality gate failed under block policy after hydro closed loop; report={}",
            report.final_quality_dir.display()
        ));
    }

    let analysis = report.final_analysis.as_ref().unwrap_or(&report.coarse);

    println!("project_hydro_cells={}", analysis.cells_geojson.display());
    println!(
        "project_hydro_corridors={}",
        analysis.corridors_geojson.display()
    );
    if let Some(path) = &analysis.cama_reaches_geojson {
        println!("project_hydro_cama_reaches={}", path.display());
    }
    if let Some(path) = &analysis.cama_river_mouths_geojson {
        println!("project_hydro_cama_river_mouths={}", path.display());
    }
    println!(
        "project_hydro_cama_reach_count={}",
        analysis.cama_reach_count
    );
    println!(
        "project_hydro_cama_river_mouth_count={}",
        analysis.cama_river_mouth_count
    );
    println!(
        "project_hydro_final_gridfile={}",
        report.final_gridfile.display()
    );
    println!("project_hydro_manifest={}", report.manifest_path.display());
    Ok(())
}

fn infer_source_namelist(project_path: &Path) -> Option<PathBuf> {
    let candidates = [
        project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("mkgrd.nml"),
        PathBuf::from(format!("{}.nml", project_path.display())),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

pub(crate) fn parse_project(path: &Path, text: &str) -> Result<ProjectConfig, String> {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        ProjectConfig::from_json(text)
    } else {
        ProjectConfig::from_yaml(text)
    }
}
