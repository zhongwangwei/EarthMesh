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

/// Sites, triangles, and what is across each edge.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshState {
    vertices: Vec<CartesianPoint>,
    triangles: Vec<[usize; 3]>,
    /// `neighbours[t][i]` is the triangle across the edge opposite corner `i`
    /// of triangle `t`, or zero where no triangle is.
    neighbours: Vec<[usize; 3]>,
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
                if corner >= vertices.len() {
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

        Ok(Self {
            vertices,
            triangles,
            neighbours,
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
        let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); mesh.nmd + 1];
        vertices[MESH_STATE_FIRST_ID..=mesh.nmd]
            .clone_from_slice(&mesh.m_points[MESH_STATE_FIRST_ID..=mesh.nmd]);
        let mut triangles = vec![[1usize; 3]; mesh.nwd + 1];
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

    /// Real sites, not counting the two reserved slots.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len().saturating_sub(MESH_STATE_FIRST_ID)
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len().saturating_sub(MESH_STATE_FIRST_ID)
    }

    /// Triangle edges with no triangle across them.
    ///
    /// Zero over a closed sphere. Non-zero is either a real boundary or a hole
    /// something opened, and the caller is the one who knows which it should
    /// be.
    pub fn open_edge_count(&self) -> usize {
        (MESH_STATE_FIRST_ID..self.triangles.len())
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
        self.triangles[triangle] = corners;
    }

    /// Add a triangle and return its id.
    pub(crate) fn push_triangle(&mut self, corners: [usize; 3]) -> usize {
        self.triangles.push(corners);
        self.neighbours.push([0usize; 3]);
        self.triangles.len() - 1
    }

    /// Drop everything above these lengths.
    ///
    /// Truncation rather than deletion: ids below the cut keep their meaning,
    /// which is the whole reason a rollback does not renumber the mesh.
    pub(crate) fn truncate_to(&mut self, vertices: usize, triangles: usize) {
        self.vertices.truncate(vertices);
        self.triangles.truncate(triangles);
        self.neighbours.truncate(triangles);
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
            let corners = self.triangles[triangle];
            for corner in 0..3 {
                let key = edge_key(corners[(corner + 1) % 3], corners[(corner + 2) % 3]);
                claims.entry(key).or_default().push(triangle);
            }
        }
        for &triangle in region {
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
        let total: f64 = self.vertices[MESH_STATE_FIRST_ID..]
            .iter()
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
            .filter(|&&triangle| triangle >= MESH_STATE_FIRST_ID && triangle < self.triangles.len())
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
            if triangle < MESH_STATE_FIRST_ID || triangle >= self.triangles.len() {
                continue;
            }
            let corners = self.triangles[triangle];
            for &corner in &corners {
                if corner >= self.vertices.len() {
                    errors.push(MeshStateError::UnknownVertex {
                        triangle,
                        vertex: corner,
                    });
                }
            }
            if corners[0] == corners[1] || corners[1] == corners[2] || corners[0] == corners[2] {
                errors.push(MeshStateError::DegenerateTriangle { triangle, corners });
            }
            for &neighbour in &self.neighbours[triangle] {
                if neighbour == 0 {
                    continue;
                }
                if neighbour >= self.triangles.len()
                    || !self.neighbours[neighbour].contains(&triangle)
                {
                    errors.push(MeshStateError::AsymmetricNeighbour {
                        triangle,
                        neighbour,
                    });
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check the invariants a consumer is entitled to assume.
    ///
    /// Adjacency is derived rather than supplied, so what this adds over
    /// construction is the symmetry check: a triangle that names a neighbour
    /// must be named back, and a state where that fails cannot be walked.
    pub fn validate(&self) -> Result<(), Vec<MeshStateError>> {
        let mut errors = Vec::new();
        for triangle in MESH_STATE_FIRST_ID..self.triangles.len() {
            for &neighbour in &self.neighbours[triangle] {
                if neighbour == 0 {
                    continue;
                }
                if neighbour >= self.triangles.len()
                    || !self.neighbours[neighbour].contains(&triangle)
                {
                    errors.push(MeshStateError::AsymmetricNeighbour {
                        triangle,
                        neighbour,
                    });
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests;
