use std::io;

use crate::*;

pub(crate) fn require_getref_column_name_at(
    column_names: &[String],
    zero_based_index: usize,
    expected: &str,
) -> io::Result<()> {
    match column_names.get(zero_based_index) {
        Some(name) if name == expected => Ok(()),
        Some(name) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "expected column {expected} at position {}, got {name}",
                zero_based_index + 1
            ),
        )),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "missing column {expected} at position {}",
                zero_based_index + 1
            ),
        )),
    }
}

pub(crate) fn validate_optional_fortran_i32_len(
    name: &str,
    values: Option<&[i32]>,
    sjx_points: usize,
) -> io::Result<()> {
    if let Some(values) = values {
        if values.len() != sjx_points + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{name} length {} must equal sjx_points {sjx_points} plus placeholder",
                    values.len()
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_optional_fortran_f64_len(
    name: &str,
    values: Option<&[f64]>,
    sjx_points: usize,
) -> io::Result<()> {
    if let Some(values) = values {
        validate_fortran_f64_len(name, values, sjx_points)?;
    }
    Ok(())
}

pub(crate) fn validate_fortran_f64_len(
    name: &str,
    values: &[f64],
    sjx_points: usize,
) -> io::Result<()> {
    if values.len() != sjx_points + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must equal sjx_points {sjx_points} plus placeholder",
                values.len()
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_fortran_layer2_len(
    name: &str,
    values: &[[f64; 2]],
    sjx_points: usize,
) -> io::Result<()> {
    if values.len() != sjx_points + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must equal sjx_points {sjx_points} plus placeholder",
                values.len()
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_getref_common_threshold_shape(
    matrix_name: &str,
    ref_colnum: usize,
    column_names: &[String],
    ref_th: &[Vec<i32>],
    allow_empty: bool,
) -> io::Result<()> {
    if ref_colnum == 0 && !allow_empty {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{matrix_name} writer requires at least one threshold column"),
        ));
    }
    if column_names.len() != ref_colnum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "column_names length {} must equal ref_colnum {ref_colnum}",
                column_names.len()
            ),
        ));
    }
    let _ = ref_th.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{matrix_name} must include a Fortran placeholder row"),
        )
    })?;
    for (row_index, row) in ref_th.iter().enumerate() {
        if row.len() <= ref_colnum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{matrix_name} row {row_index} width {} must cover ref_colnum {ref_colnum} plus placeholder column",
                    row.len()
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_getref_onelayer_reports(
    reports: &[GetRefMeanStd2DReport],
    sjx_points: usize,
) -> io::Result<()> {
    for (index, layer_report) in reports.iter().enumerate() {
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
    Ok(())
}

pub(crate) fn validate_getref_written_column_count(
    written: usize,
    ref_colnum: usize,
) -> io::Result<()> {
    if written != ref_colnum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("wrote {written} threshold value columns but ref_colnum is {ref_colnum}"),
        ));
    }
    Ok(())
}
