//! `&hfield` namelist group: compose the Method-C specified-refine regions into a
//! continuous cell-width field (`earthmesh_hfield`) and drive Method-C from
//! quantized target levels instead of per-region geometry.
//!
//! Namelist keys (all inside a `&hfield ... /` group, `NL%` prefix like every
//! other group):
//!   hfield_on         .true./.false.  master switch (`&hfield` is opt-in; explicit false disables)
//!   hfield_g          gradation limit |∇h| <= g          (default 0.2)
//!   hfield_max_level  quantization depth, 1..=5; 0 = use the run's max level
//!   hfield_base_m     background cell size in meters; 0/absent = 2πR/(5·NXP)
//!   hfield_nlon/nlat  field raster size                  (default 720 x 360)
//!   hfield_origin_lon/lat  WGS84 origin for geographic rasters on Cartesian-XY meshes

use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_hfield::HField;
use earthmesh_mesh::{CartesianPoint, LonLatDegrees, MethodCRefinementRegion};
use serde::{Deserialize, Serialize};

use crate::area_judge_threshold_inputs::{
    enabled_mean_threshold_field_specs, enabled_std_threshold_field_specs, numeric_missing_values,
    reject_invalid_threshold_values, threshold_latitude_order, threshold_longitude_coordinates,
    LatitudeOrder,
};
use crate::namelist_reader::{namelist_assignments, namelist_has_section};
use crate::GridRegion;

#[derive(Clone, Debug, PartialEq)]
pub struct HfieldRefineOptions {
    pub g: f64,
    /// `None` = follow the run's computed max refinement level.
    pub max_level: Option<usize>,
    /// `None` = derive from NXP (`2πR / (5·NXP)`).
    pub base_m: Option<f64>,
    /// WGS84 tangent-plane origin for sampling geographic threshold rasters
    /// from native Cartesian `(x, y)` meters.
    pub geographic_origin: Option<(f64, f64)>,
    pub nlon: usize,
    pub nlat: usize,
    /// Optional per-cell target field produced by the hydro refinement planner.
    /// Both files are required together; paths are normally absolute because
    /// the Project adapter materializes this group after the first hydro pass.
    pub target_cells_geojson: Option<String>,
    pub target_levels_json: Option<String>,
}

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

#[derive(Clone, Debug)]
pub(crate) struct HfieldDomainMask {
    nlon: usize,
    nlat: usize,
    active: Vec<bool>,
}

impl HfieldDomainMask {
    pub(crate) fn new(nlon: usize, nlat: usize, domain: &GridRegion) -> Self {
        let mut active = vec![false; nlon * nlat];
        for i in 0..nlon {
            let lon = -180.0 + (i as f64 + 0.5) * 360.0 / nlon as f64;
            for j in 0..nlat {
                let lat = -90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64;
                active[i * nlat + j] = domain.contains(lon, lat);
            }
        }
        Self { nlon, nlat, active }
    }

    pub(crate) fn contains(&self, lon: f64, lat: f64) -> bool {
        let i = (((earthmesh_hfield::wrap_lon_degrees(lon) + 180.0) / 360.0) * self.nlon as f64)
            .floor()
            .clamp(0.0, (self.nlon - 1) as f64) as usize;
        let j = (((lat + 90.0) / 180.0) * self.nlat as f64)
            .floor()
            .clamp(0.0, (self.nlat - 1) as f64) as usize;
        self.active[i * self.nlat + j]
    }

    fn is_active(&self, i: usize, j: usize) -> bool {
        self.active[i * self.nlat + j]
    }
}

fn parse_hfield_f64(field: &str, value: &str) -> io::Result<f64> {
    let cleaned = value
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .replace(['d', 'D'], "e");
    cleaned
        .parse::<f64>()
        .map_err(|_| invalid(format!("&hfield {field} value '{value}' is not a number")))
}

fn parse_hfield_usize(field: &str, value: &str) -> io::Result<usize> {
    let cleaned = value.trim().trim_matches('\'').trim_matches('"');
    cleaned.parse::<usize>().map_err(|_| {
        invalid(format!(
            "&hfield {field} value '{value}' is not a non-negative integer"
        ))
    })
}

fn parse_hfield_bool(field: &str, value: &str) -> io::Result<bool> {
    let cleaned = value
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .trim_matches('.')
        .to_ascii_lowercase();
    match cleaned.as_str() {
        "true" | "t" => Ok(true),
        "false" | "f" => Ok(false),
        _ => Err(invalid(format!(
            "&hfield {field} value '{value}' is not a Canonical logical"
        ))),
    }
}

/// Read the `&hfield` group. Absent group or `hfield_on = .false.` yields
/// `Ok(None)` (feature off).
pub fn read_hfield_refine_options(contents: &str) -> io::Result<Option<HfieldRefineOptions>> {
    if !namelist_has_section(contents, "hfield") {
        return Ok(None);
    }
    let mut enabled = true;
    let mut g = 0.2_f64;
    let mut max_level = 0usize;
    let mut base_m = 0.0_f64;
    let mut origin_lon = None;
    let mut origin_lat = None;
    let mut nlon = 720usize;
    let mut nlat = 360usize;
    let mut target_cells_geojson = None;
    let mut target_levels_json = None;
    for assignment in namelist_assignments(contents, "hfield")? {
        match assignment.field.as_str() {
            "hfield_on" => enabled = parse_hfield_bool(&assignment.field, &assignment.value)?,
            "hfield_g" => g = parse_hfield_f64(&assignment.field, &assignment.value)?,
            "hfield_max_level" => {
                max_level = parse_hfield_usize(&assignment.field, &assignment.value)?
            }
            "hfield_base_m" => base_m = parse_hfield_f64(&assignment.field, &assignment.value)?,
            "hfield_origin_lon" => {
                origin_lon = Some(parse_hfield_f64(&assignment.field, &assignment.value)?)
            }
            "hfield_origin_lat" => {
                origin_lat = Some(parse_hfield_f64(&assignment.field, &assignment.value)?)
            }
            "hfield_nlon" => nlon = parse_hfield_usize(&assignment.field, &assignment.value)?,
            "hfield_nlat" => nlat = parse_hfield_usize(&assignment.field, &assignment.value)?,
            "hfield_target_cells_geojson" => {
                target_cells_geojson = Some(
                    assignment
                        .value
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string(),
                )
            }
            "hfield_target_levels_json" => {
                target_levels_json = Some(
                    assignment
                        .value
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string(),
                )
            }
            _ => {
                return Err(invalid(format!(
                    "unknown &hfield field '{}'",
                    assignment.field
                )))
            }
        }
    }
    if !enabled {
        return Ok(None);
    }
    if !g.is_finite() || g <= 0.0 {
        return Err(invalid(format!(
            "hfield_g must be positive and finite, got {g}"
        )));
    }
    if max_level > 5 {
        return Err(invalid(format!(
            "hfield_max_level must be in 0..=5 (0 = auto), got {max_level}"
        )));
    }
    if !base_m.is_finite() || base_m < 0.0 {
        return Err(invalid(format!(
            "hfield_base_m must be non-negative and finite, got {base_m}"
        )));
    }
    if nlon < 4 || nlat < 2 {
        return Err(invalid(format!(
            "hfield raster {nlon}x{nlat} too small (need >= 4x2)"
        )));
    }
    let geographic_origin = match (origin_lon, origin_lat) {
        (None, None) => None,
        (Some(lon), Some(lat))
            if (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat) =>
        {
            Some((lon, lat))
        }
        (Some(_), Some(_)) => {
            return Err(invalid(
                "hfield geographic origin must be valid WGS84 lon/lat".to_string(),
            ))
        }
        _ => {
            return Err(invalid(
                "hfield_origin_lon and hfield_origin_lat must be set together".to_string(),
            ))
        }
    };
    if target_cells_geojson.is_some() != target_levels_json.is_some() {
        return Err(invalid(
            "hfield_target_cells_geojson and hfield_target_levels_json must be set together"
                .to_string(),
        ));
    }
    if target_cells_geojson.as_deref().is_some_and(str::is_empty)
        || target_levels_json.as_deref().is_some_and(str::is_empty)
    {
        return Err(invalid(
            "hydro h-field target paths must not be empty".to_string(),
        ));
    }
    Ok(Some(HfieldRefineOptions {
        g,
        max_level: if max_level == 0 {
            None
        } else {
            Some(max_level)
        },
        base_m: if base_m > 0.0 { Some(base_m) } else { None },
        geographic_origin,
        nlon,
        nlat,
        target_cells_geojson,
        target_levels_json,
    }))
}

impl HfieldRefineOptions {
    pub(crate) fn hydro_target_paths(&self) -> Option<(&Path, &Path)> {
        self.target_cells_geojson
            .as_deref()
            .zip(self.target_levels_json.as_deref())
            .map(|(cells, levels)| (Path::new(cells), Path::new(levels)))
    }
}

/// Compose the specified-refine regions into a gradient-limited cell-width
/// field: each level-L region pins `h = base / 2^L` inside its footprint, the
/// pointwise minimum wins on overlap, and `limit_gradient(g)` builds the
/// slope-g transition skirts that make nested level sets legal by construction.
pub fn build_hfield_from_regions(
    regions: &[MethodCRefinementRegion],
    base_m: f64,
    g: f64,
    nlon: usize,
    nlat: usize,
) -> io::Result<HField> {
    build_hfield_from_regions_in_domain(regions, base_m, g, nlon, nlat, None)
}

fn build_hfield_from_regions_in_domain(
    regions: &[MethodCRefinementRegion],
    base_m: f64,
    g: f64,
    nlon: usize,
    nlat: usize,
    domain: Option<&HfieldDomainMask>,
) -> io::Result<HField> {
    if !base_m.is_finite() || base_m <= 0.0 {
        return Err(invalid(format!(
            "h-field base cell size must be positive, got {base_m}"
        )));
    }
    let mut field = HField::uniform(nlon, nlat, base_m)?;
    for region in regions {
        region.validate().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid geographic HField region: {error}"),
            )
        })?;
        let level = region.level().min(5) as i32;
        if level < 1 {
            continue;
        }
        if let Some(warning) = region.canonical_geometry_warning() {
            eprintln!("warning: HField specified-refine region: {warning}");
        }
        let h_inside = base_m / 2f64.powi(level);
        field.min_with_fn(|lon, lat| {
            if domain.is_none_or(|domain| domain.contains(lon, lat))
                && region.contains_lonlat_canonical(LonLatDegrees::new(lon, lat))
            {
                h_inside
            } else {
                f64::INFINITY
            }
        });
    }
    field.limit_gradient(g)?;
    Ok(field)
}

/// Analytic Cartesian-XY h-field sample. Each region contributes its interior
/// size plus a slope-`g` transition based on Euclidean distance from the
/// region boundary; the pointwise minimum is already the largest field below
/// those constraints, so no raster or projection is needed.
pub(crate) fn cartesian_hfield_level_at(
    regions: &[MethodCRefinementRegion],
    x_meters: f64,
    y_meters: f64,
    base_m: f64,
    g: f64,
    max_level: usize,
) -> u8 {
    let point = CartesianPoint::new(x_meters, y_meters, 0.0);
    let mut h = base_m;
    for region in regions {
        let level = region.level().min(max_level).min(5);
        if level == 0 {
            continue;
        }
        let inside_h = base_m / 2f64.powi(level as i32);
        let candidate = inside_h + g * region.cartesian_xy_outside_distance_meters(point);
        h = h.min(candidate);
    }
    if !h.is_finite() || h <= 0.0 || h >= base_m {
        return 0;
    }
    (((base_m / h).log2() - 1e-9).ceil() as usize).min(max_level) as u8
}

pub(crate) fn cartesian_xy_to_lonlat(
    x_meters: f64,
    y_meters: f64,
    origin_lon: f64,
    origin_lat: f64,
) -> (f64, f64) {
    let distance = x_meters.hypot(y_meters);
    if distance <= f64::EPSILON {
        return (earthmesh_hfield::wrap_lon_degrees(origin_lon), origin_lat);
    }
    let angular = distance / earthmesh_hfield::EARTH_RADIUS_METERS;
    let bearing = x_meters.atan2(y_meters);
    let lat1 = origin_lat.to_radians();
    let lon1 = origin_lon.to_radians();
    let lat2 = (lat1.sin() * angular.cos() + lat1.cos() * angular.sin() * bearing.cos())
        .clamp(-1.0, 1.0)
        .asin();
    let lon2 = lon1
        + (bearing.sin() * angular.sin() * lat1.cos())
            .atan2(angular.cos() - lat1.sin() * lat2.sin());
    (
        earthmesh_hfield::wrap_lon_degrees(lon2.to_degrees()),
        lat2.to_degrees(),
    )
}

fn apply_mean_threshold_hfield_contributions_with_landtype_mask(
    field: &mut HField,
    refine: &RefineConfig,
    mesh_type: &str,
    base_m: f64,
    target_level: usize,
    g: f64,
    landtype_mask: Option<&LandtypeMaskSource>,
    domain: Option<&HfieldDomainMask>,
    stats_cache: &mut ThresholdStatsCache,
) -> io::Result<usize> {
    let specs = enabled_mean_threshold_field_specs(refine, mesh_type);
    if specs.is_empty() {
        return Ok(0);
    }
    let h_inside = base_m / 2f64.powi(target_level.clamp(1, 5) as i32);
    let threshold_dir = Path::new(refine.threshold_dir.trim());
    let mut applied = 0usize;
    for spec in specs {
        let input = threshold_dir.join(format!("{}.nc", spec.file_stem));
        let key = (input.display().to_string(), spec.var_name.clone());
        if !stats_cache.contains_key(&key) {
            let file = crate::open_netcdf(&input).map_err(crate::netcdf_to_io_error)?;
            let stats = read_threshold_stats_on_hfield_masked(
                &file,
                &spec.var_name,
                field,
                landtype_mask,
                domain,
            )?;
            stats_cache.insert(key.clone(), stats);
        }
        let stats = stats_cache.get(&key).expect("threshold stats cached");
        min_with_threshold_matrix(field, &stats.mean, spec.threshold, h_inside, domain);
        applied += 1;
    }
    if applied > 0 {
        field.limit_gradient(g)?;
    }
    Ok(applied)
}

#[cfg(test)]
pub(crate) fn apply_std_threshold_hfield_contributions(
    field: &mut HField,
    refine: &RefineConfig,
    mesh_type: &str,
    base_m: f64,
    target_level: usize,
    g: f64,
) -> io::Result<usize> {
    let mut stats_cache = ThresholdStatsCache::new();
    apply_std_threshold_hfield_contributions_with_landtype_mask(
        field,
        refine,
        mesh_type,
        base_m,
        target_level,
        g,
        None,
        None,
        &mut stats_cache,
    )
}

