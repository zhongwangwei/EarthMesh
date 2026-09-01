//! Find-one global merge over V3 transition-cell topology families.

use super::global_exact_merge::{
    fixed_triangles_for_face_complex, materialize_for_face_complex, mesh_edges,
    replace_fixed_link_contract_map, solve_ears_with_contracts, EarSolve,
};
use super::{
    AnnularTopology, AnnularTopologyKey, DiskCellDomain, FullAnnularFamily,
    GlobalExactMergeEvidence, GlobalExactMergeTrial, HierarchyComponent, OwnedTopologyTriangle,
    StratifiedTransitionDomainV3, TransitionCellDomain,
};
use crate::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionCellProblem {
    Disk(DiskCellDomain),
    Annulus(super::AnnularCellDomain),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularTransitionCellFamily {
    pub cell_id: u64,
    pub family: FullAnnularFamily,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionCellFamily {
    Disk {
        cell_id: u64,
        topologies: Vec<TransitionCellTopology>,
    },
    Annulus(AnnularTransitionCellFamily),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransitionCellTopologyKey {
    pub cell_id: u64,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionCellTopology {
    pub cell_id: u64,
    pub triangles: Vec<OwnedTopologyTriangle>,
    pub vertex_incidences: BTreeMap<usize, u8>,
    pub vertex_link_edges: BTreeMap<usize, BTreeSet<Edge>>,
    pub topology_key: TransitionCellTopologyKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionCellMergeLimits {
    pub topology_states: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransitionCellMergeEvidence {
    pub cell_family_counts: Vec<usize>,
    pub states_examined: usize,
    pub ear_states_examined: usize,
    pub topology_candidates_closed: usize,
    pub selected_topology_keys: Vec<TransitionCellTopologyKey>,
    pub selected_annular_keys: Vec<AnnularTopologyKey>,
    pub global: GlobalExactMergeEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionCellMergeTrial {
    pub global_trial: GlobalExactMergeTrial,
    pub evidence: TransitionCellMergeEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransitionCellMergeOutcome {
    Closed(Box<TransitionCellMergeTrial>),
    TopologyFamilyExhaustedNoSolution(TransitionCellMergeEvidence),
    SearchIncomplete(TransitionCellMergeEvidence),
    InvalidInput {
        reason: String,
        evidence: TransitionCellMergeEvidence,
    },
}

pub fn transition_cell_topology_from_annular(
    cell_id: u64,
    topology: &AnnularTopology,
) -> Result<TransitionCellTopology, String> {
    let triangles = topology
        .triangles
        .iter()
        .copied()
        .map(|vertices| OwnedTopologyTriangle {
            topology_id: 0,
            sector_id: cell_id,
            vertices,
        })
        .collect::<Vec<_>>();
    transition_cell_topology(cell_id, triangles)
}

pub fn solve_transition_cell_find_one(
    source: &MotherGrid,
    component: &HierarchyComponent,
    domain: &StratifiedTransitionDomainV3,
    families: &[TransitionCellFamily],
    limits: TransitionCellMergeLimits,
) -> TransitionCellMergeOutcome {
    let mut evidence = TransitionCellMergeEvidence {
        global: GlobalExactMergeEvidence {
            source_vertices: source.mesh.vertex_count(),
            source_faces: source.mesh.triangle_count(),
            ..GlobalExactMergeEvidence::default()
        },
        ..TransitionCellMergeEvidence::default()
    };
    let expected_ids = domain.cells.iter().map(cell_id).collect::<Vec<_>>();
    let actual_ids = families.iter().map(family_cell_id).collect::<Vec<_>>();
    if expected_ids != actual_ids {
        return invalid(
            format!("transition-cell family ids {actual_ids:?} do not match {expected_ids:?}"),
            evidence,
        );
    }
    let concrete = match concrete_families(families) {
        Ok(families) => families,
        Err(reason) => return invalid(reason, evidence),
    };
    evidence.cell_family_counts = concrete.iter().map(Vec::len).collect();
    evidence.global.sector_variant_counts = evidence.cell_family_counts.clone();
    if concrete.iter().any(Vec::is_empty) {
        return TransitionCellMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence);
    }
    let annulus_faces = domain
        .topology_domain
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let fixed = match fixed_triangles_for_face_complex(source, component, &annulus_faces) {
        Ok(fixed) => fixed,
        Err(reason) => return invalid(reason, evidence),
    };
    let fixed_edges = mesh_edges(&fixed);
    let mut contracts = domain.link_contracts.clone();
    replace_fixed_link_contract_map(&mut contracts, &fixed);
    let mut indices = vec![0usize; concrete.len()];
    loop {
        if evidence.states_examined >= limits.topology_states {
            return TransitionCellMergeOutcome::SearchIncomplete(evidence);
        }
        evidence.states_examined += 1;
        let topology_id = evidence.states_examined as u64;
        let selected = concrete
            .iter()
            .zip(&indices)
            .map(|(family, &index)| &family[index])
            .collect::<Vec<_>>();
        let triangles = selected
            .iter()
            .flat_map(|topology| {
                topology.triangles.iter().copied().map(|mut triangle| {
                    triangle.topology_id = topology_id;
                    triangle
                })
            })
            .collect::<Vec<_>>();
        let mut global = evidence.global.clone();
        global.states_examined = evidence.states_examined;
        let mut ear_states = 0;
        match solve_ears_with_contracts(
            source,
            &contracts,
            &fixed_edges,
            &fixed,
            triangles,
            &mut global,
            &mut ear_states,
        ) {
            EarSolve::Solved { triangles, ears } => {
                evidence.topology_candidates_closed += 1;
                evidence.ear_states_examined += ear_states;
                global.selected_ears = ears;
                global.ear_states_examined += ear_states;
                evidence.selected_topology_keys = selected
                    .iter()
                    .map(|topology| topology.topology_key.clone())
                    .collect();
                evidence.selected_annular_keys = families
                    .iter()
                    .zip(&indices)
                    .filter_map(|(family, &index)| match family {
                        TransitionCellFamily::Annulus(family) => {
                            Some(family.family.topologies[index].topology_key.clone())
                        }
                        TransitionCellFamily::Disk { .. } => None,
                    })
                    .collect();
                evidence.global = global.clone();
                let mesh = match materialize_for_face_complex(
                    source,
                    component,
                    &annulus_faces,
                    &triangles,
                ) {
                    Ok(mesh) => mesh,
                    Err(reason) => return invalid(reason, evidence),
                };
                return TransitionCellMergeOutcome::Closed(Box::new(TransitionCellMergeTrial {
                    global_trial: GlobalExactMergeTrial {
                        mesh,
                        custom_triangles: triangles
                            .iter()
                            .map(|triangle| triangle.vertices)
                            .collect(),
                        evidence: global,
                    },
                    evidence,
                }));
            }
            EarSolve::NoSolution => {
                evidence.ear_states_examined += ear_states;
                if better_global_evidence(&global, &evidence.global) {
                    evidence.global = global;
                }
            }
            EarSolve::Invalid(reason) => return invalid(reason, evidence),
        }
        if !increment_product(&mut indices, &concrete) {
            return TransitionCellMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence);
        }
    }
}

fn concrete_families(
    families: &[TransitionCellFamily],
) -> Result<Vec<Vec<TransitionCellTopology>>, String> {
    families
        .iter()
        .map(|family| match family {
            TransitionCellFamily::Disk { topologies, .. } => Ok(topologies.clone()),
            TransitionCellFamily::Annulus(family) => family
                .family
                .topologies
                .iter()
                .map(|topology| transition_cell_topology_from_annular(family.cell_id, topology))
                .collect(),
        })
        .collect()
}

fn transition_cell_topology(
    cell_id: u64,
    mut triangles: Vec<OwnedTopologyTriangle>,
) -> Result<TransitionCellTopology, String> {
    triangles.iter_mut().for_each(|triangle| {
        triangle.vertices.sort_unstable();
        triangle.sector_id = cell_id;
    });
    triangles.sort_unstable();
    if triangles
        .windows(2)
        .any(|pair| pair[0].vertices == pair[1].vertices)
    {
        return Err(format!("cell {cell_id} has duplicate triangles"));
    }
    let mut incidences = BTreeMap::<usize, u8>::new();
    let mut links = BTreeMap::<usize, BTreeSet<Edge>>::new();
    for triangle in &triangles {
        for corner in 0..3 {
            let vertex = triangle.vertices[corner];
            let Some(next) = incidences.entry(vertex).or_default().checked_add(1) else {
                return Err(format!("cell {cell_id} incidence exceeds u8"));
            };
            incidences.insert(vertex, next);
            links.entry(vertex).or_default().insert(edge(
                triangle.vertices[(corner + 1) % 3],
                triangle.vertices[(corner + 2) % 3],
            ));
        }
    }
    let topology_key = TransitionCellTopologyKey {
        cell_id,
        triangles: triangles.iter().map(|triangle| triangle.vertices).collect(),
    };
    Ok(TransitionCellTopology {
        cell_id,
        triangles,
        vertex_incidences: incidences,
        vertex_link_edges: links,
        topology_key,
    })
}

fn increment_product(indices: &mut [usize], families: &[Vec<TransitionCellTopology>]) -> bool {
    for index in (0..indices.len()).rev() {
        indices[index] += 1;
        if indices[index] < families[index].len() {
            return true;
        }
        indices[index] = 0;
    }
    false
}

fn better_global_evidence(
    candidate: &GlobalExactMergeEvidence,
    current: &GlobalExactMergeEvidence,
) -> bool {
    let score = |evidence: &GlobalExactMergeEvidence| {
        (
            evidence.euler.abs_diff(2) + evidence.charge.abs_diff(12),
            evidence
                .vertex_degrees
                .values()
                .map(|degree| 5usize.saturating_sub(*degree) + degree.saturating_sub(7))
                .sum::<usize>(),
        )
    };
    !candidate.vertex_degrees.is_empty()
        && (current.vertex_degrees.is_empty() || score(candidate) < score(current))
}

fn family_cell_id(family: &TransitionCellFamily) -> u64 {
    match family {
        TransitionCellFamily::Disk { cell_id, .. } => *cell_id,
        TransitionCellFamily::Annulus(family) => family.cell_id,
    }
}

fn cell_id(cell: &TransitionCellDomain) -> u64 {
    match cell {
        TransitionCellDomain::Disk(cell) => cell.cell_id,
        TransitionCellDomain::Annulus(cell) => cell.cell_id,
    }
}

fn invalid(reason: String, evidence: TransitionCellMergeEvidence) -> TransitionCellMergeOutcome {
    TransitionCellMergeOutcome::InvalidInput { reason, evidence }
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
