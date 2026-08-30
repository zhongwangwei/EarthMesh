//! Transactional retirement of one interior site from a triangulation.
//!
//! A degree-`d` fan leaves a `d`-sided hole. For supported degrees 3..=7 we
//! enumerate every triangulation of that ring, try each candidate on a clone,
//! keep the first one that validates, is locally Delaunay, and passes the
//! caller's postcondition, then swap it in. A rejected retirement never touches
//! the original mesh.

use std::collections::{BTreeMap, BTreeSet};

use crate::mesh_predicates::{in_circle_on_sphere, orientation_on_sphere, Ambiguous, Sign};
use crate::mesh_state::{FaceId, MeshState, MeshStateError, VertexId};
use crate::mesh_voronoi::VoronoiError;
use crate::{magnitude, spherical_triangle_area_unit, CartesianPoint};

/// Which ring diagonal closed a degree-four retired vertex's quadrilateral hole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetirementDiagonal {
    pub tail: usize,
    pub head: usize,
}

/// What one transactional retirement changed.
#[derive(Clone, Debug, PartialEq)]
pub struct RetirementReport {
    pub vertex: usize,
    pub vertex_id: VertexId,
    pub fan: Vec<usize>,
    pub ring: Vec<usize>,
    pub reused_faces: Vec<usize>,
    pub retired_faces: Vec<usize>,
    pub retired_face_ids: Vec<FaceId>,
    pub created_face_ids: Vec<FaceId>,
    pub replacement_faces: Vec<[usize; 3]>,
    pub diagonal: Option<RetirementDiagonal>,
}

/// Result of a bounded finite search over every triangulation of a retirement
/// ring. Only `Committed` mutates the mesh.
#[derive(Clone, Debug, PartialEq)]
pub enum RetirementSearchOutcome {
    Committed {
        report: RetirementReport,
        attempted: usize,
    },
    ProvenInfeasible {
        attempted: usize,
        last_error: Option<RetirementError>,
    },
    SearchBudgetExhausted {
        attempted: usize,
    },
    InvalidBoundary(RetirementError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetirementPostconditionOutcome {
    Accepted { states_examined: usize },
    Rejected { states_examined: usize },
    SearchBudgetExhausted { states_examined: usize },
}

impl RetirementPostconditionOutcome {
    fn states_examined(self) -> usize {
        match self {
            Self::Accepted { states_examined }
            | Self::Rejected { states_examined }
            | Self::SearchBudgetExhausted { states_examined } => states_examined,
        }
    }
}

/// Why a retirement did not commit.
#[derive(Clone, Debug, PartialEq)]
pub enum RetirementError {
    UnknownVertex { vertex: usize },
    NotDegreeFour { vertex: usize, degree: usize },
    UnsupportedDegree { vertex: usize, degree: usize },
    Fan(VoronoiError),
    RingIsNotAQuadrilateral { vertex: usize },
    RingIsNotAPolygon { vertex: usize, degree: usize },
    DegenerateCandidate { corners: [usize; 3] },
    Ambiguous(Ambiguous),
    Topology(Vec<MeshStateError>),
    IllegalLocalEdge { triangle: usize, corner: usize },
    AreaMismatch { before: f64, after: f64 },
    Rejected,
}

impl std::fmt::Display for RetirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVertex { vertex } => {
                write!(formatter, "mesh does not carry live vertex {vertex}")
            }
            Self::NotDegreeFour { vertex, degree } => {
                write!(formatter, "vertex {vertex} has degree {degree}, not 4")
            }
            Self::UnsupportedDegree { vertex, degree } => write!(
                formatter,
                "vertex {vertex} has degree {degree}; supported retirement degrees are 3..=7"
            ),
            Self::Fan(error) => write!(formatter, "cannot walk retirement fan: {error}"),
            Self::RingIsNotAQuadrilateral { vertex } => write!(
                formatter,
                "the fan around vertex {vertex} does not leave a four-ring"
            ),
            Self::RingIsNotAPolygon { vertex, degree } => write!(
                formatter,
                "the degree-{degree} fan around vertex {vertex} does not leave a simple ring"
            ),
            Self::DegenerateCandidate { corners } => {
                write!(formatter, "retirement candidate {corners:?} is degenerate")
            }
            Self::Ambiguous(error) => write!(formatter, "{error}"),
            Self::Topology(errors) => {
                write!(formatter, "retirement left invalid topology: {errors:?}")
            }
            Self::IllegalLocalEdge { triangle, corner } => write!(
                formatter,
                "retirement candidate leaves illegal edge opposite corner {corner} of triangle {triangle}"
            ),
            Self::AreaMismatch { before, after } => write!(
                formatter,
                "retirement changes spherical patch area from {before} to {after} steradians"
            ),
            Self::Rejected => write!(formatter, "retirement candidate rejected"),
        }
    }
}

