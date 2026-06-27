use std::io;

use crate::*;

/// Convert cropped Area_judge 2-D threshold files into GetRef one-layer inputs.
pub fn build_getref_onelayer_threshold_inputs<'a>(
    thresholds: &'a [Option<AreaJudgeThreshold2D>],
    refine_flags: &[bool],
    threshold_values: &[f64],
) -> io::Result<Vec<GetRefOneLayerThresholdInput<'a>>> {
    let required = thresholds.len() * 2;
    require_len("refine_onelayer flags", refine_flags.len(), required)?;
    require_len(
        "one-layer threshold values",
        threshold_values.len(),
        required,
    )?;
    let mut inputs = Vec::new();
    for (index, threshold) in thresholds.iter().enumerate() {
        let mean_enabled = refine_flags[2 * index];
        let std_enabled = refine_flags[2 * index + 1];
        if !mean_enabled && !std_enabled {
            continue;
        }
        let threshold = threshold.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("missing one-layer threshold data at pair index {index}"),
            )
        })?;
        inputs.push(GetRefOneLayerThresholdInput {
            name: threshold.name.as_str(),
            values: &threshold.values,
            mean_threshold: mean_enabled.then_some(threshold_values[2 * index]),
            std_threshold: std_enabled.then_some(threshold_values[2 * index + 1]),
        });
    }
    Ok(inputs)
}

/// Convert cropped Area_judge two-layer threshold files into GetRef two-layer inputs.
pub fn build_getref_twolayer_threshold_inputs<'a>(
    thresholds: &'a [Option<AreaJudgeThreshold2Layer>],
    refine_flags: &[bool],
    threshold_values: &[[f64; 2]],
) -> io::Result<Vec<GetRefTwoLayerThresholdInput<'a>>> {
    let required = thresholds.len() * 2;
    require_len("refine_twolayer flags", refine_flags.len(), required)?;
    require_len(
        "two-layer threshold values",
        threshold_values.len(),
        required,
    )?;
    let mut inputs = Vec::new();
    for (index, threshold) in thresholds.iter().enumerate() {
        let mean_enabled = refine_flags[2 * index];
        let std_enabled = refine_flags[2 * index + 1];
        if !mean_enabled && !std_enabled {
            continue;
        }
        let threshold = threshold.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("missing two-layer threshold data at pair index {index}"),
            )
        })?;
        inputs.push(GetRefTwoLayerThresholdInput {
            name: threshold.name.as_str(),
            layers: &threshold.layers,
            mean_thresholds: mean_enabled.then_some(threshold_values[2 * index]),
            std_thresholds: std_enabled.then_some(threshold_values[2 * index + 1]),
        });
    }
    Ok(inputs)
}
