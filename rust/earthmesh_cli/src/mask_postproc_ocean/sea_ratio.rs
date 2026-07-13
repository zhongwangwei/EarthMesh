use std::io;

use crate::{matrix_width, validate_contain_mesh, ContainMesh};

/// Pure-data port of the `MOD_mask_postproc.F90:mask_postproc_Ocn` adjustment:
///
/// ```text
/// IsInDmArea_ustr = IsInDmArea_ustr_read
/// do i = num_vertex + 1, ustr_points
///   if (ustr_id(i, 1) > 0) then
///     if (ustr_id(i, 1) / real(ustr_id(i, 3)) < mask_sea_ratio) IsInDmArea_ustr(i) = -1
///   end if
/// end do
/// ```
///
/// `num_vertex` is the Canonical one-based last initial vertex id; Rust row `0`
/// corresponds to Canonical row `1`.
pub fn apply_ocean_mask_sea_ratio_one_based(
    contain: &ContainMesh,
    num_vertex: usize,
    mask_sea_ratio: f64,
) -> io::Result<Vec<i32>> {
    validate_contain_mesh(contain)?;
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    if dim_a < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ocean mask ratio adjustment requires ustr_id rows with at least three columns",
        ));
    }
    if num_vertex > contain.ustr_id.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_vertex {num_vertex} exceeds num_ustr {}",
                contain.ustr_id.len()
            ),
        ));
    }

    let mut is_in_domain = contain.is_in_area_ustr.clone();
    for canonical_id in (num_vertex + 1)..=contain.ustr_id.len() {
        let row_idx = canonical_id - 1;
        let selected_pixels = contain.ustr_id[row_idx][0];
        if selected_pixels <= 0 {
            continue;
        }
        let denominator = contain.ustr_id[row_idx][2];
        if denominator <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "ocean mask ratio row {canonical_id} has non-positive denominator {denominator}"
                ),
            ));
        }
        if f64::from(selected_pixels) / f64::from(denominator) < mask_sea_ratio {
            is_in_domain[row_idx] = -1;
        }
    }

    Ok(is_in_domain)
}
