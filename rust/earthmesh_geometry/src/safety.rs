//! Geometry safety layer (MVP): explicit validation flags for masks, overlay,
//! hydro-coast and land/ocean fraction geometry.
//!
//! This is an additive layer — it does **not** rewrite the planar polygon engine
//! (`polygon_area` / `clip_convex_polygon` / `intersection_area` / `overlay_cell`).
//! It surfaces *what could be wrong* so callers can warn, repair, or block instead
//! of silently producing a bad mask/fraction.
//!
//! ## Preview vs production vs planar vs spherical
//! - **Preview geometry** (GUI map draw, Web-Mercator tiles): display only, never
//!   feeds area/fraction/conservation. Do not validate-or-trust it for production.
//! - **Production geometry** (this crate's `overlay_cell` + hydro masks): the values
//!   that drive refinement/coupling — validate it.
//! - **Planar approximation**: every area here is a *planar* shoelace in (lon, lat)
//!   degrees ([`AreaModel::PlanarDegree`]). Fraction ratios are ~OK for small cells
//!   but absolute areas and high-latitude / large-span cells are distorted →
//!   [`GeometryQualityFlag::PlanarAreaUsedWarning`].
//! - **Spherical / projected future path**: replace planar area with a spherical
//!   polygon area or a local equal-area projection ([`AreaModel::SphericalMeters`] /
//!   [`AreaModel::LocalEqualAreaProjected`]) — see the R3 report's roadmap.

use crate::{area_judge_first_self_intersection_one_based, polygon_area, Point};

/// |lat| above which planar geometry is flagged as polar-distorted for polygons.
pub const POLAR_LAT_WARN_DEG: f64 = 75.0;
/// |lat| above which a degree-based buffer is flagged as projection-distorted.
pub const BUFFER_DISTORTION_LAT_DEG: f64 = 60.0;
/// Longitude span above which a degree-based buffer is flagged as too wide for a
/// single planar approximation.
pub const BUFFER_DISTORTION_SPAN_DEG: f64 = 30.0;
/// A longitude range wider than this is treated as crossing the antimeridian.
pub const DATELINE_SPAN_DEG: f64 = 180.0;
/// Vertices closer than this (in degrees) are considered duplicates.
pub const VERTEX_EPS_DEG: f64 = 1.0e-12;
/// Areas at or below this (planar degree²) are treated as zero.
pub const AREA_EPS: f64 = 1.0e-12;

/// Actionable advice attached to degree-buffer warnings.
pub const KM_BUFFER_ADVICE: &str =
    "degree-based buffer distorts with latitude; use a meter/km buffer under a local \
     equal-area (or azimuthal-equidistant) projection, then reproject back to lon/lat";

/// What model produced an area/fraction — lets callers know how much to trust it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaModel {
    /// Planar shoelace in (lon, lat) degrees — current default; distorts with latitude.
    PlanarDegree,
    /// True spherical polygon area in m² (future).
    SphericalMeters,
    /// Planar area under a local equal-area projection in m² (future).
    LocalEqualAreaProjected,
}

/// Distinguishes display-only geometry from geometry that drives production output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryKind {
    /// GUI map draw / tiles — never used for area/fraction/conservation.
    Preview,
    /// Mask / overlay / fraction geometry that drives refinement and coupling.
    Production,
}

/// Explicit geometry warning/error flags. `as_str` matches the compatibility string flags
/// already emitted by `overlay_cell` (`zero_area_cell`, `missing_mask`) so existing
/// `Vec<String>` consumers keep working.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryQualityFlag {
    ZeroAreaCell,
    InvalidPolygon,
    SelfIntersection,
    DuplicateVertex,
    DatelineCrossing,
    PolarRegionWarning,
    PlanarAreaUsedWarning,
    ProjectionDistortionWarning,
    MaskOverlapConflict,
    MissingMask,
    UnresolvedFractionSumError,
    NegativeArea,
    NonFiniteCoordinate,
}

impl GeometryQualityFlag {
    pub fn as_str(&self) -> &'static str {
        match self {
            GeometryQualityFlag::ZeroAreaCell => "zero_area_cell",
            GeometryQualityFlag::InvalidPolygon => "invalid_polygon",
            GeometryQualityFlag::SelfIntersection => "self_intersection",
            GeometryQualityFlag::DuplicateVertex => "duplicate_vertex",
            GeometryQualityFlag::DatelineCrossing => "dateline_crossing",
            GeometryQualityFlag::PolarRegionWarning => "polar_region_warning",
            GeometryQualityFlag::PlanarAreaUsedWarning => "planar_area_used_warning",
            GeometryQualityFlag::ProjectionDistortionWarning => "projection_distortion_warning",
            GeometryQualityFlag::MaskOverlapConflict => "mask_overlap_conflict",
            GeometryQualityFlag::MissingMask => "missing_mask",
            GeometryQualityFlag::UnresolvedFractionSumError => "unresolved_fraction_sum_error",
            GeometryQualityFlag::NegativeArea => "negative_area",
            GeometryQualityFlag::NonFiniteCoordinate => "non_finite_coordinate",
        }
    }
}

