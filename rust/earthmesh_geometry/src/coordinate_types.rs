use crate::EARTH_RADIUS_KM;

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
    pub fn contains(&self, lon: f64, lat: f64) -> bool {
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

    /// A longitude arc and latitude band holding every point `contains`
    /// accepts, or `None` when none smaller than the globe is known.
    pub fn lonlat_bounds(&self) -> Option<LonLatBounds> {
        match self {
            GridRegion::Bbox {
                west,
                east,
                north,
                south,
            } => {
                if !(west.is_finite() && east.is_finite() && north.is_finite() && south.is_finite())
                {
                    return None;
                }
                let (south, north) = (south.min(*north).max(-90.0), south.max(*north).min(90.0));
                if (*east - *west).abs() >= 360.0 - 1.0e-12 {
                    return Some(LonLatBounds::full_longitude(south, north));
                }
                let (w, e) = (normalize_lon_degrees(*west), normalize_lon_degrees(*east));
                let width = if w <= e { e - w } else { e - w + 360.0 };
                Some(LonLatBounds {
                    west: w,
                    width,
                    south,
                    north,
                })
            }
            GridRegion::Circle {
                lon,
                lat,
                radius_km,
            } => {
                if !(lon.is_finite() && lat.is_finite() && radius_km.is_finite()) {
                    return None;
                }
                Some(LonLatBounds::cap(
                    *lon,
                    *lat,
                    (radius_km.max(0.0) / EARTH_RADIUS_KM).to_degrees(),
                ))
            }
            GridRegion::Close { points } => {
                (points.len() >= 3).then_some(())?;
                close_region_side(points)?;
                let units: Vec<[f64; 3]> = points
                    .iter()
                    .map(|point| lonlat_to_unit(point.lon, point.lat))
                    .collect();
                let (centre, cos_radius) = bounding_cap(&units)?;
                Some(LonLatBounds::cap(
                    centre[1].atan2(centre[0]).to_degrees(),
                    centre[2].clamp(-1.0, 1.0).asin().to_degrees(),
                    cos_radius.clamp(-1.0, 1.0).acos().to_degrees(),
                ))
            }
            GridRegion::Any(regions) => {
                let bounds = regions
                    .iter()
                    .map(GridRegion::lonlat_bounds)
                    .collect::<Option<Vec<_>>>()?;
                LonLatBounds::union(&bounds)
            }
        }
    }

    /// The region ready for many `contains` queries.
    ///
    /// A close ring is re-read on every query -- its area, then a winding
    /// sum with trigonometry at every vertex -- which is what a single test
    /// costs, but a threshold support mask asks once per support cell of the
    /// globe: a 130,000-vertex basin outline at a 40 km support scale took
    /// hours on one thread. Prepared, the ring is converted once and a
    /// bounding cap turns away every point outside it with one dot product.
    pub fn prepared(&self) -> PreparedGridRegion<'_> {
        match self {
            GridRegion::Close { points } => {
                let side = (points.len() >= 3)
                    .then(|| close_region_side(points))
                    .flatten();
                let units: Vec<[f64; 3]> = points
                    .iter()
                    .map(|point| lonlat_to_unit(point.lon, point.lat))
                    .collect();
                let cap = side.and_then(|_| bounding_cap(&units));
                let plane = cap.map(|(centre, _)| PlanarRing::new(&units, centre));
                PreparedGridRegion::Close {
                    units,
                    side,
                    cap,
                    plane,
                }
            }
            GridRegion::Any(regions) => {
                PreparedGridRegion::Any(regions.iter().map(GridRegion::prepared).collect())
            }
            region => PreparedGridRegion::Plain(region),
        }
    }
}

/// A longitude arc from `west` eastward over `width` degrees (360 is every
/// longitude) and the latitudes `south..=north`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatBounds {
    pub west: f64,
    pub width: f64,
    pub south: f64,
    pub north: f64,
}

impl LonLatBounds {
    fn full_longitude(south: f64, north: f64) -> Self {
        Self {
            west: -180.0,
            width: 360.0,
            south,
            north,
        }
    }

    /// The bounds of the cap of angular radius `radius` degrees about
    /// (`lon`, `lat`), a hair wider than exact.
    fn cap(lon: f64, lat: f64, radius: f64) -> Self {
        const MARGIN: f64 = 1.0e-6;
        let radius = radius + MARGIN;
        let (south, north) = (lat - radius, lat + radius);
        if south <= -90.0 || north >= 90.0 || radius >= 90.0 {
            return Self::full_longitude(south.max(-90.0), north.min(90.0));
        }
        // Tangent meridians: the widest longitude of a cap clear of the poles.
        let half = (radius.to_radians().sin() / lat.to_radians().cos())
            .min(1.0)
            .asin()
            .to_degrees()
            + MARGIN;
        Self {
            west: normalize_lon_degrees(lon - half),
            width: 2.0 * half,
            south,
            north,
        }
    }

