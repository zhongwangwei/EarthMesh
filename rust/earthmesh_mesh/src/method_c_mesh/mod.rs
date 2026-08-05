//! EarthMesh's adaptive Delaunay/Voronoi mesh core.
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

use super::*;

/// Generic Method-C-style Delaunay mesh state.
///
/// Method-C carries the triangular mesh as three reciprocal tables:
///
/// - M: Delaunay vertices / future Voronoi cell centers.
/// - U: Delaunay edges.
/// - W: Delaunay triangle faces / future Voronoi vertices.
///
/// Its validation rules are generic so global expansion and local refinement
/// share the same topology invariants.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodCDelaunayMesh {
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
    /// For each W face, the id it descended from in the mesh this one was
    /// refined out of. Indexed one-based like the faces; slot 0/1 are the
    /// canonical placeholders.
    ///
    /// A face that was never subdivided keeps its own id, so on an unrefined
    /// mesh the lineage is the identity. Refinement is the only thing that
    /// makes it differ, which is what makes it answer "where did this cell come
    /// from" after several passes and a renumbering have moved every row.
    pub(crate) w_lineage: Vec<usize>,
    /// The same for M points.
    pub(crate) m_lineage: Vec<usize>,
}

/// `[0, 1, 2, ..., count]`: every row descends from itself.
///
/// Row 1 is the canonical placeholder and stays one through refinement
/// (`iwnew[1] = 1` in the emitter), so naming itself is what keeps the lineage
/// total — every row the gridfile carries resolves to a row that existed.
fn identity_lineage(count: usize) -> Vec<usize> {
    (0..=count).collect()
}

impl MethodCDelaunayMesh {
    /// Surface (non-atmosphere) perimeter-row expansion width used by
    /// Method-C `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_SURFACE: usize = 7;

    /// Atmosphere perimeter-row expansion width used by Method-C
    /// `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_ATMOS: usize = 13;

    /// Build the generic Method-C Delaunay mesh wrapper from an already-relaxed
    /// global icosahedron.
    pub fn from_relaxed_icosahedron(relaxed: &IcosahedronRelaxedGrid) -> Self {
        Self {
            nmd: relaxed.nmd,
            nud: relaxed.nud,
            nwd: relaxed.nwd,
            impent: relaxed.impent,
            m_points: relaxed.m_points.clone(),
            m_metadata: default_method_c_m_metadata(relaxed.nmd),
            u_edges: relaxed.connectivity.u_edges.clone(),
            w_faces: relaxed.connectivity.w_faces.clone(),
            m_neighbors: relaxed.m_neighbors.clone(),
            m_prognostic: method_c_identity_prognostic_map(relaxed.nmd),
            u_prognostic: method_c_identity_prognostic_map(relaxed.nud),
            w_prognostic: method_c_identity_prognostic_map(relaxed.nwd),
            boundary_rows: Vec::new(),
            w_lineage: identity_lineage(relaxed.nwd),
            m_lineage: identity_lineage(relaxed.nmd),
        }
    }

    /// Build a validated Method-C Delaunay mesh from the current global
    /// icosahedron path.
    pub fn from_icosahedron(
        nxp0: usize,
        niter: usize,
        beta: f64,
        relax: f64,
        _diagnostic_every: usize,
    ) -> Option<Self> {
        let initial = icosahedron_initial_grid_canonical(nxp0)?;
        let mut connectivity = icosahedron_fill_diamonds_canonical(nxp0)?;
        let m_neighbors =
            derive_icosahedron_tri_neighbors_canonical(initial.nmd, &mut connectivity)?;
        let mesh = Self {
            nmd: initial.nmd,
            nud: initial.nud,
            nwd: initial.nwd,
            impent: initial.impent,
            m_points: initial.m_points,
            m_metadata: default_method_c_m_metadata(initial.nmd),
            u_edges: connectivity.u_edges,
            w_faces: connectivity.w_faces,
            m_neighbors,
            m_prognostic: method_c_identity_prognostic_map(initial.nmd),
            u_prognostic: method_c_identity_prognostic_map(initial.nud),
            w_prognostic: method_c_identity_prognostic_map(initial.nwd),
            boundary_rows: Vec::new(),
            w_lineage: identity_lineage(initial.nwd),
            m_lineage: identity_lineage(initial.nmd),
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

    /// One-based Method-C `itab_md` refinement/grid metadata.
    pub fn m_point_metadata(&self) -> &[IcosahedronMPointMetadata] {
        &self.m_metadata
    }

    /// File-boundary lineages for final triangular cells (`itab_m` rows).
    ///
    /// A final triangular M cell is a Delaunay W face, so its ancestry is the
    /// W lineage.
    pub fn gridfile_m_cell_lineages(&self) -> io::Result<Vec<i64>> {
        method_c_gridfile_lineages("M cell", &self.w_lineage, self.nwd)
    }

    /// File-boundary lineages for final polygonal cells (`itab_w` rows).
    ///
    /// A final polygonal W cell is centred on a Delaunay M point.
    pub fn gridfile_w_cell_lineages(&self) -> io::Result<Vec<i64>> {
        method_c_gridfile_lineages("W cell", &self.m_lineage, self.nmd)
    }
}

/// Lineage rows for ids `1..=active_count`, as the gridfile carries them.
fn method_c_gridfile_lineages(
    role: &str,
    lineages: &[usize],
    active_count: usize,
) -> io::Result<Vec<i64>> {
    if lineages.len() < active_count + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Method-C {role} lineage has {} rows for {} active ids",
                lineages.len(),
                active_count
            ),
        ));
    }
    (1..=active_count)
        .map(|id| {
            i64::try_from(lineages[id]).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C {role} lineage {} at id {id} exceeds i64",
                        lineages[id]
                    ),
                )
            })
        })
        .collect()
}
