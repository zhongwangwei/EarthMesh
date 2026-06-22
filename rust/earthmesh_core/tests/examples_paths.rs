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
