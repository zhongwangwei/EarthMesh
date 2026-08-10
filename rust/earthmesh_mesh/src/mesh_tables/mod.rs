//! From a triangulation to the three tables the writers consume.
//!
//! [`MeshState`] carries sites, triangles and adjacency, which is all any
//! refinement backend needs. A gridfile carries more: edges with ids, how many
//! faces meet at each site and in what order, a refinement generation per row.
//! Those are derivable, and this derives them -- the inverse of
//! [`MeshState::from_triangular_mesh`], and what lets a backend built on the
//! neutral type reach the output path at all.
//!
//! # Reusing the rebuild
//!
//! The derivation is `mesh_rebuild`'s, the same one the gridfile reader and
//! both global expansions go through. Writing a second one would be writing a
//! second set of conventions about edge ordering and W-face winding, and the
//! first set is the one the writers were built against.
//!
//! # Pentagons are carried, not derived
//!
//! `impent` names twelve sites, and the obvious derivation -- the degree-5 ones
//! -- is wrong. Euler gives `#degree-5 - #degree-7 = 12` on a sphere, so any
//! mesh with a degree-7 site has more than twelve degree-5 sites. Measured: one
//! insertion into an NXP 6 icosahedral mesh leaves fourteen. A refined Method-C
//! mesh is in the same position, which is why `TriangularMesh` carries `impent`
//! as a field rather than computing it.
//!
//! So the caller supplies them, from the mesh it started with. Site ids are
//! stable across insertion -- nothing is renumbered and nothing is removed --
//! so the twelve the icosahedron was built with still name the same twelve
//! sites however much refining has happened since.

use std::io;

use crate::mesh_rebuild::method_c_mesh_from_triangle_seeds;
use crate::mesh_state::{MeshState, MESH_STATE_FIRST_ID};
use crate::mesh_triangle_seed::MethodCTriangleSeed;
use crate::TriangularMesh;

impl MeshState {
    /// Build the three-table mesh, deriving edges, incidence and ordering.
    ///
    /// `impent` is the twelve pentagon ids, carried from whatever mesh this one
    /// descends from -- see the module docs for why they cannot be derived.
    ///
    /// `face_levels` gives each face's refinement generation, indexed like the
    /// faces themselves. `None` means one generation everywhere, which is what
    /// an unrefined mesh has and what a backend that does not track
    /// generations should say rather than invent.
    pub fn to_triangular_mesh(
        &self,
        impent: [usize; 12],
        face_levels: Option<&[usize]>,
    ) -> io::Result<TriangularMesh> {
        self.to_triangular_mesh_with_grid_number(impent, face_levels, 1)
    }

    /// The same, choosing the grid number the rows carry.
    ///
    /// `ngr` is not decoration. The nest spring moves only points whose `ngr`
    /// equals the one it is called with, so a backend that leaves every row at
    /// 1 produces a mesh no spring will touch.
    pub fn to_triangular_mesh_with_grid_number(
        &self,
        impent: [usize; 12],
        face_levels: Option<&[usize]>,
        ngr: usize,
    ) -> io::Result<TriangularMesh> {
        if let Some(&stranger) = impent
            .iter()
            .find(|&&site| site < MESH_STATE_FIRST_ID || site >= self.vertices().len())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("pentagon id {stranger} is not a site of this mesh"),
            ));
        }

        let seeds: Vec<MethodCTriangleSeed> = (MESH_STATE_FIRST_ID..self.triangles().len())
            .map(|triangle| {
                let level = face_levels
                    .and_then(|levels| levels.get(triangle))
                    .copied()
                    .unwrap_or(1);
                MethodCTriangleSeed::new(self.triangles()[triangle], (level, level, ngr))
                    // Keep the id. Every report, lineage and demand the caller
                    // is holding names a face by it.
                    .with_target_iw(triangle)
                    .with_mrow(0)
            })
            .collect();

        method_c_mesh_from_triangle_seeds(
            self.vertices().len() - 1,
            impent,
            self.vertices().to_vec(),
            &seeds,
        )
    }
}

#[cfg(test)]
mod tests;
