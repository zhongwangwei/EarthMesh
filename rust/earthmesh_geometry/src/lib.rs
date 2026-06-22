#[cfg(feature = "extension-module")]
use pyo3::prelude::*;

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

/// Earth radius in kilometers used by `MOD_Area_judge:haversine`.
///
/// The Fortran routine initializes `erad = 6371229` meters and computes
/// `erad / 1000 * central_angle`.
pub const EARTH_RADIUS_KM: f64 = 6_371.229;

/// Port of `MOD_Area_judge:cross_product`.
#[inline]
pub fn cross_product_2d(p1: Point, p2: Point, p3: Point) -> f64 {
    (p2.x - p1.x) * (p3.y - p1.y) - (p2.y - p1.y) * (p3.x - p1.x)
}

/// Port of `MOD_Area_judge:haversine`.
///
/// Inputs use the same convention as the Fortran `point_i(2)` arrays:
/// `x = longitude degrees`, `y = latitude degrees`. The return value is km.
pub fn haversine_km(point_i: Point, point_c: Point) -> f64 {
    let px1 = point_i.x.to_radians();
    let py1 = point_i.y.to_radians();
    let px2 = point_c.x.to_radians();
    let py2 = point_c.y.to_radians();

    let v = (py1 / 2.0 - py2 / 2.0).sin().powi(2)
        + py2.cos() * py1.cos() * (px1 / 2.0 - px2 / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * v.sqrt().atan2((1.0 - v).sqrt())
}

/// Port of `MOD_Area_judge:is_point_in_circle`.
#[inline]
pub fn is_point_in_circle_km(point: Point, center: Point, center_radius_km: f64) -> bool {
    haversine_km(point, center) <= center_radius_km
}

/// Port of `MOD_Area_judge:is_point_in_convex_polygon`.
///
/// Boundary points are considered inside, matching the Fortran behavior where
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
                .map_or(true, |last| !points_almost_equal(*last, *point))
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
    use super::{intersection_area, polygon_area, polygon_union_area, Point};

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

#[cfg(feature = "extension-module")]
#[pyfunction(name = "polygon_area")]
fn py_polygon_area(vertices: Vec<(f64, f64)>) -> f64 {
    let points = tuples_to_points(vertices);
    polygon_area(&points)
}

#[cfg(feature = "extension-module")]
#[pyfunction(name = "intersection_area")]
fn py_intersection_area(subject: Vec<(f64, f64)>, clip: Vec<(f64, f64)>) -> f64 {
    let subject_points = tuples_to_points(subject);
    let clip_points = tuples_to_points(clip);
    intersection_area(&subject_points, &clip_points)
}

#[cfg(feature = "extension-module")]
#[pyfunction(name = "overlay_cell")]
fn py_overlay_cell(
    cell_vertices: Vec<(f64, f64)>,
    masks: Vec<(String, String, u32, Vec<(f64, f64)>)>,
) -> (String, u32, Vec<(String, f64)>, Vec<String>, Vec<String>) {
    let cell_points = tuples_to_points(cell_vertices);
    let overlay_masks = masks
        .into_iter()
        .map(|(feature_id, mask_class, priority, polygon)| OverlayMask {
            feature_id,
            mask_class,
            priority,
            polygon: tuples_to_points(polygon),
        })
        .collect::<Vec<_>>();
    let result = overlay_cell(&cell_points, &overlay_masks);
    (
        result.winning_class,
        result.winning_priority,
        result.class_fractions,
        result.source_feature_ids,
        result.quality_flags,
    )
}

#[cfg(feature = "extension-module")]
type PyOverlayCellsResult = Vec<(
    String,
    String,
    u32,
    Vec<(String, f64)>,
    Vec<String>,
    Vec<String>,
)>;

#[cfg(feature = "extension-module")]
#[pyfunction(name = "overlay_cells")]
fn py_overlay_cells(
    cells: Vec<(String, Vec<(f64, f64)>)>,
    masks: Vec<(String, String, u32, Vec<(f64, f64)>)>,
) -> PyOverlayCellsResult {
    let overlay_cells_input = cells
        .into_iter()
        .map(|(cell_id, vertices)| OverlayCellInput {
            cell_id,
            vertices: tuples_to_points(vertices),
        })
        .collect::<Vec<_>>();
    let overlay_masks = masks
        .into_iter()
        .map(|(feature_id, mask_class, priority, polygon)| OverlayMask {
            feature_id,
            mask_class,
            priority,
            polygon: tuples_to_points(polygon),
        })
        .collect::<Vec<_>>();
    overlay_cells(&overlay_cells_input, &overlay_masks)
        .into_iter()
        .map(|result| {
            (
                result.cell_id,
                result.winning_class,
                result.winning_priority,
                result.class_fractions,
                result.source_feature_ids,
                result.quality_flags,
            )
        })
        .collect()
}

#[cfg(feature = "extension-module")]
fn tuples_to_points(vertices: Vec<(f64, f64)>) -> Vec<Point> {
    vertices
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect()
}

#[cfg(feature = "extension-module")]
#[pymodule]
fn earthmesh_geometry(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(py_polygon_area, module)?)?;
    module.add_function(wrap_pyfunction!(py_intersection_area, module)?)?;
    module.add_function(wrap_pyfunction!(py_overlay_cell, module)?)?;
    module.add_function(wrap_pyfunction!(py_overlay_cells, module)?)?;
    Ok(())
}

/// Port of `MOD_Area_judge:ray_segment_intersect`.
///
/// Fortran returns the ray start longitude as a sentinel for no intersection.
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

    if lat1 == lat2 {
        return None;
    }
    if (lat1 > lat_p && lat2 > lat_p) || (lat1 < lat_p && lat2 < lat_p) {
        return None;
    }

    let m = (lat2 - lat1) / (lon2 - lon1);
    let lon_intersect = lon1 + (lat_p - lat1) / m;
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
/// This intentionally uses the strict Fortran rule (`< 0` products), so
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
/// Fortran closes the polygon by appending point 1 at `close_num + 1`, scans
/// `i = 1..close_num-2` and `j = i+2..close_num`, prints both segment ids and
/// endpoints, then stops on the first strict intersection.  Rust preserves the
/// one-based segment ids and endpoint payload as data so callers can turn it
/// into an error without terminating the process.
pub fn area_judge_first_self_intersection_fortran_indexed(
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
