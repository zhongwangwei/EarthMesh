use std::io;

use earthmesh_mesh::refine_weak_concav_pair_special_fortran_indexed as refine_weak_concav_pair_special_mesh_fortran_indexed;

use crate::{fortran_rows_to_triangle_major, WeakConcavPairSpecialReport};

/// CLI adapter for `MOD_refine.F90:weak_concav_pair_special`.
///
/// Converts `ngrmw(1:3, triangle)` rows to the mesh kernel's triangle-major
/// connectivity, then leaves all mutable weak-concavity state in-place exactly
/// where the refine-loop executor owns it.
#[allow(clippy::too_many_arguments)]
pub fn apply_weak_concav_pair_special_fortran_indexed(
    num_weak_concav_pair: usize,
    num_ref_weak_concav: usize,
    max_triangle: usize,
    triangle_neighbors: &[Vec<usize>],
    ngrmw: &[Vec<usize>],
    mrl_new: &mut [i32],
    ref_sjx: &mut [i32],
    weak_concav_pair: &mut [[usize; 2]],
    weak_concav_segment: &mut [Vec<usize>],
) -> io::Result<WeakConcavPairSpecialReport> {
    let cells_on_triangle = fortran_rows_to_triangle_major(ngrmw, max_triangle)?;
    let previous_mrl = mrl_new.to_vec();
    let previous_ref_sjx = ref_sjx.to_vec();
    let previous_segments = weak_concav_segment.to_vec();

    refine_weak_concav_pair_special_mesh_fortran_indexed(
        num_weak_concav_pair,
        num_ref_weak_concav,
        triangle_neighbors,
        &cells_on_triangle,
        mrl_new,
        ref_sjx,
        weak_concav_pair,
        weak_concav_segment,
    )?;

    let updated_pairs = weak_concav_pair
        .iter()
        .take(num_weak_concav_pair + 1)
        .skip(1)
        .copied()
        .collect();
    let marked_ref_sjx_triangles = ref_sjx
        .iter()
        .zip(previous_ref_sjx.iter())
        .enumerate()
        .filter_map(|(triangle, (&after, &before))| (after != before).then_some(triangle))
        .collect();
    let deferred_renew_triangles = mrl_new
        .iter()
        .zip(previous_mrl.iter())
        .enumerate()
        .filter_map(|(triangle, (&after, &before))| (after != before).then_some(triangle))
        .collect();
    let segment_first_slots = weak_concav_segment
        .iter()
        .zip(previous_segments.iter())
        .enumerate()
        .filter_map(|(segment_id, (after, before))| {
            if after.first() != before.first() {
                after
                    .first()
                    .copied()
                    .map(|triangle| (segment_id, triangle))
            } else {
                None
            }
        })
        .collect();

    Ok(WeakConcavPairSpecialReport {
        updated_pairs,
        marked_ref_sjx_triangles,
        deferred_renew_triangles,
        segment_first_slots,
    })
}