fn apply_std_threshold_hfield_contributions_with_landtype_mask(
    field: &mut HField,
    refine: &RefineConfig,
    mesh_type: &str,
    base_m: f64,
    target_level: usize,
    g: f64,
    landtype_mask: Option<&LandtypeMaskSource>,
    domain: Option<&HfieldDomainMask>,
    stats_cache: &mut ThresholdStatsCache,
) -> io::Result<usize> {
    let specs = enabled_std_threshold_field_specs(refine, mesh_type);
    if specs.is_empty() {
        return Ok(0);
    }
    let h_inside = base_m / 2f64.powi(target_level.clamp(1, 5) as i32);
    let threshold_dir = Path::new(refine.threshold_dir.trim());
    let mut applied = 0usize;
    for spec in specs {
        let input = threshold_dir.join(format!("{}.nc", spec.file_stem));
        let key = (input.display().to_string(), spec.var_name.clone());
        if !stats_cache.contains_key(&key) {
            let file = crate::open_netcdf(&input).map_err(crate::netcdf_to_io_error)?;
            let stats = read_threshold_stats_on_hfield_masked(
                &file,
                &spec.var_name,
                field,
                landtype_mask,
                domain,
            )?;
            stats_cache.insert(key.clone(), stats);
        }
        let stats = stats_cache.get(&key).expect("threshold stats cached");
        min_with_threshold_matrix(field, &stats.stddev, spec.threshold, h_inside, domain);
        applied += 1;
    }
    if applied > 0 {
        field.limit_gradient(g)?;
    }
    Ok(applied)
}

pub(crate) fn has_mean_threshold_hfield_sources(refine: &RefineConfig, mesh_type: &str) -> bool {
    !enabled_mean_threshold_field_specs(refine, mesh_type).is_empty()
}

pub(crate) fn has_threshold_hfield_sources(refine: &RefineConfig, mesh_type: &str) -> bool {
    has_mean_threshold_hfield_sources(refine, mesh_type)
        || !enabled_std_threshold_field_specs(refine, mesh_type).is_empty()
        || has_landtype_basic_threshold_hfield_sources(refine, mesh_type)
}

fn has_land_thresholds(refine: &RefineConfig, mesh_type: &str) -> bool {
    supports_threshold_hfield(mesh_type)
        && (refine.refine_num_landtypes || refine.refine_area_mainland)
}

fn has_ocean_thresholds(refine: &RefineConfig, mesh_type: &str) -> bool {
    supports_threshold_hfield(mesh_type) && refine.refine_sea_ratio
}

fn supports_threshold_hfield(mesh_type: &str) -> bool {
    matches!(
        mesh_type,
        "landmesh" | "oceanmesh" | "atmos" | "atmosmesh" | "LOCmesh" | "earthmesh"
    )
}

fn has_landtype_basic_threshold_hfield_sources(refine: &RefineConfig, mesh_type: &str) -> bool {
    has_land_thresholds(refine, mesh_type) || has_ocean_thresholds(refine, mesh_type)
}

fn apply_landtype_basic_threshold_hfield_contributions(
    field: &mut HField,
    refine: &RefineConfig,
    mesh_type: &str,
    config: Option<&EarthmeshConfig>,
    base_m: f64,
    target_level: usize,
    g: f64,
    domain: Option<&HfieldDomainMask>,
) -> io::Result<usize> {
    if !has_landtype_basic_threshold_hfield_sources(refine, mesh_type) {
        return Ok(0);
    }
    let config = config.ok_or_else(|| {
        invalid("landtype basic hfield thresholds require mkgrd config".to_string())
    })?;
    if !crate::landtype_file_is_real(&config.landtype_file) {
        return Err(invalid(
            "landtype basic hfield thresholds require a real NL%landtype_file".to_string(),
        ));
    }
    let bins =
        read_landtype_source_for_hfield(Path::new(config.landtype_file.trim()), field, domain)?;
    let h_inside = base_m / 2f64.powi(target_level.clamp(1, 5) as i32);
    let applied = apply_landtype_basic_thresholds_from_bins(
        field, &bins, refine, mesh_type, h_inside, domain,
    )?;
    if applied > 0 {
        field.limit_gradient(g)?;
    }
    Ok(applied)
}

fn hfield_landtype_mask_source(
    config: Option<&EarthmeshConfig>,
) -> io::Result<Option<LandtypeMaskSource>> {
    let Some(config) = config else {
        return Ok(None);
    };
    if !crate::landtype_file_is_real(&config.landtype_file) {
        return Ok(None);
    }
    read_landtype_mask_source_for_hfield(Path::new(config.landtype_file.trim())).map(Some)
}

#[derive(Debug)]
struct LandtypeBinStats {
    hfield_len: usize,
    slot_by_hfield: Vec<usize>,
    total: Vec<usize>,
    ocean: Vec<usize>,
    land: Vec<usize>,
    class_counts: Vec<Vec<(i32, usize)>>,
}

impl LandtypeBinStats {
    fn new(field: &HField, domain: Option<&HfieldDomainMask>) -> Self {
        let len = field.nlon() * field.nlat();
        let mut slot_by_hfield = vec![usize::MAX; len];
        let mut slot_count = 0usize;
        for i in 0..field.nlon() {
            for j in 0..field.nlat() {
                if domain.is_none_or(|domain| domain.is_active(i, j)) {
                    slot_by_hfield[i * field.nlat() + j] = slot_count;
                    slot_count += 1;
                }
            }
        }
        Self {
            hfield_len: len,
            slot_by_hfield,
            total: vec![0; slot_count],
            ocean: vec![0; slot_count],
            land: vec![0; slot_count],
            class_counts: vec![Vec::new(); slot_count],
        }
    }

    fn record(&mut self, out: usize, landtype: i32) -> io::Result<()> {
        if landtype < 0 {
            return Err(invalid(format!(
                "landtype value {landtype} must be non-negative"
            )));
        }
        let Some(slot) = self.slot(out) else {
            return Ok(());
        };
        self.total[slot] += 1;
        if landtype == 0 {
            self.ocean[slot] += 1;
        } else {
            self.land[slot] += 1;
            if let Some((_, count)) = self.class_counts[slot]
                .iter_mut()
                .find(|(class, _)| *class == landtype)
            {
                *count += 1;
            } else {
                self.class_counts[slot].push((landtype, 1));
            }
        }
        Ok(())
    }

    fn exclude_class(&mut self, landtype: i32) {
        for counts in &mut self.class_counts {
            counts.retain(|(class, _)| *class != landtype);
        }
    }

    fn slot(&self, out: usize) -> Option<usize> {
        self.slot_by_hfield
            .get(out)
            .copied()
            .filter(|slot| *slot != usize::MAX)
    }

    fn total_at(&self, out: usize) -> usize {
        self.slot(out).map_or(0, |slot| self.total[slot])
    }

    fn ocean_at(&self, out: usize) -> usize {
        self.slot(out).map_or(0, |slot| self.ocean[slot])
    }

    fn land_at(&self, out: usize) -> usize {
        self.slot(out).map_or(0, |slot| self.land[slot])
    }

    fn distinct_at(&self, out: usize) -> usize {
        self.slot(out)
            .map_or(0, |slot| self.class_counts[slot].len())
    }

    #[cfg(test)]
    fn contains_class(&self, out: usize, landtype: i32) -> bool {
        self.slot(out).is_some_and(|slot| {
            self.class_counts[slot]
                .iter()
                .any(|(class, _)| *class == landtype)
        })
    }

    fn max_class_count_at(&self, out: usize) -> usize {
        self.slot(out).map_or(0, |slot| {
            self.class_counts[slot]
                .iter()
                .map(|(_, count)| *count)
                .max()
                .unwrap_or(0)
        })
    }

    #[cfg(test)]
    fn total_samples(&self) -> usize {
        self.total.iter().sum()
    }
}

#[cfg(test)]
fn landtype_hfield_bin(lon: f64, src_j: usize, src_nlat: usize, field: &HField) -> usize {
    let (i, j) = landtype_hfield_indices(lon, src_j, src_nlat, field);
    i * field.nlat() + j
}

fn landtype_hfield_indices(
    lon: f64,
    src_j: usize,
    src_nlat: usize,
    field: &HField,
) -> (usize, usize) {
    // Canonical global source rows run north-to-south: row 0 is nearest +90°.
    let lat = 90.0 - (src_j as f64 + 0.5) * 180.0 / src_nlat as f64;
    let i = (((earthmesh_hfield::wrap_lon_degrees(lon) + 180.0) / 360.0) * field.nlon() as f64)
        .floor()
        .clamp(0.0, (field.nlon() - 1) as f64) as usize;
    let j = (((lat + 90.0) / 180.0) * field.nlat() as f64)
        .floor()
        .clamp(0.0, (field.nlat() - 1) as f64) as usize;
    (i, j)
}

fn landtype_tile(
    variable: &netcdf::Variable<'_>,
    lat_lon: bool,
    lon_start: usize,
    lon_count: usize,
    lat_start: usize,
    lat_count: usize,
) -> io::Result<Vec<i8>> {
    if lat_lon {
        variable
            .get_values::<i8, _>((
                lat_start..lat_start + lat_count,
                lon_start..lon_start + lon_count,
            ))
            .map_err(crate::netcdf_to_io_error)
    } else {
        variable
            .get_values::<i8, _>((
                lon_start..lon_start + lon_count,
                lat_start..lat_start + lat_count,
            ))
            .map_err(crate::netcdf_to_io_error)
    }
}

fn landtype_source_layout(
    file: &netcdf::File,
    variable: &netcdf::Variable<'_>,
) -> io::Result<(bool, usize, usize, LatitudeOrder, Option<Vec<f64>>)> {
    let dims = variable.dimensions();
    if dims.len() != 2 {
        return Err(invalid("landtype must be 2-D".to_string()));
    }
    let names = dims
        .iter()
        .map(|dimension| dimension.name().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let lat_lon = is_lat_dim(&names[0]) && is_lon_dim(&names[1]);
    let lon_lat = is_lon_dim(&names[0]) && is_lat_dim(&names[1]);
    if !lat_lon && !lon_lat {
        return Err(invalid(format!(
            "landtype dimensions {names:?} must identify longitude and latitude axes"
        )));
    }
    let src_nlon = dims[usize::from(lat_lon)].len();
    let lat_position = usize::from(!lat_lon);
    let src_nlat = dims[lat_position].len();
    if src_nlon == 0 || src_nlat == 0 {
        return Err(invalid("landtype dimensions must be non-empty".to_string()));
    }
    let latitude_order = threshold_latitude_order(file, &dims[lat_position].name(), src_nlat)?;
    let longitude_position = usize::from(lat_lon);
    let longitudes =
        threshold_longitude_coordinates(file, &dims[longitude_position].name(), src_nlon)?;
    Ok((lat_lon, src_nlon, src_nlat, latitude_order, longitudes))
}

fn canonical_latitude_index(
    latitude_order: LatitudeOrder,
    file_index: usize,
    nlat: usize,
) -> usize {
    match latitude_order {
        LatitudeOrder::NorthToSouth => file_index,
        LatitudeOrder::SouthToNorth => nlat - 1 - file_index,
    }
}

fn file_latitude_index(
    latitude_order: LatitudeOrder,
    canonical_index: usize,
    nlat: usize,
) -> usize {
    canonical_latitude_index(latitude_order, canonical_index, nlat)
}

fn source_longitude(index: usize, nlon: usize, coordinates: Option<&[f64]>) -> f64 {
    coordinates.map_or_else(
        || -180.0 + (index as f64 + 0.5) * 360.0 / nlon as f64,
        |values| earthmesh_hfield::wrap_lon_degrees(values[index]),
    )
}

fn nearest_longitude_index(value: f64, source_len: usize, coordinates: Option<&[f64]>) -> usize {
    let Some(coordinates) = coordinates else {
        return scaled_hfield_center_index(value, source_len, true);
    };
    let ascending = coordinates.len() <= 1 || coordinates[0] < coordinates[coordinates.len() - 1];
    let target = earthmesh_hfield::wrap_lon_degrees(value);
    let mut best = (f64::INFINITY, 0usize);
    for shifted in [target - 360.0, target, target + 360.0] {
        let insertion = if ascending {
            coordinates.partition_point(|coordinate| *coordinate < shifted)
        } else {
            coordinates.partition_point(|coordinate| *coordinate > shifted)
        };
        for index in [
            insertion.saturating_sub(1),
            insertion.min(coordinates.len() - 1),
        ] {
            let distance = (coordinates[index] - shifted).abs();
            if distance < best.0 {
                best = (distance, index);
            }
        }
    }
    best.1
}

fn is_missing_numeric(value: impl Into<f64>, missing: &[f64]) -> bool {
    missing.contains(&value.into())
}

fn netcdf_longitude_tile_size(
    variable: &netcdf::Variable<'_>,
    lat_lon: bool,
    src_nlon: usize,
    fallback: usize,
) -> usize {
    variable
        .chunking()
        .ok()
        .flatten()
        .and_then(|chunking| chunking.get(usize::from(lat_lon)).copied())
        .filter(|size| *size > 0)
        .unwrap_or(fallback)
        .min(src_nlon)
}

const LANDTYPE_MAXLC_CACHE_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
struct LandtypeMaxlcIdentity {
    canonical_path: PathBuf,
    source_len: u64,
    modified_unix_nanos: u128,
    unix_device: Option<u64>,
    unix_inode: Option<u64>,
    unix_changed_nanos: Option<i128>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LandtypeMaxlcCacheRecord {
    version: u32,
    source: LandtypeMaxlcIdentity,
    maxlc: i32,
    checksum: u64,
}

fn landtype_maxlc_cache() -> &'static Mutex<HashMap<LandtypeMaxlcIdentity, i32>> {
    static CACHE: OnceLock<Mutex<HashMap<LandtypeMaxlcIdentity, i32>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn landtype_maxlc_cache_identity(path: &Path) -> io::Result<Option<LandtypeMaxlcIdentity>> {
    let metadata = std::fs::metadata(path)?;
    let Some(modified_unix_nanos) = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
    else {
        return Ok(None);
    };
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    #[cfg(unix)]
    let (unix_device, unix_inode, unix_changed_nanos) = {
        use std::os::unix::fs::MetadataExt;
        (
            Some(metadata.dev()),
            Some(metadata.ino()),
            Some(i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec())),
        )
    };
    #[cfg(not(unix))]
    let (unix_device, unix_inode, unix_changed_nanos) = (None, None, None);
    Ok(Some(LandtypeMaxlcIdentity {
        canonical_path: canonical,
        source_len: metadata.len(),
        modified_unix_nanos,
        unix_device,
        unix_inode,
        unix_changed_nanos,
    }))
}

fn open_landtype_netcdf_with<F>(
    path: &Path,
    opener: F,
) -> io::Result<(netcdf::File, Option<LandtypeMaxlcIdentity>)>
where
    F: FnOnce(&Path) -> io::Result<netcdf::File>,
{
    // Bind the path identity to the handle. Capturing identity only after open
    // can associate an already-open old inode with a newly atomically replaced
    // path and then cache the old maxlc under the new file's identity.
    let identity_before = landtype_maxlc_cache_identity(path)?;
    let file = opener(path)?;
    let identity_after = landtype_maxlc_cache_identity(path)?;
    if identity_before != identity_after {
        return Err(io::Error::other(
            "landtype source changed while opening NetCDF",
        ));
    }
    Ok((file, identity_before))
}

fn open_landtype_netcdf(path: &Path) -> io::Result<(netcdf::File, Option<LandtypeMaxlcIdentity>)> {
    open_landtype_netcdf_with(path, |path| {
        crate::open_netcdf(path).map_err(crate::netcdf_to_io_error)
    })
}

fn stable_landtype_cache_key(path: &Path) -> u64 {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes()
    };
    #[cfg(not(unix))]
    let path_text = path.to_string_lossy();
    #[cfg(not(unix))]
    let bytes = path_text.as_bytes();

    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn landtype_maxlc_cache_directories() -> Vec<PathBuf> {
    #[cfg(test)]
    {
        vec![std::env::temp_dir().join("earthmesh-hfield-maxlc-test-cache")]
    }
    #[cfg(not(test))]
    {
        let mut directories = Vec::new();
        #[cfg(target_os = "macos")]
        if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
            directories.push(PathBuf::from(home).join("Library/Caches/EarthMesh"));
        }
        #[cfg(target_os = "linux")]
        if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
            directories.push(PathBuf::from(cache).join("earthmesh"));
        } else if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
            directories.push(PathBuf::from(home).join(".cache/earthmesh"));
        }
        let fallback = std::env::temp_dir().join("earthmesh-cache");
        if !directories.contains(&fallback) {
            directories.push(fallback);
        }
        directories
    }
}

