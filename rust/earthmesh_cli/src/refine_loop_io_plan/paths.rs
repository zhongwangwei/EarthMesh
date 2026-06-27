use std::path::{Path, PathBuf};

pub(crate) fn mkgrd_gridfile_path(
    file_dir: &Path,
    nxp: usize,
    step: usize,
    mode_grid: &str,
) -> PathBuf {
    file_dir
        .join("gridfile")
        .join(format!("gridfile_NXP{nxp:04}_{step:02}_{mode_grid}.nc4"))
}

pub(crate) fn mkgrd_tmpfile_path(
    file_dir: &Path,
    nxp: usize,
    step: usize,
    suffix: &str,
) -> PathBuf {
    file_dir
        .join("tmpfile")
        .join(format!("gridfile_NXP{nxp:04}_{step:02}_{suffix}.nc4"))
}