impl std::error::Error for RetirementError {}

impl From<Ambiguous> for RetirementError {
    fn from(error: Ambiguous) -> Self {
        Self::Ambiguous(error)
    }
}

impl MeshState {
    /// Return the production retirement ring for one live degree-four vertex.
    pub fn degree_four_ring(&self, vertex: usize) -> Result<[usize; 4], RetirementError> {
        let seed = self
            .active_triangle_slots()
            .find(|&triangle| self.triangles()[triangle].contains(&vertex))
            .ok_or(RetirementError::NotDegreeFour { vertex, degree: 0 })?;
        let fan = self
            .triangle_fan_from(vertex, seed)
            .map_err(RetirementError::Fan)?;
        if fan.len() != 4 {
            return Err(RetirementError::NotDegreeFour {
                vertex,
                degree: fan.len(),
            });
        }
        four_ring(self, vertex, &fan)
    }

    /// Retire one live interior vertex of degree 3..=7 transactionally.
    pub fn retire_vertex_transactionally(
        &mut self,
        vertex: usize,
        postcondition: impl FnMut(&Self, &RetirementReport) -> bool,
    ) -> Result<RetirementReport, RetirementError> {
        match self.retire_vertex_with_budget_transactionally(vertex, usize::MAX, postcondition) {
            RetirementSearchOutcome::Committed { report, .. } => Ok(report),
            RetirementSearchOutcome::ProvenInfeasible { last_error, .. } => {
                Err(last_error.unwrap_or(RetirementError::Rejected))
            }
            RetirementSearchOutcome::InvalidBoundary(error) => Err(error),
            RetirementSearchOutcome::SearchBudgetExhausted { .. } => {
                unreachable!("usize::MAX exceeds the finite degree-3..=7 triangulation space")
            }
        }
    }

    /// Search at most `search_budget` ring triangulations and commit the first
    /// one that passes the mesh checks and caller postcondition.
    pub fn retire_vertex_with_budget_transactionally(
        &mut self,
        vertex: usize,
        search_budget: usize,
        mut postcondition: impl FnMut(&Self, &RetirementReport) -> bool,
    ) -> RetirementSearchOutcome {
        self.retire_vertex_with_budget_impl(
            vertex,
            0,
            search_budget,
            true,
            true,
            |state, report, _| {
                if postcondition(state, report) {
                    RetirementPostconditionOutcome::Accepted { states_examined: 0 }
                } else {
                    RetirementPostconditionOutcome::Rejected { states_examined: 0 }
                }
            },
        )
    }

    /// Search finite ring triangulations while allowing the caller's atomic
    /// postcondition to repair Delaunay edges before accepting a candidate.
    pub fn retire_vertex_with_budget_transactionally_repairing(
        &mut self,
        vertex: usize,
        search_budget: usize,
        postcondition: impl FnMut(&Self, &RetirementReport, usize) -> RetirementPostconditionOutcome,
    ) -> RetirementSearchOutcome {
        self.retire_vertex_with_budget_impl(vertex, 0, search_budget, false, true, postcondition)
    }

    /// Search finite ring triangulations from absolute candidate ordinal
    /// `start_index` up to exclusive absolute cap `search_budget`.
    pub fn retire_vertex_from_cursor_with_budget_transactionally_repairing(
        &mut self,
        vertex: usize,
        start_index: usize,
        search_budget: usize,
        postcondition: impl FnMut(&Self, &RetirementReport, usize) -> RetirementPostconditionOutcome,
    ) -> RetirementSearchOutcome {
        self.retire_vertex_with_budget_impl(
            vertex,
            start_index,
            search_budget,
            false,
            false,
            postcondition,
        )
    }

