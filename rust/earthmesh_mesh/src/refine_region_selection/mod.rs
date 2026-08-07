use crate::{
    lonlat_degrees_to_unit_xyz, method_c_cartesian_xy_segment_distance,
    method_c_closed_corridor_contains_cartesian, method_c_corridor_radius_at_segment,
    method_c_corridor_segment_distance_meters, method_c_ec_ps_distance_meters,
    method_c_open_corridor_contains_cartesian, xyz_to_lonlat_degrees, CartesianPoint,
    LonLatDegrees, RefinementRegion,
};

impl RefinementRegion {
    pub fn anchor_lonlat(&self) -> LonLatDegrees {
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

    pub fn contains_cartesian(&self, point: CartesianPoint, radius: f64) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => method_c_ec_ps_distance_meters(point, *center, radius) < *radius_meters,
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => lonlat_in_bbox(
                xyz_to_lonlat_degrees(point),
                *west_degrees,
                *east_degrees,
                *south_degrees,
                *north_degrees,
            ),
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return false;
                }
                points.windows(2).enumerate().any(|(idx, segment)| {
                    let (distance, t) = method_c_corridor_segment_distance_meters(
                        point, segment[0], segment[1], radius,
                    );
                    distance < method_c_corridor_radius_at_segment(radius_meters, idx, t)
                })
            }
            Self::Polygon { points, .. } => lonlat_in_polygon(xyz_to_lonlat_degrees(point), points),
        }
    }

    /// Test a geographic point with the explicit Canonical Method-C metric.
    ///
    /// Circle radii use the legacy `ec_ps` polar-stereographic distance and
    /// corridor radii use the legacy per-segment stereographic projection with
    /// endpoint-radius interpolation. Bboxes and polygons preserve Canonical
    /// lon/lat containment; polygon edges are planar after longitude unwrapping,
    /// not spherical geodesics, and are unsuitable for polar-cap geometry.
    ///
    /// This entry point intentionally fixes the Earth radius to EarthMesh's
    /// shared core constant so feature toggles cannot silently change the
    /// interpretation of `radius_meters`.
    pub fn contains_lonlat_canonical(&self, point: LonLatDegrees) -> bool {
        self.contains_cartesian(
            lonlat_degrees_to_unit_xyz(point),
            earthmesh_core::EARTH_RADIUS_METERS,
        )
    }

    pub fn close_to_cartesian(&self, point: CartesianPoint, radius: f64) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => method_c_ec_ps_distance_meters(point, *center, radius) < radius_meters * 1.5,
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                if self.contains_cartesian(point, radius) {
                    return true;
                }
                let boundary = directed_bbox_boundary(
                    *west_degrees,
                    *east_degrees,
                    *south_degrees,
                    *north_degrees,
                );
                method_c_closed_corridor_contains_cartesian(
                    point,
                    &boundary,
                    radius,
                    2_000_000.0 * 1.2,
                )
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) = method_c_corridor_segment_distance_meters(
                    point, segment[0], segment[1], radius,
                );
                distance < method_c_corridor_radius_at_segment(radius_meters, idx, t) * 1.2
            }),
            Self::Polygon { points, .. } => {
                self.contains_cartesian(point, radius)
                    || method_c_open_corridor_contains_cartesian(
                        point,
                        points,
                        radius,
                        2_000_000.0 * 1.2,
                    )
            }
        }
    }

    pub fn contains_cartesian_xy(&self, point: CartesianPoint) -> bool {
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
                    method_c_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                distance < method_c_corridor_radius_at_segment(radius_meters, idx, t)
            }),
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => cartesian_xy_in_bbox(
                point,
                *west_degrees,
                *east_degrees,
                *south_degrees,
                *north_degrees,
            ),
            Self::Polygon { points, .. } => cartesian_xy_in_polygon(point, points),
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
                    method_c_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                distance < method_c_corridor_radius_at_segment(radius_meters, idx, t) * 1.2
            }),
            Self::Bbox { .. } | Self::Polygon { .. } => {
                self.cartesian_xy_outside_distance_meters(point) < 2_000_000.0 * 1.2
            }
        }
    }

    /// Euclidean distance in meters from a Cartesian-XY point to the outside
    /// of this region. Points inside return zero.
    pub fn cartesian_xy_outside_distance_meters(&self, point: CartesianPoint) -> f64 {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => ((point.x - center.lon_degrees).hypot(point.y - center.lat_degrees)
                - radius_meters)
                .max(0.0),
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points
                .windows(2)
                .enumerate()
                .map(|(idx, segment)| {
                    let (distance, t) =
                        method_c_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                    (distance - method_c_corridor_radius_at_segment(radius_meters, idx, t)).max(0.0)
                })
                .fold(f64::INFINITY, f64::min),
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                let dx = (west_degrees - point.x).max(0.0) + (point.x - east_degrees).max(0.0);
                let dy = (south_degrees - point.y).max(0.0) + (point.y - north_degrees).max(0.0);
                dx.hypot(dy)
            }
            Self::Polygon { points, .. } => {
                if cartesian_xy_in_polygon(point, points) {
                    return 0.0;
                }
                points
                    .iter()
                    .zip(points.iter().cycle().skip(1))
                    .take(points.len())
                    .map(|(&start, &end)| {
                        method_c_cartesian_xy_segment_distance(point, start, end).0
                    })
                    .fold(f64::INFINITY, f64::min)
            }
        }
    }
}

