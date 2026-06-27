use super::*;

/// Production-facing `GetArea` output with the diagnostic summary printed by
/// the Fortran routine.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAreaProductionOutput {
    pub unit: GetAreaUnitOutput,
    pub reconstruction_error: AreaTriangleReconstructionError,
}

/// Production wrapper for `MOD_grid_preprocess:GetArea`.
///
/// This combines the migrated unit-sphere area workflow with the reconstruction
/// relative-error diagnostic that the Fortran routine prints after computing
/// `areaTriangle`.
pub fn get_area_production_fortran_indexed(
    input: GetAreaUnitInput<'_>,
) -> Option<GetAreaProductionOutput> {
    let unit = get_area_unit_fortran_indexed(input)?;
    let reconstruction_error = area_triangle_reconstruction_error_fortran_indexed(
        &unit.area_triangle,
        input.cell_points,
        input.cells_on_vertex,
    )?;

    Some(GetAreaProductionOutput {
        unit,
        reconstruction_error,
    })
}

/// Port of the `GetArea` area-triangle reconstruction error summary.
///
/// For each Fortran-indexed vertex id from `2..`, the routine recomputes the
/// triangle area from `cellsOnVertex(:, i)` cell centers and compares it with
/// the reconstructed `areaTriangle(i)`.
pub fn area_triangle_reconstruction_error_fortran_indexed(
    area_triangle: &[f64],
    cell_points: &[CartesianPoint],
    cells_on_vertex: &[[usize; 3]],
) -> Option<AreaTriangleReconstructionError> {
    if area_triangle.len() < 3 || cells_on_vertex.len() < area_triangle.len() {
        return None;
    }

    let mut max_relative = 0.0;
    let mut sum_relative = 0.0;
    let mut count = 0usize;

    for vertex_id in 2..area_triangle.len() {
        let cell_ids = cells_on_vertex[vertex_id];
        if cell_ids.contains(&0) {
            return None;
        }
        let exact = spherical_triangle_area_unit([
            *cell_points.get(cell_ids[0])?,
            *cell_points.get(cell_ids[1])?,
            *cell_points.get(cell_ids[2])?,
        ]);
        if exact == 0.0 {
            return None;
        }
        let relative = (area_triangle[vertex_id] - exact).abs() / exact;
        max_relative = f64::max(max_relative, relative);
        sum_relative += relative;
        count += 1;
    }

    Some(AreaTriangleReconstructionError {
        max_relative,
        avg_relative: sum_relative / count as f64,
    })
}
