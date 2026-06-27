use std::io;
use std::path::Path;

use earthmesh_mesh::area_judge_apply_mask_patch_fortran_indexed;

use super::area::build_area_judge_circle_area_source_fortran_indexed;
use crate::*;

/// Build the circle `IsInPaArea_grid` patch mask and apply it to `seaorland`.
pub fn apply_area_judge_circle_patch_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    let source = build_area_judge_circle_area_source_fortran_indexed(
        inputfile,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .map_err(|err| {
        io::Error::new(
            err.kind(),
            err.to_string().replace("circle area", "circle patch"),
        )
    })?;
    let report =
        area_judge_apply_mask_patch_fortran_indexed(seaorland, &source.is_in_area, source.bounds)
            .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seaorland or circle patch mask does not cover selected source bounds",
            )
        })?;

    Ok(AreaJudgePatchSourceReport {
        bounds: source.bounds,
        patched_cells: report.patched_cells,
    })
}
