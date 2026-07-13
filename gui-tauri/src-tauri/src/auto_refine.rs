//! Discovery and parsing of AutoRefine candidate-selection artifacts.

use std::{fs, path::Path};

use crate::dto::AutoRefineDecision;

const DECISION_FILE: &str = "auto_refine_decision.json";
const DECISION_KIND: &str = "earthmesh_auto_refine_decision";
const DECISION_SCHEMA_VERSION: u64 = 1;
const MAX_SCAN_DEPTH: usize = 16;

pub(crate) struct AutoRefineDecisionScan {
    pub(crate) decisions: Vec<AutoRefineDecision>,
    pub(crate) warnings: Vec<String>,
}

/// Scan only the current run directory. Directory symlinks are not followed,
/// so an output tree cannot make discovery escape the run or recurse forever.
pub(crate) fn scan_auto_refine_decisions(root: &Path) -> AutoRefineDecisionScan {
    let mut paths = Vec::new();
    let mut warnings = Vec::new();
    collect_decision_paths(root, 0, &mut paths, &mut warnings);
    paths.sort();

    let mut decisions = Vec::new();
    for path in paths {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                warnings.push(format!("read {}: {error}", path.display()));
                continue;
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("parse {}: {error}", path.display()));
                continue;
            }
        };
        let kind = value.get("kind").and_then(serde_json::Value::as_str);
        if kind != Some(DECISION_KIND) {
            warnings.push(format!(
                "ignore {}: unexpected decision kind '{}'",
                path.display(),
                kind.unwrap_or("<missing>")
            ));
            continue;
        }
        match value.get("schema_version") {
            None | Some(serde_json::Value::Null) => warnings.push(format!(
                "read {} as a legacy AutoRefine decision without schema_version",
                path.display()
            )),
            Some(version) => match version.as_u64() {
                Some(DECISION_SCHEMA_VERSION) => {}
                Some(version) => {
                    warnings.push(format!(
                        "ignore {}: unsupported AutoRefine decision schema_version {version} (supported: {DECISION_SCHEMA_VERSION})",
                        path.display()
                    ));
                    continue;
                }
                None => {
                    warnings.push(format!(
                        "ignore {}: schema_version must be an unsigned integer",
                        path.display()
                    ));
                    continue;
                }
            },
        }
        let mut decision: AutoRefineDecision = match serde_json::from_value(value) {
            Ok(decision) => decision,
            Err(error) => {
                warnings.push(format!("parse {}: {error}", path.display()));
                continue;
            }
        };
        decision.artifact_path = path.to_string_lossy().into_owned();
        decisions.push(decision);
    }
    decisions.sort_by(|left, right| {
        left.pass
            .cmp(&right.pass)
            .then_with(|| left.artifact_path.cmp(&right.artifact_path))
    });
    AutoRefineDecisionScan {
        decisions,
        warnings,
    }
}

fn collect_decision_paths(
    dir: &Path,
    depth: usize,
    paths: &mut Vec<std::path::PathBuf>,
    warnings: &mut Vec<String>,
) {
    if depth > MAX_SCAN_DEPTH {
        warnings.push(format!(
            "stop AutoRefine artifact scan below {}: depth exceeds {MAX_SCAN_DEPTH}",
            dir.display()
        ));
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("scan {}: {error}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!("scan {} entry: {error}", dir.display()));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                warnings.push(format!("inspect {}: {error}", entry.path().display()));
                continue;
            }
        };
        if file_type.is_dir() {
            collect_decision_paths(&entry.path(), depth + 1, paths, warnings);
        } else if file_type.is_file() && entry.file_name() == DECISION_FILE {
            paths.push(entry.path());
        }
    }
}
