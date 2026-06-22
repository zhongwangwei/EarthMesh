//! MERIT-Hydro / hydro-coast validation report (MVP).
//!
//! **INTEGRATION STATUS: EXPERIMENTAL / NOT WIRED.** The CLI MERIT pipeline does not
//! build [`HydroCoastInputs`] or call [`build_report`] yet — only unit tests do. (The
//! antimeridian tile-selection bug it diagnoses was fixed directly in the reader,
//! `merit_bbox_intersects`.) Wiring is future work (R6 report §5).
//!
//! A pure, dependency-light diagnostic layer: the caller (CLI MERIT pipeline) feeds
//! already-extracted facts (selected tiles, bbox, feature counts, close-mask rings…)
//! into [`HydroCoastInputs`], and [`build_report`] returns a structured
//! [`HydroCoastValidationReport`] with warnings, geometry flags and recommended fixes.
//! It does **not** read NetCDF, rewrite the MERIT reader, or pull a GIS dependency —
//! it reuses `earthmesh_geometry::safety` for polygon / buffer / dateline checks.
//!
//! Score fields are placeholders for a future optimizer (R7+); they are not computed
//! into a real refinement plan here.

use crate::QualityLevel;
use earthmesh_geometry::safety::{
    degree_buffer_warnings, max_abs_latitude, spans_dateline, validate_polygon, GeometryQualityFlag,
};
use earthmesh_geometry::{haversine_km, Point};

/// MERIT-Hydro tiles are 5°×5°.
pub const TILE_DEG: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LonLatBbox {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileBounds {
    pub name: String,
    pub bbox: LonLatBbox,
}

/// Placeholder priority/score fields (no optimizer here).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HydroCoastScores {
    pub hydro_score: f64,
    pub coast_score: f64,
    pub river_mouth_priority: f64,
    pub estuary_priority: f64,
    pub coupling_priority: f64,
}

/// Facts the caller extracts from the MERIT pipeline.
#[derive(Clone, Debug, Default)]
pub struct HydroCoastInputs {
    pub merit_root_exists: bool,
    pub bbox: Option<LonLatBbox>,
    pub stride: u32,
    pub selected_tiles: Vec<TileBounds>,
    pub nodata_count: u64,
    pub river_feature_count: usize,
    pub coast_feature_count: usize,
    /// Cells/features classified as both river and coast (priority conflict).
    pub overlap_count: usize,
    pub river_mouth_candidate_count: usize,
    pub masks_by_class: Vec<(String, usize)>,
    pub features_dropped_by_simplify: usize,
    /// Close-mask rings (lon/lat) to validate for self-intersection etc.
    pub close_mask_rings: Vec<Vec<Point>>,
    /// Buffer distance used (degrees) — flagged at high latitude / large span.
    pub buffer_deg: f64,
    /// Units, recorded explicitly for reproducibility.
    pub river_width_unit: String,
    pub upstream_area_unit: String,
}

#[derive(Clone, Debug)]
pub struct HydroCoastValidationReport {
    pub merit_root_exists: bool,
    pub bbox: Option<LonLatBbox>,
    pub bbox_valid: bool,
    pub crosses_dateline: bool,
    pub stride: u32,
    pub selected_tiles: Vec<String>,
    pub expected_tile_count: usize,
    pub coverage_fraction: f64,
    pub nodata_count: u64,
    pub river_feature_count: usize,
    pub coast_feature_count: usize,
    pub overlap_count: usize,
    pub river_mouth_candidate_count: usize,
    pub masks_by_class: Vec<(String, usize)>,
    pub features_dropped_by_simplify: usize,
    pub river_width_unit: String,
    pub upstream_area_unit: String,
    pub warnings: Vec<String>,
    pub geometry_flags: Vec<String>,
    pub recommended_fixes: Vec<String>,
    pub scores: HydroCoastScores,
    pub severity: QualityLevel,
}

/// Validate a bbox: returns (valid, crosses_dateline). A bbox with `west > east` is
/// interpreted as crossing the antimeridian (valid, flagged), not invalid.
pub fn validate_bbox(bbox: &LonLatBbox) -> (bool, bool) {
    let lat_ok = bbox.south < bbox.north
        && (-90.0..=90.0).contains(&bbox.south)
        && (-90.0..=90.0).contains(&bbox.north);
    let lon_in_range =
        (-180.0..=180.0).contains(&bbox.west) && (-180.0..=180.0).contains(&bbox.east);
    let crosses = bbox.west > bbox.east || (bbox.east - bbox.west) > 180.0;
    (lat_ok && lon_in_range, crosses)
}

