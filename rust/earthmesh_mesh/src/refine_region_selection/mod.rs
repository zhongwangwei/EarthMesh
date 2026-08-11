use std::borrow::Cow;

use crate::{
    lonlat_degrees_to_unit_xyz, method_c_cartesian_xy_segment_distance,
    method_c_closed_corridor_contains_cartesian, method_c_corridor_radius_at_segment,
    method_c_corridor_segment_distance_meters, method_c_ec_ps_distance_meters,
    method_c_open_corridor_contains_cartesian, xyz_to_lonlat_degrees, CartesianPoint,
    LonLatDegrees, RefinementRegion,
};

#[derive(Clone, Copy, Debug)]
struct IndexedCircle {
    region: usize,
    center: CartesianPoint,
    latitude_radians: f64,
    angular_radius: f64,
    radius_meters: f64,
    level: usize,
}

#[derive(Clone, Debug)]
pub struct RefinementRegionIndex<'a> {
    regions: Cow<'a, [RefinementRegion]>,
    circles_by_latitude: Vec<IndexedCircle>,
    other_regions: Vec<usize>,
    maximum_circle_radius: f64,
}

impl<'a> RefinementRegionIndex<'a> {
    pub fn new(regions: &'a [RefinementRegion]) -> Self {
        Self::build(Cow::Borrowed(regions))
    }

    fn build(regions: Cow<'a, [RefinementRegion]>) -> Self {
        let mut circles_by_latitude = Vec::new();
        let mut other_regions = Vec::new();
        let mut maximum_circle_radius = 0.0f64;
        for (region, candidate) in regions.iter().enumerate() {
            match candidate {
                RefinementRegion::Circle {
                    center,
                    radius_meters,
                    ..
                } if center.lon_degrees.is_finite()
                    && center.lat_degrees.is_finite()
                    && (-90.0..=90.0).contains(&center.lat_degrees)
                    && radius_meters.is_finite()
                    && *radius_meters >= 0.0 =>
                {
                    let angular_radius = (radius_meters / earthmesh_core::EARTH_RADIUS_METERS
                        + 64.0 * f64::EPSILON)
                        .min(std::f64::consts::PI);
                    maximum_circle_radius = maximum_circle_radius.max(angular_radius);
                    circles_by_latitude.push(IndexedCircle {
                        region,
                        center: lonlat_degrees_to_unit_xyz(*center),
                        latitude_radians: center.lat_degrees.to_radians(),
                        angular_radius,
                        radius_meters: *radius_meters,
                        level: candidate.level(),
                    });
                }
                _ => other_regions.push(region),
            }
        }
        circles_by_latitude.sort_by(|left, right| {
            left.latitude_radians
                .total_cmp(&right.latitude_radians)
                .then_with(|| left.region.cmp(&right.region))
        });
        Self {
            regions,
            circles_by_latitude,
            other_regions,
            maximum_circle_radius,
        }
    }

    pub fn contains_lonlat_canonical(&self, point: LonLatDegrees, minimum_level: usize) -> bool {
        if !point.lat_degrees.is_finite() || !(-90.0..=90.0).contains(&point.lat_degrees) {
            return false;
        }
        let latitude = point.lat_degrees.to_radians();
        let from = self.circles_by_latitude.partition_point(|circle| {
            circle.latitude_radians < latitude - self.maximum_circle_radius
        });
        let to = self.circles_by_latitude.partition_point(|circle| {
            circle.latitude_radians <= latitude + self.maximum_circle_radius
        });
        self.circles_by_latitude[from..to]
            .iter()
            .filter(|circle| self.regions[circle.region].level() >= minimum_level)
            .any(|circle| {
                (circle.latitude_radians - latitude).abs() <= circle.angular_radius
                    && self.regions[circle.region].contains_lonlat_canonical(point)
            })
            || self.other_regions.iter().copied().any(|region| {
                self.regions[region].level() >= minimum_level
                    && self.regions[region].contains_lonlat_canonical(point)
            })
    }

    /// Test only circle regions with the great-circle metric used by adaptive
    /// demand reports. Latitude pruning changes no containment result.
    pub fn contains_lonlat_great_circle(&self, point: LonLatDegrees, minimum_level: usize) -> bool {
        if !point.lon_degrees.is_finite()
            || !point.lat_degrees.is_finite()
            || !(-90.0..=90.0).contains(&point.lat_degrees)
        {
            return false;
        }
        let latitude = point.lat_degrees.to_radians();
        let from = self.circles_by_latitude.partition_point(|circle| {
            circle.latitude_radians < latitude - self.maximum_circle_radius
        });
        let to = self.circles_by_latitude.partition_point(|circle| {
            circle.latitude_radians <= latitude + self.maximum_circle_radius
        });
        let point = lonlat_degrees_to_unit_xyz(point);
        self.circles_by_latitude[from..to]
            .iter()
            .filter(|circle| circle.level >= minimum_level)
            .any(|circle| {
                (circle.latitude_radians - latitude).abs() <= circle.angular_radius
                    && great_circle_distance_meters(point, circle.center) <= circle.radius_meters
            })
            || self.other_regions.iter().copied().any(|region| {
                self.regions[region].level() >= minimum_level
                    && circle_contains_lonlat_great_circle(&self.regions[region], point)
            })
    }
}

impl RefinementRegionIndex<'static> {
    pub fn from_owned(regions: Vec<RefinementRegion>) -> Self {
        Self::build(Cow::Owned(regions))
    }
}

