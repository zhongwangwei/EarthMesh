use std::collections::BTreeMap;

use crate::hydro_close_hole_decomposition::{
    decompose_axis_aligned_rectangular_hole,
    decompose_non_axis_aligned_exterior_holes_vertical_slabs,
};
use crate::JsonNode;

pub(crate) use crate::hydro_close_buffer::{
    buffer_close_mask_line_for_refine_degree, buffer_close_mask_ring_for_refine_degree, ring_area,
    simplify_closed_ring,
};
pub(crate) use crate::hydro_close_geometry_utils::*;
pub(crate) use crate::hydro_close_proximity::is_close_mask_ring_too_close;

pub(crate) fn geojson_close_mask_rings(
    geometry: &BTreeMap<String, JsonNode>,
) -> Vec<Vec<(f64, f64)>> {
    let geometry_type = geometry
        .get("type")
        .and_then(JsonNode::as_str)
        .unwrap_or("");
    if geometry_type == "GeometryCollection" {
        return geometry
            .get("geometries")
            .and_then(JsonNode::as_array)
            .into_iter()
            .flat_map(|geometries| geometries.iter())
            .filter_map(JsonNode::as_object)
            .flat_map(geojson_close_mask_rings)
            .collect();
    }
    let Some(coordinates) = geometry.get("coordinates").and_then(JsonNode::as_array) else {
        return Vec::new();
    };
    match geometry_type {
        "Polygon" => geojson_polygon_close_mask_rings(coordinates),
        "MultiPolygon" => coordinates
            .iter()
            .filter_map(JsonNode::as_array)
            .flat_map(|rings| geojson_polygon_close_mask_rings(rings))
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn geojson_close_mask_lines(
    geometry: &BTreeMap<String, JsonNode>,
) -> Vec<Vec<(f64, f64)>> {
    let geometry_type = geometry
        .get("type")
        .and_then(JsonNode::as_str)
        .unwrap_or("");
    if geometry_type == "GeometryCollection" {
        return geometry
            .get("geometries")
            .and_then(JsonNode::as_array)
            .into_iter()
            .flat_map(|geometries| geometries.iter())
            .filter_map(JsonNode::as_object)
            .flat_map(geojson_close_mask_lines)
            .collect();
    }
    let Some(coordinates) = geometry.get("coordinates").and_then(JsonNode::as_array) else {
        return Vec::new();
    };
    match geometry_type {
        "LineString" => {
            let line = normalize_geojson_line(coordinates);
            if line.len() >= 2 {
                vec![line]
            } else {
                Vec::new()
            }
        }
        "MultiLineString" => coordinates
            .iter()
            .filter_map(JsonNode::as_array)
            .map(|line| normalize_geojson_line(line))
            .filter(|line| line.len() >= 2)
            .collect(),
        _ => Vec::new(),
    }
}

fn geojson_polygon_close_mask_rings(polygon_rings: &[JsonNode]) -> Vec<Vec<(f64, f64)>> {
    let mut rings = polygon_rings
        .iter()
        .filter_map(JsonNode::as_array)
        .map(|ring| normalize_geojson_ring(ring))
        .filter(|ring| ring.len() >= 3)
        .collect::<Vec<_>>();
    if rings.is_empty() {
        return Vec::new();
    }
    let exterior = rings.remove(0);
    if rings.is_empty() {
        return vec![exterior];
    }
    decompose_axis_aligned_rectangular_hole(&exterior, &rings)
        .or_else(|| decompose_non_axis_aligned_exterior_holes_vertical_slabs(&exterior, &rings))
        .unwrap_or_else(|| vec![exterior])
}

fn normalize_geojson_ring(ring: &[JsonNode]) -> Vec<(f64, f64)> {
    let mut coordinates = Vec::new();
    for point in ring {
        let Some(point) = point.as_array() else {
            continue;
        };
        let (Some(lon), Some(lat)) = (
            point.first().and_then(JsonNode::as_f64),
            point.get(1).and_then(JsonNode::as_f64),
        ) else {
            continue;
        };
        coordinates.push((lon, lat));
    }
    if coordinates.len() > 1 && coordinates.first() == coordinates.last() {
        coordinates.pop();
    }
    coordinates
}

fn normalize_geojson_line(line: &[JsonNode]) -> Vec<(f64, f64)> {
    let mut coordinates = Vec::new();
    for point in line {
        let Some(point) = point.as_array() else {
            continue;
        };
        let (Some(lon), Some(lat)) = (
            point.first().and_then(JsonNode::as_f64),
            point.get(1).and_then(JsonNode::as_f64),
        ) else {
            continue;
        };
        if coordinates
            .last()
            .copied()
            .is_some_and(|existing| points_equal(existing, (lon, lat)))
        {
            continue;
        }
        coordinates.push((lon, lat));
    }
    if coordinates.len() > 1
        && coordinates
            .first()
            .zip(coordinates.last())
            .is_some_and(|(first, last)| points_equal(*first, *last))
    {
        coordinates.pop();
    }
    coordinates
}
