use std::io;

use earthmesh_mesh::{
    refine_onedivide_two_fortran_indexed as refine_onedivide_two_mesh_fortran_indexed,
    LonLatDegrees,
};

use crate::{fortran_rows_to_triangle_major, LonLatPoint, OnedivideTwoReport};

#[allow(clippy::too_many_arguments)]
pub fn apply_onedivide_two_fortran_indexed(
    iter: usize,
    is_reverse: bool,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_neighbors: &[Vec<usize>],
    ngrmw: &[Vec<usize>],
    ref_sjx: &[i32],
    mrl_new: &[i32],
    mp_new: &mut [LonLatPoint],
    wp_new: &mut [LonLatPoint],
    ngrmw_new: &mut [Vec<usize>],
    sjx_child: &mut [[usize; 2]],
) -> io::Result<OnedivideTwoReport> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let old_mp = num_mp[iter - 1];
    let base_mp = *num_mp
        .get(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
    let new_mp = num_mp[iter];
    let old_wp = num_wp[iter - 1];
    let new_wp = num_wp[iter];
    if ref_sjx.len() <= base_mp || mrl_new.len() <= base_mp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("base triangle count {base_mp} requires one-based ref_sjx/mrl_new storage"),
        ));
    }
    if mp_new.len() <= new_mp || wp_new.len() <= new_wp || sjx_child.len() <= base_mp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_mp[{iter}] {new_mp} and num_wp[{iter}] {new_wp} exceed one-based output storage"),
        ));
    }
    if ngrmw.len() <= 3 || ngrmw_new.len() <= 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ngrmw/ngrmw_new must expose one-based rows 1..=3",
        ));
    }
    let parent_ngrmw: &[Vec<usize>] = if ngrmw[1..=3].iter().all(|row| row.len() > base_mp) {
        ngrmw
    } else if ngrmw_new[1..=3].iter().all(|row| row.len() > base_mp) {
        &*ngrmw_new
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "base triangle count {base_mp} requires ngrmw rows with at least {} columns",
                base_mp + 1
            ),
        ));
    };
    if ngrmw_new[1..=3].iter().any(|row| row.len() <= new_mp) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[{iter}] {new_mp} requires ngrmw_new rows with at least {} columns",
                new_mp + 1
            ),
        ));
    }

    let split_triangles: Vec<usize> = ((num_vertex + 1)..=base_mp)
        .filter(|&triangle| ref_sjx[triangle] != 0)
        .collect();
    let mut new_triangle_ids = Vec::with_capacity(split_triangles.len() * 2);
    let mut new_vertex_ids = Vec::with_capacity(split_triangles.len());
    let mut dateline_adjusted = false;
    for (split_idx, &triangle) in split_triangles.iter().enumerate() {
        let vertex_ids = [
            parent_ngrmw[1][triangle],
            parent_ngrmw[2][triangle],
            parent_ngrmw[3][triangle],
        ];
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;
        for &vertex_id in &vertex_ids {
            if vertex_id == 0 || vertex_id > old_wp || vertex_id >= wp_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "ngrmw vertex id {vertex_id} for triangle {triangle} is outside wp_new"
                    ),
                ));
            }
            min_lon = min_lon.min(wp_new[vertex_id].lon);
            max_lon = max_lon.max(wp_new[vertex_id].lon);
        }
        dateline_adjusted |= max_lon - min_lon > 180.0;
        let m1 = old_mp + split_idx * 2 + 1;
        let m2 = old_mp + split_idx * 2 + 2;
        let w4 = old_wp + split_idx + 1;
        if m2 > new_mp || w4 > new_wp {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "one-into-two split for triangle {triangle} exceeds allocated child storage"
                ),
            ));
        }
        new_triangle_ids.extend([m1, m2]);
        new_vertex_ids.push(w4);
    }

    let mut cells_on_triangle = fortran_rows_to_triangle_major(parent_ngrmw, base_mp)?;
    let mut cells_on_triangle_new = fortran_rows_to_triangle_major(ngrmw_new, new_mp)?;
    cells_on_triangle.resize(new_mp + 1, [0, 0, 0]);
    let mut triangle_points: Vec<LonLatDegrees> = mp_new
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect();
    let mut cell_points: Vec<LonLatDegrees> = wp_new
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect();

    refine_onedivide_two_mesh_fortran_indexed(
        iter,
        is_reverse,
        num_vertex,
        num_mp,
        num_wp,
        triangle_neighbors,
        &cells_on_triangle,
        ref_sjx,
        mrl_new,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
        sjx_child,
    )?;

    for triangle in 1..=new_mp {
        ngrmw_new[1][triangle] = cells_on_triangle_new[triangle][0];
        ngrmw_new[2][triangle] = cells_on_triangle_new[triangle][1];
        ngrmw_new[3][triangle] = cells_on_triangle_new[triangle][2];
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

    Ok(OnedivideTwoReport {
        split_triangles,
        new_triangle_ids,
        new_vertex_ids,
        dateline_adjusted,
    })
}
