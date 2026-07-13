use crate::{
    polygon_mesh_quality_metrics_indexed, triangle_mesh_quality_metrics_indexed, LonLatDegrees,
    PolygonMeshQualityCanonicalOutput, TriangleMeshQualityCanonicalOutput,
};

/// Polygon edge-count classes reported by
/// `MOD_grid_preprocess:Grid_Quality_Check_Global`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolygonEdgeClassCounts {
    pub pentagons: usize,
    pub hexagons: usize,
    pub heptagons: usize,
    pub less_than_five: usize,
    pub greater_than_seven: usize,
}

/// Quality summaries produced by the Rust orchestration wrapper for
/// `MOD_grid_preprocess:Grid_Quality_Check_Global`.
#[derive(Debug, Clone, PartialEq)]
pub struct GridQualityGlobalOutput {
    pub edge_class_counts: PolygonEdgeClassCounts,
    pub triangle: TriangleMeshQualityCanonicalOutput,
    pub pentagon: Option<PolygonMeshQualityCanonicalOutput>,
    pub hexagon: Option<PolygonMeshQualityCanonicalOutput>,
    pub heptagon: Option<PolygonMeshQualityCanonicalOutput>,
}

fn polygon_edge_class_counts_one_based(polygon_edge_counts: &[usize]) -> PolygonEdgeClassCounts {
    let mut counts = PolygonEdgeClassCounts {
        pentagons: 0,
        hexagons: 0,
        heptagons: 0,
        less_than_five: 0,
        greater_than_seven: 0,
    };

    for edge_count in polygon_edge_counts.iter().copied().skip(2) {
        match edge_count {
            5 => counts.pentagons += 1,
            6 => counts.hexagons += 1,
            7 => counts.heptagons += 1,
            count if count < 5 => counts.less_than_five += 1,
            _ => counts.greater_than_seven += 1,
        }
    }

    counts
}

fn polygon_quality_or_none_one_based(
    num_edges: usize,
    polygon_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
    adjust_flags: &[bool],
) -> Option<Option<PolygonMeshQualityCanonicalOutput>> {
    let matching_count = polygon_edge_counts
        .iter()
        .copied()
        .skip(2)
        .filter(|edge_count| *edge_count == num_edges)
        .count();

    if matching_count == 0 {
        return Some(None);
    }

    let length_cache = vec![vec![0.0; num_edges]; matching_count];
    let angle_cache = vec![vec![0.0; num_edges]; matching_count];
    polygon_mesh_quality_metrics_indexed(
        num_edges,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .map(Some)
}

/// Rust orchestration wrapper for `MOD_grid_preprocess:Grid_Quality_Check_Global`.
///
/// This ports the calculation side of the Canonical routine: polygon edge-class
/// counting, all-true initial adjust flags, triangle quality, and 5/6/7-sided
/// polygon quality groups. The NetCDF `quality_save_global` side effect remains
/// an adapter/output-layer responsibility.
pub fn grid_quality_check_global_one_based(
    triangle_cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
    polygon_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
) -> Option<GridQualityGlobalOutput> {
    if cells_on_polygon.len() != polygon_edge_counts.len() {
        return None;
    }

    let edge_class_counts = polygon_edge_class_counts_one_based(polygon_edge_counts);
    let triangle_adjust_flags = vec![true; cells_on_triangle.len()];
    let triangle_length_cache = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle_angle_cache = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle = triangle_mesh_quality_metrics_indexed(
        triangle_cell_points,
        cells_on_triangle,
        &triangle_adjust_flags,
        &triangle_length_cache,
        &triangle_angle_cache,
    )?;

    let polygon_adjust_flags = vec![true; cells_on_polygon.len()];
    let pentagon = polygon_quality_or_none_one_based(
        5,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;
    let hexagon = polygon_quality_or_none_one_based(
        6,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;
    let heptagon = polygon_quality_or_none_one_based(
        7,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;

    Some(GridQualityGlobalOutput {
        edge_class_counts,
        triangle,
        pentagon,
        hexagon,
        heptagon,
    })
}
