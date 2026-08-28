//! Where to put a site, and what to try when that place is refused.
//!
//! Specification sections 12.1 and 13.4. A demand names a cell; this turns it
//! into an ordered list of points to attempt. The transaction layer walks the
//! list and stops at the first one that survives the gates.
//!
//! # The order is the ladder, and it ends
//!
//! Witness, then density-weighted farthest point, then spherical off-centre,
//! then longest-edge midpoint. Earlier entries answer the demand more directly;
//! later ones are likelier to be legal. Section 13.4 is explicit that the last
//! candidate is not committed unconditionally -- a demand that exhausts the
//! ladder is unresolved, and a mesh that took a point nobody checked is worse
//! than a mesh that did not refine.
//!
//! # Why every candidate is put back on the sphere
//!
//! Midpoints and circumcentres come out inside it: the midpoint of a chord
//! sits below the surface. `insert_site` refuses a point off the mesh's sphere
//! -- it is the failure that otherwise produces a valid, closed, non-Delaunay
//! mesh -- so a generator that did not project would produce nothing but
//! refusals, and the ladder would report the demand unresolvable for a reason
//! that has nothing to do with the demand.
//!
//! The radius is taken locally, from the site being refined, for the same
//! reason the guard measures locally: a relaxed mesh's sites are not all at one
//! radius, and a global mean would place every candidate slightly off its own
//! neighbourhood's surface.

use earthmesh_mesh::{arc_length_unit_sphere, magnitude, CartesianPoint, MeshState, VoronoiError};

/// Which rung of the ladder produced a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateSource {
    /// The criterion's own point: where the evidence says the mesh is wrong.
    Witness,
    /// The cell corner farthest from the site, which is where the cell is
    /// least well represented by it.
    FarthestPoint,
    /// Along the arc from the site toward that corner, stopped short.
    ///
    /// The spherical reading of an off-centre: a circumcentre inserted whole
    /// tends to produce a thin triangle against the far side of the cavity, and
    /// stopping short of it trades some of the improvement for an angle.
    OffCentre,
    /// The midpoint of the longest edge at the site.
    ///
    /// The most conservative rung: it splits an edge that already exists rather
    /// than proposing a point in open space, so it disturbs the least.
    LongestEdgeMidpoint,
    /// A second off-centre position tried only after the ordinary ladder made
    /// no progress for a whole cycle.
    AdaptiveOffCentre,
    /// The midpoint of an incident edge other than the single longest one,
    /// tried only by the stalled-cycle fallback.
    IncidentEdgeMidpoint,
}

/// A point to try, and where it came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub point: CartesianPoint,
    pub source: CandidateSource,
    /// A triangle at the site, to start the location walk from.
    pub hint: usize,
}

/// What a candidate has to satisfy before it is worth proposing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidatePolicy {
    /// How far a candidate must be from every site of the cell it refines.
    ///
    /// Zero would let a candidate land on a site, which `insert_site` refuses
    /// as a duplicate; a little more than zero keeps the triangulation from
    /// gaining slivers that pass every topology check and ruin the quality
    /// metrics.
    pub min_separation_m: f64,
}

impl Default for CandidatePolicy {
    fn default() -> Self {
        Self {
            min_separation_m: earthmesh_core::DEFAULT_HARP_DV_MINIMUM_CANDIDATE_SEPARATION_M,
        }
    }
}

/// How far along the arc toward the farthest corner an off-centre sits.
///
/// Two thirds: far enough to move the cell meaningfully, short enough that the
/// new site is not on top of the corner the farthest-point rung already tried.
/// A rung that duplicates the one above it costs a proposal and answers nothing.
const OFF_CENTRE_FRACTION: f64 = 2.0 / 3.0;

/// Put a point on the sphere the site lives on.
fn onto_sphere(point: CartesianPoint, radius: f64) -> Option<CartesianPoint> {
    let length = magnitude(point);
    if !length.is_finite() || length <= 0.0 || !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    Some(CartesianPoint::new(
        point.x / length * radius,
        point.y / length * radius,
        point.z / length * radius,
    ))
}

/// A point a fraction of the way along the great-circle arc from `from` to
/// `to`, at `from`'s radius.
fn along_arc(from: CartesianPoint, to: CartesianPoint, fraction: f64) -> Option<CartesianPoint> {
    let radius = magnitude(from);
    let blended = CartesianPoint::new(
        from.x + (to.x - from.x) * fraction,
        from.y + (to.y - from.y) * fraction,
        from.z + (to.z - from.z) * fraction,
    );
    onto_sphere(blended, radius)
}

