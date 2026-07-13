use std::io;

use crate::{validate_refine_cell_neighbors, validate_triangle_neighbor_rows};

/// Port of `MOD_refine.F90:iterC_judge`.
///
/// Combines weak-concavity cleanup around already-refined polygons with the
/// `ref_lbx_in` transition propagation used to keep 5/6-edge cells from
/// exceeding the seven-edge refinement cap.  Inputs preserve Canonical one-based
/// indexing: row 0 is unused, triangle rows after `num_vertex` contain exactly
/// three `ngrmm` neighbors, and polygon rows after `num_center` use
/// `edge_counts[cell]` entries from `triangles_on_cell[cell]`.
pub fn refine_iter_c_judge_one_based(
    set_dis_in: usize,
    num_vertex: usize,
    num_center: usize,
    lbx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
    ref_lbx: &[i32],
) -> io::Result<Vec<i32>> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:iterC_judge",
        ));
    }
    if triangle_neighbors.len() != mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "triangle neighbor rows {} must match mrl_new length {}",
                triangle_neighbors.len(),
                mrl_new.len()
            ),
        ));
    }
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
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    validate_triangle_neighbor_rows(num_vertex, sjx_points, triangle_neighbors)?;
    for cell in (num_center + 1)..=lbx_points {
        validate_refine_cell_neighbors(cell, triangles_on_cell, edge_counts, sjx_points, None)?;
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];

    for cell in (num_center + 1)..=lbx_points {
        if ref_lbx[cell] == 0 {
            continue;
        }
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell][..num_edges];
        let state_sum: i32 = neighbors.iter().map(|&triangle| mrl_new[triangle]).sum();
        if num_edges == 5 {
            if state_sum > 10 {
                for &triangle in neighbors {
                    if mrl_new[triangle] == 1 {
                        ref_sjx[triangle] = 1;
                    }
                }
            }
        } else if num_edges == 6 && state_sum == 12 {
            for pos in 0..3 {
                let refined_a = neighbors[pos];
                let refined_b = neighbors[pos + 3];
                let gap_a = neighbors[pos + 1];
                let gap_b = neighbors[pos + 2];
                if mrl_new[refined_a] == 4
                    && mrl_new[refined_b] == 4
                    && mrl_new[gap_a] == 1
                    && mrl_new[gap_b] == 1
                {
                    ref_sjx[gap_a] = 1;
                    ref_sjx[gap_b] = 1;
                }
            }
        }
    }

    let mut mrl_in = vec![0_i32; mrl_new.len()];
    let mut mrl_bk = vec![0_i32; mrl_new.len()];
    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_new[triangle] != 4 {
            continue;
        }
        for &neighbor in &triangle_neighbors[triangle] {
            if mrl_new[neighbor] == 4 {
                continue;
            }
            mrl_in[neighbor] = 2;
        }
    }

    const HHH: [usize; 5] = [0, 1, 2, 0, 1];
    for _ in 1..set_dis_in {
        mrl_bk.fill(0);
        for triangle in (num_vertex + 1)..=sjx_points {
            if mrl_new[triangle] == 4 || mrl_in[triangle] != 0 {
                continue;
            }
            let neighbors = &triangle_neighbors[triangle];
            let transition_sum: i32 = neighbors.iter().map(|&neighbor| mrl_in[neighbor]).sum();
            if transition_sum != 4 {
                continue;
            }
            for pos in 0..3 {
                let m1 = neighbors[HHH[pos]];
                let m2 = neighbors[HHH[pos + 1]];
                let m3 = neighbors[HHH[pos + 2]];
                if mrl_in[m1] == 2 && mrl_in[m2] == 2 {
                    mrl_bk[triangle] += 2;
                    mrl_bk[m3] += 2;
                    break;
                }
            }
        }
        mrl_in.clone_from(&mrl_bk);
    }

    let mut ref_lbx_in = vec![vec![0_i32; 7]; lbx_points + 1];
    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        for (pos, &triangle) in triangles_on_cell[cell][..num_edges].iter().enumerate() {
            if mrl_bk[triangle] == 0 {
                continue;
            }
            let neighbor_state_sum: i32 = triangle_neighbors[triangle]
                .iter()
                .map(|&neighbor| mrl_new[neighbor])
                .sum();
            if neighbor_state_sum == 6 {
                ref_lbx_in[cell][pos] = 1;
            }
        }
    }

    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell][..num_edges];
        let state_sum: i32 = neighbors.iter().map(|&triangle| mrl_new[triangle]).sum();
        if ref_lbx[cell] != 0 {
            if num_edges == 6 && state_sum == 9 {
                let incoming_count: i32 = ref_lbx_in[cell][..num_edges].iter().sum();
                if 2 + incoming_count > 3 {
                    for &triangle in neighbors {
                        if mrl_new[triangle] == 1 {
                            ref_sjx[triangle] = 1;
                        }
                    }
                }
            }
        } else if num_edges == 5 || num_edges == 6 {
            let mut num_ref_lbx: Vec<f64> = ref_lbx_in[cell][..num_edges]
                .iter()
                .map(|&value| f64::from(value))
                .collect();
            for pos in 0..num_edges {
                let next = (pos + 1) % num_edges;
                if ref_lbx_in[cell][pos] == 1 && ref_lbx_in[cell][next] == 1 {
                    num_ref_lbx[pos] = 0.5;
                    num_ref_lbx[next] = 0.5;
                }
            }
            if num_ref_lbx.iter().sum::<f64>() + num_edges as f64 > 7.0 {
                for (pos, &triangle) in neighbors.iter().enumerate() {
                    if ref_lbx_in[cell][pos] != 0 && mrl_new[triangle] == 1 {
                        ref_sjx[triangle] = 1;
                    }
                }
            }
        }
    }

    Ok(ref_sjx)
}
