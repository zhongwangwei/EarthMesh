use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::{
    area_judge_refine_flag_pair_enabled, data_read::data_read_onelayer_values_fortran_indexed,
    paths::area_judge_threshold_path, AREA_JUDGE_ATMOS_ONELAYER_NAMES,
    AREA_JUDGE_LAND_ONELAYER_NAMES, AREA_JUDGE_LAND_TWOLAYER_NAMES,
    AREA_JUDGE_OCEAN_ONELAYER_NAMES,
};
use crate::{
    AreaJudgeThreshold2D, AreaJudgeThreshold2Layer, ThresholdReadAtmosConfig,
    ThresholdReadAtmosReport, ThresholdReadLndConfig, ThresholdReadLndReport,
    ThresholdReadOcnConfig, ThresholdReadOcnReport,
};

/// Read land thresholds like `MOD_data_preprocess.F90:Threshold_Read_Lnd`.
pub fn threshold_read_lnd_fortran_indexed(
    config: ThresholdReadLndConfig<'_>,
) -> io::Result<ThresholdReadLndReport> {
    Ok(ThresholdReadLndReport {
        onelayer: read_area_judge_threshold_2d_group_fortran_indexed(
            config.threshold_dir,
            &AREA_JUDGE_LAND_ONELAYER_NAMES,
            config.refine_onelayer_lnd,
            config.bounds,
        )?,
        twolayer: read_area_judge_threshold_2layer_group_fortran_indexed(
            config.threshold_dir,
            &AREA_JUDGE_LAND_TWOLAYER_NAMES,
            config.refine_twolayer_lnd,
            config.bounds,
        )?,
    })
}

/// Read ocean thresholds like `MOD_data_preprocess.F90:Threshold_Read_Ocn`.
pub fn threshold_read_ocn_fortran_indexed(
    config: ThresholdReadOcnConfig<'_>,
) -> io::Result<ThresholdReadOcnReport> {
    Ok(ThresholdReadOcnReport {
        onelayer: read_area_judge_threshold_2d_group_fortran_indexed(
            config.threshold_dir,
            &AREA_JUDGE_OCEAN_ONELAYER_NAMES,
            config.refine_onelayer_ocn,
            config.bounds,
        )?,
    })
}

/// Read atmosphere thresholds like `MOD_data_preprocess.F90:Threshold_Read_Atmos`.
pub fn threshold_read_atmos_fortran_indexed(
    config: ThresholdReadAtmosConfig<'_>,
) -> io::Result<ThresholdReadAtmosReport> {
    Ok(ThresholdReadAtmosReport {
        onelayer: read_area_judge_threshold_2d_group_fortran_indexed(
            config.threshold_dir,
            &AREA_JUDGE_ATMOS_ONELAYER_NAMES,
            config.refine_onelayer_atmos,
            config.bounds,
        )?,
    })
}

fn read_area_judge_threshold_2d_window_fortran_indexed(
    threshold_dir: &Path,
    name: &str,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeThreshold2D> {
    let selected = data_read_onelayer_values_fortran_indexed(
        area_judge_threshold_path(threshold_dir, name),
        name,
        bounds,
    )?;
    Ok(AreaJudgeThreshold2D {
        name: name.to_string(),
        values: selected,
    })
}

fn read_area_judge_threshold_2layer_window_fortran_indexed(
    threshold_dir: &Path,
    name: &str,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeThreshold2Layer> {
    let first = read_area_judge_threshold_2d_variable_window_fortran_indexed(
        threshold_dir,
        name,
        &format!("{name}_l1"),
        bounds,
    )?;
    let second = read_area_judge_threshold_2d_variable_window_fortran_indexed(
        threshold_dir,
        name,
        &format!("{name}_l2"),
        bounds,
    )?;
    Ok(AreaJudgeThreshold2Layer {
        name: name.to_string(),
        layers: vec![first, second],
    })
}

fn read_area_judge_threshold_2d_variable_window_fortran_indexed(
    threshold_dir: &Path,
    file_stem: &str,
    var_name: &str,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Vec<f64>>> {
    data_read_onelayer_values_fortran_indexed(
        area_judge_threshold_path(threshold_dir, file_stem),
        var_name,
        bounds,
    )
}

pub(super) fn read_area_judge_threshold_2d_group_fortran_indexed(
    threshold_dir: &Path,
    names: &[&str],
    flags: &[bool],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Option<AreaJudgeThreshold2D>>> {
    names
        .iter()
        .enumerate()
        .map(|(idx, name)| {
            if area_judge_refine_flag_pair_enabled(flags, idx) {
                read_area_judge_threshold_2d_window_fortran_indexed(threshold_dir, name, bounds)
                    .map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

pub(super) fn read_area_judge_threshold_2layer_group_fortran_indexed(
    threshold_dir: &Path,
    names: &[&str],
    flags: &[bool],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Option<AreaJudgeThreshold2Layer>>> {
    names
        .iter()
        .enumerate()
        .map(|(idx, name)| {
            if area_judge_refine_flag_pair_enabled(flags, idx) {
                read_area_judge_threshold_2layer_window_fortran_indexed(threshold_dir, name, bounds)
                    .map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}
