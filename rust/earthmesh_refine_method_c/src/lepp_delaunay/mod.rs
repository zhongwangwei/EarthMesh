//! Explicit, non-default LEPP-Delaunay primitives for Method-C.
//!
//! The module provides a read-only LEPP walk plus transaction-gated midpoint
//! insertion by reusing the generic `earthmesh_mesh` cavity implementation.
//! Canonical Method-C remains the default; the CLI selects this path only with
//! `&method_c algorithm='lepp_delaunay'`.

use std::collections::{BTreeMap, BTreeSet};

use earthmesh_mesh::{cross, dot, magnitude, CartesianPoint, MeshState};

const LEPP_REPORT_DETAIL_LIMIT: usize = 1024;

fn push_report_detail<T>(items: &mut Vec<T>, item: T) {
    if items.len() < LEPP_REPORT_DETAIL_LIMIT {
        items.push(item);
    }
}

mod adaptive;
mod insertion;
mod post_quality;
pub use adaptive::{
    adaptive_hybrid_target_edge_from_level, refine_adaptive_hybrid,
    refine_adaptive_hybrid_constrained, refine_adaptive_hybrid_regions, AdaptiveHybridConfig,
    AdaptiveHybridDemand, AdaptiveHybridError, AdaptiveHybridInsertionCounts,
    AdaptiveHybridPathStats, AdaptiveHybridRejection, AdaptiveHybridReport,
    AdaptiveHybridStopReason, AdaptiveHybridTargetSatisfaction, AdaptiveHybridUnresolvedDemand,
    AdaptiveHybridUnresolvedReason,
};
pub use insertion::{
    insert_lepp_terminal_midpoint, insert_lepp_terminal_midpoint_constrained,
    terminal_edge_midpoint, LeppInsertionError, LeppInsertionGates, LeppInsertionReport,
    LeppInsertionSplitReason,
};
pub use post_quality::{
    improve_lepp_post_quality, LeppPostQualityConfig, LeppPostQualityError,
    LeppPostQualityRejection, LeppPostQualityReport, LeppPostQualityStopReason,
    LeppQualitySnapshot,
};

/// Triangle id in the one-based [`MeshState`] tables.
pub type FaceId = usize;

/// Vertex-pair edge id, sorted so either incident face names the same edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LeppEdgeId {
    pub vertices: [usize; 2],
}

impl LeppEdgeId {
    pub const fn new(a: usize, b: usize) -> Self {
        if a <= b {
            Self { vertices: [a, b] }
        } else {
            Self { vertices: [b, a] }
        }
    }
}

/// Why a LEPP walk stopped successfully.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeppTerminal {
    /// The terminal edge is the longest edge of both incident faces.
    InteriorPair {
        edge: LeppEdgeId,
        faces: [FaceId; 2],
    },
    /// The terminal edge has no neighbour across it.
    Boundary { edge: LeppEdgeId, face: FaceId },
}

/// Deterministic, complete read-only LEPP path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeppPath {
    pub faces: Vec<FaceId>,
    pub edges: Vec<LeppEdgeId>,
    pub terminal: LeppTerminal,
}

/// Limits and numeric tie policy for LEPP search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeppSearchConfig {
    /// Maximum number of faces allowed in one path.
    pub maximum_path_length: usize,
    /// Relative tolerance under which edge lengths use [`LeppEdgeId`] ordering.
    pub length_tie_relative_epsilon: f64,
}

impl Default for LeppSearchConfig {
    fn default() -> Self {
        Self {
            maximum_path_length: earthmesh_core::DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH,
            length_tie_relative_epsilon: 1.0e-12,
        }
    }
}

/// Partial deterministic report attached to search errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeppSearchReport {
    pub start: FaceId,
    pub faces: Vec<FaceId>,
    pub edges: Vec<LeppEdgeId>,
}

