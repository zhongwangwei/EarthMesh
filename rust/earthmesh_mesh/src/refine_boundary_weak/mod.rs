use std::io;

use crate::{is_ngrmm, RefineWeakConcavitySegments};

/// Public pure-data port of `MOD_refine.F90:weak_concav_segment_make`.
///
/// The input boundary segments are Rust vectors in Fortran traversal order
/// (`bdy_refine_segment[:, i]` becomes one `Vec<usize>`).  Adjacent segment
/// pairs whose boundary triangles are opposite neighbors by `IsNgrmm` are
/// removed from the ordinary boundary segment list and emitted either as
/// singleton weak-concavity pairs (`weak_concav_pair`) or as weak-concavity
/// transition segments (`weak_concav_segment`).
pub fn refine_weak_concav_segment_make_fortran_indexed(
    set_dis_in: usize,
    num_ref_weak_concav: usize,
    cells_on_triangle: &[[usize; 3]],
    bdy_refine_segment: &[Vec<usize>],
) -> io::Result<RefineWeakConcavitySegments> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:weak_concav_segment_make",
        ));
    }

    let mut bdy_refine_segment_next = bdy_refine_segment.to_vec();
    let mut weak_concav_segment = Vec::<Vec<usize>>::new();
    let mut weak_concav_pair = Vec::<[usize; 2]>::new();
    let num_bdy_refine_segment = bdy_refine_segment.len();

    for i in 0..num_bdy_refine_segment {
        let j = (i + 1) % num_bdy_refine_segment;
        if bdy_refine_segment_next[i].is_empty() || bdy_refine_segment_next[j].is_empty() {
            continue;
        }
        let segment_i = &bdy_refine_segment[i];
        let segment_j = &bdy_refine_segment[j];
        if segment_i.is_empty() || segment_j.is_empty() {
            continue;
        }
        let m1 = *segment_i.last().expect("non-empty segment");
        let m2 = segment_j[0];
        if m1 >= cells_on_triangle.len() || m2 >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak-concavity pair references triangles {m1} and {m2} outside cells_on_triangle"),
            ));
        }
        if is_ngrmm(cells_on_triangle[m1], cells_on_triangle[m2]).is_none() {
            continue;
        }

        let len_i = segment_i.len();
        let len_j = segment_j.len();
        let num_max = len_i.max(len_j);
        let num_min = len_i.min(len_j);
        let num_diff = num_max - num_min;

        if num_diff == 0 {
            if len_i == 1 {
                weak_concav_pair.push([m1, 0]);
                weak_concav_pair.push([m2, 0]);
            } else {
                weak_concav_segment.push(segment_i.clone());
                weak_concav_segment.push(segment_j.clone());
            }
            bdy_refine_segment_next[i].clear();
            bdy_refine_segment_next[j].clear();
        } else if num_diff == 1 {
            if num_min < 3 {
                weak_concav_segment.push(vec![m1]);
                weak_concav_segment.push(vec![m2]);
                if num_min == 2 {
                    if len_i > len_j {
                        bdy_refine_segment_next[i].pop();
                    } else if !bdy_refine_segment_next[j].is_empty() {
                        bdy_refine_segment_next[j].remove(0);
                    }
                }
            } else {
                weak_concav_pair.push([m1, 0]);
                weak_concav_pair.push([m2, 0]);
                bdy_refine_segment_next[i].pop();
                if !bdy_refine_segment_next[j].is_empty() {
                    bdy_refine_segment_next[j].remove(0);
                }
            }
        } else if num_min == 1 {
            weak_concav_pair.push([m1, 0]);
            weak_concav_pair.push([m2, 0]);
            bdy_refine_segment_next[i].pop();
            if !bdy_refine_segment_next[j].is_empty() {
                bdy_refine_segment_next[j].remove(0);
            }
        } else {
            let common_len = num_min;
            let weak_i_start = len_i.saturating_sub(common_len);
            weak_concav_segment.push(segment_i[weak_i_start..].to_vec());
            weak_concav_segment.push(segment_j[..common_len].to_vec());
            bdy_refine_segment_next[i].truncate(weak_i_start);
            bdy_refine_segment_next[j] = segment_j[common_len..].to_vec();
        }
    }

    let num_weak_concav_segment = weak_concav_segment.len();
    let num_weak_concav_pair = weak_concav_pair.len();
    let mut all_weak_concav_segment = weak_concav_segment.clone();
    all_weak_concav_segment.extend(weak_concav_pair.iter().map(|pair| vec![pair[0]]));
    let n_weak_concav_segment = all_weak_concav_segment
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>();
    let num_ref_weak_concav = num_ref_weak_concav.max(all_weak_concav_segment.len());
    let n_bdy_refine_segment = bdy_refine_segment_next
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>();

    Ok(RefineWeakConcavitySegments {
        num_ref_weak_concav,
        num_weak_concav_segment,
        num_weak_concav_pair,
        bdy_refine_segment: bdy_refine_segment_next,
        n_bdy_refine_segment,
        weak_concav_segment: all_weak_concav_segment,
        n_weak_concav_segment,
        weak_concav_pair,
    })
}
