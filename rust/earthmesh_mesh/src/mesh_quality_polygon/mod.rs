use crate::{polygon_length_angle_metrics, LonLatDegrees};

/// Canonical-style compact cache/update output for `MOD_grid_preprocess:PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonMeshQualityCanonicalOutput {
    pub length_cache: Vec<Vec<f64>>,
    pub angle_cache: Vec<Vec<f64>>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

/// Cache-aware port of `MOD_grid_preprocess:PolyMeshQuality`.
///
/// Canonical iterates over cell ids from `2`, skips cells whose `n_ngrwm` does not
/// match `num_edges`, and stores quality caches in a compact `j` index for only
/// the matching cells. This Rust port preserves that compact-cache contract.
pub fn polygon_mesh_quality_metrics_indexed(
    num_edges: usize,
    cell_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
    adjust_flags: &[bool],
    length_cache: &[Vec<f64>],
    angle_cache: &[Vec<f64>],
) -> Option<PolygonMeshQualityCanonicalOutput> {
    let len = cells_on_polygon.len();
    if num_edges < 3 || len < 3 || polygon_edge_counts.len() != len || adjust_flags.len() != len {
        return None;
    }

    let matching_count = (2..len)
        .filter(|&cell_id| polygon_edge_counts[cell_id] == num_edges)
        .count();
    if matching_count == 0
        || length_cache.len() != matching_count
        || angle_cache.len() != matching_count
        || length_cache.iter().any(|row| row.len() != num_edges)
        || angle_cache.iter().any(|row| row.len() != num_edges)
    {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    let angle_regularless = regular_angle * 0.9;
    let angle_regularmore = regular_angle * 1.1;
    let mut updated_lengths = length_cache.to_vec();
    let mut updated_angles = angle_cache.to_vec();
    let mut angle_less_flags = vec![false; matching_count];
    let mut angle_more_flags = vec![false; matching_count];
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut compact_id = 0usize;

    for cell_id in 2..len {
        if polygon_edge_counts[cell_id] != num_edges {
            continue;
        }

        if adjust_flags[cell_id] {
            let polygon_indices = cells_on_polygon.get(cell_id)?;
            if polygon_indices.len() < num_edges {
                return None;
            }
            let mut polygon = Vec::with_capacity(num_edges);
            for &point_id in polygon_indices.iter().take(num_edges) {
                polygon.push(*cell_points.get(point_id)?);
            }
            let metrics = polygon_length_angle_metrics(&polygon)?;
            updated_angles[compact_id] = metrics.angles_degrees;
            updated_lengths[compact_id] = metrics.edge_lengths_meters;
        }

        let angles = &updated_angles[compact_id];
        let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum_min += min_angle;
        sum_max += max_angle;
        sum_squared += angles
            .iter()
            .map(|angle| (angle - regular_angle).powi(2))
            .sum::<f64>();
        global_min = global_min.min(min_angle);
        global_max = global_max.max(max_angle);
        angle_less_flags[compact_id] = min_angle < angle_regularless;
        angle_more_flags[compact_id] = max_angle > angle_regularmore;
        compact_id += 1;
    }

    Some(PolygonMeshQualityCanonicalOutput {
        length_cache: updated_lengths,
        angle_cache: updated_angles,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (
            sum_min / matching_count as f64,
            sum_max / matching_count as f64,
        ),
        angle_stddev_degrees: (sum_squared / (num_edges * matching_count) as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}
