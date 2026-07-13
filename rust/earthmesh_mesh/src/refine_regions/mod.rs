use crate::LonLatDegrees;

/// User-facing specified-region refinement request for the Method-C mesh layer.
#[derive(Debug, Clone, PartialEq)]
pub enum MethodCRefinementRegion {
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

pub(crate) const METHOD_C_MIN_GRID_SPACING_METERS: f64 = 0.001;

pub(crate) fn scale_method_c_refinement_regions_radius(
    regions: &[MethodCRefinementRegion],
    factor: f64,
) -> Option<Vec<MethodCRefinementRegion>> {
    if regions.is_empty() {
        return None;
    }
    regions
        .iter()
        .map(|region| scale_method_c_refinement_region_radius(region, factor))
        .collect()
}

pub(crate) fn scale_method_c_refinement_region_radius(
    region: &MethodCRefinementRegion,
    factor: f64,
) -> Option<MethodCRefinementRegion> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    match region {
        MethodCRefinementRegion::Circle {
            center,
            radius_meters,
            level,
        } => Some(MethodCRefinementRegion::Circle {
            center: *center,
            radius_meters: radius_meters * factor,
            level: *level,
        }),
        MethodCRefinementRegion::Corridor {
            points,
            radius_meters,
            level,
        } => Some(MethodCRefinementRegion::Corridor {
            points: points.clone(),
            radius_meters: radius_meters.iter().map(|radius| radius * factor).collect(),
            level: *level,
        }),
        MethodCRefinementRegion::Bbox { .. } | MethodCRefinementRegion::Polygon { .. } => None,
    }
}

impl MethodCRefinementRegion {
    pub fn level(&self) -> usize {
        match self {
            Self::Circle { level, .. }
            | Self::Bbox { level, .. }
            | Self::Corridor { level, .. }
            | Self::Polygon { level, .. } => *level,
        }
    }
}
