use std::io;

use crate::{is_ngrmm, refine_m1w1_to_m11w11_one_based};

/// Port of `MOD_refine.F90:sharp_concav_lop_judge`.
///
/// Builds LOP transition segment pairs for sharp-concavity boundary segments.
/// Segment matrices are represented as Canonical-indexed rows (`segment[i][j]`
/// corresponds to Canonical `segment(j, i)`).
pub fn refine_sharp_concav_lop_judge_one_based(
    num_ref: &mut usize,
    num_bdy_refine_segment: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    ngrmw_new: &[[usize; 3]],
    sjx_child: &[[usize; 2]],
    bdy_refine_segment: &[Vec<usize>],
    bdy_refine_segment_old: &[Vec<usize>],
    n_bdy_refine_segment: &[usize],
    ref_sjx_segment_temp: &mut [Vec<usize>],
    n_ref_sjx_segment_temp: &mut [usize],
) -> io::Result<()> {
    // `MOD_refine.F90:1411` allocates the segment tables with the column
    // dimension *equal* to the count, so Fortran's valid columns are `1..num`
    // and `size == num` -- there is no placeholder column. Both dimensions here
    // are counts, so the canonical `n + 1` convention, which is for tables
    // indexed by an entity id, does not apply. Guarding on `>=` rejected
    // exactly the shape the Fortran allocates and the segment maker produces.
    if num_bdy_refine_segment > bdy_refine_segment.len()
        || num_bdy_refine_segment > bdy_refine_segment_old.len()
        || num_bdy_refine_segment > ref_sjx_segment_temp.len()
        || num_bdy_refine_segment > n_ref_sjx_segment_temp.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_bdy_refine_segment must address sharp-concavity segment arrays",
        ));
    }

    if num_bdy_refine_segment > n_bdy_refine_segment.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_bdy_refine_segment must address the segment row counts",
        ));
    }

    for segment_id in 0..num_bdy_refine_segment {
        // How far along this segment there are consecutive triangles to pair
        // up. The caller has already eaten this round's head and decremented
        // the count, so the segment as it was is one longer -- and a segment
        // eaten to nothing arrives as zero, which the guard below turns into
        // the skip it means.
        //
        // Read from `n_ref_sjx_segment_temp` before, which is this function's
        // *output* count and is freshly zeroed every round -- so `tran_degree`
        // was always 1, every segment was skipped, and the judge never proposed
        // a flip. The 1-into-2 splits that build the transition rows each add
        // one to the degree of the corner they split away from, and the flips
        // are what take that back; without them a hexagonal cell reached degree
        // 8, which is past what the gridfile's dual and the mask post-process
        // are built for.
        let tran_degree = n_bdy_refine_segment[segment_id] + 1;
        if tran_degree == 1 {
            continue;
        }
        // Zero-based reach: `j` runs `0..tran_degree-1`, so this row is read to
        // `tran_degree - 2` and the `old` row to `tran_degree - 1`.
        if bdy_refine_segment[segment_id].len() < tran_degree - 1
            || bdy_refine_segment_old[segment_id].len() < tran_degree
            || ref_sjx_segment_temp[segment_id].len() < 4 * (tran_degree - 1)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("segment {segment_id} storage is shorter than tran_degree {tran_degree}"),
            ));
        }

        let mut valid_pairs = 0_usize;
        for j in 0..(tran_degree - 1) {
            let m1 = bdy_refine_segment_old[segment_id][j];
            let w0 = bdy_refine_segment[segment_id][j];
            let m2 = bdy_refine_segment_old[segment_id][j + 1];
            if m1 <= 1 || w0 <= 1 || m2 <= 1 {
                break;
            }
            if w0 == 0 || w0 >= triangle_neighbors.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("segment {segment_id} w0={w0} must address triangle_neighbors"),
                ));
            }
            let Some(w1) = triangle_neighbors[w0].iter().copied().find(|&neighbor| {
                neighbor > 0 && neighbor < mrl_new.len() && mrl_new[neighbor] != 1
            }) else {
                break;
            };

            let Some((m11, w11)) = refine_m1w1_to_m11w11_one_based(m1, w1, sjx_child, ngrmw_new)?
            else {
                continue;
            };

            if w1 >= sjx_child.len() || m2 >= sjx_child.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("w1={w1} and m2={m2} must address sjx_child"),
                ));
            }
            let mut w22 = sjx_child[w1][0];
            if w22 == w11 {
                w22 = sjx_child[w1][1];
            }
            if w22 == 0 || w22 >= ngrmw_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("w22={w22} must address child connectivity"),
                ));
            }
            let m22 = sjx_child[m2]
                .iter()
                .copied()
                .filter(|&candidate| candidate > 0 && candidate < ngrmw_new.len())
                .find(|&candidate| is_ngrmm(ngrmw_new[w22], ngrmw_new[candidate]).is_some())
                .unwrap_or(0);
            if m22 == 0 {
                continue;
            }

            valid_pairs += 1;
            let out = 4 * (valid_pairs - 1);
            ref_sjx_segment_temp[segment_id][out] = m11;
            ref_sjx_segment_temp[segment_id][out + 1] = w11;
            ref_sjx_segment_temp[segment_id][out + 2] = w22;
            ref_sjx_segment_temp[segment_id][out + 3] = m22;
        }

        let effective_tran_degree = valid_pairs + 1;
        if effective_tran_degree == 1 {
            n_ref_sjx_segment_temp[segment_id] = 0;
            continue;
        }
        let num_end = 4 * valid_pairs;
        n_ref_sjx_segment_temp[segment_id] = (effective_tran_degree / 2) * 4;
        *num_ref += n_ref_sjx_segment_temp[segment_id];
        if effective_tran_degree == 2 {
            continue;
        }
        // `ref_sjx_lop_temp(k+2:k+3, i) = ref_sjx_lop_temp(num_end-k:num_end-k+1, i)`
        // with Fortran `k = k0 + 1`, so the source starts two before `num_end - k0`.
        for k in (0..n_ref_sjx_segment_temp[segment_id]).step_by(4) {
            let src = num_end.checked_sub(k + 2).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid sharp-concavity mirror source",
                )
            })?;
            ref_sjx_segment_temp[segment_id][k + 2] = ref_sjx_segment_temp[segment_id][src];
            ref_sjx_segment_temp[segment_id][k + 3] = ref_sjx_segment_temp[segment_id][src + 1];
        }
    }

    Ok(())
}
