use super::{GetRefLocContainmentSplit, GetRefMeanStd2DReport, GetRefMeanStd3DReport};

/// In-memory land threshold report assembled in `MOD_GetRef:GetRef_Lnd` column order.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefLandThresholdReport {
    pub ref_colnum: usize,
    pub column_names: Vec<String>,
    pub ref_th_land: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
    pub n_landtypes: Option<Vec<i32>>,
    pub f_mainarea: Option<Vec<f64>>,
    pub onelayer_reports: Vec<GetRefMeanStd2DReport>,
    pub twolayer_reports: Vec<GetRefMeanStd3DReport>,
    pub last_p_num: Option<Vec<i32>>,
}

/// Configuration for `MOD_GetRef:GetRef_Ocn` in-memory threshold assembly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GetRefOceanThresholdConfig {
    pub num_vertex: usize,
    pub maxlc: i32,
    pub refine_sea_ratio: bool,
    pub th_sea_ratio: [f64; 2],
}

/// In-memory ocean threshold report assembled in `MOD_GetRef:GetRef_Ocn` column order.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefOceanThresholdReport {
    pub ref_colnum: usize,
    pub column_names: Vec<String>,
    pub ref_th: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
    pub sea_ratio: Option<Vec<f64>>,
    pub onelayer_reports: Vec<GetRefMeanStd2DReport>,
    pub last_p_num: Option<Vec<i32>>,
}

/// Configuration for `MOD_GetRef:GetRef_Atmos` in-memory threshold assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetRefAtmosThresholdConfig {
    pub num_vertex: usize,
    pub maxlc: i32,
}

/// In-memory atmosphere threshold report assembled in `MOD_GetRef:GetRef_Atmos` column order.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefAtmosThresholdReport {
    pub ref_colnum: usize,
    pub column_names: Vec<String>,
    pub ref_th: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
    pub onelayer_reports: Vec<GetRefMeanStd2DReport>,
    pub last_p_num: Option<Vec<i32>>,
}

/// Top-level `MOD_GetRef:GetRef` threshold matrix after concatenating enabled
/// land, ocean, and atmosphere component reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefThresholdAggregationReport {
    pub ref_colnum: usize,
    pub column_sources: Vec<String>,
    pub column_names: Vec<String>,
    pub ref_th: Vec<Vec<i32>>,
    pub ref_sjx: Vec<i32>,
}

/// Full in-memory LOCmesh threshold wiring report from
/// `MOD_GetRef:GetRef_LOC`.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefLocThresholdReports {
    pub split: GetRefLocContainmentSplit,
    pub land: Option<GetRefLandThresholdReport>,
    pub ocean: Option<GetRefOceanThresholdReport>,
    pub atmos: Option<GetRefAtmosThresholdReport>,
    pub aggregate: GetRefThresholdAggregationReport,
}

/// Full in-memory landmesh/oceanmesh/atmosmesh threshold wiring report from
/// the top-level `MOD_GetRef:GetRef(iter == 0)` branch.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefSingleMeshThresholdReports {
    pub mesh_type: String,
    pub land: Option<GetRefLandThresholdReport>,
    pub ocean: Option<GetRefOceanThresholdReport>,
    pub atmos: Option<GetRefAtmosThresholdReport>,
    pub aggregate: GetRefThresholdAggregationReport,
}