/// Expected 5° tiles to cover `bbox` and the fraction actually selected.
/// Antimeridian-crossing bboxes are split at ±180.
pub fn tile_coverage(selected: &[TileBounds], bbox: &LonLatBbox) -> (usize, f64) {
    let lon_segments: Vec<(f64, f64)> = if bbox.west <= bbox.east {
        vec![(bbox.west, bbox.east)]
    } else {
        vec![(bbox.west, 180.0), (-180.0, bbox.east)]
    };
    let mut expected = std::collections::BTreeSet::new();
    for (w, e) in lon_segments {
        let lon0 = (w / TILE_DEG).floor() as i64;
        let lon1 = ((e - 1e-9) / TILE_DEG).floor() as i64;
        let lat0 = (bbox.south / TILE_DEG).floor() as i64;
        let lat1 = ((bbox.north - 1e-9) / TILE_DEG).floor() as i64;
        for lo in lon0..=lon1 {
            for la in lat0..=lat1 {
                expected.insert((lo, la));
            }
        }
    }
    let expected_count = expected.len().max(1);
    // count expected tiles that have a selected tile covering their SW corner
    let mut covered = 0;
    for (lo, la) in &expected {
        let cx = (*lo as f64 + 0.5) * TILE_DEG;
        let cy = (*la as f64 + 0.5) * TILE_DEG;
        if selected.iter().any(|t| {
            cx >= t.bbox.west && cx < t.bbox.east && cy >= t.bbox.south && cy < t.bbox.north
        }) {
            covered += 1;
        }
    }
    (expected_count, covered as f64 / expected_count as f64)
}

/// Count river points within `dist_km` of any coast point (river-mouth candidates).
/// O(n·m) — intended for the already-thinned feature sets, not raw pixels.
pub fn river_mouth_candidates(river_pts: &[Point], coast_pts: &[Point], dist_km: f64) -> usize {
    river_pts
        .iter()
        .filter(|&&r| coast_pts.iter().any(|&c| haversine_km(r, c) <= dist_km))
        .count()
}