fn landtype_maxlc_cache_path(directory: &Path, source: &LandtypeMaxlcIdentity) -> PathBuf {
    directory.join(format!(
        "landtype-maxlc-v{LANDTYPE_MAXLC_CACHE_VERSION}-{:016x}.json",
        stable_landtype_cache_key(&source.canonical_path)
    ))
}

fn landtype_maxlc_checksum(source: &LandtypeMaxlcIdentity, maxlc: i32) -> u64 {
    let mut hasher = DefaultHasher::new();
    LANDTYPE_MAXLC_CACHE_VERSION.hash(&mut hasher);
    source.hash(&mut hasher);
    maxlc.hash(&mut hasher);
    hasher.finish()
}

fn read_landtype_maxlc_cache(source: &LandtypeMaxlcIdentity) -> Option<i32> {
    landtype_maxlc_cache_directories()
        .into_iter()
        .find_map(|directory| {
            let contents = std::fs::read(landtype_maxlc_cache_path(&directory, source)).ok()?;
            let record = serde_json::from_slice::<LandtypeMaxlcCacheRecord>(&contents).ok()?;
            (record.version == LANDTYPE_MAXLC_CACHE_VERSION
                && record.source == *source
                && (0..=i32::from(i8::MAX)).contains(&record.maxlc)
                && record.checksum == landtype_maxlc_checksum(source, record.maxlc))
            .then_some(record.maxlc)
        })
}

fn write_landtype_maxlc_cache(source: &LandtypeMaxlcIdentity, maxlc: i32) -> io::Result<()> {
    static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

    let record = LandtypeMaxlcCacheRecord {
        version: LANDTYPE_MAXLC_CACHE_VERSION,
        source: source.clone(),
        maxlc,
        checksum: landtype_maxlc_checksum(source, maxlc),
    };
    let mut last_error = None;
    for directory in landtype_maxlc_cache_directories() {
        let result = (|| {
            std::fs::create_dir_all(&directory)?;
            let cache = landtype_maxlc_cache_path(&directory, source);
            let mut temp_name = cache
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("earthmesh-maxlc"))
                .to_os_string();
            temp_name.push(format!(
                ".{}.{}.{}.tmp",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_nanos()),
                NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let temp = cache.with_file_name(temp_name);
            let write_result = (|| {
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temp)?;
                serde_json::to_writer(&mut file, &record).map_err(io::Error::other)?;
                file.write_all(b"\n")?;
                file.sync_all()?;
                drop(file);
                std::fs::rename(&temp, &cache)
            })();
            if write_result.is_err() {
                let _ = std::fs::remove_file(temp);
            }
            write_result
        })();
        match result {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("no landtype cache directory available")))
}

#[cfg(test)]
fn landtype_maxlc_scan_counts() -> &'static Mutex<HashMap<LandtypeMaxlcIdentity, usize>> {
    static COUNTS: OnceLock<Mutex<HashMap<LandtypeMaxlcIdentity, usize>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn landtype_global_maxlc(
    path: &Path,
    source_identity: Option<&LandtypeMaxlcIdentity>,
    variable: &netcdf::Variable<'_>,
    lat_lon: bool,
    src_nlon: usize,
    src_nlat: usize,
    missing: &[f64],
) -> io::Result<i32> {
    if let Some(source) = source_identity {
        if let Some(maxlc) = landtype_maxlc_cache()
            .lock()
            .map_err(|_| io::Error::other("landtype maxlc cache lock poisoned"))?
            .get(source)
            .copied()
        {
            return Ok(maxlc);
        }
        if let Some(maxlc) = read_landtype_maxlc_cache(source) {
            landtype_maxlc_cache()
                .lock()
                .map_err(|_| io::Error::other("landtype maxlc cache lock poisoned"))?
                .insert(source.clone(), maxlc);
            return Ok(maxlc);
        }
        #[cfg(test)]
        {
            *landtype_maxlc_scan_counts()
                .lock()
                .expect("landtype maxlc scan-count lock")
                .entry(source.clone())
                .or_default() += 1;
        }
    }

    let tile_lon = netcdf_longitude_tile_size(variable, lat_lon, src_nlon, 256);
    let mut maxlc = 0_i32;
    let mut has_valid = false;
    for lon_start in (0..src_nlon).step_by(tile_lon) {
        let lon_count = tile_lon.min(src_nlon - lon_start);
        for value in landtype_tile(variable, lat_lon, lon_start, lon_count, 0, src_nlat)? {
            if is_missing_numeric(value, missing) {
                continue;
            }
            if value < 0 {
                return Err(invalid(format!(
                    "landtype value {value} must be non-negative"
                )));
            }
            has_valid = true;
            maxlc = maxlc.max(i32::from(value));
        }
    }
    if !has_valid {
        return Err(invalid("landtype contains no valid values".to_string()));
    }
    if let Some(source) = source_identity {
        if landtype_maxlc_cache_identity(path)?.as_ref() != Some(source) {
            return Err(io::Error::other(
                "landtype source changed while scanning global maxlc",
            ));
        }
        landtype_maxlc_cache()
            .lock()
            .map_err(|_| io::Error::other("landtype maxlc cache lock poisoned"))?
            .insert(source.clone(), maxlc);
        // If every user/temp cache directory is unavailable, retain the exact
        // value in-process and retry persistence on the next CLI invocation.
        let _ = write_landtype_maxlc_cache(source, maxlc);
    }
    Ok(maxlc)
}

fn active_hfield_axes(field: &HField, domain: Option<&HfieldDomainMask>) -> (Vec<bool>, Vec<bool>) {
    domain.map_or_else(
        || (vec![true; field.nlon()], vec![true; field.nlat()]),
        |domain| {
            let mut active_lon = vec![false; field.nlon()];
            let mut active_lat = vec![false; field.nlat()];
            for (i, lon_active) in active_lon.iter_mut().enumerate() {
                for (j, lat_active) in active_lat.iter_mut().enumerate() {
                    if domain.is_active(i, j) {
                        *lon_active = true;
                        *lat_active = true;
                    }
                }
            }
            (active_lon, active_lat)
        },
    )
}

fn active_source_latitude_window(
    field: &HField,
    active_lat: &[bool],
    src_nlat: usize,
    latitude_order: LatitudeOrder,
) -> io::Result<(usize, usize, usize, usize)> {
    let mut canonical_start = None;
    let mut canonical_end = 0usize;
    let mut include_source_row = |src_j: usize| {
        canonical_start = Some(canonical_start.map_or(src_j, |start: usize| start.min(src_j)));
        canonical_end = canonical_end.max(src_j);
    };
    for src_j in 0..src_nlat {
        let (_, field_j) = landtype_hfield_indices(0.0, src_j, src_nlat, field);
        if active_lat[field_j] {
            include_source_row(src_j);
        }
    }
    // A finer HField row may contain no coarser source center. Include the
    // nearest source row needed by the dense fallback instead of rejecting a
    // valid regional domain before that fallback can run.
    for (field_j, active) in active_lat.iter().copied().enumerate() {
        if active {
            include_source_row(scaled_hfield_center_index(
                field.lat_center(field_j),
                src_nlat,
                false,
            ));
        }
    }
    let canonical_start = canonical_start.ok_or_else(|| {
        invalid("regional HField domain contains no active source rows".to_string())
    })?;
    let count = canonical_end - canonical_start + 1;
    let file_start = match latitude_order {
        LatitudeOrder::NorthToSouth => canonical_start,
        LatitudeOrder::SouthToNorth => src_nlat - 1 - canonical_end,
    };
    Ok((canonical_start, canonical_end, file_start, count))
}

/// Stream a production landtype raster directly into fixed-size HField bins.
/// Regional domains read only the source chunks intersecting their active
/// HField rows/columns instead of decompressing the entire global raster.
/// Memory is O(hfield cells + one tile), independent of source resolution.
fn read_landtype_source_for_hfield(
    path: &Path,
    field: &HField,
    domain: Option<&HfieldDomainMask>,
) -> io::Result<LandtypeBinStats> {
    let (file, source_identity) = open_landtype_netcdf(path)?;
    let variable = file
        .variable("landtype")
        .ok_or_else(|| invalid("missing landtype variable".to_string()))?;
    let (lat_lon, src_nlon, src_nlat, latitude_order, longitudes) =
        landtype_source_layout(&file, &variable)?;
    let missing = numeric_missing_values(&variable)?;
    let (active_lon, active_lat) = active_hfield_axes(field, domain);
    if !active_lon.iter().any(|active| *active) || !active_lat.iter().any(|active| *active) {
        return Ok(LandtypeBinStats::new(field, domain));
    }
    let (_, _, lat_start, lat_count) =
        active_source_latitude_window(field, &active_lat, src_nlat, latitude_order)?;
    let active_local_lat = (0..lat_count)
        .filter_map(|local_file_j| {
            let file_j = lat_start + local_file_j;
            let src_j = canonical_latitude_index(latitude_order, file_j, src_nlat);
            let (_, field_j) = landtype_hfield_indices(0.0, src_j, src_nlat, field);
            active_lat[field_j].then_some((local_file_j, field_j))
        })
        .collect::<Vec<_>>();
    // Match NetCDF-4's longitude chunk width when available. Re-reading narrow
    // stripes inside a large compressed chunk can otherwise decompress the same
    // data dozens of times.
    let tile_lon = netcdf_longitude_tile_size(&variable, lat_lon, src_nlon, 256);
    let mut bins = LandtypeBinStats::new(field, domain);
    let maxlc = landtype_global_maxlc(
        path,
        source_identity.as_ref(),
        &variable,
        lat_lon,
        src_nlon,
        src_nlat,
        &missing,
    )?;
    let mut has_valid = false;
    for lon_start in (0..src_nlon).step_by(tile_lon) {
        let lon_count = tile_lon.min(src_nlon - lon_start);
        let active_local_lon = (0..lon_count)
            .filter_map(|local_i| {
                let lon = source_longitude(lon_start + local_i, src_nlon, longitudes.as_deref());
                let (field_i, _) = landtype_hfield_indices(lon, 0, src_nlat, field);
                active_lon[field_i].then_some((local_i, field_i))
            })
            .collect::<Vec<_>>();
        if active_local_lon.is_empty() {
            continue;
        }
        let raw = landtype_tile(
            &variable, lat_lon, lon_start, lon_count, lat_start, lat_count,
        )?;
        crate::require_len("landtype tile", raw.len(), lon_count * lat_count)?;
        for (local_i, field_i) in active_local_lon {
            for &(local_file_j, field_j) in &active_local_lat {
                let raw_index = if lat_lon {
                    local_file_j * lon_count + local_i
                } else {
                    local_i * lat_count + local_file_j
                };
                let value = raw[raw_index];
                if is_missing_numeric(value, &missing) {
                    continue;
                }
                if domain.is_some_and(|domain| !domain.is_active(field_i, field_j)) {
                    continue;
                }
                has_valid = true;
                let out = field_i * field.nlat() + field_j;
                bins.record(out, i32::from(value))?;
            }
        }
    }
    // Preserve dense categorical behavior when the HField is finer than the
    // LandType raster (or an active longitude column contains no source
    // center): empty active bins inherit their nearest source class.
    let mut nearest_by_i = std::collections::BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for i in 0..field.nlon() {
        let src_i = nearest_longitude_index(field.lon_center(i), src_nlon, longitudes.as_deref());
        for j in 0..field.nlat() {
            if domain.is_some_and(|domain| !domain.is_active(i, j)) {
                continue;
            }
            let out = i * field.nlat() + j;
            if bins.total_at(out) == 0 {
                let src_j = scaled_hfield_center_index(field.lat_center(j), src_nlat, false);
                nearest_by_i.entry(src_i).or_default().push((src_j, out));
            }
        }
    }
    for (src_i, targets) in nearest_by_i {
        let row = landtype_tile(&variable, lat_lon, src_i, 1, 0, src_nlat)?;
        for (src_j, out) in targets {
            let file_j = file_latitude_index(latitude_order, src_j, src_nlat);
            let value = row[file_j];
            if is_missing_numeric(value, &missing) {
                continue;
            }
            bins.record(out, i32::from(value))?;
            has_valid = true;
        }
    }
    if !has_valid {
        return Err(invalid("landtype contains no valid values".to_string()));
    }
    bins.exclude_class(maxlc);
    Ok(bins)
}

#[derive(Clone, Debug)]
struct LandtypeMaskSource {
    path: std::path::PathBuf,
    nlon: usize,
    nlat: usize,
    lat_lon: bool,
    latitude_order: LatitudeOrder,
    longitudes: Option<Vec<f64>>,
    missing: Vec<f64>,
    maxlc: i32,
}

impl LandtypeMaskSource {
    fn excludes(&self, value: i8) -> bool {
        i32::from(value) == self.maxlc || is_missing_numeric(value, &self.missing)
    }
}

