use std::path::PathBuf;

use earthmesh_core::{EarthmeshRuntimeState, RefineConfig};

use crate::{
    ColmCouplingNetcdfWriteReport, ColmSurfaceCounts, MkgrdRefineSourceBranchReport,
    OlamRefinementRegion, UnstructuredMeshWriteReport,
};

use super::MkgrdGridinitRunReport;

/// Evidence from the direct OLAM specified-refinement path.
///
/// This bypasses the legacy refine-loop executor: the global Delaunay mesh is
/// rebuilt in the OLAM layer, specified regions are applied with `spawn_nest`,
/// and the existing EarthMesh NetCDF gridfile schema is used only at the final
/// output boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdOlamCoupledOutputReport {
    pub land_output: UnstructuredMeshWriteReport,
    pub ocean_output: UnstructuredMeshWriteReport,
    pub coupling_csv: PathBuf,
    pub coupling_netcdf: ColmCouplingNetcdfWriteReport,
    pub manifest: PathBuf,
    pub counts: ColmSurfaceCounts,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdOlamSpecifiedRefineRunReport {
    pub gridinit: MkgrdGridinitRunReport,
    pub refine: RefineConfig,
    pub regions: Vec<OlamRefinementRegion>,
    pub max_level: usize,
    pub transition_faces: usize,
    pub spring_nest_passes: usize,
    pub spring_nest_iterations: usize,
    pub raw_output: Option<UnstructuredMeshWriteReport>,
    pub landtype_masked_cells: Option<usize>,
    pub coupled_outputs: Option<MkgrdOlamCoupledOutputReport>,
    pub output: UnstructuredMeshWriteReport,
    pub runtime_state: EarthmeshRuntimeState,
}

impl MkgrdOlamSpecifiedRefineRunReport {
    pub fn source_branch_reports(&self) -> &[MkgrdRefineSourceBranchReport] {
        &[]
    }

    pub fn runtime_state(&self) -> &EarthmeshRuntimeState {
        &self.runtime_state
    }
}
