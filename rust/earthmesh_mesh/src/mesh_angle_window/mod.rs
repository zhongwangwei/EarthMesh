//! Bring every interior angle of a spherical triangulation into a window.
//!
//! The output contract (guide 11.72) fails any all-triangle mesh with an angle
//! outside 35-85 degrees, whatever built it. A refinement that closes its seams
//! by bisection (red-green) leaves 30/60/90 triangles along every level change,
//! and no amount of further refinement removes them: the closure makes them
//! again one level down. What does remove them is local, and in this order:
//!
//! 1. **Valence.** A vertex with `k` triangles around it averages `360/k`
//!    degrees, so a vertex of valence 11 cannot reach 35 and one of valence 4
//!    cannot stay under 85 however it is moved. Edge flips that bring the
//!    valences of an edge's four vertices closer to six come first.
//! 2. **Vertices of valence 3 or 4** that flips cannot fix are removed, and
//!    their ring is re-triangulated.
//! 3. **Position.** Each vertex of a triangle still outside the window is moved
//!    -- first toward the centre of its ring, then by a pattern search -- to
//!    raise the smallest margin to the window among its triangles.
//!
//! Every move and removal is accepted only if it improves the triangles it
//! touches, so the pass never makes the worst local angle worse; flips may,
//! down to `flip_floor_deg`, and the moves then recover it. Faces carry a
//! level that a replacement face inherits as the deepest of the faces it
//! was cut from.

/// Where the window is and what the pass may touch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleWindowOptions {
    /// Target window in degrees. Callers that are checked against a window
    /// should pass it narrowed by a small margin, since coordinates are
    /// rounded on the way to the file.
    pub window_deg: (f64, f64),
    /// First real vertex row; rows below are placeholders.
    pub first_vertex: usize,
    /// First real face row; rows below are placeholders.
    pub first_face: usize,
    /// Vertices with a lower id are never removed (they may still move), so
    /// ids below it are the same after the pass.
    pub removable_from: usize,
    /// No flip may raise a vertex above this many triangles.
    pub max_valence: usize,
    /// No flip may produce an angle below this.
    pub flip_floor_deg: f64,
    pub max_rounds: usize,
    /// Allow edge flips and vertex removal. Without them only vertices move,
    /// so every row of the mesh keeps its meaning.
    pub allow_topology_changes: bool,
    /// Rounds of the second phase, which pushes every angle toward 60 degrees
    /// once the window is met. Zero stops at the window.
    pub equilateral_rounds: usize,
    /// A vertex whose triangles all stay within this many degrees of 60 is
    /// left where it is in the second phase.
    pub equilateral_tolerance_deg: f64,
}

