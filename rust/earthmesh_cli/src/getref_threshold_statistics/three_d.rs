use std::io;

use crate::*;

/// Calculate two-layer mean/std thresholds like `MOD_GetRef.F90:mean_std_cal3d`.
pub fn calculate_getref_mean_std_3d_fortran_indexed(
    is_in_refine_sjx: &[i32],
    mp_id: &[Vec<i32>],
    mp_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    var3d: &[Vec<Vec<f64>>],
    config: GetRefMeanStd3DConfig,
) -> io::Result<GetRefMeanStd3DReport> {
    require_getref_lookup_width("mp_id", mp_id, 2)?;
    require_getref_lookup_width("mp_ii", mp_ii, 2)?;
    if landtypes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "landtypes must include Fortran placeholder rows",
        ));
    }
    let _ = matrix_width("landtypes", landtypes)?;
    require_getref_two_layer_values("var3d", var3d)?;
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

    let ref_colnum = usize::from(config.mean_thresholds.is_some())
        + usize::from(config.std_thresholds.is_some());
    let mut ref_th = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut ref_sjx = vec![0; sjx_points + 1];
    let mut p_num = vec![0; sjx_points + 1];
    let mut mean = vec![[0.0, 0.0]; sjx_points + 1];

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
                mean[sjx_index][0] += get_getref_layer_value(var3d, 0, row, col)?;
                mean[sjx_index][1] += get_getref_layer_value(var3d, 1, row, col)?;
                p_num[sjx_index] += 1;
            }
        }
        if p_num[sjx_index] > 0 {
            mean[sjx_index][0] /= f64::from(p_num[sjx_index]);
            mean[sjx_index][1] /= f64::from(p_num[sjx_index]);
        }
    }

    let mut col_index = 0;
    if let Some(thresholds) = config.mean_thresholds {
        col_index += 1;
        for sjx_index in config.num_vertex + 1..=sjx_points {
            if mean[sjx_index][0] > thresholds[0] || mean[sjx_index][1] > thresholds[1] {
                ref_th[sjx_index][col_index] = 1;
                ref_sjx[sjx_index] = 1;
            }
        }
    }

    let mut stddev = config
        .std_thresholds
        .map(|_| vec![[0.0, 0.0]; sjx_points + 1]);
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
                    values[sjx_index][0] +=
                        (get_getref_layer_value(var3d, 0, row, col)? - mean[sjx_index][0]).powi(2);
                    values[sjx_index][1] +=
                        (get_getref_layer_value(var3d, 1, row, col)? - mean[sjx_index][1]).powi(2);
                }
            }
            values[sjx_index][0] = (values[sjx_index][0] / f64::from(p_num[sjx_index])).sqrt();
            values[sjx_index][1] = (values[sjx_index][1] / f64::from(p_num[sjx_index])).sqrt();
        }
        if let Some(thresholds) = config.std_thresholds {
            col_index += 1;
            for sjx_index in config.num_vertex + 1..=sjx_points {
                if values[sjx_index][0] > thresholds[0] || values[sjx_index][1] > thresholds[1] {
                    ref_th[sjx_index][col_index] = 1;
                    ref_sjx[sjx_index] = 1;
                }
            }
        }
    }

    Ok(GetRefMeanStd3DReport {
        ref_colnum,
        ref_th,
        ref_sjx,
        p_num,
        mean,
        stddev,
    })
}
