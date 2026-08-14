//! A triangulation that belongs to no backend.
//!
//! `TriangularMesh` is the type every backend consumes and it is Method-C's:
//! `mrlm`, `mrow`, `ngr`, `impent` and the transition rows are its nesting
//! bookkeeping, meaningless to red-green and to HARP-DV. That is why Method-C
//! has never been lifted into its own crate -- moving it would leave the mesh
//! crate depending on the backend, with the dependency arrow pointing the wrong
//! way.
//!
//! `MeshState` is the part that is common: sites, the triangles over them, and
//! which triangle lies across each edge. Nothing here knows what a generation
//! or a transition row is.
//!
//! # Indexing
//!
//! Canonical one-based, slots 0 and 1 reserved, like everything else in the
//! repository. A neutral type is a tempting place to drop that convention, and
//! dropping it would move the bugs into the conversion rather than remove them:
//! every writer, reader and test around this type counts from one.

use std::collections::BTreeMap;
use std::io;

use crate::{CartesianPoint, TriangularMesh};

/// The first id that names a real entity. Slot 0 is unused and slot 1 is the
/// canonical placeholder.
pub const MESH_STATE_FIRST_ID: usize = 2;

/// Stable site id. The slot keeps raw table compatibility; the generation
/// invalidates ids whose slot was created, removed, then created again.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct VertexId {
    pub slot: usize,
    pub generation: u64,
}

/// Stable triangle id. Reusing a triangle slot for a new triangle bumps the
/// generation, so ids held before an insertion do not silently name new faces.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct FaceId {
    pub slot: usize,
    pub generation: u64,
}

/// Stable canonical edge id, independent of incident face winding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EdgeId {
    pub vertices: [VertexId; 2],
}

impl EdgeId {
    pub fn new(a: VertexId, b: VertexId) -> Self {
        if a <= b {
            Self { vertices: [a, b] }
        } else {
            Self { vertices: [b, a] }
        }
    }
}

/// Sites, triangles, and what is across each edge.
#[derive(Clone, Debug)]
pub struct MeshState {
    vertices: Vec<CartesianPoint>,
    triangles: Vec<[usize; 3]>,
    /// `neighbours[t][i]` is the triangle across the edge opposite corner `i`
    /// of triangle `t`, or zero where no triangle is.
    neighbours: Vec<[usize; 3]>,
    vertex_generations: Vec<u64>,
    triangle_generations: Vec<u64>,
    vertex_live: Vec<bool>,
    triangle_live: Vec<bool>,
    next_vertex_generation: u64,
    next_triangle_generation: u64,
}

impl PartialEq for MeshState {
    fn eq(&self, other: &Self) -> bool {
        self.vertices == other.vertices
            && self.triangles == other.triangles
            && self.neighbours == other.neighbours
            && self.vertex_generations == other.vertex_generations
            && self.triangle_generations == other.triangle_generations
            && self.vertex_live == other.vertex_live
            && self.triangle_live == other.triangle_live
    }
}

/// What is wrong with a triangulation, named precisely enough to act on.
#[derive(Clone, Debug, PartialEq)]
pub enum MeshStateError {
    /// A triangle names a vertex the state does not carry.
    UnknownVertex { triangle: usize, vertex: usize },
    /// A triangle names one vertex twice, so it encloses nothing.
    DegenerateTriangle {
        triangle: usize,
        corners: [usize; 3],
    },
    /// Three or more triangles claim one edge, which no surface allows.
    NonManifoldEdge {
        vertices: (usize, usize),
        triangles: usize,
    },
    /// One triangle names another as its neighbour and is not named back.
    AsymmetricNeighbour { triangle: usize, neighbour: usize },
}

