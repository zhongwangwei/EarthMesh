//! Smoke test: the runnable quickstart template (NXP=16, no external data) must run
//! end-to-end through the CLI binary and produce a gridfile + run_manifest.json.
//! Uses the bundled tiny synthetic case — no MERIT/landtype data required. Needs the
//! cli binary (NetCDF), so it runs in `make test` / CI's heavy job, not `make test-fast`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn quickstart_nml() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/00_quickstart_n16.nml")
}

fn find_netcdf_outputs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_netcdf_outputs(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("nc") | Some("nc4")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn quickstart_runs_and_produces_gridfile() {
    let nml = quickstart_nml();
    if !nml.exists() {
        // examples are not part of crate-only packaging; skip cleanly there.
        eprintln!("quickstart nml absent, skipping: {}", nml.display());
        return;
    }

    // Isolated work dir: the nml's base_dir is './cases/' (relative to cwd), so all
    // outputs stay inside tmp.
    let tmp = std::env::temp_dir().join(format!("em3_quickstart_smoke_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tmp");
    let local_nml = tmp.join("quickstart.nml");
    std::fs::copy(&nml, &local_nml).expect("copy nml");

    // Invoke with a bare filename from the case dir (the natural `mkgrd.x case.nml`
    // usage). This also regression-guards the empty-parent canonicalize fix.
    let exe = env!("CARGO_BIN_EXE_earthmesh_cli");
    let output = Command::new(exe)
        .arg("quickstart.nml")
        .current_dir(&tmp)
        .output()
        .expect("run cli binary");

    assert!(
        output.status.success(),
        "quickstart run failed (exit {:?}).\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let mut produced = Vec::new();
    find_netcdf_outputs(&tmp, &mut produced);
    assert!(
        !produced.is_empty(),
        "quickstart produced no NetCDF gridfile under {}",
        tmp.display()
    );

    // R2: every CLI run writes a reproducible manifest to the cwd.
    assert!(
        tmp.join("run_manifest.json").exists(),
        "run_manifest.json not written to {}",
        tmp.display()
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
