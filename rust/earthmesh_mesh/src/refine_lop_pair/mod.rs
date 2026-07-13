use std::io;

use crate::is_ngrmm;

/// Port of `MOD_refine.F90:m1w1_to_m11w11`.
///
/// Given two parent triangles (`m1`, `w1`) and their two recorded children in
/// `sjx_child`, returns the first child pair that shares an edge according to
/// the Canonical `IsNgrmm` test.  Missing child adjacency is represented as
/// `Ok(None)`, matching the modern Canonical optional `found=.false.` path used by
/// weak-concavity LOP handling.
pub fn refine_m1w1_to_m11w11_one_based(
    m1: usize,
    w1: usize,
    sjx_child: &[[usize; 2]],
    ngrmw_new: &[[usize; 3]],
) -> io::Result<Option<(usize, usize)>> {
    if m1 == 0 || m1 >= sjx_child.len() || w1 == 0 || w1 >= sjx_child.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("parent ids m1={m1}, w1={w1} must address sjx_child"),
        ));
    }

    for &m11 in &sjx_child[m1] {
        for &w11 in &sjx_child[w1] {
            if m11 == 0 || w11 == 0 {
                continue;
            }
            if m11 >= ngrmw_new.len() || w11 >= ngrmw_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("child ids m11={m11}, w11={w11} must address ngrmw_new"),
                ));
            }
            if is_ngrmm(ngrmw_new[w11], ngrmw_new[m11]).is_some() {
                return Ok(Some((m11, w11)));
            }
        }
    }

    Ok(None)
}
