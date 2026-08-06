use std::io;

/// Port of `MOD_refine.F90:weak_concav_pair_special`.
///
/// Handles weak-concavity pairs whose adjacent segment length is one: records
/// each weak triangle's outward transition triangle, marks that outward triangle
/// for reverse one-into-two refinement, writes the paired weak-concavity segment
/// entry for triangles sharing the paired weak triangle, and defers disjoint
/// neighbors for an `mrl_new=4` renewal after the scan.
pub fn refine_weak_concav_pair_special_one_based(
    num_weak_concav_pair: usize,
    num_ref_weak_concav: usize,
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    mrl_new: &mut [i32],
    ref_sjx: &mut [i32],
    weak_concav_pair: &mut [[usize; 2]],
    weak_concav_segment: &mut [Vec<usize>],
) -> io::Result<()> {
    if num_weak_concav_pair >= weak_concav_pair.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_weak_concav_pair {num_weak_concav_pair} must address weak_concav_pair"),
        ));
    }
    if num_ref_weak_concav < num_weak_concav_pair {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_ref_weak_concav must be at least num_weak_concav_pair",
        ));
    }

    // NOTE, before converting this to zero-based the way `refine_lop_sharp` was:
    // the pairing below turns on the *parity* of a one-based index, and parity
    // inverts when the base does -- one-based even is zero-based odd. Getting it
    // backwards pairs each triangle with the wrong partner and returns no error
    // at all; the mesh comes out valid and wrong, which is the failure class
    // section 11.1 of the technical guide is about. Check the rule against
    // `MOD_refine.F90:1712` before touching the loop, not after.
    let mut mrl_renew = vec![None; num_weak_concav_pair + 1];
    for k in 1..=num_weak_concav_pair {
        let m1 = weak_concav_pair[k][0];
        let pair_index = if k % 2 == 0 {
            k.checked_sub(1)
        } else {
            k.checked_add(1)
        }
        .filter(|&idx| idx >= 1 && idx <= num_weak_concav_pair)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak concavity pair {k} has no paired triangle"),
            )
        })?;
        let m2 = weak_concav_pair[pair_index][0];
        if m1 == 0
            || m1 >= triangle_neighbors.len()
            || m1 >= mrl_new.len()
            || m1 >= ref_sjx.len()
            || m1 >= cells_on_triangle.len()
            || m2 == 0
            || m2 >= cells_on_triangle.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak concavity triangles m1={m1}, m2={m2} must address inputs"),
            ));
        }

        let m3 = triangle_neighbors[m1]
            .iter()
            .copied()
            .find(|&neighbor| neighbor > 0 && neighbor < mrl_new.len() && mrl_new[neighbor] != 4)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("weak concavity triangle {m1} has no outward non-refined neighbor"),
                )
            })?;
        if m3 >= triangle_neighbors.len()
            || m3 >= cells_on_triangle.len()
            || m3 >= ref_sjx.len()
            || m3 >= mrl_new.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("outward triangle {m3} must address refinement inputs"),
            ));
        }
        weak_concav_pair[k][1] = m3;
        ref_sjx[m3] = 1;

        for &m4 in &triangle_neighbors[m3] {
            if m4 == 0 || m4 >= mrl_new.len() || m4 >= cells_on_triangle.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("neighbor {m4} of outward triangle {m3} must address inputs"),
                ));
            }
            if mrl_new[m4] == 4 {
                continue;
            }
            let shares_vertex_with_pair = cells_on_triangle[m4]
                .iter()
                .any(|vertex| cells_on_triangle[m2].contains(vertex));
            if shares_vertex_with_pair {
                let segment_id = num_ref_weak_concav - num_weak_concav_pair + k;
                if segment_id >= weak_concav_segment.len()
                    || weak_concav_segment[segment_id].is_empty()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("weak_concav_segment {segment_id} must have a first slot"),
                    ));
                }
                weak_concav_segment[segment_id][0] = m4;
            } else {
                mrl_renew[k] = Some(m4);
            }
        }
    }

    for triangle in mrl_renew.iter().skip(1).flatten().copied() {
        if triangle >= mrl_new.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("deferred renewal triangle {triangle} must address mrl_new"),
            ));
        }
        mrl_new[triangle] = 4;
    }

    Ok(())
}