fn cartesian_xy_in_bbox(
    point: CartesianPoint,
    west: f64,
    east: f64,
    south: f64,
    north: f64,
) -> bool {
    point.x >= west && point.x <= east && point.y >= south && point.y <= north
}

fn cartesian_xy_in_polygon(point: CartesianPoint, polygon: &[LonLatDegrees]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    for (&a, &b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        let cross = (b.lon_degrees - a.lon_degrees) * (point.y - a.lat_degrees)
            - (b.lat_degrees - a.lat_degrees) * (point.x - a.lon_degrees);
        if cross.abs() <= 1e-12
            && point.x >= a.lon_degrees.min(b.lon_degrees) - 1e-12
            && point.x <= a.lon_degrees.max(b.lon_degrees) + 1e-12
            && point.y >= a.lat_degrees.min(b.lat_degrees) - 1e-12
            && point.y <= a.lat_degrees.max(b.lat_degrees) + 1e-12
        {
            return true;
        }
        if (a.lat_degrees > point.y) != (b.lat_degrees > point.y) {
            let x_intersect = a.lon_degrees
                + (point.y - a.lat_degrees) * (b.lon_degrees - a.lon_degrees)
                    / (b.lat_degrees - a.lat_degrees);
            if x_intersect >= point.x {
                inside = !inside;
            }
        }
    }
    inside
}

fn normalize_lon_around(lon: f64, anchor: f64) -> f64 {
    let mut lon = lon;
    while lon - anchor > 180.0 {
        lon -= 360.0;
    }
    while lon - anchor < -180.0 {
        lon += 360.0;
    }
    lon
}

fn lonlat_in_bbox(point: LonLatDegrees, west: f64, east: f64, south: f64, north: f64) -> bool {
    if point.lat_degrees < south || point.lat_degrees > north {
        return false;
    }
    if (east - west).abs() >= 360.0 - 1.0e-12 {
        return true;
    }
    let normalize = |lon: f64| (lon + 180.0).rem_euclid(360.0) - 180.0;
    let lon = normalize(point.lon_degrees);
    let west = normalize(west);
    let east = normalize(east);
    if west <= east {
        lon >= west && lon <= east
    } else {
        lon >= west || lon <= east
    }
}

fn directed_bbox_boundary(west: f64, east: f64, south: f64, north: f64) -> Vec<LonLatDegrees> {
    let raw_span = east - west;
    let eastward_span = if raw_span.abs() >= 360.0 - 1.0e-12 {
        360.0
    } else {
        raw_span.rem_euclid(360.0)
    };
    let segment_count = (eastward_span / 90.0).ceil().max(1.0) as usize;
    let longitude_at = |index: usize| {
        let lon = west + eastward_span * index as f64 / segment_count as f64;
        (lon + 180.0).rem_euclid(360.0) - 180.0
    };
    let mut boundary = Vec::with_capacity(2 * (segment_count + 1));
    for index in 0..=segment_count {
        boundary.push(LonLatDegrees::new(longitude_at(index), south));
    }
    for index in (0..=segment_count).rev() {
        boundary.push(LonLatDegrees::new(longitude_at(index), north));
    }
    boundary
}

fn lonlat_in_polygon(point: LonLatDegrees, polygon: &[LonLatDegrees]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let x = normalize_lon_around(point.lon_degrees, polygon[0].lon_degrees);
    let y = point.lat_degrees;
    let mut inside = false;
    for (a, b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        let ax = normalize_lon_around(a.lon_degrees, x);
        let bx = normalize_lon_around(b.lon_degrees, x);
        let ay = a.lat_degrees;
        let by = b.lat_degrees;
        let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
        if cross.abs() <= 1e-12
            && x >= ax.min(bx) - 1e-12
            && x <= ax.max(bx) + 1e-12
            && y >= ay.min(by) - 1e-12
            && y <= ay.max(by) + 1e-12
        {
            return true;
        }
        if (ay > y) != (by > y) {
            let x_intersect = ax + (y - ay) * (bx - ax) / (by - ay);
            if x_intersect >= x {
                inside = !inside;
            }
        }
    }
    inside
}
