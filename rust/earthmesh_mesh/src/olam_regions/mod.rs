use crate::LonLatDegrees;

/// User-facing specified-region refinement request for the OLAM mesh layer.
#[derive(Debug, Clone, PartialEq)]
pub enum OlamRefinementRegion {
    Circle {
        center: LonLatDegrees,
        radius_meters: f64,
        level: usize,
    },
    Bbox {
        west_degrees: f64,
        east_degrees: f64,
        south_degrees: f64,
        north_degrees: f64,
        level: usize,
    },
    Corridor {
        points: Vec<LonLatDegrees>,
        radius_meters: Vec<f64>,
        level: usize,
    },
    Polygon {
        points: Vec<LonLatDegrees>,
        level: usize,
    },
}

pub(crate) const OLAM_METHOD_C_MIN_GRID_SPACING_METERS: f64 = 0.001;

pub(crate) fn scale_olam_refinement_regions_radius(
    regions: &[OlamRefinementRegion],
    factor: f64,
) -> Option<Vec<OlamRefinementRegion>> {
    if regions.is_empty() {
        return None;
    }
    regions
        .iter()
        .map(|region| scale_olam_refinement_region_radius(region, factor))
        .collect()
}

pub(crate) fn scale_olam_refinement_region_radius(
    region: &OlamRefinementRegion,
    factor: f64,
) -> Option<OlamRefinementRegion> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    match region {
        OlamRefinementRegion::Circle {
            center,
            radius_meters,
            level,
        } => Some(OlamRefinementRegion::Circle {
            center: *center,
            radius_meters: radius_meters * factor,
            level: *level,
        }),
        OlamRefinementRegion::Corridor {
            points,
            radius_meters,
            level,
        } => Some(OlamRefinementRegion::Corridor {
            points: points.clone(),
            radius_meters: radius_meters.iter().map(|radius| radius * factor).collect(),
            level: *level,
        }),
        OlamRefinementRegion::Bbox { .. } | OlamRefinementRegion::Polygon { .. } => None,
    }
}

impl OlamRefinementRegion {
    pub fn level(&self) -> usize {
        match self {
            Self::Circle { level, .. }
            | Self::Bbox { level, .. }
            | Self::Corridor { level, .. }
            | Self::Polygon { level, .. } => *level,
        }
    }
}
