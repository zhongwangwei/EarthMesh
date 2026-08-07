use std::path::PathBuf;

use earthmesh_core::{EarthmeshRuntimeState, RefineConfig};

use crate::{
    ColmCouplingNetcdfWriteReport, ColmSurfaceCounts, RefinementRegion, UnstructuredMeshWriteReport,
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
    pub regions: Vec<RefinementRegion>,
    /// Refinement depth the configuration asked for, derived from
    /// `max_iter_spc`/`max_iter_cal` before any refinement runs.
    pub max_level: usize,
    /// Deepest refinement level actually present in the produced mesh.
    ///
    /// A pass whose demand is clipped away — for example an h-field anchor with
    /// no complete rad3 footprint — stops descending without failing the run, so
    /// this can be lower than [`Self::max_level`]. Reporting only the requested
    /// depth made that outcome indistinguishable from a fully realized one.
    pub realized_max_level: usize,
    /// The 2nd and 98th percentile cell width of the produced mesh, in km
    /// across (`sqrt(A/pi)`, the radius of the disc with the cell's area).
    ///
    /// Percentiles rather than extremes: the mask carve leaves partial cells
    /// at a coastline, and taking the minimum reported a 2.4 km sliver on a
    /// mesh whose cells are nominally 300 km -- so `log2(max/min)` said twelve
    /// halvings for a two-level request.
    ///
    /// Backend-neutral, because it is measured off the mesh rather than taken
    /// from each backend's own bookkeeping. `realized_max_level` is not:
    /// Method-C counts nesting passes, red-green reports zero meaning "not
    /// measured", and HARP-DV counts site generations, and all three print
    /// into that one field. A run that refined to 2.6 halvings reported level
    /// 1 for several rounds because of it (guide 11.19).
    ///
    /// `log2(coarsest / finest)` is the halvings actually achieved, which is
    /// what a request in levels was asking for.
    pub finest_cell_km: f64,
    pub coarsest_cell_km: f64,
    /// What the h-field asked for versus what survived Method-C legality, summed
    /// over passes. All zero for the geometric region paths.
    pub hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics,
    pub transition_faces: usize,
    pub spring_nest_passes: usize,
    pub spring_nest_iterations: usize,
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
