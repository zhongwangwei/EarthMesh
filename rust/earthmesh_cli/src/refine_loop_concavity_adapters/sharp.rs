use std::io;

use earthmesh_mesh::refine_sharp_concav_lop_judge_fortran_indexed as refine_sharp_concav_lop_judge_mesh_fortran_indexed;

use crate::{fortran_rows_to_triangle_major, SharpConcavLopJudgeReport};

/// CLI adapter for `MOD_refine.F90:sharp_concav_lop_judge`.
///
/// Converts `ngrmw_new(1:3, triangle)` rows into the triangle-major child
/// connectivity expected by the mesh kernel, then reports the segment entries
/// written back into the caller-owned temporary LOP segment matrices.
#[allow(clippy::too_many_arguments)]
pub fn apply_sharp_concav_lop_judge_fortran_indexed(
    num_ref: &mut usize,
    num_bdy_refine_segment: usize,
    max_triangle: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    ngrmw_new: &[Vec<usize>],
    sjx_child: &[[usize; 2]],
    bdy_refine_segment: &[Vec<usize>],
    bdy_refine_segment_old: &[Vec<usize>],
    n_bdy_refine_segment: &[usize],
    ref_sjx_segment_temp: &mut [Vec<usize>],
    n_ref_sjx_segment_temp: &mut [usize],
) -> io::Result<SharpConcavLopJudgeReport> {
    let before_num_ref = *num_ref;
    let cells_on_triangle_new = fortran_rows_to_triangle_major(ngrmw_new, max_triangle)?;

    refine_sharp_concav_lop_judge_mesh_fortran_indexed(
        num_ref,
        num_bdy_refine_segment,
        mrl_new,
        triangle_neighbors,
        &cells_on_triangle_new,
        sjx_child,
        bdy_refine_segment,
        bdy_refine_segment_old,
        n_bdy_refine_segment,
        ref_sjx_segment_temp,
        n_ref_sjx_segment_temp,
    )?;

    let mut segment_lengths = Vec::new();
    let mut written_segments = Vec::new();
    for segment_id in 1..=num_bdy_refine_segment {
        let length = n_ref_sjx_segment_temp[segment_id];
        if length == 0 {
            continue;
        }
        segment_lengths.push((segment_id, length));
        let segment_values = ref_sjx_segment_temp[segment_id][1..=length].to_vec();
        if segment_values.iter().any(|&value| value != 0) {
            written_segments.push((segment_id, segment_values));
        }
    }

    Ok(SharpConcavLopJudgeReport {
        num_ref_added: *num_ref - before_num_ref,
        segment_lengths,
        written_segments,
    })
}