impl AngleWindowOptions {
    pub fn new(window_deg: (f64, f64)) -> Self {
        Self {
            window_deg,
            first_vertex: 0,
            first_face: 0,
            removable_from: 0,
            max_valence: usize::MAX,
            flip_floor_deg: 15.0,
            max_rounds: 12,
            allow_topology_changes: true,
            equilateral_rounds: 4,
            equilateral_tolerance_deg: 5.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AngleWindowReport {
    pub outside_before: usize,
    pub outside_after: usize,
    pub min_angle_before: f64,
    pub max_angle_before: f64,
    pub min_angle_after: f64,
    pub max_angle_after: f64,
    pub flips: usize,
    pub moves: usize,
    pub removed_vertices: usize,
    pub rounds: usize,
    /// Rounds the second (toward-60) phase ran.
    pub equilateral_rounds: usize,
    /// Largest and mean |angle - 60| over every interior angle.
    pub max_deviation_before: f64,
    pub mean_deviation_before: f64,
    pub max_deviation_after: f64,
    pub mean_deviation_after: f64,
    /// Mean |angle - 60| once the window phase had finished.
    pub mean_deviation_window_phase: f64,
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
fn norm(a: P) -> f64 {
    dotp(a, a).sqrt()
}
fn unit(a: P) -> P {
    let n = norm(a);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// Interior angles of a spherical triangle, in degrees: the angle between the
/// two great circles at each corner, which is what the quality check measures.
pub fn spherical_triangle_angles_deg(corners: [P; 3]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for k in 0..3 {
        let p = corners[k];
        let mut u = sub(corners[(k + 1) % 3], p);
        let mut v = sub(corners[(k + 2) % 3], p);
        let up = dotp(u, p);
        let vp = dotp(v, p);
        u = sub(u, [up * p[0], up * p[1], up * p[2]]);
        v = sub(v, [vp * p[0], vp * p[1], vp * p[2]]);
        let c = dotp(u, v) / (norm(u) * norm(v));
        out[k] = c.clamp(-1.0, 1.0).acos().to_degrees();
    }
    out
}

struct Work<'a> {
    points: &'a mut Vec<P>,
    faces: &'a mut Vec<[usize; 3]>,
    levels: &'a mut Vec<usize>,
    alive: Vec<bool>,
    sign: Vec<f64>,
    incident: Vec<Vec<usize>>,
    fixed: Vec<bool>,
    removed: Vec<bool>,
    options: AngleWindowOptions,
    mode: Mode,
}

/// What a move is scored against. Larger scores are better in every mode.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// Into the contract's window: (worst margin, sum of negative margins).
    Window,
    /// Lower the worst deviation from 60 around each vertex.
    Minimax,
    /// Inside `cap` -- the window, tightened to the worst deviation the
    /// minimax pass reached -- minimise the squared deviation from 60 over
    /// every angle, so the rest of the mesh moves toward equilateral without
    /// giving back what the minimax pass won at the extremes.
    Energy { cap: (f64, f64) },
}

impl Work<'_> {
    fn orient(&self, f: [usize; 3]) -> f64 {
        let [a, b, c] = f.map(|i| self.points[i]);
        dotp(crossp(sub(b, a), sub(c, a)), a)
    }

    fn angles(&self, f: [usize; 3]) -> [f64; 3] {
        spherical_triangle_angles_deg(f.map(|i| self.points[i]))
    }

    fn margin_to(&self, f: [usize; 3], window: (f64, f64)) -> f64 {
        let a = self.angles(f);
        let lo = a.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = a.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (lo - window.0).min(window.1 - hi)
    }

    fn margin_of(&self, f: [usize; 3]) -> f64 {
        self.margin_to(f, self.options.window_deg)
    }

    /// Larger is better. First phase: (worst margin to the window, sum of the
    /// negative margins). Second phase: (window violation, clamped at zero,
    /// then minus the sum of squared deviations from 60 over every angle), so
    /// a move may trade one angle against another but never leave the window.
    fn score_corners(&self, faces: impl Iterator<Item = [usize; 3]>) -> (f64, f64) {
        let mut worst = f64::INFINITY;
        let mut second = 0.0;
        for f in faces {
            match self.mode {
                Mode::Window => {
                    let m = self.margin_of(f);
                    worst = worst.min(m);
                    second += m.min(0.0);
                }
                Mode::Minimax => {
                    let m = self.margin_to(f, (60.0, 60.0));
                    worst = worst.min(m);
                    second += m;
                }
                Mode::Energy { cap } => {
                    worst = worst.min(self.margin_to(f, cap));
                    second -= self
                        .angles(f)
                        .iter()
                        .map(|a| (a - 60.0) * (a - 60.0))
                        .sum::<f64>();
                }
            }
        }
        match self.mode {
            Mode::Energy { .. } => (worst.min(0.0), second),
            _ => (worst, second),
        }
    }

    /// Whether the second phase leaves `v` alone: every angle around it is
    /// already within the tolerance of 60.
    fn needs_no_move(&self, v: usize) -> bool {
        match self.mode {
            Mode::Window => self.score(&self.incident[v]).0 >= 0.0,
            Mode::Minimax | Mode::Energy { .. } => self.is_settled(v),
        }
    }

    fn is_settled(&self, v: usize) -> bool {
        let tolerance = self.options.equilateral_tolerance_deg;
        self.incident[v]
            .iter()
            .all(|&f| self.deviation(f) <= tolerance)
    }

    /// Largest |angle - 60| of a face.
    fn deviation(&self, f: usize) -> f64 {
        -self.margin_to(self.faces[f], (60.0, 60.0))
    }

    fn score(&self, faces: &[usize]) -> (f64, f64) {
        self.score_corners(faces.iter().map(|&f| self.faces[f]))
    }

    fn better(new: (f64, f64), old: (f64, f64)) -> bool {
        const EPS: f64 = 1.0e-9;
        new.0 > old.0 + EPS || (new.0 > old.0 - EPS && new.1 > old.1 + EPS)
    }

    fn outside(&self, f: usize) -> bool {
        self.margin_to(self.faces[f], self.options.window_deg) < 0.0
    }

    fn orientation_kept(&self, faces: &[usize]) -> bool {
        faces
            .iter()
            .all(|&f| self.orient(self.faces[f]) * self.sign[f] > 0.0)
    }

    fn valence(&self, v: usize) -> usize {
        self.incident[v].len()
    }

    fn neighbours(&self, v: usize) -> Vec<usize> {
        let mut out: Vec<usize> = self.incident[v]
            .iter()
            .flat_map(|&f| self.faces[f])
            .filter(|&u| u != v)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    fn edge_faces(&self, a: usize, b: usize) -> Vec<usize> {
        self.incident[a]
            .iter()
            .copied()
            .filter(|&f| self.faces[f].contains(&b))
            .collect()
    }

    fn detach(&mut self, f: usize) {
        for v in self.faces[f] {
            self.incident[v].retain(|&g| g != f);
        }
    }

    fn attach(&mut self, f: usize) {
        for v in self.faces[f] {
            self.incident[v].push(f);
        }
    }

    /// Flip edge a-b to c-d. By valence: when the four valences move toward
    /// six and no angle falls below the flip floor. By margin: when the pair's
    /// worst margin to the window improves -- the endgame, where single-vertex
    /// moves have stalled a fraction of a degree outside.
    fn flip_edge(&mut self, a: usize, b: usize, by_margin: bool) -> bool {
        if self.fixed[a] && self.fixed[b] {
            return false;
        }
        let shared = self.edge_faces(a, b);
        let [f1, f2] = match shared.as_slice() {
            [f1, f2] => [*f1, *f2],
            _ => return false,
        };
        let c = self.faces[f1].into_iter().find(|&v| v != a && v != b);
        let d = self.faces[f2].into_iter().find(|&v| v != a && v != b);
        let (Some(c), Some(d)) = (c, d) else {
            return false;
        };
        if c == d || !self.edge_faces(c, d).is_empty() {
            return false;
        }
        let dev = |valence: usize| (valence as f64 - 6.0).powi(2);
        let before = dev(self.valence(a))
            + dev(self.valence(b))
            + dev(self.valence(c))
            + dev(self.valence(d));
        let after = dev(self.valence(a) - 1)
            + dev(self.valence(b) - 1)
            + dev(self.valence(c) + 1)
            + dev(self.valence(d) + 1);
        // Sum-neutral flips count when they lower the worst valence: an 8 that
        // becomes a 7 while three sixes move one step is still progress.
        let spread = |values: [usize; 4]| {
            values
                .iter()
                .map(|&v| (v as i64 - 6).unsigned_abs())
                .max()
                .unwrap_or(0)
        };
        let [va, vb, vc, vd] = [a, b, c, d].map(|v| self.valence(v));
        let lowers_valence = after < before
            || (after == before
                && spread([va - 1, vb - 1, vc + 1, vd + 1]) < spread([va, vb, vc, vd]));
        if (!by_margin && !lowers_valence)
            || self.valence(a) <= 4
            || self.valence(b) <= 4
            || self.valence(c) + 1 > self.options.max_valence
            || self.valence(d) + 1 > self.options.max_valence
        {
            return false;
        }
        // Replace edge a-b by c-d, keeping each face's orientation sign.
        let mut new1 = [c, d, a];
        let mut new2 = [d, c, b];
        if self.orient(new1) * self.sign[f1] <= 0.0 {
            new1 = [d, c, a];
            new2 = [c, d, b];
        }
        if self.orient(new1) * self.sign[f1] <= 0.0 || self.orient(new2) * self.sign[f2] <= 0.0 {
            return false;
        }
        if by_margin {
            let before = self.score_corners([self.faces[f1], self.faces[f2]].into_iter());
            if !Self::better(self.score_corners([new1, new2].into_iter()), before) {
                return false;
            }
        } else {
            let floor = self.options.flip_floor_deg;
            if self
                .angles(new1)
                .iter()
                .chain(self.angles(new2).iter())
                .any(|&angle| angle < floor)
            {
                return false;
            }
        }
        self.detach(f1);
        self.detach(f2);
        self.faces[f1] = new1;
        self.faces[f2] = new2;
        self.attach(f1);
        self.attach(f2);
        if !self.levels.is_empty() {
            let deepest = self.levels[f1].max(self.levels[f2]);
            self.levels[f1] = deepest;
            self.levels[f2] = deepest;
        }
        true
    }

    fn try_positions(&mut self, v: usize, candidates: &[P]) -> bool {
        let faces = self.incident[v].clone();
        let current = self.score(&faces);
        let original = self.points[v];
        let mut best: Option<((f64, f64), P)> = None;
        for &p in candidates {
            self.points[v] = p;
            if !self.orientation_kept(&faces) {
                continue;
            }
            let s = self.score(&faces);
            if Self::better(s, current) && best.is_none_or(|(b, _)| Self::better(s, b)) {
                best = Some((s, p));
            }
        }
        self.points[v] = best.map_or(original, |(_, p)| p);
        best.is_some()
    }

    fn centroid_move(&mut self, v: usize) -> bool {
        if self.fixed[v] || self.needs_no_move(v) {
            return false;
        }
        let ring = self.neighbours(v);
        let mut centre = [0.0; 3];
        for u in &ring {
            for k in 0..3 {
                centre[k] += self.points[*u][k];
            }
        }
        let centre = unit(centre);
        let old = self.points[v];
        let candidates: Vec<P> = [1.0, 0.5, 0.25]
            .iter()
            .map(|&t| {
                unit([
                    old[0] * (1.0 - t) + centre[0] * t,
                    old[1] * (1.0 - t) + centre[1] * t,
                    old[2] * (1.0 - t) + centre[2] * t,
                ])
            })
            .collect();
        self.try_positions(v, &candidates)
    }

    fn pattern_move(&mut self, v: usize) -> bool {
        if self.fixed[v] || self.needs_no_move(v) {
            return false;
        }
        let ring = self.neighbours(v);
        if ring.is_empty() {
            return false;
        }
        let h = ring
            .iter()
            .map(|&u| norm(sub(self.points[u], self.points[v])))
            .sum::<f64>()
            / ring.len() as f64;
        let mut step = 0.2 * h;
        let mut moved = false;
        while step > 0.005 * h {
            let p = self.points[v];
            let axis = if p[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let e1 = unit(crossp(p, axis));
            let e2 = crossp(p, e1);
            let candidates: Vec<P> = (0..8)
                .map(|k| {
                    let theta = k as f64 * std::f64::consts::FRAC_PI_4;
                    let (s, c) = theta.sin_cos();
                    unit([
                        p[0] + step * (c * e1[0] + s * e2[0]),
                        p[1] + step * (c * e1[1] + s * e2[1]),
                        p[2] + step * (c * e1[2] + s * e2[2]),
                    ])
                })
                .collect();
            if self.try_positions(v, &candidates) {
                moved = true;
            } else {
                step *= 0.5;
            }
        }
        moved
    }

    /// The ring of `v` in the rotational order of its faces, if it is closed.
    fn ring(&self, v: usize) -> Option<Vec<usize>> {
        let mut next = std::collections::HashMap::new();
        for &f in &self.incident[v] {
            let corners = self.faces[f];
            let i = corners.iter().position(|&u| u == v)?;
            next.insert(corners[(i + 1) % 3], corners[(i + 2) % 3]);
        }
        let start = *next.keys().min()?;
        let mut ring = vec![start];
        loop {
            let following = *next.get(ring.last()?)?;
            if following == start {
                break;
            }
            if ring.len() > next.len() {
                return None;
            }
            ring.push(following);
        }
        (ring.len() == next.len()).then_some(ring)
    }

    fn remove_low_valence(&mut self, v: usize) -> bool {
        if self.fixed[v] || self.removed[v] || v < self.options.removable_from {
            return false;
        }
        let faces = self.incident[v].clone();
        if !(faces.len() == 3 || faces.len() == 4) {
            return false;
        }
        let Some(ring) = self.ring(v) else {
            return false;
        };
        let sign = self.sign[faces[0]];
        if faces.iter().any(|&f| self.sign[f] != sign) {
            return false;
        }
        let orient_ok = |w: &Self, f: [usize; 3]| w.orient(f) * sign > 0.0;
        let replacement: Vec<[usize; 3]> = if ring.len() == 3 {
            let f = [ring[0], ring[1], ring[2]];
            if !orient_ok(self, f) {
                return false;
            }
            vec![f]
        } else {
            let mut best: Option<(f64, Vec<[usize; 3]>)> = None;
            for d in 0..2 {
                let (a, b, c, e) = (ring[d], ring[d + 1], ring[(d + 2) % 4], ring[(d + 3) % 4]);
                if !self.edge_faces(a, c).is_empty() {
                    continue;
                }
                let pair = [[a, b, c], [a, c, e]];
                if !pair.iter().all(|&f| orient_ok(self, f)) {
                    continue;
                }
                let worst = pair
                    .iter()
                    .map(|&f| self.margin_of(f))
                    .fold(f64::INFINITY, f64::min);
                if best.as_ref().is_none_or(|(b, _)| worst > *b) {
                    best = Some((worst, pair.to_vec()));
                }
            }
            match best {
                Some((_, pair)) => pair,
                None => return false,
            }
        };
        let deepest = faces
            .iter()
            .map(|&f| self.levels.get(f).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);
        for &f in &faces {
            self.detach(f);
        }
        for (slot, &f) in faces.iter().enumerate() {
            if let Some(&corners) = replacement.get(slot) {
                self.faces[f] = corners;
                self.attach(f);
                if !self.levels.is_empty() {
                    self.levels[f] = deepest;
                }
            } else {
                self.alive[f] = false;
            }
        }
        self.removed[v] = true;
        true
    }

    fn live_faces(&self) -> impl Iterator<Item = usize> + '_ {
        (self.options.first_face..self.faces.len()).filter(|&f| self.alive[f])
    }

    fn angle_extremes(&self) -> (usize, f64, f64) {
        let mut outside = 0;
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for f in self.live_faces() {
            let a = self.angles(self.faces[f]);
            for x in a {
                lo = lo.min(x);
                hi = hi.max(x);
            }
            if self.outside(f) {
                outside += 1;
            }
        }
        (outside, lo, hi)
    }

    /// (largest, mean) |angle - 60| over every interior angle.
    fn deviation_stats(&self) -> (f64, f64) {
        let mut largest = 0.0_f64;
        let mut total = 0.0;
        let mut count = 0usize;
        for f in self.live_faces() {
            for angle in self.angles(self.faces[f]) {
                let deviation = (angle - 60.0).abs();
                largest = largest.max(deviation);
                total += deviation;
                count += 1;
            }
        }
        (
            largest,
            if count == 0 {
                0.0
            } else {
                total / count as f64
            },
        )
    }
}

/// Repair the triangulation in place.
///
/// `points` are unit vectors; `faces` index them. `face_levels` is either
/// empty or parallel to `faces`. Vertices on an open edge stay where they are.
/// Removed vertices and faces are compacted out at the end, keeping the order
/// of what remains, so every id below `options.removable_from` is unchanged.
pub fn repair_triangle_angle_window(
    points: &mut Vec<[f64; 3]>,
    faces: &mut Vec<[usize; 3]>,
    face_levels: &mut Vec<usize>,
    options: AngleWindowOptions,
) -> AngleWindowReport {
    repair_triangle_angle_window_traced(points, faces, face_levels, options).0
}

/// Where each row of the repaired mesh came from, so records kept per row
/// beside the mesh can follow it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AngleWindowOrigins {
    /// For each output face, the input face whose row it took over. A flip
    /// keeps both rows; a removed vertex's ring reuses the first of its rows.
    pub face_origin: Vec<usize>,
    /// For each output vertex, its input id.
    pub vertex_origin: Vec<usize>,
}

/// `repair_triangle_angle_window`, also reporting where every output row
/// came from.
pub fn repair_triangle_angle_window_traced(
    points: &mut Vec<[f64; 3]>,
    faces: &mut Vec<[usize; 3]>,
    face_levels: &mut Vec<usize>,
    options: AngleWindowOptions,
) -> (AngleWindowReport, AngleWindowOrigins) {
    assert!(face_levels.is_empty() || face_levels.len() == faces.len());
    let face_count = faces.len();
    let vertex_count = points.len();
    let mut work = Work {
        points,
        faces,
        levels: face_levels,
        alive: vec![true; face_count],
        sign: vec![0.0; face_count],
        incident: vec![Vec::new(); vertex_count],
        fixed: vec![false; vertex_count],
        removed: vec![false; vertex_count],
        options,
        mode: Mode::Window,
    };
    for f in 0..options.first_face.min(face_count) {
        work.alive[f] = false;
    }
    for f in options.first_face..face_count {
        work.sign[f] = work.orient(work.faces[f]).signum();
        work.attach(f);
    }
    for v in 0..options.first_vertex.min(vertex_count) {
        work.fixed[v] = true;
    }
    // Vertices on an open edge: a carved or regional mesh keeps its outline.
    let mut edge_count = std::collections::HashMap::new();
    for f in options.first_face..face_count {
        let [a, b, c] = work.faces[f];
        for (x, y) in [(a, b), (b, c), (c, a)] {
            *edge_count.entry((x.min(y), x.max(y))).or_insert(0usize) += 1;
        }
    }
    for ((x, y), n) in edge_count {
        if n != 2 {
            work.fixed[x] = true;
            work.fixed[y] = true;
        }
    }

    let (outside_before, min_before, max_before) = work.angle_extremes();
    let (max_deviation_before, mean_deviation_before) = work.deviation_stats();
    let topology = options.allow_topology_changes;
    let mut report = AngleWindowReport {
        outside_before,
        min_angle_before: min_before,
        max_angle_before: max_before,
        max_deviation_before,
        mean_deviation_before,
        ..AngleWindowReport::default()
    };

    let mut bad: Vec<usize> = work.live_faces().filter(|&f| work.outside(f)).collect();
    while !bad.is_empty() && report.rounds < options.max_rounds {
        report.rounds += 1;
        let mut region: Vec<usize> = bad.iter().flat_map(|&f| work.faces[f]).collect();
        region.sort_unstable();
        region.dedup();
        let mut widened: Vec<usize> = region
            .iter()
            .flat_map(|&v| work.neighbours(v))
            .chain(region.iter().copied())
            .collect();
        widened.sort_unstable();
        widened.dedup();

        let mut changed = 0;
        let mut edges: Vec<(usize, usize)> = widened
            .iter()
            .flat_map(|&v| work.incident[v].clone())
            .flat_map(|f| {
                let [a, b, c] = work.faces[f];
                [(a, b), (b, c), (c, a)]
            })
            .map(|(x, y)| (x.min(y), x.max(y)))
            .collect();
        edges.sort_unstable();
        edges.dedup();
        if !topology {
            edges.clear();
        }
        for (a, b) in edges {
            if work.flip_edge(a, b, false) {
                report.flips += 1;
                changed += 1;
            }
        }
        for &v in widened.iter().filter(|_| topology) {
            if work.valence(v) <= 4 && work.remove_low_valence(v) {
                report.removed_vertices += 1;
                changed += 1;
            }
        }
        let live: Vec<usize> = widened
            .iter()
            .copied()
            .filter(|&v| !work.removed[v])
            .collect();
        for _ in 0..3 {
            for &v in &live {
                if work.centroid_move(v) {
                    report.moves += 1;
                    changed += 1;
                }
            }
        }
        for _ in 0..2 {
            for &v in &live {
                if work.pattern_move(v) {
                    report.moves += 1;
                    changed += 1;
                }
            }
        }
        let mut bad_edges: Vec<(usize, usize)> = work
            .live_faces()
            .filter(|&f| work.outside(f))
            .flat_map(|f| {
                let [a, b, c] = work.faces[f];
                [(a, b), (b, c), (c, a)]
            })
            .map(|(x, y)| (x.min(y), x.max(y)))
            .collect();
        bad_edges.sort_unstable();
        bad_edges.dedup();
        if !topology {
            bad_edges.clear();
        }
        for (a, b) in bad_edges {
            if work.flip_edge(a, b, true) {
                report.flips += 1;
                changed += 1;
            }
        }
        bad = work.live_faces().filter(|&f| work.outside(f)).collect();
        if changed == 0 {
            break;
        }
    }

    // Second phase: the window is the pass line, not the goal. Score against
    // (60, 60) and keep improving the worst angle around each vertex. A move
    // or flip is taken only when the worst local deviation does not grow, and
    // the window is symmetric about 60, so no triangle inside it is pushed out.
    report.mean_deviation_window_phase = work.deviation_stats().1;
    let window = options.window_deg;
    for phase in 0..2 {
        work.mode = if phase == 0 {
            Mode::Minimax
        } else {
            // Tighten the window to the worst deviation the minimax pass left,
            // clamped to the contract so no triangle is pushed out of it.
            let reached = work.deviation_stats().0;
            let cap = (
                (60.0 - reached).max(window.0).min(60.0),
                (60.0 + reached).min(window.1).max(60.0),
            );
            Mode::Energy { cap }
        };
        let mut rounds = 0;
        while rounds < options.equilateral_rounds {
            let tolerance = options.equilateral_tolerance_deg;
            let rough: Vec<usize> = work
                .live_faces()
                .filter(|&f| work.deviation(f) > tolerance)
                .collect();
            if rough.is_empty() {
                break;
            }
            rounds += 1;
            report.equilateral_rounds += 1;
            let mut changed = 0usize;
            if topology {
                let mut rough_edges: Vec<(usize, usize)> = rough
                    .iter()
                    .flat_map(|&f| {
                        let [a, b, c] = work.faces[f];
                        [(a, b), (b, c), (c, a)]
                    })
                    .map(|(x, y)| (x.min(y), x.max(y)))
                    .collect();
                rough_edges.sort_unstable();
                rough_edges.dedup();
                for (a, b) in rough_edges {
                    if work.flip_edge(a, b, true) {
                        report.flips += 1;
                        changed += 1;
                    }
                }
            }
            let mut vertices: Vec<usize> = rough
                .iter()
                .flat_map(|&f| work.faces[f])
                .filter(|&v| !work.fixed[v] && !work.removed[v])
                .collect();
            vertices.sort_unstable();
            vertices.dedup();
            for &v in &vertices {
                if work.centroid_move(v) {
                    report.moves += 1;
                    changed += 1;
                }
            }
            for &v in &vertices {
                if work.pattern_move(v) {
                    report.moves += 1;
                    changed += 1;
                }
            }
            // Diminishing returns: stop once a round changes under 1% of the
            // vertices it looked at.
            if changed * 100 < vertices.len() {
                break;
            }
        }
    }

    let (outside_after, min_after, max_after) = work.angle_extremes();
    let (max_deviation_after, mean_deviation_after) = work.deviation_stats();
    report.max_deviation_after = max_deviation_after;
    report.mean_deviation_after = mean_deviation_after;
    report.outside_after = outside_after;
    report.min_angle_after = min_after;
    report.max_angle_after = max_after;

    // Compact: drop removed vertices and dead faces, keeping order.
    let alive = std::mem::take(&mut work.alive);
    let removed = std::mem::take(&mut work.removed);
    drop(work);
    let mut origins = AngleWindowOrigins {
        face_origin: (0..face_count).collect(),
        vertex_origin: (0..vertex_count).collect(),
    };
    if report.removed_vertices > 0 {
        let mut new_id = vec![usize::MAX; vertex_count];
        let mut next = 0;
        for v in 0..vertex_count {
            if !removed[v] {
                new_id[v] = next;
                next += 1;
            }
        }
        let mut kept_points = Vec::with_capacity(next);
        for v in 0..vertex_count {
            if !removed[v] {
                kept_points.push(points[v]);
            }
        }
        *points = kept_points;
        origins.vertex_origin = (0..vertex_count).filter(|&v| !removed[v]).collect();
        origins.face_origin = (0..face_count)
            .filter(|&f| f < options.first_face || alive[f])
            .collect();
        let mut kept_faces = Vec::with_capacity(face_count);
        let mut kept_levels = Vec::with_capacity(face_levels.len());
        for f in 0..face_count {
            if f < options.first_face {
                kept_faces.push(faces[f]);
            } else if alive[f] {
                kept_faces.push(faces[f].map(|v| new_id[v]));
            } else {
                continue;
            }
            if !face_levels.is_empty() {
                kept_levels.push(face_levels[f]);
            }
        }
        *faces = kept_faces;
        *face_levels = kept_levels;
    }
    (report, origins)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lonlat(lon: f64, lat: f64) -> P {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    }

    /// A small hexagonal fan with its centre pulled toward one side, so the
    /// triangles on that side are squeezed below 35 degrees.
    fn skewed_fan() -> (Vec<P>, Vec<[usize; 3]>) {
        let mut points = vec![lonlat(0.6, 0.1)];
        for k in 0..6 {
            let theta = (k as f64 * 60.0_f64).to_radians();
            points.push(lonlat(theta.cos(), theta.sin()));
        }
        let faces = (0..6).map(|k| [0, 1 + k, 1 + (k + 1) % 6]).collect();
        (points, faces)
    }

    #[test]
    fn angles_match_the_equilateral_limit() {
        let a = spherical_triangle_angles_deg([
            lonlat(0.0, 0.0),
            lonlat(0.01, 0.0),
            lonlat(0.005, 0.01 * 3f64.sqrt() / 2.0),
        ]);
        for x in a {
            assert!((x - 60.0).abs() < 1.0e-3, "{a:?}");
        }
    }

    #[test]
    fn a_squeezed_interior_vertex_is_moved_into_the_window() {
        let (mut points, mut faces) = skewed_fan();
        let mut levels = vec![1, 2, 1, 1, 1, 1];
        let report = repair_triangle_angle_window(
            &mut points,
            &mut faces,
            &mut levels,
            AngleWindowOptions::new((35.0, 85.0)),
        );
        assert!(report.outside_before > 0, "{report:?}");
        assert_eq!(report.outside_after, 0, "{report:?}");
        assert!(report.min_angle_after >= 35.0 && report.max_angle_after <= 85.0);
        // The outline is an open boundary and must not move.
        assert_eq!(points[1], lonlat(1.0, 0.0));
        assert_eq!(levels, vec![1, 2, 1, 1, 1, 1]);
    }

    #[test]
    fn a_valence_four_vertex_is_removed_and_its_levels_merged() {
        // Four triangles around a centre average 90 degrees, so no position
        // brings them into the window; the centre has to go.
        let mut points = vec![
            lonlat(0.0, 0.0),
            lonlat(1.0, 0.0),
            lonlat(0.0, 1.0),
            lonlat(-1.0, 0.0),
            lonlat(0.0, -1.0),
        ];
        let mut faces = vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]];
        let mut levels = vec![0, 3, 0, 1];
        let mut options = AngleWindowOptions::new((35.0, 85.0));
        options.max_rounds = 1;
        let report = repair_triangle_angle_window(&mut points, &mut faces, &mut levels, options);
        assert_eq!(report.removed_vertices, 1, "{report:?}");
        assert_eq!(points.len(), 4);
        assert_eq!(faces.len(), 2);
        assert_eq!(levels, vec![3, 3]);
        for f in &faces {
            assert!(f.iter().all(|&v| v < 4));
        }
        // Ids above the removed vertex shift down by one.
        assert_eq!(points[0], lonlat(1.0, 0.0));
    }

