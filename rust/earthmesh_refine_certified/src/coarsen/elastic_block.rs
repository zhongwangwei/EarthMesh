//! Deterministic coordinate-only repair of a closed transition topology.

use super::{relocation_step_window, HierarchyLeafMesh, TransitionTopologyCandidate};
use crate::{
    certificate::{
        spherical_triangle_angles, voronoi_cell_is_convex_and_contains_site, Certificate,
        GeometryCertificateReport,
    },
    coarsen::TransitionTopologyTrial,
};
use earthmesh_mesh::{
    arc_length_unit_sphere, cross, in_circle_on_sphere, magnitude, orientation_on_sphere,
    CartesianPoint, MeshState, Sign,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElasticBlockLimits {
    pub elastic_iterations: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticPatch {
    pub topology: TransitionTopologyCandidate,
    pub reference_positions: Vec<CartesianPoint>,
    pub fixed_compact_vertices: Vec<usize>,
    pub movable_compact_vertices: Vec<usize>,
    pub guard_faces: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticBlockReport {
    pub component_id: u64,
    pub topology_id: usize,
    pub elastic_iterations: usize,
    pub initial_energy: f64,
    pub final_energy: f64,
    pub moved_compact_vertices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticBlockTrial {
    pub mesh: HierarchyLeafMesh,
    pub patch: ElasticPatch,
    pub geometry: GeometryCertificateReport,
    pub report: ElasticBlockReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElasticBlockOutcome {
    Certified(Box<ElasticBlockTrial>),
    ElasticNoImprovement {
        elastic_iterations: usize,
        initial_energy: f64,
        final_energy: f64,
        reason: String,
    },
    SearchBudgetExhausted {
        elastic_iterations: usize,
        initial_energy: f64,
        final_energy: f64,
    },
    InvalidPatch {
        reason: String,
    },
}

#[derive(Clone, Copy)]
enum EnergyPhase {
    Feasibility,
    Interior,
}

struct EnergyContext {
    degrees: Vec<usize>,
    guard_edges: Vec<(usize, usize)>,
    guard_faces: Vec<usize>,
    guard_seeds: Vec<(usize, usize)>,
    reference_dual_areas: BTreeMap<usize, f64>,
}

struct DualEnergy {
    hard_feasible: bool,
    violation: f64,
    center: f64,
    area: f64,
}

impl ElasticPatch {
    pub fn from_transition(trial: &TransitionTopologyTrial) -> Result<Self, String> {
        let mesh = &trial.mesh.mesh;
        if trial.mesh.source_vertex_slots.len() != mesh.vertices().len() {
            return Err("transition source-slot map does not match compact vertices".into());
        }

        let source_to_compact = trial
            .mesh
            .source_vertex_slots
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(compact, source)| source.map(|source| (source, compact)))
            .collect::<BTreeMap<_, _>>();
        let fixed_sources = trial
            .boundary
            .fine_outer_cycles
            .iter()
            .chain(&trial.boundary.coarse_inner_cycles)
            .flat_map(|cycle| cycle.iter().copied())
            .chain(trial.boundary.seam.iter().copied())
            .chain(trial.boundary.pentagon.iter().copied())
            .collect::<BTreeSet<_>>();

        let transition_parents = trial
            .boundary
            .halo_parents
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut transition_compact_vertices = trial
            .candidate
            .source_active_vertices
            .iter()
            .copied()
            .map(|source| {
                source_to_compact.get(&source).copied().ok_or_else(|| {
                    format!("transition source vertex {source} is absent from the compact mesh")
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        for face in mesh.active_triangle_slots() {
            let belongs_to_transition = trial.mesh.triangle_addresses[face]
                .and_then(|address| address.parent_2_to_1())
                .is_some_and(|parent| transition_parents.contains(&parent));
            if belongs_to_transition {
                transition_compact_vertices.extend(mesh.triangles()[face]);
            }
        }
        let movable_compact_vertices = transition_compact_vertices
            .into_iter()
            .filter(|&compact| {
                trial.mesh.source_vertex_slots[compact]
                    .is_some_and(|source| !fixed_sources.contains(&source))
            })
            .collect::<BTreeSet<_>>();
        if movable_compact_vertices.is_empty() {
            return Err("closed transition topology has no movable interior vertex".into());
        }

        let guard_faces = mesh
            .active_triangle_slots()
            .filter(|&face| {
                mesh.triangles()[face]
                    .iter()
                    .any(|site| movable_compact_vertices.contains(site))
            })
            .collect::<BTreeSet<_>>();
        if guard_faces.is_empty() {
            return Err("movable transition vertices have no incident guard faces".into());
        }
        let fixed_compact_vertices = guard_faces
            .iter()
            .flat_map(|&face| mesh.triangles()[face])
            .filter(|site| !movable_compact_vertices.contains(site))
            .collect::<BTreeSet<_>>();

        Ok(Self {
            topology: trial.candidate.clone(),
            reference_positions: mesh.vertices().to_vec(),
            fixed_compact_vertices: fixed_compact_vertices.into_iter().collect(),
            movable_compact_vertices: movable_compact_vertices.into_iter().collect(),
            guard_faces: guard_faces.into_iter().collect(),
        })
    }
}

pub fn solve_elastic_transition_block(
    transition: &TransitionTopologyTrial,
    limits: ElasticBlockLimits,
) -> ElasticBlockOutcome {
    let patch = match ElasticPatch::from_transition(transition) {
        Ok(patch) => patch,
        Err(reason) => return ElasticBlockOutcome::InvalidPatch { reason },
    };
    solve_elastic_patch(&transition.mesh, patch, limits)
}

pub fn solve_elastic_patch(
    source: &HierarchyLeafMesh,
    patch: ElasticPatch,
    limits: ElasticBlockLimits,
) -> ElasticBlockOutcome {
    if let Err(reason) = validate_patch(source, &patch) {
        return ElasticBlockOutcome::InvalidPatch { reason };
    }
    let certificate = Certificate::internal();
    let mut current = source.clone();
    let input_positions = source.mesh.vertices().to_vec();
    if let Ok(geometry) = certificate.verify_geometry(&current.mesh) {
        return certified(current, patch, geometry, 0, 0.0, 0.0, &input_positions);
    }

    let guard_faces = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();
    let Some((initial_step, minimum_step)) = relocation_step_window(&current.mesh, &guard_faces)
    else {
        return ElasticBlockOutcome::InvalidPatch {
            reason: "transition guard has no positive finite edge length".into(),
        };
    };
    let context = match EnergyContext::new(&current.mesh, &patch) {
        Ok(context) => context,
        Err(reason) => return ElasticBlockOutcome::InvalidPatch { reason },
    };
    let phase = energy_phase(&certificate, &current.mesh, &guard_faces, &context);
    let Some(initial_energy) = elastic_energy(&current.mesh, &patch, phase, &context) else {
        return ElasticBlockOutcome::InvalidPatch {
            reason: "initial transition geometry has undefined elastic energy".into(),
        };
    };
    if limits.elastic_iterations == 0 {
        return ElasticBlockOutcome::SearchBudgetExhausted {
            elastic_iterations: 0,
            initial_energy,
            final_energy: initial_energy,
        };
    }

    let no_step = |mesh: &MeshState, iteration: usize, final_energy: f64| {
        ElasticBlockOutcome::ElasticNoImprovement {
            elastic_iterations: iteration,
            initial_energy,
            final_energy,
            reason: geometry_failure_reason(&certificate, mesh),
        }
    };

    let mut energy = initial_energy;
    for iteration in 1..=limits.elastic_iterations {
        let phase = energy_phase(&certificate, &current.mesh, &guard_faces, &context);
        let Some(gradient) =
            finite_difference_gradient(&mut current.mesh, &patch, phase, initial_step, &context)
        else {
            return no_step(&current.mesh, iteration, energy);
        };
        let maximum_norm = gradient
            .iter()
            .map(|(_, vector)| magnitude(*vector))
            .max_by(f64::total_cmp)
            .unwrap_or(0.0);
        if !maximum_norm.is_finite() || maximum_norm <= 1.0e-14 {
            return no_step(&current.mesh, iteration, energy);
        }

        let mut step = initial_step;
        let accepted = loop {
            let scale = -step / maximum_norm;
            let Some(updates) = synchronous_updates(&current.mesh, &gradient, scale) else {
                if step <= minimum_step {
                    break None;
                }
                step = (step * 0.5).max(minimum_step);
                continue;
            };
            for &(site, _, point) in &updates {
                current.mesh.move_vertex(site, point);
            }
            let candidate_energy = elastic_energy(&current.mesh, &patch, phase, &context);
            if candidate_energy.is_some_and(|candidate_energy| {
                candidate_energy < energy - 1.0e-12 * energy.abs().max(1.0)
            }) {
                break candidate_energy;
            }
            for &(site, point, _) in &updates {
                current.mesh.move_vertex(site, point);
            }
            if step <= minimum_step {
                break None;
            }
            step = (step * 0.5).max(minimum_step);
        };

        let Some(candidate_energy) = accepted else {
            return no_step(&current.mesh, iteration, energy);
        };
        energy = candidate_energy;

        if certificate.geometry_region_passes(&current.mesh, &guard_faces) {
            if let Ok(geometry) = certificate.verify_geometry(&current.mesh) {
                return certified(
                    current,
                    patch,
                    geometry,
                    iteration,
                    initial_energy,
                    energy,
                    &input_positions,
                );
            }
        }
    }

    ElasticBlockOutcome::SearchBudgetExhausted {
        elastic_iterations: limits.elastic_iterations,
        initial_energy,
        final_energy: energy,
    }
}

fn geometry_failure_reason(certificate: &Certificate, mesh: &MeshState) -> String {
    match certificate.verify_geometry(mesh) {
        Ok(_) => "geometry passed but the elastic objective had no descent step".into(),
        Err(error) => format!("{error:?}"),
    }
}

fn validate_patch(mesh: &HierarchyLeafMesh, patch: &ElasticPatch) -> Result<(), String> {
    if patch.reference_positions.len() != mesh.mesh.vertices().len() {
        return Err("elastic reference positions do not match compact vertices".into());
    }
    if patch.movable_compact_vertices.is_empty() {
        return Err("elastic patch has no movable transition vertex".into());
    }
    let movable = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if movable.len() != patch.movable_compact_vertices.len()
        || movable.iter().any(|&site| !mesh.mesh.is_vertex_live(site))
    {
        return Err("elastic patch has duplicate or inactive movable vertices".into());
    }
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if !movable.is_disjoint(&fixed) {
        return Err("elastic fixed and movable vertices overlap".into());
    }
    if fixed.len() != patch.fixed_compact_vertices.len()
        || fixed.iter().any(|&site| !mesh.mesh.is_vertex_live(site))
    {
        return Err("elastic patch has duplicate or inactive fixed vertices".into());
    }
    let guard = patch.guard_faces.iter().copied().collect::<BTreeSet<_>>();
    if guard.len() != patch.guard_faces.len()
        || guard.iter().any(|&face| !mesh.mesh.is_triangle_live(face))
    {
        return Err("elastic patch has duplicate or inactive guard faces".into());
    }
    if movable.iter().any(|site| {
        !guard
            .iter()
            .any(|&face| mesh.mesh.triangles()[face].contains(site))
    }) {
        return Err("elastic movable vertex lies outside the guard faces".into());
    }
    if mesh.mesh.active_triangle_slots().any(|face| {
        mesh.mesh.triangles()[face]
            .iter()
            .any(|site| movable.contains(site))
            && !guard.contains(&face)
    }) {
        return Err("elastic guard omits a face incident to a movable vertex".into());
    }
    let expected_fixed = guard
        .iter()
        .flat_map(|&face| mesh.mesh.triangles()[face])
        .filter(|site| !movable.contains(site))
        .collect::<BTreeSet<_>>();
    if fixed != expected_fixed {
        return Err("elastic fixed vertices do not match the guarded block boundary".into());
    }
    Ok(())
}

fn certified(
    mesh: HierarchyLeafMesh,
    patch: ElasticPatch,
    geometry: GeometryCertificateReport,
    elastic_iterations: usize,
    initial_energy: f64,
    final_energy: f64,
    input_positions: &[CartesianPoint],
) -> ElasticBlockOutcome {
    let moved_compact_vertices = patch
        .movable_compact_vertices
        .iter()
        .copied()
        .filter(|&site| mesh.mesh.vertices()[site] != input_positions[site])
        .collect();
    ElasticBlockOutcome::Certified(Box::new(ElasticBlockTrial {
        report: ElasticBlockReport {
            component_id: patch.topology.component_id,
            topology_id: patch.topology.topology_id,
            elastic_iterations,
            initial_energy,
            final_energy,
            moved_compact_vertices,
        },
        mesh,
        patch,
        geometry,
    }))
}

fn energy_phase(
    certificate: &Certificate,
    mesh: &MeshState,
    guard_faces: &BTreeSet<usize>,
    context: &EnergyContext,
) -> EnergyPhase {
    if certificate.geometry_region_passes(mesh, guard_faces)
        && dual_energy(mesh, context).is_some_and(|dual| dual.hard_feasible)
    {
        EnergyPhase::Interior
    } else {
        EnergyPhase::Feasibility
    }
}

impl EnergyContext {
    fn new(mesh: &MeshState, patch: &ElasticPatch) -> Result<Self, String> {
        let mut guard_seeds = BTreeMap::new();
        let mut guard_edges = BTreeSet::new();
        for &face in &patch.guard_faces {
            let triangle = mesh.triangles()[face];
            for site in triangle {
                guard_seeds.entry(site).or_insert(face);
            }
            for edge in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                guard_edges.insert((edge.0.min(edge.1), edge.0.max(edge.1)));
            }
        }
        let reference =
            MeshState::from_parts(patch.reference_positions.clone(), mesh.triangles().to_vec())
                .map_err(|errors| {
                    errors
                        .into_iter()
                        .map(|error| error.to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                })?;
        let mut reference_dual_areas = BTreeMap::new();
        for (&site, &seed) in &guard_seeds {
            let area = reference
                .voronoi_cell_from(site, seed)
                .ok()
                .and_then(|cell| cell.area_on_unit_sphere())
                .filter(|area| area.is_finite() && *area > 0.0)
                .ok_or_else(|| format!("reference Voronoi area is undefined at site {site}"))?;
            reference_dual_areas.insert(site, area);
        }
        Ok(Self {
            degrees: vertex_degrees(mesh),
            guard_edges: guard_edges.into_iter().collect(),
            guard_faces: patch.guard_faces.clone(),
            guard_seeds: guard_seeds.into_iter().collect(),
            reference_dual_areas,
        })
    }
}

fn vertex_degrees(mesh: &MeshState) -> Vec<usize> {
    let mut degrees = vec![0usize; mesh.vertices().len()];
    for face in mesh.active_triangle_slots() {
        for site in mesh.triangles()[face] {
            degrees[site] += 1;
        }
    }
    degrees
}

fn elastic_energy(
    mesh: &MeshState,
    patch: &ElasticPatch,
    phase: EnergyPhase,
    context: &EnergyContext,
) -> Option<f64> {
    let mut energy = 0.0;
    let minimum_angle = 40.2f64.to_radians();
    let maximum_angle = 79.8f64.to_radians();
    for &face in &patch.guard_faces {
        if !mesh.is_triangle_live(face) {
            return None;
        }
        let triangle = mesh.triangles()[face];
        if orientation_on_sphere(
            mesh.vertices()[triangle[0]],
            mesh.vertices()[triangle[1]],
            mesh.vertices()[triangle[2]],
        )
        .ok()?
            != Sign::Positive
        {
            return None;
        }
        let points = triangle.map(|site| mesh.vertices()[site]);
        let angles = spherical_triangle_angles(points)?.map(f64::to_radians);
        for corner in 0..3 {
            let angle = angles[corner];
            let target = std::f64::consts::TAU / context.degrees[triangle[corner]] as f64;
            let below = (minimum_angle - angle).max(0.0);
            let above = (angle - maximum_angle).max(0.0);
            energy += 100.0 * (below * below + above * above);
            match phase {
                EnergyPhase::Feasibility => {}
                EnergyPhase::Interior => {
                    let lower = angle - minimum_angle;
                    let upper = maximum_angle - angle;
                    if lower <= 0.0 || upper <= 0.0 {
                        return None;
                    }
                    energy += 0.2 * (angle - target).powi(2) - 0.001 * (lower.ln() + upper.ln());
                }
            }
        }
        let [Some(a), Some(b), Some(c)] = points.map(normalized_point) else {
            return None;
        };
        let determinant = dot(a, cross(b, c));
        if determinant <= 0.0 || !determinant.is_finite() {
            return None;
        }
        if matches!(phase, EnergyPhase::Interior) {
            energy -= 0.0001 * determinant.ln();
        }
    }

    let edge_weight = match phase {
        EnergyPhase::Feasibility => 0.001,
        EnergyPhase::Interior => 0.01,
    };
    for &(left, right) in &context.guard_edges {
        let length = arc_length_unit_sphere(mesh.vertices()[left], mesh.vertices()[right]);
        let reference = arc_length_unit_sphere(
            patch.reference_positions[left],
            patch.reference_positions[right],
        );
        if length <= 0.0 || reference <= 0.0 || !length.is_finite() || !reference.is_finite() {
            return None;
        }
        energy += edge_weight * (length / reference).ln().powi(2);
    }
    let dual = dual_energy(mesh, context)?;
    energy += 1_000.0 * dual.violation + 0.02 * dual.center;
    energy += match phase {
        EnergyPhase::Feasibility => 0.002 * dual.area,
        EnergyPhase::Interior => 0.02 * dual.area,
    };
    energy.is_finite().then_some(energy)
}

fn dual_energy(mesh: &MeshState, context: &EnergyContext) -> Option<DualEnergy> {
    let mut hard_feasible = true;
    let mut violation = 0.0;
    let mut center = 0.0;
    let mut area = 0.0;

    for &(site, seed) in &context.guard_seeds {
        let cell = mesh.voronoi_cell_from(site, seed).ok()?;
        let degree_violation = 5usize
            .saturating_sub(cell.degree())
            .max(cell.degree().saturating_sub(7));
        hard_feasible &=
            degree_violation == 0 && voronoi_cell_is_convex_and_contains_site(mesh, &cell);
        violation += (degree_violation * degree_violation) as f64;

        let cell_area = cell.area_on_unit_sphere()?;
        if !cell_area.is_finite() || cell_area <= 0.0 {
            return None;
        }
        let target_area = context.reference_dual_areas[&site];
        area += (cell_area / target_area).ln().powi(2);

        let mut centroid_sum = CartesianPoint::new(0.0, 0.0, 0.0);
        for corner in &cell.corners {
            centroid_sum = add_points(centroid_sum, normalized_point(*corner)?);
        }
        let centroid = normalized_point(centroid_sum)?;
        let site_position = normalized_point(mesh.vertices()[site])?;
        center += dot(site_position, centroid).clamp(-1.0, 1.0).acos().powi(2);
    }

    let mut face_pairs = BTreeSet::new();
    for &face in &context.guard_faces {
        let triangle = mesh.triangles()[face];
        for corner in 0..3 {
            let other = mesh.neighbours()[face][corner];
            if other == 0 || !mesh.is_triangle_live(other) {
                return None;
            }
            if !face_pairs.insert((face.min(other), face.max(other))) {
                continue;
            }
            let edge = [triangle[(corner + 1) % 3], triangle[(corner + 2) % 3]];
            let opposite = mesh.triangles()[other]
                .iter()
                .copied()
                .find(|site| !edge.contains(site))?;
            let points = triangle.map(|site| normalized_point(mesh.vertices()[site]));
            let [Some(a), Some(b), Some(c)] = points else {
                return None;
            };
            let d = normalized_point(mesh.vertices()[opposite])?;
            match in_circle_on_sphere(a, b, c, d) {
                Ok(Sign::Positive) | Err(_) => hard_feasible = false,
                Ok(Sign::Negative | Sign::Zero) => {}
            }
            let side = dot(
                subtract_points(a, d),
                cross(subtract_points(b, d), subtract_points(c, d)),
            );
            if !side.is_finite() {
                return None;
            }
            violation += (-side).max(0.0).powi(2);
        }
    }

    Some(DualEnergy {
        hard_feasible,
        violation,
        center,
        area,
    })
}

fn finite_difference_gradient(
    mesh: &mut MeshState,
    patch: &ElasticPatch,
    phase: EnergyPhase,
    initial_step: f64,
    context: &EnergyContext,
) -> Option<Vec<(usize, CartesianPoint)>> {
    // ponytail: finite differences keep PR29 auditable; replace with analytic
    // patch derivatives only if transition-local profiling shows this dominates.
    let epsilon = (initial_step * 1.0e-3).clamp(1.0e-7, 1.0e-5);
    let mut gradient = Vec::with_capacity(patch.movable_compact_vertices.len());
    for &site in &patch.movable_compact_vertices {
        let point = mesh.vertices()[site];
        let [first, second] = tangent_basis(point)?;
        let mut derivative = |direction: CartesianPoint| {
            let plus = exponential_map(point, scale_point(direction, epsilon))?;
            let minus = exponential_map(point, scale_point(direction, -epsilon))?;
            mesh.move_vertex(site, plus);
            let plus_energy = elastic_energy(mesh, patch, phase, context);
            mesh.move_vertex(site, minus);
            let minus_energy = elastic_energy(mesh, patch, phase, context);
            mesh.move_vertex(site, point);
            Some((plus_energy? - minus_energy?) / (2.0 * epsilon))
        };
        let d_first = derivative(first)?;
        let d_second = derivative(second)?;
        gradient.push((
            site,
            add_points(scale_point(first, d_first), scale_point(second, d_second)),
        ));
    }
    Some(gradient)
}

fn synchronous_updates(
    mesh: &MeshState,
    gradient: &[(usize, CartesianPoint)],
    scale: f64,
) -> Option<Vec<(usize, CartesianPoint, CartesianPoint)>> {
    gradient
        .iter()
        .map(|&(site, vector)| {
            exponential_map(mesh.vertices()[site], scale_point(vector, scale))
                .map(|point| (site, mesh.vertices()[site], point))
        })
        .collect()
}

fn exponential_map(point: CartesianPoint, tangent_step: CartesianPoint) -> Option<CartesianPoint> {
    let radius = magnitude(point);
    if radius <= 0.0 || !radius.is_finite() {
        return None;
    }
    let unit = scale_point(point, 1.0 / radius);
    let tangent_step = add_points(tangent_step, scale_point(unit, -dot(unit, tangent_step)));
    let length = magnitude(tangent_step);
    if !length.is_finite() {
        return None;
    }
    if length == 0.0 {
        return Some(point);
    }
    let direction = scale_point(tangent_step, 1.0 / length);
    Some(scale_point(
        add_points(
            scale_point(unit, length.cos()),
            scale_point(direction, length.sin()),
        ),
        radius,
    ))
}

fn tangent_basis(point: CartesianPoint) -> Option<[CartesianPoint; 2]> {
    let unit = normalized_point(point)?;
    let axis = if unit.z.abs() < 0.8 {
        CartesianPoint::new(0.0, 0.0, 1.0)
    } else {
        CartesianPoint::new(1.0, 0.0, 0.0)
    };
    let first = normalized_point(cross(unit, axis))?;
    let second = normalized_point(cross(unit, first))?;
    Some([first, second])
}

fn normalized_point(point: CartesianPoint) -> Option<CartesianPoint> {
    let length = magnitude(point);
    (length > 0.0 && length.is_finite()).then(|| scale_point(point, 1.0 / length))
}

fn dot(left: CartesianPoint, right: CartesianPoint) -> f64 {
    left.x * right.x + left.y * right.y + left.z * right.z
}

fn scale_point(point: CartesianPoint, scale: f64) -> CartesianPoint {
    CartesianPoint::new(point.x * scale, point.y * scale, point.z * scale)
}

fn add_points(left: CartesianPoint, right: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

fn subtract_points(left: CartesianPoint, right: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(left.x - right.x, left.y - right.y, left.z - right.z)
}
