use std::collections::BTreeMap;
use std::io;

use earthmesh_geometry::Point;

use crate::{
    geojson_feature_nodes, geometry_outer_rings, json_node_to_string, JsonNode, JsonParser,
};

pub(crate) struct HydroCellFeatureGroup<'a> {
    pub cell_id: String,
    pub features: Vec<&'a JsonNode>,
}

#[derive(Debug)]
pub(crate) struct HydroRefineFeatureSet {
    pub table: earthmesh_refine_planner::CellFeatureTable,
}

fn feature_cell_id(feature: &JsonNode, feature_index: usize) -> String {
    feature
        .as_object()
        .and_then(|object| object.get("properties"))
        .and_then(JsonNode::as_object)
        .and_then(|properties| {
            properties
                .get("cell_id")
                .or_else(|| properties.get("cell_index"))
        })
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_f64().map(|_| json_node_to_string(value)))
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("feature:{feature_index}"))
}

/// Group class-specific intersection features by their canonical mesh cell.
/// First-occurrence order is stable and is the shared planner/adapter row order.
pub(crate) fn hydro_cell_feature_groups<'a>(
    root: &'a JsonNode,
) -> io::Result<Vec<HydroCellFeatureGroup<'a>>> {
    let mut groups = Vec::<HydroCellFeatureGroup<'a>>::new();
    let mut row_by_id = BTreeMap::<String, usize>::new();
    for (feature_index, feature) in geojson_feature_nodes(root).into_iter().enumerate() {
        let cell_id = feature_cell_id(feature, feature_index);
        if let Some(&row) = row_by_id.get(&cell_id) {
            let geometry = |value: &JsonNode| {
                value
                    .as_object()
                    .and_then(|object| object.get("geometry"))
                    .map(json_node_to_string)
                    .unwrap_or_else(|| "null".to_string())
            };
            if geometry(groups[row].features[0]) != geometry(feature) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "hydro intersection features for cell_id {cell_id} have different geometries"
                    ),
                ));
            }
            groups[row].features.push(feature);
        } else {
            let row = groups.len();
            row_by_id.insert(cell_id.clone(), row);
            groups.push(HydroCellFeatureGroup {
                cell_id,
                features: vec![feature],
            });
        }
    }
    Ok(groups)
}

fn feature_signal(feature: &JsonNode) -> (f64, f64, f64) {
    let props = feature
        .as_object()
        .and_then(|object| object.get("properties"))
        .and_then(JsonNode::as_object);
    let prop_f64 = |key: &str| props.and_then(|p| p.get(key)).and_then(JsonNode::as_f64);
    let river = prop_f64("river_fraction").unwrap_or(0.0);
    let coast = prop_f64("coastal_fraction").unwrap_or(0.0);
    // Area fraction alone makes every physically narrow river disappear on a
    // coarse mesh. Class is resolution demand; fraction remains coupling data.
    let class = props
        .and_then(|p| p.get("overlap_class"))
        .and_then(JsonNode::as_str)
        .or_else(|| {
            props
                .and_then(|p| p.get("river_class"))
                .and_then(JsonNode::as_str)
        })
        .or_else(|| {
            props
                .and_then(|p| p.get("mask_class"))
                .and_then(JsonNode::as_str)
        })
        .unwrap_or("")
        .to_ascii_uppercase();
    let class_demand: f64 = match class.as_str() {
        "R3" => 1.0,
        "R2" | "COAST" | "COAST_LAND" | "COAST_OCEAN" => 2.0 / 3.0,
        _ => 0.0,
    };
    (river.max(coast).max(class_demand), river, coast)
}

fn feature_centroid(feature: &JsonNode, feature_index: usize) -> io::Result<Point> {
    let object = feature.as_object();
    let props = object
        .and_then(|value| value.get("properties"))
        .and_then(JsonNode::as_object);
    match (
        props
            .and_then(|p| p.get("center_lon"))
            .and_then(JsonNode::as_f64),
        props
            .and_then(|p| p.get("center_lat"))
            .and_then(JsonNode::as_f64),
    ) {
        (Some(x), Some(y)) => Ok(Point::new(x, y)),
        _ => object
            .and_then(|value| value.get("geometry"))
            .map(geometry_outer_rings)
            .and_then(|rings| rings.into_iter().find(|ring| !ring.is_empty()))
            .map(|ring| {
                let points = if ring.len() > 1 && ring.first() == ring.last() {
                    &ring[..ring.len() - 1]
                } else {
                    ring.as_slice()
                };
                let n = points.len() as f64;
                let (x, y) = points
                    .iter()
                    .fold((0.0, 0.0), |(x, y), point| (x + point.x, y + point.y));
                Point::new(x / n, y / n)
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "hydro refine feature {} has neither center_lon/center_lat nor non-empty polygon geometry",
                        feature_index + 1
                    ),
                )
            }),
    }
}

