use earthmesh_geometry::Point;

const MAX_GEODESIC_STEP_RAD: f64 = std::f64::consts::PI / 1800.0; // 0.1 degree

type Vec3 = [f64; 3];

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: Vec3) -> Option<Vec3> {
    let norm = dot(v, v).sqrt();
    (norm > 64.0 * f64::EPSILON).then(|| [v[0] / norm, v[1] / norm, v[2] / norm])
}

fn lonlat_to_unit(point: Point) -> Vec3 {
    let lon = point.x.to_radians();
    let lat = point.y.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// Conservative spherical-cap broad phase. The extra longest-edge margin keeps
/// every minor great-circle edge inside the cap, so disjoint caps can be skipped
/// without losing a real overlay.
#[derive(Clone, Copy)]
pub struct SphericalCap {
    center: Vec3,
    radius: f64,
}

impl SphericalCap {
    pub fn for_rings(rings: &[Vec<Point>]) -> Option<Self> {
        let points = rings
            .iter()
            .flat_map(|ring| ring.iter().copied())
            .map(lonlat_to_unit)
            .collect::<Vec<_>>();
        let center = normalize(points.iter().fold([0.0; 3], |sum, point| {
            [sum[0] + point[0], sum[1] + point[1], sum[2] + point[2]]
        }))?;
        let vertex_radius = points
            .iter()
            .map(|point| dot(center, *point).clamp(-1.0, 1.0).acos())
            .fold(0.0, f64::max);
        let longest_edge = rings
            .iter()
            .filter(|ring| ring.len() >= 2)
            .flat_map(|ring| {
                (0..ring.len()).map(|index| {
                    let start = lonlat_to_unit(ring[index]);
                    let end = lonlat_to_unit(ring[(index + 1) % ring.len()]);
                    dot(start, end).clamp(-1.0, 1.0).acos()
                })
            })
            .fold(0.0, f64::max);
        Some(Self {
            center,
            radius: (vertex_radius + longest_edge).min(std::f64::consts::PI),
        })
    }

    pub fn overlaps(self, other: Self) -> bool {
        dot(self.center, other.center).clamp(-1.0, 1.0).acos()
            <= self.radius + other.radius + 1.0e-12
    }
}

/// Cell-local Lambert azimuthal equal-area projection on a unit sphere.
///
/// The projection is continuous across the antimeridian and its planar areas are
/// steradians. Rings are densified along their minor great-circle arcs before
/// projection so straight planar clips approximate the projected curved edges.
pub struct LocalEqualArea {
    center: Vec3,
    east: Vec3,
    north: Vec3,
}

impl LocalEqualArea {
    pub fn for_rings(rings: &[Vec<Point>]) -> Option<Self> {
        let sum = rings
            .iter()
            .flat_map(|ring| {
                let end = if ring.len() > 1
                    && dot(
                        lonlat_to_unit(ring[0]),
                        lonlat_to_unit(*ring.last().expect("non-empty ring")),
                    ) >= 1.0 - 1.0e-14
                {
                    ring.len() - 1
                } else {
                    ring.len()
                };
                ring[..end].iter().copied()
            })
            .map(lonlat_to_unit)
            .fold([0.0; 3], |sum, point| {
                [sum[0] + point[0], sum[1] + point[1], sum[2] + point[2]]
            });
        let center = normalize(sum)?;
        let axis_seed = if center[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let east = normalize(cross(axis_seed, center))?;
        let north = cross(center, east);
        Some(Self {
            center,
            east,
            north,
        })
    }

    fn project_unit(&self, point: Vec3) -> Option<Point> {
        let denominator = 1.0 + dot(self.center, point);
        if denominator <= 1.0e-12 {
            return None;
        }
        let scale = (2.0 / denominator).sqrt();
        Some(Point::new(
            scale * dot(point, self.east),
            scale * dot(point, self.north),
        ))
    }

    pub fn project_ring(&self, ring: &[Point]) -> Option<Vec<Point>> {
        let mut vertices = ring.to_vec();
        if vertices.len() > 1 {
            let first = lonlat_to_unit(vertices[0]);
            let last = lonlat_to_unit(*vertices.last()?);
            if dot(first, last) >= 1.0 - 1.0e-14 {
                vertices.pop();
            }
        }
        if vertices.len() < 3 {
            return None;
        }

        let mut projected = Vec::new();
        for index in 0..vertices.len() {
            let start = lonlat_to_unit(vertices[index]);
            let end = lonlat_to_unit(vertices[(index + 1) % vertices.len()]);
            let angle = dot(start, end).clamp(-1.0, 1.0).acos();
            if !angle.is_finite() || angle >= std::f64::consts::PI - 1.0e-12 {
                return None;
            }
            let segments = (angle / MAX_GEODESIC_STEP_RAD).ceil().max(1.0) as usize;
            let sin_angle = angle.sin();
            for segment in 0..segments {
                let t = segment as f64 / segments as f64;
                let point = if angle <= 1.0e-12 {
                    start
                } else {
                    let left = ((1.0 - t) * angle).sin() / sin_angle;
                    let right = (t * angle).sin() / sin_angle;
                    normalize([
                        left * start[0] + right * end[0],
                        left * start[1] + right * end[1],
                        left * start[2] + right * end[2],
                    ])?
                };
                projected.push(self.project_unit(point)?);
            }
        }
        Some(projected)
    }
}

pub fn ring_bounds(ring: &[Point]) -> (f64, f64, f64, f64) {
    ring.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point.x),
                max_x.max(point.x),
                min_y.min(point.y),
                max_y.max(point.y),
            )
        },
    )
}

