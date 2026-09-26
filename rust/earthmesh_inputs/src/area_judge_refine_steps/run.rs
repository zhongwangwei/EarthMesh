use crate::AreaJudgeRefineStepReport;
use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::{
    activation::activate_area_judge_calculated_refine_one_based,
    specified::build_area_judge_specified_refine_one_based,
    support::count_area_judge_selected_cells_one_based,
};

/// Dispatch `MOD_Area_judge.F90:Area_judge_refine(iter)` for iter zero or specified refine steps.
pub fn run_area_judge_refine_one_based<D>(
    file_dir: impl AsRef<Path>,
    iter: usize,
    calculated_refine: Option<(&[Vec<bool>], AreaJudgeSourceBounds)>,
    mask_refine_spc_type: &str,
    mask_refine_ndm: usize,
    is_in_domain: &[Vec<D>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeRefineStepReport>
where
    D: Copy + Into<i32>,
{
    if iter == 0 {
        let (is_in_refine_calculated, bounds) = calculated_refine.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Area_judge_refine(iter=0) requires calculated refine state",
            )
        })?;
        let activation =
            activate_area_judge_calculated_refine_one_based(is_in_refine_calculated, bounds)?;
        return Ok(AreaJudgeRefineStepReport {
            is_in_refine: activation.is_in_refine,
            bounds: activation.bounds,
            nlons_select: activation.nlons_select,
            nlats_select: activation.nlats_select,
            selected_cells: activation.selected_cells,
            source_numpatch: None,
        });
    }

    let specified = build_area_judge_specified_refine_one_based(
        file_dir,
        iter,
        mask_refine_spc_type,
        mask_refine_ndm,
        is_in_domain,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )?;
    let nlons_select = specified.bounds.maxlon_source - specified.bounds.minlon_source + 1;
    let nlats_select = specified.bounds.minlat_source - specified.bounds.maxlat_source + 1;
    let selected_cells =
        count_area_judge_selected_cells_one_based(&specified.is_in_area, specified.bounds);

    Ok(AreaJudgeRefineStepReport {
        is_in_refine: specified.is_in_area,
        bounds: specified.bounds,
        nlons_select,
        nlats_select,
        selected_cells,
        source_numpatch: Some(specified.numpatch),
    })
}
