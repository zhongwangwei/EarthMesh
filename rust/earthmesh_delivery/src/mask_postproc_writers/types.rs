use std::path::PathBuf;

/// Rust data shape written by `MOD_mask_postproc.F90:PatchID_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchIdMesh {
    pub elmindex: Vec<Vec<i32>>,
    pub lon_w: Vec<f64>,
    pub lon_e: Vec<f64>,
    pub lat_n: Vec<f64>,
    pub lat_s: Vec<f64>,
    pub longitude: Vec<f64>,
    pub latitude: Vec<f64>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:LOCmesh_info_save`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthmeshInfo {
    pub num_step_f: Vec<i32>,
    pub refine_degree_f: Vec<i32>,
    pub seaorland_ustr_f: Vec<i32>,
}

/// Evidence report from writing `MOD_file_preprocess.F90:LOCmesh_info_save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthmeshInfoWriteReport {
    pub output: PathBuf,
    pub num_step: usize,
    pub num_ustr: usize,
}

/// Evidence report from writing a patchtype file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchIdWriteReport {
    pub output: PathBuf,
    pub nlon: usize,
    pub nlat: usize,
}
