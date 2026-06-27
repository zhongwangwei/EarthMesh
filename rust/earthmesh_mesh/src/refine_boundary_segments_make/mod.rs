use std::io;

use crate::{refine_boundary_segments_fortran_indexed, RefineBoundarySegments};

/// Public pure-data port of `MOD_refine.F90:bdy_refine_segment_make`.
///
/// `closed_curves` are the unique boundary vertices per curve, without the
/// repeated tail slot.  The helper applies the same Fortran rotation rule for
/// `set_dis_in > 1`, splits long straight runs, and returns the unrefined
/// triangle id shared by each adjacent boundary-cell pair.
pub fn refine_boundary_segments_make_fortran_indexed(
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
    let bdy_refine_segment = refine_boundary_segments_fortran_indexed(
        set_dis_in,
        closed_curves,
        triangles_on_cell,
        edge_counts,
        mrl_new,
    )?;
    let n_bdy_refine_segment = bdy_refine_segment.iter().map(Vec::len).collect::<Vec<_>>();
    Ok(RefineBoundarySegments {
        num_bdy_refine_segment: bdy_refine_segment.len(),
        bdy_refine_segment,
        n_bdy_refine_segment,
    })
}
