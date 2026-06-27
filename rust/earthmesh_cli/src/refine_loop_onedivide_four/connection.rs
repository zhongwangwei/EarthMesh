use std::io;

use crate::OnedivideFourConnectionReport;

/// Port of `MOD_refine.F90:OnedivideFour_connection`.
///
/// Inputs use the same one-based/sentinel layout as the Fortran arrays:
/// `ref_sjx[1..=sjx_points]`, `mrl_new[1..=sjx_points]`, and
/// `ngrmw[1..=3][1..=sjx_points]`.  Selected, still-unrefined triangles after
/// `num_vertex` are marked `4`, and their three polygon vertices are marked in
/// `ref_lbx`.
pub fn apply_onedivide_four_connection_fortran_indexed(
    num_vertex: usize,
    sjx_points: usize,
    ref_sjx: &[i32],
    ngrmw: &[Vec<usize>],
    ref_lbx: &mut [i32],
    mrl_new: &mut [i32],
) -> io::Result<OnedivideFourConnectionReport> {
    if ref_sjx.len() <= sjx_points || mrl_new.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "sjx_points {sjx_points} requires one-based ref_sjx and mrl_new arrays with length at least {}",
                sjx_points + 1
            ),
        ));
    }
    if ngrmw.len() <= 3 || ngrmw[1..=3].iter().any(|row| row.len() <= sjx_points) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "sjx_points {sjx_points} requires one-based ngrmw rows 1..3 with length at least {}",
                sjx_points + 1
            ),
        ));
    }
    if num_vertex >= sjx_points {
        return Ok(OnedivideFourConnectionReport {
            marked_triangles: Vec::new(),
            marked_vertices: Vec::new(),
        });
    }

    let mut marked_triangles = Vec::new();
    let mut marked_vertices = Vec::new();
    for i in (num_vertex + 1)..=sjx_points {
        if ref_sjx[i] == 0 || mrl_new[i] != 1 {
            continue;
        }
        for row in ngrmw.iter().take(4).skip(1) {
            let w = row[i];
            if w == 0 || w >= ref_lbx.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("ngrmw vertex id {w} for triangle {i} is outside ref_lbx"),
                ));
            }
            ref_lbx[w] = 1;
            if !marked_vertices.contains(&w) {
                marked_vertices.push(w);
            }
        }
        mrl_new[i] = 4;
        marked_triangles.push(i);
    }

    Ok(OnedivideFourConnectionReport {
        marked_triangles,
        marked_vertices,
    })
}