fn read_landtype_mask_source_for_hfield(path: &Path) -> io::Result<LandtypeMaskSource> {
    let (file, source_identity) = open_landtype_netcdf(path)?;
    let variable = file
        .variable("landtype")
        .ok_or_else(|| invalid("missing landtype variable".to_string()))?;
    let (lat_lon, src_nlon, src_nlat, latitude_order, longitudes) =
        landtype_source_layout(&file, &variable)?;
    let missing = numeric_missing_values(&variable)?;
    let maxlc = landtype_global_maxlc(
        path,
        source_identity.as_ref(),
        &variable,
        lat_lon,
        src_nlon,
        src_nlat,
        &missing,
    )?;
    Ok(LandtypeMaskSource {
        path: path.to_path_buf(),
        nlon: src_nlon,
        nlat: src_nlat,
        lat_lon,
        latitude_order,
        longitudes,
        missing,
        maxlc,
    })
}

#[cfg(test)]
fn apply_landtype_basic_thresholds_from_source(
    field: &mut HField,
    landtypes: &[Vec<i32>],
    maxlc: i32,
    refine: &RefineConfig,
    mesh_type: &str,
    h_inside: f64,
) -> io::Result<usize> {
    let src_nlon = landtypes.len().checked_sub(1).ok_or_else(|| {
        invalid("landtype source must include a Canonical placeholder row".to_string())
    })?;
    let src_nlat = landtypes
        .get(1)
        .and_then(|row| row.len().checked_sub(1))
        .ok_or_else(|| {
            invalid("landtype source must include a Canonical placeholder column".to_string())
        })?;
    for row in landtypes.iter().skip(1) {
        if row.len() != src_nlat + 1 {
            return Err(invalid(
                "landtype source rows must have equal width".to_string(),
            ));
        }
    }

    let mut bins = LandtypeBinStats::new(field, None);

    for src_i in 0..src_nlon {
        let lon = source_longitude(src_i, src_nlon, None);
        for src_j in 0..src_nlat {
            let out = landtype_hfield_bin(lon, src_j, src_nlat, field);
            bins.record(out, landtypes[src_i + 1][src_j + 1])?;
        }
    }
    bins.exclude_class(maxlc);

    apply_landtype_basic_thresholds_from_bins(field, &bins, refine, mesh_type, h_inside, None)
}

fn apply_landtype_basic_thresholds_from_bins(
    field: &mut HField,
    bins: &LandtypeBinStats,
    refine: &RefineConfig,
    mesh_type: &str,
    h_inside: f64,
    domain: Option<&HfieldDomainMask>,
) -> io::Result<usize> {
    let len = field.nlon() * field.nlat();
    if bins.hfield_len != len || bins.slot_by_hfield.len() != len {
        return Err(invalid(
            "landtype HField bin count does not match the target field".to_string(),
        ));
    }

    let mut applied = 0usize;
    if has_land_thresholds(refine, mesh_type) && refine.refine_num_landtypes {
        let active = (0..len)
            .map(|idx| bins.distinct_at(idx) as i32 > refine.th_num_landtypes)
            .collect::<Vec<_>>();
        min_with_bool_matrix(field, &active, h_inside, domain);
        applied += 1;
    }
    if has_land_thresholds(refine, mesh_type) && refine.refine_area_mainland {
        let active = (0..len)
            .map(|idx| {
                bins.land_at(idx) > 0
                    && (bins.max_class_count_at(idx) as f64 / bins.land_at(idx) as f64)
                        < refine.th_area_mainland
            })
            .collect::<Vec<_>>();
        min_with_bool_matrix(field, &active, h_inside, domain);
        applied += 1;
    }
    if has_ocean_thresholds(refine, mesh_type) {
        let active = (0..len)
            .map(|idx| {
                bins.total_at(idx) > 0 && {
                    let ratio = bins.ocean_at(idx) as f64 / bins.total_at(idx) as f64;
                    ratio > refine.th_sea_ratio[0] && ratio < refine.th_sea_ratio[1]
                }
            })
            .collect::<Vec<_>>();
        min_with_bool_matrix(field, &active, h_inside, domain);
        applied += 1;
    }
    Ok(applied)
}

pub(crate) fn build_composed_hfield(
    regions: &[MethodCRefinementRegion],
    refine: &RefineConfig,
    mesh_type: &str,
    config: Option<&EarthmeshConfig>,
    base_m: f64,
    options: &HfieldRefineOptions,
    threshold_level: usize,
    domain: Option<&GridRegion>,
) -> io::Result<HField> {
    let domain = domain.map(|domain| HfieldDomainMask::new(options.nlon, options.nlat, domain));
    let mut field = build_hfield_from_regions_in_domain(
        regions,
        base_m,
        options.g,
        options.nlon,
        options.nlat,
        domain.as_ref(),
    )?;
    if refine.refine_cal {
        let needs_landtype_mask = has_mean_threshold_hfield_sources(refine, mesh_type)
            || !enabled_std_threshold_field_specs(refine, mesh_type).is_empty();
        let landtype_mask = if needs_landtype_mask {
            hfield_landtype_mask_source(config)?
        } else {
            None
        };
        let mut threshold_stats_cache = ThresholdStatsCache::new();
        apply_mean_threshold_hfield_contributions_with_landtype_mask(
            &mut field,
            refine,
            mesh_type,
            base_m,
            threshold_level,
            options.g,
            landtype_mask.as_ref(),
            domain.as_ref(),
            &mut threshold_stats_cache,
        )?;
        apply_std_threshold_hfield_contributions_with_landtype_mask(
            &mut field,
            refine,
            mesh_type,
            base_m,
            threshold_level,
            options.g,
            landtype_mask.as_ref(),
            domain.as_ref(),
            &mut threshold_stats_cache,
        )?;
        apply_landtype_basic_threshold_hfield_contributions(
            &mut field,
            refine,
            mesh_type,
            config,
            base_m,
            threshold_level,
            options.g,
            domain.as_ref(),
        )?;
    }
    Ok(field)
}

/// Keep refinement sources local to the requested output domain while leaving
/// the Method-C transition apron free to extend into the surrounding parent
/// mesh. Regional outputs discard that surrounding mesh after refinement, so
/// refining unrelated global threshold features is both wasteful and unsafe.
pub(crate) fn constrain_hfield_to_domain(
    field: &mut HField,
    domain: Option<&GridRegion>,
    base_m: f64,
    g: f64,
) -> io::Result<()> {
    let Some(domain) = domain else {
        return Ok(());
    };
    let domain = HfieldDomainMask::new(field.nlon(), field.nlat(), domain);
    for j in 0..field.nlat() {
        for i in 0..field.nlon() {
            if !domain.is_active(i, j) {
                field.set(i, j, base_m);
            }
        }
    }
    field.limit_gradient(g)?;
    Ok(())
}

#[derive(Clone, Debug)]
struct ThresholdStats {
    mean: Vec<f64>,
    stddev: Vec<f64>,
}

type ThresholdStatsCache = HashMap<(String, String), ThresholdStats>;

fn read_threshold_stats_on_hfield_masked(
    file: &netcdf::File,
    name: &str,
    field: &HField,
    landtype_mask: Option<&LandtypeMaskSource>,
    domain: Option<&HfieldDomainMask>,
) -> io::Result<ThresholdStats> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let dims = variable.dimensions();
    if dims.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} must be 2-D"),
        ));
    }
    let names = dims
        .iter()
        .map(|dimension| dimension.name().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let lengths = dims
        .iter()
        .map(|dimension| dimension.len())
        .collect::<Vec<_>>();
    let lat_lon = is_lat_dim(&names[0]) && is_lon_dim(&names[1]);
    let lon_lat = is_lon_dim(&names[0]) && is_lat_dim(&names[1]);
    if !lat_lon && !lon_lat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} dimensions {names:?} must identify longitude and latitude axes"),
        ));
    }
    let (src_nlon, src_nlat) = if lat_lon {
        (lengths[1], lengths[0])
    } else {
        (lengths[0], lengths[1])
    };
    if src_nlon == 0 || src_nlat == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} dimensions must be non-empty"),
        ));
    }
    let latitude_order =
        threshold_latitude_order(file, &dims[usize::from(!lat_lon)].name(), src_nlat)?;
    let longitudes =
        threshold_longitude_coordinates(file, &dims[usize::from(lat_lon)].name(), src_nlon)?;
    let (active_lon, active_lat) = active_hfield_axes(field, domain);
    let len = field.nlon() * field.nlat();
    if !active_lon.iter().any(|active| *active) || !active_lat.iter().any(|active| *active) {
        return Ok(ThresholdStats {
            mean: vec![0.0; len],
            stddev: vec![0.0; len],
        });
    }
    let (canonical_lat_start, canonical_lat_end, lat_start, lat_count) =
        active_source_latitude_window(field, &active_lat, src_nlat, latitude_order)?;
    let active_local_lat = (0..lat_count)
        .filter_map(|local_file_j| {
            let file_j = lat_start + local_file_j;
            let src_j = canonical_latitude_index(latitude_order, file_j, src_nlat);
            let (_, field_j) = landtype_hfield_indices(0.0, src_j, src_nlat, field);
            active_lat[field_j].then_some((local_file_j, src_j, field_j))
        })
        .collect::<Vec<_>>();
    let mask_file = landtype_mask
        .map(|mask| crate::open_netcdf(&mask.path).map_err(crate::netcdf_to_io_error))
        .transpose()?;
    let mask_variable = mask_file
        .as_ref()
        .map(|mask_file| {
            mask_file
                .variable("landtype")
                .ok_or_else(|| invalid("missing landtype variable".to_string()))
        })
        .transpose()?;
    let mask_lat_window = landtype_mask.map(|mask| {
        let mask_canonical_start = scaled_source_index(canonical_lat_start, src_nlat, mask.nlat);
        let mask_canonical_end = scaled_source_index(canonical_lat_end, src_nlat, mask.nlat);
        let mask_lat_count = mask_canonical_end - mask_canonical_start + 1;
        let mask_lat_start = match mask.latitude_order {
            LatitudeOrder::NorthToSouth => mask_canonical_start,
            LatitudeOrder::SouthToNorth => mask.nlat - 1 - mask_canonical_end,
        };
        (mask_lat_start, mask_lat_count)
    });

    let mut count = vec![0usize; len];
    let mut sum = vec![0.0; len];
    let mut sumsq = vec![0.0; len];
    let tile_lon = netcdf_longitude_tile_size(&variable, lat_lon, src_nlon, 128);
    for lon_start in (0..src_nlon).step_by(tile_lon) {
        let lon_count = tile_lon.min(src_nlon - lon_start);
        let active_local_lon = (0..lon_count)
            .filter_map(|local_i| {
                let src_i = lon_start + local_i;
                let lon = source_longitude(src_i, src_nlon, longitudes.as_deref());
                let (field_i, _) = landtype_hfield_indices(lon, 0, src_nlat, field);
                active_lon[field_i].then_some((local_i, src_i, field_i))
            })
            .collect::<Vec<_>>();
        if active_local_lon.is_empty() {
            continue;
        }
        let raw = threshold_tile(
            &variable, lat_lon, lon_start, lon_count, lat_start, lat_count, name,
        )?;
        crate::require_len(name, raw.len(), lon_count * lat_count)?;
        let mask_window =
            if let (Some(mask), Some(mask_variable)) = (landtype_mask, mask_variable.as_ref()) {
                let indices = (0..lon_count)
                    .map(|local_i| {
                        let lon =
                            source_longitude(lon_start + local_i, src_nlon, longitudes.as_deref());
                        nearest_longitude_index(lon, mask.nlon, mask.longitudes.as_deref())
                    })
                    .collect::<Vec<_>>();
                let min_i = *indices.iter().min().expect("non-empty threshold tile");
                let max_i = *indices.iter().max().expect("non-empty threshold tile");
                let ranges = if max_i - min_i <= mask.nlon / 2 {
                    vec![(min_i, max_i - min_i + 1)]
                } else {
                    let low_max = *indices
                        .iter()
                        .filter(|index| **index < mask.nlon / 2)
                        .max()
                        .expect("wrapped mask tile has low indices");
                    let high_min = *indices
                        .iter()
                        .filter(|index| **index >= mask.nlon / 2)
                        .min()
                        .expect("wrapped mask tile has high indices");
                    vec![(0, low_max + 1), (high_min, mask.nlon - high_min)]
                };
                let mut windows = Vec::with_capacity(ranges.len());
                let (mask_lat_start, mask_lat_count) =
                    mask_lat_window.expect("mask latitude window exists");
                for (start, count) in ranges {
                    windows.push((
                        start,
                        count,
                        landtype_tile(
                            mask_variable,
                            mask.lat_lon,
                            start,
                            count,
                            mask_lat_start,
                            mask_lat_count,
                        )?,
                    ));
                }
                Some((indices, windows, mask_lat_start, mask_lat_count))
            } else {
                None
            };

        for &(local_i, _src_i, field_i) in &active_local_lon {
            for &(local_file_j, src_j, field_j) in &active_local_lat {
                if domain.is_some_and(|domain| !domain.is_active(field_i, field_j)) {
                    continue;
                }
                if let (Some(mask), Some((mask_indices, windows, mask_lat_start, mask_lat_count))) =
                    (landtype_mask, mask_window.as_ref())
                {
                    let mask_i = mask_indices[local_i];
                    let mask_j = scaled_source_index(src_j, src_nlat, mask.nlat);
                    let file_mask_j = file_latitude_index(mask.latitude_order, mask_j, mask.nlat);
                    let (first_i, mask_nlon, values) = windows
                        .iter()
                        .find(|(first_i, mask_nlon, _)| {
                            (*first_i..*first_i + *mask_nlon).contains(&mask_i)
                        })
                        .expect("mask longitude index belongs to a loaded window");
                    let local_mask_i = mask_i - *first_i;
                    let local_file_mask_j = file_mask_j - *mask_lat_start;
                    let mask_index = if mask.lat_lon {
                        local_file_mask_j * *mask_nlon + local_mask_i
                    } else {
                        local_mask_i * *mask_lat_count + local_file_mask_j
                    };
                    if mask.excludes(values[mask_index]) {
                        continue;
                    }
                }
                let raw_index = if lat_lon {
                    local_file_j * lon_count + local_i
                } else {
                    local_i * lat_count + local_file_j
                };
                let out = field_i * field.nlat() + field_j;
                let value = raw[raw_index];
                count[out] += 1;
                sum[out] += value;
                sumsq[out] += value * value;
            }
        }
    }

    let mut mean = vec![0.0; len];
    let mut stddev = vec![0.0; len];
    for out in 0..len {
        if count[out] > 0 {
            mean[out] = sum[out] / count[out] as f64;
            let variance = sumsq[out] / count[out] as f64 - mean[out] * mean[out];
            stddev[out] = variance.max(0.0).sqrt();
        }
    }

    // Preserve the dense compatibility behavior when the HField is finer than
    // the threshold raster: empty bins inherit their nearest source value.
    let mut nearest_by_i = std::collections::BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for i in 0..field.nlon() {
        let src_i = nearest_longitude_index(field.lon_center(i), src_nlon, longitudes.as_deref());
        for j in 0..field.nlat() {
            if domain.is_some_and(|domain| !domain.is_active(i, j)) {
                continue;
            }
            let out = i * field.nlat() + j;
            if count[out] == 0 {
                let src_j = scaled_hfield_center_index(field.lat_center(j), src_nlat, false);
                nearest_by_i.entry(src_i).or_default().push((src_j, out));
            }
        }
    }
    for (src_i, targets) in nearest_by_i {
        let row = threshold_tile(&variable, lat_lon, src_i, 1, 0, src_nlat, name)?;
        let mask_row =
            if let (Some(mask), Some(mask_variable)) = (landtype_mask, mask_variable.as_ref()) {
                let lon = source_longitude(src_i, src_nlon, longitudes.as_deref());
                let mask_i = nearest_longitude_index(lon, mask.nlon, mask.longitudes.as_deref());
                Some(landtype_tile(
                    mask_variable,
                    mask.lat_lon,
                    mask_i,
                    1,
                    0,
                    mask.nlat,
                )?)
            } else {
                None
            };
        for (src_j, out) in targets {
            if let (Some(mask), Some(mask_row)) = (landtype_mask, mask_row.as_ref()) {
                let mask_j = scaled_source_index(src_j, src_nlat, mask.nlat);
                let file_mask_j = file_latitude_index(mask.latitude_order, mask_j, mask.nlat);
                if mask.excludes(mask_row[file_mask_j]) {
                    continue;
                }
            }
            let file_j = match latitude_order {
                LatitudeOrder::NorthToSouth => src_j,
                LatitudeOrder::SouthToNorth => src_nlat - 1 - src_j,
            };
            mean[out] = row[file_j];
        }
    }

    Ok(ThresholdStats { mean, stddev })
}

