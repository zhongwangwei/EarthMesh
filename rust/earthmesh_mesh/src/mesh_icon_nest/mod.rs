//! Plan ICON nested domains over a global triangle grid.
//!
//! A closed ICON grid cannot be refined in place: every local refinement adds
//! degree-5/degree-7 vertex pairs and ICON allows at most six edges per vertex
//! (guide 11.76). ICON's own answer is nesting: the global grid stays as it
//! is, and each finer region is a separate grid whose triangles are the
//! parent's triangles split 1->4. Every grid in the set is then a plain
//! bisection of its parent, so no vertex exceeds degree six.
//!
//! What ICON requires of a nest (read from the ICON source; guide 11.86):
//! - it is made of whole parent triangles, each split into exactly four;
//! - its lateral boundary rows are flagged at least `nudge_zone_width + 4`
//!   (12 by default) deep -- the ICON grid generator flags 14;
//! - sibling nests share no parent triangle.
//!
//! The planner only decides geometry and topology. Numbering, metrics and the
//! file format belong to the ICON writer.

use std::collections::{BTreeMap, VecDeque};

/// A unit vector.
pub type P = [f64; 3];

/// Where a nest vertex comes from, in the parent domain's point ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NestVertexOrigin {
    /// A vertex of the global grid (the global domain only).
    Base,
    /// A parent vertex kept by the bisection.
    Parent(usize),
    /// The midpoint of the parent edge between two parent vertices (`a < b`).
    Midpoint(usize, usize),
}

/// One ICON domain: the global grid or a nest.
#[derive(Clone, Debug)]
pub struct IconNestDomain {
    /// 1-based ICON domain id; the global grid is 1.
    pub id: usize,
    /// Domain id of the parent; 0 for the global grid.
    pub parent: usize,
    /// Bisections below the global grid (0 for the global grid).
    pub depth: u32,
    pub points: Vec<P>,
    /// Triangles, counter-clockwise seen from outside the sphere.
    pub triangles: Vec<[usize; 3]>,
    /// The parent triangle each triangle came from (empty for the global grid).
    pub parent_triangle: Vec<usize>,
    pub vertex_origin: Vec<NestVertexOrigin>,
    /// Distance of each vertex from the lateral boundary, in vertex rows:
    /// 1 on the boundary. `u32::MAX` for the global grid, which has none.
    pub vertex_row: Vec<u32>,
    /// Lateral boundary row of each triangle: the least row of its vertices.
    pub cell_row: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct IconNestOptions {
    /// Boundary rows ICON indexes in a nest (`bdy_indexing_depth`); a child
    /// is never placed within this many rows of its parent's boundary.
    pub boundary_depth: u32,
    /// Rows of a nest's boundary zone -- lateral interpolation plus nudging --
    /// kept clear of the demand the nest serves.
    pub boundary_zone: u32,
    /// Most domains ICON accepts, the global one included (`max_dom`).
    pub max_domains: usize,
}

impl Default for IconNestOptions {
    fn default() -> Self {
        Self {
            boundary_depth: 14,
            boundary_zone: 12,
            max_domains: 10,
        }
    }
}

fn unit(a: P) -> P {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / n, a[1] / n, a[2] / n]
}

fn centroid(points: &[P], t: [usize; 3]) -> P {
    let [a, b, c] = t.map(|i| points[i]);
    unit([a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]])
}

/// The target level of a triangle: the deepest of its centroid and corners,
/// so a demand smaller than the triangle is not stepped over.
fn triangle_target(points: &[P], t: [usize; 3], target: &impl Fn(P) -> u32) -> u32 {
    t.iter()
        .map(|&i| target(points[i]))
        .chain(std::iter::once(target(centroid(points, t))))
        .max()
        .unwrap_or(0)
}

