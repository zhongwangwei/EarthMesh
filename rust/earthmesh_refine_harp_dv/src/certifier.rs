//! Read-only mesh certification for separating topology from geometry evidence.

use std::collections::{BTreeMap, BTreeSet};

use earthmesh_mesh::LonLatDegrees;

use crate::candidate::CandidateSource;
use crate::criteria::{triangle_angles_deg, CellCriterion, CellView};
use crate::state::{AdaptiveMesh, SiteId};

const LOW_ANGLE_DEG: f64 = 40.0;
const HIGH_ANGLE_DEG: f64 = 80.0;

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
    pub violations: Vec<AngleViolation>,
}

pub fn certify_mesh(mesh: &AdaptiveMesh, criteria: &[&dyn CellCriterion]) -> MeshCertification {
    let state = mesh.state();
    let triangles = state.active_triangle_slots().collect::<Vec<_>>();
    let vertices = state.active_vertex_slots().collect::<Vec<_>>();
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

    for &triangle in &triangles {
        let corners = state.triangles()[triangle];
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
            let Some(kind) = kind else { continue };
            let vertex = corners[corner];
            if degrees[vertex] <= 4 {
                violating_angles_at_degree_le_4 += 1;
            } else {
                violating_angles_at_degree_ge_5 += 1;
            }
            let site = mesh.site_for_vertex(vertex);
            violations.push(AngleViolation {
                key: angle_key(mesh, corners, vertex),
                kind,
                triangle,
                corner_vertex: vertex,
                angle_deg: angle,
                corner_degree: degrees[vertex],
                triangle_degree_triplet: degree_triplet,
                refinement_depth: site.map(|site| site.depth),
                birth_cycle: site.map(|site| site.birth_cycle),
                birth_candidate_source: site.and_then(|site| site.birth_candidate_source),
                realized_to_raw_criterion_target_scale_ratio:
                    realized_to_raw_criterion_target_scale_ratio(
                        mesh,
                        vertex,
                        seeds[vertex],
                        criteria,
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
        violations,
    }
}

fn angle_key(mesh: &AdaptiveMesh, triangle: [usize; 3], corner_vertex: usize) -> Option<AngleKey> {
    let mut triangle_sites: [SiteId; 3] = triangle
        .map(|vertex| mesh.site_for_vertex(vertex).map(|site| site.site_id))
        .into_iter()
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()?;
    triangle_sites.sort_unstable();
    Some(AngleKey {
        triangle_sites,
        corner_site: mesh.site_for_vertex(corner_vertex)?.site_id,
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
    criteria: &[&dyn CellCriterion],
) -> Option<f64> {
    if criteria.is_empty() {
        return None;
    }
    let state = mesh.state();
    let cell = state.voronoi_cell_from(vertex, seed?).ok()?;
    let radius_m = state.sphere_radius();
    let view = CellView {
        site: vertex,
        cell: &cell,
        state,
        radius_m,
    };
    let scale = view.effective_scale_m()?;
    let target = minimum_target(criteria, view.centre(), radius_m)?;
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

    use earthmesh_mesh::{CartesianPoint, MeshState};

    fn point(x: f64, y: f64, z: f64) -> CartesianPoint {
        CartesianPoint::new(x, y, z)
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
        assert!(report
            .violations
            .iter()
            .all(|violation| violation.corner_degree == 3
                && violation.triangle_degree_triplet == [3, 3, 3]
                && violation.refinement_depth == Some(0)
                && violation.birth_cycle == Some(0)
                && violation
                    .realized_to_raw_criterion_target_scale_ratio
                    .is_none()));
    }

    #[test]
    fn percentile_matches_the_harp_dv_zero_based_convention() {
        let values = (0..=100).map(f64::from).collect::<Vec<_>>();
        assert_eq!(percentile(&values, 1), Some(1.0));
        assert_eq!(percentile(&values, 99), Some(99.0));
    }
}
