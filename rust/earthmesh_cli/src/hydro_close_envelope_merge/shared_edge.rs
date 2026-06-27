use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::*;

pub(super) fn merge_shared_edge_polygon_close_masks(
    left: &[(f64, f64)],
    right: &[(f64, f64)],
) -> Option<Vec<(f64, f64)>> {
    let mut edges = Vec::<((f64, f64), (f64, f64))>::new();
    let mut removed_interior_edge = false;
    for edge in split_polygon_ring_edges_for_union(left, right)? {
        if edge_midpoint_is_strictly_inside_ring(edge, right) {
            removed_interior_edge = true;
        } else {
            edges.push(edge);
        }
    }
    for edge in split_polygon_ring_edges_for_union(right, left)? {
        if edge_midpoint_is_strictly_inside_ring(edge, left) {
            removed_interior_edge = true;
        } else {
            edges.push(edge);
        }
    }
    let mut index = 0_usize;
    let mut removed_shared_edge = false;
    while index < edges.len() {
        if let Some(reverse_index) = edges.iter().enumerate().find_map(|(candidate, edge)| {
            (candidate != index
                && points_equal(edges[index].0, edge.1)
                && points_equal(edges[index].1, edge.0))
            .then_some(candidate)
        }) {
            let high = index.max(reverse_index);
            let low = index.min(reverse_index);
            edges.remove(high);
            edges.remove(low);
            removed_shared_edge = true;
            index = 0;
        } else {
            index += 1;
        }
    }
    if !removed_shared_edge && !removed_interior_edge {
        return None;
    }
    let merged = trace_polygon_boundary(edges).map(remove_collinear_boundary_vertices)?;
    if removed_shared_edge && !removed_interior_edge {
        let input_area = ring_area(left) + ring_area(right);
        if ring_area(&merged) + 1.0e-9 < input_area {
            return None;
        }
    }
    Some(merged)
}

fn split_polygon_ring_edges_for_union(
    coordinates: &[(f64, f64)],
    other: &[(f64, f64)],
) -> Option<Vec<((f64, f64), (f64, f64))>> {
    if coordinates.len() < 3
        || !coordinates
            .iter()
            .all(|(lon, lat)| lon.is_finite() && lat.is_finite())
        || other.len() < 3
        || !other
            .iter()
            .all(|(lon, lat)| lon.is_finite() && lat.is_finite())
    {
        return None;
    }
    let other_edges = polygon_ring_edges(other)?;
    let mut edges = Vec::<((f64, f64), (f64, f64))>::new();
    for index in 0..coordinates.len() {
        let start = coordinates[index];
        let end = coordinates[(index + 1) % coordinates.len()];
        if points_equal(start, end) {
            return None;
        }
        let mut edge_points = vec![start, end];
        edge_points.extend(
            other
                .iter()
                .copied()
                .filter(|point| !points_equal(*point, start) && !points_equal(*point, end))
                .filter(|point| point_on_segment(*point, start, end)),
        );
        for (other_start, other_end) in &other_edges {
            if let Some(point) = segment_intersection_point(start, end, *other_start, *other_end) {
                edge_points.push(point);
            }
        }
        edge_points.sort_by(|left, right| {
            segment_parameter(*left, start, end)
                .partial_cmp(&segment_parameter(*right, start, end))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        edge_points.dedup_by(|left, right| points_equal(*left, *right));
        for pair in edge_points.windows(2) {
            if !points_equal(pair[0], pair[1]) {
                edges.push((pair[0], pair[1]));
            }
        }
    }
    Some(edges)
}
