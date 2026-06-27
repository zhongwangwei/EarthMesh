use std::fs;
use std::io;
use std::path::Path;

use crate::getref_threshold_writer_helpers::{
    require_getref_column_name_at, skip_fortran_f64_placeholder, skip_fortran_i32_placeholder,
    write_f64_layer2_rows,
};
use crate::*;

use super::validation::validate_getref_land_threshold_report;

pub fn write_getref_land_threshold_netcdf(
    output: impl AsRef<Path>,
    report: &GetRefLandThresholdReport,
) -> io::Result<GetRefLandThresholdWriteReport> {
    validate_getref_land_threshold_report(report)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let sjx_points = report.ref_th_land.len() - 1;
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", sjx_points)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dima", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("ref_colnum", report.ref_colnum)
        .map_err(netcdf_to_io_error)?;

    let mut col_cursor = 0usize;
    if let Some(values) = report.n_landtypes.as_ref() {
        require_getref_column_name(report, col_cursor, "n_landtypes")?;
        write_i32_1d(
            &mut file,
            "n_landtypes",
            "sjx_points",
            &skip_fortran_i32_placeholder(values),
        )?;
        col_cursor += 1;
    }
    if let Some(values) = report.f_mainarea.as_ref() {
        require_getref_column_name(report, col_cursor, "f_mainarea")?;
        write_f64_1d(
            &mut file,
            "f_mainarea",
            "sjx_points",
            &skip_fortran_f64_placeholder(values),
        )?;
        col_cursor += 1;
    }

    for layer_report in &report.onelayer_reports {
        for _ in 0..layer_report.ref_colnum {
            let name = report.column_names.get(col_cursor).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "column_names ended before one-layer report columns",
                )
            })?;
            if name.ends_with("_m") {
                write_f64_1d(
                    &mut file,
                    name,
                    "sjx_points",
                    &skip_fortran_f64_placeholder(&layer_report.mean),
                )?;
            } else if name.ends_with("_s") {
                let stddev = layer_report.stddev.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("one-layer column {name} requires stddev values"),
                    )
                })?;
                write_f64_1d(
                    &mut file,
                    name,
                    "sjx_points",
                    &skip_fortran_f64_placeholder(stddev),
                )?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("one-layer threshold column {name} must end with _m or _s"),
                ));
            }
            col_cursor += 1;
        }
    }

    for layer_report in &report.twolayer_reports {
        for _ in 0..layer_report.ref_colnum {
            let name = report.column_names.get(col_cursor).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "column_names ended before two-layer report columns",
                )
            })?;
            if name.ends_with("_m") {
                write_f64_layer2_rows(&mut file, name, &layer_report.mean)?;
            } else if name.ends_with("_s") {
                let stddev = layer_report.stddev.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("two-layer column {name} requires stddev values"),
                    )
                })?;
                write_f64_layer2_rows(&mut file, name, stddev)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("two-layer threshold column {name} must end with _m or _s"),
                ));
            }
            col_cursor += 1;
        }
    }

    if col_cursor != report.ref_colnum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "wrote {col_cursor} threshold value columns but ref_colnum is {}",
                report.ref_colnum
            ),
        ));
    }

    if let Some(values) = report.last_p_num.as_ref() {
        write_i32_1d(
            &mut file,
            "p_num",
            "sjx_points",
            &skip_fortran_i32_placeholder(values),
        )?;
    }

    let mut ref_th_values = Vec::with_capacity(sjx_points * report.ref_colnum);
    for sjx_index in 1..=sjx_points {
        ref_th_values.extend_from_slice(&report.ref_th_land[sjx_index][1..=report.ref_colnum]);
    }
    let mut ref_th = file
        .add_variable::<i32>("ref_th_Lnd", &["sjx_points", "ref_colnum"])
        .map_err(netcdf_to_io_error)?;
    ref_th
        .put_values(&ref_th_values, (.., ..))
        .map_err(netcdf_to_io_error)?;

    Ok(GetRefLandThresholdWriteReport {
        output: output.to_path_buf(),
        sjx_points,
        dima: 2,
        ref_colnum: report.ref_colnum,
    })
}

fn require_getref_column_name(
    report: &GetRefLandThresholdReport,
    zero_based_index: usize,
    expected: &str,
) -> io::Result<()> {
    require_getref_column_name_at(&report.column_names, zero_based_index, expected)
}
