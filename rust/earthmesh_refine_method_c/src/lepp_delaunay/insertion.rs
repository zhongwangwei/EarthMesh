use std::cell::Cell;
use std::collections::BTreeSet;

use earthmesh_boundary::SegmentList;
use earthmesh_mesh::{
    magnitude, CartesianPoint, FaceId as StableFaceId, InsertionReport, InsertionTransactionError,
    MeshState, VertexId, MESH_STATE_FIRST_ID,
};

use super::{find_lepp, LeppEdgeId, LeppPath, LeppSearchConfig, LeppSearchError, LeppTerminal};

/// Hard gates for one terminal-edge insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeppInsertionGates {
    pub maximum_vertex_degree: usize,
    pub protected_vertices: Vec<usize>,
}

impl Default for LeppInsertionGates {
    fn default() -> Self {
        Self {
            maximum_vertex_degree: 7,
            protected_vertices: Vec::new(),
        }
    }
}

impl LeppInsertionGates {
    pub fn for_method_c(protected_pentagons: [usize; 12]) -> Self {
        Self {
            protected_vertices: protected_pentagons.to_vec(),
            ..Self::default()
        }
    }
}

/// Which edge a constrained LEPP insertion actually split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeppInsertionSplitReason {
    TerminalEdge,
    EncroachedSegment,
}

