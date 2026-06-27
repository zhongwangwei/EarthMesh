use std::io;

use crate::*;

/// Calculate one-layer mean/std thresholds like `MOD_GetRef.F90:mean_std_cal2d`.
pub fn calculate_getref_mean_std_2d_fortran_indexed(
    is_in_refine_sjx: &[i32],
    mp_id: &[Vec<i32>],
    mp_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    var2d: &[Vec<f64>],
    config: GetRefMeanStd2DConfig,
) -> io::Result<GetRefMeanStd2DReport> {
    require_getref_lookup_width("mp_id", mp_id, 2)?;
    require_getref_lookup_width("mp_ii", mp_ii, 2)?;
    if landtypes.is_empty() || var2d.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "landtypes and var2d must include Fortran placeholder rows",
        ));
    }
    let _ = matrix_width("landtypes", landtypes)?;
    let _ = f64_matrix_width("var2d", var2d)?;
    let sjx_points = is_in_refine_sjx.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IsInRfArea_sjx must include a Fortran placeholder element",
        )
    })?;
    if mp_id.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mp_id length {} must cover sjx_points {sjx_points}",
                mp_id.len()
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

    let ref_colnum =
        usize::from(config.mean_threshold.is_some()) + usize::from(config.std_threshold.is_some());
    let mut ref_th = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut ref_sjx = vec![0; sjx_points + 1];
    let mut p_num = vec![0; sjx_points + 1];
    let mut mean = vec![0.0; sjx_points + 1];

    for sjx_index in config.num_vertex + 1..=sjx_points {
        if is_in_refine_sjx[sjx_index] != 1 {
            continue;
        }
        let count = usize_from_i32_nonnegative(mp_id[sjx_index][0], "mp_id count")?;
        let start = usize_from_i32_positive(mp_id[sjx_index][1], "mp_id start")?;
        for offset in 0..count {
            let lookup_index = start + offset;
            let lookup = mp_ii.get(lookup_index).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("mp_ii missing lookup index {lookup_index}"),
                )
            })?;
            let row = usize_from_i32_positive(lookup[0], "mp_ii row")?;
            let col = usize_from_i32_positive(lookup[1], "mp_ii col")?;
            let landtype = *landtypes
                .get(row)
                .and_then(|row_values| row_values.get(col))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("landtypes missing ({row},{col})"),
                    )
                })?;
            if landtype != config.maxlc {
                mean[sjx_index] += *var2d
                    .get(row)
                    .and_then(|row_values| row_values.get(col))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("var2d missing ({row},{col})"),
                        )
                    })?;
                p_num[sjx_index] += 1;
            }
        }
        if p_num[sjx_index] > 0 {
            mean[sjx_index] /= f64::from(p_num[sjx_index]);
        }
    }

    let mut col_index = 0;
    if let Some(threshold) = config.mean_threshold {
        col_index += 1;
        for sjx_index in config.num_vertex + 1..=sjx_points {
            if mean[sjx_index] > threshold {
                ref_th[sjx_index][col_index] = 1;
                ref_sjx[sjx_index] = 1;
            }
        }
    }

    let mut stddev = config.std_threshold.map(|_| vec![0.0; sjx_points + 1]);
    if let Some(values) = stddev.as_mut() {
        for sjx_index in config.num_vertex + 1..=sjx_points {
            if is_in_refine_sjx[sjx_index] != 1 || p_num[sjx_index] == 0 {
                continue;
            }
            let count = usize_from_i32_nonnegative(mp_id[sjx_index][0], "mp_id count")?;
            let start = usize_from_i32_positive(mp_id[sjx_index][1], "mp_id start")?;
            for offset in 0..count {
                let lookup_index = start + offset;
                let lookup = mp_ii.get(lookup_index).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("mp_ii missing lookup index {lookup_index}"),
                    )
                })?;
                let row = usize_from_i32_positive(lookup[0], "mp_ii row")?;
                let col = usize_from_i32_positive(lookup[1], "mp_ii col")?;
                let landtype = *landtypes
                    .get(row)
                    .and_then(|row_values| row_values.get(col))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("landtypes missing ({row},{col})"),
                        )
                    })?;
                if landtype != config.maxlc {
                    let value = *var2d
                        .get(row)
                        .and_then(|row_values| row_values.get(col))
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                format!("var2d missing ({row},{col})"),
                            )
                        })?;
                    values[sjx_index] += (value - mean[sjx_index]).powi(2);
                }
            }
            values[sjx_index] = (values[sjx_index] / f64::from(p_num[sjx_index])).sqrt();
        }
        if let Some(threshold) = config.std_threshold {
            col_index += 1;
            for sjx_index in config.num_vertex + 1..=sjx_points {
                if values[sjx_index] > threshold {
                    ref_th[sjx_index][col_index] = 1;
                    ref_sjx[sjx_index] = 1;
                }
            }
        }
    }

    Ok(GetRefMeanStd2DReport {
        ref_colnum,
        ref_th,
        ref_sjx,
        p_num,
        mean,
        stddev,
    })
}
