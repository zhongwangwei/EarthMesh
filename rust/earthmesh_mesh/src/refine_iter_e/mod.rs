use std::io;

use crate::{unique_triangle_cell, validate_refine_cell_neighbors};

/// Port of `MOD_refine.F90:iterE_judge`.
///
/// Finds adjacent refined-triangle pairs around polygons (`ngrwm`) that form a
/// convex refinement region.  If either opposite polygon across the pair has a
/// matching convex region, the Fortran routine marks one neighboring triangle
/// in `ref_sjx` to avoid the conflicting convex transition.  Inputs preserve
/// one-based Fortran indexing and placeholder row 0.
pub fn refine_iter_e_judge_fortran_indexed(
    num_center: usize,
    lbx_points: usize,
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
    ref_lbx: &[i32],
) -> io::Result<Vec<i32>> {
    if lbx_points >= triangles_on_cell.len()
        || lbx_points >= edge_counts.len()
        || lbx_points >= ref_lbx.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("lbx_points {lbx_points} must be addressable in all cell arrays"),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    let mut lbx_refine1 = vec![0_i32; lbx_points + 1];
    let mut lbx_refine2 = vec![0_usize; lbx_points + 1];
    let mut lbx_refine = vec![[0_usize; 2]; lbx_points + 1];

    for cell in (num_center + 1)..=lbx_points {
        validate_refine_cell_neighbors(
            cell,
            triangles_on_cell,
            edge_counts,
            sjx_points,
            Some(cells_on_triangle.len().saturating_sub(1)),
        )?;
        if ref_lbx[cell] == 0 {
            continue;
        }
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell][..num_edges];
        let state_sum: i32 = neighbors.iter().map(|&triangle| mrl_new[triangle]).sum();
        if state_sum != num_edges as i32 + 6 {
            continue;
        }
        for pos in 0..num_edges {
            let m1 = neighbors[pos];
            if mrl_new[m1] != 4 {
                continue;
            }
            let m2 = neighbors[(pos + 1) % num_edges];
            if mrl_new[m2] != 4 {
                continue;
            }
            lbx_refine1[cell] = 1;
            lbx_refine2[cell] = pos;
            lbx_refine[cell] = [m1, m2];
        }
    }

    if lbx_refine1.iter().sum::<i32>() == 0 {
        return Ok(vec![0_i32; mrl_new.len()]);
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for cell in (num_center + 1)..=lbx_points {
        if lbx_refine1[cell] == 0 {
            continue;
        }
        let [m1, m2] = lbx_refine[cell];
        let w1 = unique_triangle_cell(m1, m2, cells_on_triangle)?;
        let w2 = unique_triangle_cell(m2, m1, cells_on_triangle)?;
        let w1_refines = w1 <= lbx_points && lbx_refine1[w1] == 1;
        let w2_refines = w2 <= lbx_points && lbx_refine1[w2] == 1;
        if w1_refines || w2_refines {
            let num_edges = edge_counts[cell];
            let pos = lbx_refine2[cell];
            let mark_pos = if w1_refines {
                if pos == 0 {
                    num_edges - 1
                } else {
                    pos - 1
                }
            } else {
                (pos + 2) % num_edges
            };
            let triangle = triangles_on_cell[cell][mark_pos];
            ref_sjx[triangle] = 1;
            if w1_refines {
                lbx_refine1[w1] = 0;
            } else {
                lbx_refine1[w2] = 0;
            }
        }
        lbx_refine1[cell] = 0;
    }

    Ok(ref_sjx)
}
