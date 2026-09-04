use std::io;

use crate::{checked_lop_edge_flip, crossline_check_canonical, LonLatDegrees};
use earthmesh_mesh::polygon_length_angle_metrics;

/// Port of `MOD_refine.F90:Delaunay_Lop`.
///
/// Applies diagonal flips for adjacent triangle pairs listed in a one-based
/// `ref_sjx_segment` array, writes replacement triangles after
/// `num_mp[iter-1]`, clears old triangle connectivity to Canonical placeholder
/// `1`, and preserves the Canonical dateline/crossline cleanup behavior.
/// Connectivity a triangle carries once something has consumed it.
const DELETED_TRIANGLE: [usize; 3] = [1, 1, 1];

fn angle_range(
    triangles: [[usize; 3]; 2],
    cell_points: &[LonLatDegrees],
) -> io::Result<(f64, f64)> {
    let mut range = (f64::INFINITY, f64::NEG_INFINITY);
    for triangle in triangles {
        let points = triangle
            .map(|cell| cell_points.get(cell).copied())
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing LOP cell"))?;
        let metrics = polygon_length_angle_metrics(&points)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "degenerate LOP pair"))?;
        for angle in metrics.angles_degrees {
            range.0 = range.0.min(angle);
            range.1 = range.1.max(angle);
        }
    }
    Ok(range)
}

pub fn refine_delaunay_lop_one_based(
    iter: usize,
    num_ref: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle: &mut [[usize; 3]],
    ref_sjx_segment: &[usize],
    protect_triangle_quality: bool,
) -> io::Result<usize> {
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
        // Two boundary segments that meet at a corner both see the pair there,
        // so the same pair can be proposed twice -- and a flip consumes the two
        // triangles it rebuilds, leaving them with the deleted marker. Flipping
        // a consumed pair is not a request that can be honoured, and is not a
        // defect in the proposal either; skipping is the same answer the loop
        // already gives for a slot that names no triangle.
        if cells_on_triangle[i] == DELETED_TRIANGLE || cells_on_triangle[j] == DELETED_TRIANGLE {
            continue;
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
        if protect_triangle_quality {
            let current_angles =
                angle_range([cells_on_triangle[i], cells_on_triangle[j]], cell_points)?;
            let candidate_angles = angle_range(flip.triangles, cell_points)?;
            if candidate_angles.0 < current_angles.0 - 1.0e-9
                || candidate_angles.1 > current_angles.1 + 1.0e-9
            {
                continue;
            }
        }
        cells_on_triangle[m1] = flip.triangles[0];
        cells_on_triangle[m2] = flip.triangles[1];
        triangle_points[m1] = flip.centroids[0];
        triangle_points[m2] = flip.centroids[1];
        cells_on_triangle[i] = [1, 1, 1];
        cells_on_triangle[j] = [1, 1, 1];
        refed_iter += 1;
    }

    crossline_check_canonical(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(refed_iter)
}
