/// Configuration for the basic land-only `MOD_GetRef:GetRef_Lnd` criteria.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GetRefLandBasicConfig {
    pub num_vertex: usize,
    pub maxlc: i32,
    pub refine_num_landtypes: bool,
    pub th_num_landtypes: i32,
    pub refine_area_mainland: bool,
    pub th_area_mainland: f64,
}

/// Basic land-threshold columns from `MOD_GetRef:GetRef_Lnd`.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefLandBasicReport {
    pub ref_colnum: usize,
    pub ref_th_land: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
    pub n_landtypes: Option<Vec<i32>>,
    pub f_mainarea: Option<Vec<f64>>,
}

/// Configuration for `MOD_GetRef:mean_std_cal2d` style threshold columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GetRefMeanStd2DConfig {
    pub num_vertex: usize,
    pub maxlc: i32,
    pub mean_threshold: Option<f64>,
    pub std_threshold: Option<f64>,
}

/// One-layer mean/std threshold report from `MOD_GetRef:mean_std_cal2d`.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefMeanStd2DReport {
    pub ref_colnum: usize,
    pub ref_th: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
    pub p_num: Vec<i32>,
    pub mean: Vec<f64>,
    pub stddev: Option<Vec<f64>>,
}

/// Configuration for `MOD_GetRef:mean_std_cal3d` two-layer threshold columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GetRefMeanStd3DConfig {
    pub num_vertex: usize,
    pub maxlc: i32,
    pub mean_thresholds: Option<[f64; 2]>,
    pub std_thresholds: Option<[f64; 2]>,
}

/// Two-layer mean/std threshold report from `MOD_GetRef:mean_std_cal3d`.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefMeanStd3DReport {
    pub ref_colnum: usize,
    pub ref_th: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
    pub p_num: Vec<i32>,
    pub mean: Vec<[f64; 2]>,
    pub stddev: Option<Vec<[f64; 2]>>,
}

/// One selected land one-layer threshold input for `GetRef_Lnd` report assembly.
#[derive(Debug, Clone, Copy)]
pub struct GetRefOneLayerThresholdInput<'a> {
    pub name: &'a str,
    pub values: &'a [Vec<f64>],
    pub mean_threshold: Option<f64>,
    pub std_threshold: Option<f64>,
}

/// One selected land two-layer threshold input for `GetRef_Lnd` report assembly.
#[derive(Debug, Clone, Copy)]
pub struct GetRefTwoLayerThresholdInput<'a> {
    pub name: &'a str,
    pub layers: &'a [Vec<Vec<f64>>],
    pub mean_thresholds: Option<[f64; 2]>,
    pub std_thresholds: Option<[f64; 2]>,
}

/// Fortran-indexed containment lookup prepared for GetRef land/ocean/atmos kernels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefContainmentLookup {
    pub mp_id: Vec<Vec<i32>>,
    pub mp_ii: Vec<Vec<i32>>,
}

/// Split `LOCmesh` containment into land, ocean, and atmosphere GetRef lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefLocContainmentSplit {
    pub sjx_points: usize,
    pub num_vertex: usize,
    pub land: GetRefContainmentLookup,
    pub ocean: GetRefContainmentLookup,
    pub atmos: GetRefContainmentLookup,
}
