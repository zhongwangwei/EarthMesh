use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

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

#[derive(Clone, Copy)]
enum SameClassOverlap {
    Possible,
    Disjoint,
}

#[derive(Clone, Copy)]
struct LonLat(earthmesh_geometry::Point);

impl<'de> Deserialize<'de> for LonLat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LonLatVisitor;

        impl<'de> Visitor<'de> for LonLatVisitor {
            type Value = LonLat;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a GeoJSON coordinate with at least longitude and latitude")
            }

            fn visit_seq<A>(self, mut values: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let lon = values
                    .next_element::<f64>()?
                    .ok_or_else(|| serde::de::Error::custom("missing longitude"))?;
                let lat = values
                    .next_element::<f64>()?
                    .ok_or_else(|| serde::de::Error::custom("missing latitude"))?;
                while values.next_element::<IgnoredAny>()?.is_some() {}
                Ok(LonLat(earthmesh_geometry::Point::new(lon, lat)))
            }
        }

        deserializer.deserialize_seq(LonLatVisitor)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TextProperty {
    Text(String),
    Number(f64),
}

impl TextProperty {
    fn into_nonempty_string(self) -> Option<String> {
        match self {
            Self::Text(value) => (!value.is_empty()).then_some(value),
            Self::Number(value) => Some(crate::format_coupling_number(value)),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BoolProperty {
    Bool(bool),
    Text(String),
}

impl BoolProperty {
    fn value(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Text(value) => value.eq_ignore_ascii_case("true"),
        }
    }
}

#[derive(Default, Deserialize)]
struct CorridorProperties {
    #[serde(default)]
    river_class: String,
    #[serde(default)]
    mask_class: String,
    source: Option<TextProperty>,
    is_estuary: Option<BoolProperty>,
    reach_id: Option<TextProperty>,
}

impl CorridorProperties {
    fn class(&self) -> &str {
        if self.river_class.is_empty() {
            &self.mask_class
        } else {
            &self.river_class
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum CorridorGeometry {
    Polygon {
        coordinates: Vec<Vec<LonLat>>,
    },
    MultiPolygon {
        coordinates: Vec<Vec<Vec<LonLat>>>,
    },
    #[serde(other)]
    Unsupported,
}

impl CorridorGeometry {
    fn into_outer_rings(self) -> Vec<Vec<earthmesh_geometry::Point>> {
        let points = |ring: Vec<LonLat>| ring.into_iter().map(|point| point.0).collect();
        match self {
            Self::Polygon { coordinates } => coordinates
                .into_iter()
                .next()
                .map(|ring| vec![points(ring)])
                .unwrap_or_default(),
            Self::MultiPolygon { coordinates } => coordinates
                .into_iter()
                .filter_map(|polygon| polygon.into_iter().next())
                .map(points)
                .collect(),
            Self::Unsupported => Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct CorridorFeature {
    geometry: Option<CorridorGeometry>,
    #[serde(default)]
    properties: Option<CorridorProperties>,
}

#[derive(Deserialize)]
struct CorridorFeatureCollection {
    features: Vec<CorridorFeature>,
}

fn read_corridor_rings(
    path: &Path,
    included: &std::collections::BTreeSet<&str>,
    validate_ring: &impl Fn(&[earthmesh_geometry::Point], &str) -> io::Result<f64>,
) -> io::Result<BTreeMap<String, Vec<CorridorRing>>> {
    let text = read_text_maybe_gzip(path)?;
    let collection: CorridorFeatureCollection = serde_json::from_str(&text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid corridor GeoJSON {}: {error}", path.display()),
        )
    })?;
    let mut class_rings: BTreeMap<String, Vec<CorridorRing>> = BTreeMap::new();
    for feature in collection.features {
        let properties = feature.properties.unwrap_or_default();
        let class = properties.class().to_string();
        if class.is_empty() || !included.contains(class.as_str()) {
            continue;
        }
        let source = properties
            .source
            .and_then(TextProperty::into_nonempty_string);
        let is_estuary = properties
            .is_estuary
            .as_ref()
            .is_some_and(BoolProperty::value);
        let reach_id = properties
            .reach_id
            .and_then(TextProperty::into_nonempty_string);
        let Some(geometry) = feature.geometry else {
            continue;
        };
        for ring in geometry.into_outer_rings() {
            if ring.len() < 3 {
                continue;
            }
            validate_ring(&ring, "corridor")?;
            let cap = SphericalCap::for_rings(std::slice::from_ref(&ring)).ok_or_else(|| {
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
    Ok(class_rings)
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
    write_earthmesh_intersection_geojson_with_overlap(
        cell_geojson,
        corridor_geojson,
        output_geojson,
        include_classes,
        min_fraction,
        unit_sphere_area,
        domain,
        SameClassOverlap::Possible,
    )
}

/// Exact fast path for inputs whose same-class polygons have disjoint interiors,
/// such as the native MERIT-Hydro classification where every raster cell owns one
/// class. Shared boundaries have zero area, so summing clipped pieces is identical
/// to a polygon union and avoids quadratic edge-pair enumeration.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn write_disjoint_earthmesh_intersection_geojson(
    cell_geojson: impl AsRef<Path>,
    corridor_geojson: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    include_classes: &[String],
    min_fraction: f64,
    unit_sphere_area: bool,
    domain: Option<&[Vec<(f64, f64)>]>,
) -> io::Result<usize> {
    write_earthmesh_intersection_geojson_with_overlap(
        cell_geojson,
        corridor_geojson,
        output_geojson,
        include_classes,
        min_fraction,
        unit_sphere_area,
        domain,
        SameClassOverlap::Disjoint,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_earthmesh_intersection_geojson_with_overlap(
    cell_geojson: impl AsRef<Path>,
    corridor_geojson: impl AsRef<Path>,
    output_geojson: impl AsRef<Path>,
    include_classes: &[String],
    min_fraction: f64,
    unit_sphere_area: bool,
    domain: Option<&[Vec<(f64, f64)>]>,
    same_class_overlap: SameClassOverlap,
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
    let included: std::collections::BTreeSet<&str> =
        include_classes.iter().map(|s| s.as_str()).collect();
    let class_rings = read_corridor_rings(corridor_geojson.as_ref(), &included, &validate_ring)?;

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
            let mut disjoint_area = 0.0;
            let mut disjoint_estuary_area = 0.0;
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
                let corridor_area = match same_class_overlap {
                    SameClassOverlap::Possible => polygon_union_area(&corridor_clipped),
                    SameClassOverlap::Disjoint => corridor_clipped
                        .iter()
                        .map(|piece| polygon_area(piece))
                        .sum(),
                };
                if corridor_area <= 0.0 {
                    continue;
                }
                if let Some(source) = &corridor.source {
                    corridor_sources.insert(source.clone());
                }
                if let Some(reach_id) = &corridor.reach_id {
                    reach_ids.insert(reach_id.clone());
                }
                if corridor.is_estuary {
                    match same_class_overlap {
                        SameClassOverlap::Possible => {
                            estuary_clipped.extend(corridor_clipped.iter().cloned())
                        }
                        SameClassOverlap::Disjoint => disjoint_estuary_area += corridor_area,
                    }
                }
                match same_class_overlap {
                    SameClassOverlap::Possible => clipped.extend(corridor_clipped),
                    SameClassOverlap::Disjoint => disjoint_area += corridor_area,
                }
            }
            let projected_intersection = match same_class_overlap {
                SameClassOverlap::Possible => polygon_union_area(&clipped),
                SameClassOverlap::Disjoint => disjoint_area,
            }
            .min(projected_cell_area);
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
            props.insert(
                "same_class_overlap_handling".into(),
                match same_class_overlap {
                    SameClassOverlap::Possible => "\"polygon_union\"".into(),
                    SameClassOverlap::Disjoint => "\"disjoint_area_sum\"".into(),
                },
            );
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
                let estuary_area = match same_class_overlap {
                    SameClassOverlap::Possible => polygon_union_area(&estuary_clipped),
                    SameClassOverlap::Disjoint => disjoint_estuary_area,
                };
                let estuary_fraction = (estuary_area.min(projected_intersection)
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