    /// The smallest bounds holding all of `parts`: their latitudes, and the
    /// circle of longitude less its widest gap.
    fn union(parts: &[Self]) -> Option<Self> {
        let south = parts.iter().map(|b| b.south).reduce(f64::min)?;
        let north = parts.iter().map(|b| b.north).reduce(f64::max)?;
        if parts.iter().any(|b| b.width >= 360.0) {
            return Some(Self::full_longitude(south, north));
        }
        let mut arcs: Vec<(f64, f64)> = parts
            .iter()
            .map(|b| {
                let west = normalize_lon_degrees(b.west);
                (west, west + b.width)
            })
            .collect();
        arcs.sort_by(|a, b| a.0.total_cmp(&b.0));
        // (gap length, longitude where the covering arc starts)
        let mut widest = (0.0_f64, arcs[0].0);
        let mut reach = arcs[0].1;
        for &(start, end) in &arcs[1..] {
            if start - reach > widest.0 {
                widest = (start - reach, start);
            }
            reach = reach.max(end);
        }
        let wrap = arcs[0].0 + 360.0 - reach;
        if wrap > widest.0 {
            widest = (wrap, arcs[0].0);
        }
        Some(if widest.0 <= 0.0 {
            Self::full_longitude(south, north)
        } else {
            Self {
                west: widest.1,
                width: 360.0 - widest.0,
                south,
                north,
            }
        })
    }
}

/// A [`GridRegion`] prepared for repeated queries; `contains` answers exactly
/// as the region's own does.
#[derive(Debug, Clone)]
pub enum PreparedGridRegion<'a> {
    Plain(&'a GridRegion),
    Close {
        units: Vec<[f64; 3]>,
        side: Option<f64>,
        /// Centre and cosine of the radius of a cap under 90 degrees holding
        /// every vertex, and so the whole smaller side of the ring.
        cap: Option<([f64; 3], f64)>,
        /// The ring in the gnomonic projection about the cap's centre.
        plane: Option<PlanarRing>,
    },
    Any(Vec<PreparedGridRegion<'a>>),
}

/// A ring inside a cap under 90 degrees, projected gnomonically about the
/// cap's centre -- where great circles are straight lines, so the planar ring
/// is the spherical one -- with its edges bucketed into bands of `y`.
#[derive(Debug, Clone)]
pub struct PlanarRing {
    centre: [f64; 3],
    east: [f64; 3],
    north: [f64; 3],
    xy: Vec<(f64, f64)>,
    y_min: f64,
    band_height: f64,
    edges_in: Vec<Vec<u32>>,
}

impl PlanarRing {
    fn new(units: &[[f64; 3]], centre: [f64; 3]) -> Self {
        let axis = if centre[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let east = cross(centre, axis);
        let length = norm(east);
        let east = [east[0] / length, east[1] / length, east[2] / length];
        // (east, north, centre) is right-handed: counter-clockwise seen from
        // outside the sphere stays counter-clockwise in the plane.
        let north = cross(centre, east);
        let mut ring = Self {
            centre,
            east,
            north,
            xy: Vec::with_capacity(units.len()),
            y_min: 0.0,
            band_height: 1.0,
            edges_in: Vec::new(),
        };
        ring.xy = units.iter().map(|&u| ring.project(u)).collect();
        let (y_min, y_max) = ring
            .xy
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, y)| {
                (lo.min(y), hi.max(y))
            });
        let bands = ((units.len() as f64).sqrt().ceil() as usize).max(1);
        ring.y_min = y_min;
        ring.band_height = ((y_max - y_min) / bands as f64).max(f64::MIN_POSITIVE);
        ring.edges_in = vec![Vec::new(); bands];
        for k in 0..ring.xy.len() {
            let (a, b) = (ring.xy[k], ring.xy[(k + 1) % ring.xy.len()]);
            for band in ring.band_of(a.1.min(b.1))..=ring.band_of(a.1.max(b.1)) {
                ring.edges_in[band].push(k as u32);
            }
        }
        ring
    }

    fn project(&self, u: [f64; 3]) -> (f64, f64) {
        let z = dot(u, self.centre);
        (dot(u, self.east) / z, dot(u, self.north) / z)
    }

    fn band_of(&self, y: f64) -> usize {
        (((y - self.y_min) / self.band_height).floor().max(0.0) as usize)
            .min(self.edges_in.len() - 1)
    }

    /// The winding number of the ring about `here`, or `None` when `here` is
    /// too near an edge for the planar count to be trusted.
    fn winding(&self, here: [f64; 3]) -> Option<i64> {
        let (x, y) = self.project(here);
        // The gnomonic scale grows as 1 + r^2 away from the centre; this is
        // far above the spherical test's own boundary tolerances there.
        let near = 1.0e-5 * (1.0 + x * x + y * y);
        let band = self.band_of(y);
        let mut winding = 0i64;
        for b in band.saturating_sub(1)..=(band + 1).min(self.edges_in.len() - 1) {
            for &k in &self.edges_in[b] {
                let k = k as usize;
                let (p, q) = (self.xy[k], self.xy[(k + 1) % self.xy.len()]);
                if segment_distance((x, y), p, q) <= near {
                    return None;
                }
                if b == band && (p.1 > y) != (q.1 > y) {
                    let t = (y - p.1) / (q.1 - p.1);
                    if p.0 + t * (q.0 - p.0) > x {
                        winding += if q.1 > p.1 { 1 } else { -1 };
                    }
                }
            }
        }
        Some(winding)
    }
}

