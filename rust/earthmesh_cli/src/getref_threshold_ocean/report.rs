use std::io;

use crate::*;

/// Assemble `GetRef_Ocn` threshold columns without writing the NetCDF report.
pub fn calculate_getref_ocean_threshold_report_fortran_indexed(
    is_in_refine_sjx: &[i32],
    ocn_id: &[Vec<i32>],
    ocn_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    config: GetRefOceanThresholdConfig,
    onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
) -> io::Result<GetRefOceanThresholdReport> {
    require_getref_lookup_width("Ocn_id", ocn_id, 3)?;
    require_getref_lookup_width("Ocn_ii", ocn_ii, 2)?;
    let sjx_points = is_in_refine_sjx.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IsInRfArea_sjx must include a Fortran placeholder element",
        )
    })?;
    if ocn_id.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Ocn_id length {} must cover sjx_points {sjx_points}",
                ocn_id.len()
            ),
        ));
    }
    if config.num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_vertex {} exceeds sjx_points {sjx_points}",
                config.num_vertex
            ),
        ));
    }

    let ref_colnum = usize::from(config.refine_sea_ratio)
        + onelayer_inputs
            .iter()
            .map(|input| {
                usize::from(input.mean_threshold.is_some())
                    + usize::from(input.std_threshold.is_some())
            })
            .sum::<usize>();
    let mut column_names = Vec::with_capacity(ref_colnum);
    let mut ref_th = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut sea_ratio = config.refine_sea_ratio.then(|| vec![0.0; sjx_points + 1]);
    let mut onelayer_reports = Vec::new();
    let mut last_p_num = None;
    let mut target_col = 0;

    if let Some(values) = sea_ratio.as_mut() {
        target_col += 1;
        column_names.push("sea_ratio".to_string());
        for sjx_index in config.num_vertex + 1..=sjx_points {
            if is_in_refine_sjx[sjx_index] != 1 {
                continue;
            }
            let selected_pixels =
                usize_from_i32_nonnegative(ocn_id[sjx_index][0], "Ocn_id selected count")?;
            let total_pixels = usize_from_i32_positive(ocn_id[sjx_index][2], "Ocn_id total count")?;
            values[sjx_index] = selected_pixels as f64 / total_pixels as f64;
            if values[sjx_index] > config.th_sea_ratio[0]
                && values[sjx_index] < config.th_sea_ratio[1]
            {
                ref_th[sjx_index][target_col] = 1;
            }
        }
    }

    for input in onelayer_inputs {
        if input.mean_threshold.is_none() && input.std_threshold.is_none() {
            continue;
        }
        let report = calculate_getref_mean_std_2d_fortran_indexed(
            is_in_refine_sjx,
            ocn_id,
            ocn_ii,
            landtypes,
            input.values,
            GetRefMeanStd2DConfig {
                num_vertex: config.num_vertex,
                maxlc: config.maxlc,
                mean_threshold: input.mean_threshold,
                std_threshold: input.std_threshold,
            },
        )?;
        let mut report_col = 0;
        if input.mean_threshold.is_some() {
            report_col += 1;
            target_col += 1;
            column_names.push(format!("{}_m", input.name));
            copy_getref_threshold_column(&report.ref_th, report_col, &mut ref_th, target_col)?;
        }
        if input.std_threshold.is_some() {
            report_col += 1;
            target_col += 1;
            column_names.push(format!("{}_s", input.name));
            copy_getref_threshold_column(&report.ref_th, report_col, &mut ref_th, target_col)?;
        }
        last_p_num = Some(report.p_num.clone());
        onelayer_reports.push(report);
    }

    let ref_sjx = aggregate_getref_ref_sjx(&ref_th, config.num_vertex, ref_colnum)?;
    Ok(GetRefOceanThresholdReport {
        ref_colnum,
        column_names,
        ref_th,
        ref_sjx,
        sea_ratio,
        onelayer_reports,
        last_p_num,
    })
}
