/// Additive geometry safety/validation layer (flags, polygon/overlay/fraction checks).
pub mod safety;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Earth radius in kilometers used by `MOD_Area_judge:haversine`, derived from
/// the workspace-wide canonical radius.
pub const EARTH_RADIUS_KM: f64 = earthmesh_core::EARTH_RADIUS_METERS / 1000.0;

/// Which side of an oriented spherical ring should be measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SphericalAreaBranch {
    /// The smaller of the two regions bounded by the ring, in `[0, 2π]`.
    Minor,
    /// The complement of [`Self::Minor`], in `[2π, 4π]` for a non-degenerate ring.
    MajorComplement,
    /// The region on the left of the directed ring, in `[0, 4π)`.
    Oriented,
    /// Signed minor area, in `[-2π, 2π]`; useful for winding checks.
    SignedMinor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SphericalWinding {
    CounterClockwise,
    Clockwise,
    Indeterminate,
}

/// Both complement candidates for a validated directed spherical ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphericalArea {
    pub signed_minor_sr: f64,
    pub minor_sr: f64,
    pub major_complement_sr: f64,
    pub oriented_left_sr: f64,
    pub winding: SphericalWinding,
}

/// Structured validation failures for spherical lon/lat polygon area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SphericalPolygonError {
    TooFewVertices {
        found: usize,
    },
    NonFiniteCoordinate {
        vertex: usize,
    },
    LatitudeOutOfRange {
        vertex: usize,
    },
    DuplicateConsecutiveVertex {
        start_vertex: usize,
    },
    AntipodalEdge {
        start_vertex: usize,
    },
    SelfIntersection {
        first_edge: usize,
        second_edge: usize,
    },
    DegenerateArea,
    AmbiguousTriangulation {
        vertex: usize,
    },
}

impl std::fmt::Display for SphericalPolygonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewVertices { found } => {
                write!(f, "spherical polygon needs at least three vertices; found {found}")
            }
            Self::NonFiniteCoordinate { vertex } => {
                write!(f, "spherical polygon vertex {vertex} is not finite")
            }
            Self::LatitudeOutOfRange { vertex } => {
                write!(f, "spherical polygon vertex {vertex} has latitude outside [-90, 90]")
            }
            Self::DuplicateConsecutiveVertex { start_vertex } => write!(
                f,
                "spherical polygon edge {start_vertex} has duplicate endpoints"
            ),
            Self::AntipodalEdge { start_vertex } => write!(
                f,
                "spherical polygon edge {start_vertex} has antipodal endpoints and no unique geodesic"
            ),
            Self::SelfIntersection {
                first_edge,
                second_edge,
            } => write!(
                f,
                "spherical polygon edges {first_edge} and {second_edge} intersect"
            ),
            Self::DegenerateArea => write!(f, "spherical polygon has no resolved surface area"),
            Self::AmbiguousTriangulation { vertex } => write!(
                f,
                "spherical polygon fan triangle ending at vertex {vertex} is ambiguous"
            ),
        }
    }
}

impl std::error::Error for SphericalPolygonError {}

/// Normalize a longitude delta in radians into [-π, π].
#[inline]
pub fn normalize_delta_lon_radians(delta: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let normalized = (delta + pi).rem_euclid(2.0 * pi) - pi;
    if normalized == -pi && delta > 0.0 {
        pi
    } else {
        normalized
    }
}

fn raw_spherical_polygon_excess(ring: &[Point]) -> Result<f64, SphericalPolygonError> {
    for (vertex, point) in ring.iter().enumerate() {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(SphericalPolygonError::NonFiniteCoordinate { vertex });
        }
        if !(-90.0..=90.0).contains(&point.y) {
            return Err(SphericalPolygonError::LatitudeOutOfRange { vertex });
        }
    }
    let to_unit = |point: Point| {
        let lon = point.x.to_radians();
        let lat = point.y.to_radians();
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let determinant = |a: [f64; 3], b: [f64; 3], c: [f64; 3]| {
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0])
    };
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let norm = |a: [f64; 3]| dot(a, a).sqrt();
    let angle = |a: [f64; 3], b: [f64; 3]| dot(a, b).clamp(-1.0, 1.0).acos();

    let mut vertices = ring.iter().copied().map(to_unit).collect::<Vec<_>>();
    if vertices.len() > 1 && dot(vertices[0], *vertices.last().unwrap()) >= 1.0 - 1.0e-14 {
        vertices.pop();
    }
    if vertices.len() < 3 {
        return Err(SphericalPolygonError::TooFewVertices {
            found: vertices.len(),
        });
    }
    for index in 0..vertices.len() {
        let edge_dot = dot(vertices[index], vertices[(index + 1) % vertices.len()]);
        if edge_dot >= 1.0 - 1.0e-14 {
            return Err(SphericalPolygonError::DuplicateConsecutiveVertex {
                start_vertex: index,
            });
        }
        if edge_dot <= -1.0 + 1.0e-14 {
            return Err(SphericalPolygonError::AntipodalEdge {
                start_vertex: index,
            });
        }
    }
    let on_minor_arc = |point: [f64; 3], start: [f64; 3], end: [f64; 3]| {
        let total = angle(start, end);
        (angle(start, point) + angle(point, end) - total).abs() <= 1.0e-10
    };
    for first in 0..vertices.len() {
        let first_next = (first + 1) % vertices.len();
        for second in (first + 1)..vertices.len() {
            let second_next = (second + 1) % vertices.len();
            if first_next == second || second_next == first {
                continue;
            }
            let normal1 = cross(vertices[first], vertices[first_next]);
            let normal2 = cross(vertices[second], vertices[second_next]);
            let intersections = cross(normal1, normal2);
            let intersection_norm = norm(intersections);
            let mut intersects = false;
            if intersection_norm > 64.0 * f64::EPSILON {
                let candidate = [
                    intersections[0] / intersection_norm,
                    intersections[1] / intersection_norm,
                    intersections[2] / intersection_norm,
                ];
                for point in [candidate, [-candidate[0], -candidate[1], -candidate[2]]] {
                    if on_minor_arc(point, vertices[first], vertices[first_next])
                        && on_minor_arc(point, vertices[second], vertices[second_next])
                    {
                        intersects = true;
                        break;
                    }
                }
            } else {
                intersects = [vertices[first], vertices[first_next]]
                    .into_iter()
                    .any(|point| on_minor_arc(point, vertices[second], vertices[second_next]))
                    || [vertices[second], vertices[second_next]]
                        .into_iter()
                        .any(|point| on_minor_arc(point, vertices[first], vertices[first_next]));
            }
            if intersects {
                return Err(SphericalPolygonError::SelfIntersection {
                    first_edge: first,
                    second_edge: second,
                });
            }
        }
    }

    let anchor = vertices[0];
    let mut raw = 0.0;
    for index in 1..vertices.len() - 1 {
        let b = vertices[index];
        let c = vertices[index + 1];
        let numerator = determinant(anchor, b, c);
        let denominator = 1.0 + dot(anchor, b) + dot(b, c) + dot(c, anchor);
        if numerator.abs() <= 64.0 * f64::EPSILON && denominator.abs() <= 64.0 * f64::EPSILON {
            return Err(SphericalPolygonError::AmbiguousTriangulation { vertex: index + 1 });
        }
        let excess = 2.0 * numerator.atan2(denominator);
        if !excess.is_finite() {
            return Err(SphericalPolygonError::AmbiguousTriangulation { vertex: index + 1 });
        }
        raw += excess;
    }
    Ok(raw)
}

