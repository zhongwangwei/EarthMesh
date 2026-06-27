use std::io;

use crate::{
    average_lonlat3, check_crossing_fortran_lonlat, crossline_check_fortran, midpoint_lonlat,
    LonLatDegrees,
};

/// Port of `MOD_refine.F90:OnedivideFour_renew`.
///
/// For each marked triangle, generates three new polygon-center points on the
/// original triangle edges, four child triangle-center points, and the
/// `ngrmw_new` child connectivity stencil used later by `NGR_RENEW`.
/// `num_mp` and `num_wp` preserve the Fortran iteration-count arrays, so
/// `num_mp[iter - 1]`/`num_wp[iter - 1]` are the previous endpoints and
/// `num_mp[iter]`/`num_wp[iter]` are the required output endpoints.
pub fn refine_onedivide_four_renew_fortran_indexed(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    cells_on_triangle: &[[usize; 3]],
    ref_sjx_segment: &[i32],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle_new: &mut [[usize; 3]],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let sjx_points = num_mp[iter - 1];
    if sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx_segment.len()
        || sjx_points >= cells_on_triangle_new.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("previous sjx_points {sjx_points} must be addressable in triangle arrays"),
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_mp[iter] >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[{iter}] {} exceeds triangle output storage",
                num_mp[iter]
            ),
        ));
    }
    if num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_wp[{iter}] {} exceeds cell output storage",
                num_wp[iter]
            ),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds previous sjx_points {sjx_points}"),
        ));
    }

    let mut refed_iter = 0_usize;
    for triangle in (num_vertex + 1)..=sjx_points {
        if ref_sjx_segment[triangle] == 0 {
            continue;
        }
        let parent_cells = cells_on_triangle[triangle];
        for &cell in &parent_cells {
            if cell == 0 || cell >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid parent cell {cell}"),
                ));
            }
        }

        let mut parent_points = [
            cell_points[parent_cells[0]],
            cell_points[parent_cells[1]],
            cell_points[parent_cells[2]],
        ];
        let crosses_dateline = parent_points
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::NEG_INFINITY, f64::max)
            - parent_points
                .iter()
                .map(|point| point.lon_degrees)
                .fold(f64::INFINITY, f64::min)
            > 180.0;
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut parent_points);
        }

        let mut new_cell_points = [
            midpoint_lonlat(parent_points[1], parent_points[2]),
            midpoint_lonlat(parent_points[0], parent_points[2]),
            midpoint_lonlat(parent_points[0], parent_points[1]),
        ];
        let mut new_triangle_points = [
            average_lonlat3(parent_points[0], new_cell_points[1], new_cell_points[2]),
            average_lonlat3(parent_points[1], new_cell_points[0], new_cell_points[2]),
            average_lonlat3(parent_points[2], new_cell_points[0], new_cell_points[1]),
            average_lonlat3(new_cell_points[2], new_cell_points[0], new_cell_points[1]),
        ];
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut new_triangle_points);
            check_crossing_fortran_lonlat(&mut new_cell_points);
        }

        let m0 = num_mp[iter - 1] + refed_iter * 4;
        let w0 = num_wp[iter - 1] + refed_iter * 3;
        if m0 + 4 >= triangle_points.len()
            || m0 + 4 >= cells_on_triangle_new.len()
            || w0 + 3 >= cell_points.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refined triangle {triangle} exceeds allocated child storage"),
            ));
        }

        triangle_points[(m0 + 1)..=(m0 + 4)].copy_from_slice(&new_triangle_points);
        cell_points[(w0 + 1)..=(w0 + 3)].copy_from_slice(&new_cell_points);

        cells_on_triangle_new[m0 + 1][1] = w0 + 3;
        cells_on_triangle_new[m0 + 1][2] = w0 + 2;
        cells_on_triangle_new[m0 + 2][1] = w0 + 1;
        cells_on_triangle_new[m0 + 2][2] = w0 + 3;
        cells_on_triangle_new[m0 + 3][1] = w0 + 2;
        cells_on_triangle_new[m0 + 3][2] = w0 + 1;
        cells_on_triangle_new[m0 + 4] = [w0 + 1, w0 + 2, w0 + 3];
        for k in 0..3 {
            cells_on_triangle_new[triangle][k] = 1;
            cells_on_triangle_new[m0 + 1 + k][0] = parent_cells[k];
        }

        refed_iter += 1;
    }

    crossline_check_fortran(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}
