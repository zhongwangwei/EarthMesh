//! Guard: committed `examples/` must not carry accidental personal absolute paths
//! (`/Users/...`, `/home/...`). External-data examples use the `${EARTHMESH_DATA}`
//! placeholder and document the requirement in their README instead.

use std::fs;
use std::path::{Path, PathBuf};

fn examples_root() -> PathBuf {
    // rust/earthmesh_core/../../examples
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn collect_text_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_text_files(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("nml") | Some("json") | Some("md") | Some("toml") | Some("txt")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn examples_have_no_personal_absolute_paths() {
    let root = examples_root();
    if !root.exists() {
        // Examples are not part of every checkout (e.g. crate-only packaging); skip.
        return;
    }
    let mut files = Vec::new();
    collect_text_files(&root, &mut files);
    assert!(
        !files.is_empty(),
        "no example text files found under {:?}",
        root
    );

    let mut offenders = Vec::new();
    for f in &files {
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        for (lineno, line) in text.lines().enumerate() {
            if line.contains("/Users/") || line.contains("/home/") {
                offenders.push(format!("{}:{}: {}", f.display(), lineno + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "examples contain personal absolute paths (use ${{EARTHMESH_DATA}} + README note instead):\n{}",
        offenders.join("\n")
    );
}

/// Runnable templates must NOT reference `${EARTHMESH_DATA}` (they work out-of-box);
/// every external-data case directory must ship a README documenting the requirement.
#[test]
fn runnable_templates_need_no_external_data_and_external_cases_are_documented() {
    let root = examples_root();
    if !root.exists() {
        return;
    }

    // Runnable templates: the quickstart + the default/ meshes.
    let runnable = [
        root.join("00_quickstart_n16.nml"),
        root.join("default/atmosphere_hex_global.nml"),
        root.join("default/land_hex_global.nml"),
        root.join("default/ocean_hex_global.nml"),
    ];
    for f in &runnable {
        if !f.exists() {
            continue;
        }
        let text = fs::read_to_string(f).unwrap_or_default();
        assert!(
            !text.contains("${EARTHMESH_DATA}"),
            "runnable template {:?} must not require external data (${{EARTHMESH_DATA}})",
            f
        );
    }

    // External-data cases live under merit_hydro/<case>/ and must each ship a README.
    let merit = root.join("merit_hydro");
    if merit.exists() {
        for entry in fs::read_dir(&merit).into_iter().flatten().flatten() {
            let dir = entry.path();
            if dir.is_dir() {
                assert!(
                    dir.join("README.md").exists(),
                    "external-data case {:?} must ship a README.md explaining the dataset",
                    dir
                );
            }
        }
    }
}
