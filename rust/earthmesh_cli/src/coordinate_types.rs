use earthmesh_geometry::EARTH_RADIUS_KM;

/// Shared longitude/latitude row used by circle and close masks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatPoint {
    pub lon: f64,
    pub lat: f64,
}

/// A geographic region used to carve a gridfile down to an area of interest.
#[derive(Debug, Clone)]
pub enum GridRegion {
    Bbox {
        west: f64,
        east: f64,
        north: f64,
        south: f64,
    },
    Circle {
        lon: f64,
        lat: f64,
        radius_km: f64,
    },
    Close {
        points: Vec<LonLatPoint>,
    },
    Any(Vec<GridRegion>),
}

impl GridRegion {
    pub(crate) fn contains(&self, lon: f64, lat: f64) -> bool {
        let norm = normalize_lon_degrees;
        match self {
            GridRegion::Bbox {
                west,
                east,
                north,
                south,
            } => {
                let (s, n) = ((*south).min(*north), (*south).max(*north));
                let full_longitude = west.is_finite()
                    && east.is_finite()
                    && (*east - *west).abs() >= 360.0 - 1.0e-12;
                let (w, e) = (norm(*west), norm(*east));
                let lon = norm(lon);
                let in_lon = if full_longitude {
                    true
                } else if w <= e {
                    lon >= w && lon <= e
                } else {
                    lon >= w || lon <= e
                };
                lat >= s && lat <= n && in_lon
            }
            GridRegion::Circle {
                lon: clon,
                lat: clat,
                radius_km,
            } => {
                let (la1, la2) = (clat.to_radians(), lat.to_radians());
                let dlat = (lat - *clat).to_radians();
                let dlon = (norm(lon) - norm(*clon)).to_radians();
                let a =
                    (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
                2.0 * EARTH_RADIUS_KM * a.sqrt().asin() <= *radius_km
            }
            GridRegion::Close { points } => point_in_close_region(points, lon, lat),
            GridRegion::Any(regions) => regions.iter().any(|region| region.contains(lon, lat)),
        }
    }
}

fn normalize_lon_degrees(x: f64) -> f64 {
    ((x + 180.0).rem_euclid(360.0)) - 180.0
}

fn lonlat_to_unit(lon: f64, lat: f64) -> [f64; 3] {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(point: [f64; 3]) -> f64 {
    dot(point, point).sqrt()
}

fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    dot(a, b).clamp(-1.0, 1.0).acos()
}

fn point_on_minor_arc(a: [f64; 3], b: [f64; 3], point: [f64; 3]) -> bool {
    let edge_angle = angle(a, b);
    if edge_angle <= 1.0e-12 || (std::f64::consts::PI - edge_angle).abs() <= 1.0e-10 {
        return false;
    }
    let normal = cross(a, b);
    dot(normal, point).abs() <= 1.0e-10 * norm(normal).max(1.0)
        && angle(a, point) + angle(point, b) <= edge_angle + 1.0e-10
}

fn signed_spherical_area(points: &[LonLatPoint]) -> Option<f64> {
    if points.len() < 3
        || points.iter().any(|point| {
            !point.lon.is_finite() || !point.lat.is_finite() || !(-90.0..=90.0).contains(&point.lat)
        })
    {
        return None;
    }
    let apex = lonlat_to_unit(points[0].lon, points[0].lat);
    let mut total = 0.0;
    for step in 1..points.len() - 1 {
        let b = lonlat_to_unit(points[step].lon, points[step].lat);
        let c = lonlat_to_unit(points[step + 1].lon, points[step + 1].lat);
        total += 2.0 * dot(apex, cross(b, c)).atan2(1.0 + dot(apex, b) + dot(b, c) + dot(c, apex));
    }
    total.is_finite().then_some(total)
}

fn point_in_close_region(points: &[LonLatPoint], lon: f64, lat: f64) -> bool {
    if points.len() < 3 || !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
        return false;
    }
    let Some(area) = signed_spherical_area(points) else {
        return false;
    };
    if area.abs() <= 1.0e-14 {
        return false;
    }
    // A close mask historically had no orientation contract. Preserve that
    // property on the sphere by selecting the smaller side of the ring.
    let desired_sign = if area.abs() <= std::f64::consts::TAU {
        area.signum()
    } else {
        -area.signum()
    };
    let here = lonlat_to_unit(lon, lat);
    let tangent = |point: [f64; 3]| -> Option<(f64, f64)> {
        let projection = dot(here, point);
        let flat = [
            point[0] - here[0] * projection,
            point[1] - here[1] * projection,
            point[2] - here[2] * projection,
        ];
        let length = norm(flat);
        if length <= 1.0e-12 {
            return None;
        }
        let east = [-here[1], here[0], 0.0];
        let east_length = (east[0] * east[0] + east[1] * east[1]).sqrt();
        let east = if east_length > 1.0e-12 {
            [east[0] / east_length, east[1] / east_length, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let north = cross(here, east);
        Some((dot(flat, east) / length, dot(flat, north) / length))
    };

    let mut turned = 0.0;
    for i in 0..points.len() {
        let a = lonlat_to_unit(points[i].lon, points[i].lat);
        let b = lonlat_to_unit(
            points[(i + 1) % points.len()].lon,
            points[(i + 1) % points.len()].lat,
        );
        if dot(here, a) > 1.0 - 1.0e-12 || dot(here, b) > 1.0 - 1.0e-12 {
            return true;
        }
        if dot(here, a) < -1.0 + 1.0e-12 || dot(here, b) < -1.0 + 1.0e-12 {
            return false;
        }
        if point_on_minor_arc(a, b, here) {
            return true;
        }
        let (Some(a), Some(b)) = (tangent(a), tangent(b)) else {
            return false;
        };
        turned += (a.0 * b.1 - a.1 * b.0).atan2(a.0 * b.0 + a.1 * b.1);
    }
    if desired_sign > 0.0 {
        turned > std::f64::consts::PI
    } else {
        turned < -std::f64::consts::PI
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_contains_antimeridian_span() {
        let region = GridRegion::Bbox {
            west: 170.0,
            east: -170.0,
            north: 10.0,
            south: -10.0,
        };
        assert!(region.contains(175.0, 0.0));
        assert!(region.contains(-175.0, 0.0));
        assert!(!region.contains(0.0, 0.0));
    }

    #[test]
    fn bbox_contains_full_longitude_band() {
        let region = GridRegion::Bbox {
            west: -180.0,
            east: 180.0,
            north: 10.0,
            south: -10.0,
        };
        for lon in [-179.0, -90.0, 0.0, 90.0, 179.0] {
            assert!(region.contains(lon, 0.0));
        }
        assert!(!region.contains(0.0, 11.0));
    }

    #[test]
    fn close_region_does_not_wrap_normal_polygon_around_query_longitude() {
        let points = vec![
            LonLatPoint {
                lon: 100.0,
                lat: 10.0,
            },
            LonLatPoint {
                lon: 130.0,
                lat: 10.0,
            },
            LonLatPoint {
                lon: 130.0,
                lat: 40.0,
            },
            LonLatPoint {
                lon: 100.0,
                lat: 40.0,
            },
        ];
        assert!(point_in_close_region(&points, 115.0, 20.0));
        assert!(!point_in_close_region(&points, -70.0, 20.0));
    }

    #[test]
    fn close_region_uses_spherical_smaller_side_at_a_pole() {
        let points = vec![
            LonLatPoint {
                lon: -120.0,
                lat: 80.0,
            },
            LonLatPoint {
                lon: 0.0,
                lat: 80.0,
            },
            LonLatPoint {
                lon: 120.0,
                lat: 80.0,
            },
        ];
        assert!(point_in_close_region(&points, 45.0, 90.0));
        assert!(!point_in_close_region(&points, 45.0, -80.0));

        let reversed = points.iter().rev().copied().collect::<Vec<_>>();
        assert!(point_in_close_region(&reversed, -90.0, 90.0));
        assert!(!point_in_close_region(&reversed, -90.0, -80.0));
    }

    #[test]
    fn close_region_uses_great_circle_edges_across_the_dateline() {
        let points = vec![
            LonLatPoint {
                lon: 170.0,
                lat: -10.0,
            },
            LonLatPoint {
                lon: -170.0,
                lat: -10.0,
            },
            LonLatPoint {
                lon: -170.0,
                lat: 10.0,
            },
            LonLatPoint {
                lon: 170.0,
                lat: 10.0,
            },
        ];
        assert!(point_in_close_region(&points, 180.0, 0.0));
        assert!(point_in_close_region(&points, -179.0, 0.0));
        assert!(!point_in_close_region(&points, 0.0, 0.0));
    }
}
