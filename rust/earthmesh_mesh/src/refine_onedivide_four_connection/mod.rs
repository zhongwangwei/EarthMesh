use std::io;

/// Port of `MOD_refine.F90:OnedivideFour_connection`.
///
/// Applies the refinement marker `ref_sjx` to the current refinement state:
/// each requested, still-unrefined triangle (`mrl_new == 1`) marks its three
/// parent polygon cells in `ref_lbx` and promotes the triangle state to `4`.
/// Inputs and mutable outputs preserve Fortran one-based indexing.
pub fn refine_onedivide_four_connection_fortran_indexed(
    num_vertex: usize,
    sjx_points: usize,
    cells_on_triangle: &[[usize; 3]],
    ref_sjx: &[i32],
    ref_lbx: &mut [i32],
    mrl_new: &mut [i32],
) -> io::Result<()> {
    if sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx.len()
        || sjx_points >= mrl_new.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sjx_points {sjx_points} must be addressable in all triangle arrays"),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }

    for triangle in (num_vertex + 1)..=sjx_points {
        if ref_sjx[triangle] == 0 || mrl_new[triangle] != 1 {
            continue;
        }
        for &cell in &cells_on_triangle[triangle] {
            if cell == 0 || cell >= ref_lbx.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid parent cell {cell}"),
                ));
            }
            ref_lbx[cell] = 1;
        }
        mrl_new[triangle] = 4;
    }

    Ok(())
}