/// Triangles incident to each vertex.
fn vertex_triangles(point_count: usize, triangles: &[[usize; 3]]) -> Vec<Vec<usize>> {
    let mut incident = vec![Vec::new(); point_count];
    for (t, tri) in triangles.iter().enumerate() {
        for &v in tri {
            incident[v].push(t);
        }
    }
    incident
}

/// Add every triangle sharing a vertex with `region`, `rows` times, keeping
/// only triangles `allowed` admits.
fn dilate(
    region: &mut [bool],
    rows: u32,
    triangles: &[[usize; 3]],
    incident: &[Vec<usize>],
    allowed: &[bool],
) {
    for _ in 0..rows {
        let grown = region
            .iter()
            .enumerate()
            .filter(|(_, &inside)| inside)
            .flat_map(|(t, _)| {
                triangles[t]
                    .iter()
                    .flat_map(|&v| incident[v].iter().copied())
            })
            .filter(|&n| allowed[n] && !region[n])
            .collect::<Vec<_>>();
        if grown.is_empty() {
            return;
        }
        for t in grown {
            region[t] = true;
        }
    }
}

/// Components of the triangles in `region`, two triangles joined when they
/// share a vertex.
fn components(
    region: &[bool],
    triangles: &[[usize; 3]],
    incident: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut label = vec![usize::MAX; region.len()];
    let mut out = Vec::new();
    for seed in 0..region.len() {
        if !region[seed] || label[seed] != usize::MAX {
            continue;
        }
        let id = out.len();
        let mut members = vec![seed];
        label[seed] = id;
        let mut queue = VecDeque::from([seed]);
        while let Some(t) = queue.pop_front() {
            for &v in &triangles[t] {
                for &n in &incident[v] {
                    if region[n] && label[n] == usize::MAX {
                        label[n] = id;
                        members.push(n);
                        queue.push_back(n);
                    }
                }
            }
        }
        members.sort_unstable();
        out.push(members);
    }
    out
}

/// Fill holes: every part of the complement that is not the largest one is
/// enclosed by the region and joins it (restricted to `allowed`).
fn fill_holes(
    region: &mut [bool],
    triangles: &[[usize; 3]],
    incident: &[Vec<usize>],
    allowed: &[bool],
) {
    let outside = region.iter().map(|inside| !inside).collect::<Vec<_>>();
    let mut parts = components(&outside, triangles, incident);
    if parts.len() < 2 {
        return;
    }
    parts.sort_by_key(|part| std::cmp::Reverse(part.len()));
    for part in parts.into_iter().skip(1) {
        if part.iter().all(|&t| allowed[t]) {
            for t in part {
                region[t] = true;
            }
        }
    }
}

/// Close pinched vertices: where the region's triangles around a vertex fall
/// into more than one fan, the boundary would touch itself there. Take the
/// whole fan instead, so every boundary vertex has one run of triangles.
fn close_pinches(
    region: &mut [bool],
    triangles: &[[usize; 3]],
    incident: &[Vec<usize>],
    allowed: &[bool],
) {
    loop {
        let mut added = false;
        for (v, around) in incident.iter().enumerate() {
            let inside = around.iter().filter(|&&t| region[t]).count();
            if inside == 0 || inside == around.len() {
                continue;
            }
            if fan_runs(v, around, triangles, region) > 1 && around.iter().all(|&t| allowed[t]) {
                for &t in around {
                    region[t] = true;
                }
                added = true;
            }
        }
        if !added {
            return;
        }
    }
}

