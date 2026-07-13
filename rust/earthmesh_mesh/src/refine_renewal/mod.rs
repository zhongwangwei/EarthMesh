use super::*;

/// File-I/O-free port of `MOD_refine.F90:NGR_RENEW` including the final
/// `GetSortNew` adjacency ordering pass.
pub fn refine_ngr_renew_one_based(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points_new: &[LonLatDegrees],
    cell_points_new: &[LonLatDegrees],
    cells_on_triangle_new: &[[usize; 3]],
    boundary_refine: &[usize],
    boundary_refine_transition: &[usize],
) -> io::Result<RefineNgrRenewCore> {
    let mut renewed = refine_ngr_renew_core_one_based(
        iter,
        num_vertex,
        num_mp,
        num_wp,
        triangle_points_new,
        cell_points_new,
        cells_on_triangle_new,
        boundary_refine,
        boundary_refine_transition,
    )?;
    get_sort_new_one_based(
        renewed.num_dbx,
        &renewed.n_triangles_on_cell,
        &renewed.cells_on_triangle,
        &renewed.triangle_points,
        &mut renewed.triangles_on_cell,
    )?;
    Ok(renewed)
}
