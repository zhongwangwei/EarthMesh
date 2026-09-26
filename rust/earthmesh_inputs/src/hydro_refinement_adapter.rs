//! Adapter from the per-cell hydro refinement plan to the production HField /
//! Method-C refinement pipeline.
//!
//! The first hydro pass is evaluated on a generated mesh. Its cell polygons
//! and target levels are converted to a gradient-limited `HField`, then the
//! normal refine pipeline is rerun from the realized parent mesh against the
//! union of original sources and new absolute targets.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_hfield::{great_circle_distance_m, HField, HRegion, EARTH_RADIUS_METERS};
use earthmesh_project::{content_addressed_stage_key, StageCache};

use crate::hfield_refine::HfieldDomainMask;
use crate::{
    geometry_outer_rings, hydro_cell_features::hydro_cell_feature_groups, json_node_to_string,
    json_node_to_usize, read_text_maybe_gzip, GridRegion, HfieldRefineOptions, JsonNode,
    JsonParser,
};

#[derive(Clone, Debug, PartialEq)]
pub struct HydroTargetFieldSummary {
    pub total_rows: usize,
    pub refined_rows: usize,
    pub polygon_count: usize,
    pub max_level: u8,
    pub cache_hit: bool,
}

#[derive(Clone, Debug)]
pub struct HydroTargetField {
    pub field: HField,
    pub summary: HydroTargetFieldSummary,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

const HFIELD_CACHE_MAGIC: &[u8] = b"EARTHMESH_HFIELD_V1\0";

fn hydro_target_cache_key(
    cells: &[u8],
    levels: &[u8],
    base_m: f64,
    g: f64,
    nlon: usize,
    nlat: usize,
) -> String {
    let base_m = base_m.to_bits().to_le_bytes();
    let g = g.to_bits().to_le_bytes();
    let nlon = (nlon as u64).to_le_bytes();
    let nlat = (nlat as u64).to_le_bytes();
    content_addressed_stage_key(
        "target-cell-hfield-v1",
        &[
            ("tool_version", env!("CARGO_PKG_VERSION").as_bytes()),
            ("cells", cells),
            ("levels", levels),
            ("base_m", &base_m),
            ("gradation", &g),
            ("nlon", &nlon),
            ("nlat", &nlat),
        ],
    )
}

fn encode_cached_target(target: &HydroTargetField) -> Vec<u8> {
    let mut bytes =
        Vec::with_capacity(HFIELD_CACHE_MAGIC.len() + 41 + target.field.values().len() * 8);
    bytes.extend_from_slice(HFIELD_CACHE_MAGIC);
    for value in [
        target.field.nlon(),
        target.field.nlat(),
        target.summary.total_rows,
        target.summary.refined_rows,
        target.summary.polygon_count,
    ] {
        bytes.extend_from_slice(&(value as u64).to_le_bytes());
    }
    bytes.push(target.summary.max_level);
    for value in target.field.values() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_cached_target(bytes: &[u8], nlon: usize, nlat: usize) -> io::Result<HydroTargetField> {
    if !bytes.starts_with(HFIELD_CACHE_MAGIC) {
        return Err(invalid("cached HField has an unknown format"));
    }
    let mut cursor = HFIELD_CACHE_MAGIC.len();
    let mut read_u64 = || -> io::Result<u64> {
        let end = cursor
            .checked_add(8)
            .ok_or_else(|| invalid("cached HField offset overflow"))?;
        let chunk: [u8; 8] = bytes
            .get(cursor..end)
            .ok_or_else(|| invalid("cached HField header is truncated"))?
            .try_into()
            .expect("slice length checked");
        cursor = end;
        Ok(u64::from_le_bytes(chunk))
    };
    let cached_nlon =
        usize::try_from(read_u64()?).map_err(|_| invalid("cached nlon overflows usize"))?;
    let cached_nlat =
        usize::try_from(read_u64()?).map_err(|_| invalid("cached nlat overflows usize"))?;
    let total_rows =
        usize::try_from(read_u64()?).map_err(|_| invalid("cached row count overflows usize"))?;
    let refined_rows = usize::try_from(read_u64()?)
        .map_err(|_| invalid("cached refined count overflows usize"))?;
    let polygon_count = usize::try_from(read_u64()?)
        .map_err(|_| invalid("cached polygon count overflows usize"))?;
    let max_level = *bytes
        .get(cursor)
        .ok_or_else(|| invalid("cached HField header is truncated"))?;
    cursor += 1;
    if (cached_nlon, cached_nlat) != (nlon, nlat) {
        return Err(invalid("cached HField dimensions do not match the request"));
    }
    if refined_rows > total_rows || max_level > 5 {
        return Err(invalid("cached HField summary is invalid"));
    }
    let value_count = nlon
        .checked_mul(nlat)
        .ok_or_else(|| invalid("cached HField dimensions overflow usize"))?;
    if bytes.len().saturating_sub(cursor) != value_count.saturating_mul(8) {
        return Err(invalid("cached HField payload length is invalid"));
    }
    let values = bytes[cursor..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| f64::from_le_bytes(*chunk))
        .collect();
    Ok(HydroTargetField {
        field: HField::from_values(nlon, nlat, values)?,
        summary: HydroTargetFieldSummary {
            total_rows,
            refined_rows,
            polygon_count,
            max_level,
            cache_hit: true,
        },
    })
}

struct HydroPlanRow {
    feature_index: Option<usize>,
    cell_id: Option<String>,
    target_level: u8,
}

fn plan_rows(root: &JsonNode) -> io::Result<Vec<HydroPlanRow>> {
    let object = root
        .as_object()
        .ok_or_else(|| invalid("hydro refinement plan root must be an object"))?;
    if object.get("kind").and_then(JsonNode::as_str) != Some("earthmesh_refinement_plan") {
        return Err(invalid(
            "hydro target-level input is not an earthmesh_refinement_plan",
        ));
    }
    let cells = object
        .get("cells")
        .and_then(JsonNode::as_array)
        .ok_or_else(|| invalid("hydro refinement plan is missing cells[]"))?;
    let mut rows = Vec::with_capacity(cells.len());
    for (row_index, row) in cells.iter().enumerate() {
        let row = row.as_object().ok_or_else(|| {
            invalid(format!(
                "hydro refinement cells[{row_index}] is not an object"
            ))
        })?;
        let feature_index = row.get("cell").map(json_node_to_usize).transpose()?;
        let target = json_node_to_usize(row.get("target_level").ok_or_else(|| {
            invalid(format!(
                "hydro refinement cells[{row_index}] is missing target_level"
            ))
        })?)?;
        if target > 5 {
            return Err(invalid(format!(
                "hydro refinement cells[{row_index}] target_level {target} exceeds Method-C cap 5"
            )));
        }
        let cell_id = row
            .get("cell_id")
            .map(|value| {
                value.as_str().map(str::to_string).ok_or_else(|| {
                    invalid(format!(
                        "hydro refinement cells[{row_index}] cell_id must be a string"
                    ))
                })
            })
            .transpose()?;
        if feature_index.is_none() && cell_id.is_none() {
            return Err(invalid(format!(
                "hydro refinement cells[{row_index}] needs cell_id or legacy cell index"
            )));
        }
        if cell_id.as_deref().is_some_and(|id| id.trim().is_empty()) {
            return Err(invalid(format!(
                "hydro refinement cells[{row_index}] cell_id must not be empty"
            )));
        }
        rows.push(HydroPlanRow {
            feature_index,
            cell_id,
            target_level: target as u8,
        });
    }
    if let Some(total) = object.get("total_cells") {
        let total = json_node_to_usize(total)?;
        if total != rows.len() {
            return Err(invalid(format!(
                "hydro refinement plan total_cells {total} does not match cells[] length {}",
                rows.len()
            )));
        }
    }
    Ok(rows)
}

fn levels_by_group_id(rows: &[HydroPlanRow], group_ids: &[String]) -> io::Result<Vec<u8>> {
    let mut source_ids = Vec::with_capacity(rows.len());
    let mut source_levels = Vec::with_capacity(rows.len());
    for (row_index, row) in rows.iter().enumerate() {
        let cell_id = match row.cell_id.as_deref() {
            Some(cell_id) => cell_id.to_string(),
            None => {
                let feature_index = row.feature_index.expect("validated legacy row index");
                group_ids.get(feature_index).cloned().ok_or_else(|| {
                    invalid(format!(
                        "hydro refinement cells[{row_index}] legacy cell index {feature_index} is outside {} target cells",
                        group_ids.len()
                    ))
                })?
            }
        };
        source_ids.push(cell_id);
        source_levels.push(row.target_level);
    }
    earthmesh_refine_planner::align_cell_values_by_id(&source_ids, &source_levels, group_ids)
        .map_err(invalid)
}

fn feature_center(
    feature: &JsonNode,
    rings: &[Vec<earthmesh_geometry::Point>],
) -> Option<(f64, f64)> {
    let props = feature
        .as_object()
        .and_then(|object| object.get("properties"))
        .and_then(JsonNode::as_object);
    match (
        props
            .and_then(|p| p.get("center_lon"))
            .and_then(JsonNode::as_f64),
        props
            .and_then(|p| p.get("center_lat"))
            .and_then(JsonNode::as_f64),
    ) {
        (Some(lon), Some(lat)) if lon.is_finite() && lat.is_finite() => Some((lon, lat)),
        _ => rings.iter().find_map(|ring| {
            let points = if ring.len() > 1 && ring.first() == ring.last() {
                &ring[..ring.len() - 1]
            } else {
                ring.as_slice()
            };
            if points.is_empty() {
                return None;
            }
            let (x, y, z) = points.iter().fold((0.0, 0.0, 0.0), |sum, point| {
                let lon = point.x.to_radians();
                let lat = point.y.to_radians();
                (
                    sum.0 + lat.cos() * lon.cos(),
                    sum.1 + lat.cos() * lon.sin(),
                    sum.2 + lat.sin(),
                )
            });
            let horizontal = x.hypot(y);
            (horizontal > 0.0 || z != 0.0)
                .then_some((y.atan2(x).to_degrees(), z.atan2(horizontal).to_degrees()))
        }),
    }
}

/// Build the hydro contribution to `h(x)` from the exact source cell polygons
/// referenced by `refinement_plan.json`.
///
/// A small centroid seed (one raster diagonal) prevents a sub-raster source
/// cell from disappearing before fast sweeping. It is conservative: it can
/// widen a target by one HField sample, but never loses a requested cell.
pub fn load_hydro_target_field(
    cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
    base_m: f64,
    g: f64,
    nlon: usize,
    nlat: usize,
) -> io::Result<HydroTargetField> {
    load_hydro_target_field_in_domain(
        cells_geojson,
        target_levels_json,
        base_m,
        g,
        nlon,
        nlat,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn load_hydro_target_field_in_domain(
    cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
    base_m: f64,
    g: f64,
    nlon: usize,
    nlat: usize,
    domain: Option<&GridRegion>,
) -> io::Result<HydroTargetField> {
    if !base_m.is_finite() || base_m <= 0.0 {
        return Err(invalid(
            "hydro h-field base size must be positive and finite",
        ));
    }
    let domain = domain.map(|domain| HfieldDomainMask::new(nlon, nlat, domain));
    let cells_geojson = cells_geojson.as_ref();
    let target_levels_json = target_levels_json.as_ref();
    let cells_text = read_text_maybe_gzip(cells_geojson)?;
    let levels_text = read_text_maybe_gzip(target_levels_json)?;
    let cache = StageCache::new(
        target_levels_json
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".earthmesh-cache"),
    );
    let cache_key = hydro_target_cache_key(
        cells_text.as_bytes(),
        levels_text.as_bytes(),
        base_m,
        g,
        nlon,
        nlat,
    );
    if domain.is_none() {
        if let Ok(Some(bytes)) = cache.load(&cache_key) {
            if let Ok(target) = decode_cached_target(&bytes, nlon, nlat) {
                return Ok(target);
            }
        }
    }

    let cells_root = JsonParser::new(&cells_text).parse()?;
    let groups = hydro_cell_feature_groups(&cells_root)?;
    if groups.is_empty() {
        return Err(invalid("hydro target cell GeoJSON contains no features"));
    }
    let levels_root = JsonParser::new(&levels_text).parse()?;
    let rows = plan_rows(&levels_root)?;
    let group_ids = groups
        .iter()
        .map(|group| group.cell_id.clone())
        .collect::<Vec<_>>();
    let target_levels = levels_by_group_id(&rows, &group_ids)?;

    let mut field = HField::uniform(nlon, nlat, base_m)?;
    let max_level = target_levels.iter().copied().max().unwrap_or(0);
    let dlat_m = EARTH_RADIUS_METERS * (180.0 / nlat as f64).to_radians();
    let mut refined_rows = 0usize;
    let mut polygon_count = 0usize;
    for (group, &level) in groups.iter().zip(&target_levels) {
        if level == 0 {
            continue;
        }
        refined_rows += 1;
        let feature = group.features[0];
        let geometry = feature
            .as_object()
            .and_then(|object| object.get("geometry"))
            .ok_or_else(|| {
                invalid(format!(
                    "hydro target cell {} has no geometry",
                    group.cell_id
                ))
            })?;
        let rings = geometry_outer_rings(geometry);
        if rings.is_empty() {
            return Err(invalid(format!(
                "hydro target cell {} has no polygon outer ring",
                group.cell_id
            )));
        }
        let h_inside = base_m / 2f64.powi(i32::from(level));
        for ring in &rings {
            let points = ring
                .iter()
                .map(|point| (point.x, point.y))
                .collect::<Vec<_>>();
            let region = HRegion::Polygon { points };
            region.validate().map_err(|error| {
                invalid(format!(
                    "hydro target cell {} has invalid polygon ring: {error}",
                    group.cell_id
                ))
            })?;
            field.min_with_fn(|lon, lat| {
                if domain
                    .as_ref()
                    .is_none_or(|domain| domain.contains(lon, lat))
                    && region.contains(lon, lat)
                {
                    h_inside
                } else {
                    f64::INFINITY
                }
            });
            polygon_count += 1;
        }
        if let Some((lon, lat)) = feature_center(feature, &rings) {
            let dlon_m = great_circle_distance_m(lon, lat, lon + 360.0 / nlon as f64, lat);
            let seed_radius_m = 0.55 * dlon_m.hypot(dlat_m);
            let seed = HRegion::Circle {
                lon,
                lat,
                radius_m: seed_radius_m,
            };
            seed.validate().map_err(|error| {
                invalid(format!(
                    "hydro target cell {} has invalid centroid seed: {error}",
                    group.cell_id
                ))
            })?;
            field.min_with_fn(|sample_lon, sample_lat| {
                if domain
                    .as_ref()
                    .is_none_or(|domain| domain.contains(sample_lon, sample_lat))
                    && seed.contains(sample_lon, sample_lat)
                {
                    h_inside
                } else {
                    f64::INFINITY
                }
            });
        }
    }
    if refined_rows == 0 {
        return Err(invalid("hydro refinement plan requests no refined cells"));
    }
    field.limit_gradient(g)?;
    let target = HydroTargetField {
        field,
        summary: HydroTargetFieldSummary {
            total_rows: rows.len(),
            refined_rows,
            polygon_count,
            max_level,
            cache_hit: false,
        },
    };
    if domain.is_none() {
        let _ = cache.store(&cache_key, &encode_cached_target(&target));
    }
    Ok(target)
}

pub fn hydro_target_max_level(options: &HfieldRefineOptions) -> io::Result<usize> {
    let Some((_, levels)) = options.hydro_target_paths() else {
        return Ok(0);
    };
    let root = JsonParser::new(&read_text_maybe_gzip(levels)?).parse()?;
    Ok(plan_rows(&root)?
        .into_iter()
        .map(|row| usize::from(row.target_level))
        .max()
        .unwrap_or(0))
}

pub fn apply_hydro_target_to_field(
    field: &mut HField,
    options: &HfieldRefineOptions,
    base_m: f64,
    domain: Option<&GridRegion>,
) -> io::Result<Option<HydroTargetFieldSummary>> {
    let Some((cells, levels)) = options.hydro_target_paths() else {
        return Ok(None);
    };
    let hydro = load_hydro_target_field_in_domain(
        cells,
        levels,
        base_m,
        options.g,
        options.nlon,
        options.nlat,
        domain,
    )?;
    field.min_with_field(&hydro.field)?;
    field.limit_gradient(options.g)?;
    Ok(Some(hydro.summary))
}

pub fn quote_path(path: &Path) -> io::Result<String> {
    let value = path.to_string_lossy();
    if value.contains('\'') {
        return Err(invalid(format!(
            "hydro refinement path cannot contain a single quote: {}",
            path.display()
        )));
    }
    Ok(format!("'{value}'"))
}

fn prefixed_target_source(
    cells_geojson: &Path,
    target_levels_json: &Path,
    prefix: &str,
) -> io::Result<(Vec<JsonNode>, Vec<(String, u8)>)> {
    let cells_root = JsonParser::new(&read_text_maybe_gzip(cells_geojson)?).parse()?;
    let groups = hydro_cell_feature_groups(&cells_root)?;
    let levels_root = JsonParser::new(&read_text_maybe_gzip(target_levels_json)?).parse()?;
    let rows = plan_rows(&levels_root)?;
    let group_ids = groups
        .iter()
        .map(|group| group.cell_id.clone())
        .collect::<Vec<_>>();
    let levels = levels_by_group_id(&rows, &group_ids)?;
    let mut features = Vec::new();
    let mut plan = Vec::with_capacity(groups.len());
    for (group, level) in groups.iter().zip(levels) {
        let cell_id = format!("{prefix}:{}", group.cell_id);
        for source in &group.features {
            let mut feature = (*source).clone();
            let JsonNode::Object(object) = &mut feature else {
                return Err(invalid("target-cell feature must be an object"));
            };
            let Some(JsonNode::Object(properties)) = object.get_mut("properties") else {
                return Err(invalid("target-cell feature is missing properties"));
            };
            properties.insert("cell_id".to_string(), JsonNode::String(cell_id.clone()));
            features.push(feature);
        }
        plan.push((cell_id, level));
    }
    Ok((features, plan))
}

pub fn combine_target_sources(
    first_cells: &Path,
    first_levels: &Path,
    second_cells: &Path,
    second_levels: &Path,
    out_dir: &Path,
) -> io::Result<(PathBuf, PathBuf)> {
    let (mut features, mut plan) = prefixed_target_source(first_cells, first_levels, "base")?;
    let (second_features, second_plan) =
        prefixed_target_source(second_cells, second_levels, "overlay")?;
    features.extend(second_features);
    plan.extend(second_plan);
    fs::create_dir_all(out_dir)?;
    let cells_path = out_dir.join("target_cells.geojson");
    let levels_path = out_dir.join("target_levels.json");
    let mut root = std::collections::BTreeMap::new();
    root.insert(
        "type".to_string(),
        JsonNode::String("FeatureCollection".to_string()),
    );
    root.insert(
        "kind".to_string(),
        JsonNode::String("earthmesh_composed_target_cells".to_string()),
    );
    root.insert("features".to_string(), JsonNode::Array(features));
    fs::write(
        &cells_path,
        format!("{}\n", json_node_to_string(&JsonNode::Object(root))),
    )?;
    let mut text = format!(
        "{{\n  \"kind\": \"earthmesh_refinement_plan\",\n  \"total_cells\": {},\n  \"cells\": [\n",
        plan.len()
    );
    for (index, (cell_id, target_level)) in plan.iter().enumerate() {
        let comma = if index + 1 == plan.len() { "" } else { "," };
        text.push_str(&format!(
            "    {{\"cell\": {index}, \"cell_id\": \"{}\", \"target_level\": {target_level}}}{comma}\n",
            crate::json_escape_string(cell_id)
        ));
    }
    text.push_str("  ]\n}\n");
    fs::write(&levels_path, text)?;
    Ok((cells_path, levels_path))
}

/// Materialize a second-pass namelist whose only new refinement input is the
/// hydro target field, then execute the normal production refine pipeline.
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "earthmesh_hydro_refinement_adapter_{name}_{}",
            std::process::id()
        ))
    }

    #[test]
    fn hydro_plan_becomes_a_gradient_limited_target_field() {
        let root = temp_path("field");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}},{"type":"Feature","properties":{"center_lon":100,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[98,-2],[102,-2],[102,2],[98,2],[98,-2]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":2,"cells":[{"cell":0,"target_level":2},{"cell":1,"target_level":0}]}"#,
        )
        .unwrap();
        let hydro = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 360, 180)
            .expect("build hydro target field");
        assert_eq!(hydro.summary.refined_rows, 1);
        assert_eq!(hydro.summary.max_level, 2);
        assert!(!hydro.summary.cache_hit);
        assert_eq!(hydro.field.level_at(0.0, 0.0, 1_000_000.0, 5), 2);
        assert_eq!(hydro.field.level_at(100.0, 0.0, 1_000_000.0, 5), 0);
        assert!(hydro.field.level_at(4.0, 0.0, 1_000_000.0, 5) > 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn regional_hydro_targets_are_scoped_before_gradient_limiting() {
        let root = temp_path("regional_field");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"outside","center_lon":107,"center_lat":22},"geometry":{"type":"Polygon","coordinates":[[[106.8,21.8],[107.2,21.8],[107.2,22.2],[106.8,22.2],[106.8,21.8]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell_id":"outside","target_level":2}]}"#,
        )
        .unwrap();
        let domain = GridRegion::Bbox {
            west: 108.0,
            east: 120.0,
            south: 18.0,
            north: 26.0,
        };
        let target = load_hydro_target_field_in_domain(
            &cells,
            &levels,
            100_000.0,
            0.2,
            360,
            180,
            Some(&domain),
        )
        .unwrap();

        assert_eq!(target.field.level_at(108.5, 22.5, 100_000.0, 2), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn target_field_cache_hits_and_invalidates_on_content_or_parameters() {
        let root = temp_path("cache");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"target_level":1}]}"#,
        )
        .unwrap();

        let first = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 36, 18).unwrap();
        assert!(!first.summary.cache_hit);
        let second = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 36, 18).unwrap();
        assert!(second.summary.cache_hit);
        assert_eq!(first.field.values(), second.field.values());

        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"target_level":2}]}"#,
        )
        .unwrap();
        let changed_content =
            load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 36, 18).unwrap();
        assert!(!changed_content.summary.cache_hit);
        assert_eq!(changed_content.summary.max_level, 2);

        let changed_parameter =
            load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.1, 36, 18).unwrap();
        assert!(!changed_parameter.summary.cache_hit);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_hydro_plan_rejects_an_out_of_range_cell_index() {
        let root = temp_path("order");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":9,"target_level":1}]}"#,
        )
        .unwrap();
        let error = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 36, 18)
            .expect_err("invalid legacy cell index must fail");
        assert!(error.to_string().contains("outside 1 target cells"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hydro_target_field_rejects_invalid_polygon_rings() {
        let root = temp_path("invalid_polygon");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"bad","center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[0,0],[10,10],[0,10],[10,0],[0,0]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell_id":"bad","target_level":1}]}"#,
        )
        .unwrap();

        let error = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 36, 18)
            .expect_err("invalid polygon must fail before silent no-refinement");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("bad"));
        assert!(error.to_string().contains("invalid polygon ring"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stable_cell_ids_make_plan_row_order_irrelevant() {
        let root = temp_path("stable_ids");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{"cell_id":"west","center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}},
              {"type":"Feature","properties":{"cell_id":"east","center_lon":100,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[98,-2],[102,-2],[102,2],[98,2],[98,-2]]]}}
            ]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":2,"cells":[{"cell":1,"cell_id":"east","target_level":0},{"cell":0,"cell_id":"west","target_level":2}]}"#,
        )
        .unwrap();

        let target = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 360, 180)
            .expect("stable IDs should align rows by identity");
        assert_eq!(target.field.level_at(0.0, 0.0, 1_000_000.0, 5), 2);
        assert_eq!(target.field.level_at(100.0, 0.0, 1_000_000.0, 5), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stable_cell_id_join_rejects_duplicates_and_missing_cells() {
        let group_ids = vec!["a".to_string(), "b".to_string()];
        let duplicate = vec![
            HydroPlanRow {
                feature_index: Some(0),
                cell_id: Some("a".into()),
                target_level: 1,
            },
            HydroPlanRow {
                feature_index: Some(1),
                cell_id: Some("a".into()),
                target_level: 2,
            },
        ];
        let error = levels_by_group_id(&duplicate, &group_ids)
            .expect_err("duplicate stable identity must fail");
        assert!(error.to_string().contains("duplicate cell_id a"));

        let missing = vec![HydroPlanRow {
            feature_index: Some(0),
            cell_id: Some("a".into()),
            target_level: 1,
        }];
        let error = levels_by_group_id(&missing, &group_ids)
            .expect_err("missing target identity must fail");
        assert!(error.to_string().contains("missing target cell_id b"));
    }

    #[test]
    fn duplicate_class_features_become_one_exact_parent_target() {
        let root = temp_path("duplicate_cell");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let levels = root.join("levels.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{"cell_id":"42","overlap_class":"R2","center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}},
              {"type":"Feature","properties":{"cell_id":"42","overlap_class":"R3","center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}}
            ]}"#,
        )
        .unwrap();
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"cell_id":"42","target_level":2}]}"#,
        )
        .unwrap();

        let hydro = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 360, 180)
            .expect("aggregate duplicate class rows");
        assert_eq!(hydro.summary.total_rows, 1);
        assert_eq!(hydro.summary.refined_rows, 1);
        assert_eq!(hydro.summary.polygon_count, 1);
        assert_eq!(hydro.summary.max_level, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn quality_repair_artifacts_feed_the_existing_target_cell_hfield() {
        let root = temp_path("quality_overlay");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let input = earthmesh_quality::QualityMeshInput {
            vertices: vec![
                earthmesh_geometry::Point::new(0.0, 0.0),
                earthmesh_geometry::Point::new(4.0, 0.0),
                earthmesh_geometry::Point::new(4.1, 0.5),
                earthmesh_geometry::Point::new(2.0, 2.0),
                earthmesh_geometry::Point::new(0.0, 2.0),
                earthmesh_geometry::Point::new(-0.1, 1.0),
            ],
            cells: vec![earthmesh_quality::QualityCell {
                vertices: vec![0, 1, 2, 3, 4, 5],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        };
        let thresholds = earthmesh_quality::QualityThresholds {
            min_angle_warn_deg: 0.0,
            ..earthmesh_quality::QualityThresholds::default()
        };
        let report = earthmesh_quality::compute(&input, &thresholds);
        let center = report.worst_cells[0].centroid;
        let cells = root.join("quality_repair_cells.geojson");
        let levels = root.join("quality_repair_plan.json");
        fs::write(
            &cells,
            earthmesh_quality::io::to_quality_repair_cells_geojson(&report),
        )
        .unwrap();
        fs::write(
            &levels,
            earthmesh_quality::io::to_quality_repair_plan_json(&report),
        )
        .unwrap();

        let target = load_hydro_target_field(&cells, &levels, 1_000_000.0, 0.2, 360, 180)
            .expect("quality repair overlay");
        assert_eq!(target.summary.refined_rows, 1);
        assert_eq!(target.summary.max_level, 1);
        assert_eq!(target.field.level_at(center.x, center.y, 1_000_000.0, 5), 1);
        assert_eq!(target.field.level_at(100.0, 0.0, 1_000_000.0, 5), 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn composed_target_sources_preserve_the_existing_hfield_overlay() {
        let root = temp_path("composed_overlay");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let first_cells = root.join("first_cells.geojson");
        let first_levels = root.join("first_levels.json");
        let second_cells = root.join("second_cells.geojson");
        let second_levels = root.join("second_levels.json");
        fs::write(
            &first_cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"1","center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &first_levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell_id":"1","target_level":2}]}"#,
        )
        .unwrap();
        fs::write(
            &second_cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"1","center_lon":100,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[98,-2],[102,-2],[102,2],[98,2],[98,-2]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &second_levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell_id":"1","target_level":1}]}"#,
        )
        .unwrap();

        let (cells, levels) = combine_target_sources(
            &first_cells,
            &first_levels,
            &second_cells,
            &second_levels,
            &root.join("combined"),
        )
        .unwrap();
        let target = load_hydro_target_field(cells, levels, 1_000_000.0, 0.2, 360, 180).unwrap();
        assert_eq!(target.summary.refined_rows, 2);
        assert_eq!(target.field.level_at(0.0, 0.0, 1_000_000.0, 5), 2);
        assert_eq!(target.field.level_at(100.0, 0.0, 1_000_000.0, 5), 1);
        let _ = fs::remove_dir_all(root);
    }
}
