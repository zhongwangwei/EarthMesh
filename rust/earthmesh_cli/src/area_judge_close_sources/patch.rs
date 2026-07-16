use crate::AreaJudgePatchSourceReport;
use std::io;
use std::path::Path;

use earthmesh_mesh::area_judge_apply_mask_patch_one_based;

use super::area::build_area_judge_close_area_source_one_based;

/// Build the close-curve `IsInPaArea_grid` patch mask and apply it to `seaorland`.
pub fn apply_area_judge_close_patch_source_one_based(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<bool>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    let source = build_area_judge_close_area_source_one_based(
        inputfile,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .map_err(|err| {
        io::Error::new(
            err.kind(),
            err.to_string().replace("close area", "close patch"),
        )
    })?;
    let report =
        area_judge_apply_mask_patch_one_based(seaorland, &source.is_in_area, source.bounds)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "seaorland or close patch mask does not cover selected source bounds",
                )
            })?;

    Ok(AreaJudgePatchSourceReport {
        bounds: source.bounds,
        patched_cells: report.patched_cells,
    })
}
