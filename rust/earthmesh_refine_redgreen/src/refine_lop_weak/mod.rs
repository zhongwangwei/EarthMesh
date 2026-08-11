use std::io;

use crate::refine_m1w1_to_m11w11_one_based;

/// Port of `MOD_refine.F90:weak_concav_lop_judge`.
///
/// Weak-concavity segment matrices use zero-based inner slots for the Canonical
/// first dimension (`weak_concav_segment[i][0]` is Canonical
/// `weak_concav_segment(1, i)`), matching `weak_concav_pair_special` output.
/// `ref_sjx_segment_temp` remains one-based in the inner slot to match the LOP
/// segment consumers and `sharp_concav_lop_judge`.
pub fn refine_weak_concav_lop_judge_one_based(
    num_ref: &mut usize,
    num_bdy_refine_segment: usize,
    num_ref_weak_concav: usize,
    num_weak_concav_segment: usize,
    num_weak_concav_pair: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    ngrmw_new: &[[usize; 3]],
    sjx_child: &[[usize; 2]],
    weak_concav_segment: &mut [Vec<usize>],
    weak_concav_segment_old: &[Vec<usize>],
    n_weak_concav_segment: &[usize],
    weak_concav_pair: &[[usize; 2]],
    ref_sjx_segment_temp: &mut [Vec<usize>],
    n_ref_sjx_segment_temp: &mut [usize],
) -> io::Result<()> {
    // Zero-based on the *outer* index, as `MOD_refine.F90:1849` allocates:
    // `do i = 1, num` over tables sized by that count.
    //
    // The inner axes were already mixed and stay that way, which is what makes
    // this look inconsistent and is nevertheless correct: `weak_concav_segment`
    // and its `_old` are read zero-based (`(n(i)+1, i)` in Fortran is slot `n`
    // here), while `ref_sjx_segment_temp` was one-based and is converted, so it
    // agrees with `refine_lop_sharp` -- the driver hands both the same table.
    //
    // Both parities invert with the base. Fortran's `mod(i, 2) /= 0` is true
    // exactly when the zero-based index is even, and
    // `weak_concav_segment_old(j - mod(i,2) + 1, i)` reaches one slot further
    // on an odd one.
    if num_weak_concav_pair > weak_concav_pair.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_weak_concav_pair must address weak_concav_pair",
        ));
    }

    let num_end = if num_weak_concav_pair != 0 {
        for pair_id in 0..num_weak_concav_pair {
            let [m1, w1] = weak_concav_pair[pair_id];
            let Some((m11, w11)) = refine_m1w1_to_m11w11_one_based(m1, w1, sjx_child, ngrmw_new)?
            else {
                continue;
            };
            let segment_id = num_bdy_refine_segment + num_weak_concav_segment + pair_id;
            if segment_id >= n_ref_sjx_segment_temp.len()
                || segment_id >= ref_sjx_segment_temp.len()
                || ref_sjx_segment_temp[segment_id].len() < 2
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("pair segment {segment_id} must address ref_sjx_segment_temp"),
                ));
            }
            n_ref_sjx_segment_temp[segment_id] = 2;
            *num_ref += 2;
            ref_sjx_segment_temp[segment_id][0] = m11;
            ref_sjx_segment_temp[segment_id][1] = w11;
        }
        num_weak_concav_segment
    } else {
        num_ref_weak_concav
    };

    if num_weak_concav_segment == 0 {
        return Ok(());
    }
    if num_end > weak_concav_segment.len()
        || num_end > weak_concav_segment_old.len()
        || num_end > n_weak_concav_segment.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "weak concavity segment counts must address segment arrays",
        ));
    }

    for segment_id_weak in 0..num_end {
        if weak_concav_segment[segment_id_weak].is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak_concav_segment {segment_id_weak} must have a first slot"),
            ));
        }
        if weak_concav_segment[segment_id_weak][0] == 1 {
            continue;
        }
        let segment_id = segment_id_weak + num_bdy_refine_segment;
        if segment_id >= ref_sjx_segment_temp.len() || segment_id >= n_ref_sjx_segment_temp.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP segment {segment_id} must address ref_sjx_segment_temp"),
            ));
        }
        let mut kk = 0_usize;
        let n_segment = n_weak_concav_segment[segment_id_weak];

        if segment_id_weak % 2 == 0 {
            if segment_id_weak + 1 >= weak_concav_segment_old.len()
                || weak_concav_segment_old[segment_id_weak].len() <= n_segment
                || weak_concav_segment_old[segment_id_weak + 1].is_empty()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("odd weak segment {segment_id_weak} lacks intersegment old endpoints"),
                ));
            }
            let m1 = weak_concav_segment_old[segment_id_weak][n_segment];
            let w1 = weak_concav_segment_old[segment_id_weak + 1][0];
            if let Some((m11, w11)) = refine_m1w1_to_m11w11_one_based(m1, w1, sjx_child, ngrmw_new)?
            {
                if ref_sjx_segment_temp[segment_id].len() < kk + 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("LOP segment {segment_id} lacks intersegment output slots"),
                    ));
                }
                n_ref_sjx_segment_temp[segment_id] = 2;
                *num_ref += 2;
                ref_sjx_segment_temp[segment_id][kk] = m11;
                ref_sjx_segment_temp[segment_id][kk + 1] = w11;
                kk += 2;
            } else {
                continue;
            }
            if n_segment == 0 {
                for offset in 0..=1 {
                    let row = segment_id_weak + offset;
                    if row < weak_concav_segment.len() {
                        for value in &mut weak_concav_segment[row] {
                            *value = 1;
                        }
                    }
                }
                continue;
            }
        }

        for j in 0..n_segment {
            let old_slot = if segment_id_weak % 2 == 1 { j + 1 } else { j };
            if weak_concav_segment_old[segment_id_weak].len() <= old_slot
                || weak_concav_segment[segment_id_weak].len() <= j
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("weak segment {segment_id_weak} lacks internal slot {j}"),
                ));
            }
            let m1 = weak_concav_segment_old[segment_id_weak][old_slot];
            let w0 = weak_concav_segment[segment_id_weak][j];
            if w0 == 1 {
                break;
            }
            if w0 == 0 || w0 >= triangle_neighbors.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("weak segment {segment_id_weak} w0={w0} must address neighbors"),
                ));
            }
            let w1 = triangle_neighbors[w0]
                .iter()
                .copied()
                .find(|&neighbor| {
                    neighbor > 0 && neighbor < mrl_new.len() && mrl_new[neighbor] != 1
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "weak segment {segment_id_weak} w0={w0} has no reverse split neighbor"
                        ),
                    )
                })?;
            let Some((m11, w11)) = refine_m1w1_to_m11w11_one_based(m1, w1, sjx_child, ngrmw_new)?
            else {
                continue;
            };
            if ref_sjx_segment_temp[segment_id].len() < kk + 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("LOP segment {segment_id} lacks internal output slots"),
                ));
            }
            n_ref_sjx_segment_temp[segment_id] += 2;
            *num_ref += 2;
            ref_sjx_segment_temp[segment_id][kk] = m11;
            ref_sjx_segment_temp[segment_id][kk + 1] = w11;
            kk += 2;
        }
    }

    Ok(())
}
