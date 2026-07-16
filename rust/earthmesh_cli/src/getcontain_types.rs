use std::path::{Path, PathBuf};

use crate::ContainWriteReport;

/// Runtime inputs for a file-backed `MOD_GetContain.F90:Get_Contain(iter)`
/// refinement pass. The input gridfile is the current triangular refine grid,
/// and `area_grid_file` is the selected `Area_judge_refine(iter)` grid.
#[derive(Debug, Clone, Copy)]
pub struct GetContainRefineFileRunConfig<'a> {
    pub gridfile: &'a Path,
    pub area_grid_file: &'a Path,
    pub output: &'a Path,
    pub mesh_kind: GetContainMeshKind,
    pub seaorland: &'a [Vec<bool>],
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub num_vertex: usize,
}

/// Explicit runtime counter handoff that Canonical stored in
/// `consts_coms:num_mp_step`, `num_wp_step`, and `num_vertex` during
/// `MOD_GetContain.F90:Get_Contain(iter)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetContainRuntimeCounts {
    pub current_num_mp_step: usize,
    pub current_num_wp_step: usize,
    pub previous_num_vertex: usize,
}

/// Evidence from writing one refine `Contain_Save` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetContainRefineFileRunReport {
    pub output: PathBuf,
    pub active_unstructured_cells: usize,
    pub contained_source_pixels: usize,
    pub runtime_counts: GetContainRuntimeCounts,
    pub write: ContainWriteReport,
}

/// Axis-aligned domain/refinement bounds used by
/// `MOD_GetContain.F90:IsInArea_ustr_Calculation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GetContainAreaBounds {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
}

/// Mesh-specific containment semantics from
/// `MOD_GetContain.F90:Contain_Calculation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetContainMeshKind {
    Land,
    Ocean,
    Atmos,
    Loc,
}

impl GetContainMeshKind {
    pub(crate) fn ustr_id_width(self) -> usize {
        match self {
            Self::Ocean | Self::Loc => 3,
            Self::Land | Self::Atmos => 2,
        }
    }

    pub(crate) fn ustr_ii_width(self) -> usize {
        match self {
            Self::Atmos | Self::Loc => 3,
            Self::Land | Self::Ocean => 2,
        }
    }
}
