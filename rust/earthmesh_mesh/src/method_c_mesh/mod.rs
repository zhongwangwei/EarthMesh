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

impl TriangularMesh {
    /// Surface (non-atmosphere) perimeter-row expansion width used by
    /// Method-C `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_SURFACE: usize = 7;

    /// Atmosphere perimeter-row expansion width used by Method-C
    /// `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_ATMOS: usize = 13;

    /// Final W-face ids that were generated as transition rows by the most
    /// recent specified-region refinement pass.
    pub fn boundary_rows(&self) -> &[usize] {
        &self.boundary_rows
    }
}
