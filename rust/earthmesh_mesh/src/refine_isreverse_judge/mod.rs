use std::io;

/// Port of `MOD_refine.F90:ref_sjx_isreverse_judge`.
///
/// For each active boundary/weak-concavity segment, adjacent segment
/// triangles determine the shared neighbor that must be refined by reverse
/// one-into-two.  The segment is rewritten in-place to contain the next round
/// forward one-into-two candidates, preserving Fortran's placeholder `1`
/// behavior.
pub fn refine_isreverse_judge_fortran_indexed(
    set_dis_in: usize,
    num_segment: usize,
    triangle_neighbors: &[Vec<usize>],
    mrl_new: &[i32],
    segments: &mut [Vec<usize>],
    n_segments: &[usize],
) -> io::Result<Vec<i32>> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:ref_sjx_isreverse_judge",
        ));
    }
    if num_segment > segments.len() || num_segment > n_segments.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_segment {num_segment} exceeds segment arrays"),
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
    let sjx_points = mrl_new.len().saturating_sub(1);
    for (triangle, neighbors) in triangle_neighbors.iter().enumerate().skip(2) {
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle neighbor row {triangle} must contain exactly three neighbors"),
            ));
        }
        for &neighbor in neighbors {
            if neighbor > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle neighbor row {triangle} has invalid neighbor {neighbor}"),
                ));
            }
        }
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for segment_id in 0..num_segment {
        if n_segments[segment_id] == 0 {
            continue;
        }
        if segments[segment_id].len() < set_dis_in {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("segment {segment_id} length is shorter than set_dis_in {set_dis_in}"),
            ));
        }
        let segment_select = segments[segment_id][..set_dis_in].to_vec();
        segments[segment_id][..set_dis_in].fill(1);
        let mut next_segment_pos = 0usize;
        for j in 0..(set_dis_in - 1) {
            if segment_select[j + 1] == 1 {
                break;
            }
            let m0 = segment_select[j];
            let w0 = segment_select[j + 1];
            if m0 == 0 || m0 > sjx_points || w0 == 0 || w0 > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("segment {segment_id} references invalid triangle pair {m0}, {w0}"),
                ));
            }
            let Some(shared_neighbor) = triangle_neighbors[m0]
                .iter()
                .copied()
                .find(|candidate| *candidate > 1 && triangle_neighbors[w0].contains(candidate))
            else {
                break;
            };
            let next_triangle = triangle_neighbors[shared_neighbor]
                .iter()
                .copied()
                .rfind(|&candidate| candidate > 1 && mrl_new[candidate] != 4);
            let Some(next_triangle) = next_triangle else {
                continue;
            };
            ref_sjx[shared_neighbor] = 1;
            segments[segment_id][next_segment_pos] = next_triangle;
            next_segment_pos += 1;
        }
    }

    Ok(ref_sjx)
}