/// Validate a directed ring and return both spherical complement candidates.
pub fn try_spherical_polygon_area(ring: &[Point]) -> Result<SphericalArea, SphericalPolygonError> {
    let raw = raw_spherical_polygon_excess(ring)?;
    if raw.abs() <= 64.0 * f64::EPSILON {
        return Err(SphericalPolygonError::DegenerateArea);
    }
    let half_sphere = 2.0 * std::f64::consts::PI;
    let full_sphere = 2.0 * half_sphere;
    let normalized = (raw + half_sphere).rem_euclid(full_sphere) - half_sphere;
    let signed_minor = if normalized == -half_sphere && raw > 0.0 {
        half_sphere
    } else {
        normalized
    };
    let minor = signed_minor.abs();
    Ok(SphericalArea {
        signed_minor_sr: signed_minor,
        minor_sr: minor,
        major_complement_sr: full_sphere - minor,
        oriented_left_sr: raw.rem_euclid(full_sphere),
        winding: if signed_minor > 64.0 * f64::EPSILON {
            SphericalWinding::CounterClockwise
        } else if signed_minor < -64.0 * f64::EPSILON {
            SphericalWinding::Clockwise
        } else {
            SphericalWinding::Indeterminate
        },
    })
}

/// Validated spherical excess on the unit sphere, in steradians.
pub fn try_spherical_polygon_excess(
    ring: &[Point],
    branch: SphericalAreaBranch,
) -> Result<f64, SphericalPolygonError> {
    let area = try_spherical_polygon_area(ring)?;
    Ok(match branch {
        SphericalAreaBranch::Minor => area.minor_sr,
        SphericalAreaBranch::MajorComplement => area.major_complement_sr,
        SphericalAreaBranch::Oriented => area.oriented_left_sr,
        SphericalAreaBranch::SignedMinor => area.signed_minor_sr,
    })
}

/// Validated spherical lon/lat polygon area in km².
pub fn try_spherical_polygon_area_km2(
    ring: &[Point],
    branch: SphericalAreaBranch,
) -> Result<f64, SphericalPolygonError> {
    try_spherical_polygon_excess(ring, branch)
        .map(|excess| excess * EARTH_RADIUS_KM * EARTH_RADIUS_KM)
}

/// Signed minor spherical excess on the unit sphere, in steradians.
///
/// This compatibility wrapper preserves the historical `0` for short rings and
/// `NaN` for invalid geometry. New callers should use
/// [`try_spherical_polygon_excess`] to retain structured errors.
pub fn signed_spherical_polygon_excess(ring: &[Point]) -> f64 {
    if ring.len() < 3 {
        0.0
    } else {
        match try_spherical_polygon_excess(ring, SphericalAreaBranch::SignedMinor) {
            Ok(area) => area,
            Err(SphericalPolygonError::DegenerateArea) => 0.0,
            Err(_) => f64::NAN,
        }
    }
}

/// Signed spherical lon/lat polygon area in km², antimeridian-safe.
pub fn signed_spherical_polygon_area_km2(ring: &[Point]) -> f64 {
    signed_spherical_polygon_excess(ring) * EARTH_RADIUS_KM * EARTH_RADIUS_KM
}

/// Unsigned minor-branch spherical lon/lat polygon area in km².
pub fn spherical_polygon_area_km2(ring: &[Point]) -> f64 {
    signed_spherical_polygon_area_km2(ring).abs()
}

