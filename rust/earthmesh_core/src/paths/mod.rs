//! Unified path resolution and pre-run input checks shared by the CLI and GUI.
//!
//! Goal: a relative path means the same thing whether it comes from a CLI cwd, a
//! GUI session, or an example namelist, and missing inputs produce an actionable
//! error instead of a late failure. Dependency-free (no serde / no I/O beyond
//! `std::fs::exists` checks) so it lives in `earthmesh_core` and is unit-testable
//! without the heavy `netcdf`-linked crates.

use std::path::{Path, PathBuf};

/// Cross-platform home directory: `HOME` on Unix, `USERPROFILE` on Windows.
///
/// The codebase previously read only `HOME`, which is absent on Windows; callers
/// should prefer this helper.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

/// Resolves possibly-relative input/config/resource paths against a single base
/// directory so CLI and GUI agree on what a relative path means.
#[derive(Clone, Debug)]
pub struct PathResolver {
    /// Directory that relative paths resolve against — usually the CLI working
    /// directory or the project/case directory.
    pub base_dir: PathBuf,
}

impl PathResolver {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Resolver rooted at the process current working directory.
    pub fn from_cwd() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Resolve a path string. Absolute paths pass through unchanged; relative
    /// paths join `base_dir`. Surrounding whitespace is trimmed so a stray leading
    /// space (e.g. the legacy `" /tmp"` default) does not create a bogus directory.
    pub fn resolve(&self, raw: impl AsRef<str>) -> PathBuf {
        let trimmed = raw.as_ref().trim();
        let p = Path::new(trimmed);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.base_dir.join(p)
        }
    }

    /// Candidate locations for a packaged resource (e.g. `"examples"`), in priority
    /// order: `$EARTHMESH_RESOURCE_DIR/<rel>`, the macOS `.app` `../Resources/<rel>`
    /// next to the executable, then `<base_dir>/<rel>` for dev checkouts.
    pub fn resource_candidates(&self, relative: &str) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Some(root) = std::env::var_os("EARTHMESH_RESOURCE_DIR").filter(|v| !v.is_empty()) {
            candidates.push(PathBuf::from(root).join(relative));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("../Resources").join(relative));
            }
        }
        candidates.push(self.base_dir.join(relative));
        candidates
    }

    /// First existing resource candidate, if any.
    pub fn resource(&self, relative: &str) -> Option<PathBuf> {
        self.resource_candidates(relative)
            .into_iter()
            .find(|c| c.exists())
    }
}

/// Resolved input/output paths for one run, ready to record in a [`crate::run_manifest::RunManifest`].
#[derive(Clone, Debug, Default)]
pub struct ResolvedProjectPaths {
    pub base_dir: PathBuf,
    /// `(role, resolved path)` pairs, e.g. `("landtype_file", "/data/landtype.nc")`.
    pub inputs: Vec<(String, PathBuf)>,
    pub outputs: Vec<(String, PathBuf)>,
}

/// A single required (or optional) input file and how a user would point at it.
#[derive(Clone, Debug)]
pub struct InputDataCheck {
    /// Human-facing role, e.g. `"landtype_file"` or `"merit_root"`.
    pub role: String,
    pub path: PathBuf,
    pub required: bool,
    /// What the user sets to fix a miss, e.g. `"NL%landtype_file"` or `"EARTHMESH_DATA"`.
    pub config_key: String,
}

impl InputDataCheck {
    pub fn required(role: &str, path: impl Into<PathBuf>, config_key: &str) -> Self {
        Self {
            role: role.to_string(),
            path: path.into(),
            required: true,
            config_key: config_key.to_string(),
        }
    }

    pub fn optional(role: &str, path: impl Into<PathBuf>, config_key: &str) -> Self {
        Self {
            role: role.to_string(),
            path: path.into(),
            required: false,
            config_key: config_key.to_string(),
        }
    }
}

/// Validate that all required inputs exist before a run. On failure returns an
/// actionable, multi-line message naming each missing file and the key to set.
pub fn validate_paths_before_run(checks: &[InputDataCheck]) -> Result<(), String> {
    let missing: Vec<&InputDataCheck> = checks
        .iter()
        .filter(|c| c.required && !c.path.exists())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let mut msg = String::from("Cannot start run — missing required input data:\n");
    for c in &missing {
        msg.push_str(&format!(
            "  - {} not found at '{}'.\n    Fix: set {} to an existing path (or place the file there).\n",
            c.role,
            c.path.display(),
            c.config_key,
        ));
    }
    Err(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_resolves_under_base_and_absolute_passes_through() {
        let r = PathResolver::new("/tmp/case");
        assert_eq!(
            r.resolve("input/x.nc"),
            PathBuf::from("/tmp/case/input/x.nc")
        );
        assert_eq!(r.resolve("/abs/y.nc"), PathBuf::from("/abs/y.nc"));
        // leading/trailing whitespace is trimmed (guards the legacy " /tmp" default).
        assert_eq!(r.resolve("  rel.nc  "), PathBuf::from("/tmp/case/rel.nc"));
    }

    #[test]
    fn resource_candidates_end_with_base_join() {
        let r = PathResolver::new("/dev/repo");
        let cands = r.resource_candidates("examples");
        assert_eq!(cands.last().unwrap(), &PathBuf::from("/dev/repo/examples"));
    }

    #[test]
    fn missing_required_input_reports_actionable_error() {
        let checks = vec![InputDataCheck::required(
            "merit_root",
            "/no/such/merit_hydro",
            "EARTHMESH_DATA / NL hydro root",
        )];
        let err = validate_paths_before_run(&checks).unwrap_err();
        assert!(err.contains("merit_root"));
        assert!(err.contains("/no/such/merit_hydro"));
        assert!(err.contains("EARTHMESH_DATA"));
        assert!(err.contains("Fix:"));
    }

    #[test]
    fn optional_missing_input_does_not_fail() {
        let checks = vec![InputDataCheck::optional(
            "landtype_file",
            "/no/such.nc",
            "NL%landtype_file",
        )];
        assert!(validate_paths_before_run(&checks).is_ok());
    }
}
