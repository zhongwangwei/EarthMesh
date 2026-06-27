use std::io;

use crate::{MkgrdRefinePrepareSourceGridOptions, MkgrdRestartAreaJudgeOptions};

/// One-based global lon/lat source axes reconstructed from source dimensions.
///
/// This is the Rust-owned replacement for restart/refine paths that used to
/// rebuild `data_preprocess`-style source coordinate arrays in the CLI front-end
/// before calling migrated `mkgrd` kernels.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalSourceAxes {
    pub lon_vertex: Vec<f64>,
    pub lat_vertex: Vec<f64>,
    pub lon_i: Vec<f64>,
    pub lat_i: Vec<f64>,
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
}

impl GlobalSourceAxes {
    pub fn refine_prepare_source_grid(
        &self,
        first_triangle_id: usize,
    ) -> MkgrdRefinePrepareSourceGridOptions<'_> {
        MkgrdRefinePrepareSourceGridOptions {
            lon_vertex: &self.lon_vertex,
            lat_vertex: &self.lat_vertex,
            lon_i: &self.lon_i,
            lat_i: &self.lat_i,
            gridnum_perdegree: self.gridnum_perdegree,
            nlons_source: self.nlons_source,
            nlats_source: self.nlats_source,
            first_triangle_id,
        }
    }

    pub fn restart_area_judge_options(&self) -> MkgrdRestartAreaJudgeOptions<'_> {
        MkgrdRestartAreaJudgeOptions {
            lon_vertex: &self.lon_vertex,
            lat_vertex: &self.lat_vertex,
            lon_i: &self.lon_i,
            lat_i: &self.lat_i,
            gridnum_perdegree: self.gridnum_perdegree,
            nlons_source: self.nlons_source,
            nlats_source: self.nlats_source,
        }
    }
}

/// Build Fortran-indexed global source axes for migrated restart/refine handoffs.
pub fn build_global_source_axes_fortran_indexed(
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<GlobalSourceAxes> {
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive for source axes",
        ));
    }
    let step = 1.0 / gridnum_perdegree as f64;
    let lon_vertex = std::iter::once(f64::NAN)
        .chain((0..=nlons_source).map(|idx| -180.0 + idx as f64 * step))
        .collect::<Vec<_>>();
    let lat_vertex = std::iter::once(f64::NAN)
        .chain((0..=nlats_source).map(|idx| 90.0 - idx as f64 * step))
        .collect::<Vec<_>>();
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..nlons_source).map(|idx| -180.0 + (idx as f64 + 0.5) * step))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..nlats_source).map(|idx| 90.0 - (idx as f64 + 0.5) * step))
        .collect::<Vec<_>>();
    Ok(GlobalSourceAxes {
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    })
}
