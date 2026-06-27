use std::io;

use earthmesh_mesh::refine_isreverse_judge_fortran_indexed as refine_isreverse_judge_mesh_fortran_indexed;

use crate::IsreverseJudgeReport;

/// CLI adapter for `MOD_refine.F90:ref_sjx_isreverse_judge`.
///
/// The underlying mesh kernel owns the Fortran-indexed neighbor/segment logic;
/// this wrapper adds execution evidence useful for stitching the transition-row
/// loop into the CLI refine-loop executor.
pub fn apply_isreverse_judge_fortran_indexed(
    set_dis_in: usize,
    num_segment: usize,
    triangle_neighbors: &[Vec<usize>],
    mrl_new: &[i32],
    segments: &mut [Vec<usize>],
    n_segments: &[usize],
) -> io::Result<IsreverseJudgeReport> {
    let ref_sjx = refine_isreverse_judge_mesh_fortran_indexed(
        set_dis_in,
        num_segment,
        triangle_neighbors,
        mrl_new,
        segments,
        n_segments,
    )?;
    let marked_triangles = ref_sjx
        .iter()
        .enumerate()
        .filter_map(|(triangle, &marker)| (marker != 0).then_some(triangle))
        .collect();
    let mut active_segments = Vec::new();
    let mut rewritten_segments = Vec::new();
    for segment_id in 0..num_segment.min(segments.len()).min(n_segments.len()) {
        if n_segments[segment_id] == 0 {
            continue;
        }
        active_segments.push(segment_id);
        rewritten_segments.push(segments[segment_id].clone());
    }

    Ok(IsreverseJudgeReport {
        ref_sjx,
        marked_triangles,
        active_segments,
        rewritten_segments,
    })
}
