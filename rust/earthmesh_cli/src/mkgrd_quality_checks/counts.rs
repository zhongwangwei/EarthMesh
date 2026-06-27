use std::io;

use crate::*;

/// Reproduce the post-`NGR_RENEW` counters that Fortran stores in
/// `num_mp_step(step)` and `num_wp_step(step)` for later global distance layers.
pub fn refine_loop_post_counts_fortran_indexed(
    old_triangle_count: usize,
    old_cell_count: usize,
    expanded_triangle_count: usize,
    compact_mesh: &UnstructuredMesh,
    lop_triangle_count: usize,
) -> io::Result<(usize, usize)> {
    let compact_triangle_count = compact_mesh.m_points.len();
    let removed_triangle_count = expanded_triangle_count
        .checked_sub(compact_triangle_count)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "expanded triangle count must be >= compact triangle count",
            )
        })?;
    let num_vertex = old_triangle_count
        .saturating_sub(removed_triangle_count)
        .checked_add(lop_triangle_count)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "post-refine num_vertex calculation overflowed",
            )
        })?;
    let mut num_center = old_cell_count;
    for triangle_id in (num_vertex + 1)..=compact_triangle_count {
        let row = compact_mesh.m_to_w.get(triangle_id - 1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("compact mesh is missing triangle row {triangle_id}"),
            )
        })?;
        for &cell_id in row {
            if cell_id > 0 && (cell_id as usize) < num_center {
                num_center = cell_id as usize;
            }
        }
    }
    Ok((num_vertex, num_center))
}