/// Search failures. Public paths return errors rather than panicking.
#[derive(Clone, Debug, PartialEq)]
pub enum LeppSearchError {
    InvalidConfig {
        message: String,
    },
    EmptyMesh {
        start: FaceId,
    },
    InvalidStartFace {
        start: FaceId,
    },
    InvalidFace {
        face: FaceId,
        report: LeppSearchReport,
    },
    InvalidVertex {
        face: FaceId,
        vertex: usize,
        report: LeppSearchReport,
    },
    InvalidCoordinate {
        vertex: usize,
        report: LeppSearchReport,
    },
    InvalidRadius {
        radius: f64,
        report: LeppSearchReport,
    },
    InvalidEdgeLength {
        face: FaceId,
        edge: LeppEdgeId,
        length: f64,
        report: LeppSearchReport,
    },
    AsymmetricNeighbour {
        face: FaceId,
        edge: LeppEdgeId,
        neighbour: FaceId,
        report: LeppSearchReport,
    },
    NonManifoldEdge {
        edge: LeppEdgeId,
        faces: Vec<FaceId>,
        report: LeppSearchReport,
    },
    Cycle {
        face: FaceId,
        report: LeppSearchReport,
    },
    MaximumPathLength {
        maximum_path_length: usize,
        report: LeppSearchReport,
    },
}

impl std::fmt::Display for LeppSearchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig { message } => write!(formatter, "invalid LEPP search config: {message}"),
            Self::EmptyMesh { start } => write!(formatter, "cannot search LEPP from {start}: mesh has no faces"),
            Self::InvalidStartFace { start } => write!(formatter, "cannot search LEPP from invalid start face {start}"),
            Self::InvalidFace { face, .. } => write!(formatter, "LEPP reached invalid face {face}"),
            Self::InvalidVertex { face, vertex, .. } => write!(formatter, "face {face} names invalid vertex {vertex}"),
            Self::InvalidCoordinate { vertex, .. } => write!(formatter, "vertex {vertex} has non-finite coordinates or zero radius"),
            Self::InvalidRadius { radius, .. } => write!(formatter, "mesh radius {radius} is not finite and positive"),
            Self::InvalidEdgeLength { face, edge, length, .. } => write!(formatter, "face {face} edge {edge:?} has invalid length {length}"),
            Self::AsymmetricNeighbour { face, edge, neighbour, .. } => write!(formatter, "face {face} crosses edge {edge:?} to {neighbour}, but the neighbour does not point back across the same edge"),
            Self::NonManifoldEdge { edge, faces, .. } => write!(formatter, "edge {edge:?} is claimed by {} faces", faces.len()),
            Self::Cycle { face, .. } => write!(formatter, "LEPP cycle detected at face {face}"),
            Self::MaximumPathLength { maximum_path_length, .. } => write!(formatter, "LEPP exceeded maximum path length {maximum_path_length}"),
        }
    }
}

impl std::error::Error for LeppSearchError {}

trait ReadOnlyTriangulation {
    fn vertices(&self) -> &[CartesianPoint];
    fn triangles(&self) -> &[[usize; 3]];
    fn neighbours(&self) -> &[[usize; 3]];

    fn edge_claimants(
        &self,
        _face: FaceId,
        _corner: usize,
        _edge: LeppEdgeId,
    ) -> Option<Vec<FaceId>> {
        None
    }
}

impl ReadOnlyTriangulation for MeshState {
    fn vertices(&self) -> &[CartesianPoint] {
        self.vertices()
    }

    fn triangles(&self) -> &[[usize; 3]] {
        self.triangles()
    }

    fn neighbours(&self) -> &[[usize; 3]] {
        self.neighbours()
    }

    fn edge_claimants(
        &self,
        face: FaceId,
        corner: usize,
        _edge: LeppEdgeId,
    ) -> Option<Vec<FaceId>> {
        let neighbour = *self.neighbours().get(face)?.get(corner)?;
        if neighbour == 0 {
            Some(vec![face])
        } else {
            Some(vec![face, neighbour])
        }
    }
}

