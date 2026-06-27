use std::io;
use std::path::{Path, PathBuf};

pub(super) fn area_judge_patch_source_path(
    file_dir: impl AsRef<Path>,
    mask_patch_type: &str,
    iter: usize,
    source_index: usize,
) -> io::Result<PathBuf> {
    let count_width = match mask_patch_type {
        "close" => 3,
        "bbox" | "circle" | "lambert" => 2,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported mask_patch_type {other}"),
            ));
        }
    };
    Ok(file_dir.as_ref().join("tmpfile").join(format!(
        "mask_patch_{mask_patch_type}_{iter}_{source_index:0count_width$}.nc4"
    )))
}

pub(crate) fn area_judge_area_source_path(
    file_dir: impl AsRef<Path>,
    type_select: &str,
    mask_type: &str,
    iter: usize,
    source_index: usize,
) -> io::Result<PathBuf> {
    let count_width = match mask_type {
        "close" => 3,
        "bbox" | "circle" | "lambert" => 2,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported mask_type {other}"),
            ));
        }
    };
    Ok(file_dir.as_ref().join("tmpfile").join(format!(
        "{type_select}_{mask_type}_{iter}_{source_index:0count_width$}.nc4"
    )))
}
