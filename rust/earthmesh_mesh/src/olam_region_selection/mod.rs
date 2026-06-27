use crate::{
    olam_cartesian_xy_segment_distance, olam_closed_corridor_contains_cartesian,
    olam_corridor_radius_at_segment, olam_corridor_segment_distance_meters,
    olam_ec_ps_distance_meters, olam_open_corridor_contains_cartesian, CartesianPoint,
    LonLatDegrees, OlamRefinementRegion,
};

impl OlamRefinementRegion {
    pub(crate) fn anchor_lonlat(&self) -> LonLatDegrees {
        match self {
            Self::Circle { center, .. } => *center,
            Self::Bbox {
                west_degrees,
                south_degrees,
                ..
            } => LonLatDegrees::new(*west_degrees, *south_degrees),
            Self::Corridor { points, .. } | Self::Polygon { points, .. } => points[0],
        }
    }

    pub(crate) fn contains_cartesian(&self, point: CartesianPoint, radius: f64) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => olam_ec_ps_distance_meters(point, *center, radius) < *radius_meters,
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                let corners = [
                    LonLatDegrees::new(*west_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *north_degrees),
                    LonLatDegrees::new(*west_degrees, *north_degrees),
                ];
                olam_closed_corridor_contains_cartesian(point, &corners, radius, 2_000_000.0)
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return false;
                }
                points.windows(2).enumerate().any(|(idx, segment)| {
                    let (distance, t) = olam_corridor_segment_distance_meters(
                        point, segment[0], segment[1], radius,
                    );
                    distance < olam_corridor_radius_at_segment(radius_meters, idx, t)
                })
            }
            Self::Polygon { points, .. } => {
                olam_open_corridor_contains_cartesian(point, points, radius, 2_000_000.0)
            }
        }
    }

    pub(crate) fn close_to_cartesian(&self, point: CartesianPoint, radius: f64) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => olam_ec_ps_distance_meters(point, *center, radius) < radius_meters * 1.5,
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                let corners = [
                    LonLatDegrees::new(*west_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *north_degrees),
                    LonLatDegrees::new(*west_degrees, *north_degrees),
                ];
                olam_closed_corridor_contains_cartesian(point, &corners, radius, 2_000_000.0 * 1.2)
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) =
                    olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius);
                distance < olam_corridor_radius_at_segment(radius_meters, idx, t) * 1.2
            }),
            Self::Polygon { points, .. } => {
                olam_open_corridor_contains_cartesian(point, points, radius, 2_000_000.0 * 1.2)
            }
        }
    }

    pub(crate) fn contains_cartesian_xy(&self, point: CartesianPoint) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                let dx = point.x - center.lon_degrees;
                let dy = point.y - center.lat_degrees;
                dx.hypot(dy) < *radius_meters
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) =
                    olam_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                distance < olam_corridor_radius_at_segment(radius_meters, idx, t)
            }),
            Self::Bbox { .. } | Self::Polygon { .. } => false,
        }
    }

    pub(crate) fn close_to_cartesian_xy(&self, point: CartesianPoint) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                let dx = point.x - center.lon_degrees;
                let dy = point.y - center.lat_degrees;
                dx.hypot(dy) < radius_meters * 1.5
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) =
                    olam_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                distance < olam_corridor_radius_at_segment(radius_meters, idx, t) * 1.2
            }),
            Self::Bbox { .. } | Self::Polygon { .. } => false,
        }
    }
}
