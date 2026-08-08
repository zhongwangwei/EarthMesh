//! The carved domain's boundary, as the neutral model rather than as curves.
//!
//! The carve already walks the domain edge into closed curves; what it does not
//! do is say which curve is the coast and which is a lake inside an island, or
//! check that the set is a boundary at all. `earthmesh_boundary` holds both --
//! `LoopType`, the nesting, and the invariants -- and this is what puts a run's
//! actual boundary into it.
//!
//! # Why here rather than in a backend
//!
//! The carve runs after whichever backend refined, so this sees the same shape
//! whichever one it was. A check written inside Method-C would have to be
//! written again for red-green and again for HARP-DV, and the third copy is
//! where they start to disagree.
//!
//! # What `topology_counts` is for
//!
//! Outer loops and holes are the pair a refinement must leave unchanged. A run
//! that ends with fewer outer loops has removed an island; one with fewer holes
//! has filled in a lake or closed a channel. Either is a mesh that is valid,
//! passes its quality checks, and is not the domain that was asked for -- the
//! failure class section 11.1 of the guide is about. Counting them is what
//! makes that sayable.

use std::io;

use earthmesh_mesh::RefinementRegion;

use earthmesh_boundary::{
    BoundaryLoop, BoundaryRole, BoundaryVertex, LoopType, SphericalBoundaryModel,
};

/// A point on the mesh, as the carve holds it.
pub trait BoundaryPointSource {
    /// Longitude and latitude of a vertex, or `None` if it is not one.
    fn lonlat_degrees(&self, vertex: usize) -> Option<(f64, f64)>;
}

impl<F> BoundaryPointSource for F
where
    F: Fn(usize) -> Option<(f64, f64)>,
{
    fn lonlat_degrees(&self, vertex: usize) -> Option<(f64, f64)> {
        self(vertex)
    }
}

/// Build the neutral boundary model from the carve's closed curves.
///
/// `curves` are rings of mesh vertex ids in traversal order, as
/// `BoundaryClosedCurves::close_curves` gives them -- slot 0 is Canonical's
/// placeholder and is skipped, and a ring's first vertex may be repeated at the
/// end, which the model does not want.
///
/// Rings are classified by nesting: a ring every one of whose vertices lies
/// inside another ring is that ring's hole. Orientation is fixed here rather
/// than assumed, because a ring walked off a mesh has no inherent direction and
/// `contains` reads direction to decide a side -- so each outer ring is turned
/// until it encloses its own interior, and each hole until it encloses its own.
pub fn boundary_model_from_closed_curves(
    curves: &[Vec<usize>],
    points: &impl BoundaryPointSource,
) -> io::Result<SphericalBoundaryModel> {
    let mut rings: Vec<Vec<usize>> = Vec::new();
    let mut vertices: Vec<BoundaryVertex> = Vec::new();
    let mut index_of: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();

    for curve in curves.iter().skip(1) {
        let mut ring = Vec::with_capacity(curve.len());
        for &vertex in curve {
            let Some((lon, lat)) = points.lonlat_degrees(vertex) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary curve names vertex {vertex}, which carries no position"),
                ));
            };
            let slot = *index_of.entry(vertex).or_insert_with(|| {
                vertices.push(BoundaryVertex {
                    lon_degrees: lon,
                    lat_degrees: lat,
                    // Every carved boundary point is a corner of the domain and
                    // not free to be moved; a run that slides one has moved the
                    // coast.
                    pinned: true,
                });
                vertices.len() - 1
            });
            // The walker may close the ring by repeating its first vertex. The
            // model closes implicitly, so the repeat has to go or `validate`
            // reports a pinch that is not there.
            if ring.last() != Some(&slot) && ring.first() != Some(&slot) {
                ring.push(slot);
            }
        }
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }

    Ok(nest_and_orient(vertices, rings, BoundaryRole::HardDomain))
}