impl std::fmt::Display for MeshStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVertex { triangle, vertex } => write!(
                formatter,
                "triangle {triangle} names vertex {vertex}, which the mesh does not carry"
            ),
            Self::DegenerateTriangle { triangle, corners } => write!(
                formatter,
                "triangle {triangle} has corners {corners:?} and so encloses nothing"
            ),
            Self::NonManifoldEdge {
                vertices,
                triangles,
            } => write!(
                formatter,
                "edge {vertices:?} is claimed by {triangles} triangles; a surface allows two"
            ),
            Self::AsymmetricNeighbour {
                triangle,
                neighbour,
            } => write!(
                formatter,
                "triangle {triangle} names {neighbour} across an edge, and is not named back"
            ),
        }
    }
}

impl std::error::Error for MeshStateError {}

/// The two corners of an edge, smaller id first, so the same edge from either
/// side is the same key.
fn edge_key(left: usize, right: usize) -> (usize, usize) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn valid_vertex_slot(slot: usize, vertices_len: usize) -> bool {
    (MESH_STATE_FIRST_ID..vertices_len).contains(&slot)
}

impl MeshState {
    /// Build from vertices and triangles, deriving adjacency.
    ///
    /// Both slices are one-based with slots 0 and 1 reserved, so a caller
    /// passing dense zero-based arrays gets an error rather than a mesh shifted
    /// by two.
    pub fn from_parts(
        vertices: Vec<CartesianPoint>,
        triangles: Vec<[usize; 3]>,
    ) -> Result<Self, Vec<MeshStateError>> {
        let mut errors = Vec::new();
        for (triangle, corners) in triangles.iter().enumerate().skip(MESH_STATE_FIRST_ID) {
            for &corner in corners {
                if !valid_vertex_slot(corner, vertices.len()) {
                    errors.push(MeshStateError::UnknownVertex {
                        triangle,
                        vertex: corner,
                    });
                }
            }
            if corners[0] == corners[1] || corners[1] == corners[2] || corners[0] == corners[2] {
                errors.push(MeshStateError::DegenerateTriangle {
                    triangle,
                    corners: *corners,
                });
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        // One pass to collect who claims each edge, one to write the opposites.
        let mut claims: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (triangle, corners) in triangles.iter().enumerate().skip(MESH_STATE_FIRST_ID) {
            for corner in 0..3 {
                let key = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                claims.entry(key).or_default().push(triangle);
            }
        }
        for (vertices_of_edge, claimants) in &claims {
            if claimants.len() > 2 {
                errors.push(MeshStateError::NonManifoldEdge {
                    vertices: *vertices_of_edge,
                    triangles: claimants.len(),
                });
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let mut neighbours = vec![[0usize; 3]; triangles.len()];
        for (triangle, corners) in triangles.iter().enumerate().skip(MESH_STATE_FIRST_ID) {
            for corner in 0..3 {
                let key = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                let opposite = claims
                    .get(&key)
                    .and_then(|claimants| {
                        claimants.iter().copied().find(|&other| other != triangle)
                    })
                    .unwrap_or(0);
                neighbours[triangle][corner] = opposite;
            }
        }

        let vertex_generations = vec![0; vertices.len()];
        let triangle_generations = vec![0; triangles.len()];
        let mut vertex_live = vec![false; vertices.len()];
        vertex_live[MESH_STATE_FIRST_ID..].fill(true);
        let mut triangle_live = vec![false; triangles.len()];
        triangle_live[MESH_STATE_FIRST_ID..].fill(true);
        Ok(Self {
            vertices,
            triangles,
            neighbours,
            vertex_generations,
            triangle_generations,
            vertex_live,
            triangle_live,
            next_vertex_generation: 1,
            next_triangle_generation: 1,
        })
    }

    /// Take the neutral part of a Method-C mesh.
    ///
    /// The M points are the sites and each W face's `im` is a triangle over
    /// them. Everything else the face carries -- generation, transition row,
    /// grid number -- stays behind, which is the point of this type.
    pub fn from_triangular_mesh(mesh: &TriangularMesh) -> io::Result<Self> {
        if mesh.nmd < MESH_STATE_FIRST_ID || mesh.nwd < MESH_STATE_FIRST_ID {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "a mesh with {} points and {} faces carries no triangulation",
                    mesh.nmd, mesh.nwd
                ),
            ));
        }
        let required_points = mesh.nmd + 1;
        if mesh.m_points.len() < required_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "m_points has {} rows but nmd={} requires at least {required_points}",
                    mesh.m_points.len(),
                    mesh.nmd
                ),
            ));
        }
        let required_faces = mesh.nwd + 1;
        if mesh.w_faces.len() < required_faces {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "w_faces has {} rows but nwd={} requires at least {required_faces}",
                    mesh.w_faces.len(),
                    mesh.nwd
                ),
            ));
        }

        let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); required_points];
        vertices[MESH_STATE_FIRST_ID..=mesh.nmd]
            .clone_from_slice(&mesh.m_points[MESH_STATE_FIRST_ID..=mesh.nmd]);
        let mut triangles = vec![[1usize; 3]; required_faces];
        for iw in MESH_STATE_FIRST_ID..=mesh.nwd {
            triangles[iw] = mesh.w_faces[iw].im;
        }
        Self::from_parts(vertices, triangles).map_err(|errors| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "the mesh does not convert to a neutral triangulation: {}",
                    errors
                        .iter()
                        .take(4)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )
        })
    }

    pub fn vertices(&self) -> &[CartesianPoint] {
        &self.vertices
    }

    pub fn triangles(&self) -> &[[usize; 3]] {
        &self.triangles
    }

    pub fn neighbours(&self) -> &[[usize; 3]] {
        &self.neighbours
    }

    pub fn vertex_id(&self, slot: usize) -> Option<VertexId> {
        self.is_vertex_live(slot).then(|| VertexId {
            slot,
            generation: self.vertex_generations[slot],
        })
    }

    pub fn face_id(&self, slot: usize) -> Option<FaceId> {
        self.is_triangle_live(slot).then(|| FaceId {
            slot,
            generation: self.triangle_generations[slot],
        })
    }

    pub fn edge_id(&self, a: usize, b: usize) -> Option<EdgeId> {
        Some(EdgeId::new(self.vertex_id(a)?, self.vertex_id(b)?))
    }

    pub fn contains_vertex_id(&self, id: VertexId) -> bool {
        self.vertex_id(id.slot) == Some(id)
    }

    pub fn contains_face_id(&self, id: FaceId) -> bool {
        self.face_id(id.slot) == Some(id)
    }

    pub(crate) fn triangle_generations(&self) -> &[u64] {
        &self.triangle_generations
    }

    pub fn is_vertex_live(&self, slot: usize) -> bool {
        slot >= MESH_STATE_FIRST_ID && self.vertex_live.get(slot).copied().unwrap_or(false)
    }

    pub fn is_triangle_live(&self, slot: usize) -> bool {
        slot >= MESH_STATE_FIRST_ID && self.triangle_live.get(slot).copied().unwrap_or(false)
    }

    pub fn active_vertex_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (MESH_STATE_FIRST_ID..self.vertices.len()).filter(|&slot| self.is_vertex_live(slot))
    }

    pub fn active_triangle_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (MESH_STATE_FIRST_ID..self.triangles.len()).filter(|&slot| self.is_triangle_live(slot))
    }

    pub(crate) fn retire_vertex_slot(&mut self, vertex: usize) {
        if vertex >= MESH_STATE_FIRST_ID && vertex < self.vertex_live.len() {
            self.vertex_live[vertex] = false;
            self.vertex_generations[vertex] = self.next_vertex_generation;
            self.next_vertex_generation += 1;
        }
    }

    #[cfg(test)]
    pub(crate) fn retire_vertex_for_test(&mut self, vertex: usize) {
        self.retire_vertex_slot(vertex);
    }

    #[cfg(test)]
    pub(crate) fn retire_triangle_in_region_for_test(
        &mut self,
        triangle: usize,
        region: &std::collections::BTreeSet<usize>,
    ) {
        if triangle < MESH_STATE_FIRST_ID || triangle >= self.triangle_live.len() {
            return;
        }
        self.triangle_live[triangle] = false;
        self.triangle_generations[triangle] = self.next_triangle_generation;
        self.next_triangle_generation += 1;
        self.neighbours[triangle] = [0; 3];
        for &row in region {
            if let Some(neighbours) = self.neighbours.get_mut(row) {
                for neighbour in neighbours {
                    if *neighbour == triangle {
                        *neighbour = 0;
                    }
                }
            }
        }
    }

    pub(crate) fn retire_triangle_slot(&mut self, triangle: usize) {
        if triangle < MESH_STATE_FIRST_ID || triangle >= self.triangle_live.len() {
            return;
        }
        self.triangle_live[triangle] = false;
        self.triangle_generations[triangle] = self.next_triangle_generation;
        self.next_triangle_generation += 1;
        self.neighbours[triangle] = [0; 3];
    }

    pub(crate) fn restore_triangle_generation(&mut self, triangle: usize, generation: u64) {
        self.triangle_generations[triangle] = generation;
    }

    /// Real sites, not counting the two reserved slots.
    pub fn vertex_count(&self) -> usize {
        self.active_vertex_slots().count()
    }

    pub fn triangle_count(&self) -> usize {
        self.active_triangle_slots().count()
    }

    /// Triangle edges with no triangle across them.
    ///
    /// Zero over a closed sphere. Non-zero is either a real boundary or a hole
    /// something opened, and the caller is the one who knows which it should
    /// be.
    pub fn open_edge_count(&self) -> usize {
        self.active_triangle_slots()
            .map(|triangle| {
                self.neighbours[triangle]
                    .iter()
                    .filter(|&&neighbour| neighbour == 0)
                    .count()
            })
            .sum()
    }

    /// Add a site and return its id.
    pub(crate) fn push_vertex(&mut self, point: CartesianPoint) -> usize {
        self.vertices.push(point);
        self.vertex_generations.push(self.next_vertex_generation);
        self.vertex_live.push(true);
        self.next_vertex_generation += 1;
        self.vertices.len() - 1
    }

    /// Move one site.
    ///
    /// Leaves the triangles alone, which is the point and also the hazard: the
    /// mesh is still a triangulation and is very likely no longer Delaunay, so
    /// a caller owes the neighbourhood a `legalize_around`.
    pub fn move_vertex(&mut self, vertex: usize, point: CartesianPoint) {
        self.vertices[vertex] = point;
    }

    /// Overwrite one triangle's corners, leaving its id alone.
    ///
    /// Reusing a slot rather than deleting is what keeps every other id stable
    /// through an insertion. A caller doing this owes the adjacency a repair.
    pub(crate) fn set_triangle(&mut self, triangle: usize, corners: [usize; 3]) {
        debug_assert!(self.is_triangle_live(triangle));
        self.triangles[triangle] = corners;
        self.triangle_generations[triangle] = self.next_triangle_generation;
        self.next_triangle_generation += 1;
    }

    /// Add a triangle and return its id.
    pub(crate) fn push_triangle(&mut self, corners: [usize; 3]) -> usize {
        self.triangles.push(corners);
        self.neighbours.push([0usize; 3]);
        self.triangle_generations
            .push(self.next_triangle_generation);
        self.triangle_live.push(true);
        self.next_triangle_generation += 1;
        self.triangles.len() - 1
    }

    /// Drop everything above these lengths.
    ///
    /// Truncation rather than deletion: ids below the cut keep their meaning,
    /// which is the whole reason a rollback does not renumber the mesh.
    pub(crate) fn truncate_to(&mut self, vertices: usize, triangles: usize) {
        self.vertices.truncate(vertices);
        self.vertex_generations.truncate(vertices);
        self.vertex_live.truncate(vertices);
        self.triangles.truncate(triangles);
        self.neighbours.truncate(triangles);
        self.triangle_generations.truncate(triangles);
        self.triangle_live.truncate(triangles);
    }

    /// Write back one triangle's corners and adjacency together.
    ///
    /// Only a restore has both in hand and knows they are consistent; every
    /// other caller writes corners and then owes the adjacency a repair.
    pub(crate) fn restore_row(
        &mut self,
        triangle: usize,
        corners: [usize; 3],
        neighbours: [usize; 3],
    ) {
        self.triangles[triangle] = corners;
        self.neighbours[triangle] = neighbours;
    }

    /// Rebuild adjacency across exactly the given triangles.
    ///
    /// The set has to be supplied rather than grown from `self.neighbours`,
    /// because a caller calls this precisely when that array is stale: a
    /// triangle written into a reused slot still carries the adjacency of what
    /// was there, and a freshly pushed one carries none. Growing the set from
    /// it would miss the triangles just outside the change, which is how an
    /// insertion leaves open edges behind.
    ///
    /// Every edge that moved is incident to at least one changed triangle, so a
    /// caller passing the changed triangles together with the ring immediately
    /// outside them covers all of them.
    /// `authoritative` names the triangles whose adjacency this region fully
    /// describes. For those, an edge with no claimant here really has none and
    /// is written as a boundary. For the rest -- the ring just outside a change
    /// -- only edges that found a claimant are rewritten, because their outward
    /// edges face triangles the region does not hold, and zeroing those is how
    /// a repair opens the very holes it was called to close.
    pub(crate) fn repair_adjacency_across(
        &mut self,
        region: &std::collections::BTreeSet<usize>,
        authoritative: &std::collections::BTreeSet<usize>,
    ) {
        let mut claims: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for &triangle in region {
            if !self.is_triangle_live(triangle) {
                continue;
            }
            let corners = self.triangles[triangle];
            for corner in 0..3 {
                let key = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                claims.entry(key).or_default().push(triangle);
            }
        }
        for &triangle in region {
            if !self.is_triangle_live(triangle) {
                continue;
            }
            let corners = self.triangles[triangle];
            for corner in 0..3 {
                let key = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                let opposite = claims.get(&key).and_then(|claimants| {
                    claimants.iter().copied().find(|&other| other != triangle)
                });
                match opposite {
                    Some(other) => self.neighbours[triangle][corner] = other,
                    None if authoritative.contains(&triangle) => {
                        self.neighbours[triangle][corner] = 0
                    }
                    None => {}
                }
            }
        }
    }

    /// The radius of the sphere the sites sit on, averaged over them.
    ///
    /// Not a constant of the type: a relaxed mesh's points are not exactly
    /// equidistant, and the value here is only meant to answer "is this
    /// candidate on the same sphere as the mesh", which is a question about
    /// orders of magnitude rather than metres.
    pub fn sphere_radius(&self) -> f64 {
        let count = self.vertex_count();
        if count == 0 {
            return 0.0;
        }
        let total: f64 = self
            .active_vertex_slots()
            .map(|vertex| self.vertices[vertex])
            .map(|point| (point.x * point.x + point.y * point.y + point.z * point.z).sqrt())
            .sum();
        total / count as f64
    }

    /// Edges with nothing across them, among these triangles only.
    ///
    /// What a local change can afford. [`Self::open_edge_count`] answers for
    /// the mesh, which costs the mesh -- and a change that touched nine
    /// triangles paying for a million is the shape that makes a per-change
    /// gate quadratic over a run.
    ///
    /// Sound for a local change on a surface that was closed: everything
    /// outside the region was closed before and was not touched, so the region
    /// and the ring around it are where an opening could be.
    pub fn open_edges_in(&self, region: &std::collections::BTreeSet<usize>) -> usize {
        region
            .iter()
            .filter(|&&triangle| self.is_triangle_live(triangle))
            .map(|&triangle| {
                self.neighbours[triangle]
                    .iter()
                    .filter(|&&neighbour| neighbour == 0)
                    .count()
            })
            .sum()
    }

    /// Check the invariants over these triangles only.
    ///
    /// Same reasoning as [`Self::open_edges_in`], and the same soundness
    /// condition: it says nothing about a mesh that was already broken outside
    /// the region.
    pub fn validate_region(
        &self,
        region: &std::collections::BTreeSet<usize>,
    ) -> Result<(), Vec<MeshStateError>> {
        let mut errors = Vec::new();
        for &triangle in region {
            if !self.is_triangle_live(triangle) {
                continue;
            }
            self.validate_triangle_row(triangle, &mut errors);
            self.validate_neighbour_edges_for_triangle(triangle, &mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check the invariants a consumer is entitled to assume.
    ///
    /// Adjacency is derived rather than supplied, so this checks table rows,
    /// non-manifold claims, and that neighbour rows point back across the same
    /// canonical edge.
    pub fn validate(&self) -> Result<(), Vec<MeshStateError>> {
        let mut errors = Vec::new();
        let mut claims: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for triangle in self.active_triangle_slots() {
            self.validate_triangle_row(triangle, &mut errors);
            let corners = self.triangles[triangle];
            if corners.iter().all(|corner| self.is_vertex_live(*corner))
                && corners[0] != corners[1]
                && corners[1] != corners[2]
                && corners[0] != corners[2]
            {
                for corner in 0..3 {
                    claims
                        .entry(edge_key(
                            corners[(corner + 1) % 3],
                            corners[(corner + 2) % 3],
                        ))
                        .or_default()
                        .push(triangle);
                }
            }
            self.validate_neighbour_edges_for_triangle(triangle, &mut errors);
        }
        for (vertices, claimants) in claims {
            if claimants.len() > 2 {
                errors.push(MeshStateError::NonManifoldEdge {
                    vertices,
                    triangles: claimants.len(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn validate_triangle_row(&self, triangle: usize, errors: &mut Vec<MeshStateError>) {
        let corners = self.triangles[triangle];
        for &corner in &corners {
            if !self.is_vertex_live(corner) {
                errors.push(MeshStateError::UnknownVertex {
                    triangle,
                    vertex: corner,
                });
            }
        }
        if corners[0] == corners[1] || corners[1] == corners[2] || corners[0] == corners[2] {
            errors.push(MeshStateError::DegenerateTriangle { triangle, corners });
        }
    }

    fn validate_neighbour_edges_for_triangle(
        &self,
        triangle: usize,
        errors: &mut Vec<MeshStateError>,
    ) {
        let corners = self.triangles[triangle];
        if corners.iter().any(|corner| !self.is_vertex_live(*corner)) {
            return;
        }
        for corner in 0..3 {
            let neighbour = self.neighbours[triangle][corner];
            if neighbour == 0 {
                continue;
            }
            let edge = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
            if !self.is_triangle_live(neighbour)
                || !self.neighbour_points_back_across_edge(neighbour, triangle, edge)
            {
                errors.push(MeshStateError::AsymmetricNeighbour {
                    triangle,
                    neighbour,
                });
            }
        }
    }

    fn neighbour_points_back_across_edge(
        &self,
        neighbour: usize,
        triangle: usize,
        edge: (usize, usize),
    ) -> bool {
        let corners = self.triangles[neighbour];
        if corners.iter().any(|corner| !self.is_vertex_live(*corner)) {
            return false;
        }
        for corner in 0..3 {
            let candidate = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
            if candidate == edge && self.neighbours[neighbour][corner] == triangle {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