fn threshold_tile(
    variable: &netcdf::Variable<'_>,
    lat_lon: bool,
    lon_start: usize,
    lon_count: usize,
    lat_start: usize,
    lat_count: usize,
    name: &str,
) -> io::Result<Vec<f64>> {
    let extent = if lat_lon {
        (
            lat_start..lat_start + lat_count,
            lon_start..lon_start + lon_count,
        )
    } else {
        (
            lon_start..lon_start + lon_count,
            lat_start..lat_start + lat_count,
        )
    };
    let values = if let Ok(values) = variable.get_values::<f64, _>(extent.clone()) {
        values
    } else if let Ok(values) = variable.get_values::<f32, _>(extent) {
        values.into_iter().map(f64::from).collect()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} variable must be readable as f64 or f32"),
        ));
    };
    reject_invalid_threshold_values(variable, name, &values)?;
    Ok(values)
}

fn scaled_source_index(index: usize, source_len: usize, target_len: usize) -> usize {
    (((index as u128 * 2 + 1) * target_len as u128) / (source_len as u128 * 2))
        .min((target_len - 1) as u128) as usize
}

fn scaled_hfield_center_index(value: f64, source_len: usize, longitude: bool) -> usize {
    let fraction = if longitude {
        (earthmesh_hfield::wrap_lon_degrees(value) + 180.0) / 360.0
    } else {
        (90.0 - value) / 180.0
    };
    (fraction * source_len as f64)
        .floor()
        .clamp(0.0, (source_len - 1) as f64) as usize
}

#[cfg(test)]
fn threshold_stats_on_hfield_from_source(
    source: &[f64],
    src_nlon: usize,
    src_nlat: usize,
    field: &HField,
) -> ThresholdStats {
    threshold_stats_on_hfield_from_source_masked(source, src_nlon, src_nlat, field, None)
}

#[cfg(test)]
fn threshold_stats_on_hfield_from_source_masked(
    source: &[f64],
    src_nlon: usize,
    src_nlat: usize,
    field: &HField,
    landtype_mask: Option<&(Vec<Vec<i32>>, i32)>,
) -> ThresholdStats {
    let mut count = vec![0usize; field.nlon() * field.nlat()];
    let mut sum = vec![0.0; field.nlon() * field.nlat()];
    let mut sumsq = vec![0.0; field.nlon() * field.nlat()];
    for src_i in 0..src_nlon {
        let lon = -180.0 + (src_i as f64 + 0.5) * 360.0 / src_nlon as f64;
        let i = (((earthmesh_hfield::wrap_lon_degrees(lon) + 180.0) / 360.0) * field.nlon() as f64)
            .floor()
            .clamp(0.0, (field.nlon() - 1) as f64) as usize;
        for src_j in 0..src_nlat {
            let lat = 90.0 - (src_j as f64 + 0.5) * 180.0 / src_nlat as f64;
            if threshold_landtype_is_maxlc(lon, lat, landtype_mask) {
                continue;
            }
            let j = (((lat + 90.0) / 180.0) * field.nlat() as f64)
                .floor()
                .clamp(0.0, (field.nlat() - 1) as f64) as usize;
            let out = i * field.nlat() + j;
            let value = source[src_i * src_nlat + src_j];
            count[out] += 1;
            sum[out] += value;
            sumsq[out] += value * value;
        }
    }

    let mut mean = vec![0.0; field.nlon() * field.nlat()];
    let mut stddev = vec![0.0; field.nlon() * field.nlat()];
    for i in 0..field.nlon() {
        for j in 0..field.nlat() {
            let out = i * field.nlat() + j;
            if count[out] > 0 {
                mean[out] = sum[out] / count[out] as f64;
                let variance = (sumsq[out] / count[out] as f64) - mean[out] * mean[out];
                stddev[out] = variance.max(0.0).sqrt();
            }
        }
    }

    // If the hfield is finer than the source, empty cells inherit nearest-source
    // mean and std=0. No new dependency, and no silent shape requirement.
    for i in 0..field.nlon() {
        let src_i = (((earthmesh_hfield::wrap_lon_degrees(field.lon_center(i)) + 180.0) / 360.0)
            * src_nlon as f64)
            .floor()
            .clamp(0.0, (src_nlon - 1) as f64) as usize;
        for j in 0..field.nlat() {
            let out = i * field.nlat() + j;
            if count[out] > 0 {
                continue;
            }
            let src_j = (((field.lat_center(j) + 90.0) / 180.0) * src_nlat as f64)
                .floor()
                .clamp(0.0, (src_nlat - 1) as f64) as usize;
            mean[out] = source[src_i * src_nlat + src_j];
        }
    }

    ThresholdStats { mean, stddev }
}

#[cfg(test)]
fn threshold_landtype_is_maxlc(
    lon: f64,
    lat: f64,
    landtype_mask: Option<&(Vec<Vec<i32>>, i32)>,
) -> bool {
    let Some((landtypes, maxlc)) = landtype_mask else {
        return false;
    };
    let Some(nlon) = landtypes.len().checked_sub(1) else {
        return false;
    };
    let Some(nlat) = landtypes.get(1).and_then(|row| row.len().checked_sub(1)) else {
        return false;
    };
    if nlon == 0 || nlat == 0 {
        return false;
    }
    let i = (((earthmesh_hfield::wrap_lon_degrees(lon) + 180.0) / 360.0) * nlon as f64)
        .floor()
        .clamp(0.0, (nlon - 1) as f64) as usize
        + 1;
    let j = (((90.0 - lat) / 180.0) * nlat as f64)
        .floor()
        .clamp(0.0, (nlat - 1) as f64) as usize
        + 1;
    landtypes.get(i).and_then(|row| row.get(j)).copied() == Some(*maxlc)
}

fn is_lon_dim(name: &str) -> bool {
    is_axis_dim(name, &["lon", "longitude"], "x")
}

fn is_lat_dim(name: &str) -> bool {
    is_axis_dim(name, &["lat", "latitude"], "y")
}

fn is_axis_dim(name: &str, aliases: &[&str], short_axis: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    if normalized == short_axis || aliases.contains(&normalized.as_str()) {
        return true;
    }
    normalized
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| aliases.contains(&token))
}

fn min_with_threshold_matrix(
    field: &mut HField,
    values: &[f64],
    threshold: f64,
    h_inside: f64,
    domain: Option<&HfieldDomainMask>,
) {
    let nlon = field.nlon();
    let nlat = field.nlat();
    field.min_with_fn(|lon, lat| {
        let i = (((earthmesh_hfield::wrap_lon_degrees(lon) + 180.0) / 360.0) * nlon as f64)
            .floor()
            .clamp(0.0, (nlon - 1) as f64) as usize;
        let j = (((lat + 90.0) / 180.0) * nlat as f64)
            .floor()
            .clamp(0.0, (nlat - 1) as f64) as usize;
        if domain.is_none_or(|domain| domain.contains(lon, lat)) && values[i * nlat + j] > threshold
        {
            h_inside
        } else {
            f64::INFINITY
        }
    });
}

