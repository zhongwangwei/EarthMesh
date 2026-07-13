use crate::classify_area_judge_landtype_one_based;
use crate::ensure_leading_mask_postproc_placeholder;
use crate::finalize_mask_postproc_layout_with_reindex_report;
use crate::mask_postproc_layout_from_unstructured_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::sample_landtype_values_for_points_one_based;
use crate::write_unstructured_mesh_netcdf_with_method_c_metadata;
use crate::AreaJudgeLandtypeClass;
use std::io;
use std::path::Path;

use super::levels::{final_method_c_metadata_for_mask_postproc, refine_levels_from_gridfile};

/// Carve a gridfile to land-only or ocean-only cells from a land-type NetCDF.
///
/// This is the direct, post-run counterpart of the data_preprocess/Area_judge
/// sea-land classification: sample each grid cell centre against `landtype_file`,
/// keep land cells for `landmesh`, keep ocean cells for `oceanmesh`, then use the
/// existing mask-postproc finalization path so connectivity is compacted and
/// renumbered consistently. Returns the number of cells kept.
pub fn write_landtype_masked_gridfile(
    input_gridfile: impl AsRef<Path>,
    output_gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mode_grid: &str,
    mesh_type: &str,
) -> io::Result<usize> {
    write_landtype_masked_gridfile_with_refine_levels(
        input_gridfile,
        output_gridfile,
        landtype_file,
        gridnum_perdegree,
        mode_grid,
        mesh_type,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn write_landtype_masked_gridfile_with_refine_levels(
    input_gridfile: impl AsRef<Path>,
    output_gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mode_grid: &str,
    mesh_type: &str,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
) -> io::Result<usize> {
    let keep_land = match mesh_type.trim() {
        "landmesh" => true,
        "oceanmesh" => false,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("landtype masked gridfile supports landmesh or oceanmesh, got {other}"),
            ));
        }
    };
    let input_gridfile = input_gridfile.as_ref();
    let mesh = read_unstructured_mesh_netcdf(input_gridfile)?;
    let raw_layout = mask_postproc_layout_from_unstructured_mesh(&mesh, mode_grid)?;
    let layout = ensure_leading_mask_postproc_placeholder(raw_layout);
    let landtype_values = sample_landtype_values_for_points_one_based(
        landtype_file,
        gridnum_perdegree,
        &layout.center_points[2..],
    )?;

    let mut is_in_domain = vec![-1i32; layout.ustr_points];
    let mut kept = 0usize;
    if !is_in_domain.is_empty() {
        is_in_domain[0] = 0;
    }
    if is_in_domain.len() > 1 {
        is_in_domain[1] = 0;
    }
    for (i, landtype_value) in (2..layout.ustr_points).zip(landtype_values) {
        let is_land = matches!(
            classify_area_judge_landtype_one_based(landtype_value),
            AreaJudgeLandtypeClass::Land
        );
        if is_land == keep_land {
            is_in_domain[i] = 1;
            kept += 1;
        }
    }
    if kept == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{mesh_type} landtype mask kept no cells"),
        ));
    }
    let report =
        finalize_mask_postproc_layout_with_reindex_report(&layout, &is_in_domain, mode_grid)?;
    let mut source_metadata = refine_levels_from_gridfile(input_gridfile)?;
    if let Some(levels) = m_refine_level {
        source_metadata.m = levels.to_vec();
    }
    if let Some(levels) = w_refine_level {
        source_metadata.w = levels.to_vec();
    }
    let final_metadata = final_method_c_metadata_for_mask_postproc(
        mode_grid,
        &report,
        &is_in_domain,
        layout.ustr_points,
        &source_metadata,
    )?;
    write_unstructured_mesh_netcdf_with_method_c_metadata(
        output_gridfile,
        &report.mesh,
        final_metadata.slices(),
    )?;
    Ok(kept)
}
