use std::io;

use crate::{LonLatPoint, OnedivideFourRenewReport};

use super::geometry::{
    centroid3, check_crossing_fortran_points, crossline_check_fortran_points, midpoint,
};

/// Port of the non-dateline coordinate/connectivity core from
/// `MOD_refine.F90:OnedivideFour_renew`.
///
/// Inputs keep the same one-based/sentinel layout as the Fortran arrays.
/// `num_mp[1]`/`num_wp[1]` are the pre-renewal counts and `num_mp[iter]` /
/// `num_wp[iter]` are the allocated post-renewal capacities.  Dateline
/// `CheckCrossing`/`crossline_check` behavior is intentionally rejected here
/// until covered by an explicit fixture.
pub fn apply_onedivide_four_renew_fortran_indexed(
    num_vertex: usize,
    iter: usize,
    ngrmw: &[Vec<usize>],
    ref_sjx_segment: &[i32],
    num_mp: &[usize],
    num_wp: &[usize],
    mp_new: &mut [LonLatPoint],
    wp_new: &mut [LonLatPoint],
    ngrmw_new: &mut [Vec<usize>],
) -> io::Result<OnedivideFourRenewReport> {
    if iter == 0
        || iter >= num_mp.len()
        || iter >= num_wp.len()
        || num_mp.len() <= 1
        || num_wp.len() <= 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} requires one-based num_mp/num_wp arrays with slots 1 and iter"),
        ));
    }
    let old_mp = num_mp[1];
    let old_wp = num_wp[1];
    let new_mp_capacity = num_mp[iter];
    let new_wp_capacity = num_wp[iter];
    if ref_sjx_segment.len() <= old_mp || mp_new.len() <= new_mp_capacity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[1] {old_mp} and num_mp[{iter}] {new_mp_capacity} require one-based ref_sjx_segment/mp_new storage"
            ),
        ));
    }
    if wp_new.len() <= new_wp_capacity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_wp[{iter}] {new_wp_capacity} requires one-based wp_new storage"),
        ));
    }
    if ngrmw.len() <= 3 || ngrmw[1..=3].iter().any(|row| row.len() <= old_mp) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[1] {old_mp} requires one-based ngrmw rows 1..3 with length at least {}",
                old_mp + 1
            ),
        ));
    }
    if ngrmw_new.len() <= 3
        || ngrmw_new[1..=3]
            .iter()
            .any(|row| row.len() <= new_mp_capacity)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[{iter}] {new_mp_capacity} requires one-based ngrmw_new rows 1..3 with length at least {}",
                new_mp_capacity + 1
            ),
        ));
    }

    let mut refined_triangles = Vec::new();
    let mut new_triangle_ids = Vec::new();
    let mut new_vertex_ids = Vec::new();
    let mut dateline_adjusted = false;
    let mut refed_iter = 0usize;
    for i in (num_vertex + 1)..=old_mp {
        if ref_sjx_segment[i] == 0 {
            continue;
        }
        let source_vertices = [ngrmw[1][i], ngrmw[2][i], ngrmw[3][i]];
        for &w in &source_vertices {
            if w == 0 || w > old_wp || w >= wp_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("ngrmw vertex id {w} for triangle {i} is outside wp_new"),
                ));
            }
        }

        let mut sjx = [
            wp_new[source_vertices[0]],
            wp_new[source_vertices[1]],
            wp_new[source_vertices[2]],
        ];
        let min_lon = sjx.iter().map(|p| p.lon).fold(f64::INFINITY, f64::min);
        let max_lon = sjx.iter().map(|p| p.lon).fold(f64::NEG_INFINITY, f64::max);
        let crosses_dateline = max_lon - min_lon > 180.0;
        if crosses_dateline {
            check_crossing_fortran_points(&mut sjx);
            dateline_adjusted = true;
        }

        let m0 = old_mp + refed_iter * 4;
        let w0 = old_wp + refed_iter * 3;
        if m0 + 4 > new_mp_capacity || w0 + 3 > new_wp_capacity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "renewal for triangle {i} would write m{}..m{} and w{}..w{} beyond allocated capacities",
                    m0 + 1,
                    m0 + 4,
                    w0 + 1,
                    w0 + 3
                ),
            ));
        }

        let mut newdbx = [
            midpoint(sjx[1], sjx[2]),
            midpoint(sjx[0], sjx[2]),
            midpoint(sjx[0], sjx[1]),
        ];
        let mut newsjx = [
            centroid3(sjx[0], newdbx[1], newdbx[2]),
            centroid3(sjx[1], newdbx[0], newdbx[2]),
            centroid3(sjx[2], newdbx[0], newdbx[1]),
            centroid3(newdbx[2], newdbx[0], newdbx[1]),
        ];
        if crosses_dateline {
            check_crossing_fortran_points(&mut newsjx);
            check_crossing_fortran_points(&mut newdbx);
        }

        for (offset, point) in newdbx.into_iter().enumerate() {
            wp_new[w0 + offset + 1] = point;
            new_vertex_ids.push(w0 + offset + 1);
        }
        for (offset, point) in newsjx.into_iter().enumerate() {
            mp_new[m0 + offset + 1] = point;
            new_triangle_ids.push(m0 + offset + 1);
        }

        ngrmw_new[2][m0 + 1] = w0 + 3;
        ngrmw_new[3][m0 + 1] = w0 + 2;
        ngrmw_new[2][m0 + 2] = w0 + 1;
        ngrmw_new[3][m0 + 2] = w0 + 3;
        ngrmw_new[2][m0 + 3] = w0 + 2;
        ngrmw_new[3][m0 + 3] = w0 + 1;
        ngrmw_new[1][m0 + 4] = w0 + 1;
        ngrmw_new[2][m0 + 4] = w0 + 2;
        ngrmw_new[3][m0 + 4] = w0 + 3;
        for k in 1..=3 {
            ngrmw_new[k][i] = 1;
            ngrmw_new[1][m0 + k] = ngrmw[k][i];
        }

        refined_triangles.push(i);
        refed_iter += 1;
    }

    crossline_check_fortran_points(iter, num_mp, num_wp, mp_new, wp_new)?;

    Ok(OnedivideFourRenewReport {
        refined_triangles,
        new_triangle_ids,
        new_vertex_ids,
        dateline_adjusted,
    })
}