    fn retire_vertex_with_budget_impl(
        &mut self,
        vertex: usize,
        start_index: usize,
        search_budget: usize,
        require_local_delaunay: bool,
        count_postcondition_states: bool,
        mut postcondition: impl FnMut(&Self, &RetirementReport, usize) -> RetirementPostconditionOutcome,
    ) -> RetirementSearchOutcome {
        let vertex_id = self
            .vertex_id(vertex)
            .ok_or(RetirementError::UnknownVertex { vertex });
        let Ok(vertex_id) = vertex_id else {
            return RetirementSearchOutcome::InvalidBoundary(vertex_id.unwrap_err());
        };
        let seed = self
            .active_triangle_slots()
            .find(|&triangle| self.triangles()[triangle].contains(&vertex))
            .ok_or(RetirementError::UnsupportedDegree { vertex, degree: 0 });
        let Ok(seed) = seed else {
            return RetirementSearchOutcome::InvalidBoundary(seed.unwrap_err());
        };
        let fan = self
            .triangle_fan_from(vertex, seed)
            .map_err(RetirementError::Fan);
        let Ok(fan) = fan else {
            return RetirementSearchOutcome::InvalidBoundary(fan.unwrap_err());
        };
        if !(3..=7).contains(&fan.len()) {
            return RetirementSearchOutcome::InvalidBoundary(RetirementError::UnsupportedDegree {
                vertex,
                degree: fan.len(),
            });
        }
        let ring = polygon_ring(self, vertex, &fan);
        let Ok(ring) = ring else {
            return RetirementSearchOutcome::InvalidBoundary(ring.unwrap_err());
        };
        let outside = outside_faces_from_fan(self, vertex, &fan);
        let candidates = triangulations(&ring);
        if start_index >= search_budget {
            return RetirementSearchOutcome::SearchBudgetExhausted {
                attempted: search_budget,
            };
        }
        if start_index >= candidates.len() {
            return RetirementSearchOutcome::ProvenInfeasible {
                attempted: candidates.len(),
                last_error: None,
            };
        }

        let mut last_error = None;
        let mut attempted = start_index;
        for (ordinal, replacement) in candidates.into_iter().enumerate().skip(start_index) {
            if attempted == search_budget
                || (!count_postcondition_states && ordinal == search_budget)
            {
                return RetirementSearchOutcome::SearchBudgetExhausted {
                    attempted: search_budget,
                };
            }
            let ring_attempted = ordinal + 1;
            attempted = if count_postcondition_states {
                attempted + 1
            } else {
                ring_attempted
            };
            let remaining = search_budget - attempted;
            let mut decision = None;
            let result = try_retirement(
                self,
                vertex,
                vertex_id,
                &fan,
                &ring,
                &outside,
                &replacement,
                require_local_delaunay,
                &mut |state, report| {
                    let next = postcondition(state, report, remaining);
                    let accepts = matches!(next, RetirementPostconditionOutcome::Accepted { .. })
                        && (!count_postcondition_states || next.states_examined() <= remaining);
                    decision = Some(next);
                    accepts
                },
            );
            if let Some(decision) = decision {
                let states_examined = decision.states_examined();
                if count_postcondition_states && states_examined > remaining {
                    return RetirementSearchOutcome::SearchBudgetExhausted {
                        attempted: search_budget,
                    };
                }
                if count_postcondition_states {
                    attempted += states_examined;
                }
                if matches!(
                    decision,
                    RetirementPostconditionOutcome::SearchBudgetExhausted { .. }
                ) {
                    return RetirementSearchOutcome::SearchBudgetExhausted { attempted };
                }
            }
            match result {
                Ok(report) => {
                    return RetirementSearchOutcome::Committed { report, attempted };
                }
                Err(error) => last_error = Some(error),
            }
        }
        RetirementSearchOutcome::ProvenInfeasible {
            attempted,
            last_error,
        }
    }

    /// Retire one live interior degree-four vertex transactionally.
    pub fn retire_degree_four_vertex_transactionally(
        &mut self,
        vertex: usize,
        postcondition: impl FnMut(&Self, &RetirementReport) -> bool,
    ) -> Result<RetirementReport, RetirementError> {
        self.degree_four_ring(vertex)?;
        self.retire_vertex_transactionally(vertex, postcondition)
    }
}

fn try_retirement(
    state: &mut MeshState,
    vertex: usize,
    vertex_id: VertexId,
    fan: &[usize],
    ring: &[usize],
    outside: &[usize],
    replacement: &[[usize; 3]],
    require_local_delaunay: bool,
    postcondition: &mut impl FnMut(&MeshState, &RetirementReport) -> bool,
) -> Result<RetirementReport, RetirementError> {
    let mut trial = state.clone();
    let report = retire_on_trial(
        &mut trial,
        vertex,
        vertex_id,
        fan,
        ring,
        outside,
        replacement,
        require_local_delaunay,
    )?;
    if !postcondition(&trial, &report) {
        return Err(RetirementError::Rejected);
    }
    *state = trial;
    Ok(report)
}

