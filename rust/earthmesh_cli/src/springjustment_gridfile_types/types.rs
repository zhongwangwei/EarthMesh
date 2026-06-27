use earthmesh_mesh::{
    DistanceLayerSpacing, GlobalDistanceStep, SpringjustmentGlobalCoreOutput,
    SpringjustmentRegionalCoreOutput,
};

use crate::{CellwidthWriteReport, DistsOnEdgeWriteReport, UnstructuredMesh};

/// Evidence report from writing `MOD_grid_preprocess.F90:Springjustment_global`
/// persistence side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpringjustmentGlobalPersistenceReport {
    pub dists_on_edge: DistsOnEdgeWriteReport,
    pub cellwidth: Option<CellwidthWriteReport>,
}

/// Runtime controls needed to reproduce the migrated
/// `MOD_grid_preprocess.F90:Springjustment_global` calculation from a gridfile.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentGlobalRunOptions<'a> {
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub distance_num_rc: usize,
    pub distance_spacing: DistanceLayerSpacing,
    pub distance_steps: &'a [GlobalDistanceStep<'a>],
    pub niter_refine: usize,
    pub relax: f64,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Evidence report from the gridfile-backed Rust replacement path for
/// `MOD_grid_preprocess.F90:Springjustment_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentGlobalGridfileReport {
    pub core: SpringjustmentGlobalCoreOutput,
    pub persistence: SpringjustmentGlobalPersistenceReport,
    pub mesh: UnstructuredMesh,
}

/// Runtime controls needed to reproduce the migrated
/// `MOD_grid_preprocess.F90:Springjustment_regional_step` calculation from a
/// gridfile when the regional move mask has already been resolved.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalRunOptions<'a> {
    pub move_mask: &'a [bool],
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Evidence report from the gridfile-backed Rust replacement path for
/// `MOD_grid_preprocess.F90:Springjustment_regional_step`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalGridfileReport {
    pub core: SpringjustmentRegionalCoreOutput,
    pub mesh: UnstructuredMesh,
}