/// Build the validation report from extracted facts.
pub fn build_report(inputs: &HydroCoastInputs) -> HydroCoastValidationReport {
    let mut warnings = Vec::new();
    let mut geometry_flags = Vec::new();
    let mut fixes = Vec::new();
    let mut severity = QualityLevel::Pass;
    let bump = |s: &mut QualityLevel, level: QualityLevel| {
        if matches!(level, QualityLevel::Fail)
            || (matches!(level, QualityLevel::Warn) && matches!(s, QualityLevel::Pass))
        {
            *s = level;
        }
    };

    if !inputs.merit_root_exists {
        bump(&mut severity, QualityLevel::Fail);
        fixes
            .push("set NL hydro root / EARTHMESH_DATA to an existing MERIT-Hydro directory".into());
        warnings.push("MERIT-Hydro root does not exist".into());
    }

    let (bbox_valid, crosses_dateline) = match &inputs.bbox {
        Some(b) => validate_bbox(b),
        None => (false, false),
    };
    if inputs.bbox.is_some() && !bbox_valid {
        bump(&mut severity, QualityLevel::Fail);
        fixes.push("fix bbox: south<north, lon/lat within range".into());
        warnings.push("bbox is invalid".into());
    }
    if crosses_dateline {
        bump(&mut severity, QualityLevel::Warn);
        warnings
            .push("bbox crosses the antimeridian (±180°) — tile selection split at 180°".into());
        fixes.push("verify antimeridian handling in tile selection (merit_bbox_intersects)".into());
    }

    let (expected_tile_count, coverage_fraction) = match &inputs.bbox {
        Some(b) => tile_coverage(&inputs.selected_tiles, b),
        None => (inputs.selected_tiles.len().max(1), 1.0),
    };
    if coverage_fraction < 1.0 {
        bump(&mut severity, QualityLevel::Warn);
        warnings.push(format!(
            "tile coverage {:.0}% (< 100%): some bbox area has no MERIT tile",
            coverage_fraction * 100.0
        ));
        fixes.push("add the missing MERIT tiles or shrink the bbox".into());
    }

    if inputs.stride > 1 {
        bump(&mut severity, QualityLevel::Warn);
        warnings.push(format!(
            "stride={} subsamples MERIT pixels — narrow rivers (< {} px) may be skipped",
            inputs.stride, inputs.stride
        ));
        fixes.push(
            "use stride=1 near narrow channels, or aggregate (max-pool) instead of subsample"
                .into(),
        );
    }

    if inputs.nodata_count > 0 {
        warnings.push(format!("{} nodata pixels encountered", inputs.nodata_count));
    }

    if inputs.overlap_count > 0 {
        bump(&mut severity, QualityLevel::Warn);
        warnings.push(format!(
            "{} river/coast class overlap conflicts (priority: river > coast)",
            inputs.overlap_count
        ));
        fixes.push(
            "apply explicit river>coast priority, or split river-mouth as its own class".into(),
        );
    }

    // degree-buffer / high-latitude warnings (reuse geometry safety layer)
    if let Some(b) = &inputs.bbox {
        let max_lat = b.south.abs().max(b.north.abs());
        let lon_span = (b.east - b.west).abs();
        if inputs.buffer_deg > 0.0 {
            for flag in degree_buffer_warnings(inputs.buffer_deg, max_lat, lon_span) {
                if !matches!(flag, GeometryQualityFlag::PlanarAreaUsedWarning) {
                    bump(&mut severity, QualityLevel::Warn);
                }
                geometry_flags.push(flag.as_str().to_string());
            }
            if max_lat >= 60.0 {
                fixes.push(
                    "use km buffer under a local equal-area projection at high latitude".into(),
                );
            }
        }
    }

    // validate close-mask rings (self-intersection / degenerate / dateline / polar)
    let mut self_intersecting = 0;
    for ring in &inputs.close_mask_rings {
        let flags = validate_polygon(ring);
        if flags.contains(&GeometryQualityFlag::SelfIntersection) {
            self_intersecting += 1;
        }
        for f in &flags {
            let s = f.as_str().to_string();
            if !geometry_flags.contains(&s) {
                geometry_flags.push(s);
            }
        }
        if !ring.is_empty() && max_abs_latitude(ring) >= 75.0 {
            // already covered by validate_polygon's polar flag; keep severity bump
        }
        if spans_dateline(ring) {
            bump(&mut severity, QualityLevel::Warn);
        }
    }
    if self_intersecting > 0 {
        bump(&mut severity, QualityLevel::Fail);
        warnings.push(format!("{self_intersecting} close mask(s) self-intersect"));
        fixes.push("reject/repair self-intersecting close masks before refinement".into());
    }

    if inputs.features_dropped_by_simplify > 0 {
        bump(&mut severity, QualityLevel::Warn);
        warnings.push(format!(
            "{} feature(s) dropped by simplify — narrow channels may be lost",
            inputs.features_dropped_by_simplify
        ));
        fixes.push("lower simplify tolerance or protect narrow-channel vertices".into());
    }

    // composite duplicate/overlap by class+degree
    let composite_dups = composite_duplicate_count(&inputs.masks_by_class);
    if composite_dups > 0 {
        warnings.push(format!(
            "{composite_dups} class with > expected masks — possible composite duplication"
        ));
    }

    if inputs.river_width_unit.is_empty() || inputs.upstream_area_unit.is_empty() {
        warnings.push("river width / upstream area units not recorded".into());
        fixes.push(
            "record river_width_unit (m) and upstream_area_unit (km²) for reproducibility".into(),
        );
    }

    HydroCoastValidationReport {
        merit_root_exists: inputs.merit_root_exists,
        bbox: inputs.bbox,
        bbox_valid,
        crosses_dateline,
        stride: inputs.stride,
        selected_tiles: inputs
            .selected_tiles
            .iter()
            .map(|t| t.name.clone())
            .collect(),
        expected_tile_count,
        coverage_fraction,
        nodata_count: inputs.nodata_count,
        river_feature_count: inputs.river_feature_count,
        coast_feature_count: inputs.coast_feature_count,
        overlap_count: inputs.overlap_count,
        river_mouth_candidate_count: inputs.river_mouth_candidate_count,
        masks_by_class: inputs.masks_by_class.clone(),
        features_dropped_by_simplify: inputs.features_dropped_by_simplify,
        river_width_unit: inputs.river_width_unit.clone(),
        upstream_area_unit: inputs.upstream_area_unit.clone(),
        warnings,
        geometry_flags,
        recommended_fixes: fixes,
        scores: HydroCoastScores::default(),
        severity,
    }
}

