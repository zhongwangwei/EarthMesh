use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::{
    geojson_feature_nodes, json_escape_string, read_text_maybe_gzip, JsonNode, JsonParser,
    HYDRO_EARTH_RADIUS_M,
};

use super::geometry::{
    bounds_overlap, geometry_outer_rings, is_convex, ring_bounds, LocalEqualArea, SphericalCap,
};
use super::json::json_node_to_string;

struct CorridorRing {
    ring: Vec<earthmesh_geometry::Point>,
    cap: SphericalCap,
    source: Option<String>,
    is_estuary: bool,
    reach_id: Option<String>,
}

fn feature_river_class(props: Option<&BTreeMap<String, JsonNode>>) -> String {
    props
        .and_then(|p| {
            p.get("river_class")
                .and_then(JsonNode::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| p.get("mask_class").and_then(JsonNode::as_str))
        })
        .unwrap_or("")
        .to_string()
}

fn feature_string_property(
    props: Option<&BTreeMap<String, JsonNode>>,
    name: &str,
) -> Option<String> {
    match props.and_then(|values| values.get(name)) {
        Some(JsonNode::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(JsonNode::Number(value)) => Some(crate::format_coupling_number(*value)),
        _ => None,
    }
}

fn feature_bool_property(props: Option<&BTreeMap<String, JsonNode>>, name: &str) -> bool {
    match props.and_then(|values| values.get(name)) {
        Some(JsonNode::Bool(value)) => *value,
        Some(JsonNode::String(value)) => value.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Conservative cell×corridor overlay on the sphere.
///
/// Each cell defines a Lambert azimuthal equal-area plane. Cell, corridor and optional
/// domain edges are densified as minor great-circle arcs before projection. Same-class
/// overlaps are dissolved in that plane, then normalized against the projected cell and
/// scaled by the cell's validated spherical area. This makes fractions conservative,
/// longitude-wrap independent and suitable for production coupling.
#[allow(clippy::too_many_arguments)]
pub fn write_earthmesh_intersection_geojson(
    cell_geojson: impl AsRef<Path>,
    corridor_geojson: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    include_classes: &[String],
    min_fraction: f64,
    unit_sphere_area: bool,
    domain: Option<&[Vec<(f64, f64)>]>,
) -> io::Result<usize> {
    use earthmesh_geometry::{
        clip_convex_polygon, polygon_area, polygon_intersection_pieces, polygon_union_area,
        try_spherical_polygon_excess, Point, SphericalAreaBranch,
    };
    let domain_rings: Option<Vec<Vec<Point>>> = domain.map(|polys| {
        polys
            .iter()
            .map(|ring| ring.iter().map(|&(x, y)| Point::new(x, y)).collect())
            .collect()
    });
    let validate_ring = |ring: &[Point], kind: &str| -> io::Result<f64> {
        try_spherical_polygon_excess(ring, SphericalAreaBranch::Minor).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid spherical {kind} polygon: {error}"),
            )
        })
    };
    if let Some(rings) = &domain_rings {
        for ring in rings {
            validate_ring(ring, "domain")?;
        }
    }
    if !(0.0..=1.0).contains(&min_fraction) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "min_fraction must be between 0 and 1",
        ));
    }
    let cells_root = JsonParser::new(&read_text_maybe_gzip(cell_geojson.as_ref())?).parse()?;
    let corridors_root =
        JsonParser::new(&read_text_maybe_gzip(corridor_geojson.as_ref())?).parse()?;
    let included: std::collections::BTreeSet<&str> =
        include_classes.iter().map(|s| s.as_str()).collect();

    let mut class_rings: BTreeMap<String, Vec<CorridorRing>> = BTreeMap::new();
    for feature in geojson_feature_nodes(&corridors_root) {
        let obj = feature.as_object();
        let corridor_props = obj
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let class = feature_river_class(corridor_props);
        if class.is_empty() || !included.contains(class.as_str()) {
            continue;
        }
        let source = feature_string_property(corridor_props, "source");
        let is_estuary = feature_bool_property(corridor_props, "is_estuary");
        let reach_id = feature_string_property(corridor_props, "reach_id");
        if let Some(geom) = obj.and_then(|o| o.get("geometry")) {
            for ring in geometry_outer_rings(geom) {
                if ring.len() < 3 {
                    continue;
                }
                validate_ring(&ring, "corridor")?;
                let cap =
                    SphericalCap::for_rings(std::slice::from_ref(&ring)).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "corridor has no spherical center",
                        )
                    })?;
                class_rings
                    .entry(class.clone())
                    .or_default()
                    .push(CorridorRing {
                        ring,
                        cap,
                        source: source.clone(),
                        is_estuary,
                        reach_id: reach_id.clone(),
                    });
            }
        }
    }

    let mut features = Vec::new();
    for (cell_index, cell) in geojson_feature_nodes(&cells_root).into_iter().enumerate() {
        let cell_obj = cell.as_object();
        let geom = cell_obj.and_then(|o| o.get("geometry")).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell feature {} has no geometry", cell_index + 1),
            )
        })?;
        let cell_rings = geometry_outer_rings(geom);
        if cell_rings.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell feature {} has no polygon ring", cell_index + 1),
            ));
        }
        for ring in &cell_rings {
            validate_ring(ring, "cell")?;
        }
        let projection = LocalEqualArea::for_rings(&cell_rings).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell feature {} has no equal-area center", cell_index + 1),
            )
        })?;
        let cell_cap = SphericalCap::for_rings(&cell_rings).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell feature {} has no spherical cap", cell_index + 1),
            )
        })?;
        let projected_cells = cell_rings
            .iter()
            .map(|ring| {
                projection.project_ring(ring).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "cell cannot be represented in its local equal-area hemisphere",
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let cell_bounds = projected_cells.iter().map(|r| ring_bounds(r)).fold(
            (
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ),
            |(min_x, max_x, min_y, max_y), b| {
                (
                    min_x.min(b.0),
                    max_x.max(b.1),
                    min_y.min(b.2),
                    max_y.max(b.3),
                )
            },
        );
        let projected_cell_area: f64 = projected_cells.iter().map(|r| polygon_area(r)).sum();
        let cell_area_sr = cell_rings
            .iter()
            .map(|ring| validate_ring(ring, "cell"))
            .sum::<io::Result<f64>>()?;
        if projected_cell_area <= 0.0 || cell_area_sr <= 0.0 {
            continue;
        }
        let projected_domains = domain_rings
            .as_ref()
            .map(|rings| {
                rings
                    .iter()
                    .map(|ring| {
                        projection.project_ring(ring).ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "domain cannot be represented in the cell equal-area hemisphere",
                            )
                        })
                    })
                    .collect::<io::Result<Vec<_>>>()
            })
            .transpose()?;
        let cell_props = cell_obj
            .and_then(|o| o.get("properties"))
            .and_then(JsonNode::as_object);
        let cell_id = cell_props
            .and_then(|p| p.get("cell_id"))
            .map(|n| match n {
                JsonNode::String(s) => s.clone(),
                other => json_node_to_string(other),
            })
            .unwrap_or_default();
        let source_area = cell_props
            .and_then(|p| p.get("source_areaCell"))
            .and_then(JsonNode::as_f64);

        for (class, rings) in &class_rings {
            let mut clipped: Vec<Vec<earthmesh_geometry::Point>> = Vec::new();
            let mut estuary_clipped: Vec<Vec<earthmesh_geometry::Point>> = Vec::new();
            let mut corridor_sources = std::collections::BTreeSet::new();
            let mut reach_ids = std::collections::BTreeSet::new();
            for corridor in rings {
                if !cell_cap.overlaps(corridor.cap) {
                    continue;
                }
                let projected_corridor =
                    projection.project_ring(&corridor.ring).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "corridor cannot be represented in the cell equal-area hemisphere",
                        )
                    })?;
                let corridor_bounds = ring_bounds(&projected_corridor);
                if !bounds_overlap(cell_bounds, corridor_bounds) {
                    continue;
                }
                let mut corridor_clipped = Vec::new();
                for cr in &projected_cells {
                    let piece = clip_convex_polygon(&projected_corridor, cr);
                    if piece.len() >= 3 {
                        if let Some(domains) = &projected_domains {
                            for domain in domains {
                                if is_convex(domain) {
                                    let domain_piece = clip_convex_polygon(&piece, domain);
                                    if domain_piece.len() >= 3 {
                                        corridor_clipped.push(domain_piece);
                                    }
                                } else {
                                    corridor_clipped
                                        .extend(polygon_intersection_pieces(&piece, domain));
                                }
                            }
                        } else {
                            corridor_clipped.push(piece);
                        }
                    }
                }
                if polygon_union_area(&corridor_clipped) <= 0.0 {
                    continue;
                }
                if let Some(source) = &corridor.source {
                    corridor_sources.insert(source.clone());
                }
                if let Some(reach_id) = &corridor.reach_id {
                    reach_ids.insert(reach_id.clone());
                }
                if corridor.is_estuary {
                    estuary_clipped.extend(corridor_clipped.iter().cloned());
                }
                clipped.extend(corridor_clipped);
            }
            let projected_intersection = polygon_union_area(&clipped).min(projected_cell_area);
            if projected_intersection <= 0.0 {
                continue;
            }
            let fraction = (projected_intersection / projected_cell_area).clamp(0.0, 1.0);
            if fraction < min_fraction {
                continue;
            }
            let intersection_area_sr = cell_area_sr * fraction;
            let cell_area_m2 = cell_area_sr * HYDRO_EARTH_RADIUS_M * HYDRO_EARTH_RADIUS_M;
            let intersection_area_m2 = cell_area_m2 * fraction;
            let mut props: BTreeMap<String, String> = BTreeMap::new();
            if let Some(cp) = cell_props {
                for (k, v) in cp {
                    props.insert(k.clone(), json_node_to_string(v));
                }
            }
            props.insert(
                "cell_id".into(),
                format!("\"{}\"", json_escape_string(&cell_id)),
            );
            props.insert("grid_kind".into(), "\"earthmesh_cell\"".into());
            props.insert(
                "corridor_source_geometry".into(),
                "\"earthmesh_spherical_cell_intersection\"".into(),
            );
            props.insert(
                "overlay_method".into(),
                "\"cell_local_lambert_azimuthal_equal_area\"".into(),
            );
            props.insert("overlay_max_geodesic_step_deg".into(), "0.1".into());
            props.insert("area_conservation".into(), "\"cell_normalized\"".into());
            props.insert("cell_area_sr".into(), format!("{cell_area_sr}"));
            props.insert(
                "intersection_area_sr".into(),
                format!("{intersection_area_sr}"),
            );
            props.insert("cell_area_m2".into(), format!("{cell_area_m2}"));
            props.insert(
                "intersection_area_m2".into(),
                format!("{intersection_area_m2}"),
            );
            // Keep the established CoLM field names populated from the production
            // spherical result regardless of legacy source-area normalization flags.
            props.insert("normalized_cell_area_m2".into(), format!("{cell_area_m2}"));
            props.insert(
                "estimated_river_area_m2".into(),
                format!("{intersection_area_m2}"),
            );
            props.insert(
                "area_normalization".into(),
                "\"spherical_equal_area_m2\"".into(),
            );
            props.insert(
                "overlap_class".into(),
                format!("\"{}\"", json_escape_string(class)),
            );
            props.insert("overlap_fraction".into(), format!("{fraction}"));
            props.insert(
                "domain_clip_applied".into(),
                if domain_rings.is_some() {
                    "true".into()
                } else {
                    "false".into()
                },
            );
            if class.to_ascii_uppercase().starts_with('R') {
                let estuary_fraction = (polygon_union_area(&estuary_clipped)
                    .min(projected_intersection)
                    / projected_cell_area)
                    .clamp(0.0, fraction);
                props.insert(
                    "river_class".into(),
                    format!("\"{}\"", json_escape_string(class)),
                );
                props.insert("river_fraction".into(), format!("{fraction}"));
                props.insert(
                    "corridor_sources".into(),
                    format!(
                        "\"{}\"",
                        json_escape_string(
                            &corridor_sources.into_iter().collect::<Vec<_>>().join(";")
                        )
                    ),
                );
                props.insert(
                    "is_estuary".into(),
                    if estuary_fraction > 0.0 {
                        "true".into()
                    } else {
                        "false".into()
                    },
                );
                props.insert("estuary_fraction".into(), format!("{estuary_fraction}"));
                props.insert(
                    "reach_ids".into(),
                    format!(
                        "\"{}\"",
                        json_escape_string(&reach_ids.into_iter().collect::<Vec<_>>().join(";"))
                    ),
                );
                if let Some(sa) = source_area {
                    props.insert(
                        "source_estimated_river_area".into(),
                        format!("{}", sa * fraction),
                    );
                    if unit_sphere_area {
                        let norm = sa * HYDRO_EARTH_RADIUS_M * HYDRO_EARTH_RADIUS_M;
                        props.insert(
                            "source_area_normalization".into(),
                            "\"unit_sphere_area_to_m2\"".into(),
                        );
                        props.insert("source_normalized_cell_area_m2".into(), format!("{norm}"));
                        props.insert(
                            "source_estimated_river_area_m2".into(),
                            format!("{}", norm * fraction),
                        );
                    }
                }
            } else {
                props.insert(
                    "mask_class".into(),
                    format!("\"{}\"", json_escape_string(class)),
                );
                if class == "COAST" || class.starts_with("COAST_") {
                    props.insert("coastal_fraction".into(), format!("{fraction}"));
                }
            }
            let body = props
                .iter()
                .map(|(k, v)| format!("\"{}\": {}", json_escape_string(k), v))
                .collect::<Vec<_>>()
                .join(", ");
            features.push(format!(
                "    {{\"type\": \"Feature\", \"geometry\": {}, \"properties\": {{{}}}}}",
                json_node_to_string(geom),
                body
            ));
        }
    }

    let out = format!(
        "{{\n  \"type\": \"FeatureCollection\",\n  \"features\": [\n{}\n  ]\n}}\n",
        features.join(",\n")
    );
    crate::ensure_parent_dir(output_geojson.as_ref())?;
    fs::write(output_geojson, out)?;
    Ok(features.len())
}
