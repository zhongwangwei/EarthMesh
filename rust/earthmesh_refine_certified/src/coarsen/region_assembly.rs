//! The whole sphere from a built region (on-demand reverse coarsening,
//! design B1f in `docs/certified_mesh/on_demand_reverse_coarsening.md`).
//!
//! A region run leaves the coarsened region R and its frame F in its state and
//! the settled base faces S at the level they coarsened to. The whole sphere is
//! those faces together, and it must be the mesh the whole-sphere run ends
//! with: vertices numbered by their finest slot (the order of their origins),
//! leaf faces in address order with their corners in source order, the
//! transition triangles after them -- what `rebuild_from_leaf_set` builds.

use super::{ComponentTransactionState, SettledRegion};
use crate::mother_grid::region::descendant_faces;
use crate::mother_grid::{
    push_oriented, vertex_origin, MotherGrid, TriangleAddress, TriangleOrientation, VertexOrigin,
};
use earthmesh_mesh::{CartesianPoint, MeshState};
use std::collections::BTreeMap;

/// The whole sphere's final mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct AssembledSphere {
    pub mesh: MeshState,
    /// Origin of each vertex slot (the reserved slots have none).
    pub origins: Vec<Option<VertexOrigin>>,
    /// Delivered level of each active vertex, in slot order.
    pub delivered_levels: Vec<usize>,
}

/// The corner `corner` of the leaf `leaf` as `source_corner_site` finds it:
/// down the corner's children to the finest level, then that face's corner in
/// the order `generate` stores it.
fn finest_corner_origin(
    mut leaf: TriangleAddress,
    corner: usize,
    finest_n: usize,
) -> Result<VertexOrigin, String> {
    while leaf.n < finest_n {
        let children = leaf
            .children_2_to_1()
            .ok_or_else(|| format!("invalid hierarchy leaf {leaf:?}"))?;
        let child = match (leaf.orientation, corner) {
            (TriangleOrientation::Up, corner) => corner,
            (TriangleOrientation::Down, 0) => 0,
            (TriangleOrientation::Down, 1) => 2,
            (TriangleOrientation::Down, _) => 1,
        };
        leaf = children[child];
    }
    let mut corners = [VertexOrigin {
        face: 0,
        i: 0,
        j: 0,
    }; 3];
    for (slot, (i, j)) in corners
        .iter_mut()
        .zip(crate::mother_grid::region::face_lattice_corners(leaf))
    {
        *slot = vertex_origin(finest_n, leaf.base_face, i, j)?;
    }
    let positions = corners
        .iter()
        .map(|&origin| crate::mother_grid::region::origin_position(finest_n, origin))
        .collect::<Result<Vec<_>, _>>()?;
    // `generate` stores the face through `push_oriented`, which swaps the last
    // two corners of a negatively oriented face.
    let mut stored = vec![[0usize; 3]; 2];
    push_oriented(&mut stored, &positions, [0, 1, 2])?;
    Ok(corners[stored[2][corner]])
}

/// The whole sphere from a region run's final `state` over `source` (the
/// finest mother built over R and F), with the settled base faces at
/// `settled_n` and their cells at `settled_level`.
pub fn assemble_region_sphere(
    source: &MotherGrid,
    state: &ComponentTransactionState,
    settled: &SettledRegion,
    settled_n: usize,
    settled_level: usize,
) -> Result<AssembledSphere, String> {
    let region = source
        .region
        .as_ref()
        .ok_or_else(|| "the sphere is assembled from a built region".to_string())?;
    let finest_n = source.subdivision;
    let mesh = state.mesh();

    // Vertices by origin: the region's, at their final positions, then the
    // settled faces' corners, at their lattice positions.
    let mut vertices = BTreeMap::<VertexOrigin, (CartesianPoint, usize)>::new();
    for compact in mesh.mesh.active_vertex_slots() {
        let slot = mesh.source_vertex_slots[compact]
            .ok_or_else(|| format!("compact vertex {compact} has no source slot"))?;
        let origin = region
            .origin(slot)
            .ok_or_else(|| format!("source slot {slot} has no origin"))?;
        let level = state.source_delivered_levels()[slot]
            .ok_or_else(|| format!("source slot {slot} has no delivered level"))?;
        vertices.insert(origin, (mesh.mesh.vertices()[compact], level));
    }
    let settled_faces = descendant_faces(settled.faces(), settled_n)?;
    let mut settled_corners = BTreeMap::new();
    for &face in &settled_faces {
        let mut corners = [VertexOrigin {
            face: 0,
            i: 0,
            j: 0,
        }; 3];
        for (corner, slot) in corners.iter_mut().enumerate() {
            *slot = finest_corner_origin(face, corner, finest_n)?;
            vertices.entry(*slot).or_insert((
                crate::mother_grid::region::origin_position(finest_n, *slot)?,
                settled_level,
            ));
        }
        settled_corners.insert(face, corners);
    }
    let mut slot_of = BTreeMap::new();
    let mut lattice = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
    let mut positions = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
    let mut origins = vec![None, None];
    let mut delivered_levels = Vec::with_capacity(vertices.len());
    for (&origin, &(position, level)) in &vertices {
        slot_of.insert(origin, lattice.len());
        lattice.push(crate::mother_grid::region::origin_position(
            finest_n, origin,
        )?);
        positions.push(position);
        origins.push(Some(origin));
        delivered_levels.push(level);
    }

    // Leaves in address order, oriented at their lattice positions as the
    // rebuild orients them; then the transition triangles.
    let mut leaves = BTreeMap::new();
    for face in mesh.mesh.active_triangle_slots() {
        if let Some(address) = mesh.triangle_addresses[face] {
            leaves.insert(address, face);
        }
    }
    let mut triangles = vec![[1usize; 3]; 2];
    let mut leaf_order = leaves.keys().copied().collect::<Vec<_>>();
    leaf_order.extend(settled_corners.keys().copied());
    leaf_order.sort_unstable();
    for leaf in leaf_order {
        let corners = match settled_corners.get(&leaf) {
            Some(corners) => corners.map(|origin| slot_of[&origin]),
            None => {
                let mut corners = [0usize; 3];
                for (corner, slot) in corners.iter_mut().enumerate() {
                    let fine = super::core_condensation::source_corner_site(source, leaf, corner)?;
                    *slot = slot_of[&region
                        .origin(fine)
                        .ok_or_else(|| format!("source slot {fine} has no origin"))?];
                }
                corners
            }
        };
        push_oriented(&mut triangles, &lattice, corners)?;
    }
    for triangles_of_parent in state.custom_transition_triangles().values() {
        for &triangle in triangles_of_parent {
            let mut corners = [0usize; 3];
            for (corner, slot) in corners.iter_mut().zip(triangle) {
                *corner = slot_of[&region
                    .origin(slot)
                    .ok_or_else(|| format!("source slot {slot} has no origin"))?];
            }
            push_oriented(&mut triangles, &lattice, corners)?;
        }
    }
    let mesh = MeshState::from_parts(positions, triangles).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    Ok(AssembledSphere {
        mesh,
        origins,
        delivered_levels,
    })
}
