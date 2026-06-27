use std::io;

use crate::getref_threshold_writer_helpers::{
    validate_fortran_f64_len, validate_fortran_layer2_len, validate_optional_fortran_f64_len,
    validate_optional_fortran_i32_len,
};
use crate::GetRefLandThresholdReport;

pub(super) fn validate_getref_land_threshold_report(
    report: &GetRefLandThresholdReport,
) -> io::Result<()> {
    validate_getref_land_threshold_report_shape(report, false)
}

pub(crate) fn validate_getref_land_threshold_report_for_aggregation(
    report: &GetRefLandThresholdReport,
) -> io::Result<()> {
    validate_getref_land_threshold_report_shape(report, true)
}

fn validate_getref_land_threshold_report_shape(
    report: &GetRefLandThresholdReport,
    allow_empty: bool,
) -> io::Result<()> {
    if report.ref_colnum == 0 && !allow_empty {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "GetRef land threshold writer requires at least one threshold column",
        ));
    }
    if report.column_names.len() != report.ref_colnum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "column_names length {} must equal ref_colnum {}",
                report.column_names.len(),
                report.ref_colnum
            ),
        ));
    }
    let sjx_points = report.ref_th_land.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "ref_th_land must include a Fortran placeholder row",
        )
    })?;
    for (row_index, row) in report.ref_th_land.iter().enumerate() {
        if row.len() <= report.ref_colnum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "ref_th_land row {row_index} width {} must cover ref_colnum {} plus placeholder column",
                    row.len(), report.ref_colnum
                ),
            ));
        }
    }
    validate_optional_fortran_i32_len("n_landtypes", report.n_landtypes.as_deref(), sjx_points)?;
    validate_optional_fortran_f64_len("f_mainarea", report.f_mainarea.as_deref(), sjx_points)?;
    validate_optional_fortran_i32_len("last_p_num", report.last_p_num.as_deref(), sjx_points)?;
    for (index, layer_report) in report.onelayer_reports.iter().enumerate() {
        validate_fortran_f64_len(
            &format!("onelayer_reports[{index}].mean"),
            &layer_report.mean,
            sjx_points,
        )?;
        validate_optional_fortran_f64_len(
            &format!("onelayer_reports[{index}].stddev"),
            layer_report.stddev.as_deref(),
            sjx_points,
        )?;
    }
    for (index, layer_report) in report.twolayer_reports.iter().enumerate() {
        validate_fortran_layer2_len(
            &format!("twolayer_reports[{index}].mean"),
            &layer_report.mean,
            sjx_points,
        )?;
        if let Some(stddev) = layer_report.stddev.as_ref() {
            validate_fortran_layer2_len(
                &format!("twolayer_reports[{index}].stddev"),
                stddev,
                sjx_points,
            )?;
        }
    }
    Ok(())
}