#[derive(Clone, Copy)]
struct FaceEdge {
    corner: usize,
    id: LeppEdgeId,
    length: f64,
}

struct SearchContext {
    radius: f64,
    incidence: Option<BTreeMap<LeppEdgeId, Vec<FaceId>>>,
}

/// Spherical great-circle length `R * atan2(norm(cross(a,b)), dot(a,b))`.
pub fn spherical_edge_length(radius: f64, a: CartesianPoint, b: CartesianPoint) -> f64 {
    radius * magnitude(cross(a, b)).atan2(dot(a, b))
}

/// Read-only LEPP search over [`MeshState`].
pub fn find_lepp(
    mesh: &MeshState,
    start: FaceId,
    config: &LeppSearchConfig,
) -> Result<LeppPath, LeppSearchError> {
    find_lepp_in(mesh, start, config)
}

fn find_lepp_in<M: ReadOnlyTriangulation>(
    mesh: &M,
    start: FaceId,
    config: &LeppSearchConfig,
) -> Result<LeppPath, LeppSearchError> {
    validate_config(config)?;
    if mesh.triangles().len() <= earthmesh_mesh::MESH_STATE_FIRST_ID {
        return Err(LeppSearchError::EmptyMesh { start });
    }
    if start < earthmesh_mesh::MESH_STATE_FIRST_ID || start >= mesh.triangles().len() {
        return Err(LeppSearchError::InvalidStartFace { start });
    }

    let mut faces = Vec::new();
    let mut edges = Vec::new();
    let context = build_context(mesh, start, &faces, &edges)?;
    let mut visited = BTreeSet::new();
    let mut current = start;

    loop {
        if faces.len() >= config.maximum_path_length {
            return Err(LeppSearchError::MaximumPathLength {
                maximum_path_length: config.maximum_path_length,
                report: make_report(start, &faces, &edges),
            });
        }
        if !visited.insert(current) {
            return Err(LeppSearchError::Cycle {
                face: current,
                report: make_report(start, &faces, &edges),
            });
        }
        faces.push(current);

        let edge = longest_edge(mesh, current, config, &context, start, &faces, &edges)?;
        edges.push(edge.id);

        let claimants = edge_claimants(mesh, &context, current, edge.corner, edge.id);
        if claimants.len() > 2 {
            return Err(LeppSearchError::NonManifoldEdge {
                edge: edge.id,
                faces: claimants,
                report: make_report(start, &faces, &edges),
            });
        }

        let neighbour = *mesh
            .neighbours()
            .get(current)
            .and_then(|row| row.get(edge.corner))
            .ok_or_else(|| LeppSearchError::InvalidFace {
                face: current,
                report: make_report(start, &faces, &edges),
            })?;
        if neighbour == 0 {
            return Ok(LeppPath {
                faces,
                edges,
                terminal: LeppTerminal::Boundary {
                    edge: edge.id,
                    face: current,
                },
            });
        }
        if neighbour == current {
            return Err(LeppSearchError::Cycle {
                face: current,
                report: make_report(start, &faces, &edges),
            });
        }
        require_reverse_neighbour(mesh, current, edge.id, neighbour, start, &faces, &edges)?;

        let neighbour_edge =
            longest_edge(mesh, neighbour, config, &context, start, &faces, &edges)?;
        if neighbour_edge.id == edge.id {
            let mut pair = [current, neighbour];
            pair.sort();
            if !faces.contains(&neighbour) {
                if faces.len() >= config.maximum_path_length {
                    return Err(LeppSearchError::MaximumPathLength {
                        maximum_path_length: config.maximum_path_length,
                        report: make_report(start, &faces, &edges),
                    });
                }
                faces.push(neighbour);
            }
            return Ok(LeppPath {
                faces,
                edges,
                terminal: LeppTerminal::InteriorPair {
                    edge: edge.id,
                    faces: pair,
                },
            });
        }

        current = neighbour;
    }
}

