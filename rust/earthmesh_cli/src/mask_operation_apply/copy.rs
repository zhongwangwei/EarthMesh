use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::MaskCountState;

/// Copy a circle NetCDF source into the Fortran tmpfile naming scheme.
pub fn copy_circle_mask_netcdf_with_refine(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    refine_degree: usize,
    max_iter_spc: usize,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    copy_mask_netcdf_with_output(
        inputfile,
        refine_degree,
        max_iter_spc,
        |counts, refine_degree, file_dir| {
            counts.next_circle_output(mask_select, refine_degree, file_dir)
        },
        file_dir,
        counts,
    )
}

/// Copy a close NetCDF source into the Fortran tmpfile naming scheme.
pub fn copy_close_mask_netcdf_with_refine(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    refine_degree: usize,
    max_iter_spc: usize,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    copy_mask_netcdf_with_output(
        inputfile,
        refine_degree,
        max_iter_spc,
        |counts, refine_degree, file_dir| {
            counts.next_close_output(mask_select, refine_degree, file_dir)
        },
        file_dir,
        counts,
    )
}

/// Copy a bbox NetCDF source into the Fortran tmpfile naming scheme.
///
/// This covers the `bbox_mask_make` `.nc/.nc4` branch after the caller has
/// obtained `bbox_refine` from the NetCDF metadata. If `refine_degree` is above
/// `max_iter_spc`, the function returns `Ok(None)` and leaves counters/files
/// untouched, matching the Fortran early return.
pub fn copy_bbox_mask_netcdf_with_refine(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    refine_degree: usize,
    max_iter_spc: usize,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    copy_mask_netcdf_with_output(
        inputfile,
        refine_degree,
        max_iter_spc,
        |counts, refine_degree, file_dir| {
            counts.next_bbox_output(mask_select, refine_degree, file_dir)
        },
        file_dir,
        counts,
    )
}

fn copy_mask_netcdf_with_output<F>(
    inputfile: impl AsRef<Path>,
    refine_degree: usize,
    max_iter_spc: usize,
    output_fn: F,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>>
where
    F: FnOnce(&mut MaskCountState, usize, &Path) -> io::Result<PathBuf>,
{
    let inputfile = inputfile.as_ref();
    let extension = inputfile.extension().and_then(|value| value.to_str());
    if !matches!(extension, Some("nc") | Some("nc4")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask NetCDF input must end with .nc or .nc4",
        ));
    }
    if refine_degree > max_iter_spc {
        return Ok(None);
    }
    let output = output_fn(counts, refine_degree, file_dir.as_ref())?;
    crate::ensure_parent_dir(&output)?;
    fs::copy(inputfile, &output)?;
    Ok(Some(output))
}