    /// A hexagonal fan whose centre sits off-centre enough to leave its
    /// triangles inside the window but well away from 60 degrees.
    fn lopsided_fan() -> (Vec<P>, Vec<[usize; 3]>) {
        let mut points = vec![lonlat(0.25, 0.05)];
        for k in 0..6 {
            let theta = (k as f64 * 60.0_f64).to_radians();
            points.push(lonlat(theta.cos(), theta.sin()));
        }
        let faces = (0..6).map(|k| [0, 1 + k, 1 + (k + 1) % 6]).collect();
        (points, faces)
    }

    #[test]
    fn inside_the_window_angles_keep_moving_toward_sixty() {
        let (mut points, mut faces) = lopsided_fan();
        let report = repair_triangle_angle_window(
            &mut points,
            &mut faces,
            &mut Vec::new(),
            AngleWindowOptions::new((35.0, 85.0)),
        );
        assert_eq!(report.outside_before, 0, "{report:?}");
        assert!(report.max_deviation_before > 10.0, "{report:?}");
        assert!(report.equilateral_rounds >= 1, "{report:?}");
        assert!(
            report.max_deviation_after < 1.0,
            "a regular hexagon's centre is reachable: {report:?}"
        );
        assert_eq!(report.outside_after, 0);

        // With the second phase off, the same mesh is left as it came.
        let (mut points, mut faces) = lopsided_fan();
        let mut options = AngleWindowOptions::new((35.0, 85.0));
        options.equilateral_rounds = 0;
        let report =
            repair_triangle_angle_window(&mut points, &mut faces, &mut Vec::new(), options);
        assert_eq!(report.moves, 0, "{report:?}");
    }