fn longest_edge<M: ReadOnlyTriangulation>(
    mesh: &M,
    face: FaceId,
    config: &LeppSearchConfig,
    context: &SearchContext,
    start: FaceId,
    faces: &[FaceId],
    edges: &[LeppEdgeId],
) -> Result<FaceEdge, LeppSearchError> {
    let corners = *mesh
        .triangles()
        .get(face)
        .ok_or_else(|| LeppSearchError::InvalidFace {
            face,
            report: make_report(start, faces, edges),
        })?;
    let mut best = None;
    for corner in 0..3 {
        let left = corners[(corner + 1) % 3];
        let right = corners[(corner + 2) % 3];
        let a = *mesh
            .vertices()
            .get(left)
            .ok_or_else(|| LeppSearchError::InvalidVertex {
                face,
                vertex: left,
                report: make_report(start, faces, edges),
            })?;
        let b = *mesh
            .vertices()
            .get(right)
            .ok_or_else(|| LeppSearchError::InvalidVertex {
                face,
                vertex: right,
                report: make_report(start, faces, edges),
            })?;
        let candidate = FaceEdge {
            corner,
            id: LeppEdgeId::new(left, right),
            length: spherical_edge_length(context.radius, a, b),
        };
        if !candidate.length.is_finite() || candidate.length <= 0.0 {
            return Err(LeppSearchError::InvalidEdgeLength {
                face,
                edge: candidate.id,
                length: candidate.length,
                report: make_report(start, faces, edges),
            });
        }
        if best.is_none_or(|current| better_edge(candidate, current, config)) {
            best = Some(candidate);
        }
    }
    best.ok_or_else(|| LeppSearchError::InvalidFace {
        face,
        report: make_report(start, faces, edges),
    })
}

fn better_edge(candidate: FaceEdge, current: FaceEdge, config: &LeppSearchConfig) -> bool {
    let scale = candidate.length.abs().max(current.length.abs()).max(1.0);
    if (candidate.length - current.length).abs() <= config.length_tie_relative_epsilon * scale {
        candidate.id < current.id
    } else {
        candidate.length > current.length
    }
}

fn make_report(start: FaceId, faces: &[FaceId], edges: &[LeppEdgeId]) -> LeppSearchReport {
    LeppSearchReport {
        start,
        faces: faces.to_vec(),
        edges: edges.to_vec(),
    }
}

fn validate_config(config: &LeppSearchConfig) -> Result<(), LeppSearchError> {
    if config.maximum_path_length == 0 {
        return Err(LeppSearchError::InvalidConfig {
            message: "maximum_path_length must be greater than zero".to_string(),
        });
    }
    if !config.length_tie_relative_epsilon.is_finite()
        || config.length_tie_relative_epsilon < 0.0
        || config.length_tie_relative_epsilon >= 1.0
    {
        return Err(LeppSearchError::InvalidConfig {
            message: "length_tie_relative_epsilon must be finite and in [0, 1)".to_string(),
        });
    }
    Ok(())
}

fn build_context<M: ReadOnlyTriangulation>(
    mesh: &M,
    start: FaceId,
    faces: &[FaceId],
    edges: &[LeppEdgeId],
) -> Result<SearchContext, LeppSearchError> {
    let radius = mesh_radius(mesh, start, faces, edges)?;
    if mesh
        .edge_claimants(start, 0, LeppEdgeId::new(0, 0))
        .is_some()
    {
        return Ok(SearchContext {
            radius,
            incidence: None,
        });
    }

    let mut incidence: BTreeMap<LeppEdgeId, Vec<FaceId>> = BTreeMap::new();
    for (face, corners) in mesh
        .triangles()
        .iter()
        .enumerate()
        .skip(earthmesh_mesh::MESH_STATE_FIRST_ID)
    {
        for corner in 0..3 {
            let left = corners[(corner + 1) % 3];
            let right = corners[(corner + 2) % 3];
            require_vertex(mesh, face, left, start, faces, edges)?;
            require_vertex(mesh, face, right, start, faces, edges)?;
            incidence
                .entry(LeppEdgeId::new(left, right))
                .or_default()
                .push(face);
        }
    }
    Ok(SearchContext {
        radius,
        incidence: Some(incidence),
    })
}