/// One committed LEPP terminal-edge insertion.
#[derive(Clone, Debug, PartialEq)]
pub struct LeppInsertionReport {
    pub path: LeppPath,
    pub requested_edge: LeppEdgeId,
    pub split_edge: LeppEdgeId,
    pub split_reason: LeppInsertionSplitReason,
    pub point: CartesianPoint,
    pub insertion: InsertionReport,
    pub affected_sites: Vec<VertexId>,
    pub created_faces: Vec<StableFaceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GateFailure {
    OpenEdges(usize),
    MissingSegment(LeppEdgeId),
    Degree {
        vertex: usize,
        degree: usize,
    },
    ProtectedDegree {
        vertex: usize,
        before: usize,
        after: usize,
    },
    UnmeasurableDegree {
        vertex: usize,
    },
}

/// Why a LEPP terminal-edge insertion did not commit.
#[derive(Clone, Debug, PartialEq)]
pub enum LeppInsertionError {
    Search(LeppSearchError),
    InvalidGates {
        message: String,
    },
    RequiresClosedMesh {
        open_edges: usize,
    },
    BoundaryTerminal {
        edge: LeppEdgeId,
        face: usize,
    },
    UnprotectedBoundaryTerminal {
        edge: LeppEdgeId,
        face: usize,
    },
    StaleProtectedSegment {
        edge: LeppEdgeId,
    },
    InvalidTerminalEdge {
        edge: LeppEdgeId,
    },
    NearAntipodalTerminalEdge {
        edge: LeppEdgeId,
    },
    DegreeLimit {
        vertex: usize,
        degree: usize,
        maximum: usize,
    },
    ProtectedVertexDegreeWouldChange {
        vertex: usize,
        before: usize,
        after: usize,
    },
    UnmeasurableVertexDegree {
        vertex: usize,
    },
    Transaction(InsertionTransactionError),
}

impl std::fmt::Display for LeppInsertionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Search(error) => write!(formatter, "{error}"),
            Self::InvalidGates { message } => {
                write!(formatter, "invalid LEPP insertion gates: {message}")
            }
            Self::RequiresClosedMesh { open_edges } => write!(
                formatter,
                "LEPP terminal midpoint insertion currently requires a closed mesh; found \
                 {open_edges} open edges"
            ),
            Self::BoundaryTerminal { edge, face } => write!(
                formatter,
                "boundary terminal edge {edge:?} on face {face} requires the later constrained \
                 boundary phase"
            ),
            Self::UnprotectedBoundaryTerminal { edge, face } => write!(
                formatter,
                "boundary terminal edge {edge:?} on face {face} is not in the protected segment \
                 list"
            ),
            Self::StaleProtectedSegment { edge } => {
                write!(
                    formatter,
                    "protected segment {edge:?} is not a current mesh edge"
                )
            }
            Self::InvalidTerminalEdge { edge } => {
                write!(
                    formatter,
                    "terminal edge {edge:?} has invalid endpoint geometry"
                )
            }
            Self::NearAntipodalTerminalEdge { edge } => write!(
                formatter,
                "terminal edge {edge:?} is near-antipodal and has no stable unique midpoint"
            ),
            Self::DegreeLimit {
                vertex,
                degree,
                maximum,
            } => write!(
                formatter,
                "terminal midpoint would leave vertex {vertex} at degree {degree}; maximum is \
                 {maximum}"
            ),
            Self::ProtectedVertexDegreeWouldChange {
                vertex,
                before,
                after,
            } => write!(
                formatter,
                "terminal midpoint would change protected vertex {vertex}'s degree from \
                 {before} to {after}"
            ),
            Self::UnmeasurableVertexDegree { vertex } => {
                write!(
                    formatter,
                    "could not measure vertex {vertex}'s degree after insertion"
                )
            }
            Self::Transaction(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for LeppInsertionError {}

impl From<LeppSearchError> for LeppInsertionError {
    fn from(error: LeppSearchError) -> Self {
        Self::Search(error)
    }
}

impl From<InsertionTransactionError> for LeppInsertionError {
    fn from(error: InsertionTransactionError) -> Self {
        Self::Transaction(error)
    }
}

/// The minor-arc midpoint of a terminal edge, projected to the endpoints'
/// average radius.
pub fn terminal_edge_midpoint(
    mesh: &MeshState,
    edge: LeppEdgeId,
) -> Result<CartesianPoint, LeppInsertionError> {
    let [tail, head] = edge.vertices;
    let a = mesh
        .vertices()
        .get(tail)
        .copied()
        .ok_or(LeppInsertionError::InvalidTerminalEdge { edge })?;
    let b = mesh
        .vertices()
        .get(head)
        .copied()
        .ok_or(LeppInsertionError::InvalidTerminalEdge { edge })?;
    let a_radius = magnitude(a);
    let b_radius = magnitude(b);
    if !a_radius.is_finite() || !b_radius.is_finite() || a_radius <= 0.0 || b_radius <= 0.0 {
        return Err(LeppInsertionError::InvalidTerminalEdge { edge });
    }
    let cosine = (a.x * b.x + a.y * b.y + a.z * b.z) / (a_radius * b_radius);
    if !cosine.is_finite() {
        return Err(LeppInsertionError::InvalidTerminalEdge { edge });
    }
    if cosine <= -1.0 + 64.0 * f64::EPSILON {
        return Err(LeppInsertionError::NearAntipodalTerminalEdge { edge });
    }

    let sum = CartesianPoint::new(
        a.x / a_radius + b.x / b_radius,
        a.y / a_radius + b.y / b_radius,
        a.z / a_radius + b.z / b_radius,
    );
    let norm = magnitude(sum);
    let radius = (a_radius + b_radius) / 2.0;
    if !norm.is_finite() || norm <= 0.0 || !radius.is_finite() || radius <= 0.0 {
        return Err(LeppInsertionError::InvalidTerminalEdge { edge });
    }
    Ok(CartesianPoint::new(
        sum.x / norm * radius,
        sum.y / norm * radius,
        sum.z / norm * radius,
    ))
}

/// Follow LEPP from `start`, split its interior terminal edge at the spherical
/// midpoint, and commit only a closed, valid Delaunay insertion.
pub fn insert_lepp_terminal_midpoint(
    mesh: &mut MeshState,
    start: usize,
    config: &LeppSearchConfig,
    gates: &LeppInsertionGates,
) -> Result<LeppInsertionReport, LeppInsertionError> {
    insert_lepp_terminal_midpoint_with_postcondition(mesh, start, config, gates, |_, _| true)
}

/// Follow LEPP and split a protected open-boundary terminal edge if that is
/// where the path ends. Interior terminals still use the ordinary insertion.
pub fn insert_lepp_terminal_midpoint_constrained(
    mesh: &mut MeshState,
    segments: &mut SegmentList,
    start: usize,
    config: &LeppSearchConfig,
    gates: &LeppInsertionGates,
) -> Result<LeppInsertionReport, LeppInsertionError> {
    insert_lepp_terminal_midpoint_constrained_with_postcondition(
        mesh,
        segments,
        start,
        config,
        gates,
        |_, _| true,
    )
}

pub(crate) fn insert_lepp_terminal_midpoint_with_postcondition(
    mesh: &mut MeshState,
    start: usize,
    config: &LeppSearchConfig,
    gates: &LeppInsertionGates,
    postcondition: impl FnOnce(&MeshState, &InsertionReport) -> bool,
) -> Result<LeppInsertionReport, LeppInsertionError> {
    if gates.maximum_vertex_degree < 3 {
        return Err(LeppInsertionError::InvalidGates {
            message: "maximum_vertex_degree must be at least three".to_string(),
        });
    }
    if let Some(&vertex) = gates
        .protected_vertices
        .iter()
        .find(|&&vertex| vertex < MESH_STATE_FIRST_ID || vertex >= mesh.vertices().len())
    {
        return Err(LeppInsertionError::InvalidGates {
            message: format!("protected vertex {vertex} is not in the mesh"),
        });
    }
    let open_edges = mesh.open_edge_count();
    if open_edges != 0 {
        return Err(LeppInsertionError::RequiresClosedMesh { open_edges });
    }
    let path = find_lepp(mesh, start, config)?;
    let edge = match path.terminal {
        LeppTerminal::InteriorPair { edge, .. } => edge,
        LeppTerminal::Boundary { edge, face } => {
            return Err(LeppInsertionError::BoundaryTerminal { edge, face });
        }
    };
    let point = terminal_edge_midpoint(mesh, edge)?;
    let protected_degrees = gates
        .protected_vertices
        .iter()
        .map(|&vertex| {
            mesh.vertex_degree(vertex)
                .map(|degree| (vertex, degree))
                .map_err(|error| LeppInsertionError::InvalidGates {
                    message: error.to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let failure = Cell::new(None);
    let transaction = mesh.insert_site_transactionally(point, |state, report| {
        let open_edges = state.open_edge_count();
        if open_edges != 0 {
            failure.set(Some(GateFailure::OpenEdges(open_edges)));
            return false;
        }

        let changed: BTreeSet<_> = report.created.iter().copied().collect();
        for (vertex, seed) in state.sites_touching(&changed) {
            let Ok(degree) = state.vertex_degree_from(vertex, seed) else {
                failure.set(Some(GateFailure::UnmeasurableDegree { vertex }));
                return false;
            };
            if degree > gates.maximum_vertex_degree {
                failure.set(Some(GateFailure::Degree { vertex, degree }));
                return false;
            }
        }
        for &(vertex, before) in &protected_degrees {
            let Ok(after) = state.vertex_degree(vertex) else {
                failure.set(Some(GateFailure::UnmeasurableDegree { vertex }));
                return false;
            };
            if after != before {
                failure.set(Some(GateFailure::ProtectedDegree {
                    vertex,
                    before,
                    after,
                }));
                return false;
            }
        }
        postcondition(state, report)
    });
    let insertion = match transaction {
        Ok(report) => report,
        Err(InsertionTransactionError::Rejected) => match failure.get() {
            Some(GateFailure::OpenEdges(open_edges)) => {
                return Err(LeppInsertionError::RequiresClosedMesh { open_edges });
            }
            Some(GateFailure::MissingSegment(edge)) => {
                return Err(LeppInsertionError::StaleProtectedSegment { edge });
            }
            Some(GateFailure::Degree { vertex, degree }) => {
                return Err(LeppInsertionError::DegreeLimit {
                    vertex,
                    degree,
                    maximum: gates.maximum_vertex_degree,
                });
            }
            Some(GateFailure::ProtectedDegree {
                vertex,
                before,
                after,
            }) => {
                return Err(LeppInsertionError::ProtectedVertexDegreeWouldChange {
                    vertex,
                    before,
                    after,
                });
            }
            Some(GateFailure::UnmeasurableDegree { vertex }) => {
                return Err(LeppInsertionError::UnmeasurableVertexDegree { vertex });
            }
            None => {
                return Err(LeppInsertionError::Transaction(
                    InsertionTransactionError::Rejected,
                ))
            }
        },
        Err(error) => return Err(LeppInsertionError::Transaction(error)),
    };

    let changed: BTreeSet<_> = insertion.created.iter().copied().collect();
    let affected_sites = mesh
        .sites_touching(&changed)
        .keys()
        .filter_map(|&vertex| mesh.vertex_id(vertex))
        .collect();
    let created_faces = insertion.created_ids.clone();
    Ok(LeppInsertionReport {
        path,
        requested_edge: edge,
        split_edge: edge,
        split_reason: LeppInsertionSplitReason::TerminalEdge,
        point,
        insertion,
        affected_sites,
        created_faces,
    })
}

pub(crate) fn insert_lepp_terminal_midpoint_constrained_with_postcondition(
    mesh: &mut MeshState,
    segments: &mut SegmentList,
    start: usize,
    config: &LeppSearchConfig,
    gates: &LeppInsertionGates,
    postcondition: impl FnOnce(&MeshState, &InsertionReport) -> bool,
) -> Result<LeppInsertionReport, LeppInsertionError> {
    if gates.maximum_vertex_degree < 3 {
        return Err(LeppInsertionError::InvalidGates {
            message: "maximum_vertex_degree must be at least three".to_string(),
        });
    }
    if let Some(&vertex) = gates
        .protected_vertices
        .iter()
        .find(|&&vertex| vertex < MESH_STATE_FIRST_ID || vertex >= mesh.vertices().len())
    {
        return Err(LeppInsertionError::InvalidGates {
            message: format!("protected vertex {vertex} is not in the mesh"),
        });
    }

    validate_segments_are_mesh_edges(mesh, segments)?;

    let path = find_lepp(mesh, start, config)?;
    let (terminal_edge, boundary_face) = match path.terminal {
        LeppTerminal::InteriorPair { edge, .. } => (edge, None),
        LeppTerminal::Boundary { edge, face } => (edge, Some(face)),
    };
    let terminal_point = terminal_edge_midpoint(mesh, terminal_edge)?;
    let encroached = mesh.encroached_segment_edges(terminal_point, segments.iter());
    let (edge, point, split_segment, split_reason) = if let Some(encroachment) = encroached {
        let edge = LeppEdgeId::new(encroachment.tail, encroachment.head);
        if edge == terminal_edge {
            (
                terminal_edge,
                terminal_point,
                boundary_face.is_some(),
                LeppInsertionSplitReason::TerminalEdge,
            )
        } else {
            (
                edge,
                encroachment.split_at,
                true,
                LeppInsertionSplitReason::EncroachedSegment,
            )
        }
    } else {
        if let Some(face) = boundary_face {
            if !segments.contains(terminal_edge.vertices[0], terminal_edge.vertices[1]) {
                return Err(LeppInsertionError::UnprotectedBoundaryTerminal {
                    edge: terminal_edge,
                    face,
                });
            }
        }
        (
            terminal_edge,
            terminal_point,
            boundary_face.is_some(),
            LeppInsertionSplitReason::TerminalEdge,
        )
    };
    let before_open_edges = mesh.open_edge_count();
    let [tail, head] = edge.vertices;
    let open_edge_face = split_segment
        .then(|| find_open_edge_face(mesh, tail, head))
        .flatten();
    let splits_open_edge = open_edge_face.is_some();
    let cavity = insertion_cavity(mesh, point, open_edge_face)?;
    let cavity_sites = mesh.sites_touching(&cavity);
    let protected_degrees = gates
        .protected_vertices
        .iter()
        .filter_map(|&vertex| cavity_sites.get(&vertex).map(|&seed| (vertex, seed)))
        .map(|(vertex, seed)| {
            measured_degree(mesh, vertex, seed, before_open_edges != 0)
                .map(|degree| (vertex, degree))
                .ok_or(LeppInsertionError::UnmeasurableVertexDegree { vertex })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut segments_at_risk = protected_segments_in_cavity(mesh, segments, &cavity);
    let before_segments = segments.clone();
    let failure = Cell::new(None);
    if split_segment {
        // `MeshState` appends vertices, so this is the midpoint slot the
        // transaction will allocate. Update the external boundary state first
        // and restore it on every transaction failure; that leaves no
        // post-commit branch where the mesh changed but the marker did not.
        let midpoint = mesh.vertices().len();
        if !segments.split(tail, head, midpoint) {
            return Err(LeppInsertionError::UnprotectedBoundaryTerminal {
                edge,
                face: boundary_face.unwrap_or(start),
            });
        }
        segments_at_risk.remove(&(tail.min(head), tail.max(head)));
        segments_at_risk.insert((tail.min(midpoint), tail.max(midpoint)));
        segments_at_risk.insert((head.min(midpoint), head.max(midpoint)));
    }
    let transaction = if splits_open_edge {
        mesh.insert_site_on_boundary_edge_transactionally(point, tail, head, |state, report| {
            if state.open_edge_count() != before_open_edges + 1 {
                failure.set(Some(GateFailure::OpenEdges(state.open_edge_count())));
                return false;
            }
            constrained_gates_pass(
                state,
                report,
                gates,
                &protected_degrees,
                &segments_at_risk,
                before_open_edges != 0,
                &failure,
            ) && postcondition(state, report)
        })
    } else {
        mesh.insert_site_transactionally(point, |state, report| {
            if state.open_edge_count() != before_open_edges {
                failure.set(Some(GateFailure::OpenEdges(state.open_edge_count())));
                return false;
            }
            constrained_gates_pass(
                state,
                report,
                gates,
                &protected_degrees,
                &segments_at_risk,
                before_open_edges != 0,
                &failure,
            ) && postcondition(state, report)
        })
    };
    let insertion = match transaction {
        Ok(report) => report,
        Err(InsertionTransactionError::Rejected) => {
            *segments = before_segments;
            match failure.get() {
                Some(GateFailure::OpenEdges(open_edges)) => {
                    return Err(LeppInsertionError::RequiresClosedMesh { open_edges });
                }
                Some(GateFailure::MissingSegment(edge)) => {
                    return Err(LeppInsertionError::StaleProtectedSegment { edge });
                }
                Some(GateFailure::Degree { vertex, degree }) => {
                    return Err(LeppInsertionError::DegreeLimit {
                        vertex,
                        degree,
                        maximum: gates.maximum_vertex_degree,
                    });
                }
                Some(GateFailure::ProtectedDegree {
                    vertex,
                    before,
                    after,
                }) => {
                    return Err(LeppInsertionError::ProtectedVertexDegreeWouldChange {
                        vertex,
                        before,
                        after,
                    });
                }
                Some(GateFailure::UnmeasurableDegree { vertex }) => {
                    return Err(LeppInsertionError::UnmeasurableVertexDegree { vertex });
                }
                None => {
                    return Err(LeppInsertionError::Transaction(
                        InsertionTransactionError::Rejected,
                    ))
                }
            }
        }
        Err(error) => {
            *segments = before_segments;
            return Err(LeppInsertionError::Transaction(error));
        }
    };
    let changed: BTreeSet<_> = insertion.created.iter().copied().collect();
    let affected_sites = mesh
        .sites_touching(&changed)
        .keys()
        .filter_map(|&vertex| mesh.vertex_id(vertex))
        .collect();
    let created_faces = insertion.created_ids.clone();
    Ok(LeppInsertionReport {
        path,
        requested_edge: terminal_edge,
        split_edge: edge,
        split_reason,
        point,
        insertion,
        affected_sites,
        created_faces,
    })
}

fn validate_segments_are_mesh_edges(
    state: &MeshState,
    segments: &SegmentList,
) -> Result<(), LeppInsertionError> {
    let mesh_edges = state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .flat_map(|corners| {
            [
                (corners[0].min(corners[1]), corners[0].max(corners[1])),
                (corners[1].min(corners[2]), corners[1].max(corners[2])),
                (corners[2].min(corners[0]), corners[2].max(corners[0])),
            ]
        })
        .collect::<BTreeSet<_>>();
    if let Some((tail, head)) = segments.iter().find(|edge| !mesh_edges.contains(edge)) {
        Err(LeppInsertionError::StaleProtectedSegment {
            edge: LeppEdgeId::new(tail, head),
        })
    } else {
        Ok(())
    }
}

fn insertion_cavity(
    state: &MeshState,
    point: CartesianPoint,
    containing: Option<usize>,
) -> Result<BTreeSet<usize>, LeppInsertionError> {
    let containing = containing
        .map(Ok)
        .unwrap_or_else(|| state.locate_triangle(point, None))
        .map_err(|error| {
            LeppInsertionError::Transaction(InsertionTransactionError::Insert(error))
        })?;
    state
        .delaunay_cavity(point, containing)
        .map_err(|error| LeppInsertionError::Transaction(InsertionTransactionError::Insert(error)))
}

fn protected_segments_in_cavity(
    state: &MeshState,
    segments: &SegmentList,
    cavity: &BTreeSet<usize>,
) -> BTreeSet<(usize, usize)> {
    cavity
        .iter()
        .flat_map(|&triangle| {
            let corners = state.triangles()[triangle];
            [
                (corners[0].min(corners[1]), corners[0].max(corners[1])),
                (corners[1].min(corners[2]), corners[1].max(corners[2])),
                (corners[2].min(corners[0]), corners[2].max(corners[0])),
            ]
        })
        .filter(|&(tail, head)| segments.contains(tail, head))
        .collect()
}

fn constrained_gates_pass(
    state: &MeshState,
    report: &InsertionReport,
    gates: &LeppInsertionGates,
    protected_degrees: &[(usize, usize)],
    segments_at_risk: &BTreeSet<(usize, usize)>,
    mesh_has_open_edges: bool,
    failure: &Cell<Option<GateFailure>>,
) -> bool {
    let changed: BTreeSet<_> = report.created.iter().copied().collect();
    if let Some(&(tail, head)) = segments_at_risk.iter().find(|&&(tail, head)| {
        !report.created.iter().any(|&face| {
            let corners = state.triangles()[face];
            corners.contains(&tail) && corners.contains(&head)
        })
    }) {
        failure.set(Some(GateFailure::MissingSegment(LeppEdgeId::new(
            tail, head,
        ))));
        return false;
    }
    let touched = state.sites_touching(&changed);
    for (&vertex, &seed) in &touched {
        let Some(degree) = measured_degree(state, vertex, seed, mesh_has_open_edges) else {
            failure.set(Some(GateFailure::UnmeasurableDegree { vertex }));
            return false;
        };
        if degree > gates.maximum_vertex_degree {
            failure.set(Some(GateFailure::Degree { vertex, degree }));
            return false;
        }
    }
    for &(vertex, before) in protected_degrees {
        let Some(&seed) = touched.get(&vertex) else {
            continue;
        };
        let Some(after) = measured_degree(state, vertex, seed, mesh_has_open_edges) else {
            failure.set(Some(GateFailure::UnmeasurableDegree { vertex }));
            return false;
        };
        if after != before {
            failure.set(Some(GateFailure::ProtectedDegree {
                vertex,
                before,
                after,
            }));
            return false;
        }
    }
    true
}

fn measured_degree(
    state: &MeshState,
    vertex: usize,
    seed: usize,
    mesh_has_open_edges: bool,
) -> Option<usize> {
    if mesh_has_open_edges {
        Some(incident_triangle_count(state, vertex))
    } else {
        state.vertex_degree_from(vertex, seed).ok()
    }
}

fn incident_triangle_count(state: &MeshState, vertex: usize) -> usize {
    state.triangles()[MESH_STATE_FIRST_ID..]
        .iter()
        .filter(|corners| corners.contains(&vertex))
        .count()
}

fn find_open_edge_face(state: &MeshState, tail: usize, head: usize) -> Option<usize> {
    let key = (tail.min(head), tail.max(head));
    for triangle in MESH_STATE_FIRST_ID..state.triangles().len() {
        let corners = state.triangles()[triangle];
        for corner in 0..3 {
            let a = corners[(corner + 1) % 3];
            let b = corners[(corner + 2) % 3];
            if (a.min(b), a.max(b)) == key && state.neighbours()[triangle][corner] == 0 {
                return Some(triangle);
            }
        }
    }
    None
}

#[cfg(test)]
mod preservation_tests {
    use super::*;

    #[test]
    fn constrained_gate_rejects_a_protected_segment_missing_after_the_cavity() {
        let state = MeshState::from_parts(
            vec![
                CartesianPoint::new(0.0, 0.0, 0.0),
                CartesianPoint::new(0.0, 0.0, 0.0),
                CartesianPoint::new(1.0, 0.0, 0.0),
                CartesianPoint::new(0.0, 1.0, 0.0),
                CartesianPoint::new(0.0, 0.0, 1.0),
                CartesianPoint::new(-1.0, 0.0, 0.0),
            ],
            vec![[1, 1, 1], [1, 1, 1], [2, 3, 4]],
        )
        .expect("test mesh");
        let report = InsertionReport {
            site: 4,
            site_id: state.vertex_id(4).expect("site id"),
            removed: Vec::new(),
            removed_ids: Vec::new(),
            created: vec![2],
            created_ids: vec![state.face_id(2).expect("face id")],
        };
        let failure = Cell::new(None);

        assert!(!constrained_gates_pass(
            &state,
            &report,
            &LeppInsertionGates::default(),
            &[],
            &BTreeSet::from([(2, 5)]),
            true,
            &failure,
        ));
        assert_eq!(
            failure.get(),
            Some(GateFailure::MissingSegment(LeppEdgeId::new(2, 5)))
        );
    }
}