/// Port of `MOD_Area_judge:cross_product`.
#[inline]
pub fn cross_product_2d(p1: Point, p2: Point, p3: Point) -> f64 {
    (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
}

/// Port of `MOD_Area_judge:haversine`.
///
/// Inputs use the same convention as the Canonical `point_i(2)` arrays:
/// `x = longitude degrees`, `y = latitude degrees`. The return value is km.
pub fn haversine_km(point_i: Point, point_c: Point) -> f64 {
    let px1 = point_i.x.to_radians();
    let py1 = point_i.y.to_radians();
    let px2 = point_c.x.to_radians();
    let py2 = point_c.y.to_radians();

    let v = ((py1 / 2.0 - py2 / 2.0).sin().powi(2)
        + py2.cos() * py1.cos() * (px1 / 2.0 - px2 / 2.0).sin().powi(2))
    .clamp(0.0, 1.0);
    EARTH_RADIUS_KM * 2.0 * v.sqrt().atan2((1.0 - v).sqrt())
}

/// Port of `MOD_Area_judge:is_point_in_circle`.
#[inline]
pub fn is_point_in_circle_km(point: Point, center: Point, center_radius_km: f64) -> bool {
    haversine_km(point, center) <= center_radius_km
}

/// Port of `MOD_Area_judge:is_point_in_convex_polygon`.
///
/// Boundary points are considered inside, matching the Canonical behavior where
/// zero cross products do not flip the sign test.
pub fn is_point_in_convex_polygon(polygon: &[Point], point: Point) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut prev_cross = 0.0;
    for i in 0..polygon.len() {
        let p1 = polygon[i];
        let p2 = polygon[(i + 1) % polygon.len()];
        let cross = cross_product_2d(p1, p2, point);
        if cross != 0.0 {
            if prev_cross == 0.0 {
                prev_cross = cross;
            } else if (prev_cross > 0.0) != (cross > 0.0) {
                return false;
            }
        }
    }
    true
}

pub fn polygon_area(polygon: &[Point]) -> f64 {
    if polygon.len() < 3 {
        return 0.0;
    }
    let mut total = 0.0;
    for (index, point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        total += point.x * next.y - next.x * point.y;
    }
    total.abs() * 0.5
}

pub fn clip_convex_polygon(subject: &[Point], clip: &[Point]) -> Vec<Point> {
    if subject.len() < 3
        || clip.len() < 3
        || polygon_area(subject) == 0.0
        || polygon_area(clip) == 0.0
    {
        return Vec::new();
    }
    let mut output = subject.to_vec();
    let clip_ccw = signed_area(clip) >= 0.0;
    for (index, edge_start) in clip.iter().enumerate() {
        let edge_end = clip[(index + 1) % clip.len()];
        let input = output;
        output = Vec::new();
        if input.is_empty() {
            break;
        }
        let mut previous = *input.last().expect("non-empty input polygon");
        for current in input {
            let current_inside = inside(current, *edge_start, edge_end, clip_ccw);
            let previous_inside = inside(previous, *edge_start, edge_end, clip_ccw);
            if current_inside {
                if !previous_inside {
                    output.push(line_intersection(previous, current, *edge_start, edge_end));
                }
                output.push(current);
            } else if previous_inside {
                output.push(line_intersection(previous, current, *edge_start, edge_end));
            }
            previous = current;
        }
    }
    output
}

/// Intersection of two simple polygons as a set of disjoint convex pieces (each a
/// triangle-pair clip). Exact for arbitrary (non-convex) simple polygons via
/// ear-clip triangulation. Empty if they do not overlap. Falls back to a single
/// convex clip if a polygon cannot be triangulated.
pub fn polygon_intersection_pieces(a: &[Point], b: &[Point]) -> Vec<Vec<Point>> {
    let (Some(a_tri), Some(b_tri)) = (triangulate_simple_polygon(a), triangulate_simple_polygon(b))
    else {
        let c = clip_convex_polygon(a, b);
        return if c.len() >= 3 && polygon_area(&c) > 0.0 {
            vec![c]
        } else {
            Vec::new()
        };
    };
    let mut pieces = Vec::new();
    for at in &a_tri {
        for bt in &b_tri {
            let c = clip_convex_polygon(at, bt);
            if c.len() >= 3 && polygon_area(&c) > 0.0 {
                pieces.push(c);
            }
        }
    }
    pieces
}

pub fn intersection_area(a: &[Point], b: &[Point]) -> f64 {
    polygon_intersection_pieces(a, b)
        .iter()
        .map(|p| polygon_area(p))
        .sum()
}

/// x-coordinate of the intersection point of two segments, if they properly cross.
fn segment_intersection_x(p1: Point, p2: Point, p3: Point, p4: Point) -> Option<f64> {
    let d1 = (p2.x - p1.x, p2.y - p1.y);
    let d2 = (p4.x - p3.x, p4.y - p3.y);
    let denom = d1.0 * d2.1 - d1.1 * d2.0;
    if denom.abs() < 1e-15 {
        return None; // parallel / collinear
    }
    let t = ((p3.x - p1.x) * d2.1 - (p3.y - p1.y) * d2.0) / denom;
    let u = ((p3.x - p1.x) * d1.1 - (p3.y - p1.y) * d1.0) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(p1.x + t * d1.0)
    } else {
        None
    }
}

