use std::env;
use std::process::ExitCode;

mod cli_args;
mod cli_colm_netcdf;
mod cli_dispatch;
mod cli_hydro_close;
mod cli_hydro_export;
mod cli_hydro_workflow;
mod cli_mkgrd_output;
mod cli_mkgrd_run;
mod cli_project_hydro;
mod cli_project_quality;
mod cli_quality;
mod cli_runtime;

use cli_dispatch::run_cli_command;
use cli_runtime::{now_epoch_secs, write_cli_run_manifest};

fn main() -> ExitCode {
    let started = now_epoch_secs();
    let command = env::args().collect::<Vec<_>>().join(" ");
    // Skip pure help / no-arg invocations; every real run records a manifest.
    let is_help = matches!(
        env::args().nth(1).as_deref(),
        None | Some("-h") | Some("--help")
    );
    let result = run_cli_command();
    if !is_help {
        write_cli_run_manifest(&command, started, &result);
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("earthmesh_cli: {err}");
            ExitCode::from(2)
        }
    }
}