    #[test]
    fn without_topology_changes_only_vertices_move() {
        let mut points = vec![
            lonlat(0.0, 0.0),
            lonlat(1.0, 0.0),
            lonlat(0.0, 1.0),
            lonlat(-1.0, 0.0),
            lonlat(0.0, -1.0),
        ];
        let mut faces = vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]];
        let before = faces.clone();
        let mut options = AngleWindowOptions::new((35.0, 85.0));
        options.allow_topology_changes = false;
        let (report, origins) =
            repair_triangle_angle_window_traced(&mut points, &mut faces, &mut Vec::new(), options);
        assert_eq!(report.removed_vertices, 0);
        assert_eq!(report.flips, 0);
        assert_eq!(faces, before);
        assert_eq!(origins.vertex_origin, vec![0, 1, 2, 3, 4]);
        assert_eq!(origins.face_origin, vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_removed_vertex_is_traced_out_of_the_rows() {
        let mut points = vec![
            lonlat(0.0, 0.0),
            lonlat(1.0, 0.0),
            lonlat(0.0, 1.0),
            lonlat(-1.0, 0.0),
            lonlat(0.0, -1.0),
        ];
        let mut faces = vec![[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]];
        let mut options = AngleWindowOptions::new((35.0, 85.0));
        options.max_rounds = 1;
        let (report, origins) =
            repair_triangle_angle_window_traced(&mut points, &mut faces, &mut Vec::new(), options);
        assert_eq!(report.removed_vertices, 1);
        assert_eq!(origins.vertex_origin, vec![1, 2, 3, 4]);
        assert_eq!(origins.face_origin.len(), faces.len());
        assert_eq!(faces.len(), 2);
    }
}
