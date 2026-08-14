use std::io;
use std::path::Path;

use crate::{geojson_feature_nodes, read_text_maybe_gzip, JsonNode, JsonParser};

use earthmesh_geometry::Point;

pub(crate) use earthmesh_boundary::{
    bounds_overlap, is_convex, ring_bounds, LocalEqualArea, SphericalCap,
};

/// Outer ring(s) of a Polygon / MultiPolygon geometry node as lon/lat point lists.
/// Holes are ignored (the hydro masks are simple polygons).
/// Read all Polygon/MultiPolygon outer rings from a GeoJSON file (e.g. an analysis
/// domain) as lon/lat tuple rings. Used to feed an arbitrary-polygon domain into the
/// intersection writer.
pub fn read_polygon_outer_rings(geojson: impl AsRef<Path>) -> io::Result<Vec<Vec<(f64, f64)>>> {
    let root = JsonParser::new(&read_text_maybe_gzip(geojson.as_ref())?).parse()?;
    let mut rings = Vec::new();
    for feature in geojson_feature_nodes(&root) {
        if let Some(geom) = feature.as_object().and_then(|o| o.get("geometry")) {
            for ring in geometry_outer_rings(geom) {
                if ring.len() >= 3 {
                    rings.push(ring.iter().map(|p| (p.x, p.y)).collect());
                }
            }
        }
    }
    Ok(rings)
}

pub(crate) fn geometry_outer_rings(geometry: &JsonNode) -> Vec<Vec<earthmesh_geometry::Point>> {
    let obj = geometry.as_object();
    let gtype = obj
        .and_then(|o| o.get("type"))
        .and_then(JsonNode::as_str)
        .unwrap_or("");
    let coords = obj
        .and_then(|o| o.get("coordinates"))
        .and_then(JsonNode::as_array);
    let ring_points = |ring: &JsonNode| -> Vec<Point> {
        ring.as_array()
            .map(|pts| {
                pts.iter()
                    .filter_map(|p| {
                        let a = p.as_array()?;
                        Some(Point::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    match gtype {
        "Polygon" => coords
            .and_then(|c| c.first())
            .map(|r| vec![ring_points(r)])
            .unwrap_or_default(),
        "MultiPolygon" => coords
            .map(|polys| {
                polys
                    .iter()
                    .filter_map(|poly| poly.as_array().and_then(|p| p.first()).map(ring_points))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod equal_area_tests {
    use super::*;
    use earthmesh_geometry::{intersection_area, Point};

    fn ring(west: f64, east: f64, south: f64, north: f64) -> Vec<Point> {
        vec![
            Point { x: west, y: south },
            Point { x: east, y: south },
            Point { x: east, y: north },
            Point { x: west, y: north },
        ]
    }

    /// At high latitude a lon/lat area reading overstates the poleward half.
    ///
    /// The complete-mask writer compares areas *between* surface classes to pick
    /// a winner, so a distortion that varies across the cell tilts the vote. It
    /// does vary: a degree of longitude is shorter the further poleward it sits.
    ///
    /// Measured here on the cell the review used -- 80 to 89 north, mask over
    /// its poleward half. On the sphere that half is 0.296 of the cell; read as
    /// a plane it is 0.500, an overstatement of about 69%.
    #[test]
    fn a_high_latitude_half_is_not_half_its_cell() {
        let cell = ring(0.0, 10.0, 80.0, 89.0);
        let poleward = ring(0.0, 10.0, 84.5, 89.0);

        let planar = intersection_area(&cell, &poleward) / earthmesh_geometry::polygon_area(&cell);
        assert!(
            (planar - 0.5).abs() < 1.0e-9,
            "the lon/lat reading is exactly half: {planar}"
        );

        let projection = LocalEqualArea::for_rings(std::slice::from_ref(&cell)).expect("centre");
        let cell_flat = projection.project_ring(&cell).expect("cell");
        let poleward_flat = projection.project_ring(&poleward).expect("mask");
        let equal_area = intersection_area(&cell_flat, &poleward_flat)
            / earthmesh_geometry::polygon_area(&cell_flat);

        // sin(89) - sin(84.5) over sin(89) - sin(80) is 0.2958...
        assert!(
            (equal_area - 0.296).abs() < 0.01,
            "the equal-area reading must track the spherical fraction: {equal_area}"
        );
        assert!(
            planar > equal_area * 1.5,
            "and the planar one overstates it by more than half again: {planar} vs {equal_area}"
        );
    }

    /// Near the equator the two readings agree, so the projection is not a
    /// blanket correction applied where nothing was wrong.
    #[test]
    fn near_the_equator_the_two_readings_agree() {
        let cell = ring(0.0, 10.0, -5.0, 5.0);
        let upper = ring(0.0, 10.0, 0.0, 5.0);

        let planar = intersection_area(&cell, &upper) / earthmesh_geometry::polygon_area(&cell);
        let projection = LocalEqualArea::for_rings(std::slice::from_ref(&cell)).expect("centre");
        let cell_flat = projection.project_ring(&cell).expect("cell");
        let upper_flat = projection.project_ring(&upper).expect("mask");
        let equal_area = intersection_area(&cell_flat, &upper_flat)
            / earthmesh_geometry::polygon_area(&cell_flat);

        assert!(
            (planar - equal_area).abs() < 0.01,
            "{planar} vs {equal_area}"
        );
    }
}