/// Exact area of the union of a set of simple polygons (overlaps counted once),
/// via vertical-slab decomposition + even-odd coverage. Slab boundaries are every
/// vertex x and every edge-edge intersection x, so within a slab the covered length
/// is linear in x and the midpoint rule is exact. Handles arbitrary (non-convex)
/// simple polygons and overlaps; no external GIS dependency.
pub fn polygon_union_area(polygons: &[Vec<Point>]) -> f64 {
    let polys: Vec<&Vec<Point>> = polygons.iter().filter(|p| p.len() >= 3).collect();
    if polys.is_empty() {
        return 0.0;
    }
    let mut edges: Vec<(Point, Point)> = Vec::new();
    let mut xs: Vec<f64> = Vec::new();
    for p in &polys {
        for i in 0..p.len() {
            let a = p[i];
            let b = p[(i + 1) % p.len()];
            edges.push((a, b));
            xs.push(a.x);
        }
    }
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            if let Some(x) = segment_intersection_x(edges[i].0, edges[i].1, edges[j].0, edges[j].1)
            {
                xs.push(x);
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

    let mut area = 0.0;
    for w in xs.windows(2) {
        let (xl, xr) = (w[0], w[1]);
        let width = xr - xl;
        if width <= 1e-15 {
            continue;
        }
        let xm = 0.5 * (xl + xr);
        let mut intervals: Vec<(f64, f64)> = Vec::new();
        for p in &polys {
            let mut ys: Vec<f64> = Vec::new();
            for i in 0..p.len() {
                let a = p[i];
                let b = p[(i + 1) % p.len()];
                if (a.x < xm && xm < b.x) || (b.x < xm && xm < a.x) {
                    let t = (xm - a.x) / (b.x - a.x);
                    ys.push(a.y + t * (b.y - a.y));
                }
            }
            ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mut k = 0;
            while k + 1 < ys.len() {
                intervals.push((ys[k], ys[k + 1]));
                k += 2;
            }
        }
        intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut covered = 0.0;
        let mut cur: Option<(f64, f64)> = None;
        for (s, e) in intervals {
            match cur {
                None => cur = Some((s, e)),
                Some((cs, ce)) => {
                    if s <= ce {
                        cur = Some((cs, ce.max(e)));
                    } else {
                        covered += ce - cs;
                        cur = Some((s, e));
                    }
                }
            }
        }
        if let Some((cs, ce)) = cur {
            covered += ce - cs;
        }
        area += covered * width;
    }
    area
}

/// Dissolve a set of axis-aligned boxes `(x0, y0, x1, y1)` (e.g. equal grid cells)
/// into the boundary rings of their union, via directed-edge cancellation + face
/// tracing. Shared full edges between adjacent boxes cancel (opposite directions);
/// remaining edges are traced into closed rings (outer rings CCW / positive signed
/// area, holes CW / negative). Exact + robust for a grid-aligned set (combinatorial,
/// no floating-point area test). Rings touching only at a corner are kept separate.
// The ring-tracing loops inspect `out` (out.get/find) and then mutate it (out.get_mut)
// in the same body, so they cannot be rewritten as `while let` without a borrow clash.
#[allow(clippy::while_let_loop)]
pub fn dissolve_axis_aligned_boxes(boxes: &[(f64, f64, f64, f64)]) -> Vec<Vec<Point>> {
    use std::collections::HashMap;
    type NodeKey = (i64, i64);
    type EdgeCount = HashMap<(NodeKey, NodeKey), i32>;
    let key = |x: f64, y: f64| -> NodeKey { ((x * 1e9).round() as i64, (y * 1e9).round() as i64) };
    let mut edge_count: EdgeCount = HashMap::new();
    let mut pts: HashMap<NodeKey, Point> = HashMap::new();
    let mut add_edge = |a: Point, b: Point, ec: &mut EdgeCount| {
        let (ka, kb) = (key(a.x, a.y), key(b.x, b.y));
        pts.insert(ka, a);
        pts.insert(kb, b);
        if let Some(c) = ec.get_mut(&(kb, ka)) {
            *c -= 1;
            if *c == 0 {
                ec.remove(&(kb, ka));
            }
        } else {
            *ec.entry((ka, kb)).or_insert(0) += 1;
        }
    };
    for &(x0, y0, x1, y1) in boxes {
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        add_edge(Point::new(x0, y0), Point::new(x1, y0), &mut edge_count); // bottom →
        add_edge(Point::new(x1, y0), Point::new(x1, y1), &mut edge_count); // right ↑
        add_edge(Point::new(x1, y1), Point::new(x0, y1), &mut edge_count); // top ←
        add_edge(Point::new(x0, y1), Point::new(x0, y0), &mut edge_count); // left ↓
    }

    // outgoing edges per node
    let mut out: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for ((a, b), c) in &edge_count {
        for _ in 0..(*c).max(0) {
            out.entry(*a).or_default().push(*b);
        }
    }

    let ang = |from: (i64, i64), to: (i64, i64)| -> f64 {
        let a = pts[&from];
        let b = pts[&to];
        (b.y - a.y).atan2(b.x - a.x)
    };
    let mut rings: Vec<Vec<Point>> = Vec::new();
    loop {
        let Some(start) = out.iter().find(|(_, v)| !v.is_empty()).map(|(k, _)| *k) else {
            break;
        };
        let mut ring_keys: Vec<(i64, i64)> = Vec::new();
        let mut cur = start;
        let mut prev: Option<(i64, i64)> = None;
        loop {
            let Some(candidates) = out.get(&cur) else {
                break;
            };
            if candidates.is_empty() {
                break;
            }
            // pick the next edge: the first one clockwise from the reverse-incoming
            // direction (left-hand boundary trace). With no incoming edge, take the
            // smallest-angle outgoing edge for determinism.
            let chosen = match prev {
                None => candidates
                    .iter()
                    .copied()
                    .min_by(|x, y| {
                        ang(cur, *x)
                            .partial_cmp(&ang(cur, *y))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap(),
                Some(p) => {
                    let rev_in = ang(cur, p);
                    *candidates
                        .iter()
                        .min_by(|x, y| {
                            let dx = (rev_in - ang(cur, **x)).rem_euclid(std::f64::consts::TAU);
                            let dy = (rev_in - ang(cur, **y)).rem_euclid(std::f64::consts::TAU);
                            // smallest positive clockwise delta (treat ~0 as full turn)
                            let norm = |d: f64| if d < 1e-9 { std::f64::consts::TAU } else { d };
                            norm(dx)
                                .partial_cmp(&norm(dy))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap()
                }
            };
            let v = out.get_mut(&cur).unwrap();
            let pos = v.iter().position(|&n| n == chosen).unwrap();
            v.remove(pos);
            ring_keys.push(cur);
            prev = Some(cur);
            cur = chosen;
            if cur == start {
                break;
            }
        }
        if ring_keys.len() >= 3 {
            rings.push(ring_keys.iter().map(|k| pts[k]).collect());
        }
    }
    rings
}

/// Signed area of a ring (positive = CCW). Useful to split union rings into outer
/// rings vs holes.
pub fn signed_ring_area(ring: &[Point]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut total = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        total += a.x * b.y - b.x * a.y;
    }
    total * 0.5
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayMask {
    pub feature_id: String,
    pub mask_class: String,
    pub priority: u32,
    pub polygon: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayCellInput {
    pub cell_id: String,
    pub vertices: Vec<Point>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayCellResult {
    pub cell_id: String,
    pub winning_class: String,
    pub winning_priority: u32,
    pub class_fractions: Vec<(String, f64)>,
    pub source_feature_ids: Vec<String>,
    pub quality_flags: Vec<String>,
}

pub fn overlay_cell(cell_vertices: &[Point], masks: &[OverlayMask]) -> OverlayCellResult {
    use crate::safety::GeometryQualityFlag as Flag;

    // Reject non-finite cell geometry up front: a NaN area would otherwise slip
    // past the `<= 0.0` check below and silently produce bogus fractions.
    if cell_vertices
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return OverlayCellResult {
            cell_id: String::new(),
            winning_class: String::new(),
            winning_priority: 0,
            class_fractions: Vec::new(),
            source_feature_ids: Vec::new(),
            quality_flags: vec![Flag::NonFiniteCoordinate.as_str().to_string()],
        };
    }

    let cell_area = polygon_area(cell_vertices);
    if !cell_area.is_finite() || cell_area <= 0.0 {
        // `polygon_area` is unsigned, so a true negative cannot occur here; keep the
        // branch defensive in case the area model changes to a signed one.
        let flag = if cell_area.is_finite() && cell_area < 0.0 {
            Flag::NegativeArea
        } else {
            Flag::ZeroAreaCell
        };
        return OverlayCellResult {
            cell_id: String::new(),
            winning_class: String::new(),
            winning_priority: 0,
            class_fractions: Vec::new(),
            source_feature_ids: Vec::new(),
            quality_flags: vec![flag.as_str().to_string()],
        };
    }

    let mut class_fractions = Vec::<(String, f64)>::new();
    let mut source_feature_ids = Vec::<String>::new();
    let mut contributing_priorities = Vec::<u32>::new();
    let mut winning_class = String::new();
    let mut winning_priority = 0_u32;

    for mask in masks {
        let overlap_area = intersection_area(cell_vertices, &mask.polygon);
        if overlap_area <= 1.0e-12 {
            continue;
        }
        let fraction = (overlap_area / cell_area).min(1.0);
        add_class_fraction(&mut class_fractions, &mask.mask_class, fraction);
        source_feature_ids.push(mask.feature_id.clone());
        contributing_priorities.push(mask.priority);
        if mask.priority >= winning_priority {
            winning_class = mask.mask_class.clone();
            winning_priority = mask.priority;
        }
    }

    if class_fractions.is_empty() {
        return OverlayCellResult {
            cell_id: String::new(),
            winning_class: "UNKNOWN".to_string(),
            winning_priority: 0,
            class_fractions: vec![("UNKNOWN".to_string(), 1.0)],
            source_feature_ids,
            quality_flags: vec![Flag::MissingMask.as_str().to_string()],
        };
    }

    for (_, fraction) in &mut class_fractions {
        *fraction = (*fraction).min(1.0);
    }

    // Ambiguous winner: two or more contributing masks tie at the max priority.
    // (A per-class fraction sum > 1 is legitimate overlap of distinct classes, not
    // an error — use `safety::validate_fraction_partition` for exclusive partitions.)
    let mut quality_flags = Vec::new();
    let tie_at_top = contributing_priorities
        .iter()
        .filter(|&&p| p == winning_priority)
        .count();
    if tie_at_top >= 2 {
        quality_flags.push(Flag::MaskOverlapConflict.as_str().to_string());
    }

    OverlayCellResult {
        cell_id: String::new(),
        winning_class,
        winning_priority,
        class_fractions,
        source_feature_ids,
        quality_flags,
    }
}

pub fn overlay_cells(cells: &[OverlayCellInput], masks: &[OverlayMask]) -> Vec<OverlayCellResult> {
    cells
        .iter()
        .map(|cell| {
            let mut result = overlay_cell(&cell.vertices, masks);
            result.cell_id = cell.cell_id.clone();
            result
        })
        .collect()
}

fn add_class_fraction(class_fractions: &mut Vec<(String, f64)>, mask_class: &str, fraction: f64) {
    if let Some((_, current)) = class_fractions
        .iter_mut()
        .find(|(existing_class, _)| existing_class == mask_class)
    {
        *current += fraction;
    } else {
        class_fractions.push((mask_class.to_string(), fraction));
    }
}

fn signed_area(polygon: &[Point]) -> f64 {
    if polygon.len() < 3 {
        return 0.0;
    }
    let mut total = 0.0;
    for (index, point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        total += point.x * next.y - next.x * point.y;
    }
    total * 0.5
}

fn triangulate_simple_polygon(polygon: &[Point]) -> Option<Vec<Vec<Point>>> {
    let mut vertices = normalized_polygon_vertices(polygon);
    if vertices.len() < 3 || signed_area(&vertices).abs() <= 1.0e-12 {
        return None;
    }
    if signed_area(&vertices) < 0.0 {
        vertices.reverse();
    }

    let mut triangles = Vec::<Vec<Point>>::new();
    let mut guard = 0_usize;
    while vertices.len() > 3 {
        guard += 1;
        if guard > polygon.len() * polygon.len().max(1) {
            return None;
        }
        let mut clipped_ear = false;
        for index in 0..vertices.len() {
            let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
            let current = vertices[index];
            let next = vertices[(index + 1) % vertices.len()];
            if !is_convex_ccw_vertex(previous, current, next) {
                continue;
            }
            let triangle = [previous, current, next];
            if vertices.iter().enumerate().any(|(candidate_index, point)| {
                candidate_index != index
                    && candidate_index != (index + vertices.len() - 1) % vertices.len()
                    && candidate_index != (index + 1) % vertices.len()
                    && point_in_triangle_inclusive(*point, triangle)
            }) {
                continue;
            }
            triangles.push(vec![previous, current, next]);
            vertices.remove(index);
            clipped_ear = true;
            break;
        }
        if !clipped_ear {
            return None;
        }
    }
    triangles.push(vertices);
    Some(triangles)
}

fn normalized_polygon_vertices(polygon: &[Point]) -> Vec<Point> {
    let mut vertices = Vec::<Point>::new();
    for point in polygon {
        if point.x.is_finite()
            && point.y.is_finite()
            && vertices
                .last()
                .is_none_or(|last| !points_almost_equal(*last, *point))
        {
            vertices.push(*point);
        }
    }
    if vertices.len() > 1
        && points_almost_equal(
            *vertices.first().expect("non-empty vertices"),
            *vertices.last().expect("non-empty vertices"),
        )
    {
        vertices.pop();
    }
    vertices
}

fn points_almost_equal(left: Point, right: Point) -> bool {
    (left.x - right.x).abs() <= 1.0e-12 && (left.y - right.y).abs() <= 1.0e-12
}

fn is_convex_ccw_vertex(previous: Point, current: Point, next: Point) -> bool {
    cross_product_2d(previous, current, next) > 1.0e-12
}

fn point_in_triangle_inclusive(point: Point, triangle: [Point; 3]) -> bool {
    let a = cross_product_2d(triangle[0], triangle[1], point);
    let b = cross_product_2d(triangle[1], triangle[2], point);
    let c = cross_product_2d(triangle[2], triangle[0], point);
    (a >= -1.0e-12 && b >= -1.0e-12 && c >= -1.0e-12)
        || (a <= 1.0e-12 && b <= 1.0e-12 && c <= 1.0e-12)
}

fn inside(point: Point, edge_start: Point, edge_end: Point, clip_ccw: bool) -> bool {
    let cross = (edge_end.x - edge_start.x) * (point.y - edge_start.y)
        - (edge_end.y - edge_start.y) * (point.x - edge_start.x);
    if clip_ccw {
        cross >= -1.0e-12
    } else {
        cross <= 1.0e-12
    }
}

fn line_intersection(a0: Point, a1: Point, b0: Point, b1: Point) -> Point {
    let denominator = (a0.x - a1.x) * (b0.y - b1.y) - (a0.y - a1.y) * (b0.x - b1.x);
    if denominator.abs() < 1.0e-12 {
        return a1;
    }
    let a_cross = a0.x * a1.y - a0.y * a1.x;
    let b_cross = b0.x * b1.y - b0.y * b1.x;
    Point::new(
        (a_cross * (b0.x - b1.x) - (a0.x - a1.x) * b_cross) / denominator,
        (a_cross * (b0.y - b1.y) - (a0.y - a1.y) * b_cross) / denominator,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        intersection_area, normalize_delta_lon_radians, polygon_area, polygon_union_area,
        signed_spherical_polygon_excess, spherical_polygon_area_km2, try_spherical_polygon_area,
        try_spherical_polygon_excess, Point, SphericalAreaBranch, SphericalPolygonError,
        SphericalWinding, EARTH_RADIUS_KM,
    };

    #[test]
    fn longitude_delta_normalization_handles_multiple_turns() {
        let pi = std::f64::consts::PI;
        assert_eq!(normalize_delta_lon_radians(5.0 * pi), pi);
        assert_eq!(normalize_delta_lon_radians(-5.0 * pi), -pi);
        assert!((normalize_delta_lon_radians(8.5 * pi) - 0.5 * pi).abs() < 1.0e-14);
        assert!((normalize_delta_lon_radians(-8.5 * pi) + 0.5 * pi).abs() < 1.0e-14);
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    #[test]
    fn union_area_overlap_disjoint_nested_identical() {
        // overlapping half: |A|+|B|-|A∩B| = 4+4-2 = 6
        let a = rect(0.0, 0.0, 2.0, 2.0);
        let b = rect(1.0, 0.0, 3.0, 2.0);
        assert!((polygon_union_area(&[a.clone(), b.clone()]) - 6.0).abs() < 1e-9);
        // disjoint -> sum
        let far = rect(5.0, 0.0, 7.0, 2.0);
        assert!((polygon_union_area(&[a.clone(), far]) - 8.0).abs() < 1e-9);
        // identical -> single
        assert!((polygon_union_area(&[a.clone(), a.clone()]) - 4.0).abs() < 1e-9);
        // nested -> outer
        let big = rect(0.0, 0.0, 4.0, 4.0);
        let small = rect(1.0, 1.0, 3.0, 3.0);
        assert!((polygon_union_area(&[big, small]) - 16.0).abs() < 1e-9);
        // single polygon -> its own area
        assert!((polygon_union_area(&[a]) - 4.0).abs() < 1e-9);
        // empty
        assert_eq!(polygon_union_area(&[]), 0.0);
    }

    #[test]
    fn dissolve_boxes_into_union_rings() {
        use super::{dissolve_axis_aligned_boxes, signed_ring_area};
        let sum_abs =
            |rings: &[Vec<Point>]| rings.iter().map(|r| signed_ring_area(r).abs()).sum::<f64>();
        let net = |rings: &[Vec<Point>]| rings.iter().map(|r| signed_ring_area(r)).sum::<f64>();

        // single unit box
        let r = dissolve_axis_aligned_boxes(&[(0.0, 0.0, 1.0, 1.0)]);
        assert_eq!(r.len(), 1);
        assert!((signed_ring_area(&r[0]).abs() - 1.0).abs() < 1e-9);

        // 2x2 block of unit cells -> one outer ring, area 4
        let block = [
            (0.0, 0.0, 1.0, 1.0),
            (1.0, 0.0, 2.0, 1.0),
            (0.0, 1.0, 1.0, 2.0),
            (1.0, 1.0, 2.0, 2.0),
        ];
        let r = dissolve_axis_aligned_boxes(&block);
        assert_eq!(r.len(), 1, "2x2 block dissolves to one ring");
        assert!((sum_abs(&r) - 4.0).abs() < 1e-9);

        // 3x3 ring with a hole in the middle -> outer ring + hole ring, net area 8
        let mut donut = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                if i == 1 && j == 1 {
                    continue; // hole
                }
                donut.push((i as f64, j as f64, i as f64 + 1.0, j as f64 + 1.0));
            }
        }
        let r = dissolve_axis_aligned_boxes(&donut);
        assert_eq!(r.len(), 2, "donut -> outer + hole");
        assert!((net(&r) - 8.0).abs() < 1e-9, "net signed area = 9 - 1 = 8");
        // one ring CCW (+), one CW (-)
        assert!(r.iter().any(|x| signed_ring_area(x) > 0.0));
        assert!(r.iter().any(|x| signed_ring_area(x) < 0.0));
    }

    #[test]
    fn union_area_three_overlapping_squares() {
        // chain: [0,2],[1,3],[2,4] each 2x2; union spans x[0,4] fully covered y[0,2] -> 8
        let a = rect(0.0, 0.0, 2.0, 2.0);
        let b = rect(1.0, 0.0, 3.0, 2.0);
        let c = rect(2.0, 0.0, 4.0, 2.0);
        assert!((polygon_union_area(&[a, b, c]) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn empty_or_short_polygons_have_zero_area() {
        assert_eq!(polygon_area(&[]), 0.0);
        assert_eq!(
            polygon_area(&[Point::new(0.0, 0.0), Point::new(1.0, 1.0)]),
            0.0
        );
    }
    #[test]
    fn spherical_area_handles_dateline_and_latitude() {
        let equator = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        let high_lat = vec![
            Point::new(0.0, 60.0),
            Point::new(1.0, 60.0),
            Point::new(1.0, 61.0),
            Point::new(0.0, 61.0),
        ];
        let dateline = vec![
            Point::new(179.0, 0.0),
            Point::new(-179.0, 0.0),
            Point::new(-179.0, 1.0),
            Point::new(179.0, 1.0),
        ];
        let mut dateline_reversed = dateline.clone();
        dateline_reversed.reverse();

        assert!(spherical_polygon_area_km2(&equator) > spherical_polygon_area_km2(&high_lat));
        assert!(
            (spherical_polygon_area_km2(&dateline) / spherical_polygon_area_km2(&equator) - 2.0)
                .abs()
                < 0.02
        );
        assert!(
            (spherical_polygon_area_km2(&dateline)
                - spherical_polygon_area_km2(&dateline_reversed))
            .abs()
                < 1.0e-9
        );
        assert!(signed_spherical_polygon_excess(&dateline) > 0.0);
        assert!(signed_spherical_polygon_excess(&dateline_reversed) < 0.0);
        assert!(
            (signed_spherical_polygon_excess(&dateline)
                + signed_spherical_polygon_excess(&dateline_reversed))
            .abs()
                < 1.0e-15
        );
    }

    #[test]
    fn spherical_area_uses_minor_branch_for_polar_ring() {
        let mut polar = vec![
            Point::new(0.0, 80.0),
            Point::new(120.0, 80.0),
            Point::new(-120.0, 80.0),
        ];
        let unit_area = spherical_polygon_area_km2(&polar) / (EARTH_RADIUS_KM * EARTH_RADIUS_KM);
        let expected_triangle = 0.03992494515762063;

        assert!(
            (unit_area - expected_triangle).abs() < 1.0e-12,
            "unit area={unit_area}"
        );
        assert!(unit_area < 2.0 * std::f64::consts::PI);

        polar.reverse();
        let reversed = spherical_polygon_area_km2(&polar) / (EARTH_RADIUS_KM * EARTH_RADIUS_KM);
        assert!((reversed - expected_triangle).abs() < 1.0e-12);
    }

    #[test]
    fn structured_spherical_area_distinguishes_minor_major_and_orientation() {
        let mut polar = vec![
            Point::new(0.0, 80.0),
            Point::new(120.0, 80.0),
            Point::new(-120.0, 80.0),
        ];
        let minor = try_spherical_polygon_excess(&polar, SphericalAreaBranch::Minor).unwrap();
        let major =
            try_spherical_polygon_excess(&polar, SphericalAreaBranch::MajorComplement).unwrap();
        let oriented = try_spherical_polygon_excess(&polar, SphericalAreaBranch::Oriented).unwrap();
        assert!((minor - 0.03992494515762063).abs() < 1.0e-12);
        assert!((minor + major - 4.0 * std::f64::consts::PI).abs() < 1.0e-12);
        assert!((oriented - minor).abs() < 1.0e-12);
        let summary = try_spherical_polygon_area(&polar).unwrap();
        assert_eq!(summary.winding, SphericalWinding::CounterClockwise);
        assert!((summary.major_complement_sr - major).abs() < 1.0e-12);

        polar.reverse();
        let reversed = try_spherical_polygon_excess(&polar, SphericalAreaBranch::Oriented).unwrap();
        assert!((reversed - major).abs() < 1.0e-12);
        assert_eq!(
            try_spherical_polygon_area(&polar).unwrap().winding,
            SphericalWinding::Clockwise
        );
    }

    #[test]
    fn structured_spherical_area_rejects_invalid_and_antipodal_edges() {
        assert_eq!(
            try_spherical_polygon_excess(
                &[Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
                SphericalAreaBranch::Minor,
            ),
            Err(SphericalPolygonError::TooFewVertices { found: 2 })
        );
        assert_eq!(
            try_spherical_polygon_excess(
                &[
                    Point::new(0.0, 0.0),
                    Point::new(f64::NAN, 0.0),
                    Point::new(0.0, 1.0),
                ],
                SphericalAreaBranch::Minor,
            ),
            Err(SphericalPolygonError::NonFiniteCoordinate { vertex: 1 })
        );
        assert_eq!(
            try_spherical_polygon_excess(
                &[
                    Point::new(0.0, 0.0),
                    Point::new(180.0, 0.0),
                    Point::new(90.0, 45.0),
                ],
                SphericalAreaBranch::Minor,
            ),
            Err(SphericalPolygonError::AntipodalEdge { start_vertex: 0 })
        );
        assert_eq!(
            try_spherical_polygon_excess(
                &[
                    Point::new(-10.0, -10.0),
                    Point::new(10.0, 10.0),
                    Point::new(-10.0, 10.0),
                    Point::new(10.0, -10.0),
                ],
                SphericalAreaBranch::Minor,
            ),
            Err(SphericalPolygonError::SelfIntersection {
                first_edge: 0,
                second_edge: 2,
            })
        );
        let degenerate = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(20.0, 0.0),
        ];
        assert_eq!(
            try_spherical_polygon_excess(&degenerate, SphericalAreaBranch::Minor),
            Err(SphericalPolygonError::DegenerateArea)
        );
        assert_eq!(signed_spherical_polygon_excess(&degenerate), 0.0);
    }

    #[test]
    fn nested_rectangle_intersection_keeps_inner_area() {
        let outer = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
        ];
        let inner = vec![
            Point::new(1.0, 1.0),
            Point::new(2.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(1.0, 2.0),
        ];

        assert_eq!(intersection_area(&outer, &inner), 1.0);
    }
}

/// Port of `MOD_Area_judge:ray_segment_intersect`.
///
/// Canonical returns the ray start longitude as a sentinel for no intersection.
/// Rust exposes that sentinel as `None` while keeping the same intersection math.
pub fn ray_segment_intersection_lon(
    ray_start: Point,
    lat1: f64,
    lon1: f64,
    lat2: f64,
    lon2: f64,
) -> Option<f64> {
    let lon_p = ray_start.x;
    let lat_p = ray_start.y;

    // Half-open endpoint rule: a vertex lying on the ray belongs to exactly
    // one of its two incident edges, preventing a double crossing that breaks
    // even-odd parity.
    if (lat1 > lat_p) == (lat2 > lat_p) {
        return None;
    }

    let lon_intersect = lon1 + (lat_p - lat1) * (lon2 - lon1) / (lat2 - lat1);
    if lon_intersect == lon_p {
        None
    } else {
        Some(lon_intersect)
    }
}

/// Port of `MOD_Area_judge:cross_product2`.
#[inline]
pub fn cross_product_components(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    x1 * y2 - x2 * y1
}

/// Port of `MOD_Area_judge:segments_intersect`.
///
/// This intentionally uses the strict Canonical rule (`< 0` products), so
/// endpoint touches and collinear overlaps are not intersections.
pub fn segments_intersect_strict(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    let cp1 = cross_product_components(a2.x - a1.x, a2.y - a1.y, b1.x - a1.x, b1.y - a1.y);
    let cp2 = cross_product_components(a2.x - a1.x, a2.y - a1.y, b2.x - a1.x, b2.y - a1.y);
    let cp3 = cross_product_components(b2.x - b1.x, b2.y - b1.y, a1.x - b1.x, a1.y - b1.y);
    let cp4 = cross_product_components(b2.x - b1.x, b2.y - b1.y, a2.x - b1.x, a2.y - b1.y);
    cp1 * cp2 < 0.0 && cp3 * cp4 < 0.0
}

/// First crossing reported by `MOD_Area_judge:check_self_intersection`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaJudgeSelfIntersection {
    pub first_segment_id: usize,
    pub second_segment_id: usize,
    pub first_segment: [Point; 2],
    pub second_segment: [Point; 2],
}

/// Return-value wrapper for `MOD_Area_judge:check_self_intersection`.
///
/// Canonical closes the polygon by appending point 1 at `close_num + 1`, scans
/// `i = 1..close_num-2` and `j = i+2..close_num`, prints both segment ids and
/// endpoints, then stops on the first strict intersection.  Rust preserves the
/// one-based segment ids and endpoint payload as data so callers can turn it
/// into an error without terminating the process.
pub fn area_judge_first_self_intersection_one_based(
    close_points: &[Point],
) -> Option<AreaJudgeSelfIntersection> {
    let close_num = close_points.len();
    if close_num < 3 {
        return None;
    }

    for i in 0..=(close_num - 3) {
        let first_segment = [close_points[i], close_points[i + 1]];
        for j in (i + 2)..close_num {
            let second_segment = [
                close_points[j],
                if j + 1 == close_num {
                    close_points[0]
                } else {
                    close_points[j + 1]
                },
            ];
            if segments_intersect_strict(
                first_segment[0],
                first_segment[1],
                second_segment[0],
                second_segment[1],
            ) {
                return Some(AreaJudgeSelfIntersection {
                    first_segment_id: i + 1,
                    second_segment_id: j + 1,
                    first_segment,
                    second_segment,
                });
            }
        }
    }

    None
}

/// Port of `MOD_Area_judge:CheckCrossing`.
///
/// This shifts longitudes by 180 degrees when a closed polygon crosses the
/// dateline so ray-intersection intervals can be processed on a continuous axis.
pub fn shift_longitudes_for_dateline_crossing(points: &[Point]) -> Vec<Point> {
    points
        .iter()
        .map(|point| {
            let shifted_lon = if point.x < 0.0 {
                point.x + 180.0
            } else {
                point.x - 180.0
            };
            Point::new(shifted_lon, point.y)
        })
        .collect()
}