fn retire_on_trial(
    state: &mut MeshState,
    vertex: usize,
    vertex_id: VertexId,
    fan: &[usize],
    ring: &[usize],
    outside: &[usize],
    replacement: &[[usize; 3]],
    require_local_delaunay: bool,
) -> Result<RetirementReport, RetirementError> {
    let before = orientation_on_sphere(
        state.vertices()[state.triangles()[fan[0]][0]],
        state.vertices()[state.triangles()[fan[0]][1]],
        state.vertices()[state.triangles()[fan[0]][2]],
    )?;
    let new_faces = replacement
        .iter()
        .copied()
        .map(|corners| oriented(state, before, corners))
        .collect::<Result<Vec<_>, _>>()?;
    let before_area = fan
        .iter()
        .try_fold(0.0, |sum, &face| {
            Some(sum + triangle_area_on_unit_sphere(state, state.triangles()[face])?)
        })
        .ok_or(RetirementError::Rejected)?;
    let after_area = new_faces
        .iter()
        .try_fold(0.0, |sum, &face| {
            Some(sum + triangle_area_on_unit_sphere(state, face)?)
        })
        .ok_or(RetirementError::Rejected)?;
    if !before_area.is_finite()
        || !after_area.is_finite()
        || (after_area - before_area).abs() > 1.0e-10_f64.max(before_area * 1.0e-8)
    {
        return Err(RetirementError::AreaMismatch {
            before: before_area,
            after: after_area,
        });
    }

    let reused_faces = fan
        .iter()
        .copied()
        .take(new_faces.len())
        .collect::<Vec<_>>();
    let retired_faces = fan
        .iter()
        .copied()
        .skip(new_faces.len())
        .collect::<Vec<_>>();
    let retired_face_ids = retired_faces
        .iter()
        .map(|&face| state.face_id(face).expect("live fan face"))
        .collect::<Vec<_>>();

    for (&face, corners) in reused_faces.iter().zip(new_faces.iter().copied()) {
        state.set_triangle(face, corners);
    }
    for &face in &retired_faces {
        state.retire_triangle_slot(face);
    }
    state.retire_vertex_slot(vertex);

    let authoritative: BTreeSet<_> = reused_faces.iter().copied().collect();
    let mut region = authoritative.clone();
    region.extend(outside.iter().copied());
    state.repair_adjacency_across(&region, &authoritative);

    if require_local_delaunay {
        state
            .legalize_within(&authoritative, Some(&authoritative))
            .map_err(|_| RetirementError::Rejected)?;
        if let Some((triangle, corner)) = illegal_edge_in(state, &authoritative) {
            return Err(RetirementError::IllegalLocalEdge { triangle, corner });
        }
    }

    let mut affected = region;
    affected.extend(authoritative.iter().copied());
    state
        .validate_region(&affected)
        .map_err(RetirementError::Topology)?;

    let created_face_ids = reused_faces
        .iter()
        .map(|&face| state.face_id(face).expect("reused face stayed live"))
        .collect();
    Ok(RetirementReport {
        vertex,
        vertex_id,
        fan: fan.to_vec(),
        ring: ring.to_vec(),
        reused_faces,
        retired_faces,
        retired_face_ids,
        created_face_ids,
        replacement_faces: new_faces.clone(),
        diagonal: degree_four_diagonal(ring, &new_faces),
    })
}

fn triangle_area_on_unit_sphere(state: &MeshState, corners: [usize; 3]) -> Option<f64> {
    let mut points = corners.map(|corner| state.vertices()[corner]);
    for point in &mut points {
        let norm = magnitude(*point);
        if !norm.is_finite() || norm <= f64::EPSILON {
            return None;
        }
        *point = CartesianPoint::new(point.x / norm, point.y / norm, point.z / norm);
    }
    Some(spherical_triangle_area_unit(points).abs())
}

fn oriented(
    state: &MeshState,
    sign: Sign,
    corners: [usize; 3],
) -> Result<[usize; 3], RetirementError> {
    if sign == Sign::Zero {
        return Err(RetirementError::DegenerateCandidate { corners });
    }
    let candidate = orientation_on_sphere(
        state.vertices()[corners[0]],
        state.vertices()[corners[1]],
        state.vertices()[corners[2]],
    )?;
    match candidate {
        Sign::Zero => Err(RetirementError::DegenerateCandidate { corners }),
        _ if candidate == sign => Ok(corners),
        _ => Ok([corners[0], corners[2], corners[1]]),
    }
}