pub fn bounds_overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 <= b.1 && a.1 >= b.0 && a.2 <= b.3 && a.3 >= b.2
}

pub fn is_convex(ring: &[Point]) -> bool {
    let mut sign = 0.0_f64;
    for index in 0..ring.len() {
        let a = ring[index];
        let b = ring[(index + 1) % ring.len()];
        let c = ring[(index + 2) % ring.len()];
        let turn = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if turn.abs() <= 1.0e-14 {
            continue;
        }
        if sign != 0.0 && sign.is_sign_positive() != turn.is_sign_positive() {
            return false;
        }
        sign = turn;
    }
    sign != 0.0
}

/// Equal-area overlap fraction of `source` covered by `target` after projecting
/// both lon/lat rings into a local equal-area plane centered on `source`.
pub fn local_equal_area_overlap_fraction(source: &[Point], target: &[Point]) -> Option<f64> {
    let projection = LocalEqualArea::for_rings(&[source.to_vec(), target.to_vec()])?;
    let source_flat = projection.project_ring(source)?;
    let target_flat = projection.project_ring(target)?;
    let source_area = earthmesh_geometry::polygon_area(&source_flat).abs();
    if source_area <= 0.0 || !source_area.is_finite() {
        return None;
    }
    Some((earthmesh_geometry::intersection_area(&source_flat, &target_flat) / source_area).max(0.0))
}

pub fn local_equal_area_overlap_fraction_lonlat(
    source: &[(f64, f64)],
    target: &[(f64, f64)],
) -> Option<f64> {
    let to_points = |ring: &[(f64, f64)]| {
        ring.iter()
            .map(|&(lon, lat)| Point::new(lon, lat))
            .collect::<Vec<_>>()
    };
    local_equal_area_overlap_fraction(&to_points(source), &to_points(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuple_ring(west: f64, east: f64, south: f64, north: f64) -> Vec<(f64, f64)> {
        vec![(west, south), (east, south), (east, north), (west, north)]
    }

    fn ring(west: f64, east: f64, south: f64, north: f64) -> Vec<Point> {
        vec![
            Point { x: west, y: south },
            Point { x: east, y: south },
            Point { x: east, y: north },
            Point { x: west, y: north },
        ]
    }

    #[test]
    fn local_equal_area_overlap_is_one_for_self() {
        let cell = ring(0.0, 10.0, 80.0, 89.0);
        let fraction = local_equal_area_overlap_fraction(&cell, &cell).expect("overlap");
        assert!((fraction - 1.0).abs() < 1.0e-9, "{fraction}");

        let cell = tuple_ring(0.0, 10.0, 80.0, 89.0);
        let fraction = local_equal_area_overlap_fraction_lonlat(&cell, &cell).expect("overlap");
        assert!((fraction - 1.0).abs() < 1.0e-9, "{fraction}");
    }

    #[test]
    fn local_equal_area_overlap_is_zero_for_disjoint_rings() {
        let left = ring(0.0, 1.0, 0.0, 1.0);
        let right = ring(10.0, 11.0, 10.0, 11.0);
        let fraction = local_equal_area_overlap_fraction(&left, &right).expect("overlap");
        assert!(fraction.abs() < 1.0e-12, "{fraction}");
    }
}
