use std::io;

use crate::{
    average_lonlat3, check_crossing_fortran_lonlat, crossline_check_fortran, LonLatDegrees,
};

/// Port of `MOD_refine.F90:Delaunay_Lop`.
///
/// Applies diagonal flips for adjacent triangle pairs listed in a one-based
/// `ref_sjx_segment` array, writes replacement triangles after
/// `num_mp[iter-1]`, clears old triangle connectivity to Fortran placeholder
/// `1`, and preserves the Fortran dateline/crossline cleanup behavior.
pub fn refine_delaunay_lop_fortran_indexed(
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
    if num_ref >= ref_sjx_segment.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_ref must address one-based ref_sjx_segment entries",
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
    for k in 1..=(num_ref / 2) {
        let i = ref_sjx_segment[2 * k - 1];
        let j = ref_sjx_segment[2 * k];
        if i == 0 || j == 0 {
            continue;
        }
        if i >= cells_on_triangle.len() || j >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP pair i={i}, j={j} must address triangle connectivity"),
            ));
        }
        let tri_i = cells_on_triangle[i];
        let tri_j = cells_on_triangle[j];
        let w1 = tri_i
            .iter()
            .copied()
            .find(|vertex| !tri_j.contains(vertex))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {i} has no vertex opposite {j}"),
                )
            })?;
        let w2 = tri_i
            .iter()
            .copied()
            .find(|&vertex| vertex != w1)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {i} lacks a first shared vertex"),
                )
            })?;
        let w4 = tri_i
            .iter()
            .copied()
            .find(|&vertex| vertex != w1 && vertex != w2)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {i} lacks a second shared vertex"),
                )
            })?;
        let w3 = tri_j
            .iter()
            .copied()
            .find(|vertex| !tri_i.contains(vertex))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {j} has no vertex opposite {i}"),
                )
            })?;
        for &cell in &[w1, w2, w3, w4] {
            if cell == 0 || cell >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("LOP pair i={i}, j={j} references invalid cell {cell}"),
                ));
            }
        }

        let m1 = num_mp[iter - 1] + refed_iter * 2 + 1;
        let m2 = num_mp[iter - 1] + refed_iter * 2 + 2;
        if m2 >= triangle_points.len() || m2 >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP output m2={m2} exceeds triangle storage"),
            ));
        }

        cells_on_triangle[m1] = [w1, w2, w3];
        cells_on_triangle[m2] = [w1, w4, w3];

        let mut quad_points = [
            cell_points[w1],
            cell_points[w2],
            cell_points[w3],
            cell_points[w4],
        ];
        let crosses_dateline = quad_points
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::NEG_INFINITY, f64::max)
            - quad_points
                .iter()
                .map(|point| point.lon_degrees)
                .fold(f64::INFINITY, f64::min)
            > 180.0;
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut quad_points);
        }
        let mut new_triangles = [
            average_lonlat3(quad_points[0], quad_points[1], quad_points[2]),
            average_lonlat3(quad_points[0], quad_points[3], quad_points[2]),
        ];
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut new_triangles);
        }
        triangle_points[m1] = new_triangles[0];
        triangle_points[m2] = new_triangles[1];
        cells_on_triangle[i] = [1, 1, 1];
        cells_on_triangle[j] = [1, 1, 1];
        refed_iter += 1;
    }

    crossline_check_fortran(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}
