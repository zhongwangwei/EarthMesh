//! Even out the hexagonal (Voronoi) cells of a spherical triangulation.
//!
//! A hex mesh is read off a triangulation: a cell's corners are the
//! circumcentres of the triangles around its generator. Every triangle can
//! sit inside the angle window while a cell is far from a hexagon: where the
//! neighbours on one side are much finer than on the other, one dual edge can
//! be a quarter of another (a LEPP-Delaunay transition cell, 31 against
//! 128 km, all its triangles between 40 and 80 degrees). The quality check
//! warns at a longest-to-shortest edge ratio of 4 and an edge-length CV of
//! 0.35, and nothing in the angle-window repair looks at either.
//!
//! This pass moves generators -- never the connectivity -- to lower the worst
//! of the two ratios, each over its limit, among the cells a move touches:
//! the vertex's own and its neighbours'. A move must keep every triangle at
//! the vertex in its orientation and inside the angle window, and every edge
//! there locally Delaunay: past 180 degrees of opposite angles the two
//! circumcentres change places and the cell's ring folds, which a length
//! measured between them would not show.
//!
//! The moves keep the triangles inside the window narrowed on both sides
//! first, so the triangles give up as little as the cells need; a cell still
//! over a limit after that may use the whole window.

use crate::mesh_angle_window::spherical_triangle_angles_deg;
use std::collections::HashMap;

/// What the pass works toward and what it may touch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DualShapeOptions {
    /// Longest over shortest edge of a cell that counts as over the limit.
    pub aspect_limit: f64,
    /// Edge-length coefficient of variation of a cell that counts as over.
    pub edge_cv_limit: f64,
    /// Largest departure (degrees) of an interior angle from the angle of a
    /// regular polygon of the cell's size that counts as over -- the quality
    /// check's angle deviation. In the badness so that evening edges or sizes
    /// cannot buy it with a bent cell.
    pub angle_deviation_limit: f64,
    /// Cells above this fraction of either limit are worked on.
    pub work_above: f64,
    /// Ratio of two neighbouring cells' sizes (square roots of their areas)
    /// past which the larger counts as over: the quality check warns past
    /// 2. Only a ratio over it counts -- a level jump is a ratio of two by
    /// construction, and working every transition cell toward one would be
    /// work for nothing.
    pub resolution_ratio_limit: f64,
    /// Every triangle at a moved vertex stays inside this window (degrees).
    pub window_deg: (f64, f64),
    /// The first stage keeps the triangles this far inside the window.
    pub narrowing_deg: f64,
    /// An edge stays this far below 180 degrees of opposite angles.
    pub delaunay_margin_deg: f64,
    /// First real vertex row; rows below are placeholders.
    pub first_vertex: usize,
    /// First real face row; rows below are placeholders.
    pub first_face: usize,
    /// Sweeps over the cells above the bar, per stage.
    pub max_sweeps: usize,
    /// A cell's corners are its triangles' centroids rather than their
    /// circumcentres: the convention of the mesh being written, which is the
    /// cell a model reads and the quality check measures.
    pub centroid_corners: bool,
}

