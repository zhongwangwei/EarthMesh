use crate::{GetRefOneLayerThresholdInput, GetRefTwoLayerThresholdInput};

pub(super) fn has_getref_onelayer_thresholds(inputs: &[GetRefOneLayerThresholdInput<'_>]) -> bool {
    inputs
        .iter()
        .any(|input| input.mean_threshold.is_some() || input.std_threshold.is_some())
}

pub(super) fn has_getref_twolayer_thresholds(inputs: &[GetRefTwoLayerThresholdInput<'_>]) -> bool {
    inputs
        .iter()
        .any(|input| input.mean_thresholds.is_some() || input.std_thresholds.is_some())
}
