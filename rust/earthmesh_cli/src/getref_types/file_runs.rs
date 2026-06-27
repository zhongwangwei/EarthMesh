use std::path::{Path, PathBuf};

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::{AreaJudgeThresholdInputsReport, ContainMesh, GetRefThresholdFileWrites};

use super::{
    GetRefAtmosThresholdConfig, GetRefLandBasicConfig, GetRefLocThresholdReports,
    GetRefOceanThresholdConfig, GetRefOneLayerThresholdInput, GetRefSingleMeshThresholdReports,
    GetRefTwoLayerThresholdInput,
};

/// Runtime inputs for a non-LOC top-level GetRef file run.
#[derive(Debug, Clone, Copy)]
pub struct GetRefSingleMeshFileRunConfig<'a> {
    pub mesh_type: &'a str,
    pub contain_file: &'a Path,
    pub land_threshold_output: Option<&'a Path>,
    pub ocean_threshold_output: Option<&'a Path>,
    pub atmos_threshold_output: Option<&'a Path>,
    pub is_in_refine_sjx: &'a [i32],
    pub landtypes: &'a [Vec<i32>],
    pub land_basic_config: GetRefLandBasicConfig,
    pub land_onelayer_inputs: &'a [GetRefOneLayerThresholdInput<'a>],
    pub land_twolayer_inputs: &'a [GetRefTwoLayerThresholdInput<'a>],
    pub ocean_config: GetRefOceanThresholdConfig,
    pub ocean_onelayer_inputs: &'a [GetRefOneLayerThresholdInput<'a>],
    pub atmos_config: GetRefAtmosThresholdConfig,
    pub atmos_onelayer_inputs: &'a [GetRefOneLayerThresholdInput<'a>],
}

/// Evidence from a non-LOC top-level GetRef file run.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefSingleMeshFileRunReport {
    pub contain: ContainMesh,
    pub threshold: GetRefSingleMeshThresholdReports,
    pub writes: GetRefThresholdFileWrites,
}

/// Runtime inputs for a LOCmesh top-level GetRef file run.
#[derive(Debug, Clone, Copy)]
pub struct GetRefLocMeshFileRunConfig<'a> {
    pub contain_file: &'a Path,
    pub land_threshold_output: Option<&'a Path>,
    pub ocean_threshold_output: Option<&'a Path>,
    pub atmos_threshold_output: Option<&'a Path>,
    pub is_in_refine_sjx: &'a [i32],
    pub landtypes: &'a [Vec<i32>],
    pub land_basic_config: GetRefLandBasicConfig,
    pub land_onelayer_inputs: &'a [GetRefOneLayerThresholdInput<'a>],
    pub land_twolayer_inputs: &'a [GetRefTwoLayerThresholdInput<'a>],
    pub ocean_config: GetRefOceanThresholdConfig,
    pub ocean_onelayer_inputs: &'a [GetRefOneLayerThresholdInput<'a>],
    pub atmos_config: GetRefAtmosThresholdConfig,
    pub atmos_onelayer_inputs: &'a [GetRefOneLayerThresholdInput<'a>],
}

/// Evidence from a LOCmesh top-level GetRef file run.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefLocMeshFileRunReport {
    pub contain: ContainMesh,
    pub threshold: GetRefLocThresholdReports,
    pub writes: GetRefThresholdFileWrites,
}

/// Runtime inputs for the integrated calculated-threshold GetRef path:
/// Area_judge threshold files -> GetRef bridge inputs -> legacy threshold outputs.
#[derive(Debug, Clone, Copy)]
pub struct GetRefIntegratedFileRunConfig<'a> {
    pub mesh_type: &'a str,
    pub threshold_dir: &'a Path,
    pub contain_file: &'a Path,
    pub land_threshold_output: Option<&'a Path>,
    pub ocean_threshold_output: Option<&'a Path>,
    pub atmos_threshold_output: Option<&'a Path>,
    pub landtypes_global: &'a [Vec<i32>],
    pub threshold_bounds: AreaJudgeSourceBounds,
    pub is_in_refine_sjx: &'a [i32],
    pub refine_onelayer_lnd: &'a [bool],
    pub th_onelayer_lnd: &'a [f64],
    pub refine_twolayer_lnd: &'a [bool],
    pub th_twolayer_lnd: &'a [[f64; 2]],
    pub refine_onelayer_ocn: &'a [bool],
    pub th_onelayer_ocn: &'a [f64],
    pub refine_onelayer_atmos: &'a [bool],
    pub th_onelayer_atmos: &'a [f64],
    pub land_basic_config: GetRefLandBasicConfig,
    pub ocean_config: GetRefOceanThresholdConfig,
    pub atmos_config: GetRefAtmosThresholdConfig,
}

/// Evidence from the integrated calculated-threshold GetRef file run.
#[derive(Debug, Clone, PartialEq)]
pub struct GetRefIntegratedFileRunReport {
    pub threshold_inputs: AreaJudgeThresholdInputsReport,
    pub single_mesh: Option<GetRefSingleMeshFileRunReport>,
    pub loc_mesh: Option<GetRefLocMeshFileRunReport>,
}

impl GetRefIntegratedFileRunReport {
    pub fn written_threshold_outputs(&self) -> Vec<PathBuf> {
        let writes = self
            .single_mesh
            .as_ref()
            .map(|report| &report.writes)
            .or_else(|| self.loc_mesh.as_ref().map(|report| &report.writes));
        let Some(writes) = writes else {
            return Vec::new();
        };

        let mut outputs = Vec::new();
        if let Some(report) = writes.land.as_ref() {
            outputs.push(report.output.clone());
        }
        if let Some(report) = writes.ocean.as_ref() {
            outputs.push(report.output.clone());
        }
        if let Some(report) = writes.atmos.as_ref() {
            outputs.push(report.output.clone());
        }
        outputs
    }
}
