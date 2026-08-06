use crate::LonLatDegrees;

/// User-facing specified-region refinement request for the Method-C mesh layer.
#[derive(Debug, Clone, PartialEq)]
/// A region a run asks to refine, in the shape a project describes one.
///
/// Shared: a circle, a corridor, a box or a closed curve means the same thing
/// whichever algorithm builds the mesh. What differs is whether the algorithm
/// can build *that* shape -- Method-C refuses shapes off its lattice, red-green
/// grows a marking until it is legal -- and that is the backends' business, not
/// this type's.
pub enum RefinementRegion {
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

pub(crate) fn scale_refinement_regions_radius(
    regions: &[RefinementRegion],
    factor: f64,
) -> Option<Vec<RefinementRegion>> {
    if regions.is_empty() {
        return None;
    }
    regions
        .iter()
        .map(|region| scale_refinement_region_radius(region, factor))
        .collect()
}

pub(crate) fn scale_refinement_region_radius(
    region: &RefinementRegion,
    factor: f64,
) -> Option<RefinementRegion> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    match region {
        RefinementRegion::Circle {
            center,
            radius_meters,
            level,
        } => Some(RefinementRegion::Circle {
            center: *center,
            radius_meters: radius_meters * factor,
            level: *level,
        }),
        RefinementRegion::Corridor {
            points,
            radius_meters,
            level,
        } => Some(RefinementRegion::Corridor {
            points: points.clone(),
            radius_meters: radius_meters.iter().map(|radius| radius * factor).collect(),
            level: *level,
        }),
        RefinementRegion::Bbox { .. } | RefinementRegion::Polygon { .. } => None,
    }
}

impl RefinementRegion {
    pub fn level(&self) -> usize {
        match self {
            Self::Circle { level, .. }
            | Self::Bbox { level, .. }
            | Self::Corridor { level, .. }
            | Self::Polygon { level, .. } => *level,
        }
    }
}
