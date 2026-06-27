use std::io;

use earthmesh_mesh::refine_m1w1_to_m11w11_fortran_indexed as refine_m1w1_to_m11w11_mesh_fortran_indexed;

use crate::{fortran_rows_to_triangle_major, M1W1LookupReport};

pub fn lookup_m1w1_to_m11w11_fortran_indexed(
    m1: usize,
    w1: usize,
    sjx_child: &[[usize; 2]],
    ngrmw_new: &[Vec<usize>],
    num_mp: usize,
) -> io::Result<M1W1LookupReport> {
    let cells_on_triangle_new = fortran_rows_to_triangle_major(ngrmw_new, num_mp)?;
    let child_pair =
        refine_m1w1_to_m11w11_mesh_fortran_indexed(m1, w1, sjx_child, &cells_on_triangle_new)?;
    Ok(M1W1LookupReport {
        parent_pair: (m1, w1),
        child_pair,
    })
}
