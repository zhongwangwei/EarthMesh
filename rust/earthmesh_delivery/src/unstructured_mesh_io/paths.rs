use std::path::{Path, PathBuf};

/// Build the `gridfile_write` output path:
/// `file_dir/gridfile/gridfile_NXP####_##_<mode_grid>.nc4`.
pub fn gridfile_output_path(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
) -> PathBuf {
    file_dir.as_ref().join("gridfile").join(format!(
        "gridfile_NXP{nxp:04}_{step:02}_{}.nc4",
        mode_grid.trim()
    ))
}
