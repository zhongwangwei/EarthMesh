use std::io;

use crate::*;

/// Assemble `GetRef_Lnd` threshold columns without writing the NetCDF report.
pub fn calculate_getref_land_threshold_report_fortran_indexed(
    is_in_refine_sjx: &[i32],
    lnd_id: &[Vec<i32>],
    lnd_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    basic_config: GetRefLandBasicConfig,
    onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
    twolayer_inputs: &[GetRefTwoLayerThresholdInput<'_>],
) -> io::Result<GetRefLandThresholdReport> {
    let basic_report = calculate_getref_land_basic_fortran_indexed(
        is_in_refine_sjx,
        lnd_id,
        lnd_ii,
        landtypes,
        basic_config,
    )?;
    let sjx_points = is_in_refine_sjx.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IsInRfArea_sjx must include a Fortran placeholder element",
        )
    })?;
    let ref_colnum = basic_report.ref_colnum
        + onelayer_inputs
            .iter()
            .map(|input| {
                usize::from(input.mean_threshold.is_some())
                    + usize::from(input.std_threshold.is_some())
            })
            .sum::<usize>()
        + twolayer_inputs
            .iter()
            .map(|input| {
                usize::from(input.mean_thresholds.is_some())
                    + usize::from(input.std_thresholds.is_some())
            })
            .sum::<usize>();
    let mut column_names = Vec::with_capacity(ref_colnum);
    let mut ref_th_land = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut onelayer_reports = Vec::new();
    let mut twolayer_reports = Vec::new();
    let mut last_p_num = None;

    let mut target_col = 0;
    let mut source_col = 0;
    if basic_config.refine_num_landtypes {
        source_col += 1;
        target_col += 1;
        column_names.push("n_landtypes".to_string());
        copy_getref_threshold_column(
            &basic_report.ref_th_land,
            source_col,
            &mut ref_th_land,
            target_col,
        )?;
    }
    if basic_config.refine_area_mainland {
        source_col += 1;
        target_col += 1;
        column_names.push("f_mainarea".to_string());
        copy_getref_threshold_column(
            &basic_report.ref_th_land,
            source_col,
            &mut ref_th_land,
            target_col,
        )?;
    }

    for input in onelayer_inputs {
        if input.mean_threshold.is_none() && input.std_threshold.is_none() {
            continue;
        }
        let report = calculate_getref_mean_std_2d_fortran_indexed(
            is_in_refine_sjx,
            lnd_id,
            lnd_ii,
            landtypes,
            input.values,
            GetRefMeanStd2DConfig {
                num_vertex: basic_config.num_vertex,
                maxlc: basic_config.maxlc,
                mean_threshold: input.mean_threshold,
                std_threshold: input.std_threshold,
            },
        )?;
        let mut report_col = 0;
        if input.mean_threshold.is_some() {
            report_col += 1;
            target_col += 1;
            column_names.push(format!("{}_m", input.name));
            copy_getref_threshold_column(&report.ref_th, report_col, &mut ref_th_land, target_col)?;
        }
        if input.std_threshold.is_some() {
            report_col += 1;
            target_col += 1;
            column_names.push(format!("{}_s", input.name));
            copy_getref_threshold_column(&report.ref_th, report_col, &mut ref_th_land, target_col)?;
        }
        last_p_num = Some(report.p_num.clone());
        onelayer_reports.push(report);
    }

    for input in twolayer_inputs {
        if input.mean_thresholds.is_none() && input.std_thresholds.is_none() {
            continue;
        }
        let report = calculate_getref_mean_std_3d_fortran_indexed(
            is_in_refine_sjx,
            lnd_id,
            lnd_ii,
            landtypes,
            input.layers,
            GetRefMeanStd3DConfig {
                num_vertex: basic_config.num_vertex,
                maxlc: basic_config.maxlc,
                mean_thresholds: input.mean_thresholds,
                std_thresholds: input.std_thresholds,
            },
        )?;
        let mut report_col = 0;
        if input.mean_thresholds.is_some() {
            report_col += 1;
            target_col += 1;
            column_names.push(format!("{}_m", input.name));
            copy_getref_threshold_column(&report.ref_th, report_col, &mut ref_th_land, target_col)?;
        }
        if input.std_thresholds.is_some() {
            report_col += 1;
            target_col += 1;
            column_names.push(format!("{}_s", input.name));
            copy_getref_threshold_column(&report.ref_th, report_col, &mut ref_th_land, target_col)?;
        }
        last_p_num = Some(report.p_num.clone());
        twolayer_reports.push(report);
    }

    let mut ref_sjx = vec![0; sjx_points + 1];
    for sjx_index in basic_config.num_vertex + 1..=sjx_points {
        if ref_th_land[sjx_index][1..=ref_colnum]
            .iter()
            .any(|flag| *flag != 0)
        {
            ref_sjx[sjx_index] = 1;
        }
    }

    Ok(GetRefLandThresholdReport {
        ref_colnum,
        column_names,
        ref_th_land,
        ref_sjx,
        n_landtypes: basic_report.n_landtypes,
        f_mainarea: basic_report.f_mainarea,
        onelayer_reports,
        twolayer_reports,
        last_p_num,
    })
}
