use std::path::{Path, PathBuf};

use crate::{
    GetContainMeshKind, GetContainRefineFileRunReport, MaskPostprocEarthDomainReport,
    MaskPostprocEarthRunOptions, MaskPostprocLandDomainReport, MaskPostprocLandRunOptions,
    MaskPostprocOceanDomainReport, MaskPostprocOceanRunOptions, MpasFullMeshPipelineReport,
    MpasSimpleMeshWriteReport,
};

/// Runtime options for the final `mask_postproc(mesh_type)` call after
/// `mkgrd.F90` has performed the final `Get_Contain(0)` domain handoff.
pub enum MkgrdFinalDomainPostprocOptions<'a> {
    Earth(MaskPostprocEarthRunOptions<'a>),
    EarthFromFinalGrid(MkgrdFinalDomainEarthAutoPostprocOptions<'a>),
    Land(MaskPostprocLandRunOptions<'a>),
    Ocean(MaskPostprocOceanRunOptions),
    Atmos { output_format: &'a str },
}

/// Runtime controls for restart/refine earth final postprocess when the final
/// unstructured grid count is only known after the refine handoff has copied
/// `gridfile_NXP####_<mode>.nc4` into `result/`.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdFinalDomainEarthAutoPostprocOptions<'a> {
    pub mask_sea_ratio: f64,
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
}

/// Runtime inputs for the final `MOD_GetContain.F90:Get_Contain(0)` call after
/// the refine loop has copied the selected final gridfile into `result/`.
#[derive(Debug, Clone, Copy)]
pub struct MkgrdFinalDomainContainOptions<'a> {
    pub area_grid_file: &'a Path,
    pub mesh_kind: GetContainMeshKind,
    pub seaorland: &'a [Vec<i32>],
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub num_vertex: usize,
}

/// Evidence from the final `mask_postproc(mesh_type)` call after a refine loop.
#[derive(Debug, Clone, PartialEq)]
pub enum MkgrdFinalDomainPostprocReport {
    Earth(MaskPostprocEarthDomainReport),
    Land(MaskPostprocLandDomainReport),
    Ocean(MaskPostprocOceanDomainReport),
    Atmos(MpasSimpleMeshWriteReport),
    AtmosFull(MpasFullMeshPipelineReport),
}

/// Evidence from executing the final `Get_Contain(0)` gridfile handoff and,
/// optionally, the already-migrated domain `mask_postproc` branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdRefineLoopFinalDomainHandoffReport {
    pub copied_result_gridfile: PathBuf,
    pub copied_bytes: u64,
    pub contain_domain: PathBuf,
    pub generated_contain: Option<GetContainRefineFileRunReport>,
    pub postproc: Option<MkgrdFinalDomainPostprocReport>,
}
