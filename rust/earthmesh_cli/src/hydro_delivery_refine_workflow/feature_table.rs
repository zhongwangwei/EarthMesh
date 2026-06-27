use std::collections::BTreeMap;
use std::io;

use earthmesh_geometry::Point;

use crate::*;

pub fn hydro_refine_feature_table(
    geojson: &str,
) -> io::Result<earthmesh_refine_planner::CellFeatureTable> {
    let root = JsonParser::new(geojson).parse()?;
    let mut hydro = Vec::new();
    let mut river = Vec::new();
    let mut coast = Vec::new();
    let mut centroids = Vec::new();
    for feature in geojson_feature_nodes(&root) {
        let obj = feature.as_object();
        let props = obj
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object());
        let prop_f64 = |k: &str| props.and_then(|p| p.get(k)).and_then(|v| v.as_f64());
        let rf = prop_f64("river_fraction").unwrap_or(0.0);
        let cf = prop_f64("coastal_fraction").unwrap_or(0.0);
        river.push(rf);
        coast.push(cf);
        hydro.push(rf.max(cf));
        let centroid = match (prop_f64("center_lon"), prop_f64("center_lat")) {
            (Some(x), Some(y)) => Point::new(x, y),
            _ => obj
                .and_then(|o| o.get("geometry"))
                .map(geometry_outer_rings)
                .and_then(|rings| rings.into_iter().find(|r| !r.is_empty()))
                .map(|ring| {
                    let n = ring.len() as f64;
                    let (sx, sy) = ring
                        .iter()
                        .fold((0.0, 0.0), |(ax, ay), p| (ax + p.x, ay + p.y));
                    Point::new(sx / n, sy / n)
                })
                .unwrap_or(Point::new(0.0, 0.0)),
        };
        centroids.push(centroid);
    }
    let cell_count = hydro.len();
    let mut columns = BTreeMap::new();
    columns.insert("hydro_coast_score".to_string(), hydro);
    columns.insert("river_fraction".to_string(), river);
    columns.insert("coastal_fraction".to_string(), coast);
    Ok(earthmesh_refine_planner::CellFeatureTable {
        cell_count,
        centroids,
        columns,
        neighbors: Vec::new(),
        regions: Vec::new(),
    })
}
