//! Exact 2:1 coarse/fine interface closure over hierarchy addresses.
//!
//! Core parent edges stay coarse. Only adjacent fine parent patches are
//! retriangulated, using their source slots and a finite non-crossing polygon
//! state space; geometry relocation belongs to the later elastic stage.

use super::{HierarchyComponent, HierarchyLeafMesh, HierarchyLeafSet};
use crate::mother_grid::{MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::{orientation_on_sphere, MeshState, Sign};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionTopologyLimits {
    pub topology_states: usize,
    pub maximum_halo_expansions: usize,
}

impl TransitionTopologyLimits {
    pub fn solve_from_cursor(
        self,
        source: &MotherGrid,
        component: &HierarchyComponent,
        topology_states_cursor: usize,
    ) -> TransitionTopologyOutcome {
        solve_transition_topology_from_cursor(source, component, self, topology_states_cursor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransitionBoundary {
    pub fine_outer_cycles: Vec<Vec<usize>>,
    pub coarse_inner_cycles: Vec<Vec<usize>>,
    pub halo_parents: Vec<TriangleAddress>,
    pub seam: Vec<usize>,
    pub pentagon: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionTopologyCandidate {
    pub component_id: u64,
    pub topology_id: usize,
    pub core_parents: Vec<TriangleAddress>,
    pub custom_transition_triangles: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    pub source_triangles: Vec<[usize; 3]>,
    pub source_active_vertices: Vec<usize>,
    pub source_degree_forecast: BTreeMap<usize, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransitionTopologyReport {
    pub component_id: u64,
    pub core_parent_count: usize,
    pub transition_parent_count: usize,
    pub halo_expansions: usize,
    pub topology_states: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransitionTopologyTrial {
    pub mesh: HierarchyLeafMesh,
    pub boundary: TransitionBoundary,
    pub candidate: TransitionTopologyCandidate,
    pub report: TransitionTopologyReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransitionTopologyOutcome {
    Closed(Box<TransitionTopologyTrial>),
    RequiresWiderHalo {
        states_examined: usize,
        halo_expansions: usize,
    },
    ProvenInfeasible {
        states_examined: usize,
        halo_expansions: usize,
        reason: String,
    },
    SearchBudgetExhausted {
        states_examined: usize,
        halo_expansions: usize,
    },
    InvalidBoundary {
        states_examined: usize,
        halo_expansions: usize,
        reason: String,
    },
}

#[derive(Debug, Clone)]
struct ParentPatch {
    corners: [usize; 3],
    midpoints: [usize; 3],
    neighbours: [TriangleAddress; 3],
    child_triangles: [[usize; 3]; 4],
}

pub fn solve_transition_topology(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: TransitionTopologyLimits,
) -> TransitionTopologyOutcome {
    solve_transition_topology_from_cursor(source, component, limits, 0)
}

pub fn solve_transition_topology_from_cursor(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: TransitionTopologyLimits,
    topology_states_cursor: usize,
) -> TransitionTopologyOutcome {
    let mut core = set(component.core_parents.iter().copied());
    let mut transition = set(component.transition_parents.iter().copied());
    if let Err(reason) = preflight(source, component, &core, &transition) {
        return TransitionTopologyOutcome::InvalidBoundary {
            states_examined: 0,
            halo_expansions: 0,
            reason,
        };
    }
    let mut halo_expansions = 0usize;
    let mut states_examined = 0usize;

    loop {
        if core.is_empty() {
            return TransitionTopologyOutcome::ProvenInfeasible {
                states_examined,
                halo_expansions,
                reason: "halo expansion leaves no coarse core".into(),
            };
        }

        let uncovered = core
            .iter()
            .copied()
            .filter(|&parent| {
                parent_patch(source, parent).is_ok_and(|patch| {
                    patch.neighbours.iter().any(|neighbour| {
                        !core.contains(neighbour) && !transition.contains(neighbour)
                    })
                })
            })
            .collect::<BTreeSet<_>>();
        if uncovered.is_empty() && transition.is_empty() {
            return pure_core(source, component.id, &core, halo_expansions);
        }
        if !uncovered.is_empty() {
            if uncovered.len() == core.len() {
                return TransitionTopologyOutcome::RequiresWiderHalo {
                    states_examined,
                    halo_expansions,
                };
            }
            if halo_expansions == limits.maximum_halo_expansions {
                return TransitionTopologyOutcome::RequiresWiderHalo {
                    states_examined,
                    halo_expansions,
                };
            }
            promote_to_transition(&mut core, &mut transition, uncovered);
            halo_expansions += 1;
            continue;
        }

        if topology_states_cursor >= limits.topology_states {
            return TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined: limits.topology_states,
                halo_expansions,
            };
        }
        if states_examined == limits.topology_states {
            return TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined,
                halo_expansions,
            };
        }
        let local_cursor = topology_states_cursor.saturating_sub(states_examined);
        let local_limit = limits.topology_states - states_examined;
        match solve_once(
            source,
            component.id,
            core.clone(),
            transition.clone(),
            halo_expansions,
            local_cursor,
            local_limit,
        ) {
            TransitionTopologyOutcome::Closed(mut trial) => {
                trial.candidate.topology_id += states_examined;
                states_examined += trial.report.topology_states;
                trial.report.topology_states = states_examined;
                trial.report.halo_expansions = halo_expansions;
                return TransitionTopologyOutcome::Closed(trial);
            }
            TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined: local,
                ..
            } => {
                return TransitionTopologyOutcome::SearchBudgetExhausted {
                    states_examined: states_examined + local,
                    halo_expansions,
                };
            }
            TransitionTopologyOutcome::InvalidBoundary { reason, .. } => {
                return invalid(states_examined, halo_expansions, reason);
            }
            TransitionTopologyOutcome::ProvenInfeasible {
                states_examined: local,
                reason,
                ..
            } => {
                states_examined += local;
                if states_examined == limits.topology_states {
                    return TransitionTopologyOutcome::SearchBudgetExhausted {
                        states_examined,
                        halo_expansions,
                    };
                }
                let peel = core_boundary(source, &core);
                if peel.is_empty() || peel.len() == core.len() {
                    return TransitionTopologyOutcome::ProvenInfeasible {
                        states_examined,
                        halo_expansions,
                        reason,
                    };
                }
                if halo_expansions == limits.maximum_halo_expansions {
                    return TransitionTopologyOutcome::RequiresWiderHalo {
                        states_examined,
                        halo_expansions,
                    };
                }
                promote_to_transition(&mut core, &mut transition, peel);
                halo_expansions += 1;
            }
            TransitionTopologyOutcome::RequiresWiderHalo { .. } => unreachable!(),
        }
    }
}

fn preflight(
    source: &MotherGrid,
    component: &HierarchyComponent,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<(), String> {
    if source.subdivision < 2 || !source.subdivision.is_multiple_of(2) {
        return Err("transition topology requires an even source subdivision >= 2".into());
    }
    let expected_n = source.subdivision / 2;
    let parents = set(component.parents.iter().copied());
    if parents.is_empty() {
        return Err("transition component has no parents".into());
    }
    if parents.len() != component.parents.len()
        || core.len() != component.core_parents.len()
        || transition.len() != component.transition_parents.len()
    {
        return Err("transition component contains duplicate parents".into());
    }
    if !core.is_disjoint(transition) {
        return Err("component core and transition parents overlap".into());
    }
    let union = core.union(transition).copied().collect::<BTreeSet<_>>();
    if union != parents {
        return Err("component parents must equal core union transition parents".into());
    }
    for &parent in &parents {
        if parent.n != expected_n {
            return Err(format!(
                "component parent {:?} is not at expected coarse subdivision {expected_n}",
                parent
            ));
        }
        parent_patch(source, parent)?;
    }
    let seed = *parents.first().expect("non-empty component");
    let mut seen = BTreeSet::from([seed]);
    let mut stack = vec![seed];
    while let Some(parent) = stack.pop() {
        for neighbour in parent_patch(source, parent)?.neighbours {
            if parents.contains(&neighbour) && seen.insert(neighbour) {
                stack.push(neighbour);
            }
        }
    }
    if seen != parents {
        return Err("transition component parents are disconnected".into());
    }
    Ok(())
}

fn promote_to_transition(
    core: &mut BTreeSet<TriangleAddress>,
    transition: &mut BTreeSet<TriangleAddress>,
    promoted: BTreeSet<TriangleAddress>,
) {
    for parent in promoted {
        core.remove(&parent);
        transition.insert(parent);
    }
}

fn core_boundary(
    source: &MotherGrid,
    core: &BTreeSet<TriangleAddress>,
) -> BTreeSet<TriangleAddress> {
    core.iter()
        .copied()
        .filter(|&parent| {
            parent_patch(source, parent)
                .is_ok_and(|patch| patch.neighbours.iter().any(|p| !core.contains(p)))
        })
        .collect()
}

fn pure_core(
    source: &MotherGrid,
    component_id: u64,
    core: &BTreeSet<TriangleAddress>,
    halo_expansions: usize,
) -> TransitionTopologyOutcome {
    let mut leaf_set = match HierarchyLeafSet::from_mother_grid(source) {
        Ok(v) => v,
        Err(reason) => return invalid(0, halo_expansions, reason),
    };
    if let Err(reason) = leaf_set.condense_core(&core.iter().copied().collect::<Vec<_>>()) {
        return invalid(0, halo_expansions, reason);
    }
    let mesh = match super::core_condensation::rebuild_from_leaf_set(source, &leaf_set) {
        Ok(mesh) => mesh,
        Err(reason) => return invalid(0, halo_expansions, reason),
    };
    if let Err(reason) = hard_gate(&mesh.mesh) {
        return TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: 0,
            halo_expansions,
            reason,
        };
    }
    let boundary = match boundary(source, core, &BTreeSet::new()) {
        Ok(boundary) => boundary,
        Err(reason) => return invalid(0, halo_expansions, reason),
    };
    TransitionTopologyOutcome::Closed(Box::new(TransitionTopologyTrial {
        mesh,
        boundary,
        candidate: TransitionTopologyCandidate {
            component_id,
            topology_id: 0,
            core_parents: core.iter().copied().collect(),
            custom_transition_triangles: BTreeMap::new(),
            source_triangles: Vec::new(),
            source_active_vertices: Vec::new(),
            source_degree_forecast: BTreeMap::new(),
        },
        report: TransitionTopologyReport {
            component_id,
            core_parent_count: core.len(),
            transition_parent_count: 0,
            halo_expansions,
            topology_states: 0,
        },
    }))
}

fn solve_once(
    source: &MotherGrid,
    component_id: u64,
    core: BTreeSet<TriangleAddress>,
    transition: BTreeSet<TriangleAddress>,
    halo_expansions: usize,
    start_index: usize,
    budget: usize,
) -> TransitionTopologyOutcome {
    let mut states = 0usize;
    let mut leaf_set = match HierarchyLeafSet::from_mother_grid(source) {
        Ok(v) => v,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    if let Err(reason) = leaf_set.condense_core(&core.iter().copied().collect::<Vec<_>>()) {
        return invalid(states, halo_expansions, reason);
    }
    let mut patches = BTreeMap::<TriangleAddress, ParentPatch>::new();
    for &parent in core.iter().chain(&transition) {
        let patch = match parent_patch(source, parent) {
            Ok(patch) => patch,
            Err(reason) => return invalid(states, halo_expansions, reason),
        };
        patches.insert(parent, patch);
    }
    let custom_transition = transition
        .iter()
        .copied()
        .filter(|parent| {
            patches[parent]
                .neighbours
                .iter()
                .any(|neighbour| core.contains(neighbour))
        })
        .collect::<BTreeSet<_>>();

    for parent in &custom_transition {
        let Some(children) = parent.children_2_to_1() else {
            return invalid(
                states,
                halo_expansions,
                format!("invalid transition parent {parent:?}"),
            );
        };
        for child in children {
            leaf_set.leaves.remove(&child);
        }
    }

    let mut variants = Vec::<Vec<Vec<[usize; 3]>>>::new();
    for parent in &custom_transition {
        let patch = &patches[parent];
        let polygon = transition_polygon(patch, &core);
        if !(3..=5).contains(&polygon.len()) {
            return invalid(
                states,
                halo_expansions,
                format!(
                    "transition parent {:?} produced {} boundary vertices",
                    parent,
                    polygon.len()
                ),
            );
        }
        let variants_for_parent = ranked_triangulations(source, &polygon, &patch.child_triangles);
        if variants_for_parent.is_empty() {
            return TransitionTopologyOutcome::ProvenInfeasible {
                states_examined: states,
                halo_expansions,
                reason: format!("transition parent {parent:?} has no positive topology candidate"),
            };
        }
        variants.push(variants_for_parent);
    }

    let total_candidates = variants
        .iter()
        .try_fold(1usize, |total, parent| total.checked_mul(parent.len()));
    if mixed_radix_indices(start_index, &variants).is_none() {
        return TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: total_candidates.expect("finite product when cursor is outside it"),
            halo_expansions,
            reason: "no transition triangulation passed hard topology gates".into(),
        };
    }
    let boundary = match boundary(source, &core, &transition) {
        Ok(boundary) => boundary,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    let forecast = match base_degree_forecast(source, &core, &custom_transition, &patches) {
        Ok(forecast) => forecast,
        Err(reason) => return invalid(states, halo_expansions, reason),
    };
    let mut closed = None;
    ProductSearch {
        source,
        leaf_set: &leaf_set,
        transition: &custom_transition,
        variants: &variants,
        start_index,
        budget,
        states: &mut states,
        forecast: &forecast,
        closed: &mut closed,
    }
    .run();
    let Some(hit) = closed else {
        if total_candidates.is_none_or(|total| states < total) {
            return TransitionTopologyOutcome::SearchBudgetExhausted {
                states_examined: states,
                halo_expansions,
            };
        }
        return TransitionTopologyOutcome::ProvenInfeasible {
            states_examined: states,
            halo_expansions,
            reason: "no transition triangulation passed hard topology gates".into(),
        };
    };

    let candidate_triangles = hit.triangles.clone();
    let active_vertices = hit
        .triangles
        .iter()
        .flat_map(|tri| tri.iter().copied())
        .collect::<BTreeSet<_>>();
    TransitionTopologyOutcome::Closed(Box::new(TransitionTopologyTrial {
        mesh: hit.mesh,
        boundary,
        candidate: TransitionTopologyCandidate {
            component_id,
            topology_id: hit.topology_id,
            core_parents: core.iter().copied().collect(),
            custom_transition_triangles: hit.triangles_by_parent,
            source_triangles: candidate_triangles,
            source_active_vertices: active_vertices.into_iter().collect(),
            source_degree_forecast: hit.degree_forecast,
        },
        report: TransitionTopologyReport {
            component_id,
            core_parent_count: core.len(),
            transition_parent_count: transition.len(),
            halo_expansions,
            topology_states: states,
        },
    }))
}

struct ProductSearch<'a> {
    source: &'a MotherGrid,
    leaf_set: &'a HierarchyLeafSet,
    transition: &'a BTreeSet<TriangleAddress>,
    variants: &'a [Vec<Vec<[usize; 3]>>],
    start_index: usize,
    budget: usize,
    states: &'a mut usize,
    forecast: &'a BTreeMap<usize, isize>,
    closed: &'a mut Option<SearchHit>,
}

struct SearchHit {
    mesh: HierarchyLeafMesh,
    triangles_by_parent: BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
    triangles: Vec<[usize; 3]>,
    degree_forecast: BTreeMap<usize, usize>,
    topology_id: usize,
}

impl ProductSearch<'_> {
    fn run(&mut self) {
        *self.states = self.start_index;
        if *self.states >= self.budget {
            return;
        }

        let Some(mut indices) = mixed_radix_indices(self.start_index, self.variants) else {
            return;
        };
        loop {
            let mut chosen_by_parent = BTreeMap::<TriangleAddress, Vec<[usize; 3]>>::new();
            let mut forecast = self.forecast.clone();
            for ((parent, parent_variants), variant_index) in self
                .transition
                .iter()
                .zip(self.variants)
                .zip(indices.iter().copied())
            {
                let variant = parent_variants[variant_index].clone();
                adjust_triangles(&mut forecast, &variant, 1);
                chosen_by_parent.insert(*parent, variant);
            }
            let chosen = flatten_custom_triangles(&chosen_by_parent);

            *self.states += 1;
            if !forecast
                .values()
                .any(|&degree| degree != 0 && !(5..=7).contains(&degree))
            {
                if let Ok(mesh) =
                    super::core_condensation::rebuild_from_leaf_set_with_custom_triangles(
                        self.source,
                        self.leaf_set,
                        self.transition,
                        &chosen,
                    )
                {
                    if let Ok(()) = hard_gate(&mesh.mesh) {
                        let degree_forecast = forecast
                            .iter()
                            .filter_map(|(&site, &degree)| {
                                usize::try_from(degree).ok().map(|degree| (site, degree))
                            })
                            .collect();
                        *self.closed = Some(SearchHit {
                            mesh,
                            triangles_by_parent: chosen_by_parent,
                            triangles: chosen,
                            degree_forecast,
                            topology_id: *self.states - 1,
                        });
                        return;
                    }
                }
            }

            if *self.states >= self.budget || !advance_mixed_radix(&mut indices, self.variants) {
                return;
            }
        }
    }
}

fn flatten_custom_triangles(
    triangles_by_parent: &BTreeMap<TriangleAddress, Vec<[usize; 3]>>,
) -> Vec<[usize; 3]> {
    triangles_by_parent
        .values()
        .flat_map(|triangles| triangles.iter().copied())
        .collect()
}

fn mixed_radix_indices(
    mut ordinal: usize,
    variants: &[Vec<Vec<[usize; 3]>>],
) -> Option<Vec<usize>> {
    let mut indices = vec![0usize; variants.len()];
    for position in (0..variants.len()).rev() {
        let radix = variants[position].len();
        if radix == 0 {
            return None;
        }
        indices[position] = ordinal % radix;
        ordinal /= radix;
    }
    (ordinal == 0).then_some(indices)
}

fn advance_mixed_radix(indices: &mut [usize], variants: &[Vec<Vec<[usize; 3]>>]) -> bool {
    for position in (0..indices.len()).rev() {
        indices[position] += 1;
        if indices[position] < variants[position].len() {
            return true;
        }
        indices[position] = 0;
    }
    false
}

fn invalid(states: usize, halo_expansions: usize, reason: String) -> TransitionTopologyOutcome {
    TransitionTopologyOutcome::InvalidBoundary {
        states_examined: states,
        halo_expansions,
        reason,
    }
}

fn set(values: impl Iterator<Item = TriangleAddress>) -> BTreeSet<TriangleAddress> {
    values.collect()
}

fn source_face_slot(source: &MotherGrid, address: TriangleAddress) -> Result<usize, String> {
    super::core_condensation::source_face_slot(source, address)
}

fn parent_patch(source: &MotherGrid, parent: TriangleAddress) -> Result<ParentPatch, String> {
    let children = parent
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?;
    let mut child_triangles = [[0usize; 3]; 4];
    for (index, child) in children.into_iter().enumerate() {
        child_triangles[index] = source.mesh.triangles()[source_face_slot(source, child)?];
    }
    let corners = match parent.orientation {
        crate::mother_grid::TriangleOrientation::Up => [
            child_triangles[0][0],
            child_triangles[1][1],
            child_triangles[2][2],
        ],
        crate::mother_grid::TriangleOrientation::Down => [
            child_triangles[0][0],
            child_triangles[2][1],
            child_triangles[1][2],
        ],
    };
    let edge_set = child_triangles
        .iter()
        .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
        .map(|(a, b)| edge(a, b))
        .collect::<BTreeSet<_>>();
    let sites = child_triangles
        .iter()
        .flat_map(|t| t.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut midpoints = [0usize; 3];
    for side in 0..3 {
        let a = corners[side];
        let b = corners[(side + 1) % 3];
        midpoints[side] = sites
            .iter()
            .copied()
            .find(|&m| {
                m != a && m != b && edge_set.contains(&edge(a, m)) && edge_set.contains(&edge(m, b))
            })
            .ok_or_else(|| format!("parent {parent:?} side {side} has no exact midpoint"))?;
    }
    let mut neighbours = [parent; 3];
    for side in 0..3 {
        neighbours[side] = neighbour_parent(
            source,
            parent,
            corners[side],
            midpoints[side],
            corners[(side + 1) % 3],
        )?;
    }
    Ok(ParentPatch {
        corners,
        midpoints,
        neighbours,
        child_triangles,
    })
}

fn neighbour_parent(
    source: &MotherGrid,
    parent: TriangleAddress,
    a: usize,
    midpoint: usize,
    b: usize,
) -> Result<TriangleAddress, String> {
    let mut neighbours = BTreeSet::new();
    for target in [edge(a, midpoint), edge(midpoint, b)] {
        let mut found = None;
        for child in parent.children_2_to_1().unwrap() {
            let slot = source_face_slot(source, child)?;
            let tri = source.mesh.triangles()[slot];
            for side in 0..3 {
                if edge(tri[side], tri[(side + 1) % 3]) != target {
                    continue;
                }
                if found.is_some() {
                    return Err(format!(
                        "parent {parent:?} boundary segment {target:?} has multiple child claims"
                    ));
                }
                let neighbour = source.mesh.neighbours()[slot][(side + 2) % 3];
                if neighbour == 0 {
                    return Err(format!(
                        "parent {parent:?} boundary segment {target:?} is open"
                    ));
                }
                found =
                    source.triangle_addresses[neighbour].and_then(TriangleAddress::parent_2_to_1);
            }
        }
        neighbours.insert(found.ok_or_else(|| {
            format!("parent {parent:?} boundary segment {target:?} has no neighbour")
        })?);
    }
    if neighbours.len() != 1 {
        return Err(format!(
            "parent {parent:?} coarse side has inconsistent fine neighbours"
        ));
    }
    let neighbour = *neighbours.first().expect("one exact neighbour");
    if neighbour == parent {
        return Err(format!(
            "parent {parent:?} names itself across a coarse side"
        ));
    }
    Ok(neighbour)
}

fn transition_polygon(patch: &ParentPatch, core: &BTreeSet<TriangleAddress>) -> Vec<usize> {
    let mut polygon = Vec::with_capacity(6);
    for side in 0..3 {
        polygon.push(patch.corners[side]);
        if !core.contains(&patch.neighbours[side]) {
            polygon.push(patch.midpoints[side]);
        }
    }
    polygon.dedup();
    if polygon.first() == polygon.last() {
        polygon.pop();
    }
    polygon
}

fn triangulations(polygon: &[usize]) -> Vec<Vec<[usize; 3]>> {
    if polygon.len() == 3 {
        return vec![vec![[polygon[0], polygon[1], polygon[2]]]];
    }
    let mut out = Vec::new();
    for split in 1..polygon.len() - 1 {
        let tri = [polygon[0], polygon[split], polygon[polygon.len() - 1]];
        for left in triangulations_or_empty(&polygon[..=split]) {
            for right in triangulations_or_empty(&polygon[split..]) {
                let mut candidate = vec![tri];
                candidate.extend(left.iter().copied());
                candidate.extend(right.iter().copied());
                out.push(candidate);
            }
        }
    }
    out
}

fn triangulations_or_empty(polygon: &[usize]) -> Vec<Vec<[usize; 3]>> {
    if polygon.len() < 3 {
        vec![Vec::new()]
    } else {
        triangulations(polygon)
    }
}

fn ranked_triangulations(
    source: &MotherGrid,
    polygon: &[usize],
    original: &[[usize; 3]; 4],
) -> Vec<Vec<[usize; 3]>> {
    // Maximum face reuse puts the known one-site-retirement signatures first.
    // The remaining Catalan candidates are exactly the finite diagonal/flip
    // alternatives for this convex hierarchy-parent polygon.
    let original = original
        .iter()
        .copied()
        .map(canonical_triangle)
        .collect::<BTreeSet<_>>();
    let mut variants = triangulations(polygon)
        .into_iter()
        .filter(|candidate| {
            candidate.iter().all(|triangle| {
                orientation_on_sphere(
                    source.mesh.vertices()[triangle[0]],
                    source.mesh.vertices()[triangle[1]],
                    source.mesh.vertices()[triangle[2]],
                ) == Ok(Sign::Positive)
            })
        })
        .collect::<Vec<_>>();
    variants.sort_by(|left, right| {
        let left_reuse = left
            .iter()
            .filter(|triangle| original.contains(&canonical_triangle(**triangle)))
            .count();
        let right_reuse = right
            .iter()
            .filter(|triangle| original.contains(&canonical_triangle(**triangle)))
            .count();
        right_reuse
            .cmp(&left_reuse)
            .then_with(|| canonical_candidate(left).cmp(&canonical_candidate(right)))
    });
    variants.dedup_by(|left, right| canonical_candidate(left) == canonical_candidate(right));
    variants
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn canonical_candidate(triangles: &[[usize; 3]]) -> Vec<[usize; 3]> {
    let mut canonical = triangles
        .iter()
        .copied()
        .map(canonical_triangle)
        .collect::<Vec<_>>();
    canonical.sort_unstable();
    canonical
}

fn base_degree_forecast(
    source: &MotherGrid,
    core: &BTreeSet<TriangleAddress>,
    custom_transition: &BTreeSet<TriangleAddress>,
    patches: &BTreeMap<TriangleAddress, ParentPatch>,
) -> Result<BTreeMap<usize, isize>, String> {
    let mut forecast = BTreeMap::new();
    for parent in core {
        let patch = &patches[parent];
        adjust_source_triangles(source, &mut forecast, &patch.child_triangles, -1)?;
        adjust_source_triangles(source, &mut forecast, &[patch.corners], 1)?;
    }
    for parent in custom_transition {
        adjust_source_triangles(source, &mut forecast, &patches[parent].child_triangles, -1)?;
    }
    Ok(forecast)
}

fn adjust_source_triangles(
    source: &MotherGrid,
    forecast: &mut BTreeMap<usize, isize>,
    triangles: &[[usize; 3]],
    delta: isize,
) -> Result<(), String> {
    for vertex in triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
    {
        let degree = forecast
            .entry(vertex)
            .or_insert(source_degree(source, vertex)?);
        *degree += delta;
    }
    Ok(())
}

fn adjust_triangles(forecast: &mut BTreeMap<usize, isize>, triangles: &[[usize; 3]], delta: isize) {
    for vertex in triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
    {
        *forecast
            .get_mut(&vertex)
            .expect("transition candidate only uses removed source-patch vertices") += delta;
    }
}

fn source_degree(source: &MotherGrid, vertex: usize) -> Result<isize, String> {
    match source.addresses.get(vertex).and_then(Option::as_ref) {
        Some(VertexAddress::IcosahedronVertex(_)) => Ok(5),
        Some(_) => Ok(6),
        None => Err(format!("source vertex {vertex} has no hierarchy address")),
    }
}

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn boundary(
    source: &MotherGrid,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<TransitionBoundary, String> {
    let mut coarse_edges = Vec::new();
    for &parent in core {
        let patch = parent_patch(source, parent)?;
        for side in 0..3 {
            if transition.contains(&patch.neighbours[side]) {
                coarse_edges.push((patch.corners[(side + 1) % 3], patch.corners[side]));
            }
        }
    }
    let mut fine_edges = Vec::new();
    for &parent in transition {
        let patch = parent_patch(source, parent)?;
        for side in 0..3 {
            if !core.contains(&patch.neighbours[side])
                && !transition.contains(&patch.neighbours[side])
            {
                fine_edges.extend([
                    (patch.corners[side], patch.midpoints[side]),
                    (patch.midpoints[side], patch.corners[(side + 1) % 3]),
                ]);
            }
        }
    }
    let coarse = cycles_from_edges(coarse_edges)?;
    let fine = cycles_from_edges(fine_edges)?;
    let boundary_sites = coarse
        .iter()
        .chain(&fine)
        .flat_map(|cycle| cycle.iter().copied())
        .collect::<BTreeSet<_>>();
    let seam = boundary_sites
        .iter()
        .copied()
        .filter(|&site| {
            matches!(
                source.addresses.get(site).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronEdge { .. } | VertexAddress::IcosahedronVertex(_))
            )
        })
        .collect();
    let pentagon = boundary_sites
        .into_iter()
        .filter(|&site| {
            matches!(
                source.addresses.get(site).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronVertex(_))
            )
        })
        .collect();
    Ok(TransitionBoundary {
        fine_outer_cycles: fine,
        coarse_inner_cycles: coarse,
        halo_parents: transition.iter().copied().collect(),
        seam,
        pentagon,
    })
}

fn cycles_from_edges(edges: Vec<(usize, usize)>) -> Result<Vec<Vec<usize>>, String> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let mut next = BTreeMap::<usize, usize>::new();
    let mut incoming = BTreeMap::<usize, usize>::new();
    for (a, b) in edges {
        if next.insert(a, b).is_some() {
            return Err(format!("boundary vertex {a} has multiple outgoing edges"));
        }
        *incoming.entry(b).or_default() += 1;
    }
    if let Some((&vertex, &degree)) = incoming.iter().find(|(_, degree)| **degree != 1) {
        return Err(format!(
            "boundary vertex {vertex} has incoming degree {degree}, expected 1"
        ));
    }
    if next.keys().copied().collect::<BTreeSet<_>>()
        != incoming.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err("boundary directed edges do not form closed cycles".into());
    }
    let mut cycles = Vec::new();
    while let Some(&start) = next.keys().next() {
        let mut cycle = Vec::new();
        let mut current = start;
        loop {
            cycle.push(current);
            let following = next
                .remove(&current)
                .ok_or_else(|| "boundary cycle ended before returning to its start".to_string())?;
            current = following;
            if current == start {
                break;
            }
            if cycle.contains(&current) {
                return Err("boundary cycle repeats a vertex before closing".into());
            }
        }
        cycles.push(cycle);
    }
    cycles.sort();
    Ok(cycles)
}

fn hard_gate(mesh: &MeshState) -> Result<(), String> {
    mesh.validate().map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    if mesh.open_edge_count() != 0 {
        return Err(format!("mesh has {} open edges", mesh.open_edge_count()));
    }
    let mut edges = BTreeSet::new();
    let mut degrees = vec![0usize; mesh.vertices().len()];
    let mut seeds = vec![0usize; mesh.vertices().len()];
    let mut triangles = BTreeSet::new();
    for face in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[face];
        if orientation_on_sphere(
            mesh.vertices()[tri[0]],
            mesh.vertices()[tri[1]],
            mesh.vertices()[tri[2]],
        )
        .map_err(|e| e.to_string())?
            != Sign::Positive
        {
            return Err(format!("triangle {face} is not positively oriented"));
        }
        let mut canonical = tri;
        canonical.sort_unstable();
        if !triangles.insert(canonical) {
            return Err(format!("duplicate triangle {canonical:?}"));
        }
        for [u, v] in [[tri[0], tri[1]], [tri[1], tri[2]], [tri[2], tri[0]]] {
            edges.insert(edge(u, v));
        }
        for vertex in tri {
            degrees[vertex] += 1;
            if seeds[vertex] == 0 {
                seeds[vertex] = face;
            }
        }
    }
    let euler =
        mesh.vertex_count() as isize - edges.len() as isize + mesh.triangle_count() as isize;
    if euler != 2 {
        return Err(format!("Euler characteristic is {euler}, expected 2"));
    }
    if let Some((vertex, degree)) = degrees
        .iter()
        .copied()
        .enumerate()
        .find(|&(_, degree)| degree != 0 && !(5..=7).contains(&degree))
    {
        return Err(format!("vertex {vertex} degree {degree} outside 5..=7"));
    }
    for vertex in mesh.active_vertex_slots() {
        let fan = mesh
            .triangle_fan_from(vertex, seeds[vertex])
            .map_err(|error| error.to_string())?;
        if fan.len() != degrees[vertex] {
            return Err(format!(
                "vertex {vertex} has {} incident faces but a {}-face connected fan",
                degrees[vertex],
                fan.len()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_polygon_enumeration_has_the_catalan_counts() {
        assert_eq!(triangulations(&[1, 2, 3]).len(), 1);
        assert_eq!(triangulations(&[1, 2, 3, 4]).len(), 2);
        assert_eq!(triangulations(&[1, 2, 3, 4, 5]).len(), 5);
    }

    #[test]
    fn boundary_walk_rejects_an_open_chain() {
        assert!(cycles_from_edges(vec![(1, 2), (2, 3)]).is_err());
        assert_eq!(
            cycles_from_edges(vec![(1, 2), (2, 3), (3, 1)]).unwrap(),
            vec![vec![1, 2, 3]]
        );
    }

    #[test]
    fn mixed_radix_walks_the_product_with_last_slot_fastest() {
        let variants = vec![
            vec![vec![[1, 2, 3]], vec![[1, 3, 4]]],
            vec![vec![[5, 6, 7]], vec![[5, 7, 8]], vec![[5, 8, 9]]],
        ];
        let mut indices = vec![0, 0];
        let mut visited = vec![indices.clone()];
        while advance_mixed_radix(&mut indices, &variants) {
            visited.push(indices.clone());
        }
        assert_eq!(
            visited,
            vec![
                vec![0, 0],
                vec![0, 1],
                vec![0, 2],
                vec![1, 0],
                vec![1, 1],
                vec![1, 2],
            ]
        );
    }
}
