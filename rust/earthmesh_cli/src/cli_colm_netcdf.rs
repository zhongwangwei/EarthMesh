use std::path::PathBuf;

use crate::cli_args::usage;

pub(crate) fn run_colm_coupling_csv_to_netcdf(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_csv = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--colm-coupling-csv-to-netcdf requires an input CSV"))?,
    );
    let output_netcdf = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--colm-coupling-csv-to-netcdf requires an output NetCDF"))?,
    );
    let rest = args.collect::<Vec<_>>();
    let mut case_name = String::new();
    let mut delivery_manifest = PathBuf::new();
    let mut restart_template_netcdf: Option<PathBuf> = None;
    let mut forcing_template_netcdf: Option<PathBuf> = None;
    let mut index = 0_usize;
    while index < rest.len() {
        match rest[index].as_str() {
            "--case-name" => {
                index += 1;
                case_name = rest
                    .get(index)
                    .ok_or_else(|| usage("--case-name requires a value"))?
                    .clone();
                index += 1;
            }
            "--delivery-manifest" => {
                index += 1;
                delivery_manifest = PathBuf::from(
                    rest.get(index)
                        .ok_or_else(|| usage("--delivery-manifest requires a value"))?,
                );
                index += 1;
            }
            "--restart-template-netcdf" => {
                index += 1;
                restart_template_netcdf =
                    Some(PathBuf::from(rest.get(index).ok_or_else(|| {
                        usage("--restart-template-netcdf requires a value")
                    })?));
                index += 1;
            }
            "--forcing-template-netcdf" => {
                index += 1;
                forcing_template_netcdf =
                    Some(PathBuf::from(rest.get(index).ok_or_else(|| {
                        usage("--forcing-template-netcdf requires a value")
                    })?));
                index += 1;
            }
            "-h" | "--help" => return Err(usage("")),
            other => {
                return Err(usage(&format!(
                    "unknown CoLM coupling NetCDF argument {other}"
                )));
            }
        }
    }

    let report = earthmesh_cli::colm_package_io::write_colm_coupling_netcdf_from_csv(
        &input_csv,
        &output_netcdf,
        &case_name,
        &delivery_manifest,
    )
    .map_err(|err| err.to_string())?;
    println!("colm_coupling_netcdf={}", report.output.display());
    println!("colm_coupling_rows={}", report.rows);
    let mut restart_template_output: Option<PathBuf> = None;
    let mut forcing_template_output: Option<PathBuf> = None;
    if let Some(restart_template_netcdf) = restart_template_netcdf {
        let restart_report =
            earthmesh_cli::colm_package_io::write_colm_restart_template_netcdf_from_csv(
                &input_csv,
                &restart_template_netcdf,
                &case_name,
            )
            .map_err(|err| err.to_string())?;
        println!(
            "colm_restart_template_netcdf={}",
            restart_report.output.display()
        );
        println!("colm_restart_template_rows={}", restart_report.rows);
        restart_template_output = Some(restart_report.output);
    }
    if let Some(forcing_template_netcdf) = forcing_template_netcdf {
        let forcing_report =
            earthmesh_cli::colm_package_io::write_colm_forcing_template_netcdf_from_csv(
                &input_csv,
                &forcing_template_netcdf,
                &case_name,
            )
            .map_err(|err| err.to_string())?;
        println!(
            "colm_forcing_template_netcdf={}",
            forcing_report.output.display()
        );
        println!("colm_forcing_template_rows={}", forcing_report.rows);
        forcing_template_output = Some(forcing_report.output);
    }
    if !delivery_manifest.as_os_str().is_empty() {
        let manifest = earthmesh_cli::colm_package_io::write_colm_package_delivery_manifest(
            &delivery_manifest,
            &case_name,
            report.rows,
            &report.output,
            restart_template_output.as_deref(),
            forcing_template_output.as_deref(),
        )
        .map_err(|err| err.to_string())?;
        println!("colm_delivery_manifest={}", manifest.display());
    }
    Ok(())
}
