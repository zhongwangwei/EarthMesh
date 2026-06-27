use std::io;
use std::path::Path;

use crate::*;

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
    let mesh = read_unstructured_mesh_netcdf(input_gridfile)?;
    let raw_layout = mask_postproc_layout_from_unstructured_mesh(&mesh, mode_grid)?;
    let layout = ensure_leading_mask_postproc_placeholder(raw_layout);
    let landtype_values = sample_landtype_values_for_points_fortran_indexed(
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
            classify_area_judge_landtype_fortran_indexed(landtype_value),
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
    write_unstructured_mesh_netcdf(output_gridfile, &report.mesh)?;
    Ok(kept)
}
