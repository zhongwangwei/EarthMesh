use std::io;

use crate::{refine_boundary_segments_one_based, RefineBoundarySegments};

/// Public pure-data port of `MOD_refine.F90:bdy_refine_segment_make`.
///
/// `closed_curves` are the unique boundary vertices per curve, without the
/// repeated tail slot.  The helper applies the same Canonical rotation rule for
/// `set_dis_in > 1`, splits long straight runs, and returns the unrefined
/// triangle id shared by each adjacent boundary-cell pair.
pub fn refine_boundary_segments_make_one_based(
    set_dis_in: usize,
    closed_curves: &[Vec<usize>],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<RefineBoundarySegments> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:bdy_refine_segment_make",
        ));
    }
    let mut bdy_refine_segment = Vec::new();
    let mut curve_segment_ends = Vec::with_capacity(closed_curves.len());
    for curve in closed_curves {
        bdy_refine_segment.extend(refine_boundary_segments_one_based(
            set_dis_in,
            std::slice::from_ref(curve),
            triangles_on_cell,
            edge_counts,
            mrl_new,
        )?);
        curve_segment_ends.push(bdy_refine_segment.len());
    }
    let n_bdy_refine_segment = bdy_refine_segment.iter().map(Vec::len).collect::<Vec<_>>();
    // `MOD_refine.F90:1411` gives the table a fixed row dimension of
    // `set_dis_in`, with the "triangle id 1" placeholder standing for a slot a
    // shorter segment does not fill. `n_bdy_refine_segment` already carries the
    // real length, so padding costs nothing and spares every consumer a bounds
    // check the Fortran never had -- the reverse judge rejects a ragged row
    // outright, and the forward pass was only surviving one by clamping its own
    // loop.
    for row in &mut bdy_refine_segment {
        row.resize(set_dis_in.max(row.len()), 1);
    }
    Ok(RefineBoundarySegments {
        num_bdy_refine_segment: bdy_refine_segment.len(),
        bdy_refine_segment,
        n_bdy_refine_segment,
        curve_segment_ends,
    })
}
