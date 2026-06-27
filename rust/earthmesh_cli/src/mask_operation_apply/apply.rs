use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::MaskOperation;

use crate::{
    convert_lambert_mask_netcdf, discover_mask_sources, parse_bbox_mask_nml, parse_circle_mask_nml,
    parse_close_mask_nml, read_bbox_refine_netcdf, read_circle_refine_netcdf,
    read_close_refine_netcdf, source_extension, unsupported_mask_source, write_bbox_mask_netcdf,
    write_circle_mask_netcdf, write_close_mask_netcdf, MaskCountState,
};

use super::copy::{
    copy_bbox_mask_netcdf_with_refine, copy_circle_mask_netcdf_with_refine,
    copy_close_mask_netcdf_with_refine,
};
use super::report::MaskOperationReport;

/// Apply one `mkgrd.F90:Mask_make(mask_select, type_select, mask_fprefix)` call.
pub fn apply_mask_operation(
    operation: &MaskOperation,
    file_dir: impl AsRef<Path>,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<MaskOperationReport> {
    let discovery = discover_mask_sources(&operation.mask_fprefix)?;
    if discovery.files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no mask sources matched mask_fprefix",
        ));
    }

    let file_dir = file_dir.as_ref();
    let mut report = MaskOperationReport {
        sources: discovery.files.clone(),
        outputs: Vec::new(),
    };

    for source in discovery.files {
        let output = match operation.type_select.as_str() {
            "bbox" => apply_bbox_source(
                &source,
                &operation.mask_select,
                file_dir,
                max_iter_spc,
                counts,
            )?,
            "circle" => apply_circle_source(
                &source,
                &operation.mask_select,
                file_dir,
                max_iter_spc,
                counts,
            )?,
            "close" => apply_close_source(
                &source,
                &operation.mask_select,
                file_dir,
                max_iter_spc,
                counts,
            )?,
            "lambert" => Some(convert_lambert_mask_netcdf(
                &source,
                &operation.mask_select,
                file_dir,
                counts,
            )?),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported type_select {other}"),
                ));
            }
        };
        if let Some(output) = output {
            report.outputs.push(output);
        }
    }

    Ok(report)
}

fn apply_bbox_source(
    source: &Path,
    mask_select: &str,
    file_dir: &Path,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    match source_extension(source).as_deref() {
        Some("nml") => {
            let Some(mask) = parse_bbox_mask_nml(source, max_iter_spc)? else {
                return Ok(None);
            };
            let output = counts.next_bbox_output(mask_select, mask.refine_degree, file_dir)?;
            write_bbox_mask_netcdf(&output, &mask)?;
            Ok(Some(output))
        }
        Some("nc") | Some("nc4") => {
            let refine = read_bbox_refine_netcdf(source)?;
            copy_bbox_mask_netcdf_with_refine(
                source,
                mask_select,
                refine,
                max_iter_spc,
                file_dir,
                counts,
            )
        }
        _ => Err(unsupported_mask_source(source)),
    }
}

fn apply_circle_source(
    source: &Path,
    mask_select: &str,
    file_dir: &Path,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    match source_extension(source).as_deref() {
        Some("nml") => {
            let Some(mask) = parse_circle_mask_nml(source, max_iter_spc)? else {
                return Ok(None);
            };
            let output = counts.next_circle_output(mask_select, mask.refine_degree, file_dir)?;
            write_circle_mask_netcdf(&output, &mask)?;
            Ok(Some(output))
        }
        Some("nc") | Some("nc4") => {
            let refine = read_circle_refine_netcdf(source)?;
            copy_circle_mask_netcdf_with_refine(
                source,
                mask_select,
                refine,
                max_iter_spc,
                file_dir,
                counts,
            )
        }
        _ => Err(unsupported_mask_source(source)),
    }
}

fn apply_close_source(
    source: &Path,
    mask_select: &str,
    file_dir: &Path,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    match source_extension(source).as_deref() {
        Some("nml") => {
            let Some(mask) = parse_close_mask_nml(source, max_iter_spc)? else {
                return Ok(None);
            };
            let output = counts.next_close_output(mask_select, mask.refine_degree, file_dir)?;
            write_close_mask_netcdf(&output, &mask)?;
            Ok(Some(output))
        }
        Some("nc") | Some("nc4") => {
            let refine = read_close_refine_netcdf(source)?;
            copy_close_mask_netcdf_with_refine(
                source,
                mask_select,
                refine,
                max_iter_spc,
                file_dir,
                counts,
            )
        }
        _ => Err(unsupported_mask_source(source)),
    }
}
