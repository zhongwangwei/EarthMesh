use std::fs;
use std::path::Path;

use super::super::cli_args::usage;
use super::prepare::prepare_mkgrd_namelist;

/// `--cmrc-merge-preview <project.yaml|json|mkgrd.nml>`: what CMRC's merge
/// criteria alone ask of the domain (guide 11.111), as JSON on stdout --
/// the layers read, the levels required and the criterion mesh per level --
/// without building or coarsening a mesh.
pub(crate) fn run_cmrc_merge_preview(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let source = args
        .next()
        .ok_or_else(|| usage("--cmrc-merge-preview needs a project or namelist path"))?;
    if args.next().is_some() {
        return Err(usage("--cmrc-merge-preview takes one path"));
    }
    let prepared = if [".yaml", ".yml", ".json"]
        .iter()
        .any(|extension| source.ends_with(extension))
    {
        prepare_mkgrd_namelist("--project".to_string(), &mut std::iter::once(source))?
    } else {
        prepare_mkgrd_namelist(source, &mut std::iter::empty())?
    };
    let preview = earthmesh_cli::certified_merge_preview(Path::new(&prepared.namelist))
        .map_err(|error| format!("CMRC merge preview: {error}"));
    // A compiled project leaves a run directory; a preview keeps nothing.
    for directory in [&prepared.project_run_dir, &prepared.cleanup_dir]
        .into_iter()
        .flatten()
    {
        let _ = fs::remove_dir_all(directory);
    }
    let preview = serde_json::to_string_pretty(&preview?).map_err(|error| error.to_string())?;
    println!("{preview}");
    Ok(())
}
