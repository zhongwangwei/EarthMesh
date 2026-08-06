use std::io;

use crate::{
    is_ngrmm, refine_boundary_closed_curves_one_based, refine_boundary_segments_one_based,
    validate_refine_cell_neighbors, validate_triangle_neighbor_rows,
};

/// Port of `MOD_refine.F90:iterD_judge`.
///
/// Finds weak-concavity boundary segment pairs where one side has one
/// transition triangle and the neighboring side has more than one (`1+n`).
/// Such pairs are marked for extra refinement by setting both boundary
/// triangles in `ref_sjx`.  Inputs preserve Canonical one-based indexing:
/// triangle row 0 is unused, active triangle rows after `num_vertex` have
/// exactly three `triangle_neighbors`, and polygon rows after `num_center`
/// expose `triangles_on_cell[cell][..edge_counts[cell]]`.
pub fn refine_iter_d_judge_one_based(
    set_dis_in: usize,
    num_vertex: usize,
    sjx_points: usize,
    num_center: usize,
    lbx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<Vec<i32>> {
    if sjx_points >= mrl_new.len()
        || sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sjx_points {sjx_points} must be addressable in all triangle arrays"),
        ));
    }
    if lbx_points >= triangles_on_cell.len() || lbx_points >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("lbx_points {lbx_points} must be addressable in all cell arrays"),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }

    let mut ref_sjx = vec![0_i32; sjx_points + 1];
    if set_dis_in == 1 {
        return Ok(ref_sjx);
    }
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:iterD_judge",
        ));
    }

    validate_triangle_neighbor_rows(num_vertex, sjx_points, triangle_neighbors)?;
    for cell in (num_center + 1)..=lbx_points {
        validate_refine_cell_neighbors(cell, triangles_on_cell, edge_counts, sjx_points, None)?;
    }

    let closed_curves = refine_boundary_closed_curves_one_based(
        num_vertex,
        sjx_points,
        num_center,
        lbx_points,
        triangle_neighbors,
        cells_on_triangle,
        mrl_new,
    )?;
    let bdy_refine_segments = refine_boundary_segments_one_based(
        set_dis_in,
        &closed_curves,
        triangles_on_cell,
        edge_counts,
        mrl_new,
    )?;

    let num_bdy_refine_segment = bdy_refine_segments.len();
    if num_bdy_refine_segment == 0 {
        return Ok(ref_sjx);
    }

    for i in 0..num_bdy_refine_segment {
        let j = (i + 1) % num_bdy_refine_segment;
        let segment_i = &bdy_refine_segments[i];
        let segment_j = &bdy_refine_segments[j];
        if segment_i.is_empty() || segment_j.is_empty() {
            continue;
        }
        let m1 = *segment_i.last().expect("non-empty segment");
        let m2 = segment_j[0];
        if is_ngrmm(cells_on_triangle[m1], cells_on_triangle[m2]).is_none() {
            continue;
        }
        let num_max = segment_i.len().max(segment_j.len());
        let num_min = segment_i.len().min(segment_j.len());
        if num_min == 1 && num_max > 1 {
            ref_sjx[m1] = 1;
            ref_sjx[m2] = 1;
        }
    }

    Ok(ref_sjx)
}
