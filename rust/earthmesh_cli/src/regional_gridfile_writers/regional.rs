use std::io;
use std::path::Path;

use crate::*;

/// Carve a global gridfile down to `region` and write the regional gridfile, in
/// pure Rust. Reuses the engine's mask-postproc compaction/re-index: cells whose
/// centre is outside the region are dropped and the mesh is renumbered. The
/// gridfile's leading placeholder (Fortran id 1 / array index 0) is preserved.
/// Returns the number of cells kept. `mode_grid` selects the primal cells
/// (`hex` -> hexagons / W cells, `tri` -> triangles / M cells).
pub fn write_regional_gridfile(
    global_gridfile: impl AsRef<Path>,
    regional_gridfile: impl AsRef<Path>,
    region: &GridRegion,
    mode_grid: &str,
) -> io::Result<usize> {
    let mesh = read_unstructured_mesh_netcdf(global_gridfile)?;
    let raw_layout = mask_postproc_layout_from_unstructured_mesh(&mesh, mode_grid)?;
    let layout = ensure_leading_mask_postproc_placeholder(raw_layout);
    let mut is_in_domain = vec![-1i32; layout.ustr_points];
    let mut kept = 0usize;
    if !is_in_domain.is_empty() {
        is_in_domain[0] = 0;
    }
    if is_in_domain.len() > 1 {
        is_in_domain[1] = 0;
    }
    for i in 2..layout.ustr_points {
        let c = layout.center_points[i];
        if region.contains(c.lon, c.lat) {
            is_in_domain[i] = 1;
            kept += 1;
        }
    }
    let report =
        finalize_mask_postproc_layout_with_reindex_report(&layout, &is_in_domain, mode_grid)?;
    write_unstructured_mesh_netcdf(regional_gridfile, &report.mesh)?;
    Ok(kept)
}
