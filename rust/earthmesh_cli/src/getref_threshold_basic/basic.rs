use std::io;

use crate::*;

fn validate_getref_landtype_class(
    landtype: i32,
    maxlc: i32,
    row: usize,
    col: usize,
) -> io::Result<()> {
    if !(0..=maxlc).contains(&landtype) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("landtypes value {landtype} at ({row},{col}) outside 0..=maxlc {maxlc}"),
        ));
    }
    Ok(())
}

/// Calculate `refine_num_landtypes` and `refine_area_mainland` like `GetRef_Lnd`.
pub fn calculate_getref_land_basic_fortran_indexed(
    is_in_refine_sjx: &[i32],
    lnd_id: &[Vec<i32>],
    lnd_ii: &[Vec<i32>],
    landtypes: &[Vec<i32>],
    config: GetRefLandBasicConfig,
) -> io::Result<GetRefLandBasicReport> {
    let sjx_points = is_in_refine_sjx.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "IsInRfArea_sjx must include a Fortran placeholder element",
        )
    })?;
    if !config.refine_num_landtypes && !config.refine_area_mainland {
        return Ok(GetRefLandBasicReport {
            ref_colnum: 0,
            ref_th_land: vec![vec![0]; sjx_points + 1],
            ref_sjx: vec![0; sjx_points + 1],
            n_landtypes: None,
            f_mainarea: None,
        });
    }

    require_getref_lookup_width("Lnd_id", lnd_id, 2)?;
    require_getref_lookup_width("Lnd_ii", lnd_ii, 2)?;
    if landtypes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "landtypes must include a Fortran placeholder row",
        ));
    }
    let _ = matrix_width("landtypes", landtypes)?;
    if lnd_id.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Lnd_id length {} must cover sjx_points {sjx_points}",
                lnd_id.len()
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
        usize::from(config.refine_num_landtypes) + usize::from(config.refine_area_mainland);
    let mut ref_th_land = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut ref_sjx = vec![0; sjx_points + 1];
    let mut n_landtypes = config.refine_num_landtypes.then(|| vec![0; sjx_points + 1]);
    let mut f_mainarea = config
        .refine_area_mainland
        .then(|| vec![0.0; sjx_points + 1]);

    let mut col = 0;
    if let Some(values) = n_landtypes.as_mut() {
        col += 1;
        for sjx_index in config.num_vertex + 1..=sjx_points {
            if is_in_refine_sjx[sjx_index] != 1 {
                continue;
            }
            let count = usize_from_i32_nonnegative(lnd_id[sjx_index][0], "Lnd_id count")?;
            let start = usize_from_i32_positive(lnd_id[sjx_index][1], "Lnd_id start")?;
            let mut present = std::collections::BTreeSet::new();
            for offset in 0..count {
                let lookup_index = start + offset;
                let lookup = lnd_ii.get(lookup_index).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Lnd_ii missing lookup index {lookup_index}"),
                    )
                })?;
                let row = usize_from_i32_positive(lookup[0], "Lnd_ii row")?;
                let col_index = usize_from_i32_positive(lookup[1], "Lnd_ii col")?;
                let landtype = landtypes
                    .get(row)
                    .and_then(|row_values| row_values.get(col_index))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("landtypes missing ({row},{col_index})"),
                        )
                    })?;
                validate_getref_landtype_class(*landtype, config.maxlc, row, col_index)?;
                if *landtype != config.maxlc {
                    present.insert(*landtype);
                }
            }
            values[sjx_index] = i32::try_from(present.len()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "n_landtypes exceeds i32 range")
            })?;
            if values[sjx_index] > config.th_num_landtypes {
                ref_th_land[sjx_index][col] = 1;
                ref_sjx[sjx_index] = 1;
            }
        }
    }

    if let Some(values) = f_mainarea.as_mut() {
        col += 1;
        for sjx_index in config.num_vertex + 1..=sjx_points {
            if is_in_refine_sjx[sjx_index] != 1 {
                continue;
            }
            let count = usize_from_i32_nonnegative(lnd_id[sjx_index][0], "Lnd_id count")?;
            if count == 0 {
                continue;
            }
            let start = usize_from_i32_positive(lnd_id[sjx_index][1], "Lnd_id start")?;
            let mut counts = std::collections::BTreeMap::<i32, usize>::new();
            for offset in 0..count {
                let lookup_index = start + offset;
                let lookup = lnd_ii.get(lookup_index).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Lnd_ii missing lookup index {lookup_index}"),
                    )
                })?;
                let row = usize_from_i32_positive(lookup[0], "Lnd_ii row")?;
                let col_index = usize_from_i32_positive(lookup[1], "Lnd_ii col")?;
                let landtype = *landtypes
                    .get(row)
                    .and_then(|row_values| row_values.get(col_index))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("landtypes missing ({row},{col_index})"),
                        )
                    })?;
                validate_getref_landtype_class(landtype, config.maxlc, row, col_index)?;
                if landtype != config.maxlc {
                    *counts.entry(landtype).or_insert(0) += 1;
                }
            }
            let main_count = counts.values().copied().max().unwrap_or(0);
            values[sjx_index] = (main_count as f64 / count as f64).min(1.0);
            if values[sjx_index] < config.th_area_mainland {
                ref_th_land[sjx_index][col] = 1;
                ref_sjx[sjx_index] = 1;
            }
        }
    }

    Ok(GetRefLandBasicReport {
        ref_colnum,
        ref_th_land,
        ref_sjx,
        n_landtypes,
        f_mainarea,
    })
}
