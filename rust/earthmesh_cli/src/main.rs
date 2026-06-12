use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("earthmesh_cli: {err}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let namelist = args
        .next()
        .ok_or_else(|| usage("missing mkgrd namelist path"))?;
    let mut max_tris = 100_000usize;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--max-tris" => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--max-tris requires a value"))?;
                max_tris = value
                    .parse::<usize>()
                    .map_err(|_| usage("--max-tris must be a positive integer"))?;
            }
            "-h" | "--help" => return Err(usage("")),
            other => return Err(usage(&format!("unknown argument {other}"))),
        }
    }

    let workdir = env::current_dir().map_err(|err| err.to_string())?;
    let report = earthmesh_cli::run_mkgrd_gridinit_global_namelist(
        PathBuf::from(namelist),
        &workdir,
        max_tris,
    )
    .map_err(|err| err.to_string())?;

    println!("gridfile={}", report.gridfile.output.display());
    println!("sjx_points={}", report.gridfile.sjx_points);
    println!("lbx_points={}", report.gridfile.lbx_points);
    Ok(())
}

fn usage(message: &str) -> String {
    let prefix = if message.is_empty() {
        String::new()
    } else {
        format!("{message}\n")
    };
    format!("{prefix}usage: earthmesh_cli <mkgrd.nml> [--max-tris N]")
}
