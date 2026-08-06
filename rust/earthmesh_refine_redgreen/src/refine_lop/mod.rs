use std::io;

use crate::{checked_lop_edge_flip, crossline_check_canonical, LonLatDegrees};

/// Port of `MOD_refine.F90:Delaunay_Lop`.
///
/// Applies diagonal flips for adjacent triangle pairs listed in a one-based
/// `ref_sjx_segment` array, writes replacement triangles after
/// `num_mp[iter-1]`, clears old triangle connectivity to Canonical placeholder
/// `1`, and preserves the Canonical dateline/crossline cleanup behavior.
pub fn refine_delaunay_lop_one_based(
    iter: usize,
    num_ref: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle: &mut [[usize; 3]],
    ref_sjx_segment: &[usize],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    // `MOD_refine.F90:1938` takes `ref_sjx_lop(num_ref)` -- size equals the
    // count, no placeholder -- and reads `2k-1`/`2k` for `k = 1, num_ref/2`,
    // which in zero-based terms is `2k`/`2k+1` for `k = 0, num_ref/2`.
    if num_ref > ref_sjx_segment.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_ref must address ref_sjx_segment entries",
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_mp[iter] >= cells_on_triangle.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_mp[{iter}] {} exceeds triangle storage", num_mp[iter]),
        ));
    }
    if num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_wp[{iter}] {} exceeds cell storage", num_wp[iter]),
        ));
    }

    let mut refed_iter = 0_usize;
    for k in 0..(num_ref / 2) {
        let i = ref_sjx_segment[2 * k];
        let j = ref_sjx_segment[2 * k + 1];
        if i == 0 || j == 0 {
            continue;
        }
        if i >= cells_on_triangle.len() || j >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP pair i={i}, j={j} must address triangle connectivity"),
            ));
        }
        let m1 = num_mp[iter - 1] + refed_iter * 2 + 1;
        let m2 = num_mp[iter - 1] + refed_iter * 2 + 2;
        if m2 >= triangle_points.len() || m2 >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP output m2={m2} exceeds triangle storage"),
            ));
        }

        let flip = checked_lop_edge_flip(
            i,
            j,
            cells_on_triangle[i],
            cells_on_triangle[j],
            cell_points,
        )?;
        cells_on_triangle[m1] = flip.triangles[0];
        cells_on_triangle[m2] = flip.triangles[1];
        triangle_points[m1] = flip.centroids[0];
        triangle_points[m2] = flip.centroids[1];
        cells_on_triangle[i] = [1, 1, 1];
        cells_on_triangle[j] = [1, 1, 1];
        refed_iter += 1;
    }

    crossline_check_canonical(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}