/// The ladder for one site, in the order it should be tried.
///
/// `witness` is what the criterion flagged, if it flagged a place rather than
/// only a cell. It goes first and is not projected differently from the rest:
/// a criterion working in lon/lat produces a point on the sphere already, and
/// one that does not should be corrected rather than quietly moved.
pub fn candidates_for_site(
    state: &MeshState,
    site: usize,
    witness: Option<CartesianPoint>,
    policy: CandidatePolicy,
) -> Result<Vec<Candidate>, VoronoiError> {
    let cell = state.voronoi_cell(site)?;
    let centre = state.vertices()[site];
    let radius = magnitude(centre);
    let mut ladder = Vec::with_capacity(4);
    let hint = *cell.triangles.first().expect("a cell has triangles");

    if let Some(point) = witness {
        ladder.push(Candidate {
            point,
            source: CandidateSource::Witness,
            hint,
        });
    }

    // The corner farthest from the site: where the cell is least well
    // represented by the site that owns it.
    let farthest = cell
        .corners
        .iter()
        .copied()
        .max_by(|left, right| {
            arc_length_unit_sphere(centre, *left)
                .partial_cmp(&arc_length_unit_sphere(centre, *right))
                // Ties resolve by coordinate rather than by iteration order, so
                // two runs over the same mesh agree.
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    (left.x, left.y, left.z)
                        .partial_cmp(&(right.x, right.y, right.z))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .and_then(|corner| onto_sphere(corner, radius));
    if let Some(point) = farthest {
        ladder.push(Candidate {
            point,
            source: CandidateSource::FarthestPoint,
            hint,
        });
        if let Some(off_centre) = along_arc(centre, point, OFF_CENTRE_FRACTION) {
            ladder.push(Candidate {
                point: off_centre,
                source: CandidateSource::OffCentre,
                hint,
            });
        }
    }

    // The longest edge at the site, split in the middle.
    let neighbours: Vec<usize> = cell
        .triangles
        .iter()
        .flat_map(|&triangle| state.triangles()[triangle])
        .filter(|&corner| corner != site)
        .collect();
    let longest = neighbours
        .iter()
        .map(|&other| state.vertices()[other])
        .max_by(|left, right| {
            arc_length_unit_sphere(centre, *left)
                .partial_cmp(&arc_length_unit_sphere(centre, *right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    (left.x, left.y, left.z)
                        .partial_cmp(&(right.x, right.y, right.z))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .and_then(|other| along_arc(centre, other, 0.5));
    if let Some(point) = longest {
        ladder.push(Candidate {
            point,
            source: CandidateSource::LongestEdgeMidpoint,
            hint,
        });
    }

    // Section 12.2's separation constraint, against the cell's own sites --
    // which are the only ones near enough to violate it.
    let mut nearby: Vec<CartesianPoint> = neighbours
        .iter()
        .map(|&other| state.vertices()[other])
        .collect();
    nearby.push(centre);
    ladder.retain(|candidate| {
        nearby
            .iter()
            .all(|site| arc_length_unit_sphere(*site, candidate.point) >= policy.min_separation_m)
    });
    Ok(ladder)
}

/// Extra deterministic positions for a demand the ordinary ladder could not
/// serve during a whole cycle.
///
/// Kept out of [`candidates_for_site`] so productive cycles still pay for the
/// short ladder. A stalled run can afford a broader search over the same cell:
/// two off-centre fractions for every Voronoi corner and every incident edge
/// midpoint, excluding points the ordinary ladder already tried.
pub(crate) fn fallback_candidates_for_site(
    state: &MeshState,
    site: usize,
    policy: CandidatePolicy,
) -> Result<Vec<Candidate>, VoronoiError> {
    let cell = state.voronoi_cell(site)?;
    let centre = state.vertices()[site];
    let hint = *cell.triangles.first().expect("a cell has triangles");
    let ordinary = candidates_for_site(state, site, None, policy)?;
    let neighbours: std::collections::BTreeSet<usize> = cell
        .triangles
        .iter()
        .flat_map(|&triangle| state.triangles()[triangle])
        .filter(|&corner| corner != site)
        .collect();
    let mut fallback = Vec::new();

    for corner in cell.corners.iter().copied() {
        for fraction in [0.8, 0.5] {
            if let Some(point) = along_arc(centre, corner, fraction) {
                fallback.push(Candidate {
                    point,
                    source: CandidateSource::AdaptiveOffCentre,
                    hint,
                });
            }
        }
    }
    for neighbour in neighbours.iter().copied() {
        if let Some(point) = along_arc(centre, state.vertices()[neighbour], 0.5) {
            fallback.push(Candidate {
                point,
                source: CandidateSource::IncidentEdgeMidpoint,
                hint,
            });
        }
    }

    let mut nearby: Vec<CartesianPoint> = neighbours
        .iter()
        .map(|&other| state.vertices()[other])
        .collect();
    nearby.push(centre);
    fallback.retain(|candidate| {
        ordinary.iter().all(|tried| tried.point != candidate.point)
            && nearby.iter().all(|point| {
                arc_length_unit_sphere(*point, candidate.point) >= policy.min_separation_m
            })
    });
    fallback.dedup_by(|left, right| left.point == right.point);
    Ok(fallback)
}

#[cfg(test)]
mod tests;
