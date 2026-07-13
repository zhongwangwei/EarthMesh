use std::path::PathBuf;

/// Initial grid selected for the default restart-to-Method-C-direct handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdDefaultRestartRefineHandoff {
    pub initial_gridfile: PathBuf,
}