pub(crate) fn hydro_refine_feature_set(geojson: &str) -> io::Result<HydroRefineFeatureSet> {
    let root = JsonParser::new(geojson).parse()?;
    let groups = hydro_cell_feature_groups(&root)?;
    let mut hydro = Vec::with_capacity(groups.len());
    let mut river = Vec::with_capacity(groups.len());
    let mut coast = Vec::with_capacity(groups.len());
    let mut centroids = Vec::with_capacity(groups.len());
    let mut cell_ids = Vec::with_capacity(groups.len());
    for (group_index, group) in groups.iter().enumerate() {
        let (mut hydro_score, mut river_fraction, mut coastal_fraction) =
            (0.0_f64, 0.0_f64, 0.0_f64);
        for feature in &group.features {
            let (feature_hydro, feature_river, feature_coast) = feature_signal(feature);
            hydro_score = hydro_score.max(feature_hydro);
            river_fraction = river_fraction.max(feature_river);
            coastal_fraction = coastal_fraction.max(feature_coast);
        }
        hydro.push(hydro_score);
        river.push(river_fraction);
        coast.push(coastal_fraction);
        centroids.push(feature_centroid(group.features[0], group_index)?);
        cell_ids.push(group.cell_id.clone());
    }
    let cell_count = groups.len();
    let mut columns = BTreeMap::new();
    columns.insert("hydro_coast_score".to_string(), hydro);
    columns.insert("river_fraction".to_string(), river);
    columns.insert("coastal_fraction".to_string(), coast);
    Ok(HydroRefineFeatureSet {
        table: earthmesh_refine_planner::CellFeatureTable {
            cell_count,
            cell_ids,
            centroids,
            columns,
            neighbors: vec![Vec::new(); cell_count],
            regions: Vec::new(),
        },
    })
}

pub fn hydro_refine_feature_table(
    geojson: &str,
) -> io::Result<earthmesh_refine_planner::CellFeatureTable> {
    Ok(hydro_refine_feature_set(geojson)?.table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_centroid_and_geometry_is_rejected() {
        let geojson = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"river_fraction":0.5},"geometry":null}]}"#;
        let err = hydro_refine_feature_table(geojson).unwrap_err();
        assert!(err.to_string().contains("neither center_lon/center_lat"));
    }

    #[test]
    fn neighbor_rows_match_cell_count_when_adjacency_is_unavailable() {
        let geojson = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"river_fraction":0.5,"coastal_fraction":0.2},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}]}"#;
        let table = hydro_refine_feature_table(geojson).unwrap();
        assert_eq!(table.cell_count, 1);
        assert_eq!(table.neighbors, vec![Vec::<usize>::new()]);
        assert_eq!(table.centroids, vec![Point::new(1.0, 1.0)]);
    }

    #[test]
    fn duplicate_class_rows_share_one_cell_budget_and_take_maximum_demand() {
        let geojson = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"cell_id":"42","overlap_class":"R2","river_fraction":0.4,"center_lon":1,"center_lat":1},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}},
          {"type":"Feature","properties":{"cell_id":"42","overlap_class":"R3","river_fraction":0.1,"center_lon":1,"center_lat":1},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]]}}
        ]}"#;
        let set = hydro_refine_feature_set(geojson).unwrap();
        assert_eq!(set.table.cell_count, 1);
        assert_eq!(set.table.cell_ids, vec!["42"]);
        assert_eq!(set.table.columns["river_fraction"], vec![0.4]);
        assert_eq!(set.table.columns["hydro_coast_score"], vec![1.0]);
    }

    #[test]
    fn duplicate_cell_id_with_different_geometry_is_rejected() {
        let geojson = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"cell_id":"42"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]]}},
          {"type":"Feature","properties":{"cell_id":"42"},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[2,1],[2,0]]]}}
        ]}"#;
        let error = hydro_refine_feature_set(geojson).unwrap_err();
        assert!(error.to_string().contains("different geometries"));
    }

    #[test]
    fn narrow_r3_is_not_rounded_out_of_the_refinement_plan() {
        let geojson = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"river_class":"R3","river_fraction":0.00001},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]]}}]}"#;
        let table = hydro_refine_feature_table(geojson).unwrap();
        assert_eq!(table.columns["river_fraction"], vec![0.00001]);
        assert_eq!(table.columns["hydro_coast_score"], vec![1.0]);
    }
}
