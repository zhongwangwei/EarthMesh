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
    netcdf_to_io_error, require_len, AreaJudgeThreshold2D, AreaJudgeThreshold2Layer,
    ThresholdReadAtmosConfig, ThresholdReadAtmosReport, ThresholdReadLndConfig,
    ThresholdReadLndReport, ThresholdReadOcnConfig, ThresholdReadOcnReport,
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
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let file =
        netcdf::open(area_judge_threshold_path(threshold_dir, name)).map_err(netcdf_to_io_error)?;
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let start_lon = bounds.minlon_source.checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "minlon_source must be one-based",
        )
    })?;
    let start_lat = bounds.maxlat_source.checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "maxlat_source must be one-based",
        )
    })?;
    let values = variable
        .get_values::<f64, _>((
            start_lon..start_lon + nlons_select,
            start_lat..start_lat + nlats_select,
        ))
        .map_err(netcdf_to_io_error)?;
    let expected = nlons_select * nlats_select;
    require_len(name, values.len(), expected)?;

    let mut selected = vec![vec![0.0; nlats_select + 1]; nlons_select + 1];
    for lon_offset in 0..nlons_select {
        for lat_offset in 0..nlats_select {
            selected[lon_offset + 1][lat_offset + 1] =
                values[lon_offset * nlats_select + lat_offset];
        }
    }
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
