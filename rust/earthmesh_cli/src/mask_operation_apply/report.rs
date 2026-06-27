use std::path::PathBuf;

/// Result of applying one Rust `Mask_make` operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaskOperationReport {
    pub sources: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
}