/// Runs of consecutive region triangles around `v`, walking its fan through
/// shared edges.
fn fan_runs(v: usize, around: &[usize], triangles: &[[usize; 3]], region: &[bool]) -> usize {
    // Two triangles of the fan are consecutive when they share an edge at v.
    let others = |t: usize| {
        let tri = triangles[t];
        let k = tri
            .iter()
            .position(|&x| x == v)
            .expect("incident triangle holds v");
        [tri[(k + 1) % 3], tri[(k + 2) % 3]]
    };
    let inside = around
        .iter()
        .copied()
        .filter(|&t| region[t])
        .collect::<Vec<_>>();
    let mut seen = vec![false; inside.len()];
    let mut runs = 0;
    for start in 0..inside.len() {
        if seen[start] {
            continue;
        }
        runs += 1;
        seen[start] = true;
        let mut stack = vec![start];
        while let Some(i) = stack.pop() {
            let [a, b] = others(inside[i]);
            for (j, &u) in inside.iter().enumerate() {
                if !seen[j] {
                    let [c, d] = others(u);
                    if a == c || a == d || b == c || b == d {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
    }
    runs
}

/// Vertex rows from the lateral boundary (1 on it) and the cell rows they give.
fn boundary_rows(point_count: usize, triangles: &[[usize; 3]]) -> (Vec<u32>, Vec<u32>) {
    let mut edge_uses = BTreeMap::<(usize, usize), u32>::new();
    let mut neighbours = vec![Vec::new(); point_count];
    for tri in triangles {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            *edge_uses.entry((a.min(b), a.max(b))).or_default() += 1;
            neighbours[a].push(b);
        }
    }
    let mut row = vec![u32::MAX; point_count];
    let mut queue = VecDeque::new();
    for (&(a, b), &uses) in &edge_uses {
        if uses == 1 {
            for v in [a, b] {
                if row[v] == u32::MAX {
                    row[v] = 1;
                    queue.push_back(v);
                }
            }
        }
    }
    while let Some(v) = queue.pop_front() {
        for &n in &neighbours[v] {
            if row[n] == u32::MAX {
                row[n] = row[v] + 1;
                queue.push_back(n);
            }
        }
    }
    let cells = triangles
        .iter()
        .map(|tri| tri.iter().map(|&v| row[v]).min().unwrap_or(u32::MAX))
        .collect();
    (row, cells)
}

/// Split the parent triangles `members` of `parent` 1->4 into a nest.
fn bisect(parent: &IconNestDomain, members: &[usize], id: usize) -> IconNestDomain {
    let mut points = Vec::new();
    let mut origin = Vec::new();
    let mut kept = BTreeMap::<usize, usize>::new();
    let mut mids = BTreeMap::<(usize, usize), usize>::new();
    let mut triangles = Vec::with_capacity(members.len() * 4);
    let mut parent_triangle = Vec::with_capacity(members.len() * 4);
    for &t in members {
        let [a, b, c] = parent.triangles[t];
        let mut vertex = |v: usize, points: &mut Vec<P>, origin: &mut Vec<NestVertexOrigin>| {
            *kept.entry(v).or_insert_with(|| {
                points.push(parent.points[v]);
                origin.push(NestVertexOrigin::Parent(v));
                points.len() - 1
            })
        };
        let (a2, b2, c2) = (
            vertex(a, &mut points, &mut origin),
            vertex(b, &mut points, &mut origin),
            vertex(c, &mut points, &mut origin),
        );
        let mut mid =
            |u: usize, w: usize, points: &mut Vec<P>, origin: &mut Vec<NestVertexOrigin>| {
                let key = (u.min(w), u.max(w));
                *mids.entry(key).or_insert_with(|| {
                    let (p, q) = (parent.points[u], parent.points[w]);
                    points.push(unit([p[0] + q[0], p[1] + q[1], p[2] + q[2]]));
                    origin.push(NestVertexOrigin::Midpoint(key.0, key.1));
                    points.len() - 1
                })
            };
        let ab = mid(a, b, &mut points, &mut origin);
        let bc = mid(b, c, &mut points, &mut origin);
        let ca = mid(c, a, &mut points, &mut origin);
        for tri in [[a2, ab, ca], [ab, b2, bc], [ca, bc, c2], [ab, bc, ca]] {
            triangles.push(tri);
            parent_triangle.push(t);
        }
    }
    let (vertex_row, cell_row) = boundary_rows(points.len(), &triangles);
    IconNestDomain {
        id,
        parent: parent.id,
        depth: parent.depth + 1,
        points,
        triangles,
        parent_triangle,
        vertex_origin: origin,
        vertex_row,
        cell_row,
    }
}

/// Rows of margin, in the rows of the domain a level-`level` nest is cut
/// from, that keep the level's demand clear of the nest's boundary zone and
/// leave room for the deeper nests inside it.
fn margin_rows(level: u32, deepest: u32, options: &IconNestOptions) -> u32 {
    // Child rows the demand of this level must sit inside the nest.
    let child_rows = if level >= deepest {
        options.boundary_zone.max(options.boundary_depth)
    } else {
        options.boundary_depth + margin_rows(level + 1, deepest, options)
    };
    child_rows.div_ceil(2)
}

/// Plan the global domain plus nests serving `target` (a level per point).
///
/// Nest level `k` holds every triangle of its parent whose target is at
/// least `k`, widened by `margin_rows`, kept outside the parent's own
/// boundary indexing depth, with holes and pinched vertices closed. Each
/// vertex-connected part is its own sibling nest.
pub fn plan_icon_nests(
    points: Vec<P>,
    triangles: Vec<[usize; 3]>,
    target: impl Fn(P) -> u32,
    options: &IconNestOptions,
) -> Result<Vec<IconNestDomain>, String> {
    let deepest = triangles
        .iter()
        .map(|&t| triangle_target(&points, t, &target))
        .max()
        .unwrap_or(0);
    let global = IconNestDomain {
        id: 1,
        parent: 0,
        depth: 0,
        vertex_origin: vec![NestVertexOrigin::Base; points.len()],
        vertex_row: vec![u32::MAX; points.len()],
        cell_row: vec![u32::MAX; triangles.len()],
        parent_triangle: Vec::new(),
        points,
        triangles,
    };
    let mut domains = vec![global];
    let mut frontier = vec![0usize];
    for level in 1..=deepest {
        let margin = margin_rows(level, deepest, options);
        let mut next = Vec::new();
        for &index in &frontier {
            let parent = &domains[index];
            let incident = vertex_triangles(parent.points.len(), &parent.triangles);
            // A nest is never cut within its parent's boundary indexing depth.
            let allowed = parent
                .cell_row
                .iter()
                .map(|&row| row > options.boundary_depth)
                .collect::<Vec<_>>();
            let mut region = parent
                .triangles
                .iter()
                .enumerate()
                .map(|(t, &tri)| {
                    allowed[t] && triangle_target(&parent.points, tri, &target) >= level
                })
                .collect::<Vec<_>>();
            if !region.iter().any(|&inside| inside) {
                continue;
            }
            dilate(&mut region, margin, &parent.triangles, &incident, &allowed);
            fill_holes(&mut region, &parent.triangles, &incident, &allowed);
            close_pinches(&mut region, &parent.triangles, &incident, &allowed);
            for members in components(&region, &parent.triangles, &incident) {
                let id = domains.len() + next.len() + 1;
                next.push(bisect(&domains[index], &members, id));
            }
        }
        if next.is_empty() {
            break;
        }
        if domains.len() + next.len() > options.max_domains {
            return Err(format!(
                "ICON nesting needs {} domains by level {level}, more than ICON's {} (max_dom); \
                 merge nearby demands or lower the deepest level",
                domains.len() + next.len(),
                options.max_domains
            ));
        }
        frontier = (domains.len()..domains.len() + next.len()).collect();
        for nest in &next {
            let deepest_row = nest.cell_row.iter().copied().max().unwrap_or(0);
            if deepest_row < options.boundary_zone {
                return Err(format!(
                    "ICON nest {} (level {level}) is only {deepest_row} cell rows deep; ICON needs {} \
                     for its boundary zone -- its parent leaves too little interior around the demand",
                    nest.id, options.boundary_zone
                ));
            }
        }
        domains.extend(next);
    }
    Ok(domains)
}

#[cfg(test)]
mod tests;
