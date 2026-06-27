use std::path::{Path, PathBuf};

/// Legacy output path for `MOD_mask_postproc.F90:bdy_calculation`.
pub fn obc_boundary_output_path(file_dir: impl AsRef<Path>, mask_patch_on: bool) -> PathBuf {
    let filename = if mask_patch_on {
        "obc_patch.nc4"
    } else {
        "obc.nc4"
    };
    file_dir.as_ref().join("result").join(filename)
}

/// Legacy output path for `MOD_mask_postproc.F90:bdy_connection`.
pub fn obcv2_boundary_output_path(file_dir: impl AsRef<Path>, mask_patch_on: bool) -> PathBuf {
    let filename = if mask_patch_on {
        "obcv2_patch.nc4"
    } else {
        "obcv2.nc4"
    };
    file_dir.as_ref().join("result").join(filename)
}
