use crate::{
    certificate::{Certificate, GeometryCertificateReport},
    mother_grid::{MotherGrid, TriangleAddress, VertexAddress},
    outcome::GeometryCertifiedMotherGrid,
    remap::{ConservativeRemap, RemapCertificate},
    requirement::{
        certify_final_cell_requirements, graded_envelope, target_site_edges,
        FinalCellRequirementError, FinalCellRequirementReport, SourceLevelField, TargetLevelField,
    },
};
use earthmesh_mesh::{
    arc_length_unit_sphere, cross, magnitude, CartesianPoint, MeshState,
    RetirementPostconditionOutcome, RetirementReport, RetirementSearchOutcome,
};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

const BLOCK_RELOCATION_STEPS: usize = 64;
const BLOCK_RELOCATION_DIRECTIONS: usize = 32;
const BLOCK_RELOCATION_INITIAL_STEP_RADIANS: f64 = 0.04;
const BLOCK_RELOCATION_FINAL_STEP_RADIANS: f64 = 0.0005;
const BLOCK_RELOCATION_INITIAL_EDGE_FRACTION: f64 = 0.25;
const BLOCK_RELOCATION_FINAL_EDGE_FRACTION: f64 = 0.01;

enum BlockRelocationOutcome {
    Certified {
        mesh: Box<MeshState>,
        geometry: GeometryCertificateReport,
        states_examined: usize,
    },
    ProvenInfeasible {
        states_examined: usize,
    },
    SearchBudgetExhausted {
        states_examined: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyPatchCandidate {
    pub parent_face: usize,
}

#[derive(Debug, Clone)]
pub enum HierarchyRebuildOutcome {
    Rebuilt {
        mesh: Box<GeometryCertifiedMotherGrid>,
        removed_vertices: usize,
        removed_faces: usize,
        candidates: Vec<HierarchyPatchCandidate>,
        remap: ConservativeRemap,
        remap_certificate: RemapCertificate,
    },
    SearchBudgetExhausted {
        attempted_patches: usize,
        snapshot_unchanged: bool,
        mesh: MotherGrid,
    },
    UnsupportedCavity {
        reason: String,
        mesh: MotherGrid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoarseningPatch {
    pub vertex: usize,
    pub seed_face: usize,
    pub address: VertexAddress,
    pub level: usize,
    pub parent_faces: Vec<TriangleAddress>,
    pub boundary_cycle: Vec<usize>,
    pub retained_vertices: Vec<usize>,
    pub removable_vertices: Vec<usize>,
    pub transition_halo: Vec<usize>,
    pub requirement_margin: isize,
}

#[derive(Debug, Clone)]
pub enum CavitySolveOutcome {
    Feasible {
        report: Box<RetirementReport>,
        certificate: GeometryCertificateReport,
        states_examined: usize,
    },
    ProvenInfeasible {
        states_examined: usize,
        reason: String,
    },
    SearchBudgetExhausted {
        states_examined: usize,
    },
    InvalidBoundary {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub enum CertifiedCavitySolveOutcome {
    Feasible {
        report: Box<RetirementReport>,
        geometry: GeometryCertificateReport,
        requirements: Box<FinalCellRequirementReport>,
        states_examined: usize,
    },
    ProvenInfeasible {
        states_examined: usize,
        reason: String,
    },
    SearchBudgetExhausted {
        states_examined: usize,
    },
    InvalidBoundary {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertifiedEpochLimits {
    pub max_adjacent_level_delta: usize,
    pub search_state_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochCandidateStatus {
    Committed,
    ProvenInfeasible,
    SearchBudgetExhausted,
    InvalidBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochCandidateAttempt {
    pub epoch: usize,
    pub vertex: usize,
    pub address: VertexAddress,
    pub status: EpochCandidateStatus,
    pub states_examined: usize,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertifiedEpochReport {
    pub initial_vertices: usize,
    pub final_vertices: usize,
    pub epoch_commits: Vec<usize>,
    pub states_examined: usize,
    pub attempts: Vec<EpochCandidateAttempt>,
    pub transition_promotion: TransitionPromotionReport,
}

impl CertifiedEpochReport {
    pub fn epochs(&self) -> usize {
        self.epoch_commits.len()
    }

    pub fn candidates_attempted(&self) -> usize {
        self.attempts.len()
    }

    pub fn candidates_accepted(&self) -> usize {
        self.attempts
            .iter()
            .filter(|attempt| attempt.status == EpochCandidateStatus::Committed)
            .count()
    }

    pub fn vertices_removed(&self) -> usize {
        self.initial_vertices.saturating_sub(self.final_vertices)
    }
}

#[derive(Debug, Clone)]
pub enum CertifiedEpochOutcome {
    Certified {
        report: Box<CertifiedEpochReport>,
        requirements: Box<FinalCellRequirementReport>,
    },
    NotCertifiable {
        report: Box<CertifiedEpochReport>,
        error: FinalCellRequirementError,
    },
    SearchBudgetExhausted {
        report: Box<CertifiedEpochReport>,
    },
    InvalidInput {
        reason: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransitionBlockedRegion {
    pub patch_vertices: Vec<usize>,
    pub patch_addresses: Vec<VertexAddress>,
    pub sites: Vec<usize>,
    pub retained_level: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransitionPromotionReport {
    pub blocked_regions: Vec<TransitionBlockedRegion>,
    pub directly_promoted_sites: usize,
    pub grading_promoted_sites: usize,
}

/// Keep failed cavity components fine, include their transition halo, then
/// apply the existing one-level-per-ring grading envelope. Levels only rise.
pub fn promote_blocked_transition_halos(
    mesh: &MeshState,
    blocked_patches: &[CoarseningPatch],
    delivered_levels_by_site: &mut [Option<usize>],
) -> Result<TransitionPromotionReport, String> {
    if delivered_levels_by_site.len() != mesh.vertices().len()
        || mesh.active_vertex_slots().any(|site| {
            delivered_levels_by_site
                .get(site)
                .is_none_or(Option::is_none)
        })
    {
        return Err("delivered level slots do not match the active target mesh".into());
    }

    let mut regions = Vec::<TransitionBlockedRegion>::new();
    for patch in blocked_patches {
        if !mesh.is_vertex_live(patch.vertex) {
            continue;
        }
        let mut sites = BTreeSet::from([patch.vertex]);
        sites.extend(
            patch
                .retained_vertices
                .iter()
                .copied()
                .filter(|&site| mesh.is_vertex_live(site)),
        );
        for &face in &patch.transition_halo {
            if mesh.is_triangle_live(face) {
                sites.extend(mesh.triangles()[face]);
            }
        }
        let mut region = TransitionBlockedRegion {
            patch_vertices: vec![patch.vertex],
            patch_addresses: vec![patch.address.clone()],
            sites: sites.into_iter().collect(),
            retained_level: patch.level,
        };
        let mut index = 0;
        while index < regions.len() {
            if sorted_sets_intersect(&region.sites, &regions[index].sites) {
                let other = regions.remove(index);
                region.patch_vertices.extend(other.patch_vertices);
                region.patch_addresses.extend(other.patch_addresses);
                region.sites.extend(other.sites);
                region.sites.sort_unstable();
                region.sites.dedup();
                region.retained_level = region.retained_level.max(other.retained_level);
                index = 0;
            } else {
                index += 1;
            }
        }
        regions.push(region);
    }
    regions.sort_by(|left, right| left.patch_addresses.cmp(&right.patch_addresses));
    for region in &mut regions {
        region.patch_vertices.sort_unstable();
        region.patch_vertices.dedup();
        region.patch_addresses.sort_unstable();
        region.patch_addresses.dedup();
    }

    let before = delivered_levels_by_site.to_vec();
    for region in &regions {
        for &site in &region.sites {
            let level =
                delivered_levels_by_site[site].expect("active transition site was validated above");
            delivered_levels_by_site[site] = Some(level.max(region.retained_level));
        }
    }
    let directly_promoted_sites = mesh
        .active_vertex_slots()
        .filter(|&site| delivered_levels_by_site[site] > before[site])
        .count();

    let active_sites = mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut cell_by_site = vec![usize::MAX; mesh.vertices().len()];
    for (cell, &site) in active_sites.iter().enumerate() {
        cell_by_site[site] = cell;
    }
    let mut adjacency = vec![Vec::new(); active_sites.len()];
    for (left, right) in target_site_edges(mesh) {
        let (left, right) = (cell_by_site[left], cell_by_site[right]);
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    let levels = active_sites
        .iter()
        .map(|&site| delivered_levels_by_site[site].expect("active level was validated"))
        .collect::<Vec<_>>();
    let graded = graded_envelope(&adjacency, &levels, 1);
    let mut grading_promoted_sites = 0;
    for (&site, level) in active_sites.iter().zip(graded) {
        if Some(level) > delivered_levels_by_site[site] {
            delivered_levels_by_site[site] = Some(level);
            grading_promoted_sites += 1;
        }
    }
    Ok(TransitionPromotionReport {
        blocked_regions: regions,
        directly_promoted_sites,
        grading_promoted_sites,
    })
}

/// Build stable one-site finite cavity candidates without mutating the mesh.
/// The candidate is the smallest exact search surface supported by the shared
/// mesh kernel; wider hierarchy patches can be added without changing the
/// transaction result semantics.
pub fn coarsening_patch_candidates(
    grid: &MotherGrid,
    required_levels: &[usize],
    coarse_level: usize,
) -> Vec<CoarseningPatch> {
    let level = usize::from(grid.subdivision > 1);
    let mut seeds = vec![usize::MAX; grid.mesh.vertices().len()];
    for face in grid.mesh.active_triangle_slots() {
        for site in grid.mesh.triangles()[face] {
            if seeds[site] == usize::MAX {
                seeds[site] = face;
            }
        }
    }
    let mut patches = removable_hierarchy_sites(grid)
        .into_par_iter()
        .filter_map(|vertex| {
            let address = grid.addresses.get(vertex)?.as_ref()?.clone();
            let seed_face = seeds[vertex];
            let fan = grid.mesh.triangle_fan_from(vertex, seed_face).ok()?;
            let boundary_cycle = retirement_ring(&grid.mesh, vertex, &fan)?;
            let mut parent_faces = fan
                .iter()
                .filter_map(|&face| grid.triangle_addresses.get(face)?.as_ref()?.parent_2_to_1())
                .collect::<Vec<_>>();
            parent_faces.sort_unstable();
            parent_faces.dedup();
            let maximum_requirement = fan
                .iter()
                .flat_map(|&face| grid.mesh.triangles()[face])
                .filter_map(|site| required_levels.get(site).copied())
                .max()
                .unwrap_or(usize::MAX);
            let requirement_margin = signed_margin(coarse_level, maximum_requirement);
            let transition_halo = outside_faces(&grid.mesh, vertex, &fan);
            Some(CoarseningPatch {
                vertex,
                seed_face,
                address,
                level,
                parent_faces,
                retained_vertices: boundary_cycle.clone(),
                boundary_cycle,
                removable_vertices: vec![vertex],
                transition_halo,
                requirement_margin,
            })
        })
        .collect::<Vec<_>>();
    patches.par_sort_unstable_by(|left, right| {
        right
            .requirement_margin
            .cmp(&left.requirement_margin)
            .then_with(|| right.level.cmp(&left.level))
            .then_with(|| left.address.cmp(&right.address))
            .then_with(|| left.vertex.cmp(&right.vertex))
    });
    patches
}

fn relocate_coarsening_block(
    candidate: &MeshState,
    movable_sites: &[usize],
    initial_region: &BTreeSet<usize>,
    search_budget: usize,
) -> BlockRelocationOutcome {
    if search_budget == 0 {
        return BlockRelocationOutcome::SearchBudgetExhausted { states_examined: 0 };
    }
    let mut states_examined = 1;
    let certificate = Certificate::internal();
    let mut current = candidate.clone();
    let mut movable_sites = movable_sites
        .iter()
        .copied()
        .filter(|&site| current.is_vertex_live(site))
        .collect::<Vec<_>>();
    movable_sites.sort_unstable();
    movable_sites.dedup();
    let mut seeds = initial_region
        .iter()
        .copied()
        .filter(|&face| current.is_triangle_live(face))
        .flat_map(|face| {
            current.triangles()[face]
                .into_iter()
                .map(move |site| (site, face))
        })
        .filter(|(site, _)| movable_sites.binary_search(site).is_ok())
        .collect::<Vec<_>>();
    seeds.sort_unstable_by_key(|&(site, _)| site);
    seeds.dedup_by_key(|(site, _)| *site);
    let mut incident = BTreeSet::new();
    for &site in &movable_sites {
        let Ok(seed) = seeds.binary_search_by_key(&site, |&(candidate, _)| candidate) else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        let Ok(fan) = current.triangle_fan_from(site, seeds[seed].1) else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        incident.extend(fan);
    }
    let Ok((_, touched)) = current.legalize_within_with_touched(&incident, Some(&incident)) else {
        return BlockRelocationOutcome::ProvenInfeasible { states_examined };
    };
    if current.validate_region(&touched).is_err() {
        return BlockRelocationOutcome::ProvenInfeasible { states_examined };
    }
    let mut changed_region = initial_region.clone();
    changed_region.extend(incident.iter().copied());
    changed_region.extend(touched);
    let Some((initial_step, final_step)) = relocation_step_window(&current, &incident) else {
        return BlockRelocationOutcome::ProvenInfeasible { states_examined };
    };

    for step_index in 0..BLOCK_RELOCATION_STEPS {
        if certificate.geometry_region_passes(&current, &changed_region) {
            if let Ok(report) = certificate.verify_geometry(&current) {
                return BlockRelocationOutcome::Certified {
                    mesh: Box::new(current),
                    geometry: report,
                    states_examined,
                };
            }
        }
        let fraction = step_index as f64 / (BLOCK_RELOCATION_STEPS - 1) as f64;
        let step = initial_step * (1.0 - fraction) + final_step * fraction;
        let Some(current_penalty) = certificate.geometry_penalty_in(&current, &incident) else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        let mut best = None::<(f64, usize, CartesianPoint, BTreeSet<usize>)>;
        for &site in &movable_sites {
            let point = current.vertices()[site];
            let Some(seed) = incident.iter().copied().find(|&face| {
                current.is_triangle_live(face) && current.triangles()[face].contains(&site)
            }) else {
                continue;
            };
            let Ok(site_incident) = current.triangle_fan_from(site, seed) else {
                continue;
            };
            let site_incident = site_incident.into_iter().collect::<BTreeSet<_>>();
            for direction in 0..BLOCK_RELOCATION_DIRECTIONS {
                if states_examined == search_budget {
                    return BlockRelocationOutcome::SearchBudgetExhausted { states_examined };
                }
                states_examined += 1;
                let Some(destination) = relocation_destination(point, direction, step) else {
                    continue;
                };
                let snapshot = current.snapshot_around(&incident);
                current.move_vertex(site, destination);
                let trial = current
                    .legalize_within_with_touched(&site_incident, Some(&incident))
                    .ok()
                    .and_then(|(_, touched)| {
                        current
                            .validate_region(&touched)
                            .is_ok()
                            .then(|| certificate.geometry_penalty_in(&current, &incident))
                            .flatten()
                            .map(|penalty| (penalty, touched))
                    });
                current.move_vertex(site, point);
                current
                    .restore_patch(snapshot)
                    .expect("bounded legalization rollback restores its local rows");
                let Some((penalty, touched)) = trial else {
                    continue;
                };
                let improvement = penalty - current_penalty;
                if improvement >= -1.0e-12
                    || best
                        .as_ref()
                        .is_some_and(|(best_improvement, _, _, _)| improvement >= *best_improvement)
                {
                    continue;
                }
                best = Some((improvement, site, destination, touched));
            }
        }
        let Some((_, site, destination, expected_touched)) = best else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        let Some(seed) = incident.iter().copied().find(|&face| {
            current.is_triangle_live(face) && current.triangles()[face].contains(&site)
        }) else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        let Ok(site_incident) = current.triangle_fan_from(site, seed) else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        current.move_vertex(site, destination);
        let Ok((_, touched)) = current
            .legalize_within_with_touched(&site_incident.into_iter().collect(), Some(&incident))
        else {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        };
        debug_assert_eq!(touched, expected_touched);
        if current.validate_region(&touched).is_err() {
            return BlockRelocationOutcome::ProvenInfeasible { states_examined };
        }
        changed_region.extend(touched);
    }
    match certificate.verify_geometry(&current) {
        Ok(geometry) => BlockRelocationOutcome::Certified {
            mesh: Box::new(current),
            geometry,
            states_examined,
        },
        Err(_) => BlockRelocationOutcome::ProvenInfeasible { states_examined },
    }
}

fn relocation_step_window(mesh: &MeshState, faces: &BTreeSet<usize>) -> Option<(f64, f64)> {
    let local_edge = faces
        .iter()
        .copied()
        .filter(|&face| mesh.is_triangle_live(face))
        .flat_map(|face| {
            let triangle = mesh.triangles()[face];
            [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ]
        })
        .filter_map(|(left, right)| {
            let angle = arc_length_unit_sphere(mesh.vertices()[left], mesh.vertices()[right]);
            (angle.is_finite() && angle > 0.0).then_some(angle)
        })
        .min_by(f64::total_cmp)?;
    Some((
        BLOCK_RELOCATION_INITIAL_STEP_RADIANS
            .min(local_edge * BLOCK_RELOCATION_INITIAL_EDGE_FRACTION),
        BLOCK_RELOCATION_FINAL_STEP_RADIANS.min(local_edge * BLOCK_RELOCATION_FINAL_EDGE_FRACTION),
    ))
}

fn relocation_destination(
    point: CartesianPoint,
    direction: usize,
    step_radians: f64,
) -> Option<CartesianPoint> {
    let radius = magnitude(point);
    if radius <= 0.0 || !radius.is_finite() {
        return None;
    }
    let unit = scale_point(point, 1.0 / radius);
    let axis = if unit.z.abs() < 0.8 {
        CartesianPoint::new(0.0, 0.0, 1.0)
    } else {
        CartesianPoint::new(1.0, 0.0, 0.0)
    };
    let first = normalized_point(cross(unit, axis))?;
    let second = normalized_point(cross(unit, first))?;
    let azimuth = std::f64::consts::TAU * direction as f64 / BLOCK_RELOCATION_DIRECTIONS as f64;
    let tangent = add_points(
        scale_point(first, azimuth.cos()),
        scale_point(second, azimuth.sin()),
    );
    Some(scale_point(
        add_points(
            scale_point(unit, step_radians.cos()),
            scale_point(tangent, step_radians.sin()),
        ),
        radius,
    ))
}

fn normalized_point(point: CartesianPoint) -> Option<CartesianPoint> {
    let length = magnitude(point);
    (length > 0.0 && length.is_finite()).then(|| scale_point(point, 1.0 / length))
}

fn scale_point(point: CartesianPoint, scale: f64) -> CartesianPoint {
    CartesianPoint::new(point.x * scale, point.y * scale, point.z * scale)
}

fn add_points(left: CartesianPoint, right: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

/// Exhaustively search the supported finite ring unless the explicit state
/// budget is reached. Failed and exhausted searches leave `mesh` unchanged.
pub fn solve_coarsening_patch(
    mesh: &mut MeshState,
    patch: &CoarseningPatch,
    search_budget: usize,
) -> CavitySolveOutcome {
    if patch.removable_vertices.as_slice() != [patch.vertex] {
        return CavitySolveOutcome::InvalidBoundary {
            reason: "the supported cavity must retire exactly its named hierarchy site".into(),
        };
    }
    let mut accepted_certificate = None;
    match mesh.retire_vertex_with_budget_transactionally(
        patch.vertex,
        search_budget,
        |candidate, _| match Certificate::internal().verify_geometry(candidate) {
            Ok(certificate) => {
                accepted_certificate = Some(certificate);
                true
            }
            Err(_) => false,
        },
    ) {
        RetirementSearchOutcome::Committed { report, attempted } => CavitySolveOutcome::Feasible {
            report: Box::new(report),
            certificate: accepted_certificate
                .expect("a committed cavity passed the certificate postcondition"),
            states_examined: attempted,
        },
        RetirementSearchOutcome::ProvenInfeasible {
            attempted,
            last_error,
        } => CavitySolveOutcome::ProvenInfeasible {
            states_examined: attempted,
            reason: last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "the finite cavity has no triangulation".into()),
        },
        RetirementSearchOutcome::SearchBudgetExhausted { attempted } => {
            CavitySolveOutcome::SearchBudgetExhausted {
                states_examined: attempted,
            }
        }
        RetirementSearchOutcome::InvalidBoundary(error) => CavitySolveOutcome::InvalidBoundary {
            reason: error.to_string(),
        },
    }
}

/// Run one finite cavity as an atomic geometry + final-cell physical + balance
/// + conservative-overlap transaction.
///
/// `delivered_levels_by_site` uses mesh slots so retirement can tombstone a
/// site without renumbering any survivor.
#[allow(clippy::too_many_arguments)]
pub fn solve_certified_coarsening_patch(
    mesh: &mut MeshState,
    patch: &CoarseningPatch,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    delivered_levels_by_site: &mut [Option<usize>],
    coarse_level: usize,
    max_adjacent_level_delta: usize,
    search_budget: usize,
) -> CertifiedCavitySolveOutcome {
    if patch.removable_vertices.as_slice() != [patch.vertex] {
        return CertifiedCavitySolveOutcome::InvalidBoundary {
            reason: "the supported cavity must retire exactly its named hierarchy site".into(),
        };
    }
    if delivered_levels_by_site.len() != mesh.vertices().len()
        || mesh.active_vertex_slots().any(|site| {
            delivered_levels_by_site
                .get(site)
                .is_none_or(Option::is_none)
        })
    {
        return CertifiedCavitySolveOutcome::InvalidBoundary {
            reason: "delivered level slots do not match the active target mesh".into(),
        };
    }

    let mut accepted_geometry = None;
    let mut accepted_requirements = None;
    let mut accepted_levels = None;
    let mut accepted_relocated_mesh = None;
    match mesh.retire_vertex_with_budget_transactionally_repairing(
        patch.vertex,
        search_budget,
        |candidate, retirement, relocation_budget| {
            let mut relocation_states = 0;
            let certificate = Certificate::internal();
            let mut changed_region = retirement
                .fan
                .iter()
                .copied()
                .filter(|&face| candidate.is_triangle_live(face))
                .collect::<BTreeSet<_>>();
            changed_region.extend(
                patch
                    .transition_halo
                    .iter()
                    .copied()
                    .filter(|&face| candidate.is_triangle_live(face)),
            );
            let direct_geometry = certificate
                .geometry_region_passes(candidate, &changed_region)
                .then(|| certificate.verify_geometry(candidate))
                .and_then(Result::ok);
            let (relocated, geometry) = match direct_geometry {
                Some(geometry) => (None, geometry),
                None => {
                    let mut movable_sites = patch.retained_vertices.clone();
                    for &face in &patch.transition_halo {
                        if candidate.is_triangle_live(face) {
                            movable_sites.extend(candidate.triangles()[face]);
                        }
                    }
                    match relocate_coarsening_block(
                        candidate,
                        &movable_sites,
                        &changed_region,
                        relocation_budget,
                    ) {
                        BlockRelocationOutcome::Certified {
                            mesh,
                            geometry,
                            states_examined,
                        } => {
                            relocation_states = states_examined;
                            (Some(*mesh), geometry)
                        }
                        BlockRelocationOutcome::ProvenInfeasible { states_examined } => {
                            return RetirementPostconditionOutcome::Rejected { states_examined };
                        }
                        BlockRelocationOutcome::SearchBudgetExhausted { states_examined } => {
                            return RetirementPostconditionOutcome::SearchBudgetExhausted {
                                states_examined,
                            };
                        }
                    }
                }
            };
            let target_mesh = relocated.as_ref().unwrap_or(candidate);
            let mut target_levels = target_mesh
                .active_vertex_slots()
                .map(|site| {
                    let level = delivered_levels_by_site[site]
                        .expect("active target site was validated above");
                    if patch.retained_vertices.contains(&site) {
                        level.min(coarse_level)
                    } else {
                        level
                    }
                })
                .collect::<Vec<_>>();
            let Ok(mut target_field) =
                TargetLevelField::from_active_voronoi_cells(target_mesh, target_levels.clone())
            else {
                return RetirementPostconditionOutcome::Rejected {
                    states_examined: relocation_states,
                };
            };
            let requirements = match certify_final_cell_requirements(
                source_mesh,
                source_levels,
                target_mesh,
                &target_field,
                max_adjacent_level_delta,
            ) {
                Ok(requirements) => requirements,
                Err(FinalCellRequirementError::Residuals(report)) => {
                    for (delivered, &required) in
                        target_levels.iter_mut().zip(report.required_levels())
                    {
                        *delivered = (*delivered).max(required);
                    }
                    if !target_mesh.active_vertex_slots().zip(&target_levels).any(
                        |(site, &level)| {
                            patch.retained_vertices.contains(&site) && level <= coarse_level
                        },
                    ) {
                        return RetirementPostconditionOutcome::Rejected {
                            states_examined: relocation_states,
                        };
                    }
                    let Ok(promoted) = TargetLevelField::from_active_voronoi_cells(
                        target_mesh,
                        target_levels.clone(),
                    ) else {
                        return RetirementPostconditionOutcome::Rejected {
                            states_examined: relocation_states,
                        };
                    };
                    target_field = promoted;
                    let Ok(requirements) = certify_final_cell_requirements(
                        source_mesh,
                        source_levels,
                        target_mesh,
                        &target_field,
                        max_adjacent_level_delta,
                    ) else {
                        return RetirementPostconditionOutcome::Rejected {
                            states_examined: relocation_states,
                        };
                    };
                    requirements
                }
                Err(FinalCellRequirementError::InvalidInput(_)) => {
                    return RetirementPostconditionOutcome::Rejected {
                        states_examined: relocation_states,
                    };
                }
            };
            accepted_geometry = Some(geometry);
            accepted_requirements = Some(requirements);
            accepted_levels = Some(target_levels);
            accepted_relocated_mesh = relocated;
            RetirementPostconditionOutcome::Accepted {
                states_examined: relocation_states,
            }
        },
    ) {
        RetirementSearchOutcome::Committed { report, attempted } => {
            if let Some(relocated) = accepted_relocated_mesh {
                *mesh = relocated;
            }
            let target_levels =
                accepted_levels.expect("a committed certified cavity staged delivered levels");
            delivered_levels_by_site[patch.vertex] = None;
            for (site, level) in mesh.active_vertex_slots().zip(target_levels) {
                delivered_levels_by_site[site] = Some(level);
            }
            CertifiedCavitySolveOutcome::Feasible {
                report: Box::new(report),
                geometry: accepted_geometry.expect("a committed certified cavity passed geometry"),
                requirements: Box::new(
                    accepted_requirements
                        .expect("a committed certified cavity passed final-cell requirements"),
                ),
                states_examined: attempted,
            }
        }
        RetirementSearchOutcome::ProvenInfeasible {
            attempted,
            last_error,
        } => CertifiedCavitySolveOutcome::ProvenInfeasible {
            states_examined: attempted,
            reason: last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "the finite certified cavity has no triangulation".into()),
        },
        RetirementSearchOutcome::SearchBudgetExhausted { attempted } => {
            CertifiedCavitySolveOutcome::SearchBudgetExhausted {
                states_examined: attempted,
            }
        }
        RetirementSearchOutcome::InvalidBoundary(error) => {
            CertifiedCavitySolveOutcome::InvalidBoundary {
                reason: error.to_string(),
            }
        }
    }
}

/// Attempt each stable candidate at most once per epoch. Only candidates in a
/// committed patch's one-ring are reconsidered in the next epoch.
pub fn run_certified_coarsening_epochs(
    mesh: &mut MeshState,
    candidates: Vec<CoarseningPatch>,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    delivered_levels_by_site: &mut [Option<usize>],
    coarse_level: usize,
    limits: CertifiedEpochLimits,
) -> CertifiedEpochOutcome {
    if source_levels
        .active_sites()
        .iter()
        .copied()
        .ne(source_mesh.active_vertex_slots())
    {
        return CertifiedEpochOutcome::InvalidInput {
            reason: "source level field active cell ids do not match the source mesh".into(),
        };
    }
    if delivered_levels_by_site.len() != mesh.vertices().len()
        || mesh.active_vertex_slots().any(|site| {
            delivered_levels_by_site
                .get(site)
                .is_none_or(Option::is_none)
        })
    {
        return CertifiedEpochOutcome::InvalidInput {
            reason: "delivered level slots do not match the active target mesh".into(),
        };
    }

    let initial_vertices = mesh.vertex_count();
    let mut pool = candidates;
    sort_and_deduplicate_patches(&mut pool);
    let mut pending = (0..pool.len()).collect::<Vec<_>>();
    let mut remaining_budget = limits.search_state_budget;
    let mut report = CertifiedEpochReport {
        initial_vertices,
        final_vertices: initial_vertices,
        epoch_commits: Vec::new(),
        states_examined: 0,
        attempts: Vec::new(),
        transition_promotion: TransitionPromotionReport::default(),
    };
    let mut blocked_patches = BTreeMap::<usize, CoarseningPatch>::new();

    loop {
        let epoch = report.epoch_commits.len() + 1;
        let mut committed = 0;
        let mut dirty_sites = BTreeSet::new();
        for patch_index in std::mem::take(&mut pending) {
            let patch = pool[patch_index].clone();
            if !mesh.is_vertex_live(patch.vertex) {
                continue;
            }
            if remaining_budget == 0 {
                report.final_vertices = mesh.vertex_count();
                return CertifiedEpochOutcome::SearchBudgetExhausted {
                    report: Box::new(report),
                };
            }
            let candidate_vertex = patch.vertex;
            let candidate_address = patch.address.clone();
            let Some(patch) = refreshed_patch(mesh, patch) else {
                report.attempts.push(EpochCandidateAttempt {
                    epoch,
                    vertex: candidate_vertex,
                    address: candidate_address,
                    status: EpochCandidateStatus::InvalidBoundary,
                    states_examined: 0,
                    reason: Some("candidate no longer has a closed one-ring".into()),
                });
                continue;
            };
            let vertex = patch.vertex;
            let address = patch.address.clone();
            match solve_certified_coarsening_patch(
                mesh,
                &patch,
                source_mesh,
                source_levels,
                delivered_levels_by_site,
                coarse_level,
                limits.max_adjacent_level_delta,
                remaining_budget,
            ) {
                CertifiedCavitySolveOutcome::Feasible {
                    report: cavity,
                    states_examined,
                    ..
                } => {
                    remaining_budget = remaining_budget.saturating_sub(states_examined);
                    report.states_examined += states_examined;
                    report.attempts.push(EpochCandidateAttempt {
                        epoch,
                        vertex,
                        address,
                        status: EpochCandidateStatus::Committed,
                        states_examined,
                        reason: None,
                    });
                    dirty_sites.extend(cavity.ring.iter().copied());
                    dirty_sites.extend(cavity.replacement_faces.iter().flatten().copied());
                    blocked_patches.remove(&vertex);
                    committed += 1;
                }
                CertifiedCavitySolveOutcome::ProvenInfeasible {
                    states_examined,
                    reason,
                } => {
                    remaining_budget = remaining_budget.saturating_sub(states_examined);
                    report.states_examined += states_examined;
                    report.attempts.push(EpochCandidateAttempt {
                        epoch,
                        vertex,
                        address,
                        status: EpochCandidateStatus::ProvenInfeasible,
                        states_examined,
                        reason: Some(reason),
                    });
                    blocked_patches.insert(vertex, patch);
                }
                CertifiedCavitySolveOutcome::SearchBudgetExhausted { states_examined } => {
                    report.states_examined += states_examined;
                    report.attempts.push(EpochCandidateAttempt {
                        epoch,
                        vertex,
                        address,
                        status: EpochCandidateStatus::SearchBudgetExhausted,
                        states_examined,
                        reason: None,
                    });
                    report.final_vertices = mesh.vertex_count();
                    return CertifiedEpochOutcome::SearchBudgetExhausted {
                        report: Box::new(report),
                    };
                }
                CertifiedCavitySolveOutcome::InvalidBoundary { reason } => {
                    report.attempts.push(EpochCandidateAttempt {
                        epoch,
                        vertex,
                        address,
                        status: EpochCandidateStatus::InvalidBoundary,
                        states_examined: 0,
                        reason: Some(reason),
                    });
                }
            }
        }
        report.epoch_commits.push(committed);
        if committed == 0 {
            break;
        }
        pending = pool
            .iter()
            .enumerate()
            .filter(|(_, patch)| mesh.is_vertex_live(patch.vertex))
            .filter_map(|(index, patch)| {
                let patch = refreshed_patch(mesh, patch.clone())?;
                (dirty_sites.contains(&patch.vertex)
                    || patch
                        .retained_vertices
                        .iter()
                        .any(|site| dirty_sites.contains(site)))
                .then_some(index)
            })
            .collect();
    }

    report.final_vertices = mesh.vertex_count();
    report.transition_promotion = match promote_blocked_transition_halos(
        mesh,
        &blocked_patches.into_values().collect::<Vec<_>>(),
        delivered_levels_by_site,
    ) {
        Ok(promotion) => promotion,
        Err(reason) => return CertifiedEpochOutcome::InvalidInput { reason },
    };
    let target_levels = mesh
        .active_vertex_slots()
        .map(|site| delivered_levels_by_site[site].expect("active target levels were validated"))
        .collect::<Vec<_>>();
    let target_levels = match TargetLevelField::from_active_voronoi_cells(mesh, target_levels) {
        Ok(levels) => levels,
        Err(reason) => return CertifiedEpochOutcome::InvalidInput { reason },
    };
    match certify_final_cell_requirements(
        source_mesh,
        source_levels,
        mesh,
        &target_levels,
        limits.max_adjacent_level_delta,
    ) {
        Ok(requirements) => CertifiedEpochOutcome::Certified {
            report: Box::new(report),
            requirements: Box::new(requirements),
        },
        Err(error) => CertifiedEpochOutcome::NotCertifiable {
            report: Box::new(report),
            error,
        },
    }
}

fn sort_and_deduplicate_patches(patches: &mut Vec<CoarseningPatch>) {
    patches.sort_by(|left, right| {
        right
            .requirement_margin
            .cmp(&left.requirement_margin)
            .then_with(|| right.level.cmp(&left.level))
            .then_with(|| left.address.cmp(&right.address))
            .then_with(|| left.vertex.cmp(&right.vertex))
    });
    patches.dedup_by_key(|patch| patch.vertex);
}

fn refreshed_patch(mesh: &MeshState, mut patch: CoarseningPatch) -> Option<CoarseningPatch> {
    let seed_face = if mesh.is_triangle_live(patch.seed_face)
        && mesh.triangles()[patch.seed_face].contains(&patch.vertex)
    {
        patch.seed_face
    } else {
        mesh.active_triangle_slots()
            .find(|&face| mesh.triangles()[face].contains(&patch.vertex))?
    };
    let fan = mesh.triangle_fan_from(patch.vertex, seed_face).ok()?;
    patch.seed_face = *fan.iter().min()?;
    let boundary_cycle = retirement_ring(mesh, patch.vertex, &fan)?;
    patch.boundary_cycle = boundary_cycle.clone();
    patch.retained_vertices = boundary_cycle;
    patch.transition_halo = outside_faces(mesh, patch.vertex, &fan);
    Some(patch)
}

fn sorted_sets_intersect(left: &[usize], right: &[usize]) -> bool {
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].cmp(&right[right_index]) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => return true,
        }
    }
    false
}

pub fn complete_four_child_patch_candidates(grid: &MotherGrid) -> Vec<HierarchyPatchCandidate> {
    let parent_faces = if grid.subdivision.is_multiple_of(2) {
        20 * (grid.subdivision / 2) * (grid.subdivision / 2)
    } else {
        0
    };
    (0..parent_faces)
        .map(|parent_face| HierarchyPatchCandidate { parent_face })
        .collect()
}

pub fn rebuild_one_level_from_complete_mother_patches(
    grid: MotherGrid,
    search_budget: usize,
) -> HierarchyRebuildOutcome {
    if !grid.subdivision.is_multiple_of(2) || grid.subdivision < 2 {
        return HierarchyRebuildOutcome::UnsupportedCavity {
            reason: "only complete four-child mother hierarchy patches are implemented".into(),
            mesh: grid,
        };
    }
    let candidates = complete_four_child_patch_candidates(&grid);
    if search_budget < candidates.len() {
        let snapshot = grid.clone();
        return HierarchyRebuildOutcome::SearchBudgetExhausted {
            attempted_patches: search_budget,
            snapshot_unchanged: snapshot == grid,
            mesh: grid,
        };
    }
    let coarse = match MotherGrid::generate(grid.subdivision / 2) {
        Ok(coarse) => coarse,
        Err(reason) => return HierarchyRebuildOutcome::UnsupportedCavity { reason, mesh: grid },
    };
    let report = match Certificate::final_delivery().verify_mother_grid(&coarse) {
        Ok(report) => report,
        Err(error) => {
            return HierarchyRebuildOutcome::UnsupportedCavity {
                reason: error.to_string(),
                mesh: grid,
            }
        }
    };
    let Some(remap) = ConservativeRemap::hierarchy_2_to_1_average(&coarse, &grid) else {
        return HierarchyRebuildOutcome::UnsupportedCavity {
            reason: "complete hierarchy remap lineage is not closed".into(),
            mesh: grid,
        };
    };
    let remap_certificate = remap.certify_hierarchy_2_to_1_average(&coarse, &grid);
    if remap_certificate.negative_weights()
        + remap_certificate.bad_row_sums()
        + remap_certificate.bad_lineage_rows()
        != 0
        || remap_certificate.constant_closure_error() > remap_certificate.closure_tolerance()
        || remap_certificate.global_area_closure_error() > remap_certificate.closure_tolerance()
    {
        return HierarchyRebuildOutcome::UnsupportedCavity {
            reason: "complete hierarchy remap certificate has residuals".into(),
            mesh: grid,
        };
    }
    HierarchyRebuildOutcome::Rebuilt {
        removed_vertices: grid.mesh.vertex_count() - coarse.mesh.vertex_count(),
        removed_faces: grid.mesh.triangle_count() - coarse.mesh.triangle_count(),
        mesh: Box::new(GeometryCertifiedMotherGrid::new(coarse.mesh, report)),
        candidates,
        remap,
        remap_certificate,
    }
}

#[derive(Debug, Clone)]
pub struct ReverseCoarsenReport {
    pub mesh: Box<GeometryCertifiedMotherGrid>,
    pub attempted_vertices: usize,
    pub committed_vertices: usize,
    pub rejected_vertices: usize,
    pub protected_vertices: usize,
    pub initial_vertices: usize,
    pub final_vertices: usize,
    pub cavity_states_examined: usize,
    pub proven_infeasible_vertices: usize,
    pub budget_exhausted_vertices: usize,
}

#[derive(Debug, Clone)]
pub enum ReverseCoarsenOutcome {
    Completed(ReverseCoarsenReport),
    SearchBudgetExhausted(ReverseCoarsenReport),
    InitialCertificationFailure { reason: String, mesh: MotherGrid },
    FinalCertificationFailure { reason: String, mesh: MeshState },
}

/// Try stable hierarchy-site retirements on an epoch clone and commit the
/// epoch only when its complete internal geometry certificate passes.
///
/// This is intentionally conservative: a protected site or rejected cavity is
/// kept fine. The current postcondition scans the whole trial mesh; a local
/// certificate cache can replace that only when it proves the same gates.
pub fn reverse_coarsen_mother_grid(
    grid: MotherGrid,
    required_levels: &[usize],
    coarse_level: usize,
    search_budget: usize,
) -> ReverseCoarsenOutcome {
    if let Err(error) = Certificate::internal().verify_mother_grid(&grid) {
        return ReverseCoarsenOutcome::InitialCertificationFailure {
            reason: error.to_string(),
            mesh: grid,
        };
    }
    let initial_vertices = grid.mesh.vertex_count();
    let candidates = coarsening_patch_candidates(&grid, required_levels, coarse_level);
    let mut mesh = grid.mesh;
    let mut trial = mesh.clone();
    let mut attempted_vertices = 0;
    let mut committed_vertices = 0;
    let mut rejected_vertices = 0;
    let mut protected_vertices = 0;
    let mut cavity_states_examined = 0;
    let mut proven_infeasible_vertices = 0;
    let mut budget_exhausted_vertices = 0;
    let mut remaining_budget = search_budget;
    let mut exhausted = false;
    for patch in candidates {
        let vertex = patch.vertex;
        if !trial.is_vertex_live(vertex) {
            continue;
        }
        if patch.requirement_margin < 0
            || !one_ring_can_coarsen(&trial, vertex, required_levels, coarse_level)
        {
            protected_vertices += 1;
            continue;
        }
        if remaining_budget == 0 {
            exhausted = true;
            break;
        }
        attempted_vertices += 1;
        match solve_coarsening_patch(&mut trial, &patch, remaining_budget) {
            CavitySolveOutcome::Feasible {
                states_examined, ..
            } => {
                cavity_states_examined += states_examined;
                remaining_budget = remaining_budget.saturating_sub(states_examined);
                committed_vertices += 1;
            }
            CavitySolveOutcome::ProvenInfeasible {
                states_examined, ..
            } => {
                cavity_states_examined += states_examined;
                remaining_budget = remaining_budget.saturating_sub(states_examined);
                proven_infeasible_vertices += 1;
                rejected_vertices += 1;
            }
            CavitySolveOutcome::InvalidBoundary { .. } => {
                rejected_vertices += 1;
            }
            CavitySolveOutcome::SearchBudgetExhausted { states_examined } => {
                cavity_states_examined += states_examined;
                budget_exhausted_vertices += 1;
                exhausted = true;
                break;
            }
        }
    }
    if committed_vertices > 0 {
        if Certificate::internal().verify_geometry(&trial).is_ok() {
            mesh = trial;
        } else {
            rejected_vertices += committed_vertices;
            committed_vertices = 0;
        }
    }
    let report = match Certificate::final_delivery().verify_geometry(&mesh) {
        Ok(certificate) => ReverseCoarsenReport {
            final_vertices: mesh.vertex_count(),
            mesh: Box::new(GeometryCertifiedMotherGrid::new(mesh, certificate)),
            attempted_vertices,
            committed_vertices,
            rejected_vertices,
            protected_vertices,
            initial_vertices,
            cavity_states_examined,
            proven_infeasible_vertices,
            budget_exhausted_vertices,
        },
        Err(error) => {
            return ReverseCoarsenOutcome::FinalCertificationFailure {
                reason: error.to_string(),
                mesh,
            }
        }
    };
    if exhausted {
        ReverseCoarsenOutcome::SearchBudgetExhausted(report)
    } else {
        ReverseCoarsenOutcome::Completed(report)
    }
}

fn signed_margin(allowed: usize, required: usize) -> isize {
    if allowed >= required {
        isize::try_from(allowed - required).unwrap_or(isize::MAX)
    } else {
        -isize::try_from(required - allowed).unwrap_or(isize::MAX)
    }
}

fn retirement_ring(mesh: &MeshState, vertex: usize, fan: &[usize]) -> Option<Vec<usize>> {
    if fan.len() < 3 {
        return None;
    }
    let mut ring = Vec::with_capacity(fan.len());
    for (index, &face) in fan.iter().enumerate() {
        let next = fan[(index + 1) % fan.len()];
        let mut shared = None;
        for site in mesh.triangles()[face] {
            if site != vertex
                && mesh.triangles()[next].contains(&site)
                && shared.replace(site).is_some()
            {
                return None;
            }
        }
        let shared = shared?;
        if ring.contains(&shared) {
            return None;
        }
        ring.push(shared);
    }
    let start = ring.iter().enumerate().min_by_key(|(_, site)| *site)?.0;
    ring.rotate_left(start);
    let mut reverse = ring.iter().copied().rev().collect::<Vec<_>>();
    let reverse_start = reverse.iter().enumerate().min_by_key(|(_, site)| *site)?.0;
    reverse.rotate_left(reverse_start);
    Some(ring.min(reverse))
}

fn outside_faces(mesh: &MeshState, vertex: usize, fan: &[usize]) -> Vec<usize> {
    let mut outside = Vec::with_capacity(fan.len());
    for &face in fan {
        let Some(corner) = mesh.triangles()[face]
            .iter()
            .position(|&candidate| candidate == vertex)
        else {
            continue;
        };
        let neighbour = mesh.neighbours()[face][corner];
        if mesh.is_triangle_live(neighbour) && !fan.contains(&neighbour) {
            outside.push(neighbour);
        }
    }
    outside.sort_unstable();
    outside.dedup();
    outside
}

fn removable_hierarchy_sites(grid: &MotherGrid) -> Vec<usize> {
    grid.addresses
        .iter()
        .enumerate()
        .filter_map(|(site, address)| {
            let removable = match address.as_ref()? {
                crate::mother_grid::VertexAddress::IcosahedronVertex(_) => false,
                crate::mother_grid::VertexAddress::IcosahedronEdge { step, .. } => {
                    !step.is_multiple_of(2)
                }
                crate::mother_grid::VertexAddress::IcosahedronFace { i, j, k, .. } => {
                    !i.is_multiple_of(2) || !j.is_multiple_of(2) || !k.is_multiple_of(2)
                }
            };
            removable.then_some(site)
        })
        .collect()
}

fn one_ring_can_coarsen(
    mesh: &MeshState,
    vertex: usize,
    required_levels: &[usize],
    coarse_level: usize,
) -> bool {
    mesh.active_triangle_slots()
        .filter(|&face| mesh.triangles()[face].contains(&vertex))
        .flat_map(|face| mesh.triangles()[face])
        .all(|site| required_levels.get(site).copied().unwrap_or(usize::MAX) <= coarse_level)
}