fn segment_distance(point: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point.0 - a.0 - t * dx).hypot(point.1 - a.1 - t * dy)
}

impl PreparedGridRegion<'_> {
    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        match self {
            PreparedGridRegion::Plain(region) => region.contains(lon, lat),
            PreparedGridRegion::Close {
                units,
                side,
                cap,
                plane,
            } => {
                if units.len() < 3
                    || !lon.is_finite()
                    || !lat.is_finite()
                    || !(-90.0..=90.0).contains(&lat)
                {
                    return false;
                }
                let Some(side) = side else {
                    return false;
                };
                let here = lonlat_to_unit(lon, lat);
                if cap.is_some_and(|(centre, cos_radius)| dot(here, centre) < cos_radius) {
                    return false;
                }
                // Inside the cap, count crossings in the plane; the spherical
                // test's `turned` is 2 pi times this winding number. Next to
                // an edge, where the two could round apart, ask the sphere.
                match plane.as_ref().and_then(|plane| plane.winding(here)) {
                    Some(winding) if *side > 0.0 => winding >= 1,
                    Some(winding) => winding <= -1,
                    None => winding_contains(units, *side, here),
                }
            }
            PreparedGridRegion::Any(regions) => {
                regions.iter().any(|region| region.contains(lon, lat))
            }
        }
    }
}

/// The smallest-radius cap about the vertices' mean direction that holds them
/// all, if it is under 90 degrees. A cap that small is convex, so it holds
/// every edge too, and the side of the ring inside it is the smaller one --
/// the side a close region is -- since the other holds the whole complement.
fn bounding_cap(units: &[[f64; 3]]) -> Option<([f64; 3], f64)> {
    let sum = units.iter().fold([0.0; 3], |acc, u| {
        [acc[0] + u[0], acc[1] + u[1], acc[2] + u[2]]
    });
    let length = norm(sum);
    if length <= 1.0e-12 {
        return None;
    }
    let centre = [sum[0] / length, sum[1] / length, sum[2] / length];
    let radius = units
        .iter()
        .map(|&u| angle(centre, u))
        .fold(0.0_f64, f64::max);
    // A margin well above the winding test's own tolerances.
    let radius = radius + 1.0e-5;
    (radius < 89.0_f64.to_radians()).then(|| (centre, radius.cos()))
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
    let Some(desired_sign) = close_region_side(points) else {
        return false;
    };
    let units: Vec<[f64; 3]> = points
        .iter()
        .map(|point| lonlat_to_unit(point.lon, point.lat))
        .collect();
    winding_contains(&units, desired_sign, lonlat_to_unit(lon, lat))
}

/// Which way round the ring the region lies, or `None` for a ring with no
/// area. A close mask historically had no orientation contract; on the sphere
/// that is kept by taking the smaller side of the ring.
fn close_region_side(points: &[LonLatPoint]) -> Option<f64> {
    let area = signed_spherical_area(points)?;
    if area.abs() <= 1.0e-14 {
        return None;
    }
    Some(if area.abs() <= std::f64::consts::TAU {
        area.signum()
    } else {
        -area.signum()
    })
}

