use std::cell::RefCell;
use std::collections::BTreeSet;

use rayon::prelude::*;

use earthmesh_mesh::{dot, magnitude, CartesianPoint, FaceId as StableFaceId, MeshState};

use super::insertion::insert_lepp_terminal_midpoint_with_postcondition;
use super::{
    push_report_detail, spherical_edge_length, FaceId, LeppInsertionError, LeppInsertionGates,
    LeppInsertionReport, LeppSearchConfig,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LeppPostQualityConfig {
    pub maximum_edge_length: Option<f64>,
    pub minimum_spherical_triangle_angle_degrees: Option<f64>,
    pub maximum_insertions: usize,
    pub search: LeppSearchConfig,
    pub gates: LeppInsertionGates,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeppQualitySnapshot {
    pub worst_violation: f64,
    pub total_violation: f64,
    pub violating_faces: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeppPostQualityStopReason {
    NoViolations,
    MaximumInsertions,
    NoCommittableInsertion,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LeppPostQualityReport {
    pub before: LeppQualitySnapshot,
    pub after: LeppQualitySnapshot,
    pub attempted: usize,
    pub committed: usize,
    pub rejected: usize,
    pub insertions: Vec<LeppInsertionReport>,
    pub rejections: Vec<LeppPostQualityRejection>,
    pub stop_reason: LeppPostQualityStopReason,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LeppPostQualityRejection {
    pub face: StableFaceId,
    pub error: LeppInsertionError,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LeppPostQualityError {
    InvalidConfig { message: String },
    InvalidMesh { message: String },
}

impl std::fmt::Display for LeppPostQualityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig { message } => {
                write!(formatter, "invalid LEPP post-quality config: {message}")
            }
            Self::InvalidMesh { message } => {
                write!(formatter, "invalid mesh for LEPP post-quality: {message}")
            }
        }
    }
}

impl std::error::Error for LeppPostQualityError {}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Objective {
    worst: f64,
    total: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Candidate {
    face: FaceId,
    violation: f64,
    id: StableFaceId,
}

const IMPROVEMENT_EPSILON: f64 = 1.0e-12;

pub fn improve_lepp_post_quality(
    mesh: &mut MeshState,
    config: &LeppPostQualityConfig,
) -> Result<LeppPostQualityReport, LeppPostQualityError> {
    validate_config(config)?;
    if let Err(errors) = mesh.validate() {
        return Err(LeppPostQualityError::InvalidMesh {
            message: format!("topology validation failed: {errors:?}"),
        });
    }
    validate_mesh_for_insertion(mesh, config)?;
    let before = quality_snapshot(mesh, config)?;
    let mut after = before;
    let mut attempted = 0usize;
    let mut committed = 0usize;
    let mut rejected = 0usize;
    let mut insertions = Vec::new();
    let mut rejections = Vec::new();
    let mut skipped = BTreeSet::new();
    let stop_reason;

    loop {
        if after.violating_faces == 0 {
            stop_reason = LeppPostQualityStopReason::NoViolations;
            break;
        }
        if committed >= config.maximum_insertions {
            stop_reason = LeppPostQualityStopReason::MaximumInsertions;
            break;
        }
        let Some(candidate) = worst_candidate(mesh, config, &skipped)? else {
            stop_reason = LeppPostQualityStopReason::NoCommittableInsertion;
            break;
        };
        attempted += 1;
        let baseline = objective(mesh, config)?;
        let objective_error = RefCell::new(None);
        let insertion = insert_lepp_terminal_midpoint_with_postcondition(
            mesh,
            candidate.face,
            &config.search,
            &config.gates,
            |state, _| match objective(state, config) {
                Ok(candidate) => strictly_improves(candidate, baseline),
                Err(error) => {
                    objective_error.replace(Some(error));
                    false
                }
            },
        );
        if let Some(error) = objective_error.into_inner() {
            return Err(error);
        }
        match insertion {
            Ok(report) => {
                committed += 1;
                push_report_detail(&mut insertions, report);
                skipped.clear();
                after = quality_snapshot(mesh, config)?;
            }
            Err(error) => {
                rejected += 1;
                skipped.insert(candidate.id);
                push_report_detail(
                    &mut rejections,
                    LeppPostQualityRejection {
                        face: candidate.id,
                        error,
                    },
                );
            }
        }
    }

    Ok(LeppPostQualityReport {
        before,
        after,
        attempted,
        committed,
        rejected,
        insertions,
        rejections,
        stop_reason,
    })
}

fn validate_mesh_for_insertion(
    mesh: &MeshState,
    config: &LeppPostQualityConfig,
) -> Result<(), LeppPostQualityError> {
    let open_edges = mesh.open_edge_count();
    if open_edges != 0 {
        return Err(LeppPostQualityError::InvalidMesh {
            message: format!(
                "LEPP post-quality requires a closed mesh; found {open_edges} open edges"
            ),
        });
    }
    if let Some(&vertex) = config.gates.protected_vertices.iter().find(|&&vertex| {
        vertex < earthmesh_mesh::MESH_STATE_FIRST_ID || vertex >= mesh.vertices().len()
    }) {
        return Err(LeppPostQualityError::InvalidConfig {
            message: format!("protected vertex {vertex} is not in the mesh"),
        });
    }
    Ok(())
}

fn validate_config(config: &LeppPostQualityConfig) -> Result<(), LeppPostQualityError> {
    if config.maximum_edge_length.is_none()
        && config.minimum_spherical_triangle_angle_degrees.is_none()
    {
        return Err(LeppPostQualityError::InvalidConfig {
            message: "at least one quality trigger is required".to_string(),
        });
    }
    if let Some(length) = config.maximum_edge_length {
        if !length.is_finite() || length <= 0.0 {
            return Err(LeppPostQualityError::InvalidConfig {
                message: "maximum_edge_length must be finite and positive".to_string(),
            });
        }
    }
    if let Some(angle) = config.minimum_spherical_triangle_angle_degrees {
        if !angle.is_finite() || angle <= 0.0 || angle >= 60.0 {
            return Err(LeppPostQualityError::InvalidConfig {
                message: "minimum_spherical_triangle_angle_degrees must be finite and in (0, 60)"
                    .to_string(),
            });
        }
    }
    if config.maximum_insertions == 0 {
        return Err(LeppPostQualityError::InvalidConfig {
            message: "maximum_insertions must be greater than zero".to_string(),
        });
    }
    if config.search.maximum_path_length == 0
        || !config.search.length_tie_relative_epsilon.is_finite()
        || config.search.length_tie_relative_epsilon < 0.0
        || config.search.length_tie_relative_epsilon >= 1.0
    {
        return Err(LeppPostQualityError::InvalidConfig {
            message: "search config is invalid".to_string(),
        });
    }
    if config.gates.maximum_vertex_degree < 3 {
        return Err(LeppPostQualityError::InvalidConfig {
            message: "maximum_vertex_degree must be at least three".to_string(),
        });
    }
    Ok(())
}

pub(crate) fn quality_snapshot(
    mesh: &MeshState,
    config: &LeppPostQualityConfig,
) -> Result<LeppQualitySnapshot, LeppPostQualityError> {
    let violations = face_violations(mesh, config)?;
    let objective = objective_from_violations(&violations)?;
    Ok(LeppQualitySnapshot {
        worst_violation: objective.worst,
        total_violation: objective.total,
        violating_faces: violations
            .iter()
            .filter(|(_, violation)| *violation > 0.0)
            .count(),
    })
}

fn objective(
    mesh: &MeshState,
    config: &LeppPostQualityConfig,
) -> Result<Objective, LeppPostQualityError> {
    objective_from_violations(&face_violations(mesh, config)?)
}

fn objective_from_violations(
    violations: &[(FaceId, f64)],
) -> Result<Objective, LeppPostQualityError> {
    let mut worst = 0.0f64;
    let mut total = 0.0f64;
    for &(_, violation) in violations {
        worst = worst.max(violation);
        total += violation;
    }
    if !worst.is_finite() || !total.is_finite() {
        return Err(LeppPostQualityError::InvalidMesh {
            message: "non-finite quality objective".to_string(),
        });
    }
    Ok(Objective { worst, total })
}

fn face_violations(
    mesh: &MeshState,
    config: &LeppPostQualityConfig,
) -> Result<Vec<(FaceId, f64)>, LeppPostQualityError> {
    // ponytail: only the read-only scan is parallel; ordered scan below keeps FP sums/errors deterministic.
    let scanned: Vec<_> = (earthmesh_mesh::MESH_STATE_FIRST_ID..mesh.triangles().len())
        .into_par_iter()
        .map(|face| face_violation(mesh, face, config).map(|violation| (face, violation)))
        .collect();
    let mut violations = Vec::with_capacity(scanned.len());
    for result in scanned {
        violations.push(result?);
    }
    Ok(violations)
}

fn worst_candidate(
    mesh: &MeshState,
    config: &LeppPostQualityConfig,
    skipped: &BTreeSet<StableFaceId>,
) -> Result<Option<Candidate>, LeppPostQualityError> {
    let mut best: Option<Candidate> = None;
    for (face, violation) in face_violations(mesh, config)? {
        if violation <= 0.0 {
            continue;
        }
        let id = mesh
            .face_id(face)
            .ok_or_else(|| LeppPostQualityError::InvalidMesh {
                message: format!("face {face} has no active stable id"),
            })?;
        if skipped.contains(&id) {
            continue;
        }
        let candidate = Candidate {
            face,
            violation,
            id,
        };
        if best.is_none_or(|best| better_candidate(candidate, best)) {
            best = Some(candidate);
        }
    }
    Ok(best)
}

fn better_candidate(candidate: Candidate, current: Candidate) -> bool {
    if (candidate.violation - current.violation).abs()
        > IMPROVEMENT_EPSILON
            * candidate
                .violation
                .abs()
                .max(current.violation.abs())
                .max(1.0)
    {
        return candidate.violation > current.violation;
    }
    candidate.id.slot < current.id.slot
        || (candidate.id.slot == current.id.slot && candidate.id.generation < current.id.generation)
}

fn strictly_improves(candidate: Objective, baseline: Objective) -> bool {
    candidate.worst + IMPROVEMENT_EPSILON < baseline.worst
        || ((candidate.worst - baseline.worst).abs() <= IMPROVEMENT_EPSILON
            && candidate.total + IMPROVEMENT_EPSILON < baseline.total)
}

pub(crate) fn strictly_improves_quality_snapshot(
    candidate: LeppQualitySnapshot,
    baseline: LeppQualitySnapshot,
) -> bool {
    strictly_improves(
        Objective {
            worst: candidate.worst_violation,
            total: candidate.total_violation,
        },
        Objective {
            worst: baseline.worst_violation,
            total: baseline.total_violation,
        },
    )
}

fn face_violation(
    mesh: &MeshState,
    face: FaceId,
    config: &LeppPostQualityConfig,
) -> Result<f64, LeppPostQualityError> {
    let corners = *mesh
        .triangles()
        .get(face)
        .ok_or_else(|| LeppPostQualityError::InvalidMesh {
            message: format!("face {face} is out of range"),
        })?;
    let radius = average_radius(mesh, &corners)?;
    let [a, b, c] = [
        point_at(mesh, face, corners[0])?,
        point_at(mesh, face, corners[1])?,
        point_at(mesh, face, corners[2])?,
    ];
    let mut violation = 0.0f64;
    if let Some(maximum) = config.maximum_edge_length {
        for (left, right) in [(a, b), (b, c), (c, a)] {
            let length = spherical_edge_length(radius, left, right);
            if !length.is_finite() || length <= 0.0 {
                return Err(LeppPostQualityError::InvalidMesh {
                    message: format!("face {face} has invalid edge length"),
                });
            }
            violation = violation.max((length / maximum) - 1.0);
        }
    }
    if let Some(minimum) = config.minimum_spherical_triangle_angle_degrees {
        for angle in triangle_angles_degrees([a, b, c])? {
            violation = violation.max((minimum - angle) / minimum);
        }
    }
    Ok(violation.max(0.0))
}

fn point_at(
    mesh: &MeshState,
    face: FaceId,
    vertex: usize,
) -> Result<CartesianPoint, LeppPostQualityError> {
    mesh.vertices()
        .get(vertex)
        .copied()
        .ok_or_else(|| LeppPostQualityError::InvalidMesh {
            message: format!("face {face} names invalid vertex {vertex}"),
        })
}

fn average_radius(mesh: &MeshState, corners: &[usize; 3]) -> Result<f64, LeppPostQualityError> {
    let mut total = 0.0;
    for &corner in corners {
        let point =
            *mesh
                .vertices()
                .get(corner)
                .ok_or_else(|| LeppPostQualityError::InvalidMesh {
                    message: format!("invalid vertex {corner}"),
                })?;
        let radius = magnitude(point);
        if !radius.is_finite() || radius <= 0.0 {
            return Err(LeppPostQualityError::InvalidMesh {
                message: format!("vertex {corner} has invalid radius"),
            });
        }
        total += radius;
    }
    Ok(total / 3.0)
}

fn triangle_angles_degrees(points: [CartesianPoint; 3]) -> Result<[f64; 3], LeppPostQualityError> {
    let mut angles = [0.0; 3];
    for i in 0..3 {
        let previous = unit(points[(i + 2) % 3])?;
        let current = unit(points[i])?;
        let next = unit(points[(i + 1) % 3])?;
        let to_previous = tangent_toward(current, previous)?;
        let to_next = tangent_toward(current, next)?;
        let angle = dot(to_previous, to_next)
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        if !angle.is_finite() || angle <= 0.0 {
            return Err(LeppPostQualityError::InvalidMesh {
                message: "invalid spherical triangle angle".to_string(),
            });
        }
        angles[i] = angle;
    }
    Ok(angles)
}

fn tangent_toward(
    from: CartesianPoint,
    to: CartesianPoint,
) -> Result<CartesianPoint, LeppPostQualityError> {
    let projected = CartesianPoint::new(
        to.x - from.x * dot(from, to),
        to.y - from.y * dot(from, to),
        to.z - from.z * dot(from, to),
    );
    unit(projected)
}

fn unit(point: CartesianPoint) -> Result<CartesianPoint, LeppPostQualityError> {
    let norm = magnitude(point);
    if !norm.is_finite() || norm <= 0.0 {
        return Err(LeppPostQualityError::InvalidMesh {
            message: "zero or non-finite vector".to_string(),
        });
    }
    Ok(CartesianPoint::new(
        point.x / norm,
        point.y / norm,
        point.z / norm,
    ))
}
