use super::*;

/// Generic OLAM-style Delaunay mesh state.
///
/// OLAM carries the triangular mesh as three reciprocal tables:
///
/// - M: Delaunay vertices / future Voronoi cell centers.
/// - U: Delaunay edges.
/// - W: Delaunay triangle faces / future Voronoi vertices.
///
/// This type is the replacement boundary for new grid construction work.  It
/// currently wraps the migrated icosahedron tables, but its validation rules are
/// intentionally generic so global expansion and `spawn_nest` can plug into the
/// same invariant checks instead of patching local connectivity by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct OlamDelaunayMesh {
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
    pub impent: [usize; 12],
    pub m_points: Vec<CartesianPoint>,
    pub(crate) m_metadata: Vec<IcosahedronMPointMetadata>,
    pub u_edges: Vec<IcosahedronUEdge>,
    pub w_faces: Vec<IcosahedronWFace>,
    pub m_neighbors: Vec<IcosahedronMPointNeighbors>,
    pub m_prognostic: Vec<usize>,
    pub u_prognostic: Vec<usize>,
    pub w_prognostic: Vec<usize>,
    pub(crate) boundary_rows: Vec<usize>,
}

impl OlamDelaunayMesh {
    /// Surface (non-atmosphere) perimeter-row expansion width used by
    /// OLAM Method-C `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_SURFACE: usize = 7;

    /// Atmosphere perimeter-row expansion width used by OLAM Method-C
    /// `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_ATMOS: usize = 13;

    /// Build the generic OLAM Delaunay mesh wrapper from an already-relaxed
    /// global icosahedron.
    pub fn from_relaxed_icosahedron(relaxed: &IcosahedronRelaxedGrid) -> Self {
        Self {
            nmd: relaxed.nmd,
            nud: relaxed.nud,
            nwd: relaxed.nwd,
            impent: relaxed.impent,
            m_points: relaxed.m_points.clone(),
            m_metadata: default_olam_m_metadata(relaxed.nmd),
            u_edges: relaxed.connectivity.u_edges.clone(),
            w_faces: relaxed.connectivity.w_faces.clone(),
            m_neighbors: relaxed.m_neighbors.clone(),
            m_prognostic: olam_identity_prognostic_map(relaxed.nmd),
            u_prognostic: olam_identity_prognostic_map(relaxed.nud),
            w_prognostic: olam_identity_prognostic_map(relaxed.nwd),
            boundary_rows: Vec::new(),
        }
    }

    /// Build a validated OLAM Delaunay mesh from the migrated global
    /// icosahedron path.
    pub fn from_icosahedron(
        nxp0: usize,
        niter: usize,
        beta: f64,
        relax: f64,
        _diagnostic_every: usize,
    ) -> Option<Self> {
        let initial = icosahedron_initial_grid_fortran(nxp0)?;
        let mut connectivity = icosahedron_fill_diamonds_fortran(nxp0)?;
        let m_neighbors = derive_icosahedron_tri_neighbors_fortran(initial.nmd, &mut connectivity)?;
        let mesh = Self {
            nmd: initial.nmd,
            nud: initial.nud,
            nwd: initial.nwd,
            impent: initial.impent,
            m_points: initial.m_points,
            m_metadata: default_olam_m_metadata(initial.nmd),
            u_edges: connectivity.u_edges,
            w_faces: connectivity.w_faces,
            m_neighbors,
            m_prognostic: olam_identity_prognostic_map(initial.nmd),
            u_prognostic: olam_identity_prognostic_map(initial.nud),
            w_prognostic: olam_identity_prognostic_map(initial.nwd),
            boundary_rows: Vec::new(),
        };
        mesh.validate_topology().ok()?;
        if niter == 0 {
            Some(mesh)
        } else {
            mesh.spring_global_with_controls(nxp0, niter, beta, relax)
                .ok()
        }
    }

    /// Final W-face ids that were generated as transition rows by the most
    /// recent specified-region refinement pass.
    pub fn boundary_rows(&self) -> &[usize] {
        &self.boundary_rows
    }

    /// One-based OLAM `itab_md` refinement/grid metadata.
    pub fn m_point_metadata(&self) -> &[IcosahedronMPointMetadata] {
        &self.m_metadata
    }
}
