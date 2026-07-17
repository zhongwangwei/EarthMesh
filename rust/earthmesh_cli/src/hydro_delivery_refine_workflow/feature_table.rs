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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HydroRefinementPolicy {
    pub river_width: bool,
    pub river_upstream_area: bool,
    pub legacy_river_classes: bool,
    pub coast_land: bool,
    pub coast_ocean: bool,
}

impl Default for HydroRefinementPolicy {
    fn default() -> Self {
        Self {
            river_width: true,
            river_upstream_area: true,
            legacy_river_classes: true,
            coast_land: true,
            coast_ocean: true,
        }
    }
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

fn feature_signal(
    feature: &JsonNode,
    policy: HydroRefinementPolicy,
    secondary_demand: f64,
) -> (f64, f64, f64) {
    let props = feature
        .as_object()
        .and_then(|object| object.get("properties"))
        .and_then(JsonNode::as_object);
    let prop_f64 = |key: &str| props.and_then(|p| p.get(key)).and_then(JsonNode::as_f64);
    let prop_bool = |key: &str| props.and_then(|p| p.get(key)).and_then(JsonNode::as_bool);
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
    let width_triggered = prop_bool("river_width_triggered");
    let upstream_area_triggered = prop_bool("river_upstream_area_triggered");
    let has_explicit_river_criteria =
        width_triggered.is_some() || upstream_area_triggered.is_some();
    let selected_river_criterion = width_triggered == Some(true) && policy.river_width
        || upstream_area_triggered == Some(true) && policy.river_upstream_area;
    let legacy_river_enabled =
        policy.legacy_river_classes && (policy.river_width || policy.river_upstream_area);
    let (class_demand, explicit_river, explicit_coast) = match class.as_str() {
        "R3" => (
            if if has_explicit_river_criteria {
                selected_river_criterion
            } else {
                legacy_river_enabled
            } {
                1.0
            } else {
                0.0
            },
            true,
            false,
        ),
        "R2" => (
            if has_explicit_river_criteria && selected_river_criterion {
                1.0
            } else if !has_explicit_river_criteria && legacy_river_enabled {
                secondary_demand
            } else {
                0.0
            },
            true,
            false,
        ),
        "COAST" | "COAST_LAND" => (if policy.coast_land { 1.0 } else { 0.0 }, false, true),
        "COAST_OCEAN" => (if policy.coast_ocean { 1.0 } else { 0.0 }, false, true),
        "COAST_DISTANCE_LAND" | "C2_COAST_BUFFER_LAND" => {
            (if policy.coast_land { 1.0 } else { 0.0 }, false, true)
        }
        "COAST_DISTANCE_OCEAN" | "C2_COAST_BUFFER_OCEAN" => {
            (if policy.coast_ocean { 1.0 } else { 0.0 }, false, true)
        }
        _ => (0.0, false, false),
    };
    // Explicit classes own their level. Fractions remain coupling data and must
    // not accidentally promote a legacy secondary river class to the maximum.
    let river_demand = if explicit_river {
        0.0
    } else if policy.river_width || policy.river_upstream_area {
        river
    } else {
        0.0
    };
    let coast_demand = if explicit_coast {
        0.0
    } else if policy.coast_land || policy.coast_ocean {
        coast
    } else {
        0.0
    };
    (
        river_demand.max(coast_demand).max(class_demand),
        river,
        coast,
    )
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
    hydro_refine_feature_set_with_policy(geojson, HydroRefinementPolicy::default())
}

pub(crate) fn hydro_refine_feature_set_with_policy(
    geojson: &str,
    policy: HydroRefinementPolicy,
) -> io::Result<HydroRefineFeatureSet> {
    hydro_refine_feature_set_with_policy_and_secondary(geojson, policy, 2.0 / 3.0)
}

pub(crate) fn hydro_refine_feature_set_with_policy_and_secondary(
    geojson: &str,
    policy: HydroRefinementPolicy,
    secondary_demand: f64,
) -> io::Result<HydroRefineFeatureSet> {
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
            let (feature_hydro, feature_river, feature_coast) =
                feature_signal(feature, policy, secondary_demand);
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
    fn river_and_coast_refinement_demands_are_independent() {
        let geojson = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"cell_id":"river","mask_class":"R3","river_fraction":0.01},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
          {"type":"Feature","properties":{"cell_id":"coast","mask_class":"COAST_LAND","coastal_fraction":0.8},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[3,1],[2,1],[2,0]]]}}
        ]}"#;
        let river_only = hydro_refine_feature_set_with_policy(
            geojson,
            HydroRefinementPolicy {
                river_width: true,
                river_upstream_area: true,
                legacy_river_classes: true,
                coast_land: false,
                coast_ocean: false,
            },
        )
        .unwrap();
        assert_eq!(
            river_only.table.columns["hydro_coast_score"],
            vec![1.0, 0.0]
        );
        let coast_only = hydro_refine_feature_set_with_policy(
            geojson,
            HydroRefinementPolicy {
                river_width: false,
                river_upstream_area: false,
                legacy_river_classes: true,
                coast_land: true,
                coast_ocean: true,
            },
        )
        .unwrap();
        assert_eq!(
            coast_only.table.columns["hydro_coast_score"],
            vec![0.0, 1.0]
        );
    }

    #[test]
    fn river_width_and_upstream_area_criteria_are_independent_and_full_level() {
        let geojson = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"cell_id":"width","mask_class":"R3","river_width_triggered":true,"river_upstream_area_triggered":false,"center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
          {"type":"Feature","properties":{"cell_id":"upa","mask_class":"R3","river_width_triggered":false,"river_upstream_area_triggered":true,"center_lon":2,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[3,1],[2,1],[2,0]]]}}
        ]}"#;
        let width = hydro_refine_feature_set_with_policy(
            geojson,
            HydroRefinementPolicy {
                river_width: true,
                river_upstream_area: false,
                legacy_river_classes: false,
                coast_land: false,
                coast_ocean: false,
            },
        )
        .unwrap();
        assert_eq!(width.table.columns["hydro_coast_score"], vec![1.0, 0.0]);
        let upstream = hydro_refine_feature_set_with_policy(
            geojson,
            HydroRefinementPolicy {
                river_width: false,
                river_upstream_area: true,
                legacy_river_classes: false,
                coast_land: false,
                coast_ocean: false,
            },
        )
        .unwrap();
        assert_eq!(upstream.table.columns["hydro_coast_score"], vec![0.0, 1.0]);
    }

    #[test]
    fn legacy_r2_remains_secondary_and_coast_alias_is_full_level() {
        let geojson = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"cell_id":"river","mask_class":"R2","river_fraction":1.0,"center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
          {"type":"Feature","properties":{"cell_id":"coast","mask_class":"C2_COAST_BUFFER_LAND","coastal_fraction":1.0,"center_lon":2,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[3,1],[2,1],[2,0]]]}}
        ]}"#;
        let set = hydro_refine_feature_set_with_policy_and_secondary(
            geojson,
            HydroRefinementPolicy::default(),
            0.8,
        )
        .unwrap();
        assert_eq!(set.table.columns["hydro_coast_score"], vec![0.8, 1.0]);
    }

    #[test]
    fn disabled_coast_side_cannot_leak_through_fraction() {
        let geojson = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"ocean","mask_class":"COAST_OCEAN","coastal_fraction":1.0,"center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#;
        let set = hydro_refine_feature_set_with_policy(
            geojson,
            HydroRefinementPolicy {
                river_width: false,
                river_upstream_area: false,
                legacy_river_classes: true,
                coast_land: true,
                coast_ocean: false,
            },
        )
        .unwrap();
        assert_eq!(set.table.columns["hydro_coast_score"], vec![0.0]);
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
