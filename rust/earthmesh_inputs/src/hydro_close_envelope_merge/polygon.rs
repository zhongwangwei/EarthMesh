use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::*;

pub(super) fn merge_contained_polygon_close_masks(
    left: &[(f64, f64)],
    right: &[(f64, f64)],
) -> Option<Vec<(f64, f64)>> {
    if let (Some(left_cells), Some(right_cells)) =
        (rectilinear_ring_cells(left), rectilinear_ring_cells(right))
    {
        if rectilinear_cells_contain_cells(&left_cells, &right_cells) {
            return Some(left.to_vec());
        }
        if rectilinear_cells_contain_cells(&right_cells, &left_cells) {
            return Some(right.to_vec());
        }
        return None;
    }
    if ring_contains_ring(left, right) {
        return Some(left.to_vec());
    }
    if ring_contains_ring(right, left) {
        return Some(right.to_vec());
    }
    None
}

pub(super) fn merge_convex_overlapping_close_masks(
    left: &[(f64, f64)],
    right: &[(f64, f64)],
) -> Option<Vec<(f64, f64)>> {
    if !is_convex_ring(left) || !is_convex_ring(right) {
        return None;
    }
    let intersection_area = convex_polygon_intersection_area(left, right)?;
    if intersection_area <= 1.0e-9 {
        return None;
    }
    let hull = convex_hull_points(left.iter().chain(right.iter()).copied())?;
    let hull_area = ring_area(&hull);
    let union_area = ring_area(left) + ring_area(right) - intersection_area;
    if (hull_area - union_area).abs() > 1.0e-8_f64.max(union_area * 1.0e-9) {
        return None;
    }
    Some(remove_collinear_boundary_vertices(hull))
}

pub(super) fn ring_has_all_bbox_corners_and_interior_vertices(coordinates: &[(f64, f64)]) -> bool {
    if coordinates.len() < 5 {
        return false;
    }
    let (min_lon, min_lat, max_lon, max_lat) = ring_bbox(coordinates);
    let corners = [
        (min_lon, min_lat),
        (max_lon, min_lat),
        (max_lon, max_lat),
        (min_lon, max_lat),
    ];
    corners.iter().all(|corner| {
        coordinates
            .iter()
            .any(|point| points_equal(*point, *corner))
    }) && coordinates.iter().any(|(lon, lat)| {
        *lon > min_lon + 1.0e-9
            && *lon < max_lon - 1.0e-9
            && *lat > min_lat + 1.0e-9
            && *lat < max_lat - 1.0e-9
    })
}
