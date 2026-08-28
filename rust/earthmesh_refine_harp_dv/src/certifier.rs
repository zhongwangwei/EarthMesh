//! Read-only mesh certification for separating topology from geometry evidence.

use std::collections::{BTreeMap, BTreeSet};

use earthmesh_mesh::{arc_length_unit_sphere, LonLatDegrees};

use crate::candidate::CandidateSource;
use crate::criteria::{triangle_angles_deg, CellCriterion, CellView};
use crate::error::{HarpDvError, Result};
use crate::state::{AdaptiveMesh, AdaptiveSite, SiteId};

const LOW_ANGLE_DEG: f64 = 40.0;
const HIGH_ANGLE_DEG: f64 = 80.0;
pub(crate) const TARGET_SCALE_GRADIENT_LIMIT: f64 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleViolationKind {
    Below40,
    Above80,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AngleKey {
    pub triangle_sites: [SiteId; 3],
    pub corner_site: SiteId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BirthSourceClass {
    Inherited,
    Candidate(CandidateSource),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineageCohortKey {
    pub birth_source_class: BirthSourceClass,
    pub refinement_depth: u16,
    pub birth_cycle: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineageAngleExposure {
    pub active_site_count: usize,
    pub sites_with_violation_count: usize,
    pub measurable_angle_count: usize,
    pub below_40_count: usize,
    pub above_80_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefinementBoundaryClass {
    Neither,
    LineageOnly,
    RawCriterionOnly,
    Both,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetGradientBin {
    Unavailable,
    Le0_25,
    Gt0_25Le0_5,
    Gt0_5Le1,
    Gt1Le2,
    Gt2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TriangleContextKey {
    pub refinement_boundary_class: RefinementBoundaryClass,
    pub raw_criterion_target_gradient_bin: TargetGradientBin,
    pub frozen_gradated_target_gradient_bin: TargetGradientBin,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TriangleContextAngleExposure {
    pub measurable_angle_count: usize,
    pub below_40_count: usize,
    pub above_80_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AngleViolation {
    pub key: Option<AngleKey>,
    pub kind: AngleViolationKind,
    pub triangle: usize,
    pub corner_vertex: usize,
    pub angle_deg: f64,
    pub corner_degree: usize,
    pub triangle_degree_triplet: [usize; 3],
    pub refinement_depth: Option<u16>,
    pub birth_cycle: Option<u32>,
    pub birth_candidate_source: Option<CandidateSource>,
    pub lineage_depth_span: Option<u16>,
    pub raw_target_coverage_count: u8,
    pub refinement_boundary_class: RefinementBoundaryClass,
    pub raw_criterion_target_gradient_to_limit_ratio: Option<f64>,
    pub frozen_gradated_target_gradient_to_limit_ratio: Option<f64>,
    /// Realised cell scale divided by the raw criterion target resampled at
    /// the vertex's current position, not the optimiser's frozen gradated field.
    pub realized_to_raw_criterion_target_scale_ratio: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshCertification {
    pub vertex_count: usize,
    pub edge_count: usize,
    pub triangle_count: usize,
    pub open_edge_count: usize,
    pub topology_error_count: usize,
    pub euler_characteristic: isize,
    pub degree_sum: usize,
    pub twice_edge_count: usize,
    pub euler_degree_charge: isize,
    pub degree_histogram: BTreeMap<usize, usize>,
    pub measurable_angle_count: usize,
    pub min_angle_deg: Option<f64>,
    pub p1_angle_deg: Option<f64>,
    pub p99_angle_deg: Option<f64>,
    pub max_angle_deg: Option<f64>,
    pub below_40_count: usize,
    pub above_80_count: usize,
    pub unmeasurable_triangle_count: usize,
    pub unmeasurable_angle_count: usize,
    pub violating_angles_at_degree_le_4: usize,
    pub violating_angles_at_degree_ge_5: usize,
    pub unmapped_identity_count: usize,
    pub attribution_closure_error_count: usize,
    pub lineage_angle_exposure: BTreeMap<LineageCohortKey, LineageAngleExposure>,
    pub triangle_context_angle_exposure: BTreeMap<TriangleContextKey, TriangleContextAngleExposure>,
    pub violations: Vec<AngleViolation>,
}

pub fn certify_mesh(mesh: &AdaptiveMesh, criteria: &[&dyn CellCriterion]) -> MeshCertification {
    certify_mesh_with_frozen_target_scales(mesh, criteria, None)
}

pub(crate) fn certify_mesh_with_frozen_target_scales(
    mesh: &AdaptiveMesh,
    criteria: &[&dyn CellCriterion],
    frozen_target_scales: Option<&[f64]>,
) -> MeshCertification {
    let state = mesh.state();
    let triangles = state.active_triangle_slots().collect::<Vec<_>>();
    let vertices = state.active_vertex_slots().collect::<Vec<_>>();
    let radius_m = state.sphere_radius();
    let mut raw_target_scales = vec![None; state.vertices().len()];
    if !criteria.is_empty() {
        for &vertex in &vertices {
            raw_target_scales[vertex] = minimum_target(
                criteria,
                earthmesh_mesh::xyz_to_lonlat_degrees(state.vertices()[vertex]),
                radius_m,
            );
        }
    }
    let frozen_target_scale_error = frozen_target_scales.is_some_and(|scales| {
        scales.len() < state.vertices().len()
            || vertices.iter().any(|&vertex| {
                scales
                    .get(vertex)
                    .is_none_or(|value| !value.is_finite() || *value <= 0.0)
            })
    });
    let mut edges = BTreeSet::new();
    let mut degrees = vec![0usize; state.vertices().len()];
    let mut seeds = vec![None; state.vertices().len()];

    for &triangle in &triangles {
        let corners = state.triangles()[triangle];
        for corner in 0..3 {
            let a = corners[(corner + 1) % 3];
            let b = corners[(corner + 2) % 3];
            edges.insert(if a <= b { (a, b) } else { (b, a) });
        }
        for vertex in corners {
            degrees[vertex] += 1;
            seeds[vertex].get_or_insert(triangle);
        }
    }

    let mut degree_histogram = BTreeMap::new();
    for &vertex in &vertices {
        *degree_histogram.entry(degrees[vertex]).or_insert(0) += 1;
    }

    let mut angles = Vec::new();
    let mut violations = Vec::new();
    let mut below_40_count = 0;
    let mut above_80_count = 0;
    let mut unmeasurable_triangle_count = 0;
    let mut unmeasurable_angle_count = 0;
    let mut violating_angles_at_degree_le_4 = 0;
    let mut violating_angles_at_degree_ge_5 = 0;
    let mut unmapped_vertices = BTreeSet::new();
    let mut lineage_angle_exposure = BTreeMap::<LineageCohortKey, LineageAngleExposure>::new();
    let mut site_violation_keys = BTreeSet::<(LineageCohortKey, SiteId)>::new();
    let mut triangle_context_angle_exposure =
        BTreeMap::<TriangleContextKey, TriangleContextAngleExposure>::new();

    for &vertex in &vertices {
        let Some(site) = mesh.site_for_vertex(vertex) else {
            unmapped_vertices.insert(vertex);
            continue;
        };
        lineage_angle_exposure
            .entry(lineage_key(site))
            .or_default()
            .active_site_count += 1;
    }

    for &triangle in &triangles {
        let corners = state.triangles()[triangle];
        let corner_sites = corners.map(|vertex| mesh.site_for_vertex(vertex));
        let Some(triangle_angles) =
            triangle_angles_deg(corners.map(|vertex| state.vertices()[vertex]))
        else {
            unmeasurable_triangle_count += 1;
            unmeasurable_angle_count += 3;
            continue;
        };
        if triangle_angles.iter().any(|angle| !angle.is_finite()) {
            unmeasurable_triangle_count += 1;
            unmeasurable_angle_count += 3;
            continue;
        }

        let mut degree_triplet = corners.map(|vertex| degrees[vertex]);
        degree_triplet.sort_unstable();
        let context = triangle_context(
            mesh,
            corners,
            corner_sites,
            &raw_target_scales,
            frozen_target_scales,
        );
        let context_key = context.key;
        triangle_context_angle_exposure
            .entry(context_key)
            .or_default()
            .measurable_angle_count += 3;
        for corner in 0..3 {
            let angle = triangle_angles[corner];
            angles.push(angle);
            let kind = if angle < LOW_ANGLE_DEG {
                below_40_count += 1;
                Some(AngleViolationKind::Below40)
            } else if angle > HIGH_ANGLE_DEG {
                above_80_count += 1;
                Some(AngleViolationKind::Above80)
            } else {
                None
            };
            let vertex = corners[corner];
            if let Some(site) = corner_sites[corner] {
                let key = lineage_key(site);
                let exposure = lineage_angle_exposure.entry(key).or_default();
                exposure.measurable_angle_count += 1;
                match kind {
                    Some(AngleViolationKind::Below40) => exposure.below_40_count += 1,
                    Some(AngleViolationKind::Above80) => exposure.above_80_count += 1,
                    None => {}
                }
                if kind.is_some() {
                    site_violation_keys.insert((key, site.site_id));
                }
            }
            let Some(kind) = kind else { continue };
            match kind {
                AngleViolationKind::Below40 => {
                    triangle_context_angle_exposure
                        .entry(context_key)
                        .or_default()
                        .below_40_count += 1;
                }
                AngleViolationKind::Above80 => {
                    triangle_context_angle_exposure
                        .entry(context_key)
                        .or_default()
                        .above_80_count += 1;
                }
            }
            if degrees[vertex] <= 4 {
                violating_angles_at_degree_le_4 += 1;
            } else {
                violating_angles_at_degree_ge_5 += 1;
            }
            let site = corner_sites[corner];
            violations.push(AngleViolation {
                key: angle_key(corner_sites, corner),
                kind,
                triangle,
                corner_vertex: vertex,
                angle_deg: angle,
                corner_degree: degrees[vertex],
                triangle_degree_triplet: degree_triplet,
                refinement_depth: site.map(|site| site.depth),
                birth_cycle: site.map(|site| site.birth_cycle),
                birth_candidate_source: site.and_then(|site| site.birth_candidate_source),
                lineage_depth_span: context.lineage_depth_span,
                raw_target_coverage_count: context.raw_target_coverage_count,
                refinement_boundary_class: context.key.refinement_boundary_class,
                raw_criterion_target_gradient_to_limit_ratio: context.raw_gradient_to_limit_ratio,
                frozen_gradated_target_gradient_to_limit_ratio: context
                    .frozen_gradient_to_limit_ratio,
                realized_to_raw_criterion_target_scale_ratio:
                    realized_to_raw_criterion_target_scale_ratio(
                        mesh,
                        vertex,
                        seeds[vertex],
                        radius_m,
                        raw_target_scales[vertex],
                    ),
            });
        }
    }

    angles.sort_by(|a, b| a.total_cmp(b));
    let vertex_count = vertices.len();
    let edge_count = edges.len();
    let triangle_count = triangles.len();
    let degree_sum = vertices
        .iter()
        .map(|&vertex| degrees[vertex])
        .sum::<usize>();

    for (key, _) in site_violation_keys {
        lineage_angle_exposure
            .entry(key)
            .or_default()
            .sites_with_violation_count += 1;
    }

    let unmapped_identity_count = unmapped_vertices.len();
    let attribution_closure_error_count = closure_error_count(
        unmapped_identity_count,
        vertex_count,
        [angles.len(), below_40_count, above_80_count],
        frozen_target_scale_error,
        &lineage_angle_exposure,
        &triangle_context_angle_exposure,
    );

    violations.sort_by_key(|violation| violation.key);

    MeshCertification {
        vertex_count,
        edge_count,
        triangle_count,
        open_edge_count: state.open_edge_count(),
        topology_error_count: state.validate().err().map_or(0, |errors| errors.len()),
        euler_characteristic: vertex_count as isize - edge_count as isize + triangle_count as isize,
        degree_sum,
        twice_edge_count: edge_count * 2,
        euler_degree_charge: vertices
            .iter()
            .map(|&vertex| 6isize - degrees[vertex] as isize)
            .sum(),
        degree_histogram,
        measurable_angle_count: angles.len(),
        min_angle_deg: angles.first().copied(),
        p1_angle_deg: percentile(&angles, 1),
        p99_angle_deg: percentile(&angles, 99),
        max_angle_deg: angles.last().copied(),
        below_40_count,
        above_80_count,
        unmeasurable_triangle_count,
        unmeasurable_angle_count,
        violating_angles_at_degree_le_4,
        violating_angles_at_degree_ge_5,
        unmapped_identity_count,
        attribution_closure_error_count,
        lineage_angle_exposure,
        triangle_context_angle_exposure,
        violations,
    }
}

pub(crate) fn keyed_angle_violations(
    mesh: &AdaptiveMesh,
) -> Result<BTreeMap<AngleKey, AngleViolationKind>> {
    let state = mesh.state();
    let mut keyed = BTreeMap::new();
    for triangle in state.active_triangle_slots() {
        let corners = state.triangles()[triangle];
        let corner_sites = corners.map(|vertex| mesh.site_for_vertex(vertex));
        if corner_sites.iter().any(Option::is_none) {
            return Err(HarpDvError::TopologyViolation(
                "active triangle corner is missing a stable SiteId".to_string(),
            ));
        }
        let Some(angles) = triangle_angles_deg(corners.map(|vertex| state.vertices()[vertex]))
        else {
            continue;
        };
        if angles.iter().any(|angle| !angle.is_finite()) {
            continue;
        }
        for (corner, angle) in angles.into_iter().enumerate() {
            let kind = if angle < LOW_ANGLE_DEG {
                AngleViolationKind::Below40
            } else if angle > HIGH_ANGLE_DEG {
                AngleViolationKind::Above80
            } else {
                continue;
            };
            let key = angle_key(corner_sites, corner).ok_or_else(|| {
                HarpDvError::TopologyViolation(
                    "angle violation is missing a stable AngleKey".to_string(),
                )
            })?;
            if keyed.insert(key, kind).is_some() {
                return Err(HarpDvError::TopologyViolation(
                    "duplicate stable AngleKey in active mesh".to_string(),
                ));
            }
        }
    }
    Ok(keyed)
}

pub(crate) fn validate_trace_closure(certification: &MeshCertification) -> Result<()> {
    if certification.unmapped_identity_count > 0
        || certification.attribution_closure_error_count > 0
        || certification
            .violations
            .iter()
            .any(|violation| violation.key.is_none())
    {
        return Err(HarpDvError::InvalidMesh(
            "HARP trace attribution does not close over stable SiteId identities".to_string(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct TriangleContext {
    key: TriangleContextKey,
    lineage_depth_span: Option<u16>,
    raw_target_coverage_count: u8,
    raw_gradient_to_limit_ratio: Option<f64>,
    frozen_gradient_to_limit_ratio: Option<f64>,
}

fn triangle_context(
    mesh: &AdaptiveMesh,
    corners: [usize; 3],
    corner_sites: [Option<&AdaptiveSite>; 3],
    raw_target_scales: &[Option<f64>],
    frozen_target_scales: Option<&[f64]>,
) -> TriangleContext {
    let [left, middle, right] = corner_sites.map(|site| site.map(|site| site.depth));
    let lineage_depth_span = match (left, middle, right) {
        (Some(left), Some(middle), Some(right)) => {
            Some(left.max(middle).max(right) - left.min(middle).min(right))
        }
        _ => None,
    };
    let raw_targets = corners.map(|vertex| raw_target_scales.get(vertex).copied().flatten());
    let raw_target_coverage_count =
        raw_targets.iter().filter(|target| target.is_some()).count() as u8;
    let raw_gradient_to_limit_ratio = gradient_to_limit(mesh, corners, raw_targets);
    let frozen_gradient_to_limit_ratio = frozen_target_scales.and_then(|target| {
        let targets = corners.map(|vertex| {
            target
                .get(vertex)
                .copied()
                .filter(|value| value.is_finite() && *value > 0.0)
        });
        gradient_to_limit(mesh, corners, targets)
    });
    let refinement_boundary_class = match lineage_depth_span {
        None => RefinementBoundaryClass::Unknown,
        Some(span) => match (span > 0, matches!(raw_target_coverage_count, 1 | 2)) {
            (false, false) => RefinementBoundaryClass::Neither,
            (true, false) => RefinementBoundaryClass::LineageOnly,
            (false, true) => RefinementBoundaryClass::RawCriterionOnly,
            (true, true) => RefinementBoundaryClass::Both,
        },
    };
    TriangleContext {
        key: TriangleContextKey {
            refinement_boundary_class,
            raw_criterion_target_gradient_bin: gradient_bin(raw_gradient_to_limit_ratio),
            frozen_gradated_target_gradient_bin: gradient_bin(frozen_gradient_to_limit_ratio),
        },
        lineage_depth_span,
        raw_target_coverage_count,
        raw_gradient_to_limit_ratio,
        frozen_gradient_to_limit_ratio,
    }
}

fn lineage_key(site: &AdaptiveSite) -> LineageCohortKey {
    LineageCohortKey {
        birth_source_class: birth_source_class(site),
        refinement_depth: site.depth,
        birth_cycle: site.birth_cycle,
    }
}

fn birth_source_class(site: &AdaptiveSite) -> BirthSourceClass {
    match (site.birth_cycle, site.depth, site.birth_candidate_source) {
        (0, 0, None) => BirthSourceClass::Inherited,
        (cycle, depth, Some(source)) if cycle > 0 && depth > 0 => {
            BirthSourceClass::Candidate(source)
        }
        _ => BirthSourceClass::Unknown,
    }
}

fn gradient_to_limit(
    mesh: &AdaptiveMesh,
    corners: [usize; 3],
    targets: [Option<f64>; 3],
) -> Option<f64> {
    let [ta, tb, tc] = targets;
    let targets = [ta?, tb?, tc?];
    let state = mesh.state();
    let vertices = state.vertices();
    let edges = [(0, 1), (1, 2), (2, 0)];
    let mut max_gradient = 0.0f64;
    for (left, right) in edges {
        let length = arc_length_unit_sphere(vertices[corners[left]], vertices[corners[right]]);
        if !length.is_finite() || length <= 0.0 {
            return None;
        }
        let gradient = (targets[left] - targets[right]).abs() / length;
        if !gradient.is_finite() {
            return None;
        }
        max_gradient = max_gradient.max(gradient);
    }
    Some(max_gradient / TARGET_SCALE_GRADIENT_LIMIT)
}

fn gradient_bin(ratio: Option<f64>) -> TargetGradientBin {
    let Some(ratio) = ratio.filter(|ratio| ratio.is_finite()) else {
        return TargetGradientBin::Unavailable;
    };
    if ratio <= 0.25 {
        TargetGradientBin::Le0_25
    } else if ratio <= 0.5 {
        TargetGradientBin::Gt0_25Le0_5
    } else if ratio <= 1.0 {
        TargetGradientBin::Gt0_5Le1
    } else if ratio <= 2.0 {
        TargetGradientBin::Gt1Le2
    } else {
        TargetGradientBin::Gt2
    }
}

fn closure_error_count(
    unmapped_identity_count: usize,
    vertex_count: usize,
    [measurable_angle_count, below_40_count, above_80_count]: [usize; 3],
    frozen_target_scale_error: bool,
    lineage: &BTreeMap<LineageCohortKey, LineageAngleExposure>,
    context: &BTreeMap<TriangleContextKey, TriangleContextAngleExposure>,
) -> usize {
    let mut errors = usize::from(unmapped_identity_count > 0);
    errors += usize::from(frozen_target_scale_error);
    let lineage_sites = lineage
        .values()
        .map(|row| row.active_site_count)
        .sum::<usize>();
    let lineage_measurable = lineage
        .values()
        .map(|row| row.measurable_angle_count)
        .sum::<usize>();
    let lineage_below = lineage
        .values()
        .map(|row| row.below_40_count)
        .sum::<usize>();
    let lineage_above = lineage
        .values()
        .map(|row| row.above_80_count)
        .sum::<usize>();
    let context_measurable = context
        .values()
        .map(|row| row.measurable_angle_count)
        .sum::<usize>();
    let context_below = context
        .values()
        .map(|row| row.below_40_count)
        .sum::<usize>();
    let context_above = context
        .values()
        .map(|row| row.above_80_count)
        .sum::<usize>();

    errors += usize::from(lineage_sites != vertex_count);
    errors += usize::from(lineage_measurable != measurable_angle_count);
    errors += usize::from(lineage_below != below_40_count);
    errors += usize::from(lineage_above != above_80_count);
    errors += usize::from(context_measurable != measurable_angle_count);
    errors += usize::from(context_below != below_40_count);
    errors += usize::from(context_above != above_80_count);
    errors += lineage
        .values()
        .filter(|row| {
            row.sites_with_violation_count > row.active_site_count
                || row.below_40_count + row.above_80_count > row.measurable_angle_count
        })
        .count();
    errors += context
        .values()
        .filter(|row| row.below_40_count + row.above_80_count > row.measurable_angle_count)
        .count();
    errors
}

fn angle_key(corner_sites: [Option<&AdaptiveSite>; 3], corner: usize) -> Option<AngleKey> {
    let [Some(left), Some(middle), Some(right)] = corner_sites else {
        return None;
    };
    let mut triangle_sites = [left.site_id, middle.site_id, right.site_id];
    triangle_sites.sort_unstable();
    Some(AngleKey {
        triangle_sites,
        corner_site: corner_sites[corner]?.site_id,
    })
}

fn percentile(sorted: &[f64], percent: usize) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    // Match the zero-based convention used by the HARP-DV cycle eta report.
    sorted
        .get((sorted.len() * percent / 100).min(sorted.len() - 1))
        .copied()
}

fn realized_to_raw_criterion_target_scale_ratio(
    mesh: &AdaptiveMesh,
    vertex: usize,
    seed: Option<usize>,
    radius_m: f64,
    raw_target_scale: Option<f64>,
) -> Option<f64> {
    let state = mesh.state();
    let cell = state.voronoi_cell_from(vertex, seed?).ok()?;
    let view = CellView {
        site: vertex,
        cell: &cell,
        state,
        radius_m,
    };
    let scale = view.effective_scale_m()?;
    let target = raw_target_scale?;
    (target > 0.0 && target.is_finite()).then_some(scale / target)
}

fn minimum_target(
    criteria: &[&dyn CellCriterion],
    point: LonLatDegrees,
    radius_m: f64,
) -> Option<f64> {
    criteria
        .iter()
        .filter_map(|criterion| criterion.target_scale_m_at(point, radius_m))
        .filter(|target| target.is_finite() && *target > 0.0)
        .min_by(|a, b| a.total_cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, CartesianPoint, MeshState, TriangularMesh};
    use earthmesh_refine::{CriterionSemantics, DemandEvidence};

    use crate::transaction::{Acceptance, HardGates};

    fn point(x: f64, y: f64, z: f64) -> CartesianPoint {
        CartesianPoint::new(x, y, z)
    }

    fn permissive() -> HardGates {
        HardGates {
            min_triangle_angle_deg: 0.0,
            ..HardGates::default()
        }
    }

    fn sphere(nxp: usize) -> AdaptiveMesh {
        let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
        AdaptiveMesh::from_triangular_mesh(&mesh).expect("adaptive mesh")
    }

    fn on(mesh: &AdaptiveMesh, lon: f64, lat: f64) -> CartesianPoint {
        let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
        let radius = mesh.state().sphere_radius();
        CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
    }

    struct ConstantTarget(f64);

    impl CellCriterion for ConstantTarget {
        fn id(&self) -> &str {
            "constant"
        }

        fn semantics(&self) -> CriterionSemantics {
            CriterionSemantics::TargetScale
        }

        fn evaluate(&self, _cell: &CellView<'_>) -> crate::Result<DemandEvidence> {
            Ok(DemandEvidence::satisfied(self.id(), self.semantics()))
        }

        fn target_scale_m_at(&self, _point: LonLatDegrees, _radius_m: f64) -> Option<f64> {
            Some(self.0)
        }
    }

    fn tetrahedron() -> AdaptiveMesh {
        AdaptiveMesh::from_mesh_state(
            MeshState::from_parts(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(0.0, 0.0, 0.0),
                    point(1.0, 1.0, 1.0),
                    point(1.0, -1.0, -1.0),
                    point(-1.0, 1.0, -1.0),
                    point(-1.0, -1.0, 1.0),
                ],
                vec![
                    [1, 1, 1],
                    [1, 1, 1],
                    [2, 3, 4],
                    [2, 4, 5],
                    [2, 5, 3],
                    [3, 5, 4],
                ],
            )
            .expect("tetrahedron"),
        )
        .expect("adaptive tetrahedron")
    }

    #[test]
    fn certifier_reports_closed_sphere_degree_three_topology_and_is_read_only() {
        let mesh = tetrahedron();
        let before_state = mesh.state().clone();
        let before_sites = mesh.sites().to_vec();

        let report = certify_mesh(&mesh, &[]);

        assert_eq!(mesh.state(), &before_state);
        assert_eq!(mesh.sites(), before_sites.as_slice());
        assert_eq!(report.vertex_count, 4);
        assert_eq!(report.edge_count, 6);
        assert_eq!(report.triangle_count, 4);
        assert_eq!(report.open_edge_count, 0);
        assert_eq!(report.topology_error_count, 0);
        assert_eq!(report.euler_characteristic, 2);
        assert_eq!(report.degree_sum, report.twice_edge_count);
        assert_eq!(report.euler_degree_charge, 12);
        assert_eq!(report.degree_histogram.get(&3), Some(&4));
        assert_eq!(report.measurable_angle_count, 12);
        assert_eq!(report.unmeasurable_angle_count, 0);
        assert_eq!(report.below_40_count, 0);
        assert_eq!(report.above_80_count, 12);
        assert_eq!(report.violating_angles_at_degree_le_4, 12);
        assert_eq!(report.violating_angles_at_degree_ge_5, 0);
        assert!(report.violations.iter().all(|violation| {
            violation.corner_degree == 3
                && violation.triangle_degree_triplet == [3, 3, 3]
                && violation.refinement_depth == Some(0)
                && violation.birth_cycle == Some(0)
                && violation.lineage_depth_span == Some(0)
                && violation.raw_target_coverage_count == 0
                && violation.refinement_boundary_class == RefinementBoundaryClass::Neither
                && violation
                    .raw_criterion_target_gradient_to_limit_ratio
                    .is_none()
                && violation
                    .frozen_gradated_target_gradient_to_limit_ratio
                    .is_none()
                && violation
                    .realized_to_raw_criterion_target_scale_ratio
                    .is_none()
        }));

        let inherited = LineageCohortKey {
            birth_source_class: BirthSourceClass::Inherited,
            refinement_depth: 0,
            birth_cycle: 0,
        };
        assert_eq!(
            report.lineage_angle_exposure.get(&inherited),
            Some(&LineageAngleExposure {
                active_site_count: 4,
                sites_with_violation_count: 4,
                measurable_angle_count: 12,
                below_40_count: 0,
                above_80_count: 12,
            })
        );
        let context = TriangleContextKey {
            refinement_boundary_class: RefinementBoundaryClass::Neither,
            raw_criterion_target_gradient_bin: TargetGradientBin::Unavailable,
            frozen_gradated_target_gradient_bin: TargetGradientBin::Unavailable,
        };
        assert_eq!(
            report.triangle_context_angle_exposure.get(&context),
            Some(&TriangleContextAngleExposure {
                measurable_angle_count: 12,
                below_40_count: 0,
                above_80_count: 12,
            })
        );
        assert_eq!(report.unmapped_identity_count, 0);
        assert_eq!(report.attribution_closure_error_count, 0);
        validate_trace_closure(&report).expect("closed attribution");
    }

    #[test]
    fn certifier_marks_mixed_depth_triangles_as_lineage_boundary() {
        let mut mesh = sphere(6);
        let candidate = crate::candidate::Candidate {
            point: on(&mesh, 41.0, 19.0),
            source: CandidateSource::OffCentre,
            hint: 20,
        };
        match mesh
            .propose_candidate_for(candidate, permissive(), 20)
            .expect("proposal")
        {
            Acceptance::Committed(_) => {}
            Acceptance::RolledBack(reason) => panic!("candidate rolled back: {reason:?}"),
        };

        let report = certify_mesh(&mesh, &[]);

        assert!(report
            .lineage_angle_exposure
            .contains_key(&LineageCohortKey {
                birth_source_class: BirthSourceClass::Candidate(CandidateSource::OffCentre),
                refinement_depth: 1,
                birth_cycle: 1,
            }));
        assert!(report
            .triangle_context_angle_exposure
            .contains_key(&TriangleContextKey {
                refinement_boundary_class: RefinementBoundaryClass::LineageOnly,
                raw_criterion_target_gradient_bin: TargetGradientBin::Unavailable,
                frozen_gradated_target_gradient_bin: TargetGradientBin::Unavailable,
            }));
        assert_eq!(report.attribution_closure_error_count, 0);
        validate_trace_closure(&report).expect("closed attribution");
    }

    #[test]
    fn certifier_separates_raw_and_frozen_gradient_bins() {
        let mesh = tetrahedron();
        let criterion = ConstantTarget(1.0);
        let criteria: [&dyn CellCriterion; 1] = [&criterion];
        let mut frozen = vec![1.0; mesh.state().vertices().len()];
        frozen[2] = 10.0;

        let report = certify_mesh_with_frozen_target_scales(&mesh, &criteria, Some(&frozen));

        assert!(report
            .triangle_context_angle_exposure
            .contains_key(&TriangleContextKey {
                refinement_boundary_class: RefinementBoundaryClass::Neither,
                raw_criterion_target_gradient_bin: TargetGradientBin::Le0_25,
                frozen_gradated_target_gradient_bin: TargetGradientBin::Gt2,
            }));
        assert!(report.violations.iter().any(|violation| {
            violation.raw_target_coverage_count == 3
                && violation.raw_criterion_target_gradient_to_limit_ratio == Some(0.0)
                && violation
                    .frozen_gradated_target_gradient_to_limit_ratio
                    .is_some_and(|ratio| ratio > 2.0)
        }));
        assert_eq!(report.attribution_closure_error_count, 0);
        validate_trace_closure(&report).expect("closed attribution");
    }

    #[test]
    fn percentile_matches_the_harp_dv_zero_based_convention() {
        let values = (0..=100).map(f64::from).collect::<Vec<_>>();
        assert_eq!(percentile(&values, 1), Some(1.0));
        assert_eq!(percentile(&values, 99), Some(99.0));
    }
}