fn points_almost_equal(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= VERTEX_EPS_DEG && (a.y - b.y).abs() <= VERTEX_EPS_DEG
}

fn unique_vertex_count(points: &[Point]) -> usize {
    let mut count = 0;
    for (i, p) in points.iter().enumerate() {
        if !points[..i].iter().any(|q| points_almost_equal(*q, *p)) {
            count += 1;
        }
    }
    count
}

/// True if the (lon) range suggests the ring crosses the antimeridian (±180°).
pub fn spans_dateline(points: &[Point]) -> bool {
    let finite: Vec<f64> = points
        .iter()
        .map(|p| p.x)
        .filter(|x| x.is_finite())
        .collect();
    let (Some(min), Some(max)) = (
        finite.iter().cloned().reduce(f64::min),
        finite.iter().cloned().reduce(f64::max),
    ) else {
        return false;
    };
    max - min > DATELINE_SPAN_DEG
}

/// Largest absolute latitude among finite vertices.
pub fn max_abs_latitude(points: &[Point]) -> f64 {
    points
        .iter()
        .map(|p| p.y)
        .filter(|y| y.is_finite())
        .fold(0.0_f64, |acc, y| acc.max(y.abs()))
}

/// True if the ring is wound clockwise (signed planar area < 0).
pub fn ring_is_clockwise(points: &[Point]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut total = 0.0;
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        total += p.x * q.y - q.x * p.y;
    }
    total < 0.0
}

/// Validate a production polygon (mask ring or cell). Returns all triggered flags.
/// Empty result = no problems detected by this MVP layer.
pub fn validate_polygon(points: &[Point]) -> Vec<GeometryQualityFlag> {
    let mut flags = Vec::new();

    let any_non_finite = points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite());
    if any_non_finite {
        flags.push(GeometryQualityFlag::NonFiniteCoordinate);
    }

    let n = points.len();
    if n > 1 {
        let has_consecutive_dup =
            (0..n).any(|i| points_almost_equal(points[i], points[(i + 1) % n]));
        if has_consecutive_dup {
            flags.push(GeometryQualityFlag::DuplicateVertex);
        }
    }

    if unique_vertex_count(points) < 3 {
        flags.push(GeometryQualityFlag::InvalidPolygon);
    } else if !any_non_finite {
        let area = polygon_area(points);
        if !area.is_finite() {
            if !flags.contains(&GeometryQualityFlag::NonFiniteCoordinate) {
                flags.push(GeometryQualityFlag::NonFiniteCoordinate);
            }
        } else if area <= AREA_EPS {
            flags.push(GeometryQualityFlag::ZeroAreaCell);
        }
        // Self-intersection is checked independently of area: a symmetric bow-tie
        // has zero shoelace area (its two triangles cancel) yet is self-intersecting.
        // Collinear/degenerate rings give zero strict cross-products, so they do not
        // false-positive here.
        if area_judge_first_self_intersection_one_based(points).is_some() {
            flags.push(GeometryQualityFlag::SelfIntersection);
        }
    }

    if spans_dateline(points) {
        flags.push(GeometryQualityFlag::DatelineCrossing);
    }
    if max_abs_latitude(points) >= POLAR_LAT_WARN_DEG {
        flags.push(GeometryQualityFlag::PolarRegionWarning);
    }
    flags
}

/// Validate a set of **mutually-exclusive** partition fractions (e.g. land/ocean/coast
/// of one cell that should sum to 1). Distinct from `overlay_cell`'s overlapping
/// per-class coverage, where a sum > 1 is legitimate overlap, not an error.
pub fn validate_fraction_partition(fractions: &[f64], tolerance: f64) -> Vec<GeometryQualityFlag> {
    let mut flags = Vec::new();
    if fractions.iter().any(|f| !f.is_finite()) {
        flags.push(GeometryQualityFlag::NonFiniteCoordinate);
        return flags;
    }
    if fractions.iter().any(|f| *f < -tolerance) {
        flags.push(GeometryQualityFlag::NegativeArea);
    }
    let sum: f64 = fractions.iter().sum();
    if (sum - 1.0).abs() > tolerance {
        flags.push(GeometryQualityFlag::UnresolvedFractionSumError);
    }
    flags
}

