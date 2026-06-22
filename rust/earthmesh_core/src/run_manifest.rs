//! `run_manifest.json` — a minimal, reproducible record of one EarthMesh run.
//!
//! Dependency-free (manual JSON, matching the project's existing hand-rolled JSON
//! writers in `earthmesh_cli`) so it lives in `earthmesh_core` and is testable
//! without serde or the `netcdf`-linked crates. Timestamps and git SHA are passed
//! in by the caller (CLI/GUI) to keep this module pure and its tests deterministic.

use std::io;
use std::path::Path;

/// Lifecycle status of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Started,
    Completed,
    Failed,
    DryRun,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Started => "started",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::DryRun => "dry_run",
        }
    }
}

impl Default for RunStatus {
    fn default() -> Self {
        RunStatus::Started
    }
}

/// One reproducible run record. Build it incrementally over a run, then
/// [`RunManifest::write_json`] it to `<workdir>/run_manifest.json`.
#[derive(Clone, Debug, Default)]
pub struct RunManifest {
    pub case_name: String,
    pub command: String,
    pub cwd: String,
    /// Path to the input namelist / project config.
    pub input_config: String,
    /// `(role, resolved absolute path)` for each input.
    pub resolved_inputs: Vec<(String, String)>,
    /// `(role, path)` for each produced output.
    pub outputs: Vec<(String, String)>,
    /// Tool version, e.g. `earthmesh_cli` `CARGO_PKG_VERSION`.
    pub software_version: String,
    pub git_sha: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub status: RunStatus,
    pub warnings: Vec<String>,
    pub quality_report: Option<String>,
}

impl RunManifest {
    /// Start a manifest with the essentials filled in.
    pub fn new(case_name: &str, command: &str, cwd: &str) -> Self {
        Self {
            case_name: case_name.to_string(),
            command: command.to_string(),
            cwd: cwd.to_string(),
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    pub fn add_input(&mut self, role: &str, path: &str) {
        self.resolved_inputs
            .push((role.to_string(), path.to_string()));
    }

    pub fn add_output(&mut self, role: &str, path: &str) {
        self.outputs.push((role.to_string(), path.to_string()));
    }

    pub fn add_warning(&mut self, warning: &str) {
        self.warnings.push(warning.to_string());
    }

    /// Serialize to pretty-ish JSON (hand-rolled; no serde dependency).
    pub fn to_json(&self) -> String {
        let mut s = String::new();
        s.push_str("{\n");
        s.push_str("  \"kind\": \"earthmesh_run_manifest\",\n");
        s.push_str(&kv("case_name", &self.case_name));
        s.push_str(&kv("command", &self.command));
        s.push_str(&kv("cwd", &self.cwd));
        s.push_str(&kv("input_config", &self.input_config));
        s.push_str(&kv("software_version", &self.software_version));
        s.push_str(&kv_opt("git_sha", &self.git_sha));
        s.push_str(&kv_opt("started_at", &self.started_at));
        s.push_str(&kv_opt("completed_at", &self.completed_at));
        s.push_str(&kv("status", self.status.as_str()));
        s.push_str(&kv_opt("quality_report", &self.quality_report));
        s.push_str(&pairs("resolved_inputs", &self.resolved_inputs));
        s.push_str(&pairs("outputs", &self.outputs));
        s.push_str(&list("warnings", &self.warnings, true));
        s.push_str("}\n");
        s
    }

    /// Write `run_manifest.json`, creating the parent directory if needed.
    pub fn write_json(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_json())
    }
}

fn esc(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn kv(key: &str, value: &str) -> String {
    format!("  \"{}\": \"{}\",\n", key, esc(value))
}

fn kv_opt(key: &str, value: &Option<String>) -> String {
    match value {
        Some(v) => format!("  \"{}\": \"{}\",\n", key, esc(v)),
        None => format!("  \"{}\": null,\n", key),
    }
}

fn pairs(key: &str, items: &[(String, String)]) -> String {
    let mut s = format!("  \"{}\": {{", key);
    if items.is_empty() {
        s.push_str("},\n");
        return s;
    }
    s.push('\n');
    for (i, (k, v)) in items.iter().enumerate() {
        let comma = if i + 1 < items.len() { "," } else { "" };
        s.push_str(&format!("    \"{}\": \"{}\"{}\n", esc(k), esc(v), comma));
    }
    s.push_str("  },\n");
    s
}

fn list(key: &str, items: &[String], last: bool) -> String {
    let tail = if last { "\n" } else { ",\n" };
    if items.is_empty() {
        return format!("  \"{}\": []{}", key, tail);
    }
    let body = items
        .iter()
        .map(|v| format!("    \"{}\"", esc(v)))
        .collect::<Vec<_>>()
        .join(",\n");
    format!("  \"{}\": [\n{}\n  ]{}", key, body, tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_written_for_dry_run_has_required_fields() {
        let mut m = RunManifest::new("gba_smoke", "earthmesh refine plan case.nml", "/work");
        m.input_config = "case.nml".to_string();
        m.status = RunStatus::DryRun;
        m.started_at = Some("2026-06-22T00:00:00Z".to_string());
        m.completed_at = Some("2026-06-22T00:00:01Z".to_string());
        m.git_sha = Some("abc1234".to_string());
        m.add_input("landtype_file", "/data/landtype.nc");
        m.add_output("gridfile", "/work/gridfile.nc4");
        m.add_warning("merit_root not set; hydro masks skipped");
        m.quality_report = Some("/work/quality.nc4".to_string());

        let dir = std::env::temp_dir().join(format!("em3_manifest_test_{}", std::process::id()));
        let path = dir.join("run_manifest.json");
        m.write_json(&path).expect("write manifest");
        let text = std::fs::read_to_string(&path).expect("read manifest");
        let _ = std::fs::remove_dir_all(&dir);

        for needle in [
            "earthmesh_run_manifest",
            "gba_smoke",
            "\"command\"",
            "\"cwd\": \"/work\"",
            "\"status\": \"dry_run\"",
            "landtype_file",
            "/work/gridfile.nc4",
            "abc1234",
            "started_at",
            "completed_at",
            "merit_root not set",
            "quality_report",
            "software_version",
        ] {
            assert!(
                text.contains(needle),
                "manifest JSON missing: {needle}\n{text}"
            );
        }
    }

    #[test]
    fn json_escapes_quotes_and_newlines() {
        let mut m = RunManifest::new("c\"a\\se", "cmd\nline", "/w");
        m.status = RunStatus::Completed;
        let j = m.to_json();
        assert!(j.contains("c\\\"a\\\\se"));
        assert!(j.contains("cmd\\nline"));
        assert!(j.contains("\"status\": \"completed\""));
    }
}