/// The winding test of [`point_in_close_region`] on a ring already turned
/// into unit vectors.
fn winding_contains(units: &[[f64; 3]], desired_sign: f64, here: [f64; 3]) -> bool {
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
    for i in 0..units.len() {
        let a = units[i];
        let b = units[(i + 1) % units.len()];
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

pub fn lon_values(points: &[LonLatPoint]) -> Vec<f64> {
    points.iter().map(|point| point.lon).collect()
}

pub fn lat_values(points: &[LonLatPoint]) -> Vec<f64> {
    points.iter().map(|point| point.lat).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(lon: f64, lat: f64, radius_deg: f64, vertices: usize) -> Vec<LonLatPoint> {
        (0..vertices)
            .map(|k| {
                let t = k as f64 / vertices as f64 * std::f64::consts::TAU;
                // Uneven radii: a concave ring, as a basin outline is.
                let r = radius_deg * (0.55 + 0.45 * (5.0 * t).sin().abs());
                LonLatPoint {
                    lon: lon + r * t.cos() / lat.to_radians().cos(),
                    lat: lat + r * t.sin(),
                }
            })
            .collect()
    }

    #[test]
    fn lonlat_bounds_hold_every_point_the_region_contains() {
        let inside_bounds = |b: &LonLatBounds, lon: f64, lat: f64| {
            lat >= b.south
                && lat <= b.north
                && (b.width >= 360.0 || (lon - b.west).rem_euclid(360.0) <= b.width)
        };
        let mut regions = vec![
            GridRegion::Close {
                points: star(100.0, 40.0, 4.0, 400),
            },
            GridRegion::Close {
                points: star(179.0, -20.0, 6.0, 90),
            },
            GridRegion::Close {
                points: star(0.0, 80.0, 7.0, 60),
            },
            GridRegion::Circle {
                lon: 179.5,
                lat: 60.0,
                radius_km: 900.0,
            },
            GridRegion::Circle {
                lon: 10.0,
                lat: -85.0,
                radius_km: 700.0,
            },
            GridRegion::Bbox {
                west: 170.0,
                east: -170.0,
                north: 10.0,
                south: -5.0,
            },
        ];
        regions.push(GridRegion::Any(vec![
            regions[0].clone(),
            regions[3].clone(),
            regions[5].clone(),
        ]));
        for region in &regions {
            let bounds = region.lonlat_bounds().expect("bounded region");
            assert!(bounds.width < 360.0 || bounds.south <= -80.0 || bounds.north >= 80.0);
            let prepared = region.prepared();
            let mut inside = 0;
            for i in 0..1440 {
                for j in 0..721 {
                    let (lon, lat) = (-180.0 + i as f64 * 0.25 + 0.01, -90.0 + j as f64 * 0.25);
                    if prepared.contains(lon, lat) {
                        inside += 1;
                        assert!(inside_bounds(&bounds, lon, lat), "{region:?} {lon} {lat}");
                    }
                }
            }
            assert!(inside > 0);
        }
        // The union keeps the dateline arc, not the long way round.
        let any = regions.last().unwrap().lonlat_bounds().unwrap();
        assert!(any.width < 120.0, "{any:?}");
        let degenerate = GridRegion::Close {
            points: star(30.0, 0.0, 1.0, 2),
        };
        assert_eq!(degenerate.lonlat_bounds(), None);
    }

    #[test]
    fn a_prepared_region_answers_exactly_as_the_region_does() {
        // The prepared form is only a faster way to ask: every answer on a
        // global grid, and on the ring's own vertices, must be the same.
        let mut rings = vec![
            star(100.0, 40.0, 4.0, 400),
            star(179.0, -20.0, 6.0, 90), // across the antimeridian
            star(0.0, 80.0, 7.0, 60),    // near the pole
        ];
        // More than a hemisphere: no cap, the winding test alone.
        let mut wide = star(30.0, 0.0, 80.0, 50);
        wide.reverse();
        rings.push(wide);
        let mut regions: Vec<GridRegion> = rings
            .iter()
            .map(|points| GridRegion::Close {
                points: points.clone(),
            })
            .collect();
        regions.push(GridRegion::Any(vec![
            regions[0].clone(),
            GridRegion::Circle {
                lon: -60.0,
                lat: 10.0,
                radius_km: 800.0,
            },
        ]));
        for region in &regions {
            let prepared = region.prepared();
            let mut inside = 0;
            for i in 0..360 {
                for j in 0..181 {
                    let (lon, lat) = (-180.0 + i as f64 + 0.37, -90.0 + j as f64);
                    let expected = region.contains(lon, lat);
                    assert_eq!(prepared.contains(lon, lat), expected, "{lon} {lat}");
                    inside += usize::from(expected);
                }
            }
            assert!(inside > 0);
            // Densely around the first ring, where the planar count decides.
            if std::ptr::eq(region, &regions[0]) {
                for i in 0..240 {
                    for j in 0..180 {
                        let (lon, lat) = (91.003 + i as f64 * 0.075, 35.501 + j as f64 * 0.05);
                        assert_eq!(
                            prepared.contains(lon, lat),
                            region.contains(lon, lat),
                            "{lon} {lat}"
                        );
                    }
                }
            }
            if let GridRegion::Close { points } = region {
                for point in points {
                    assert_eq!(
                        prepared.contains(point.lon, point.lat),
                        region.contains(point.lon, point.lat)
                    );
                }
            }
        }
    }

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