fn min_with_bool_matrix(
    field: &mut HField,
    active: &[bool],
    h_inside: f64,
    domain: Option<&HfieldDomainMask>,
) {
    let nlon = field.nlon();
    let nlat = field.nlat();
    field.min_with_fn(|lon, lat| {
        let i = (((earthmesh_hfield::wrap_lon_degrees(lon) + 180.0) / 360.0) * nlon as f64)
            .floor()
            .clamp(0.0, (nlon - 1) as f64) as usize;
        let j = (((lat + 90.0) / 180.0) * nlat as f64)
            .floor()
            .clamp(0.0, (nlat - 1) as f64) as usize;
        if domain.is_none_or(|domain| domain.contains(lon, lat)) && active[i * nlat + j] {
            h_inside
        } else {
            f64::INFINITY
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_mesh::LonLatDegrees;

    fn test_landtype_global_maxlc(path: &Path) -> i32 {
        read_landtype_mask_source_for_hfield(path).unwrap().maxlc
    }

    fn clear_test_landtype_maxlc_memory_cache(path: &Path) -> LandtypeMaxlcIdentity {
        let identity = landtype_maxlc_cache_identity(path).unwrap().unwrap();
        landtype_maxlc_cache().lock().unwrap().remove(&identity);
        identity
    }

    fn test_landtype_maxlc_scan_count(identity: &LandtypeMaxlcIdentity) -> usize {
        landtype_maxlc_scan_counts()
            .lock()
            .unwrap()
            .get(identity)
            .copied()
            .unwrap_or(0)
    }

    fn test_landtype_maxlc_cache_path(identity: &LandtypeMaxlcIdentity) -> PathBuf {
        landtype_maxlc_cache_path(&landtype_maxlc_cache_directories()[0], identity)
    }

    #[test]
    fn absent_group_and_disabled_group_read_as_none() {
        assert!(read_hfield_refine_options("&mkgrd\n NL%nxp = 6\n/\n")
            .unwrap()
            .is_none());
        let off = "&hfield\n NL%hfield_on = .false.\n NL%hfield_g = 0.3\n/\n";
        assert!(read_hfield_refine_options(off).unwrap().is_none());
    }

    #[test]
    fn present_group_parses_values_and_defaults() {
        let text =
            "&hfield\n NL%hfield_on = .true.\n NL%hfield_g = 0.15\n NL%hfield_max_level = 3\n/\n";
        let options = read_hfield_refine_options(text).unwrap().unwrap();
        assert!((options.g - 0.15).abs() < 1e-12);
        assert_eq!(options.max_level, Some(3));
        assert_eq!(options.base_m, None);
        assert_eq!(options.geographic_origin, None);
        assert_eq!((options.nlon, options.nlat), (720, 360));

        let bad = "&hfield\n NL%hfield_g = -1.0\n/\n";
        assert!(read_hfield_refine_options(bad).is_err());
    }

    #[test]
    fn hfield_group_rejects_unknown_fields() {
        let error = read_hfield_refine_options("&hfield\n NL%hfield_gr = 0.15\n/\n").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown &hfield field 'hfield_gr'"),
            "{error}"
        );
    }

    #[test]
    fn cartesian_geographic_origin_parses_and_maps_native_meters() {
        let text = "&hfield\n NL%hfield_origin_lon=120.0\n NL%hfield_origin_lat=30.0\n/\n";
        let options = read_hfield_refine_options(text).unwrap().unwrap();
        assert_eq!(options.geographic_origin, Some((120.0, 30.0)));
        let center = cartesian_xy_to_lonlat(0.0, 0.0, 120.0, 30.0);
        assert_eq!(center, (120.0, 30.0));
        let east = cartesian_xy_to_lonlat(100_000.0, 0.0, 120.0, 30.0);
        assert!(east.0 > 120.0);
        assert!((east.1 - 30.0).abs() < 0.01);

        assert!(read_hfield_refine_options("&hfield\n NL%hfield_origin_lon=120.0\n/\n").is_err());
    }

    #[test]
    fn hfield_is_opt_in_and_can_be_explicitly_disabled() {
        let implicit = read_hfield_refine_options("&mkgrd\n/\n").unwrap();
        assert!(implicit.is_none());

        let off = read_hfield_refine_options("&hfield\n NL%hfield_on=.false.\n/\n").unwrap();
        assert!(off.is_none());
    }

    #[test]
    fn regions_pin_levels_and_field_is_graded() {
        let regions = [MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 500_000.0,
            level: 2,
        }];
        let base = 100_000.0;
        let field = build_hfield_from_regions(&regions, base, 0.2, 360, 180).unwrap();
        let center = field.sample(115.0, 25.0);
        assert!(
            (center - base / 4.0).abs() < 1.0,
            "level-2 center should pin base/4, got {center}"
        );
        let far = field.sample(-65.0, -25.0);
        assert!((far - base).abs() < 1.0, "far field keeps base, got {far}");
        assert_eq!(field.level_at(115.0, 25.0, base, 5), 2);
    }

    #[test]
    fn geographic_hfield_rejects_out_of_sphere_latitude() {
        let regions = [MethodCRefinementRegion::Bbox {
            west_degrees: 170.0,
            east_degrees: -170.0,
            south_degrees: -10.0,
            north_degrees: 91.0,
            level: 1,
        }];

        let error = build_hfield_from_regions(&regions, 100_000.0, 0.2, 36, 18)
            .expect_err("geographic HField must reject invalid latitude");
        assert!(error.to_string().contains("latitude"));
    }

    #[test]
    fn hfield_region_footprints_use_explicit_canonical_radius_metric() {
        let base = 1_000_000.0;
        let circle = MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 5_000_000.0,
            level: 1,
        };
        let circle_probe = LonLatDegrees::new(43.5, 0.5);
        assert!(
            !circle.contains_lonlat_canonical(circle_probe),
            "regression probe must distinguish Canonical stereographic and great-circle radii"
        );
        let circle_field =
            build_hfield_from_regions(std::slice::from_ref(&circle), base, 100.0, 360, 180)
                .unwrap();
        assert_eq!(circle_field.get(223, 90), base);

        let corridor = MethodCRefinementRegion::Corridor {
            points: vec![LonLatDegrees::new(-5.0, 0.0), LonLatDegrees::new(5.0, 0.0)],
            radius_meters: vec![100_000.0, 600_000.0],
            level: 1,
        };
        let corridor_field =
            build_hfield_from_regions(std::slice::from_ref(&corridor), base, 100.0, 360, 180)
                .unwrap();
        for (ilon, probe) in [
            (175, LonLatDegrees::new(-4.5, 2.5)),
            (184, LonLatDegrees::new(4.5, 2.5)),
        ] {
            let direct = corridor.contains_lonlat_canonical(probe);
            let hfield = corridor_field.get(ilon, 92) < base;
            assert_eq!(
                hfield, direct,
                "HField and direct Method-C must use the same interpolated corridor footprint at {probe:?}"
            );
        }
    }

    #[test]
    fn cartesian_regions_pin_levels_and_grade_in_native_meters() {
        let regions = [MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(1_000_000.0, -300_000.0),
            radius_meters: 100_000.0,
            level: 2,
        }];
        let base = 400_000.0;
        assert_eq!(
            cartesian_hfield_level_at(&regions, 1_000_000.0, -300_000.0, base, 0.2, 5),
            2
        );
        assert_eq!(
            cartesian_hfield_level_at(&regions, 3_000_000.0, -300_000.0, base, 0.2, 5),
            0
        );
    }

    #[test]
    fn cartesian_bbox_and_polygon_pin_hfield_levels() {
        let bbox = [MethodCRefinementRegion::Bbox {
            west_degrees: -200_000.0,
            east_degrees: 0.0,
            south_degrees: -100_000.0,
            north_degrees: 100_000.0,
            level: 1,
        }];
        let polygon = [MethodCRefinementRegion::Polygon {
            points: vec![
                LonLatDegrees::new(100_000.0, -100_000.0),
                LonLatDegrees::new(300_000.0, -100_000.0),
                LonLatDegrees::new(200_000.0, 100_000.0),
            ],
            level: 2,
        }];
        let base = 400_000.0;

        assert_eq!(
            cartesian_hfield_level_at(&bbox, -100_000.0, 0.0, base, 0.2, 5),
            1
        );
        assert_eq!(
            cartesian_hfield_level_at(&polygon, 200_000.0, 0.0, base, 0.2, 5),
            2
        );
        assert_eq!(
            cartesian_hfield_level_at(&polygon, 2_000_000.0, 0.0, base, 0.2, 5),
            0
        );
    }

    #[test]
    fn threshold_matrix_contributes_smaller_targets() {
        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        let values = vec![0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        min_with_threshold_matrix(&mut field, &values, 5.0, 25.0, None);

        assert_eq!(field.get(1, 0), 25.0);
        assert_eq!(field.get(0, 0), 100.0);
    }

    #[test]
    fn threshold_axis_names_do_not_match_arbitrary_x_or_y_substrings() {
        assert!(is_lon_dim("longitude"));
        assert!(is_lon_dim("nav_lon"));
        assert!(is_lon_dim("x"));
        assert!(is_lat_dim("latitude"));
        assert!(is_lat_dim("nav_lat"));
        assert!(is_lat_dim("y"));

        assert!(!is_lon_dim("pixel"));
        assert!(!is_lon_dim("x_index"));
        assert!(!is_lat_dim("quality"));
        assert!(!is_lat_dim("y_index"));
    }

    #[test]
    fn threshold_stats_aggregate_source_pixels_per_hfield_cell() {
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let source = vec![
            0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 10.0, 0.0, 14.0, 0.0,
        ];

        let stats = threshold_stats_on_hfield_from_source(&source, 8, 2, &field);

        assert_eq!(stats.mean[field.nlat() + 1], 4.0);
        assert_eq!(stats.stddev[field.nlat() + 1], 2.0);
        assert_eq!(stats.mean[3 * field.nlat() + 1], 12.0);
        assert_eq!(stats.stddev[3 * field.nlat() + 1], 2.0);
    }

    #[test]
    fn threshold_stats_skip_maxlc_landtype_pixels_like_getref() {
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let source = vec![
            0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let mut landtypes = vec![vec![0, 1, 1]; 9];
        landtypes[4][1] = 9;
        let mask = (landtypes, 9);

        let stats =
            threshold_stats_on_hfield_from_source_masked(&source, 8, 2, &field, Some(&mask));

        assert_eq!(stats.mean[field.nlat() + 1], 2.0);
        assert_eq!(stats.stddev[field.nlat() + 1], 0.0);
    }

    #[test]
    fn streamed_threshold_stats_match_dense_maxlc_mask_for_axis_orders_and_float_types() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_stream_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let source = vec![
            0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 10.0, 0.0, 14.0, 0.0,
        ];
        let mut landtypes = vec![vec![0, 1, 1]; 9];
        landtypes[4][1] = 9;
        let expected = threshold_stats_on_hfield_from_source_masked(
            &source,
            8,
            2,
            &field,
            Some(&(landtypes.clone(), 9)),
        );

        for (case, threshold_lat_lon, mask_lat_lon, threshold_f32) in
            [("f64", false, true, false), ("f32", true, false, true)]
        {
            let threshold_path = root.join(format!("threshold_{case}.nc"));
            let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
            threshold_file.add_dimension("longitude", 8).unwrap();
            threshold_file.add_dimension("latitude", 2).unwrap();
            let threshold_values = if threshold_lat_lon {
                let mut values = Vec::with_capacity(source.len());
                for j in 0..2 {
                    for i in 0..8 {
                        values.push(source[i * 2 + j]);
                    }
                }
                values
            } else {
                source.clone()
            };
            let threshold_dims = if threshold_lat_lon {
                &["latitude", "longitude"][..]
            } else {
                &["longitude", "latitude"][..]
            };
            if threshold_f32 {
                threshold_file
                    .add_variable::<f32>("lai", threshold_dims)
                    .unwrap()
                    .put_values(
                        &threshold_values
                            .iter()
                            .copied()
                            .map(|value| value as f32)
                            .collect::<Vec<_>>(),
                        (.., ..),
                    )
                    .unwrap();
            } else {
                threshold_file
                    .add_variable::<f64>("lai", threshold_dims)
                    .unwrap()
                    .put_values(&threshold_values, (.., ..))
                    .unwrap();
            }
            drop(threshold_file);

            let mask_path = root.join(format!("mask_{case}.nc"));
            let mut mask_file = crate::create_netcdf_quiet(&mask_path).unwrap();
            mask_file.add_dimension("longitude", 8).unwrap();
            mask_file.add_dimension("latitude", 2).unwrap();
            let mask_values = if mask_lat_lon {
                let mut values = Vec::with_capacity(16);
                for j in 0..2 {
                    for i in 0..8 {
                        values.push(landtypes[i + 1][j + 1] as i8);
                    }
                }
                values
            } else {
                let mut values = Vec::with_capacity(16);
                for i in 0..8 {
                    for j in 0..2 {
                        values.push(landtypes[i + 1][j + 1] as i8);
                    }
                }
                values
            };
            let mask_dims = if mask_lat_lon {
                &["latitude", "longitude"][..]
            } else {
                &["longitude", "latitude"][..]
            };
            mask_file
                .add_variable::<i8>("landtype", mask_dims)
                .unwrap()
                .put_values(&mask_values, (.., ..))
                .unwrap();
            drop(mask_file);

            let mask = read_landtype_mask_source_for_hfield(&mask_path).unwrap();
            let threshold_file = crate::open_netcdf(&threshold_path).unwrap();
            let actual = read_threshold_stats_on_hfield_masked(
                &threshold_file,
                "lai",
                &field,
                Some(&mask),
                None,
            )
            .unwrap();

            assert_eq!(actual.mean, expected.mean, "mean mismatch for {case}");
            assert_eq!(actual.stddev, expected.stddev, "std mismatch for {case}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn streamed_threshold_rows_are_north_to_south_including_nearest_fallback() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_north_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        // Canonical source order is north-to-south: j=0 is northern.
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[10.0, 1.0, 10.0, 1.0, 10.0, 1.0, 10.0, 1.0], (.., ..))
            .unwrap();
        drop(file);
        let file = crate::open_netcdf(&path).unwrap();

        let same_resolution = HField::uniform(4, 2, 100.0).unwrap();
        let stats =
            read_threshold_stats_on_hfield_masked(&file, "lai", &same_resolution, None, None)
                .unwrap();
        assert_eq!(
            stats.mean[1], 10.0,
            "north value must land in north HField bin"
        );
        assert_eq!(
            stats.mean[0], 1.0,
            "south value must land in south HField bin"
        );

        let finer_latitude = HField::uniform(4, 4, 100.0).unwrap();
        let stats =
            read_threshold_stats_on_hfield_masked(&file, "lai", &finer_latitude, None, None)
                .unwrap();
        assert_eq!(
            stats.mean[2], 10.0,
            "empty northern bin inherits northern source"
        );
        assert_eq!(
            stats.mean[0], 1.0,
            "empty southern bin inherits southern source"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn fine_active_row_without_source_center_uses_nearest_threshold_and_landtype_rows() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_fine_active_row_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let field = HField::uniform(4, 4, 100.0).unwrap();
        let mut active = vec![false; field.nlon() * field.nlat()];
        for i in 0..field.nlon() {
            active[i * field.nlat()] = true;
        }
        let domain = HfieldDomainMask {
            nlon: field.nlon(),
            nlat: field.nlat(),
            active,
        };
        let (_, active_lat) = active_hfield_axes(&field, Some(&domain));
        assert_eq!(
            active_source_latitude_window(&field, &active_lat, 2, LatitudeOrder::NorthToSouth,)
                .unwrap(),
            (1, 1, 1, 1),
            "fine southern row must select its nearest coarse source row"
        );

        let threshold_path = root.join("lai.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 4).unwrap();
        threshold_file.add_dimension("latitude", 2).unwrap();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[10.0, 1.0, 10.0, 1.0, 10.0, 1.0, 10.0, 1.0], (.., ..))
            .unwrap();
        drop(threshold_file);
        let threshold_file = crate::open_netcdf(&threshold_path).unwrap();
        let stats = read_threshold_stats_on_hfield_masked(
            &threshold_file,
            "lai",
            &field,
            None,
            Some(&domain),
        )
        .unwrap();
        for i in 0..field.nlon() {
            assert_eq!(stats.mean[i * field.nlat()], 1.0);
        }

        let landtype_path = root.join("landtype.nc");
        let mut landtype_file = crate::create_netcdf_quiet(&landtype_path).unwrap();
        landtype_file.add_dimension("longitude", 4).unwrap();
        landtype_file.add_dimension("latitude", 2).unwrap();
        landtype_file
            .add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[9, 1, 2, 1, 2, 1, 2, 1], (.., ..))
            .unwrap();
        drop(landtype_file);
        let bins = read_landtype_source_for_hfield(&landtype_path, &field, Some(&domain)).unwrap();
        for i in 0..field.nlon() {
            assert!(bins.contains_class(i * field.nlat(), 1));
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn streamed_threshold_rows_follow_ascending_latitude_coordinate() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_ascending_lat_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("latitude", &["latitude"])
            .unwrap()
            .put_values(&[-45.0, 45.0], ..)
            .unwrap();
        // File rows are south-to-north because the latitude coordinate ascends.
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1.0, 10.0, 1.0, 10.0, 1.0, 10.0, 1.0, 10.0], (.., ..))
            .unwrap();
        drop(file);
        let file = crate::open_netcdf(&path).unwrap();

        let same_resolution = HField::uniform(4, 2, 100.0).unwrap();
        let stats =
            read_threshold_stats_on_hfield_masked(&file, "lai", &same_resolution, None, None)
                .unwrap();
        assert_eq!(stats.mean[1], 10.0, "north row follows coordinate values");
        assert_eq!(stats.mean[0], 1.0, "south row follows coordinate values");

        let finer_latitude = HField::uniform(4, 4, 100.0).unwrap();
        let stats =
            read_threshold_stats_on_hfield_masked(&file, "lai", &finer_latitude, None, None)
                .unwrap();
        assert_eq!(stats.mean[2], 10.0, "nearest fallback preserves north");
        assert_eq!(stats.mean[0], 1.0, "nearest fallback preserves south");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn streamed_threshold_columns_follow_wrapped_and_descending_longitude_coordinates() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_longitude_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (case, coordinates, values) in [
            (
                "wrapped",
                [0.0, 90.0, 180.0, 270.0],
                [10.0, 10.0, 20.0, 20.0, 30.0, 30.0, 40.0, 40.0],
            ),
            (
                "descending",
                [270.0, 180.0, 90.0, 0.0],
                [40.0, 40.0, 30.0, 30.0, 20.0, 20.0, 10.0, 10.0],
            ),
        ] {
            let path = root.join(format!("{case}.nc"));
            let mut file = crate::create_netcdf_quiet(&path).unwrap();
            file.add_dimension("longitude", 4).unwrap();
            file.add_dimension("latitude", 2).unwrap();
            file.add_variable::<f64>("longitude", &["longitude"])
                .unwrap()
                .put_values(&coordinates, ..)
                .unwrap();
            file.add_variable::<f64>("lai", &["longitude", "latitude"])
                .unwrap()
                .put_values(&values, (.., ..))
                .unwrap();
            drop(file);
            let file = crate::open_netcdf(&path).unwrap();
            let field = HField::uniform(4, 2, 100.0).unwrap();
            let stats =
                read_threshold_stats_on_hfield_masked(&file, "lai", &field, None, None).unwrap();
            assert_eq!(
                [stats.mean[1], stats.mean[3], stats.mean[5], stats.mean[7]],
                [30.0, 40.0, 10.0, 20.0],
                "{case}"
            );
        }

        let path = root.join("non_monotonic.nc");
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(&[0.0, 90.0, 45.0, 180.0], ..)
            .unwrap();
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1.0; 8], (.., ..))
            .unwrap();
        drop(file);
        let file = crate::open_netcdf(&path).unwrap();
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let error =
            read_threshold_stats_on_hfield_masked(&file, "lai", &field, None, None).unwrap_err();
        assert!(error.to_string().contains("strictly monotonic"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn regional_threshold_stats_match_full_scan_across_dateline() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_regional_dateline_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let threshold_path = root.join("threshold.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 8).unwrap();
        threshold_file.add_dimension("latitude", 4).unwrap();
        threshold_file
            .add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(&[0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0], ..)
            .unwrap();
        let values = (0..32).map(|value| value as f64).collect::<Vec<_>>();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(threshold_file);

        let mask_path = root.join("mask.nc");
        let mut mask_file = crate::create_netcdf_quiet(&mask_path).unwrap();
        mask_file.add_dimension("longitude", 8).unwrap();
        mask_file.add_dimension("latitude", 4).unwrap();
        mask_file
            .add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(&[0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0], ..)
            .unwrap();
        let mut mask_values = vec![1_i8; 32];
        mask_values[4] = 9;
        mask_file
            .add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&mask_values, (.., ..))
            .unwrap();
        drop(mask_file);

        let field = HField::uniform(8, 4, 100.0).unwrap();
        let domain = HfieldDomainMask::new(
            8,
            4,
            &GridRegion::Bbox {
                west: 135.0,
                east: -135.0,
                south: -45.0,
                north: 45.0,
            },
        );
        let mask = read_landtype_mask_source_for_hfield(&mask_path).unwrap();
        let threshold_file = crate::open_netcdf(&threshold_path).unwrap();
        let global = read_threshold_stats_on_hfield_masked(
            &threshold_file,
            "lai",
            &field,
            Some(&mask),
            None,
        )
        .unwrap();
        let regional = read_threshold_stats_on_hfield_masked(
            &threshold_file,
            "lai",
            &field,
            Some(&mask),
            Some(&domain),
        )
        .unwrap();

        for i in 0..field.nlon() {
            for j in 0..field.nlat() {
                let out = i * field.nlat() + j;
                if domain.is_active(i, j) {
                    assert_eq!(regional.mean[out], global.mean[out], "mean ({i},{j})");
                    assert_eq!(regional.stddev[out], global.stddev[out], "std ({i},{j})");
                }
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn streamed_threshold_rows_reject_fill_missing_and_non_finite_values() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_invalid_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let cases = [
            ("fill", Some(("_FillValue", -9999.0)), -9999.0),
            ("missing", Some(("missing_value", -8888.0)), -8888.0),
            ("nan", None, f64::NAN),
            ("infinite", None, f64::INFINITY),
        ];
        for (case, attribute, invalid_value) in cases {
            let path = root.join(format!("{case}.nc"));
            let mut file = crate::create_netcdf_quiet(&path).unwrap();
            file.add_dimension("longitude", 4).unwrap();
            file.add_dimension("latitude", 2).unwrap();
            let mut variable = file
                .add_variable::<f64>("lai", &["longitude", "latitude"])
                .unwrap();
            if let Some((name, value)) = attribute {
                variable.put_attribute(name, value).unwrap();
            }
            variable
                .put_values(
                    &[invalid_value, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
                    (.., ..),
                )
                .unwrap();
            drop(file);
            let file = crate::open_netcdf(&path).unwrap();
            let field = HField::uniform(4, 4, 100.0).unwrap();
            let error = read_threshold_stats_on_hfield_masked(&file, "lai", &field, None, None)
                .expect_err(case);
            assert!(
                error.to_string().contains("missing/non-finite"),
                "{case}: {error}"
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn streamed_threshold_rows_reject_default_netcdf_fill() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_default_fill_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_value(1.0, (0, 0))
            .unwrap();
        drop(file);
        let file = crate::open_netcdf(&path).unwrap();
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let error =
            read_threshold_stats_on_hfield_masked(&file, "lai", &field, None, None).unwrap_err();
        assert!(error.to_string().contains("missing/non-finite"), "{error}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn streamed_threshold_rows_reject_integer_default_netcdf_fill() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_integer_fill_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<i16>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_value(1, (0, 0))
            .unwrap();
        drop(file);
        let file = crate::open_netcdf(&path).unwrap();
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let error =
            read_threshold_stats_on_hfield_masked(&file, "lai", &field, None, None).unwrap_err();
        assert!(error.to_string().contains("missing/non-finite"), "{error}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn streamed_threshold_rows_require_identifiable_axes() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_bad_axes_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("row", 4).unwrap();
        file.add_dimension("column", 2).unwrap();
        file.add_variable::<f64>("lai", &["row", "column"])
            .unwrap()
            .put_values(&[1.0; 8], (.., ..))
            .unwrap();
        drop(file);
        let file = crate::open_netcdf(&path).unwrap();
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let error =
            read_threshold_stats_on_hfield_masked(&file, "lai", &field, None, None).unwrap_err();
        assert!(error
            .to_string()
            .contains("identify longitude and latitude axes"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn std_thresholds_contribute_to_hfield_without_mean_flag() {
        let root =
            std::env::temp_dir().join(format!("earthmesh_hfield_lai_std_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("lai.nc");
        let values = vec![
            0.0_f64, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 8).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);

        let mut refine = RefineConfig {
            threshold_dir: root.display().to_string(),
            ..RefineConfig::default()
        };
        refine.refine_onelayer_lnd[1] = true;
        refine.th_onelayer_lnd[1] = 1.0;
        let mut field = HField::uniform(4, 2, 100.0).unwrap();

        let applied = apply_std_threshold_hfield_contributions(
            &mut field, &refine, "landmesh", 100.0, 1, 10.0,
        )
        .unwrap();

        assert_eq!(applied, 1);
        assert_eq!(field.get(0, 1), 50.0);
        assert_eq!(field.get(1, 1), 100.0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mean_and_std_thresholds_share_one_stats_read_without_reordering_limiters() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_lai_mean_std_cache_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("lai.nc");
        let values = vec![
            0.0_f64, 0.0, 2.0, 0.0, 6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 8).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);

        let mut refine = RefineConfig {
            threshold_dir: root.display().to_string(),
            ..RefineConfig::default()
        };
        refine.refine_onelayer_lnd[0] = true;
        refine.refine_onelayer_lnd[1] = true;
        refine.th_onelayer_lnd[0] = 1.0;
        refine.th_onelayer_lnd[1] = 1.0;
        let mut actual = HField::uniform(4, 2, 100.0).unwrap();
        let mut cache = ThresholdStatsCache::new();

        apply_mean_threshold_hfield_contributions_with_landtype_mask(
            &mut actual,
            &refine,
            "landmesh",
            100.0,
            1,
            10.0,
            None,
            None,
            &mut cache,
        )
        .unwrap();
        assert_eq!(cache.len(), 1);
        apply_std_threshold_hfield_contributions_with_landtype_mask(
            &mut actual,
            &refine,
            "landmesh",
            100.0,
            1,
            10.0,
            None,
            None,
            &mut cache,
        )
        .unwrap();

        assert_eq!(
            cache.len(),
            1,
            "mean/std must reuse the same ThresholdStats"
        );
        assert_eq!(actual.get(1, 1), 50.0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn continuous_thresholds_do_not_open_placeholder_landtype_source() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_continuous_without_landtype_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let threshold_path = root.join("lai.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 4).unwrap();
        threshold_file.add_dimension("latitude", 2).unwrap();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[10.0; 8], (.., ..))
            .unwrap();
        drop(threshold_file);

        let mut refine = RefineConfig {
            refine_cal: true,
            threshold_dir: root.display().to_string(),
            ..RefineConfig::default()
        };
        refine.refine_onelayer_lnd[0] = true;
        refine.th_onelayer_lnd[0] = 1.0;
        let config = EarthmeshConfig {
            landtype_file: "/tmp".to_string(),
            ..EarthmeshConfig::default()
        };
        let options = HfieldRefineOptions {
            g: 10.0,
            max_level: Some(1),
            base_m: Some(100.0),
            geographic_origin: None,
            nlon: 4,
            nlat: 2,
            target_cells_geojson: None,
            target_levels_json: None,
        };

        let field = build_composed_hfield(
            &[],
            &refine,
            "landmesh",
            Some(&config),
            100.0,
            &options,
            1,
            None,
        )
        .unwrap();

        assert_eq!(field.values(), &[50.0; 8]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn continuous_thresholds_keep_real_landtype_maxlc_mask_without_landtype_criteria() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_continuous_real_landtype_mask_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let threshold_path = root.join("lai.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 4).unwrap();
        threshold_file.add_dimension("latitude", 2).unwrap();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[10.0; 8], (.., ..))
            .unwrap();
        drop(threshold_file);
        let landtype_path = root.join("landtype.nc");
        let mut landtype_file = crate::create_netcdf_quiet(&landtype_path).unwrap();
        landtype_file.add_dimension("longitude", 4).unwrap();
        landtype_file.add_dimension("latitude", 2).unwrap();
        landtype_file
            .add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[9, 1, 1, 1, 1, 1, 1, 1], (.., ..))
            .unwrap();
        drop(landtype_file);

        let mut refine = RefineConfig {
            refine_cal: true,
            threshold_dir: root.display().to_string(),
            ..RefineConfig::default()
        };
        refine.refine_onelayer_lnd[0] = true;
        refine.th_onelayer_lnd[0] = 1.0;
        let config = EarthmeshConfig {
            landtype_file: landtype_path.display().to_string(),
            ..EarthmeshConfig::default()
        };
        let options = HfieldRefineOptions {
            g: 10.0,
            max_level: Some(1),
            base_m: Some(100.0),
            geographic_origin: None,
            nlon: 4,
            nlat: 2,
            target_cells_geojson: None,
            target_levels_json: None,
        };

        let field = build_composed_hfield(
            &[],
            &refine,
            "landmesh",
            Some(&config),
            100.0,
            &options,
            1,
            None,
        )
        .unwrap();

        assert_eq!(field.get(0, 1), 100.0, "maxlc pixel remains masked");
        assert_eq!(field.get(0, 0), 50.0, "valid land pixel still refines");
        let identity = landtype_maxlc_cache_identity(&landtype_path)
            .unwrap()
            .unwrap();
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn landtype_basic_thresholds_contribute_to_hfield() {
        let landtypes = vec![
            vec![0, 0, 0],
            vec![0, 1, 1],
            vec![0, 2, 1],
            vec![0, 1, 1],
            vec![0, 1, 1],
            vec![0, 0, 1],
            vec![0, 1, 1],
            vec![0, 1, 1],
            vec![0, 1, 1],
        ];
        let mut refine = RefineConfig {
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            refine_area_mainland: true,
            th_area_mainland: 0.75,
            refine_sea_ratio: true,
            th_sea_ratio: [0.4, 0.6],
            ..RefineConfig::default()
        };
        for mesh_type in [
            "landmesh",
            "oceanmesh",
            "atmos",
            "atmosmesh",
            "LOCmesh",
            "earthmesh",
        ] {
            assert!(has_threshold_hfield_sources(&refine, mesh_type));
            let mut field = HField::uniform(4, 2, 100.0).unwrap();
            let applied = apply_landtype_basic_thresholds_from_source(
                &mut field, &landtypes, 9, &refine, mesh_type, 25.0,
            )
            .unwrap();

            assert_eq!(applied, 3, "{mesh_type}");
            assert_eq!(field.get(0, 1), 25.0, "{mesh_type}");
            assert_eq!(field.get(2, 1), 25.0, "{mesh_type}");
            assert_eq!(field.get(1, 1), 100.0, "{mesh_type}");
        }

        refine.refine_num_landtypes = false;
        refine.refine_area_mainland = false;
        refine.refine_sea_ratio = false;
        assert!(!has_threshold_hfield_sources(&refine, "atmosmesh"));
    }

    #[test]
    fn streamed_landtype_basic_thresholds_match_dense_for_both_axis_orders() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_stream_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let longitude_major = vec![
            1_i8, 1, // HField bin (0, 0): two distinct land classes.
            2, 1, 1, 1, 1, 1, 0, 1, // HField bin (2, 0): 50% ocean.
            1, 1, 1, 1, 1, 9, // maxlc is land but excluded as a class.
        ];
        let dense = vec![
            vec![0, 0, 0],
            vec![0, 1, 1],
            vec![0, 2, 1],
            vec![0, 1, 1],
            vec![0, 1, 1],
            vec![0, 0, 1],
            vec![0, 1, 1],
            vec![0, 1, 1],
            vec![0, 1, 9],
        ];
        let refine = RefineConfig {
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            refine_area_mainland: true,
            th_area_mainland: 0.75,
            refine_sea_ratio: true,
            th_sea_ratio: [0.4, 0.6],
            ..RefineConfig::default()
        };

        let mut expected = HField::uniform(4, 2, 100.0).unwrap();
        apply_landtype_basic_thresholds_from_source(
            &mut expected,
            &dense,
            9,
            &refine,
            "earthmesh",
            25.0,
        )
        .unwrap();

        for lat_lon in [false, true] {
            let path = root.join(if lat_lon { "lat_lon.nc" } else { "lon_lat.nc" });
            let mut file = crate::create_netcdf_quiet(&path).unwrap();
            file.add_dimension("longitude", 8).unwrap();
            file.add_dimension("latitude", 2).unwrap();
            let values = if lat_lon {
                let mut transposed = Vec::with_capacity(longitude_major.len());
                for j in 0..2 {
                    for i in 0..8 {
                        transposed.push(longitude_major[i * 2 + j]);
                    }
                }
                transposed
            } else {
                longitude_major.clone()
            };
            let dimensions = if lat_lon {
                &["latitude", "longitude"][..]
            } else {
                &["longitude", "latitude"][..]
            };
            file.add_variable::<i8>("landtype", dimensions)
                .unwrap()
                .put_values(&values, (.., ..))
                .unwrap();
            drop(file);

            let mut actual = HField::uniform(4, 2, 100.0).unwrap();
            let stats = read_landtype_source_for_hfield(&path, &actual, None).unwrap();
            let applied = apply_landtype_basic_thresholds_from_bins(
                &mut actual,
                &stats,
                &refine,
                "earthmesh",
                25.0,
                None,
            )
            .unwrap();

            assert_eq!(applied, 3);
            assert_eq!(actual.values(), expected.values());
            assert_eq!(actual.get(0, 1), 25.0, "north distinct/mainland thresholds");
            assert_eq!(actual.get(2, 1), 25.0, "north sea-ratio threshold");
            assert_eq!(actual.get(0, 0), 100.0, "south must remain coarse");
            assert_eq!(actual.get(1, 1), 100.0, "uniform land remains coarse");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn streamed_landtype_rows_follow_ascending_latitude_coordinate() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_ascending_lat_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("latitude", &["latitude"])
            .unwrap()
            .put_values(&[-45.0, 45.0], ..)
            .unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[0, 1, 0, 1, 0, 1, 0, 1], (.., ..))
            .unwrap();
        drop(file);

        let field = HField::uniform(4, 2, 100.0).unwrap();
        let bins = read_landtype_source_for_hfield(&path, &field, None).unwrap();
        for i in 0..4 {
            assert_eq!(bins.ocean_at(i * 2), 1, "south row {i}");
            assert_eq!(bins.land_at(i * 2 + 1), 1, "north row {i}");
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn regional_landtype_stream_reads_only_active_hfield_bins() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_regional_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 8).unwrap();
        file.add_dimension("latitude", 4).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1; 32], (.., ..))
            .unwrap();
        drop(file);

        let field = HField::uniform(8, 4, 100.0).unwrap();
        let domain = HfieldDomainMask::new(
            8,
            4,
            &GridRegion::Bbox {
                west: 0.0,
                east: 90.0,
                south: 0.0,
                north: 90.0,
            },
        );
        let bins = read_landtype_source_for_hfield(&path, &field, Some(&domain)).unwrap();

        assert_eq!(bins.total_samples(), 4);
        for i in 0..field.nlon() {
            for j in 0..field.nlat() {
                assert_eq!(
                    bins.total_at(i * field.nlat() + j),
                    usize::from(domain.is_active(i, j))
                );
            }
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn landtype_global_maxlc_persistent_cache_hits_after_memory_cache_is_cleared() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_sidecar_hit_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1, 2, 9, 1, 1, 1, 1, 1], (.., ..))
            .unwrap();
        drop(file);

        assert_eq!(test_landtype_global_maxlc(&path), 9);
        let identity = clear_test_landtype_maxlc_memory_cache(&path);
        assert_eq!(test_landtype_maxlc_scan_count(&identity), 1);
        assert_eq!(test_landtype_global_maxlc(&path), 9);
        assert_eq!(test_landtype_maxlc_scan_count(&identity), 1);
        assert_eq!(
            read_landtype_maxlc_cache(&identity),
            Some(9),
            "the second lookup must be served by the exact persisted record"
        );
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn landtype_global_maxlc_cache_hits_when_source_directory_is_read_only() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_read_only_cache_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1, 2, 5, 1, 1, 1, 1, 1], (.., ..))
            .unwrap();
        drop(file);

        let identity = clear_test_landtype_maxlc_memory_cache(&path);
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));
        let original_permissions = std::fs::metadata(&root).unwrap().permissions();
        let mut read_only = original_permissions.clone();
        read_only.set_mode(0o555);
        std::fs::set_permissions(&root, read_only).unwrap();

        assert_eq!(test_landtype_global_maxlc(&path), 5);
        clear_test_landtype_maxlc_memory_cache(&path);
        assert_eq!(test_landtype_global_maxlc(&path), 5);
        assert_eq!(
            test_landtype_maxlc_scan_count(&identity),
            1,
            "the persistent user cache must not depend on source-directory writes"
        );

        std::fs::set_permissions(&root, original_permissions).unwrap();
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_landtype_maxlc_cache_writers_leave_one_valid_record() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_concurrent_cache_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        std::fs::write(&path, b"identity-only fixture").unwrap();
        let identity = landtype_maxlc_cache_identity(&path).unwrap().unwrap();
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));

        let writers = (0..4)
            .map(|_| {
                let identity = identity.clone();
                std::thread::spawn(move || write_landtype_maxlc_cache(&identity, 9))
            })
            .collect::<Vec<_>>();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }

        assert_eq!(read_landtype_maxlc_cache(&identity), Some(9));
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn landtype_open_rejects_path_replacement_before_binding_cache_identity() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_open_identity_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        let replacement = root.join("replacement.nc");
        for (fixture, nlon) in [(&path, 4), (&replacement, 5)] {
            let mut file = crate::create_netcdf_quiet(fixture).unwrap();
            file.add_dimension("longitude", nlon).unwrap();
            file.add_dimension("latitude", 2).unwrap();
            file.add_variable::<i8>("landtype", &["longitude", "latitude"])
                .unwrap()
                .put_values(&vec![1_i8; nlon * 2], (.., ..))
                .unwrap();
        }

        let error = match open_landtype_netcdf_with(&path, |path| {
            std::fs::remove_file(path)?;
            std::fs::rename(&replacement, path)?;
            crate::open_netcdf(path).map_err(crate::netcdf_to_io_error)
        }) {
            Ok(_) => panic!("a replaced path must not be bound to the old cache identity"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("changed while opening NetCDF"),
            "{error}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn landtype_global_maxlc_persistent_cache_invalidates_when_source_identity_changes() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_sidecar_invalidate_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1, 2, 3, 1, 1, 1, 1, 1], (.., ..))
            .unwrap();
        drop(file);

        assert_eq!(test_landtype_global_maxlc(&path), 3);
        let old_identity = clear_test_landtype_maxlc_memory_cache(&path);
        let old_modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mut file = netcdf::append(&path).unwrap();
        file.variable_mut("landtype")
            .unwrap()
            .put_value(7, (0, 0))
            .unwrap();
        drop(file);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old_modified + std::time::Duration::from_secs(1))
            .unwrap();
        let new_identity = clear_test_landtype_maxlc_memory_cache(&path);
        assert_ne!(new_identity, old_identity);

        assert_eq!(test_landtype_global_maxlc(&path), 7);
        assert_eq!(test_landtype_maxlc_scan_count(&old_identity), 1);
        assert_eq!(test_landtype_maxlc_scan_count(&new_identity), 1);
        assert_eq!(read_landtype_maxlc_cache(&new_identity), Some(7));
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&new_identity));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn landtype_global_maxlc_recovers_exactly_from_a_corrupt_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_sidecar_corrupt_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1, 2, 6, 1, 1, 1, 1, 1], (.., ..))
            .unwrap();
        drop(file);

        assert_eq!(test_landtype_global_maxlc(&path), 6);
        let identity = clear_test_landtype_maxlc_memory_cache(&path);
        std::fs::write(test_landtype_maxlc_cache_path(&identity), b"not valid json").unwrap();

        assert_eq!(test_landtype_global_maxlc(&path), 6);
        assert_eq!(test_landtype_maxlc_scan_count(&identity), 2);
        clear_test_landtype_maxlc_memory_cache(&path);
        assert_eq!(test_landtype_global_maxlc(&path), 6);
        assert_eq!(
            test_landtype_maxlc_scan_count(&identity),
            2,
            "the exact recovery scan must replace the corrupt sidecar"
        );
        let _ = std::fs::remove_file(test_landtype_maxlc_cache_path(&identity));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn regional_landtype_stream_excludes_the_global_not_local_maxlc() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_regional_maxlc_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 8).unwrap();
        file.add_dimension("latitude", 4).unwrap();
        let mut values = vec![1_i8; 32];
        values[0] = 9; // True global maxlc, outside the regional window.
        values[4 * 4] = 2; // Largest regional class; it must remain a valid class.
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);

        let field = HField::uniform(8, 4, 100.0).unwrap();
        let domain = HfieldDomainMask::new(
            8,
            4,
            &GridRegion::Bbox {
                west: 0.0,
                east: 90.0,
                south: 0.0,
                north: 90.0,
            },
        );
        let bins = read_landtype_source_for_hfield(&path, &field, Some(&domain)).unwrap();

        assert!(
            bins.contains_class(4 * field.nlat() + 3, 2),
            "a regional maximum is a valid class when a larger global maxlc exists"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn subcell_region_without_active_hfield_centers_has_no_threshold_contribution() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_empty_subcell_region_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let landtype_path = root.join("landtype.nc");
        let mut landtype_file = crate::create_netcdf_quiet(&landtype_path).unwrap();
        landtype_file.add_dimension("longitude", 8).unwrap();
        landtype_file.add_dimension("latitude", 4).unwrap();
        landtype_file
            .add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1_i8; 32], (.., ..))
            .unwrap();
        drop(landtype_file);

        let threshold_path = root.join("threshold.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 8).unwrap();
        threshold_file.add_dimension("latitude", 4).unwrap();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1.0_f64; 32], (.., ..))
            .unwrap();
        drop(threshold_file);

        let field = HField::uniform(8, 4, 100.0).unwrap();
        let domain = HfieldDomainMask::new(
            8,
            4,
            &GridRegion::Bbox {
                west: 0.0,
                east: 1.0,
                south: 0.0,
                north: 1.0,
            },
        );
        assert!((0..field.nlon()).all(|i| (0..field.nlat()).all(|j| !domain.is_active(i, j))));

        let bins = read_landtype_source_for_hfield(&landtype_path, &field, Some(&domain)).unwrap();
        assert_eq!(bins.total_samples(), 0);

        let threshold_file = crate::open_netcdf(&threshold_path).unwrap();
        let stats = read_threshold_stats_on_hfield_masked(
            &threshold_file,
            "lai",
            &field,
            None,
            Some(&domain),
        )
        .unwrap();
        assert_eq!(stats.mean, vec![0.0; field.nlon() * field.nlat()]);
        assert_eq!(stats.stddev, vec![0.0; field.nlon() * field.nlat()]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn streamed_landtype_columns_follow_wrapped_longitude_coordinate() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_longitude_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 4).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(&[0.0, 90.0, 180.0, 270.0], ..)
            .unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1, 1, 2, 2, 3, 3, 4, 9], (.., ..))
            .unwrap();
        drop(file);

        let field = HField::uniform(4, 2, 100.0).unwrap();
        let bins = read_landtype_source_for_hfield(&path, &field, None).unwrap();
        for (i, expected) in [3, 4, 1, 2].into_iter().enumerate() {
            assert!(bins.contains_class(i * 2 + 1, expected), "bin {i}");
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn landtype_basic_and_mask_readers_require_identifiable_axes() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_bad_axes_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("row", 4).unwrap();
        file.add_dimension("column", 2).unwrap();
        file.add_variable::<i8>("landtype", &["row", "column"])
            .unwrap()
            .put_values(&[1; 8], (.., ..))
            .unwrap();
        drop(file);

        let field = HField::uniform(4, 2, 100.0).unwrap();
        for error in [
            read_landtype_source_for_hfield(&path, &field, None).unwrap_err(),
            read_landtype_mask_source_for_hfield(&path).unwrap_err(),
        ] {
            assert!(
                error
                    .to_string()
                    .contains("identify longitude and latitude axes"),
                "{error}"
            );
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn nearest_threshold_fallback_keeps_maxlc_pixels_masked() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_nearest_mask_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let threshold_path = root.join("lai.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 4).unwrap();
        threshold_file.add_dimension("latitude", 2).unwrap();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[10.0, 1.0, 10.0, 1.0, 10.0, 1.0, 10.0, 1.0], (.., ..))
            .unwrap();
        drop(threshold_file);

        let mask_path = root.join("landtype.nc");
        let mut mask_file = crate::create_netcdf_quiet(&mask_path).unwrap();
        mask_file.add_dimension("longitude", 4).unwrap();
        mask_file.add_dimension("latitude", 2).unwrap();
        mask_file
            .add_variable::<f64>("latitude", &["latitude"])
            .unwrap()
            .put_values(&[-45.0, 45.0], ..)
            .unwrap();
        mask_file
            .add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[0, 9, 0, 9, 0, 9, 0, 9], (.., ..))
            .unwrap();
        drop(mask_file);

        let mask = read_landtype_mask_source_for_hfield(&mask_path).unwrap();
        let threshold_file = crate::open_netcdf(&threshold_path).unwrap();
        let field = HField::uniform(4, 4, 100.0).unwrap();
        let stats = read_threshold_stats_on_hfield_masked(
            &threshold_file,
            "lai",
            &field,
            Some(&mask),
            None,
        )
        .unwrap();
        assert_eq!(&stats.mean[..4], &[1.0, 1.0, 0.0, 0.0]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn threshold_landtype_mask_skips_default_fill_pixels() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hfield_threshold_landtype_fill_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let threshold_path = root.join("lai.nc");
        let mut threshold_file = crate::create_netcdf_quiet(&threshold_path).unwrap();
        threshold_file.add_dimension("longitude", 4).unwrap();
        threshold_file.add_dimension("latitude", 2).unwrap();
        threshold_file
            .add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[10.0; 8], (.., ..))
            .unwrap();
        drop(threshold_file);

        let mask_path = root.join("landtype.nc");
        let mut mask_file = crate::create_netcdf_quiet(&mask_path).unwrap();
        mask_file.add_dimension("longitude", 4).unwrap();
        mask_file.add_dimension("latitude", 2).unwrap();
        let mut landtype = mask_file
            .add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap();
        for i in 0..4 {
            landtype.put_value(9, (i, 0)).unwrap();
        }
        drop(mask_file);

        let mask = read_landtype_mask_source_for_hfield(&mask_path).unwrap();
        let threshold_file = crate::open_netcdf(&threshold_path).unwrap();
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let stats = read_threshold_stats_on_hfield_masked(
            &threshold_file,
            "lai",
            &field,
            Some(&mask),
            None,
        )
        .unwrap();
        assert_eq!(stats.mean, vec![0.0; 8]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn composed_hfield_reads_landtype_basic_sources_without_regions() {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_hfield_landtype_basic_{}.nc",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let values = vec![1_i8, 1, 2, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1];
        let mut file = crate::create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", 8).unwrap();
        file.add_dimension("latitude", 2).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);

        let refine = RefineConfig {
            refine_cal: true,
            max_iter_cal: 1,
            refine_sea_ratio: true,
            th_sea_ratio: [0.4, 0.6],
            ..RefineConfig::default()
        };
        let config = EarthmeshConfig {
            landtype_file: path.display().to_string(),
            ..EarthmeshConfig::default()
        };
        let options = HfieldRefineOptions {
            g: 0.2,
            max_level: Some(1),
            base_m: Some(100.0),
            geographic_origin: None,
            nlon: 4,
            nlat: 2,
            target_cells_geojson: None,
            target_levels_json: None,
        };

        let field = build_composed_hfield(
            &[],
            &refine,
            "oceanmesh",
            Some(&config),
            100.0,
            &options,
            1,
            None,
        )
        .unwrap();

        assert_eq!(field.get(2, 1), 50.0);
        assert_eq!(field.get(0, 1), 100.0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn regional_domain_discards_unrelated_global_hfield_demand() {
        let mut field = HField::uniform(360, 180, 25.0).unwrap();
        let domain = GridRegion::Bbox {
            west: 108.0,
            east: 120.0,
            south: 18.0,
            north: 26.0,
        };

        constrain_hfield_to_domain(&mut field, Some(&domain), 100.0, 0.2).unwrap();

        assert_eq!(field.level_at(114.0, 22.0, 100.0, 2), 2);
        assert_eq!(field.level_at(8.5, 43.8, 100.0, 2), 0);
    }

    #[test]
    fn regional_thresholds_are_scoped_before_gradient_limiting() {
        let mut field = HField::uniform(360, 180, 100_000.0).unwrap();
        let domain = GridRegion::Bbox {
            west: 108.0,
            east: 120.0,
            south: 18.0,
            north: 26.0,
        };
        let domain = HfieldDomainMask::new(360, 180, &domain);
        min_with_bool_matrix(&mut field, &vec![true; 360 * 180], 25_000.0, Some(&domain));
        field.limit_gradient(0.2).unwrap();

        assert_eq!(field.level_at(114.0, 22.0, 100_000.0, 2), 2);
        assert_eq!(field.level_at(8.5, 43.8, 100_000.0, 2), 0);
    }

    #[test]
    fn regional_specified_sources_are_scoped_before_gradient_limiting() {
        let domain = GridRegion::Bbox {
            west: 108.0,
            east: 120.0,
            south: 18.0,
            north: 26.0,
        };
        let outside = MethodCRefinementRegion::Circle {
            center: LonLatDegrees::new(107.0, 22.0),
            radius_meters: 50_000.0,
            level: 2,
        };
        let field = build_hfield_from_regions_in_domain(
            &[outside],
            100_000.0,
            0.2,
            360,
            180,
            Some(&HfieldDomainMask::new(360, 180, &domain)),
        )
        .unwrap();

        assert_eq!(field.level_at(108.5, 22.5, 100_000.0, 2), 0);
    }
}
