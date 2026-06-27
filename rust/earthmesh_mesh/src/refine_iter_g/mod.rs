use std::io;

/// Port of `MOD_refine.F90:iterG_judge`.
///
/// Inputs preserve Fortran indexing: row 0 is unused, polygon/cell ids start
/// after `num_center`, `triangles_on_cell[cell]` corresponds to
/// `ngrwm(1:n_ngrwm(cell), cell)`, and `mrl_new[triangle] == 1` means the
/// triangle is still unrefined.  A six-edge polygon with refinement-state sum
/// 18 marks its unrefined adjacent triangles as weak-concavity refinements.
pub fn refine_iter_g_judge_fortran_indexed(
    num_center: usize,
    lbx_points: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<Vec<i32>> {
    if lbx_points >= triangles_on_cell.len() || lbx_points >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "lbx_points {lbx_points} must be addressable in triangles_on_cell ({}) and edge_counts ({})",
                triangles_on_cell.len(),
                edge_counts.len()
            ),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    let mut ref_sjx = vec![0_i32; mrl_new.len()];

    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell];
        if num_edges > neighbors.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cell {cell} edge_count {num_edges} exceeds neighbor row length {}",
                    neighbors.len()
                ),
            ));
        }
        for &triangle in &neighbors[..num_edges] {
            if triangle == 0 || triangle > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} has invalid triangle neighbor {triangle}"),
                ));
            }
        }
        if num_edges != 6 {
            continue;
        }
        let state_sum: i32 = neighbors[..num_edges]
            .iter()
            .map(|&triangle| mrl_new[triangle])
            .sum();
        if state_sum != 18 {
            continue;
        }
        for &triangle in &neighbors[..num_edges] {
            if mrl_new[triangle] == 1 {
                ref_sjx[triangle] = 1;
            }
        }
    }

    Ok(ref_sjx)
}
