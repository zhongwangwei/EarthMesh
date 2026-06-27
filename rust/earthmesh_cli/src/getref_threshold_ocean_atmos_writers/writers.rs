use std::fs;
use std::io;
use std::path::Path;

use crate::getref_threshold_writer_helpers::{
    require_getref_column_name_at, skip_fortran_f64_placeholder, skip_fortran_i32_placeholder,
    validate_getref_common_threshold_shape, validate_getref_onelayer_reports,
    validate_getref_written_column_count, validate_optional_fortran_f64_len,
    validate_optional_fortran_i32_len, write_getref_onelayer_value_columns,
    write_getref_ref_th_matrix,
};
use crate::*;

pub fn write_getref_ocean_threshold_netcdf(
    output: impl AsRef<Path>,
    report: &GetRefOceanThresholdReport,
) -> io::Result<GetRefOceanThresholdWriteReport> {
    validate_getref_ocean_threshold_report(report)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let sjx_points = report.ref_th.len() - 1;
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", sjx_points)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("ref_colnum", report.ref_colnum)
        .map_err(netcdf_to_io_error)?;

    let mut col_cursor = 0usize;
    if let Some(values) = report.sea_ratio.as_ref() {
        require_getref_column_name_at(&report.column_names, col_cursor, "sea_ratio")?;
        write_f64_1d(
            &mut file,
            "sea_ratio",
            "sjx_points",
            &skip_fortran_f64_placeholder(values),
        )?;
        col_cursor += 1;
    }
    write_getref_onelayer_value_columns(
        &mut file,
        &report.column_names,
        &mut col_cursor,
        &report.onelayer_reports,
    )?;
    validate_getref_written_column_count(col_cursor, report.ref_colnum)?;
    if let Some(values) = report.last_p_num.as_ref() {
        write_i32_1d(
            &mut file,
            "p_num",
            "sjx_points",
            &skip_fortran_i32_placeholder(values),
        )?;
    }
    write_getref_ref_th_matrix(&mut file, "ref_th_Ocn", &report.ref_th, report.ref_colnum)?;

    Ok(GetRefOceanThresholdWriteReport {
        output: output.to_path_buf(),
        sjx_points,
        ref_colnum: report.ref_colnum,
    })
}

/// Write the `GetRef_Atmos` calculated-threshold NetCDF report using the legacy
/// `threshold_calculate_atmos_NXP####_##.nc4` schema.
pub fn write_getref_atmos_threshold_netcdf(
    output: impl AsRef<Path>,
    report: &GetRefAtmosThresholdReport,
) -> io::Result<GetRefAtmosThresholdWriteReport> {
    validate_getref_atmos_threshold_report(report)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let sjx_points = report.ref_th.len() - 1;
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", sjx_points)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("ref_colnum", report.ref_colnum)
        .map_err(netcdf_to_io_error)?;

    let mut col_cursor = 0usize;
    write_getref_onelayer_value_columns(
        &mut file,
        &report.column_names,
        &mut col_cursor,
        &report.onelayer_reports,
    )?;
    validate_getref_written_column_count(col_cursor, report.ref_colnum)?;
    if let Some(values) = report.last_p_num.as_ref() {
        write_i32_1d(
            &mut file,
            "p_num",
            "sjx_points",
            &skip_fortran_i32_placeholder(values),
        )?;
    }
    write_getref_ref_th_matrix(&mut file, "ref_th_Atmos", &report.ref_th, report.ref_colnum)?;

    Ok(GetRefAtmosThresholdWriteReport {
        output: output.to_path_buf(),
        sjx_points,
        ref_colnum: report.ref_colnum,
    })
}

fn validate_getref_ocean_threshold_report(report: &GetRefOceanThresholdReport) -> io::Result<()> {
    validate_getref_ocean_threshold_report_shape(report, false)
}

pub(crate) fn validate_getref_ocean_threshold_report_for_aggregation(
    report: &GetRefOceanThresholdReport,
) -> io::Result<()> {
    validate_getref_ocean_threshold_report_shape(report, true)
}

fn validate_getref_ocean_threshold_report_shape(
    report: &GetRefOceanThresholdReport,
    allow_empty: bool,
) -> io::Result<()> {
    validate_getref_common_threshold_shape(
        "ref_th_Ocn",
        report.ref_colnum,
        &report.column_names,
        &report.ref_th,
        allow_empty,
    )?;
    let sjx_points = report.ref_th.len() - 1;
    validate_optional_fortran_f64_len("sea_ratio", report.sea_ratio.as_deref(), sjx_points)?;
    validate_optional_fortran_i32_len("last_p_num", report.last_p_num.as_deref(), sjx_points)?;
    validate_getref_onelayer_reports(&report.onelayer_reports, sjx_points)
}

fn validate_getref_atmos_threshold_report(report: &GetRefAtmosThresholdReport) -> io::Result<()> {
    validate_getref_atmos_threshold_report_shape(report, false)
}

pub(crate) fn validate_getref_atmos_threshold_report_for_aggregation(
    report: &GetRefAtmosThresholdReport,
) -> io::Result<()> {
    validate_getref_atmos_threshold_report_shape(report, true)
}

fn validate_getref_atmos_threshold_report_shape(
    report: &GetRefAtmosThresholdReport,
    allow_empty: bool,
) -> io::Result<()> {
    validate_getref_common_threshold_shape(
        "ref_th_Atmos",
        report.ref_colnum,
        &report.column_names,
        &report.ref_th,
        allow_empty,
    )?;
    let sjx_points = report.ref_th.len() - 1;
    validate_optional_fortran_i32_len("last_p_num", report.last_p_num.as_deref(), sjx_points)?;
    validate_getref_onelayer_reports(&report.onelayer_reports, sjx_points)
}
