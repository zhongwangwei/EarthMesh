use std::io;

/// Port of `MOD_refine.F90:iterB_judge`.
///
/// Inputs preserve Fortran indexing: row 0 is unused, `ngrmm[cell]` contains
/// the three neighboring triangle ids for `cell`, and `mrl_new[cell] == 4`
/// means the triangle has already been one-into-four refined.  The returned
/// `ref_sjx` has the same placeholder-inclusive length as `mrl_new`.
pub fn refine_iter_b_judge_fortran_indexed(
    set_dis_in: usize,
    num_vertex: usize,
    ngrmm: &[Vec<usize>],
    mrl_new: &[i32],
) -> io::Result<Vec<i32>> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:iterB_judge",
        ));
    }
    if ngrmm.len() != mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ngrmm rows {} must match mrl_new length {}",
                ngrmm.len(),
                mrl_new.len()
            ),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    for (cell, neighbors) in ngrmm.iter().enumerate().skip(num_vertex.saturating_add(1)) {
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("ngrmm row {cell} must contain exactly three neighbors"),
            ));
        }
        for &neighbor in neighbors {
            if neighbor == 0 || neighbor > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("ngrmm row {cell} has invalid neighbor {neighbor}"),
                ));
            }
        }
    }

    let mut mrl_in = vec![0_i32; mrl_new.len()];
    for i in (num_vertex + 1)..=sjx_points {
        if mrl_new[i] != 4 {
            continue;
        }
        for &neighbor in &ngrmm[i] {
            if mrl_new[neighbor] == 4 {
                continue;
            }
            mrl_in[neighbor] += 2;
        }
    }

    const HHH: [usize; 5] = [0, 1, 2, 0, 1];
    for _ in 1..set_dis_in {
        let mut mrl_bk = mrl_in.clone();
        for i in (num_vertex + 1)..=sjx_points {
            if mrl_new[i] == 4 || mrl_in[i] != 0 {
                continue;
            }
            let neighbors = &ngrmm[i];
            let transition_sum: i32 = neighbors.iter().map(|&neighbor| mrl_in[neighbor]).sum();
            if transition_sum != 4 {
                continue;
            }
            for j in 0..3 {
                let m1 = neighbors[HHH[j]];
                let m2 = neighbors[HHH[j + 1]];
                let m3 = neighbors[HHH[j + 2]];
                if mrl_in[m1] == 2 && mrl_in[m2] == 2 {
                    mrl_bk[i] += 2;
                    mrl_bk[m3] += 2;
                    break;
                }
            }
        }
        mrl_in = mrl_bk;
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for i in (num_vertex + 1)..=sjx_points {
        if mrl_new[i] == 4 {
            continue;
        }
        if mrl_in[i] >= 4 {
            ref_sjx[i] = 1;
        }
    }
    Ok(ref_sjx)
}

/// Port of the empty `MOD_refine.F90:orial_vertices_protect` placeholder.
///
/// The Fortran subroutine has no executable statements, so the Rust migration
/// intentionally preserves all caller-owned refinement markers unchanged.
pub fn refine_orial_vertices_protect_fortran_indexed(_ref_sjx: &mut [i32]) {}
