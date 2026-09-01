//! Bounded W2 topology neighbourhood over local spherical 2-2 flips.

use super::{
    solve_elastic_patch_with_max_min_trust_start, ElasticBlockLimits, ElasticBlockOutcome,
    ElasticPatch, GeometryFailureWitness, GeometryStartId, HierarchyLeafMesh,
    ViolationSupportAtlas,
};
use crate::certificate::spherical_triangle_angles;
use earthmesh_mesh::MeshState;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTopologyLimits {
    pub maximum_states: usize,
    pub maximum_flips: usize,
    pub local_geometry_iterations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalEdgeFlip {
    pub face: usize,
    pub neighbour: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalTopologyEvidence {
    pub states_examined: usize,
    pub rejected_flip_count: usize,
    pub topology_gate_rejections: usize,
    pub geometry_candidates: usize,
    pub incumbent_angle_range_deg: (f64, f64),
    pub best_angle_range_deg: (f64, f64),
    pub best_signed_margin_deg: f64,
    pub best_flips: Vec<LocalEdgeFlip>,
    pub incumbent_preserved: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalTopologySearchOutcome {
    StrictCertified {
        trial: Box<GeometryFailureWitness>,
        evidence: LocalTopologyEvidence,
    },
    NoStrictCandidate(LocalTopologyEvidence),
    SearchBudgetExhausted(LocalTopologyEvidence),
    InvalidInput(String),
}

#[derive(Clone)]
struct SearchState {
    mesh: HierarchyLeafMesh,
    flips: Vec<LocalEdgeFlip>,
    used_faces: BTreeSet<usize>,
}

pub fn search_local_topology_neighbourhood(
    incumbent: &GeometryFailureWitness,
    atlas: &ViolationSupportAtlas,
    anchor_source_slots: &BTreeSet<usize>,
    limits: LocalTopologyLimits,
) -> LocalTopologySearchOutcome {
    if limits.maximum_states == 0
        || limits.maximum_flips == 0
        || limits.maximum_flips > 3
        || limits.local_geometry_iterations == 0
    {
        return LocalTopologySearchOutcome::InvalidInput(
            "local topology limits require positive states/iterations and 1..=3 flips".into(),
        );
    }
    if atlas.evidence_sets.optimization_active.is_empty() {
        return LocalTopologySearchOutcome::InvalidInput(
            "local topology search requires active angle supports".into(),
        );
    }
    let Some(incumbent_range) = angle_range(&incumbent.mesh.mesh) else {
        return LocalTopologySearchOutcome::InvalidInput(
            "local topology incumbent has undefined angles".into(),
        );
    };
    let mut evidence = LocalTopologyEvidence {
        states_examined: 0,
        rejected_flip_count: 0,
        topology_gate_rejections: 0,
        geometry_candidates: 0,
        incumbent_angle_range_deg: incumbent_range,
        best_angle_range_deg: incumbent_range,
        best_signed_margin_deg: signed_margin(incumbent_range),
        best_flips: Vec::new(),
        incumbent_preserved: true,
    };
    let seed_faces = atlas
        .evidence_sets
        .optimization_active
        .iter()
        .map(|angle| angle.face)
        .collect::<BTreeSet<_>>();
    let mut frontier = VecDeque::from([SearchState {
        mesh: incumbent.mesh.clone(),
        flips: Vec::new(),
        used_faces: BTreeSet::new(),
    }]);
    let mut seen = BTreeSet::<Vec<[usize; 3]>>::new();
    seen.insert(canonical_triangles(&incumbent.mesh.mesh));

    while let Some(state) = frontier.pop_front() {
        if state.flips.len() == limits.maximum_flips {
            continue;
        }
        for (face, corner, neighbour) in local_flip_edges(&state.mesh.mesh, &seed_faces) {
            if state.used_faces.contains(&face) || state.used_faces.contains(&neighbour) {
                continue;
            }
            if evidence.states_examined == limits.maximum_states {
                return LocalTopologySearchOutcome::SearchBudgetExhausted(evidence);
            }
            let mut candidate = state.mesh.clone();
            if candidate.mesh.flip_edge(face, corner).is_err() {
                evidence.rejected_flip_count += 1;
                continue;
            }
            let fingerprint = canonical_triangles(&candidate.mesh);
            if !seen.insert(fingerprint) {
                continue;
            }
            evidence.states_examined += 1;
            if !topology_gates_pass(&candidate, anchor_source_slots) {
                evidence.topology_gate_rejections += 1;
                continue;
            }
            candidate.triangle_addresses[face] = None;
            candidate.triangle_addresses[neighbour] = None;
            let mut flips = state.flips.clone();
            flips.push(LocalEdgeFlip { face, neighbour });
            let mut changed_faces = state.used_faces.clone();
            changed_faces.extend([face, neighbour]);
            let Some(local_patch) = local_patch(&candidate, &incumbent.patch, &changed_faces)
            else {
                evidence.topology_gate_rejections += 1;
                continue;
            };
            evidence.geometry_candidates += 1;
            let outcome = solve_elastic_patch_with_max_min_trust_start(
                &candidate,
                local_patch,
                ElasticBlockLimits {
                    elastic_iterations: limits.local_geometry_iterations,
                },
                GeometryStartId::MaterializedSource,
            );
            let (geometry_mesh, geometry_patch, range, certified) = match outcome_geometry(outcome)
            {
                Some(result) => result,
                None => continue,
            };
            let margin = signed_margin(range);
            if margin > evidence.best_signed_margin_deg {
                evidence.best_signed_margin_deg = margin;
                evidence.best_angle_range_deg = range;
                evidence.best_flips = flips.clone();
            }
            if certified {
                return LocalTopologySearchOutcome::StrictCertified {
                    trial: Box::new(GeometryFailureWitness {
                        mesh: geometry_mesh,
                        patch: geometry_patch,
                    }),
                    evidence,
                };
            }
            let mut used_faces = state.used_faces.clone();
            used_faces.extend([face, neighbour]);
            frontier.push_back(SearchState {
                mesh: candidate,
                flips,
                used_faces,
            });
        }
    }
    LocalTopologySearchOutcome::NoStrictCandidate(evidence)
}

fn local_flip_edges(mesh: &MeshState, seed_faces: &BTreeSet<usize>) -> Vec<(usize, usize, usize)> {
    let mut edges = BTreeMap::<(usize, usize), (usize, usize, usize)>::new();
    for &face in seed_faces {
        if !mesh.is_triangle_live(face) {
            continue;
        }
        for corner in 0..3 {
            let neighbour = mesh.neighbours()[face][corner];
            if !mesh.is_triangle_live(neighbour) {
                continue;
            }
            let key = (face.min(neighbour), face.max(neighbour));
            let (owner, owner_corner) = if face <= neighbour {
                (face, corner)
            } else {
                let Some(owner_corner) = mesh.neighbours()[neighbour]
                    .iter()
                    .position(|&other| other == face)
                else {
                    continue;
                };
                (neighbour, owner_corner)
            };
            edges.entry(key).or_insert((owner, owner_corner, key.1));
        }
    }
    edges.into_values().collect()
}

fn local_patch(
    mesh: &HierarchyLeafMesh,
    incumbent: &ElasticPatch,
    changed_faces: &BTreeSet<usize>,
) -> Option<ElasticPatch> {
    let protected = incumbent
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let movable = changed_faces
        .iter()
        .flat_map(|&face| mesh.mesh.triangles()[face])
        .filter(|site| !protected.contains(site))
        .collect::<BTreeSet<_>>();
    if movable.is_empty() {
        return None;
    }
    let guard_faces = mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            mesh.mesh.triangles()[face]
                .into_iter()
                .any(|site| movable.contains(&site))
        })
        .collect::<Vec<_>>();
    let fixed = guard_faces
        .iter()
        .flat_map(|&face| mesh.mesh.triangles()[face])
        .filter(|site| !movable.contains(site))
        .collect::<BTreeSet<_>>();
    let mut patch = incumbent.clone();
    patch.reference_positions = mesh.mesh.vertices().to_vec();
    patch.movable_compact_vertices = movable.into_iter().collect();
    patch.fixed_compact_vertices = fixed.into_iter().collect();
    patch.guard_faces = guard_faces;
    Some(patch)
}

fn outcome_geometry(
    outcome: ElasticBlockOutcome,
) -> Option<(HierarchyLeafMesh, ElasticPatch, (f64, f64), bool)> {
    match outcome {
        ElasticBlockOutcome::Certified(trial) => Some((
            trial.mesh,
            trial.patch,
            (
                trial.geometry.min_angle_degrees,
                trial.geometry.max_angle_degrees,
            ),
            true,
        )),
        ElasticBlockOutcome::ElasticNoImprovement {
            global_angle_degrees,
            witness,
            ..
        }
        | ElasticBlockOutcome::SearchBudgetExhausted {
            global_angle_degrees,
            witness,
            ..
        }
        | ElasticBlockOutcome::RequiresDifferentTopology {
            global_angle_degrees,
            witness,
            ..
        } => Some((witness.mesh, witness.patch, global_angle_degrees?, false)),
        ElasticBlockOutcome::InvalidPatch { .. } => None,
    }
}

fn topology_gates_pass(mesh: &HierarchyLeafMesh, anchors: &BTreeSet<usize>) -> bool {
    if mesh.mesh.validate().is_err() || mesh.mesh.open_edge_count() != 0 {
        return false;
    }
    let mut degrees = vec![0usize; mesh.mesh.vertices().len()];
    let mut edges = BTreeSet::new();
    for face in mesh.mesh.active_triangle_slots() {
        let triangle = mesh.mesh.triangles()[face];
        for vertex in triangle {
            degrees[vertex] += 1;
        }
        for [a, b] in [
            [triangle[0], triangle[1]],
            [triangle[1], triangle[2]],
            [triangle[2], triangle[0]],
        ] {
            edges.insert((a.min(b), a.max(b)));
        }
    }
    if degrees
        .iter()
        .enumerate()
        .any(|(site, degree)| mesh.mesh.is_vertex_live(site) && !(5..=7).contains(degree))
    {
        return false;
    }
    for (compact, source) in mesh.source_vertex_slots.iter().copied().enumerate() {
        if source.is_some_and(|source| anchors.contains(&source)) && degrees[compact] != 5 {
            return false;
        }
    }
    let vertices = mesh.mesh.vertex_count();
    let faces = mesh.mesh.triangle_count();
    if vertices as isize - edges.len() as isize + faces as isize != 2
        || degrees
            .iter()
            .enumerate()
            .filter(|(site, _)| mesh.mesh.is_vertex_live(*site))
            .map(|(_, degree)| 6isize - *degree as isize)
            .sum::<isize>()
            != 12
    {
        return false;
    }
    vertex_links_are_cycles(&mesh.mesh)
}

fn vertex_links_are_cycles(mesh: &MeshState) -> bool {
    let mut links = BTreeMap::<usize, BTreeMap<usize, BTreeSet<usize>>>::new();
    for face in mesh.active_triangle_slots() {
        let [a, b, c] = mesh.triangles()[face];
        for (site, left, right) in [(a, b, c), (b, c, a), (c, a, b)] {
            links
                .entry(site)
                .or_default()
                .entry(left)
                .or_default()
                .insert(right);
            links
                .entry(site)
                .or_default()
                .entry(right)
                .or_default()
                .insert(left);
        }
    }
    links.values().all(|link| {
        if link.values().any(|neighbours| neighbours.len() != 2) {
            return false;
        }
        let Some(&start) = link.keys().next() else {
            return false;
        };
        let mut visited = BTreeSet::new();
        let mut frontier = vec![start];
        while let Some(vertex) = frontier.pop() {
            if visited.insert(vertex) {
                frontier.extend(link[&vertex].iter().copied());
            }
        }
        visited.len() == link.len()
    })
}

fn canonical_triangles(mesh: &MeshState) -> Vec<[usize; 3]> {
    mesh.active_triangle_slots()
        .map(|face| {
            let mut triangle = mesh.triangles()[face];
            triangle.sort_unstable();
            triangle
        })
        .collect()
}

fn angle_range(mesh: &MeshState) -> Option<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for face in mesh.active_triangle_slots() {
        for angle in
            spherical_triangle_angles(mesh.triangles()[face].map(|site| mesh.vertices()[site]))?
        {
            minimum = minimum.min(angle);
            maximum = maximum.max(angle);
        }
    }
    (minimum.is_finite() && maximum.is_finite()).then_some((minimum, maximum))
}

