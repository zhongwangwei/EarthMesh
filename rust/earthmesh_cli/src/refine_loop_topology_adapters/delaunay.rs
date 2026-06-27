use std::io;

use earthmesh_mesh::{
    refine_delaunay_lop_fortran_indexed as refine_delaunay_lop_mesh_fortran_indexed, LonLatDegrees,
};

use crate::{fortran_rows_to_triangle_major, DelaunayLopReport, LonLatPoint};

#[allow(clippy::too_many_arguments)]
pub fn apply_delaunay_lop_fortran_indexed(
    iter: usize,
    num_ref: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    mp_new: &mut [LonLatPoint],
    wp_new: &mut [LonLatPoint],
    ngrmw_new: &mut [Vec<usize>],
    ref_sjx_segment: &[usize],
) -> io::Result<DelaunayLopReport> {
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
    let old_mp = num_mp[iter - 1];
    let new_mp = num_mp[iter];
    let new_wp = num_wp[iter];
    if mp_new.len() <= new_mp || wp_new.len() <= new_wp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_mp[{iter}] {new_mp} and num_wp[{iter}] {new_wp} exceed one-based point storage"),
        ));
    }
    if ngrmw_new.len() <= 3 || ngrmw_new[1..=3].iter().any(|row| row.len() <= new_mp) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_mp[{iter}] {new_mp} requires one-based ngrmw_new rows 1..=3"),
        ));
    }

    let mut cells_on_triangle = fortran_rows_to_triangle_major(ngrmw_new, new_mp)?;
    let mut flipped_pairs = Vec::new();
    let mut new_triangle_ids = Vec::new();
    let mut dateline_adjusted = false;
    let mut nonzero_pair_index = 0_usize;
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
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;
        for &vertex in cells_on_triangle[i]
            .iter()
            .chain(cells_on_triangle[j].iter())
        {
            if vertex == 0 || vertex >= wp_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("LOP pair i={i}, j={j} references invalid vertex {vertex}"),
                ));
            }
            min_lon = min_lon.min(wp_new[vertex].lon);
            max_lon = max_lon.max(wp_new[vertex].lon);
        }
        dateline_adjusted |= max_lon - min_lon > 180.0;
        flipped_pairs.push((i, j));
        let m1 = old_mp + nonzero_pair_index * 2 + 1;
        let m2 = old_mp + nonzero_pair_index * 2 + 2;
        if m2 > new_mp {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP pair i={i}, j={j} exceeds allocated child triangle storage"),
            ));
        }
        new_triangle_ids.extend([m1, m2]);
        nonzero_pair_index += 1;
    }

    let mut triangle_points: Vec<LonLatDegrees> = mp_new
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect();
    let mut cell_points: Vec<LonLatDegrees> = wp_new
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect();

    refine_delaunay_lop_mesh_fortran_indexed(
        iter,
        num_ref,
        num_mp,
        num_wp,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle,
        ref_sjx_segment,
    )?;

    for triangle in 1..=new_mp {
        ngrmw_new[1][triangle] = cells_on_triangle[triangle][0];
        ngrmw_new[2][triangle] = cells_on_triangle[triangle][1];
        ngrmw_new[3][triangle] = cells_on_triangle[triangle][2];
    }
    for triangle in 1..=new_mp {
        mp_new[triangle] = LonLatPoint {
            lon: triangle_points[triangle].lon_degrees,
            lat: triangle_points[triangle].lat_degrees,
        };
    }
    for vertex in 1..=new_wp {
        wp_new[vertex] = LonLatPoint {
            lon: cell_points[vertex].lon_degrees,
            lat: cell_points[vertex].lat_degrees,
        };
    }

    Ok(DelaunayLopReport {
        flipped_pairs,
        new_triangle_ids,
        dateline_adjusted,
    })
}
