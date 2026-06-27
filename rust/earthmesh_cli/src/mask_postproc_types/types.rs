use std::path::PathBuf;

use earthmesh_mesh::{
    BoundaryConnection, BoundaryOrders, IsolatedOceanRenewal, MaskPostprocRenewedData,
};

use crate::{
    ContainMesh, EarthmeshInfoWriteReport, LonLatPoint, ObcBoundaryWriteReport,
    Obcv2BoundaryWriteReport, PatchIdWriteReport, UnstructuredMesh, UnstructuredMeshWriteReport,
};

/// Result of the pure `mask_postproc_Earth` patchtypes_make loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthPatchtypes {
    pub seaorland_ustr: Vec<i32>,
    pub patchtypes_select: Vec<Vec<i32>>,
    pub sum_land_ustr: usize,
    pub sum_sea_ustr: usize,
}

/// Result of the pure `mask_postproc_Lnd` `patchtypes_make` loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandPatchtypes {
    pub seaorland: Vec<Vec<i32>>,
    pub patchtypes_select: Vec<Vec<i32>>,
    pub filled_ignored_land_pixels: usize,
}

/// Working mesh orientation used by `MOD_mask_postproc.F90:mask_postproc_*`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocLayout {
    pub ustr_points: usize,
    pub ustr_bounds: usize,
    pub center_points: Vec<LonLatPoint>,
    pub vertex_points: Vec<LonLatPoint>,
    pub center_neighbors: Vec<Vec<usize>>,
    pub vertex_neighbors: Vec<Vec<usize>>,
    pub center_neighbor_counts: Vec<usize>,
    pub vertex_neighbor_counts: Vec<usize>,
}

/// Final gridfile payload plus the vertex reindex evidence produced by the
/// final `MOD_mask_postproc.F90:mask_postproc_*` compaction sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocFinalizationReport {
    pub mesh: UnstructuredMesh,
    pub final_data: earthmesh_mesh::MaskPostprocFinalData,
    pub vertex_reindex: earthmesh_mesh::VertexReindex,
}

/// Evidence report from the gridfile/contain-backed Rust replacement path for
/// `MOD_mask_postproc.F90:mask_postproc_Earth`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocEarthDomainReport {
    pub patchtypes: EarthPatchtypes,
    pub patchtype: PatchIdWriteReport,
    pub final_gridfile: UnstructuredMeshWriteReport,
    pub earthmesh_info: EarthmeshInfoWriteReport,
}

/// Evidence report from the gridfile/contain-backed Rust replacement path for
/// `MOD_mask_postproc.F90:mask_postproc_Lnd`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocLandDomainReport {
    pub patchtypes: LandPatchtypes,
    pub patchtype: PatchIdWriteReport,
    pub final_gridfile: UnstructuredMeshWriteReport,
}

/// Evidence report from the gridfile/contain-backed Rust replacement path for
/// `MOD_mask_postproc.F90:mask_postproc_Ocn`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocOceanDomainReport {
    pub renewal: MaskPostprocOceanRenewalReport,
    pub finalization: MaskPostprocFinalizationReport,
    pub final_gridfile: UnstructuredMeshWriteReport,
    pub boundary_orders: Option<BoundaryOrders>,
    pub obc: Option<ObcBoundaryWriteReport>,
    pub obcv2: Option<Obcv2BoundaryWriteReport>,
}

/// Result of composing the tri-only ocean postprocess mask renewal routines
/// before final gridfile/OBC writing.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocOceanRenewalReport {
    pub is_in_domain_ustr: Vec<i32>,
    pub renewed: MaskPostprocRenewedData,
    pub boundary: Option<BoundaryConnection>,
    pub isolated: Option<IsolatedOceanRenewal>,
}

/// File-level I/O contract for the domain branches of
/// `MOD_mask_postproc.F90:mask_postproc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskPostprocDomainIoPlan {
    pub file_dir: PathBuf,
    pub mesh_type: String,
    pub mode_grid: String,
    pub source_gridfile: PathBuf,
    pub contain_domain: PathBuf,
    pub result_gridfile: PathBuf,
    pub patchtype_output: Option<PathBuf>,
    pub obc_output: Option<PathBuf>,
    pub obcv2_output: Option<PathBuf>,
}

/// NetCDF inputs loaded for domain `mask_postproc_Earth/Lnd/Ocn` orchestration.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocDomainInputs {
    pub layout: MaskPostprocLayout,
    pub contain: ContainMesh,
    pub is_in_domain_ustr: Vec<i32>,
}

/// Runtime controls needed to reproduce the file-backed
/// `MOD_mask_postproc.F90:mask_postproc_Earth` branch from an I/O plan.
#[derive(Debug, Clone, Copy)]
pub struct MaskPostprocEarthRunOptions<'a> {
    pub mask_sea_ratio: f64,
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub num_mp_step: &'a [usize],
    pub sjx_points: usize,
}

/// Runtime controls needed to reproduce the file-backed
/// `MOD_mask_postproc.F90:mask_postproc_Lnd` branch from an I/O plan.
#[derive(Debug, Clone, Copy)]
pub struct MaskPostprocLandRunOptions<'a> {
    pub seaorland: &'a [Vec<i32>],
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
}

/// Runtime controls needed to reproduce the file-backed
/// `MOD_mask_postproc.F90:mask_postproc_Ocn` branch from an I/O plan.
#[derive(Debug, Clone, Copy)]
pub struct MaskPostprocOceanRunOptions {
    pub mask_sea_ratio: f64,
    pub num_vertex: usize,
}

/// Restart action selected by the top-level `mkgrd.F90` mask-restart branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskRestartAction {
    /// Fortran calls `mask_postproc(mesh_type)` and stops immediately.
    RunMaskPostproc,
    /// Fortran continues into the normal mkgrd path after the read_nl restart short-circuit.
    ContinueMkgrd,
}

/// Non-destructive plan for the remask-specific state mutation around `mask_postproc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskRestartRemaskPlan {
    pub file_dir: PathBuf,
    pub mesh_type: String,
    pub step: i32,
    pub refine: bool,
    pub action: MaskRestartAction,
}