/// Nest the rings, orient each one, and give them all the same role.
///
/// Shared by the two producers because the shape of the answer is the same
/// whichever curves came in: a ring inside another is that one's hole, and a
/// ring walked off data carries no direction. Only the role differs, and it
/// differs for a reason each caller states.
fn nest_and_orient(
    vertices: Vec<BoundaryVertex>,
    rings: Vec<Vec<usize>>,
    role: BoundaryRole,
) -> SphericalBoundaryModel {
    let mut model = SphericalBoundaryModel {
        vertices,
        loops: Vec::new(),
    };
    // Oriented by the type, not by a helper the caller has to remember. A ring
    // read off data carries no direction, and `contains` reads direction to
    // pick a side -- so the loop is built through the constructor that makes
    // that choice rather than assembled and then corrected.
    let oriented: Vec<Vec<usize>> = rings
        .iter()
        .map(|ring| {
            BoundaryLoop::bounding_smaller_side(
                LoopType::Outer,
                role,
                ring.clone(),
                None,
                &model.vertices,
            )
            .map(|ring| ring.vertices().to_vec())
            .unwrap_or_else(|| ring.clone())
        })
        .collect();
    let mut parents: Vec<Option<usize>> = vec![None; oriented.len()];
    for (index, ring) in oriented.iter().enumerate() {
        let inside_of: Vec<usize> = oriented
            .iter()
            .enumerate()
            .filter(|(other, candidate)| *other != index && ring_is_inside(&model, ring, candidate))
            .map(|(other, _)| other)
            .collect();
        // Directly inside one ring is a hole of it. Inside two means a hole of
        // the innermost, and the model forbids a hole in a hole -- so anything
        // nested deeper than one level is left as its own outer loop rather
        // than producing a model that cannot validate.
        if inside_of.len() == 1 {
            parents[index] = Some(inside_of[0]);
        }
    }
    model.loops = oriented
        .into_iter()
        .enumerate()
        .map(|(index, vertices)| {
            BoundaryLoop::counter_clockwise(
                if parents[index].is_some() {
                    LoopType::Hole
                } else {
                    LoopType::Outer
                },
                role,
                vertices,
                parents[index],
            )
        })
        .collect();
    // A hole whose parent turned out to be a hole would break the model's
    // invariant, so it is promoted rather than emitted.
    for index in 0..model.loops.len() {
        if let Some(parent) = model.loops[index].parent {
            if model.loops[parent].loop_type == LoopType::Hole {
                model.loops[index].loop_type = LoopType::Outer;
                model.loops[index].parent = None;
            }
        }
    }
    model
}

/// Whether every vertex of `ring` lies inside `candidate`.
fn ring_is_inside(model: &SphericalBoundaryModel, ring: &[usize], candidate: &[usize]) -> bool {
    let probe = SphericalBoundaryModel {
        vertices: model.vertices.clone(),
        loops: vec![BoundaryLoop::counter_clockwise(
            LoopType::Outer,
            BoundaryRole::HardDomain,
            candidate.to_vec(),
            None,
        )],
    };
    ring.iter().all(|&vertex| {
        model
            .vertices
            .get(vertex)
            .is_some_and(|point| probe.contains(point.lon_degrees, point.lat_degrees))
    })
}

/// The boundary model a run's own regions describe.
///
/// The producer the crate was missing. Until now the model could only be built
/// from curves a carve had already walked, so it could *describe* a carve's
/// result and never *constrain* a refinement -- which is what a type carrying
/// "this segment may be split but never crossed" exists for.
///
/// Closed-curve refinement masks are where a run's real curves come in: a
/// `.nml` or `.nc4` close mask becomes a `RefinementRegion::Polygon`, and that
/// is a ring of lon/lat points with a level attached.
///
/// # The role is `RefinementGuide`, and that is not a placeholder
///
/// These curves say "refine inside here". They are not coastlines: nothing
/// forbids a cell from crossing one, and an edge lying on one may be flipped
/// away without losing anything -- which is exactly what
/// [`BoundaryRole::permits_edge_flip`] returns for a guide and for nothing
/// else. Marking them `HardDomain` would claim the refinement must not cross
/// its own refinement region, which is false and would eventually be acted on.
///
/// Only polygons produce loops. A circle, a box or a corridor is a region
/// whose boundary the run never discretised, and inventing a ring for one here
/// would put a curve in the model that no data behind it agrees with.
pub fn boundary_model_from_regions(regions: &[RefinementRegion]) -> SphericalBoundaryModel {
    let mut vertices: Vec<BoundaryVertex> = Vec::new();
    let mut rings: Vec<Vec<usize>> = Vec::new();
    for region in regions {
        let RefinementRegion::Polygon { points, .. } = region else {
            continue;
        };
        let mut ring = Vec::with_capacity(points.len());
        for point in points {
            let slot = vertices.len();
            vertices.push(BoundaryVertex {
                lon_degrees: point.lon_degrees,
                lat_degrees: point.lat_degrees,
                // A mask's vertices are where the data put them. Refinement may
                // add points along the curve but must not slide these.
                pinned: true,
            });
            // A mask often closes its ring by repeating the first point; the
            // model closes implicitly, and the repeat would read as a pinch.
            let repeats_first = ring.first().is_some_and(|&first: &usize| {
                let (a, b) = (&vertices[first], &vertices[slot]);
                (a.lon_degrees - b.lon_degrees).abs() < 1.0e-12
                    && (a.lat_degrees - b.lat_degrees).abs() < 1.0e-12
            });
            if repeats_first {
                vertices.pop();
                continue;
            }
            ring.push(slot);
        }
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }
    nest_and_orient(vertices, rings, BoundaryRole::RefinementGuide)
}

#[cfg(test)]
mod tests;