fn polygon_ring(
    state: &MeshState,
    vertex: usize,
    fan: &[usize],
) -> Result<Vec<usize>, RetirementError> {
    let mut neighbours: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &face in fan {
        let edge = state.triangles()[face]
            .into_iter()
            .filter(|&corner| corner != vertex)
            .collect::<Vec<_>>();
        if edge.len() != 2 {
            return Err(RetirementError::RingIsNotAPolygon {
                vertex,
                degree: fan.len(),
            });
        }
        neighbours.entry(edge[0]).or_default().push(edge[1]);
        neighbours.entry(edge[1]).or_default().push(edge[0]);
    }
    if neighbours.len() != fan.len() || neighbours.values().any(|list| list.len() != 2) {
        return Err(RetirementError::RingIsNotAPolygon {
            vertex,
            degree: fan.len(),
        });
    }
    let start = *neighbours
        .keys()
        .next()
        .expect("polygon ring has a first vertex");
    let mut ring = Vec::with_capacity(fan.len());
    let mut previous = usize::MAX;
    let mut current = start;
    for _ in 0..fan.len() {
        ring.push(current);
        let next = neighbours[&current]
            .iter()
            .copied()
            .filter(|&candidate| candidate != previous)
            .min()
            .ok_or(RetirementError::RingIsNotAPolygon {
                vertex,
                degree: fan.len(),
            })?;
        previous = current;
        current = next;
    }
    if current != start {
        return Err(RetirementError::RingIsNotAPolygon {
            vertex,
            degree: fan.len(),
        });
    }
    Ok(ring)
}

fn four_ring(
    state: &MeshState,
    vertex: usize,
    fan: &[usize],
) -> Result<[usize; 4], RetirementError> {
    let ring = polygon_ring(state, vertex, fan)
        .map_err(|_| RetirementError::RingIsNotAQuadrilateral { vertex })?;
    if ring.len() != 4 {
        return Err(RetirementError::RingIsNotAQuadrilateral { vertex });
    }
    Ok([ring[0], ring[1], ring[2], ring[3]])
}

fn triangulations(ring: &[usize]) -> Vec<Vec<[usize; 3]>> {
    if ring.len() < 3 {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    let last = ring.len() - 1;
    for split in 1..last {
        for left in triangulations(&ring[..=split]) {
            for right in triangulations(&ring[split..]) {
                let mut candidate = Vec::with_capacity(ring.len() - 2);
                candidate.extend(left.iter().copied());
                candidate.extend(right.iter().copied());
                candidate.push([ring[0], ring[split], ring[last]]);
                all.push(candidate);
            }
        }
    }
    all
}

fn degree_four_diagonal(ring: &[usize], faces: &[[usize; 3]]) -> Option<RetirementDiagonal> {
    if ring.len() != 4 || faces.len() != 2 {
        return None;
    }
    let ring_edges = (0..4)
        .map(|i| edge_key(ring[i], ring[(i + 1) % 4]))
        .collect::<BTreeSet<_>>();
    for face in faces {
        for corner in 0..3 {
            let edge = edge_key(face[(corner + 1) % 3], face[(corner + 2) % 3]);
            if !ring_edges.contains(&edge) {
                return Some(RetirementDiagonal {
                    tail: edge.0,
                    head: edge.1,
                });
            }
        }
    }
    None
}

fn outside_faces_from_fan(state: &MeshState, vertex: usize, fan: &[usize]) -> Vec<usize> {
    let fan_set: BTreeSet<_> = fan.iter().copied().collect();
    let mut outside = BTreeSet::new();
    for &face in fan {
        let Some(corner) = state.triangles()[face]
            .iter()
            .position(|&candidate| candidate == vertex)
        else {
            continue;
        };
        let neighbour = state.neighbours()[face][corner];
        if state.is_triangle_live(neighbour) && !fan_set.contains(&neighbour) {
            outside.insert(neighbour);
        }
    }
    outside.into_iter().collect()
}

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn illegal_edge_in(state: &MeshState, region: &BTreeSet<usize>) -> Option<(usize, usize)> {
    for &triangle in region {
        if !state.is_triangle_live(triangle) {
            continue;
        }
        let here = state.triangles()[triangle];
        for corner in 0..3 {
            let neighbour = state.neighbours()[triangle][corner];
            if !state.is_triangle_live(neighbour) {
                continue;
            }
            let there = state.triangles()[neighbour];
            let Some(opposite) = there.iter().copied().find(|c| !here.contains(c)) else {
                continue;
            };
            if in_circle_on_sphere(
                state.vertices()[here[0]],
                state.vertices()[here[1]],
                state.vertices()[here[2]],
                state.vertices()[opposite],
            ) == Ok(Sign::Positive)
            {
                return Some((triangle, corner));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
