use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

/// Threshold-data reader configuration for the calculated-refine branch of `Area_judge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgeThresholdReadConfig<'a> {
    pub threshold_dir: &'a Path,
    pub mesh_type: &'a str,
    pub refine_onelayer_lnd: &'a [bool],
    pub refine_twolayer_lnd: &'a [bool],
    pub refine_onelayer_ocn: &'a [bool],
    pub refine_onelayer_atmos: &'a [bool],
}

/// One selected 2-D threshold dataset, stored with local Canonical one-based indices.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaJudgeThreshold2D {
    pub name: String,
    pub values: Vec<Vec<f64>>,
}

/// One selected two-layer threshold dataset, stored as layer x local Canonical one-based grid.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaJudgeThreshold2Layer {
    pub name: String,
    pub layers: Vec<Vec<Vec<f64>>>,
}

/// Threshold inputs sliced to the calculated-refine source bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaJudgeThresholdInputsReport {
    pub bounds: AreaJudgeSourceBounds,
    pub nlons_select: usize,
    pub nlats_select: usize,
    pub landtypes: Vec<Vec<i32>>,
    pub land_onelayer: Vec<Option<AreaJudgeThreshold2D>>,
    pub land_twolayer: Vec<Option<AreaJudgeThreshold2Layer>>,
    pub ocean_onelayer: Vec<Option<AreaJudgeThreshold2D>>,
    pub atmos_onelayer: Vec<Option<AreaJudgeThreshold2D>>,
}

/// Configuration for `MOD_data_preprocess.F90:Threshold_Read_Lnd`.
#[derive(Debug, Clone, Copy)]
pub struct ThresholdReadLndConfig<'a> {
    pub threshold_dir: &'a Path,
    pub refine_onelayer_lnd: &'a [bool],
    pub refine_twolayer_lnd: &'a [bool],
    pub bounds: AreaJudgeSourceBounds,
}

/// Data loaded by `MOD_data_preprocess.F90:Threshold_Read_Lnd`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdReadLndReport {
    pub onelayer: Vec<Option<AreaJudgeThreshold2D>>,
    pub twolayer: Vec<Option<AreaJudgeThreshold2Layer>>,
}

/// Configuration for `MOD_data_preprocess.F90:Threshold_Read_Ocn`.
#[derive(Debug, Clone, Copy)]
pub struct ThresholdReadOcnConfig<'a> {
    pub threshold_dir: &'a Path,
    pub refine_onelayer_ocn: &'a [bool],
    pub bounds: AreaJudgeSourceBounds,
}

/// Data loaded by `MOD_data_preprocess.F90:Threshold_Read_Ocn`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdReadOcnReport {
    pub onelayer: Vec<Option<AreaJudgeThreshold2D>>,
}

/// Configuration for `MOD_data_preprocess.F90:Threshold_Read_Atmos`.
#[derive(Debug, Clone, Copy)]
pub struct ThresholdReadAtmosConfig<'a> {
    pub threshold_dir: &'a Path,
    pub refine_onelayer_atmos: &'a [bool],
    pub bounds: AreaJudgeSourceBounds,
}

/// Data loaded by `MOD_data_preprocess.F90:Threshold_Read_Atmos`.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdReadAtmosReport {
    pub onelayer: Vec<Option<AreaJudgeThreshold2D>>,
}