fn composite_duplicate_count(masks_by_class: &[(String, usize)]) -> usize {
    // a class appearing more than once in the list = duplicated grouping
    let mut seen = std::collections::BTreeMap::new();
    for (class, _) in masks_by_class {
        *seen.entry(class.clone()).or_insert(0usize) += 1;
    }
    seen.values().filter(|&&c| c > 1).count()
}

impl HydroCoastValidationReport {
    pub fn to_json(&self) -> String {
        let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
        let arr = |items: &[String]| {
            if items.is_empty() {
                "[]".to_string()
            } else {
                format!(
                    "[{}]",
                    items
                        .iter()
                        .map(|s| format!("\"{}\"", esc(s)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        };
        let mut s = String::from("{\n  \"kind\": \"earthmesh_hydro_coast_validation\",\n");
        s.push_str(&format!(
            "  \"severity\": \"{}\",\n",
            self.severity.as_str()
        ));
        s.push_str(&format!(
            "  \"merit_root_exists\": {},\n",
            self.merit_root_exists
        ));
        s.push_str(&format!("  \"bbox_valid\": {},\n", self.bbox_valid));
        s.push_str(&format!(
            "  \"crosses_dateline\": {},\n",
            self.crosses_dateline
        ));
        s.push_str(&format!("  \"stride\": {},\n", self.stride));
        s.push_str(&format!(
            "  \"selected_tiles\": {},\n",
            arr(&self.selected_tiles)
        ));
        s.push_str(&format!(
            "  \"expected_tile_count\": {},\n",
            self.expected_tile_count
        ));
        s.push_str(&format!(
            "  \"coverage_fraction\": {},\n",
            self.coverage_fraction
        ));
        s.push_str(&format!("  \"nodata_count\": {},\n", self.nodata_count));
        s.push_str(&format!(
            "  \"river_feature_count\": {},\n",
            self.river_feature_count
        ));
        s.push_str(&format!(
            "  \"coast_feature_count\": {},\n",
            self.coast_feature_count
        ));
        s.push_str(&format!("  \"overlap_count\": {},\n", self.overlap_count));
        s.push_str(&format!(
            "  \"river_mouth_candidate_count\": {},\n",
            self.river_mouth_candidate_count
        ));
        s.push_str(&format!(
            "  \"features_dropped_by_simplify\": {},\n",
            self.features_dropped_by_simplify
        ));
        s.push_str(&format!("  \"warnings\": {},\n", arr(&self.warnings)));
        s.push_str(&format!(
            "  \"geometry_flags\": {},\n",
            arr(&self.geometry_flags)
        ));
        s.push_str(&format!(
            "  \"recommended_fixes\": {},\n",
            arr(&self.recommended_fixes)
        ));
        s.push_str("  \"scores\": {");
        s.push_str(&format!(
            "\"hydro_score\": {}, \"coast_score\": {}, \"river_mouth_priority\": {}, \
             \"estuary_priority\": {}, \"coupling_priority\": {}",
            self.scores.hydro_score,
            self.scores.coast_score,
            self.scores.river_mouth_priority,
            self.scores.estuary_priority,
            self.scores.coupling_priority
        ));
        s.push_str("}\n}\n");
        s
    }

    pub fn write_json(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_json())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(w: f64, e: f64, s: f64, n: f64) -> LonLatBbox {
        LonLatBbox {
            west: w,
            east: e,
            south: s,
            north: n,
        }
    }
    fn tile(name: &str, w: f64, s: f64) -> TileBounds {
        TileBounds {
            name: name.to_string(),
            bbox: bbox(w, w + TILE_DEG, s, s + TILE_DEG),
        }
    }

    #[test]
    fn no_tiles_found_reports_zero_coverage() {
        let (_expected, cov) = tile_coverage(&[], &bbox(112.0, 115.0, 22.0, 24.0));
        assert_eq!(cov, 0.0);
    }

    #[test]
    fn bbox_selects_expected_tiles() {
        // bbox within a single 110-115E, 20-25N tile
        let tiles = vec![tile("e110n20", 110.0, 20.0)];
        let (expected, cov) = tile_coverage(&tiles, &bbox(112.0, 114.0, 22.0, 24.0));
        assert_eq!(expected, 1);
        assert_eq!(cov, 1.0);
    }

    #[test]
    fn invalid_bbox_is_fail() {
        let mut inp = HydroCoastInputs {
            merit_root_exists: true,
            bbox: Some(bbox(10.0, 20.0, 30.0, 10.0)), // south > north
            ..Default::default()
        };
        inp.river_width_unit = "m".into();
        inp.upstream_area_unit = "km2".into();
        let r = build_report(&inp);
        assert!(!r.bbox_valid);
        assert_eq!(r.severity, QualityLevel::Fail);
    }

    #[test]
    fn high_latitude_degree_buffer_warns() {
        let inp = HydroCoastInputs {
            merit_root_exists: true,
            bbox: Some(bbox(10.0, 12.0, 78.0, 80.0)),
            buffer_deg: 0.1,
            river_width_unit: "m".into(),
            upstream_area_unit: "km2".into(),
            ..Default::default()
        };
        let r = build_report(&inp);
        assert!(r
            .geometry_flags
            .iter()
            .any(|f| f == "projection_distortion_warning"));
        assert_eq!(r.severity, QualityLevel::Warn);
    }

    #[test]
    fn stride_gt_one_warns() {
        let inp = HydroCoastInputs {
            merit_root_exists: true,
            bbox: Some(bbox(10.0, 12.0, 20.0, 22.0)),
            stride: 5,
            river_width_unit: "m".into(),
            upstream_area_unit: "km2".into(),
            ..Default::default()
        };
        let r = build_report(&inp);
        assert!(r.warnings.iter().any(|w| w.contains("stride")));
        assert_eq!(r.severity, QualityLevel::Warn);
    }

    #[test]
    fn river_coast_overlap_warns() {
        let inp = HydroCoastInputs {
            merit_root_exists: true,
            bbox: Some(bbox(10.0, 12.0, 20.0, 22.0)),
            overlap_count: 7,
            river_width_unit: "m".into(),
            upstream_area_unit: "km2".into(),
            ..Default::default()
        };
        let r = build_report(&inp);
        assert!(r.warnings.iter().any(|w| w.contains("overlap")));
        assert_eq!(r.severity, QualityLevel::Warn);
    }

    #[test]
    fn self_intersecting_close_mask_rejected() {
        let bowtie = vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 0.0),
            Point::new(0.0, 2.0),
        ];
        let inp = HydroCoastInputs {
            merit_root_exists: true,
            bbox: Some(bbox(0.0, 3.0, 0.0, 3.0)),
            close_mask_rings: vec![bowtie],
            river_width_unit: "m".into(),
            upstream_area_unit: "km2".into(),
            ..Default::default()
        };
        let r = build_report(&inp);
        assert!(r.geometry_flags.iter().any(|f| f == "self_intersection"));
        assert_eq!(r.severity, QualityLevel::Fail);
    }

    #[test]
    fn validation_report_written_and_has_fields() {
        let inp = HydroCoastInputs {
            merit_root_exists: true,
            bbox: Some(bbox(112.0, 114.0, 22.0, 24.0)),
            stride: 1,
            selected_tiles: vec![tile("e110n20", 110.0, 20.0)],
            river_feature_count: 12,
            coast_feature_count: 5,
            river_width_unit: "m".into(),
            upstream_area_unit: "km2".into(),
            ..Default::default()
        };
        let r = build_report(&inp);
        let json = r.to_json();
        for needle in [
            "earthmesh_hydro_coast_validation",
            "\"severity\"",
            "\"selected_tiles\"",
            "\"coverage_fraction\"",
            "\"river_feature_count\": 12",
            "\"scores\"",
            "hydro_score",
            "coupling_priority",
        ] {
            assert!(json.contains(needle), "json missing {needle}:\n{json}");
        }
        let dir = std::env::temp_dir().join(format!("em3_hydro_test_{}", std::process::id()));
        let path = dir.join("hydro_coast_validation.json");
        r.write_json(&path).expect("write");
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn river_mouth_candidates_counts_near_coast() {
        let rivers = vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)];
        let coasts = vec![Point::new(0.05, 0.0)];
        // first river ~5.5 km from coast, second far away
        assert_eq!(river_mouth_candidates(&rivers, &coasts, 10.0), 1);
    }
}
