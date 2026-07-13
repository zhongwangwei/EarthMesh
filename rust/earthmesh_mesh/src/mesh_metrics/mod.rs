use super::*;

/// Output of `MOD_grid_preprocess:Get_Length_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonLengthAngleMetrics {
    pub angles_degrees: Vec<f64>,
    pub edge_lengths_meters: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Length_Angle`.
///
/// For each polygon vertex, this builds the same `(previous, current, next)`
/// triplet as the Canonical cyclic buffer, computes the spherical angle using the
/// half-angle formula, and records the current-to-next edge length scaled by
/// `erad8`.
pub fn polygon_length_angle_metrics(points: &[LonLatDegrees]) -> Option<PolygonLengthAngleMetrics> {
    let num_edges = points.len();
    if num_edges < 3 {
        return None;
    }

    let mut angles_degrees = Vec::with_capacity(num_edges);
    let mut edge_lengths_meters = Vec::with_capacity(num_edges);

    for i in 0..num_edges {
        let previous = points[(i + num_edges - 1) % num_edges];
        let current = points[i];
        let next = points[(i + 1) % num_edges];

        let previous_xyz = lonlat_degrees_to_unit_xyz(previous);
        let current_xyz = lonlat_degrees_to_unit_xyz(current);
        let next_xyz = lonlat_degrees_to_unit_xyz(next);

        let length1 = arc_length_unit_sphere(next_xyz, current_xyz);
        let length2 = arc_length_unit_sphere(next_xyz, previous_xyz);
        let length3 = arc_length_unit_sphere(previous_xyz, current_xyz);
        let semiperimeter = 0.5 * (length1 + length2 + length3);
        let denom = length1.sin() * length3.sin();
        let angle = if denom.abs() <= f64::EPSILON {
            0.0
        } else {
            let arg = ((semiperimeter - length1).sin() * (semiperimeter - length3).sin() / denom)
                .max(0.0)
                .sqrt()
                .clamp(0.0, 1.0);
            rad_to_deg(2.0 * arg.asin())
        };
        angles_degrees.push(angle);
        edge_lengths_meters.push(length1 * earthmesh_core::EARTH_RADIUS_METERS);
    }

    Some(PolygonLengthAngleMetrics {
        angles_degrees,
        edge_lengths_meters,
    })
}

/// Mesh-quality aggregate produced by Canonical `TriMeshQuality`/`PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualitySummary {
    pub cell_metrics: Vec<PolygonLengthAngleMetrics>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

fn polygon_quality_summary(
    cells: &[Vec<LonLatDegrees>],
    regular_angle_degrees: f64,
    lower_threshold_degrees: f64,
    upper_threshold_degrees: f64,
) -> Option<MeshQualitySummary> {
    if cells.is_empty() {
        return None;
    }

    let mut cell_metrics = Vec::with_capacity(cells.len());
    let mut angle_less_flags = Vec::with_capacity(cells.len());
    let mut angle_more_flags = Vec::with_capacity(cells.len());
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut angle_count = 0usize;

    for cell in cells {
        let metrics = polygon_length_angle_metrics(cell)?;
        let cell_min = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let cell_max = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        global_min = global_min.min(cell_min);
        global_max = global_max.max(cell_max);
        sum_min += cell_min;
        sum_max += cell_max;
        sum_squared += metrics
            .angles_degrees
            .iter()
            .map(|angle| (angle - regular_angle_degrees).powi(2))
            .sum::<f64>();
        angle_count += metrics.angles_degrees.len();
        angle_less_flags.push(cell_min < lower_threshold_degrees);
        angle_more_flags.push(cell_max > upper_threshold_degrees);
        cell_metrics.push(metrics);
    }

    let cell_count = cells.len() as f64;
    Some(MeshQualitySummary {
        cell_metrics,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / cell_count, sum_max / cell_count),
        angle_stddev_degrees: (sum_squared / angle_count as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Port of the aggregation core in `MOD_grid_preprocess:TriMeshQuality`.
pub fn triangle_mesh_quality(triangles: &[[LonLatDegrees; 3]]) -> Option<MeshQualitySummary> {
    let cells: Vec<Vec<LonLatDegrees>> =
        triangles.iter().map(|triangle| triangle.to_vec()).collect();
    polygon_quality_summary(&cells, 60.0, 45.0, 75.0)
}

/// Port of the aggregation core in `MOD_grid_preprocess:PolyMeshQuality`.
///
/// All cells in the input should have the same edge count, matching each
/// Canonical call for pentagons, hexagons, or heptagons. The regular angle is
/// `(num_edges - 2) * 180 / num_edges`, with 0.9/1.1 threshold bands.
pub fn polygon_mesh_quality(cells: &[Vec<LonLatDegrees>]) -> Option<MeshQualitySummary> {
    let first = cells.first()?;
    let num_edges = first.len();
    if num_edges < 3 || cells.iter().any(|cell| cell.len() != num_edges) {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    polygon_quality_summary(
        cells,
        regular_angle,
        regular_angle * 0.9,
        regular_angle * 1.1,
    )
}
