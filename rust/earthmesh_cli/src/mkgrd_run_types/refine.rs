use std::path::PathBuf;

use earthmesh_core::{EarthmeshRuntimeState, RefineConfig};
use earthmesh_mesh::{MethodCHfieldPassDiagnostics, MethodCNestSpringDiagnostics};

use crate::{
    ColmCouplingNetcdfWriteReport, ColmSurfaceCounts, MethodCRefinementRegion,
    UnstructuredMeshWriteReport,
};

use super::MkgrdGridinitRunReport;

/// Outputs and runtime evidence from the adaptive refinement pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineCoupledOutputReport {
    pub land_output: UnstructuredMeshWriteReport,
    pub ocean_output: UnstructuredMeshWriteReport,
    pub coupling_csv: PathBuf,
    pub coupling_netcdf: ColmCouplingNetcdfWriteReport,
    pub coupling_quality: PathBuf,
    pub manifest: PathBuf,
    pub counts: ColmSurfaceCounts,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefinePipelineRunReport {
    pub gridinit: MkgrdGridinitRunReport,
    pub refine: RefineConfig,
    pub regions: Vec<MethodCRefinementRegion>,
    /// Requested/configured refinement depth.
    pub max_level: usize,
    /// Highest refinement level present among active cells in the final output
    /// (`M` cells for `tri`, `W` cells for `hex`).
    pub actual_max_level: usize,
    /// Number of active cells in the final output with a positive refinement
    /// level (`M` cells for `tri`, `W` cells for `hex`).
    pub refined_cells: usize,
    pub transition_faces: usize,
    pub spring_nest_passes: usize,
    pub spring_nest_iterations: usize,
    pub spring_diagnostics: Vec<MethodCNestSpringDiagnostics>,
    pub hfield_pass_diagnostics: Vec<MethodCHfieldPassDiagnostics>,
    pub raw_output: Option<UnstructuredMeshWriteReport>,
    pub landtype_masked_cells: Option<usize>,
    pub coupled_outputs: Option<RefineCoupledOutputReport>,
    pub output: UnstructuredMeshWriteReport,
    pub runtime_state: EarthmeshRuntimeState,
}

impl RefinePipelineRunReport {
    pub fn refine_stack(&self) -> &'static str {
        "refine_pipeline"
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        &self.runtime_state
    }

    /// Unmasked gridfile that retains the complete Method-C topology needed by
    /// a later local-refinement handoff.
    pub fn refinement_parent_gridfile(&self) -> &std::path::Path {
        self.raw_output
            .as_ref()
            .unwrap_or(&self.output)
            .output
            .as_path()
    }
}