/// Flags for a degree-based buffer at a given latitude / longitude span. Always
/// flags planar use; adds projection-distortion / polar warnings when the buffer
/// would be badly distorted. Pair with [`KM_BUFFER_ADVICE`].
pub fn degree_buffer_warnings(
    _buffer_deg: f64,
    max_abs_lat_deg: f64,
    lon_span_deg: f64,
) -> Vec<GeometryQualityFlag> {
    let mut flags = vec![GeometryQualityFlag::PlanarAreaUsedWarning];
    if max_abs_lat_deg >= BUFFER_DISTORTION_LAT_DEG || lon_span_deg >= BUFFER_DISTORTION_SPAN_DEG {
        flags.push(GeometryQualityFlag::ProjectionDistortionWarning);
    }
    if max_abs_lat_deg >= POLAR_LAT_WARN_DEG {
        flags.push(GeometryQualityFlag::PolarRegionWarning);
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    fn square() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ]
    }

    #[test]
    fn valid_square_has_no_flags() {
        assert!(validate_polygon(&square()).is_empty());
    }

    #[test]
    fn zero_area_polygon_flagged() {
        let collinear = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
        ];
        assert!(validate_polygon(&collinear).contains(&GeometryQualityFlag::ZeroAreaCell));
    }

    #[test]
    fn duplicate_consecutive_vertex_flagged() {
        let dup = vec![
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
        ];
        assert!(validate_polygon(&dup).contains(&GeometryQualityFlag::DuplicateVertex));
    }

    #[test]
    fn self_intersecting_polygon_flagged() {
        // bow-tie / figure-eight
        let bowtie = vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(2.0, 0.0),
            Point::new(0.0, 2.0),
        ];
        assert!(validate_polygon(&bowtie).contains(&GeometryQualityFlag::SelfIntersection));
    }

    #[test]
    fn fewer_than_three_unique_vertices_is_invalid() {
        let line = vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
        assert!(validate_polygon(&line).contains(&GeometryQualityFlag::InvalidPolygon));
    }

    #[test]
    fn non_finite_coordinate_flagged() {
        let bad = vec![
            Point::new(0.0, 0.0),
            Point::new(f64::NAN, 0.0),
            Point::new(1.0, 1.0),
        ];
        assert!(validate_polygon(&bad).contains(&GeometryQualityFlag::NonFiniteCoordinate));
    }

    #[test]
    fn dateline_crossing_flagged() {
        let across = vec![
            Point::new(179.0, 10.0),
            Point::new(-179.0, 10.0),
            Point::new(-179.0, 12.0),
            Point::new(179.0, 12.0),
        ];
        assert!(validate_polygon(&across).contains(&GeometryQualityFlag::DatelineCrossing));
    }

    #[test]
    fn high_latitude_polygon_flagged() {
        let polar = vec![
            Point::new(0.0, 80.0),
            Point::new(1.0, 80.0),
            Point::new(1.0, 81.0),
            Point::new(0.0, 81.0),
        ];
        assert!(validate_polygon(&polar).contains(&GeometryQualityFlag::PolarRegionWarning));
    }

    #[test]
    fn high_latitude_degree_buffer_warns_projection_distortion() {
        let flags = degree_buffer_warnings(0.1, 80.0, 1.0);
        assert!(flags.contains(&GeometryQualityFlag::PlanarAreaUsedWarning));
        assert!(flags.contains(&GeometryQualityFlag::ProjectionDistortionWarning));
        assert!(flags.contains(&GeometryQualityFlag::PolarRegionWarning));
    }

    #[test]
    fn low_latitude_small_degree_buffer_only_planar_warning() {
        let flags = degree_buffer_warnings(0.05, 10.0, 1.0);
        assert_eq!(flags, vec![GeometryQualityFlag::PlanarAreaUsedWarning]);
    }

    #[test]
    fn fraction_partition_sum_over_one_flagged() {
        let flags = validate_fraction_partition(&[0.6, 0.6], 1.0e-6);
        assert!(flags.contains(&GeometryQualityFlag::UnresolvedFractionSumError));
    }

    #[test]
    fn fraction_partition_sum_one_is_clean() {
        let flags = validate_fraction_partition(&[0.4, 0.6], 1.0e-6);
        assert!(flags.is_empty());
    }

    #[test]
    fn flag_strings_match_compatibility() {
        assert_eq!(GeometryQualityFlag::ZeroAreaCell.as_str(), "zero_area_cell");
        assert_eq!(GeometryQualityFlag::MissingMask.as_str(), "missing_mask");
    }
}
