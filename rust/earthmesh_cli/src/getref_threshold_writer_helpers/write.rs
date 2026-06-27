use std::io;

use crate::*;

pub(crate) fn skip_fortran_i32_placeholder(values: &[i32]) -> Vec<i32> {
    values.iter().skip(1).copied().collect()
}

pub(crate) fn skip_fortran_f64_placeholder(values: &[f64]) -> Vec<f64> {
    values.iter().skip(1).copied().collect()
}

fn flatten_fortran_layer2_rows(values: &[[f64; 2]]) -> Vec<f64> {
    values
        .iter()
        .skip(1)
        .flat_map(|layers| layers.iter().copied())
        .collect()
}

pub(crate) fn write_f64_layer2_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    values: &[[f64; 2]],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &["sjx_points", "dima"])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_fortran_layer2_rows(values), (.., ..))
        .map_err(netcdf_to_io_error)
}

pub(crate) fn write_getref_onelayer_value_columns(
    file: &mut netcdf::FileMut,
    column_names: &[String],
    col_cursor: &mut usize,
    reports: &[GetRefMeanStd2DReport],
) -> io::Result<()> {
    for layer_report in reports {
        for _ in 0..layer_report.ref_colnum {
            let name = column_names.get(*col_cursor).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "column_names ended before one-layer report columns",
                )
            })?;
            if name.ends_with("_m") {
                write_f64_1d(
                    file,
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
                    file,
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
            *col_cursor += 1;
        }
    }
    Ok(())
}

pub(crate) fn write_getref_ref_th_matrix(
    file: &mut netcdf::FileMut,
    name: &str,
    ref_th: &[Vec<i32>],
    ref_colnum: usize,
) -> io::Result<()> {
    let sjx_points = ref_th.len() - 1;
    let mut values = Vec::with_capacity(sjx_points * ref_colnum);
    for row in ref_th.iter().take(sjx_points + 1).skip(1) {
        values.extend_from_slice(&row[1..=ref_colnum]);
    }
    let mut var = file
        .add_variable::<i32>(name, &["sjx_points", "ref_colnum"])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&values, (.., ..))
        .map_err(netcdf_to_io_error)
}
