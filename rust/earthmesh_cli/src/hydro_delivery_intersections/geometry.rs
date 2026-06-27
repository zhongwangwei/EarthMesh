use std::io;
use std::path::Path;

use crate::{geojson_feature_nodes, read_text_maybe_gzip, JsonNode, JsonParser};

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
    use earthmesh_geometry::Point;
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
