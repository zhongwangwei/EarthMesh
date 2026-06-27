use std::io;

use crate::*;

/// Assemble `GetRef_Atmos` threshold columns without writing the NetCDF report.
pub fn calculate_getref_atmos_threshold_report_fortran_indexed(
    is_in_refine_sjx: &[i32],
    atmos_id: &[Vec<i32>],
    atmos_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    config: GetRefAtmosThresholdConfig,
    onelayer_inputs: &[GetRefOneLayerThresholdInput<'_>],
) -> io::Result<GetRefAtmosThresholdReport> {
    require_getref_lookup_width("Atmos_id", atmos_id, 2)?;
    require_getref_lookup_width("Atmos_ii", atmos_ii, 2)?;
    let sjx_points = is_in_refine_sjx.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IsInRfArea_sjx must include a Fortran placeholder element",
        )
    })?;
    if atmos_id.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Atmos_id length {} must cover sjx_points {sjx_points}",
                atmos_id.len()
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

    let ref_colnum = onelayer_inputs
        .iter()
        .map(|input| {
            usize::from(input.mean_threshold.is_some()) + usize::from(input.std_threshold.is_some())
        })
        .sum::<usize>();
    let mut column_names = Vec::with_capacity(ref_colnum);
    let mut ref_th = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut onelayer_reports = Vec::new();
    let mut last_p_num = None;
    let mut target_col = 0;

    for input in onelayer_inputs {
        if input.mean_threshold.is_none() && input.std_threshold.is_none() {
            continue;
        }
        let report = calculate_getref_mean_std_2d_fortran_indexed(
            is_in_refine_sjx,
            atmos_id,
            atmos_ii,
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
    Ok(GetRefAtmosThresholdReport {
        ref_colnum,
        column_names,
        ref_th,
        ref_sjx,
        onelayer_reports,
        last_p_num,
    })
}
