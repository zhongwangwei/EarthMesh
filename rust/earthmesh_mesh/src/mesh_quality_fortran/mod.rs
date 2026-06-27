use crate::{polygon_length_angle_metrics, LonLatDegrees};

/// Fortran-style cache/update output for `MOD_grid_preprocess:TriMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriangleMeshQualityFortranOutput {
    pub length_cache: Vec<[f64; 3]>,
    pub angle_cache: Vec<[f64; 3]>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

/// Cache-aware port of `MOD_grid_preprocess:TriMeshQuality`.
///
/// Inputs use the repository's Rust convention for migrated Fortran-indexed
/// arrays: slots `0` and `1` are placeholders and triangle ids start at `2`.
/// Adjusted triangles are recalculated from `cell_points`/`cells_on_triangle`;
/// unadjusted triangles reuse the provided angle/length caches.
pub fn triangle_mesh_quality_fortran_indexed(
    cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
    adjust_flags: &[bool],
    length_cache: &[[f64; 3]],
    angle_cache: &[[f64; 3]],
) -> Option<TriangleMeshQualityFortranOutput> {
    let len = cells_on_triangle.len();
    if len < 3 || adjust_flags.len() != len || length_cache.len() != len || angle_cache.len() != len
    {
        return None;
    }

    let mut updated_lengths = length_cache.to_vec();
    let mut updated_angles = angle_cache.to_vec();
    let mut angle_less_flags = vec![false; len];
    let mut angle_more_flags = vec![false; len];
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut count = 0usize;

    for triangle_id in 2..len {
        if adjust_flags[triangle_id] {
            let cell_ids = cells_on_triangle[triangle_id];
            let triangle = [
                *cell_points.get(cell_ids[0])?,
                *cell_points.get(cell_ids[1])?,
                *cell_points.get(cell_ids[2])?,
            ];
            let metrics = polygon_length_angle_metrics(&triangle)?;
            updated_angles[triangle_id] = [
                metrics.angles_degrees[0],
                metrics.angles_degrees[1],
                metrics.angles_degrees[2],
            ];
            updated_lengths[triangle_id] = [
                metrics.edge_lengths_meters[0],
                metrics.edge_lengths_meters[1],
                metrics.edge_lengths_meters[2],
            ];
        }

        let angles = updated_angles[triangle_id];
        let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum_min += min_angle;
        sum_max += max_angle;
        sum_squared += angles
            .iter()
            .map(|angle| (angle - 60.0).powi(2))
            .sum::<f64>();
        global_min = global_min.min(min_angle);
        global_max = global_max.max(max_angle);
        angle_less_flags[triangle_id] = min_angle < 45.0;
        angle_more_flags[triangle_id] = max_angle > 75.0;
        count += 1;
    }

    Some(TriangleMeshQualityFortranOutput {
        length_cache: updated_lengths,
        angle_cache: updated_angles,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / count as f64, sum_max / count as f64),
        angle_stddev_degrees: (sum_squared / (3 * count) as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}
