use std::path::PathBuf;

use super::super::cli_args::usage;

pub(crate) fn run_hydro_composite_close_mask_nmls(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let recipe_json = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-composite-close-mask-nmls requires a recipe JSON"))?,
    );
    let output_prefix = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--hydro-composite-close-mask-nmls requires an output prefix"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut summary_json: Option<PathBuf> = None;
    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--summary-json" => {
                index += 1;
                summary_json = Some(PathBuf::from(
                    rest.get(index)
                        .ok_or_else(|| usage("--summary-json requires a value"))?,
                ));
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown hydro composite close-mask NML argument {other}"
                )));
            }
        }
    }

    let report = earthmesh_cli::write_hydro_composite_close_mask_nmls(
        &recipe_json,
        &output_prefix,
        summary_json.as_ref(),
    )
    .map_err(|err| err.to_string())?;
    println!(
        "hydro_composite_close_mask_prefix={}",
        report.output_prefix.display()
    );
    println!("hydro_composite_close_mask_files={}", report.files.len());
    if let Some(path) = &report.summary_json {
        println!("hydro_composite_close_mask_summary={}", path.display());
    }
    for file in &report.files {
        println!("hydro_composite_close_mask_file={}", file.display());
    }
    Ok(())
}