fn edge_claimants<M: ReadOnlyTriangulation>(
    mesh: &M,
    context: &SearchContext,
    face: FaceId,
    corner: usize,
    edge: LeppEdgeId,
) -> Vec<FaceId> {
    mesh.edge_claimants(face, corner, edge)
        .or_else(|| {
            context
                .incidence
                .as_ref()
                .and_then(|incidence| incidence.get(&edge).cloned())
        })
        .unwrap_or_default()
}

fn require_reverse_neighbour<M: ReadOnlyTriangulation>(
    mesh: &M,
    current: FaceId,
    edge: LeppEdgeId,
    neighbour: FaceId,
    start: FaceId,
    faces: &[FaceId],
    edges: &[LeppEdgeId],
) -> Result<(), LeppSearchError> {
    let Some(neighbour_corners) = mesh.triangles().get(neighbour) else {
        return Err(LeppSearchError::AsymmetricNeighbour {
            face: current,
            edge,
            neighbour,
            report: make_report(start, faces, edges),
        });
    };
    let Some(neighbour_edges) = mesh.neighbours().get(neighbour) else {
        return Err(LeppSearchError::AsymmetricNeighbour {
            face: current,
            edge,
            neighbour,
            report: make_report(start, faces, edges),
        });
    };
    for corner in 0..3 {
        let candidate = LeppEdgeId::new(
            neighbour_corners[(corner + 1) % 3],
            neighbour_corners[(corner + 2) % 3],
        );
        if candidate == edge && neighbour_edges[corner] == current {
            return Ok(());
        }
    }
    Err(LeppSearchError::AsymmetricNeighbour {
        face: current,
        edge,
        neighbour,
        report: make_report(start, faces, edges),
    })
}

fn mesh_radius<M: ReadOnlyTriangulation>(
    mesh: &M,
    start: FaceId,
    faces: &[FaceId],
    edges: &[LeppEdgeId],
) -> Result<f64, LeppSearchError> {
    let mut count = 0usize;
    let mut total = 0.0;
    for (vertex, point) in mesh
        .vertices()
        .iter()
        .enumerate()
        .skip(earthmesh_mesh::MESH_STATE_FIRST_ID)
    {
        let radius = point_radius(*point);
        if !radius.is_finite() || radius <= 0.0 {
            return Err(LeppSearchError::InvalidCoordinate {
                vertex,
                report: make_report(start, faces, edges),
            });
        }
        count += 1;
        total += radius;
    }
    let radius = if count == 0 {
        0.0
    } else {
        total / count as f64
    };
    if !radius.is_finite() || radius <= 0.0 {
        Err(LeppSearchError::InvalidRadius {
            radius,
            report: make_report(start, faces, edges),
        })
    } else {
        Ok(radius)
    }
}

fn require_vertex<M: ReadOnlyTriangulation>(
    mesh: &M,
    face: FaceId,
    vertex: usize,
    start: FaceId,
    faces: &[FaceId],
    edges: &[LeppEdgeId],
) -> Result<CartesianPoint, LeppSearchError> {
    let point = *mesh
        .vertices()
        .get(vertex)
        .ok_or_else(|| LeppSearchError::InvalidVertex {
            face,
            vertex,
            report: make_report(start, faces, edges),
        })?;
    let radius = point_radius(point);
    if !radius.is_finite() || radius <= 0.0 {
        Err(LeppSearchError::InvalidCoordinate {
            vertex,
            report: make_report(start, faces, edges),
        })
    } else {
        Ok(point)
    }
}

fn point_radius(point: CartesianPoint) -> f64 {
    if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
        f64::NAN
    } else {
        magnitude(point)
    }
}

#[cfg(test)]
mod tests;