fn signed_margin((minimum, maximum): (f64, f64)) -> f64 {
    (minimum - 40.2).min(79.8 - maximum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{
        build_violation_support_atlas, n6_legacy_mixed_fixture_with_source_levels,
        solve_full_polygon_merge, FullPolygonMergeLimits, FullPolygonMergeOutcome,
        GeometryDomainId, TransitionTopologyCandidate,
    };

    #[test]
    fn topology_search_rolls_back_and_is_deterministic() {
        let (source, component, _) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
        let stratified = super::super::build_stratified_annulus(&source, &component).unwrap();
        let FullPolygonMergeOutcome::Closed(topology) = solve_full_polygon_merge(
            &source,
            &component,
            FullPolygonMergeLimits {
                topology_states: 500,
            },
        ) else {
            panic!("Frozen N6 full-polygon topology must close")
        };
        let leaf = topology.global_trial.mesh.clone();
        let guard_faces = leaf.mesh.active_triangle_slots().collect::<Vec<_>>();
        let patch = ElasticPatch {
            domain_id: GeometryDomainId::PlusTwoOrdinaryRings,
            topology: TransitionTopologyCandidate {
                component_id: component.id,
                topology_id: 0,
                core_parents: Vec::new(),
                custom_transition_triangles: BTreeMap::new(),
                source_triangles: Vec::new(),
                source_active_vertices: (2..leaf.mesh.vertices().len()).collect(),
                source_degree_forecast: BTreeMap::new(),
            },
            reference_positions: leaf.mesh.vertices().to_vec(),
            fixed_compact_vertices: Vec::new(),
            movable_compact_vertices: (2..leaf.mesh.vertices().len()).collect(),
            guard_faces,
            target_mode: super::super::ElasticTargetMode::TrialReference,
            target_field: Default::default(),
        };
        let incumbent = GeometryFailureWitness { mesh: leaf, patch };
        let atlas = build_violation_support_atlas(
            &source,
            &incumbent.mesh,
            &incumbent.patch,
            &stratified,
            &topology.evidence.selected_topology_keys,
            &[],
        )
        .unwrap();
        let before = incumbent.clone();
        let limits = LocalTopologyLimits {
            maximum_states: 4,
            maximum_flips: 1,
            local_geometry_iterations: 1,
        };
        let anchors = BTreeSet::new();
        let first = search_local_topology_neighbourhood(&incumbent, &atlas, &anchors, limits);
        let second = search_local_topology_neighbourhood(&incumbent, &atlas, &anchors, limits);
        assert_eq!(first, second);
        assert_eq!(incumbent, before);
    }
}
