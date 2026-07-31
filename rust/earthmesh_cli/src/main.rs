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
    let argv = env::args().collect::<Vec<_>>();
    // Informational invocations have no run to reproduce and must not mutate
    // the caller's working directory.
    let is_informational = matches!(
        argv.get(1).map(String::as_str),
        None | Some("-h") | Some("--help") | Some("-V") | Some("--version")
    );
    if !is_informational {
        let last_progress = std::cell::RefCell::new((String::new(), usize::MAX));
        earthmesh_core::progress::set(move |phase, done, total| {
            let bucket = done.saturating_mul(20).checked_div(total).unwrap_or(20);
            let mut last = last_progress.borrow_mut();
            if last.0 != phase {
                last.0.clear();
                last.0.push_str(phase);
                last.1 = usize::MAX;
            }
            if last.1 != bucket || done == total {
                eprintln!("earthmesh_cli: progress {phase} {done}/{total}");
                last.1 = bucket;
            }
            true
        });
    }
    let result = run_cli_command();
    earthmesh_core::progress::clear();
    if !is_informational {
        write_cli_run_manifest(&argv, started, &result);
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("earthmesh_cli: {err}");
            ExitCode::from(2)
        }
    }
}
