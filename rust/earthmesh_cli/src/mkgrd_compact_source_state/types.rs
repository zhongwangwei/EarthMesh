use std::{io, path::Path};

use crate::{
    build_global_source_axes_fortran_indexed, AreaJudgeSourceBounds, GetContainMeshKind,
    GlobalSourceAxes, MkgrdAreaJudgeRestartRefineLoopOptions,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MkgrdCompactSourceStateFinalPostproc {
    Land,
    Ocean,
    Atmos,
    Earth,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdCompactSourceState {
    pub gridnum_perdegree: usize,
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub first_triangle_id: usize,
    pub num_vertex: usize,
    pub maxlc: i32,
    pub final_domain_contain: Option<GetContainMeshKind>,
    pub final_domain_postproc: Option<MkgrdCompactSourceStateFinalPostproc>,
    pub calculated_refine: Option<Vec<Vec<i32>>>,
    pub calculated_bounds: Option<AreaJudgeSourceBounds>,
    pub is_in_domain: Vec<Vec<i32>>,
    pub seaorland: Vec<Vec<i32>>,
    pub landtypes_global: Vec<Vec<i32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdCompactRestartRefineSourceState {
    pub source_state: MkgrdCompactSourceState,
    pub axes: GlobalSourceAxes,
}

impl MkgrdCompactSourceState {
    pub fn build_global_source_axes(&self) -> io::Result<GlobalSourceAxes> {
        build_global_source_axes_fortran_indexed(
            self.gridnum_perdegree,
            self.nlons_source,
            self.nlats_source,
        )
    }
}

impl MkgrdCompactRestartRefineSourceState {
    pub fn area_judge_restart_refine_loop_options<'a>(
        &'a self,
        restart_input: &'a Path,
        initial_gridfile: &'a Path,
    ) -> MkgrdAreaJudgeRestartRefineLoopOptions<'a> {
        MkgrdAreaJudgeRestartRefineLoopOptions {
            restart_input,
            initial_gridfile,
            source_grid: self
                .axes
                .refine_prepare_source_grid(self.source_state.first_triangle_id),
            landtypes_global: &self.source_state.landtypes_global,
            num_vertex: self.source_state.num_vertex,
            maxlc: self.source_state.maxlc,
        }
    }
}