impl DualShapeOptions {
    pub fn new(window_deg: (f64, f64)) -> Self {
        Self {
            aspect_limit: 4.0,
            edge_cv_limit: 0.35,
            angle_deviation_limit: 35.0,
            work_above: 0.8,
            resolution_ratio_limit: 2.0,
            window_deg,
            narrowing_deg: 3.0,
            delaunay_margin_deg: 1.0,
            first_vertex: 0,
            first_face: 0,
            max_sweeps: 5,
            centroid_corners: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DualShapeReport {
    pub max_aspect_before: f64,
    pub max_edge_cv_before: f64,
    /// Cells over either limit.
    pub over_limit_before: usize,
    pub max_aspect_after: f64,
    pub max_edge_cv_after: f64,
    pub over_limit_after: usize,
    /// Neighbouring cell pairs whose size ratio is over its limit.
    pub ratio_pairs_before: usize,
    pub ratio_pairs_after: usize,
    pub max_ratio_before: f64,
    pub max_ratio_after: f64,
    pub moves: usize,
    pub sweeps: usize,
}

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dotp(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn crossp(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: P) -> P {
    let n = dotp(a, a).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}
fn arc(a: P, b: P) -> f64 {
    let c = crossp(a, b);
    dotp(c, c).sqrt().atan2(dotp(a, b))
}

/// The circumcentre of a spherical triangle, on the triangle's side.
fn circumcentre(corners: [P; 3]) -> P {
    let [a, b, c] = corners;
    let n = crossp(sub(b, a), sub(c, a));
    let n = if dotp(n, a) < 0.0 {
        [-n[0], -n[1], -n[2]]
    } else {
        n
    };
    unit(n)
}

/// Area of a spherical triangle of unit vectors.
fn triangle_area(a: P, b: P, c: P) -> f64 {
    let numerator = dotp(a, crossp(b, c)).abs();
    let denominator = 1.0 + dotp(a, b) + dotp(b, c) + dotp(c, a);
    2.0 * numerator.atan2(denominator)
}

/// Largest |interior angle - regular-polygon angle| of a ring of corners
/// around generator `g`, in degrees.
fn angle_deviation(g: P, corners: &[P]) -> f64 {
    let n = corners.len();
    let area = (0..n)
        .map(|k| triangle_area(g, corners[k], corners[(k + 1) % n]))
        .sum::<f64>();
    let ideal = (((n as f64 - 2.0) * std::f64::consts::PI + area) / n as f64).to_degrees();
    let tangent = |p: P, q: P| {
        let along = dotp(q, p);
        unit([
            q[0] - along * p[0],
            q[1] - along * p[1],
            q[2] - along * p[2],
        ])
    };
    (0..n)
        .map(|k| {
            let p = corners[k];
            let (a, b) = (
                tangent(p, corners[(k + n - 1) % n]),
                tangent(p, corners[(k + 1) % n]),
            );
            let angle = dotp(a, b).clamp(-1.0, 1.0).acos().to_degrees();
            (angle - ideal).abs()
        })
        .fold(0.0, f64::max)
}

/// (longest / shortest edge, edge-length CV) of a ring of corners.
fn ring_shape(corners: &[P]) -> (f64, f64) {
    let lengths: Vec<f64> = (0..corners.len())
        .map(|k| arc(corners[k], corners[(k + 1) % corners.len()]))
        .collect();
    let n = lengths.len() as f64;
    let mean = lengths.iter().sum::<f64>() / n;
    let min = lengths.iter().copied().fold(f64::INFINITY, f64::min);
    let max = lengths.iter().copied().fold(0.0, f64::max);
    let var = lengths.iter().map(|l| (l - mean) * (l - mean)).sum::<f64>() / n;
    let aspect = if min > 0.0 { max / min } else { f64::INFINITY };
    let cv = if mean > 0.0 {
        var.sqrt() / mean
    } else {
        f64::INFINITY
    };
    (aspect, cv)
}

struct Dual<'a> {
    points: &'a mut [P],
    faces: &'a [[usize; 3]],
    options: DualShapeOptions,
    sign: Vec<f64>,
    /// Faces around each vertex in rotational order; `None` if the ring is
    /// open (a boundary vertex, which has no closed cell and does not move).
    rings: Vec<Option<Vec<usize>>>,
    neighbours: Vec<Vec<usize>>,
    edge_faces: HashMap<(usize, usize), Vec<usize>>,
}

impl Dual<'_> {
    fn at(&self, i: usize, over: (usize, P)) -> P {
        if i == over.0 {
            over.1
        } else {
            self.points[i]
        }
    }

    fn corners(&self, f: usize, over: (usize, P)) -> [P; 3] {
        self.faces[f].map(|i| self.at(i, over))
    }

    /// The cell corner face `f` contributes, in the mesh's convention.
    fn corner(&self, f: usize, over: (usize, P)) -> P {
        let corners = self.corners(f, over);
        if self.options.centroid_corners {
            let [a, b, c] = corners;
            unit([a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]])
        } else {
            circumcentre(corners)
        }
    }

    /// The square root of `v`'s cell area, `None` without a closed cell.
    fn scale(&self, v: usize, over: (usize, P)) -> Option<f64> {
        let ring = self.rings[v].as_ref()?;
        let g = self.at(v, over);
        let corners: Vec<P> = ring.iter().map(|&f| self.corner(f, over)).collect();
        let area = (0..corners.len())
            .map(|k| triangle_area(g, corners[k], corners[(k + 1) % corners.len()]))
            .sum::<f64>();
        Some(area.sqrt())
    }

    /// The worst size ratio of `v`'s cell to a neighbour's over the limit,
    /// as a fraction of it; 0 when none is over.
    fn ratio_badness(&self, v: usize, scale_of: impl Fn(usize) -> Option<f64>) -> f64 {
        let Some(here) = scale_of(v) else {
            return 0.0;
        };
        let limit = self.options.resolution_ratio_limit;
        self.neighbours[v]
            .iter()
            .filter_map(|&w| scale_of(w))
            .map(|there| here.max(there) / here.min(there))
            .filter(|&ratio| ratio > limit)
            .fold(0.0, |worst: f64, ratio| worst.max(ratio / limit))
    }

    /// max(aspect / limit, CV / limit, size ratio / limit) of `v`'s cell, 0
    /// without one.
    fn badness(&self, v: usize, over: (usize, P)) -> f64 {
        let Some(ring) = &self.rings[v] else {
            return 0.0;
        };
        let corners: Vec<P> = ring.iter().map(|&f| self.corner(f, over)).collect();
        let (aspect, cv) = ring_shape(&corners);
        let shape = (aspect / self.options.aspect_limit)
            .max(cv / self.options.edge_cv_limit)
            .max(angle_deviation(self.at(v, over), &corners) / self.options.angle_deviation_limit);
        shape.max(self.ratio_badness(v, |c| self.scale(c, over)))
    }

    /// `badness` with every cell's size read from `scales`.
    fn badness_with(&self, v: usize, scales: &[Option<f64>]) -> f64 {
        let Some(ring) = &self.rings[v] else {
            return 0.0;
        };
        let none = (usize::MAX, [0.0; 3]);
        let corners: Vec<P> = ring.iter().map(|&f| self.corner(f, none)).collect();
        let (aspect, cv) = ring_shape(&corners);
        let shape = (aspect / self.options.aspect_limit)
            .max(cv / self.options.edge_cv_limit)
            .max(angle_deviation(self.points[v], &corners) / self.options.angle_deviation_limit);
        shape.max(self.ratio_badness(v, |c| scales[c]))
    }

    fn scales(&self) -> Vec<Option<f64>> {
        let none = (usize::MAX, [0.0; 3]);
        (0..self.rings.len()).map(|v| self.scale(v, none)).collect()
    }

    /// (pairs of neighbouring cells over the ratio limit, worst ratio).
    fn ratio_stats(&self) -> (usize, f64) {
        let scales = self.scales();
        let mut out = (0usize, 0.0_f64);
        for (v, here) in scales.iter().enumerate() {
            let Some(here) = here else { continue };
            for &w in &self.neighbours[v] {
                if w <= v {
                    continue;
                }
                let Some(there) = scales[w] else { continue };
                let ratio = here.max(there) / here.min(there);
                out.1 = out.1.max(ratio);
                out.0 += usize::from(ratio > self.options.resolution_ratio_limit);
            }
        }
        out
    }

    /// (worst badness, sum of squares) over `u`'s cell and its neighbours'.
    fn score(&self, u: usize, over: (usize, P)) -> (f64, f64) {
        std::iter::once(u)
            .chain(self.neighbours[u].iter().copied())
            .map(|c| self.badness(c, over))
            .fold((0.0, 0.0), |(worst, sum), b| (worst.max(b), sum + b * b))
    }

    fn better(new: (f64, f64), old: (f64, f64)) -> bool {
        const EPS: f64 = 1.0e-9;
        new.0 < old.0 - EPS || (new.0 < old.0 + EPS && new.1 < old.1 - EPS)
    }

    /// The angle of face `f` opposite edge `e`, with `over` applied.
    fn opposite_angle(&self, f: usize, e: (usize, usize), over: (usize, P)) -> f64 {
        let corners = self.faces[f];
        let k = (0..3)
            .find(|&k| corners[k] != e.0 && corners[k] != e.1)
            .expect("a face has a corner off each of its edges");
        spherical_triangle_angles_deg(self.corners(f, over))[k]
    }

    /// The sum of the two angles opposite `e`, or `None` on an open edge.
    fn opposite_sum(&self, e: (usize, usize), over: (usize, P)) -> Option<f64> {
        match self.edge_faces.get(&e).map(Vec::as_slice) {
            Some(&[f1, f2]) => {
                Some(self.opposite_angle(f1, e, over) + self.opposite_angle(f2, e, over))
            }
            _ => None,
        }
    }

    /// The edges of `u`'s triangles: its spokes and the link around it.
    fn star_edges(&self, u: usize) -> Vec<(usize, usize)> {
        let mut edges: Vec<(usize, usize)> = self.rings[u]
            .iter()
            .flatten()
            .flat_map(|&f| {
                let [a, b, c] = self.faces[f];
                [(a, b), (b, c), (c, a)]
            })
            .map(|(x, y)| (x.min(y), x.max(y)))
            .collect();
        edges.sort_unstable();
        edges.dedup();
        edges
    }

    /// Whether `u` may stand at `p`: its triangles keep their orientation
    /// and stay inside `window`, and no edge of its star goes past the
    /// Delaunay bar -- or, where one already was, further past it.
    fn admissible(
        &self,
        u: usize,
        p: P,
        window: (f64, f64),
        edges: &[((usize, usize), f64)],
    ) -> bool {
        let over = (u, p);
        let ring = self.rings[u].as_deref().unwrap_or_default();
        for &f in ring {
            let [a, b, c] = self.corners(f, over);
            if dotp(crossp(sub(b, a), sub(c, a)), a) * self.sign[f] <= 0.0 {
                return false;
            }
            let angles = spherical_triangle_angles_deg([a, b, c]);
            if angles.iter().any(|&x| x < window.0 || x > window.1) {
                return false;
            }
        }
        let bar = 180.0 - self.options.delaunay_margin_deg;
        edges.iter().all(|&(e, before)| {
            self.opposite_sum(e, over)
                .is_none_or(|sum| sum <= bar.max(before))
        })
    }

    /// Pattern search on `u`'s position: eight directions, the step halved
    /// when none improves. Returns whether it moved.
    fn improve(&mut self, u: usize, window: (f64, f64)) -> bool {
        if self.rings[u].is_none() || self.neighbours[u].is_empty() {
            return false;
        }
        let edges: Vec<((usize, usize), f64)> = self
            .star_edges(u)
            .into_iter()
            .map(|e| {
                (
                    e,
                    self.opposite_sum(e, (usize::MAX, [0.0; 3])).unwrap_or(0.0),
                )
            })
            .collect();
        let h = self.neighbours[u]
            .iter()
            .map(|&w| {
                let d = sub(self.points[w], self.points[u]);
                dotp(d, d).sqrt()
            })
            .sum::<f64>()
            / self.neighbours[u].len() as f64;
        let none = (usize::MAX, [0.0; 3]);
        let mut current = self.score(u, none);
        let mut step = 0.15 * h;
        let mut accepted = 0;
        while step > 0.005 * h && accepted < 40 {
            let p = self.points[u];
            let axis = if p[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let e1 = unit(crossp(p, axis));
            let e2 = crossp(p, e1);
            let mut best: Option<((f64, f64), P)> = None;
            for k in 0..8 {
                let (s, c) = (k as f64 * std::f64::consts::FRAC_PI_4).sin_cos();
                let q = unit([
                    p[0] + step * (c * e1[0] + s * e2[0]),
                    p[1] + step * (c * e1[1] + s * e2[1]),
                    p[2] + step * (c * e1[2] + s * e2[2]),
                ]);
                if !self.admissible(u, q, window, &edges) {
                    continue;
                }
                let score = self.score(u, (u, q));
                if Self::better(score, best.map_or(current, |(b, _)| b)) {
                    best = Some((score, q));
                }
            }
            match best {
                Some((score, q)) => {
                    self.points[u] = q;
                    current = score;
                    accepted += 1;
                }
                None => step *= 0.5,
            }
        }
        accepted > 0
    }

    /// (max aspect, max CV, cells over either limit) over every closed cell.
    fn stats(&self) -> (f64, f64, usize) {
        let none = (usize::MAX, [0.0; 3]);
        let mut out = (0.0_f64, 0.0_f64, 0usize);
        for ring in self.rings.iter().flatten() {
            let corners: Vec<P> = ring.iter().map(|&f| self.corner(f, none)).collect();
            let (aspect, cv) = ring_shape(&corners);
            out.0 = out.0.max(aspect);
            out.1 = out.1.max(cv);
            if aspect > self.options.aspect_limit || cv > self.options.edge_cv_limit {
                out.2 += 1;
            }
        }
        out
    }
}

/// Move generators of `points` so the cells of the triangulation `faces`
/// come under the options' limits; the faces themselves are not changed.
///
/// `points` are unit vectors. Vertices on an open edge stay where they are,
/// and so do rows below `options.first_vertex`.
pub fn even_out_dual_cells(
    points: &mut [[f64; 3]],
    faces: &[[usize; 3]],
    options: DualShapeOptions,
) -> DualShapeReport {
    let vertex_count = points.len();
    let first_face = options.first_face.min(faces.len());
    let mut sign = vec![0.0; faces.len()];
    let mut incident = vec![Vec::new(); vertex_count];
    let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (f, &[a, b, c]) in faces.iter().enumerate().skip(first_face) {
        let (pa, pb, pc) = (points[a], points[b], points[c]);
        sign[f] = dotp(crossp(sub(pb, pa), sub(pc, pa)), pa).signum();
        for v in [a, b, c] {
            incident[v].push(f);
        }
        for (x, y) in [(a, b), (b, c), (c, a)] {
            edge_faces.entry((x.min(y), x.max(y))).or_default().push(f);
        }
    }
    let mut open = vec![false; vertex_count];
    for (&(x, y), around) in &edge_faces {
        if around.len() != 2 {
            open[x] = true;
            open[y] = true;
        }
    }
    let rings: Vec<Option<Vec<usize>>> = (0..vertex_count)
        .map(|v| {
            if v < options.first_vertex || open[v] || incident[v].len() < 3 {
                return None;
            }
            // Around `v`, face (v, a, b) is followed by the face whose corner
            // after `v` is `b`.
            let mut after = HashMap::with_capacity(incident[v].len());
            for &f in &incident[v] {
                let corners = faces[f];
                let i = corners.iter().position(|&u| u == v)?;
                after.insert(corners[(i + 1) % 3], (f, corners[(i + 2) % 3]));
            }
            let start = *incident[v].iter().min()?;
            let i = faces[start].iter().position(|&u| u == v)?;
            let mut ring = vec![start];
            let mut key = faces[start][(i + 2) % 3];
            loop {
                let &(f, next) = after.get(&key)?;
                if f == start {
                    break;
                }
                if ring.len() >= incident[v].len() {
                    return None;
                }
                ring.push(f);
                key = next;
            }
            (ring.len() == incident[v].len()).then_some(ring)
        })
        .collect();
    let neighbours: Vec<Vec<usize>> = (0..vertex_count)
        .map(|v| {
            let mut out: Vec<usize> = incident[v]
                .iter()
                .flat_map(|&f| faces[f])
                .filter(|&u| u != v)
                .collect();
            out.sort_unstable();
            out.dedup();
            out
        })
        .collect();
    let mut dual = Dual {
        points,
        faces,
        options,
        sign,
        rings,
        neighbours,
        edge_faces,
    };

    let (max_aspect_before, max_edge_cv_before, over_limit_before) = dual.stats();
    let (ratio_pairs_before, max_ratio_before) = dual.ratio_stats();
    let mut report = DualShapeReport {
        max_aspect_before,
        max_edge_cv_before,
        over_limit_before,
        ratio_pairs_before,
        max_ratio_before,
        ..DualShapeReport::default()
    };
    let (lo, hi) = options.window_deg;
    let narrowed = (lo + options.narrowing_deg, hi - options.narrowing_deg);
    let none = (usize::MAX, [0.0; 3]);
    // Narrowed first, for every cell above the working bar; then the whole
    // window, only for the cells still over a limit.
    for (window, bar) in [(narrowed, options.work_above), (options.window_deg, 1.0)] {
        if window.0 >= window.1 {
            continue;
        }
        for _ in 0..options.max_sweeps {
            let scales = dual.scales();
            let mut bad: Vec<(f64, usize)> = (0..vertex_count)
                .filter(|&v| dual.rings[v].is_some())
                .map(|v| (dual.badness_with(v, &scales), v))
                .filter(|&(b, _)| b > bar)
                .collect();
            if bad.is_empty() {
                break;
            }
            bad.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            report.sweeps += 1;
            let mut moved = 0;
            for (_, cell) in bad {
                if dual.badness(cell, none) <= bar {
                    continue;
                }
                let around = std::iter::once(cell)
                    .chain(dual.neighbours[cell].iter().copied())
                    .collect::<Vec<_>>();
                for u in around {
                    if dual.improve(u, window) {
                        moved += 1;
                    }
                }
            }
            report.moves += moved;
            if moved == 0 {
                break;
            }
        }
    }
    let (max_aspect_after, max_edge_cv_after, over_limit_after) = dual.stats();
    (report.ratio_pairs_after, report.max_ratio_after) = dual.ratio_stats();
    report.max_aspect_after = max_aspect_after;
    report.max_edge_cv_after = max_edge_cv_after;
    report.over_limit_after = over_limit_after;
    report
}

#[cfg(test)]
mod tests;
