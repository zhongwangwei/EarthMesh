use std::path::PathBuf;

/// Evidence report from writing `MOD_GetRef.F90:GetRef_Lnd` threshold output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefLandThresholdWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
    pub dima: usize,
    pub ref_colnum: usize,
}

/// Evidence report from writing `MOD_GetRef.F90:GetRef_Ocn` threshold output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefOceanThresholdWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
    pub ref_colnum: usize,
}

/// Evidence report from writing `MOD_GetRef.F90:GetRef_Atmos` threshold output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefAtmosThresholdWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
    pub ref_colnum: usize,
}

/// Evidence report from writing `MOD_GetRef.F90:GetRef(iter /= 0)` specified
/// refinement targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefSpecifiedThresholdWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
}

/// File outputs written by a top-level GetRef calculated-threshold run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRefThresholdFileWrites {
    pub land: Option<GetRefLandThresholdWriteReport>,
    pub ocean: Option<GetRefOceanThresholdWriteReport>,
    pub atmos: Option<GetRefAtmosThresholdWriteReport>,
}