fn great_circle_distance_meters(left: CartesianPoint, right: CartesianPoint) -> f64 {
    let cross = crate::cross(left, right);
    crate::magnitude(cross).atan2(crate::dot(left, right)) * earthmesh_core::EARTH_RADIUS_METERS
}

fn circle_contains_lonlat_great_circle(region: &RefinementRegion, point: CartesianPoint) -> bool {
    let RefinementRegion::Circle {
        center,
        radius_meters,
        ..
    } = region
    else {
        return false;
    };
    great_circle_distance_meters(point, lonlat_degrees_to_unit_xyz(*center)) <= *radius_meters
}

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
            Self::Polygon { points, .. } => {
                spherical_lonlat_in_polygon(xyz_to_lonlat_degrees(point), points)
            }
        }
    }

    /// Test a geographic point with the explicit Canonical Method-C metric.
    ///
    /// Circle radii use the legacy `ec_ps` polar-stereographic distance and
    /// corridor radii use the legacy per-segment stereographic projection with
    /// endpoint-radius interpolation. Bboxes preserve directed lon/lat bounds;
    /// polygon edges are minor great-circle arcs and use spherical winding.
    ///
    /// This entry point intentionally fixes the Earth radius to EarthMesh's
    /// shared core constant so feature toggles cannot silently change the
    /// interpretation of `radius_meters`.
    pub fn contains_lonlat_canonical(&self, point: LonLatDegrees) -> bool {
        if !point.lon_degrees.is_finite()
            || !point.lat_degrees.is_finite()
            || !(-90.0..=90.0).contains(&point.lat_degrees)
        {
            return false;
        }
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

fn lonlat_in_bbox(point: LonLatDegrees, west: f64, east: f64, south: f64, north: f64) -> bool {
    if !point.lon_degrees.is_finite()
        || !point.lat_degrees.is_finite()
        || !(-90.0..=90.0).contains(&point.lat_degrees)
        || point.lat_degrees < south
        || point.lat_degrees > north
    {
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

fn spherical_lonlat_in_polygon(point: LonLatDegrees, polygon: &[LonLatDegrees]) -> bool {
    earthmesh_boundary::spherical_ring_contains_minor(
        polygon,
        point.lon_degrees,
        point.lat_degrees,
        |point| (point.lon_degrees, point.lat_degrees),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lonlat_bbox_rejects_nonfinite_and_invalid_query_coordinates() {
        let global = RefinementRegion::Bbox {
            west_degrees: -180.0,
            east_degrees: 180.0,
            south_degrees: -90.0,
            north_degrees: 90.0,
            level: 1,
        };
        for point in [
            LonLatDegrees::new(f64::NAN, 0.0),
            LonLatDegrees::new(f64::INFINITY, 0.0),
            LonLatDegrees::new(0.0, f64::NAN),
            LonLatDegrees::new(0.0, f64::NEG_INFINITY),
            LonLatDegrees::new(0.0, 91.0),
        ] {
            assert!(!lonlat_in_bbox(point, -180.0, 180.0, -90.0, 90.0));
            assert!(!global.contains_lonlat_canonical(point));
        }
    }

    #[test]
    fn region_index_matches_brute_force_at_dateline_poles_and_levels() {
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(179.0, 0.0),
                radius_meters: 500_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(-45.0, 88.0),
                radius_meters: 750_000.0,
                level: 2,
            },
            RefinementRegion::Bbox {
                west_degrees: 10.0,
                east_degrees: 20.0,
                south_degrees: -5.0,
                north_degrees: 5.0,
                level: 1,
            },
        ];
        let index = RefinementRegionIndex::new(&regions);
        for point in [
            LonLatDegrees::new(-179.0, 0.0),
            LonLatDegrees::new(120.0, 89.0),
            LonLatDegrees::new(15.0, 0.0),
            LonLatDegrees::new(0.0, -60.0),
        ] {
            for level in 0..=2 {
                let expected = regions.iter().any(|region| {
                    region.level() >= level && region.contains_lonlat_canonical(point)
                });
                assert_eq!(index.contains_lonlat_canonical(point, level), expected);
            }
        }
    }

    #[test]
    fn great_circle_index_handles_dateline_poles_and_ignores_other_shapes() {
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(179.0, 0.0),
                radius_meters: 500_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(-45.0, 88.0),
                radius_meters: 750_000.0,
                level: 2,
            },
            RefinementRegion::Bbox {
                west_degrees: 10.0,
                east_degrees: 20.0,
                south_degrees: -5.0,
                north_degrees: 5.0,
                level: 3,
            },
        ];
        let index = RefinementRegionIndex::new(&regions);
        for point in [
            LonLatDegrees::new(-179.0, 0.0),
            LonLatDegrees::new(120.0, 89.0),
            LonLatDegrees::new(15.0, 0.0),
            LonLatDegrees::new(0.0, -60.0),
        ] {
            let point_xyz = lonlat_degrees_to_unit_xyz(point);
            for level in 0..=3 {
                let expected = regions.iter().any(|region| {
                    region.level() >= level
                        && circle_contains_lonlat_great_circle(region, point_xyz)
                });
                assert_eq!(index.contains_lonlat_great_circle(point, level), expected);
            }
        }
        assert!(!index.contains_lonlat_great_circle(LonLatDegrees::new(f64::NAN, 0.0), 1));
    }
}
