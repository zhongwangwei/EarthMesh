use std::io;

use crate::validate_refine_cell_neighbors;

/// Port of `MOD_refine.F90:iterF_judge`.
///
/// Builds the protection halo around the original icosahedron vertices
/// (`impent`) using Fortran one-based `ngrwm/n_ngrwm` connectivity.  If a
/// protected region still contains an `mrl_new == 1` triangle, all protected
/// `mrl_new == 0` triangles are marked for refinement.
pub fn refine_iter_f_judge_fortran_indexed(
    num_sjx: usize,
    num_dbx: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
    impent: &[usize],
    vertex_protect_layers: usize,
) -> io::Result<Vec<i32>> {
    if num_sjx >= mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_sjx {num_sjx} must be addressable in mrl_new length {}",
                mrl_new.len()
            ),
        ));
    }
    if num_dbx >= triangles_on_cell.len() || num_dbx >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_dbx {num_dbx} must be addressable in cell connectivity arrays"),
        ));
    }
    if impent.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "impent must include at least one protected original vertex cell",
        ));
    }

    for cell in 2..=num_dbx {
        validate_refine_cell_neighbors(cell, triangles_on_cell, edge_counts, num_sjx, None)?;
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for &protected_cell in impent {
        if protected_cell == 0 || protected_cell > num_dbx {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("impent protected cell {protected_cell} is outside 1..={num_dbx}"),
            ));
        }
        if edge_counts[protected_cell] < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "impent protected cell {protected_cell} must expose at least five triangles"
                ),
            ));
        }

        let mut protected_triangles = vec![0_i32; mrl_new.len()];
        for &triangle in &triangles_on_cell[protected_cell][..5] {
            if triangle == 0 || triangle > num_sjx {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "impent protected cell {protected_cell} has invalid triangle {triangle}"
                    ),
                ));
            }
            protected_triangles[triangle] = 1;
        }

        for _ in 0..vertex_protect_layers {
            let mut boundary_cells = vec![0_i32; edge_counts.len()];
            for cell in 2..=num_dbx {
                let num_edges = edge_counts[cell];
                let count: i32 = triangles_on_cell[cell][..num_edges]
                    .iter()
                    .map(|&triangle| protected_triangles[triangle])
                    .sum();
                if count == 0 || count == num_edges as i32 {
                    continue;
                }
                boundary_cells[cell] = 1;
            }

            for cell in 2..=num_dbx {
                if boundary_cells[cell] != 1 {
                    continue;
                }
                let num_edges = edge_counts[cell];
                for &triangle in &triangles_on_cell[cell][..num_edges] {
                    protected_triangles[triangle] = 1;
                }
            }
        }

        let has_unrefined_one = (2..=num_sjx)
            .any(|triangle| protected_triangles[triangle] == 1 && mrl_new[triangle] == 1);
        if !has_unrefined_one {
            continue;
        }
        for triangle in 2..=num_sjx {
            if protected_triangles[triangle] == 1 && mrl_new[triangle] == 0 {
                ref_sjx[triangle] = 1;
            }
        }
    }

    Ok(ref_sjx)
}
