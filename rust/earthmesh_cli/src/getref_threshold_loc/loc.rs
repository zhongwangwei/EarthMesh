use std::io;

use crate::*;

/// Split the mixed land-ocean containment table used by `MOD_GetRef.F90:GetRef_LOC`.
///
/// Input and output arrays include row/column zero placeholders so they can be
/// passed directly to the migrated Fortran-indexed GetRef helper functions.
pub fn split_getref_loc_containment_fortran_indexed(
    loc_id: &[Vec<i32>],
    loc_ii: &[Vec<i32>],
    num_vertex: usize,
) -> io::Result<GetRefLocContainmentSplit> {
    require_getref_lookup_width("LOC_id", loc_id, 2)?;
    require_getref_lookup_width("LOC_ii", loc_ii, 3)?;
    let sjx_points = loc_id.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "LOC_id must include a Fortran placeholder row",
        )
    })?;
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    validate_getref_loc_segments(loc_id, loc_ii, num_vertex, sjx_points)?;

    let mut land_ii = vec![vec![0, 0]];
    let mut ocean_ii = vec![vec![0, 0]];
    for row in loc_ii.iter().skip(1) {
        match row[2] {
            0 => ocean_ii.push(vec![row[0], row[1]]),
            1 => land_ii.push(vec![row[0], row[1]]),
            value => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("LOC_ii land/ocean flag must be 0 or 1, got {value}"),
                ));
            }
        }
    }

    let mut land_id = vec![vec![0, 0]; sjx_points + 1];
    let mut ocean_id = vec![vec![0, 0, 0]; sjx_points + 1];
    for sjx_index in num_vertex + 1..=sjx_points {
        let count = usize_from_i32_nonnegative(loc_id[sjx_index][0], "LOC_id count")?;
        if count == 0 {
            continue;
        }
        let start = usize_from_i32_positive(loc_id[sjx_index][1], "LOC_id start")?;
        let mut land_count = 0_i32;
        for offset in 0..count {
            land_count += loc_ii[start + offset][2];
        }
        land_id[sjx_index][0] = land_count;
        ocean_id[sjx_index][0] = usize_to_i32("LOC ocean count", count)? - land_count;
        ocean_id[sjx_index][2] = usize_to_i32("LOC total count", count)?;
    }
    if num_vertex <= sjx_points {
        land_id[num_vertex][1] = 1;
        ocean_id[num_vertex][1] = 1;
    }
    for sjx_index in num_vertex + 1..=sjx_points {
        land_id[sjx_index][1] = land_id[sjx_index - 1][1] + land_id[sjx_index - 1][0];
        ocean_id[sjx_index][1] = ocean_id[sjx_index - 1][1] + ocean_id[sjx_index - 1][0];
    }

    let atmos_id = loc_id
        .iter()
        .map(|row| vec![row[0], row[1]])
        .collect::<Vec<_>>();
    let atmos_ii = loc_ii
        .iter()
        .map(|row| vec![row[0], row[1]])
        .collect::<Vec<_>>();

    Ok(GetRefLocContainmentSplit {
        sjx_points,
        num_vertex,
        land: GetRefContainmentLookup {
            mp_id: land_id,
            mp_ii: land_ii,
        },
        ocean: GetRefContainmentLookup {
            mp_id: ocean_id,
            mp_ii: ocean_ii,
        },
        atmos: GetRefContainmentLookup {
            mp_id: atmos_id,
            mp_ii: atmos_ii,
        },
    })
}

fn validate_getref_loc_segments(
    loc_id: &[Vec<i32>],
    loc_ii: &[Vec<i32>],
    num_vertex: usize,
    sjx_points: usize,
) -> io::Result<()> {
    let max_lookup = loc_ii.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "LOC_ii must include a Fortran placeholder row",
        )
    })?;
    for sjx_index in num_vertex + 1..=sjx_points {
        let count = usize_from_i32_nonnegative(loc_id[sjx_index][0], "LOC_id count")?;
        if count == 0 {
            continue;
        }
        let start = usize_from_i32_positive(loc_id[sjx_index][1], "LOC_id start")?;
        let end = start + count - 1;
        if end > max_lookup {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "LOC_id row {sjx_index} segment {start}..{end} exceeds LOC_ii rows {max_lookup}"
                ),
            ));
        }
    }
    Ok(())
}
