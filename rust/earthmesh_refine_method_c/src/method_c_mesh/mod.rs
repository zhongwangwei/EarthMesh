//! Method-C's mesh: the shared one, plus what only the nesting has.
//!
//! Algorithm provenance: OLAM Method-C. Data ownership, validation, recovery,
//! and public interfaces are maintained by EarthMesh.
//!
//! The method is Walko, R. L., & Avissar, R. (2011), *A direct method for
//! constructing refined regions in unstructured conforming triangular-hexagonal
//! computational grids: Application to OLAM*, Monthly Weather Review 139(12),
//! 3923-3937, doi:10.1175/MWR-D-11-00021.1. Both halves of that paper are here:
//! subdividing a closed region by joining the midpoints of each triangle's
//! edges, and building the transition rows outside it that keep the mesh
//! conforming, bound how abruptly resolution changes, and hold vertex degree to
//! {5, 6, 7}. The paper names the transition rows as its own main contribution,
//! which is why they carry the strictest rules in this crate.
//!
//! # Why this type exists
//!
//! Not to hold state -- it holds one field. To hold *methods*. Method-C is 28
//! `impl` blocks, and Rust has no inherent impl of a foreign type, so as long
//! as they hung off [`TriangularMesh`] they could not leave this crate however
//! the files were arranged. Hanging them off a type Method-C owns is what makes
//! the move a move.
//!
//! The one field is `boundary_rows`, the W faces the most recent pass emitted
//! as mrow transition rows. It was on the shared mesh, where nothing outside
//! the nesting ever read it except the CLI, counting them for a report.
//!
//! # Deref
//!
//! [`MethodCMesh`] derefs to the shared mesh, so `self.nmd` and
//! `self.w_faces[..]` inside the nesting keep meaning what they meant. Without
//! it this change would have rewritten every field access in seventeen thousand
//! lines, and a rewrite that large is one nobody can review as a rename.

use std::ops::{Deref, DerefMut};

use super::*;

/// A mesh being refined by Method-C.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodCMesh {
    inner: TriangularMesh,
    /// Final W-face ids generated as transition rows by the most recent
    /// specified-region refinement pass.
    pub(crate) boundary_rows: Vec<usize>,
}

impl MethodCMesh {
    /// Surface (non-atmosphere) perimeter-row expansion width used by
    /// Method-C `perim_mrow`.
    pub const MAX_MROWS_SURFACE: usize = 7;

    /// Atmosphere perimeter-row expansion width used by Method-C
    /// `perim_mrow`.
    pub const MAX_MROWS_ATMOS: usize = 13;

    /// Take a shared mesh into the nesting.
    pub fn new(inner: TriangularMesh) -> Self {
        Self {
            inner,
            boundary_rows: Vec::new(),
        }
    }

    /// Build the base icosahedral mesh and take it into the nesting.
    ///
    /// `Deref` does not reach associated functions, so the shared
    /// constructors are named again here rather than inherited.
    pub fn from_icosahedron(nxp0: usize, niter: usize, beta: f64, relax: f64) -> Option<Self> {
        TriangularMesh::from_icosahedron(nxp0, niter, beta, relax).map(Self::new)
    }

    pub fn from_relaxed_icosahedron(relaxed: &IcosahedronRelaxedGrid) -> Self {
        Self::new(TriangularMesh::from_relaxed_icosahedron(relaxed))
    }

    pub fn from_cart_hex(nxp: usize, deltax_meters: f64) -> io::Result<Self> {
        TriangularMesh::from_cart_hex(nxp, deltax_meters).map(Self::new)
    }

    pub fn from_voronoi_gridfile_tables_with_metadata(
        m_point_lonlat: &[LonLatDegrees],
        w_face_m_points: &[[usize; 3]],
        m_face_counts: &[usize],
        metadata: MethodCGridfileMetadata<'_>,
    ) -> io::Result<Self> {
        TriangularMesh::from_voronoi_gridfile_tables_with_metadata(
            m_point_lonlat,
            w_face_m_points,
            m_face_counts,
            metadata,
        )
        .map(Self::new)
    }

    /// The same, carrying transition rows a previous pass already found.
    pub fn with_boundary_rows(inner: TriangularMesh, boundary_rows: Vec<usize>) -> Self {
        Self {
            inner,
            boundary_rows,
        }
    }

    /// The shared mesh, for the writers and the other backends.
    ///
    /// The transition rows do not come with it, which is the point: outside
    /// Method-C they mean nothing, and a mesh carrying fields nobody reads is
    /// how this crate came to look like Method-C's in the first place.
    pub fn into_inner(self) -> TriangularMesh {
        self.inner
    }

    pub fn mesh(&self) -> &TriangularMesh {
        &self.inner
    }

    /// Final W-face ids that were generated as transition rows by the most
    /// recent specified-region refinement pass.
    pub fn boundary_rows(&self) -> &[usize] {
        &self.boundary_rows
    }
}

impl From<TriangularMesh> for MethodCMesh {
    fn from(inner: TriangularMesh) -> Self {
        Self::new(inner)
    }
}

impl From<MethodCMesh> for TriangularMesh {
    fn from(mesh: MethodCMesh) -> Self {
        mesh.inner
    }
}

impl Deref for MethodCMesh {
    type Target = TriangularMesh;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for MethodCMesh {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
