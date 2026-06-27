use std::io;

use earthmesh_mesh::{
    refine_ngr_renew_fortran_indexed as refine_ngr_renew_mesh_fortran_indexed, LonLatDegrees,
};

use crate::{fortran_rows_to_triangle_major, LonLatPoint, NgrRenewReport};

#[allow(clippy::too_many_arguments)]
pub fn apply_ngr_renew_fortran_indexed(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    mp_new: &[LonLatPoint],
    wp_new: &[LonLatPoint],
    ngrmw_new: &[Vec<usize>],
    mp_f: &mut Vec<LonLatPoint>,
    wp_f: &mut Vec<LonLatPoint>,
    ngrmw_f: &mut Vec<Vec<usize>>,
    ngrwm_f: &mut Vec<Vec<usize>>,
    n_ngrwm_f: &mut Vec<usize>,
    bdy_refine: &mut Vec<usize>,
    bdy_refine_tran: &mut Vec<usize>,
) -> io::Result<NgrRenewReport> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let new_mp = num_mp[iter];
    let new_wp = num_wp[iter];
    if mp_new.len() <= new_mp || wp_new.len() <= new_wp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_mp[{iter}] {new_mp} and num_wp[{iter}] {new_wp} exceed one-based point storage"),
        ));
    }
    let cells_on_triangle_new = fortran_rows_to_triangle_major(ngrmw_new, new_mp)?;
    let triangle_points_new: Vec<LonLatDegrees> = mp_new
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect();
    let cell_points_new: Vec<LonLatDegrees> = wp_new
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect();

    let renewed = refine_ngr_renew_mesh_fortran_indexed(
        iter,
        num_vertex,
        num_mp,
        num_wp,
        &triangle_points_new,
        &cell_points_new,
        &cells_on_triangle_new,
        bdy_refine,
        bdy_refine_tran,
    )?;

    *mp_f = vec![LonLatPoint { lon: 0.0, lat: 0.0 }; renewed.num_sjx + 1];
    for triangle in 1..=renewed.num_sjx {
        mp_f[triangle] = LonLatPoint {
            lon: renewed.triangle_points[triangle].lon_degrees,
            lat: renewed.triangle_points[triangle].lat_degrees,
        };
    }

    *wp_f = vec![
        LonLatPoint {
            lon: 9999.0,
            lat: 9999.0
        };
        renewed.num_dbx + 1
    ];
    for vertex in 1..=renewed.num_dbx {
        wp_f[vertex] = LonLatPoint {
            lon: renewed.cell_points[vertex].lon_degrees,
            lat: renewed.cell_points[vertex].lat_degrees,
        };
    }

    *ngrmw_f = vec![vec![1_usize; renewed.num_sjx + 1]; 4];
    for triangle in 1..=renewed.num_sjx {
        ngrmw_f[1][triangle] = renewed.cells_on_triangle[triangle][0];
        ngrmw_f[2][triangle] = renewed.cells_on_triangle[triangle][1];
        ngrmw_f[3][triangle] = renewed.cells_on_triangle[triangle][2];
    }

    *n_ngrwm_f = renewed.n_triangles_on_cell.clone();
    let adjacency_capacity = 7_usize.max(
        renewed
            .n_triangles_on_cell
            .iter()
            .take(renewed.num_dbx + 1)
            .copied()
            .max()
            .unwrap_or(0),
    );
    *ngrwm_f = vec![vec![1_usize; renewed.num_dbx + 1]; adjacency_capacity + 1];
    for vertex in 1..=renewed.num_dbx {
        for (offset, &triangle) in renewed.triangles_on_cell[vertex].iter().enumerate() {
            ngrwm_f[offset + 1][vertex] = triangle;
        }
    }

    *bdy_refine = renewed.boundary_refine.clone();
    *bdy_refine_tran = renewed.boundary_refine_transition.clone();

    Ok(NgrRenewReport {
        num_sjx: renewed.num_sjx,
        num_dbx: renewed.num_dbx,
        vertex_mapping: renewed.vertex_mapping,
        adjacency_capacity,
        boundary_refine: bdy_refine.clone(),
        boundary_refine_transition: bdy_refine_tran.clone(),
    })
}
