//! Taking a piece of a triangulation away and putting it back.
//!
//! A local change is only worth making if it can be unmade. Everything the
//! transaction layer above wants -- propose, check, keep or discard -- rests on
//! being able to restore the neighbourhood exactly as it was, without touching
//! the rest of the mesh or renumbering anything.
//!
//! # What a patch has to cover
//!
//! Not just the triangles that change. A triangle just outside them keeps its
//! own corners but has its adjacency rewritten, because the thing across its
//! edge is now a different triangle. Restoring the changed rows alone leaves
//! that ring pointing into the new work, which reads as a valid mesh and is not
//! the old one.
//!
//! So [`MeshState::snapshot_around`] takes the seed set *and* everything
//! adjacent to it. The caller names the triangles it means to disturb; the
//! ring comes along without being asked for.
//!
//! # One restore, and only backwards
//!
//! [`MeshState::restore_patch`] takes the patch by value, so it cannot be
//! applied twice. It also truncates the arrays back to the lengths they had --
//! which is what undoes appended triangles and sites, and is also the reason a
//! patch is an undo for *the most recent* change and not a general checkpoint.
//! Restoring across an unrelated later change would throw that change away.
//! The mesh cannot tell the difference, so the layer sequencing the
//! transactions is the one that has to keep them in order.

use std::collections::BTreeSet;

use crate::mesh_state::{MeshState, MESH_STATE_FIRST_ID};

/// Rows of a triangulation, kept so they can be put back.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPatch {
    /// Triangle id, its corners, and what was across each of its edges.
    rows: Vec<(usize, [usize; 3], [usize; 3])>,
    vertex_len: usize,
    triangle_len: usize,
}

impl MeshPatch {
    /// The triangles this patch can restore.
    pub fn triangles(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows.iter().map(|&(triangle, _, _)| triangle)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Why a patch could not be put back.
#[derive(Clone, Debug, PartialEq)]
pub enum PatchError {
    /// The mesh has fewer triangles or sites than when the patch was taken, so
    /// the rows it holds have nowhere to go. Restoring can undo growth; it
    /// cannot undo a deletion it did not record.
    MeshShrankBelowThePatch {
        patch_triangles: usize,
        mesh_triangles: usize,
        patch_vertices: usize,
        mesh_vertices: usize,
    },
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MeshShrankBelowThePatch {
                patch_triangles,
                mesh_triangles,
                patch_vertices,
                mesh_vertices,
            } => write!(
                formatter,
                "the patch was taken from a mesh of {patch_triangles} triangles and \
                 {patch_vertices} sites, and this one has {mesh_triangles} and {mesh_vertices}; a \
                 patch undoes growth and cannot undo a shrink"
            ),
        }
    }
}

impl std::error::Error for PatchError {}

impl MeshState {
    /// Keep the rows of `seed` and of every triangle adjacent to it.
    ///
    /// The ring is included because a change rewrites its adjacency even though
    /// it leaves its corners alone -- see the module docs.
    pub fn snapshot_around(&self, seed: &BTreeSet<usize>) -> MeshPatch {
        let mut region: BTreeSet<usize> = BTreeSet::new();
        for &triangle in seed {
            if triangle < MESH_STATE_FIRST_ID || triangle >= self.triangles().len() {
                continue;
            }
            region.insert(triangle);
            region.extend(
                self.neighbours()[triangle]
                    .iter()
                    .copied()
                    .filter(|&neighbour| neighbour >= MESH_STATE_FIRST_ID),
            );
        }
        MeshPatch {
            rows: region
                .into_iter()
                .map(|triangle| {
                    (
                        triangle,
                        self.triangles()[triangle],
                        self.neighbours()[triangle],
                    )
                })
                .collect(),
            vertex_len: self.vertices().len(),
            triangle_len: self.triangles().len(),
        }
    }

    /// Put a patch back, discarding everything appended after it was taken.
    pub fn restore_patch(&mut self, patch: MeshPatch) -> Result<(), PatchError> {
        if self.triangles().len() < patch.triangle_len || self.vertices().len() < patch.vertex_len {
            return Err(PatchError::MeshShrankBelowThePatch {
                patch_triangles: patch.triangle_len,
                mesh_triangles: self.triangles().len(),
                patch_vertices: patch.vertex_len,
                mesh_vertices: self.vertices().len(),
            });
        }
        self.truncate_to(patch.vertex_len, patch.triangle_len);
        for (triangle, corners, neighbours) in patch.rows {
            self.restore_row(triangle, corners, neighbours);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
