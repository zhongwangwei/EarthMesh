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
use crate::{CartesianPoint, TriangularMesh};

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
        let vertex_slots: Vec<_> = self.active_vertex_slots().collect();
        let triangle_slots: Vec<_> = self.active_triangle_slots().collect();

        let mut vertex_remap = vec![0usize; self.vertices().len()];
        let mut points =
            vec![CartesianPoint::new(0.0, 0.0, 0.0); vertex_slots.len() + MESH_STATE_FIRST_ID];
        for (new_slot, old_slot) in vertex_slots.iter().copied().enumerate() {
            let new_slot = new_slot + MESH_STATE_FIRST_ID;
            vertex_remap[old_slot] = new_slot;
            points[new_slot] = self.vertices()[old_slot];
        }

        let mut compact_impent = [0usize; 12];
        for (index, site) in impent.into_iter().enumerate() {
            let Some(&mapped) = vertex_remap.get(site) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("pentagon id {site} is not a site of this mesh"),
                ));
            };
            if mapped == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("pentagon id {site} is not a site of this mesh"),
                ));
            }
            compact_impent[index] = mapped;
        }

        let seeds: Vec<MethodCTriangleSeed> = triangle_slots
            .iter()
            .copied()
            .enumerate()
            .map(|(new_index, triangle)| {
                let level = face_levels
                    .and_then(|levels| levels.get(triangle))
                    .copied()
                    .unwrap_or(1);
                let corners = self.triangles()[triangle].map(|site| vertex_remap[site]);
                MethodCTriangleSeed::new(corners, (level, level, ngr))
                    .with_target_iw(new_index + MESH_STATE_FIRST_ID)
                    .with_mrow(0)
            })
            .collect();

        method_c_mesh_from_triangle_seeds(points.len() - 1, compact_impent, points, &seeds)
    }
}

#[cfg(test)]
mod tests;
