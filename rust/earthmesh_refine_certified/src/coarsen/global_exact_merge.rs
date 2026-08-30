//! Global exact CAT merge over stratified two-chain sectors.
//!
//! PR37B stays combinatorial: finite two-chain triangulations, PR37A anchor-ear
//! flips, final manifold/valence/Euler gates, then hierarchy materialization.

use super::anchor_ear::derive_anchor_ear_candidates_with_fixed_edges;
use super::core_condensation::rebuild_from_leaf_set_with_custom_triangles;
use super::{
    apply_anchor_ear, build_stratified_annulus, condense_hierarchy_core, HierarchyComponent,
    HierarchyLeafMesh, OwnedTopologyTriangle, RingAnchorKind, StratifiedAnnulus,
};
use crate::mother_grid::{MotherGrid, TriangleAddress};
use std::collections::{BTreeMap, BTreeSet};

const MAX_EARS_PER_ANCHOR: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalExactMergeLimits {
    pub topology_states: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlobalExactMergeEvidence {
    pub states_examined: usize,
    pub sector_variant_counts: Vec<usize>,
    pub selected_ears: Vec<GlobalExactSelectedEar>,
    pub ear_states_examined: usize,
    pub vertex_degrees: BTreeMap<usize, usize>,
    pub anchor_degrees: BTreeMap<usize, usize>,
    pub ordinary_degree_histogram: BTreeMap<usize, usize>,
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
    pub source_vertices: usize,
    pub source_faces: usize,
    pub euler: isize,
    pub charge: isize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlobalExactSelectedEar {
    pub anchor_slot: usize,
    pub sector_id: u64,
    pub removed_neighbour_slot: usize,
    pub inserted_chord: (usize, usize),
    pub owner_sector_ids: BTreeSet<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalExactMergeTrial {
    pub mesh: HierarchyLeafMesh,
    pub custom_triangles: Vec<[usize; 3]>,
    pub evidence: GlobalExactMergeEvidence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalExactMergeOutcome {
    Closed(Box<GlobalExactMergeTrial>),
    NoAnchorValenceSolution(GlobalExactMergeEvidence),
    SearchBudgetExhausted(GlobalExactMergeEvidence),
    InvalidInput {
        reason: String,
        evidence: GlobalExactMergeEvidence,
    },
}

pub fn solve_global_exact_merge(
    source: &MotherGrid,
    component: &HierarchyComponent,
    limits: GlobalExactMergeLimits,
) -> GlobalExactMergeOutcome {
    let mut stratified = match build_stratified_annulus(source, component) {
        Ok(stratified) => stratified,
        Err(error) => {
            return invalid(
                format!("stratified annulus rejected component: {error:?}"),
                evidence(source),
            )
        }
    };
    let mut base_evidence = evidence(source);
    let fixed_final_triangles = match fixed_triangles(source, component) {
        Ok(triangles) => triangles,
        Err(reason) => return invalid(reason, base_evidence),
    };
    replace_fixed_link_contracts(&mut stratified, &fixed_final_triangles);
    let sectors = match sector_variants(&stratified) {
        Ok(sectors) => sectors,
        Err(reason) => return invalid(reason, base_evidence),
    };
    base_evidence.sector_variant_counts = sectors.iter().map(Vec::len).collect();
    if sectors.is_empty() || sectors.iter().any(Vec::is_empty) {
        return invalid(
            "sector triangulation product is empty".into(),
            base_evidence,
        );
    }
    if sectors.len() > 64 || sectors.iter().any(|variants| variants.len() > 64) {
        return invalid(
            "sector exact-search bitmask capacity exceeded".into(),
            base_evidence,
        );
    }
    let anchor_limits = stratified
        .link_contracts
        .iter()
        .filter(|(_, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
        })
        .map(|(&slot, contract)| {
            (
                slot,
                (
                    usize::from(contract.target_degree_min),
                    usize::from(contract.target_degree_max) + MAX_EARS_PER_ANCHOR,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sectors = prepare_variants(sectors, anchor_limits.keys().copied());
    let edge_providers = edge_providers(&sectors);
    let vertex_sectors = vertex_sector_masks(&sectors);
    let ear_touchable = ear_touchable_vertices(
        &fixed_final_triangles,
        &sectors,
        anchor_limits.keys().copied().collect(),
    );
    let fixed_anchor_edges = mesh_edges(&fixed_final_triangles);
    ExactSearch {
        source,
        component,
        stratified: &stratified,
        sectors: &sectors,
        edge_providers: &edge_providers,
        anchor_limits,
        anchor_degrees: fixed_anchor_degrees(&fixed_final_triangles, &stratified),
        vertex_degrees: triangle_incidence_counts(&fixed_final_triangles),
        vertex_sectors,
        ear_touchable,
        fixed_final_triangles: &fixed_final_triangles,
        fixed_anchor_edges: &fixed_anchor_edges,
        limit: limits.topology_states,
        states: 0,
        last: base_evidence,
        selected: vec![None; sectors.len()],
        triangle_keys: fixed_final_triangles
            .iter()
            .copied()
            .map(canonical_vertices)
            .collect(),
        edge_counts: edge_counts(&fixed_final_triangles),
    }
    .run()
}

enum SearchStep {
    Closed(Box<GlobalExactMergeTrial>),
    Invalid(String, Box<GlobalExactMergeEvidence>),
    Exhausted,
    NoSolution,
}

struct ExactSearch<'a> {
    source: &'a MotherGrid,
    component: &'a HierarchyComponent,
    stratified: &'a StratifiedAnnulus,
    sectors: &'a [Vec<PreparedVariant>],
    edge_providers: &'a BTreeMap<(usize, usize), Vec<(usize, usize)>>,
    anchor_limits: BTreeMap<usize, (usize, usize)>,
    anchor_degrees: BTreeMap<usize, usize>,
    vertex_degrees: BTreeMap<usize, usize>,
    vertex_sectors: BTreeMap<usize, u64>,
    ear_touchable: BTreeSet<usize>,
    fixed_final_triangles: &'a [[usize; 3]],
    fixed_anchor_edges: &'a BTreeSet<(usize, usize)>,
    limit: usize,
    states: usize,
    last: GlobalExactMergeEvidence,
    selected: Vec<Option<usize>>,
    triangle_keys: BTreeSet<[usize; 3]>,
    edge_counts: BTreeMap<(usize, usize), usize>,
}

struct PreparedVariant {
    triangles: Vec<OwnedTopologyTriangle>,
    triangle_keys: Vec<[usize; 3]>,
    edge_counts: Vec<((usize, usize), usize)>,
    anchor_counts: Vec<(usize, usize)>,
    vertex_counts: Vec<(usize, usize)>,
}

impl PreparedVariant {
    fn anchor_count(&self, anchor: usize) -> usize {
        self.anchor_counts
            .binary_search_by_key(&anchor, |&(slot, _)| slot)
            .map(|index| self.anchor_counts[index].1)
            .unwrap_or_default()
    }

    fn vertex_count(&self, vertex: usize) -> usize {
        self.vertex_counts
            .binary_search_by_key(&vertex, |&(slot, _)| slot)
            .map(|index| self.vertex_counts[index].1)
            .unwrap_or_default()
    }
}

fn min_max(values: impl Iterator<Item = usize>) -> Option<(usize, usize)> {
    values.fold(None, |bounds, value| {
        Some(match bounds {
            Some((min, max)) => (min.min(value), max.max(value)),
            None => (value, value),
        })
    })
}

fn prepare_variants(
    sectors: Vec<Vec<Vec<OwnedTopologyTriangle>>>,
    anchors: impl Iterator<Item = usize> + Clone,
) -> Vec<Vec<PreparedVariant>> {
    sectors
        .into_iter()
        .map(|variants| {
            variants
                .into_iter()
                .map(|triangles| {
                    let triangle_keys = triangles
                        .iter()
                        .map(|triangle| canonical_vertices(triangle.vertices))
                        .collect();
                    let mut counts = BTreeMap::new();
                    for triangle in &triangles {
                        for edge in triangle_edges(triangle.vertices) {
                            *counts.entry(edge).or_default() += 1;
                        }
                    }
                    let anchor_counts = anchors
                        .clone()
                        .map(|anchor| {
                            (
                                anchor,
                                triangles
                                    .iter()
                                    .filter(|triangle| triangle.vertices.contains(&anchor))
                                    .count(),
                            )
                        })
                        .collect();
                    let mut vertex_counts = BTreeMap::new();
                    for triangle in &triangles {
                        for vertex in triangle.vertices {
                            *vertex_counts.entry(vertex).or_default() += 1;
                        }
                    }
                    PreparedVariant {
                        triangles,
                        triangle_keys,
                        edge_counts: counts.into_iter().collect(),
                        anchor_counts,
                        vertex_counts: vertex_counts.into_iter().collect(),
                    }
                })
                .collect()
        })
        .collect()
}

fn fixed_anchor_degrees(
    triangles: &[[usize; 3]],
    stratified: &StratifiedAnnulus,
) -> BTreeMap<usize, usize> {
    stratified
        .link_contracts
        .iter()
        .filter(|(_, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
        })
        .map(|(&anchor, _)| {
            (
                anchor,
                triangles
                    .iter()
                    .filter(|triangle| triangle.contains(&anchor))
                    .count(),
            )
        })
        .collect()
}

fn triangle_incidence_counts(triangles: &[[usize; 3]]) -> BTreeMap<usize, usize> {
    let mut out = BTreeMap::new();
    for triangle in triangles {
        for &vertex in triangle {
            *out.entry(vertex).or_default() += 1;
        }
    }
    out
}

fn edge_providers(
    sectors: &[Vec<PreparedVariant>],
) -> BTreeMap<(usize, usize), Vec<(usize, usize)>> {
    let mut out = BTreeMap::<_, Vec<_>>::new();
    for (sector, variants) in sectors.iter().enumerate() {
        for (choice, variant) in variants.iter().enumerate() {
            for &(edge, count) in &variant.edge_counts {
                if count == 1 {
                    out.entry(edge).or_default().push((sector, choice));
                }
            }
        }
    }
    out
}

fn vertex_sector_masks(sectors: &[Vec<PreparedVariant>]) -> BTreeMap<usize, u64> {
    let mut out = BTreeMap::new();
    for (sector, variants) in sectors.iter().enumerate() {
        for variant in variants {
            for &(vertex, _) in &variant.vertex_counts {
                *out.entry(vertex).or_default() |= 1 << sector;
            }
        }
    }
    out
}

fn ear_touchable_vertices(
    fixed: &[[usize; 3]],
    sectors: &[Vec<PreparedVariant>],
    anchors: BTreeSet<usize>,
) -> BTreeSet<usize> {
    fixed
        .iter()
        .copied()
        .chain(
            sectors
                .iter()
                .flatten()
                .flat_map(|variant| variant.triangles.iter().map(|triangle| triangle.vertices)),
        )
        .filter(|triangle| triangle.iter().any(|vertex| anchors.contains(vertex)))
        .flatten()
        .filter(|vertex| !anchors.contains(vertex))
        .collect()
}

impl ExactSearch<'_> {
    fn run(mut self) -> GlobalExactMergeOutcome {
        match self.visit() {
            SearchStep::Closed(trial) => GlobalExactMergeOutcome::Closed(trial),
            SearchStep::Invalid(reason, evidence) => invalid(reason, *evidence),
            SearchStep::Exhausted => {
                self.last.states_examined = self.states;
                GlobalExactMergeOutcome::SearchBudgetExhausted(self.last)
            }
            SearchStep::NoSolution => {
                self.last.states_examined = self.states;
                GlobalExactMergeOutcome::NoAnchorValenceSolution(self.last)
            }
        }
    }

    fn visit(&mut self) -> SearchStep {
        let Some(choices) = self.next_choices() else {
            return self.evaluate();
        };
        if choices.is_empty() {
            return SearchStep::NoSolution;
        }
        for (sector, choice) in choices {
            if self.states >= self.limit {
                return SearchStep::Exhausted;
            }
            self.states += 1;
            let variant = &self.sectors[sector][choice];
            self.add_variant(variant);
            self.selected[sector] = Some(choice);
            let result = self.visit();
            self.selected[sector] = None;
            self.remove_variant(variant);
            match result {
                SearchStep::NoSolution => {}
                terminal => return terminal,
            }
        }
        SearchStep::NoSolution
    }

    fn next_choices(&self) -> Option<Vec<(usize, usize)>> {
        let mut compatible = vec![0u64; self.sectors.len()];
        let mut best = None::<Vec<(usize, usize)>>;
        let remaining = self
            .selected
            .iter()
            .filter(|choice| choice.is_none())
            .count();
        for (sector, selected) in self.selected.iter().enumerate() {
            if selected.is_some() {
                continue;
            }
            let choices = self.sectors[sector]
                .iter()
                .enumerate()
                .filter_map(|(choice, variant)| {
                    (self.variant_fits(sector, variant)
                        && self.excess_repairable_after(variant)
                        && (remaining != 1
                            || (self.variant_closes(variant)
                                && self.degrees_repairable_after(variant))))
                    .then_some((sector, choice))
                })
                .collect::<Vec<_>>();
            compatible[sector] = choices
                .iter()
                .fold(0, |mask, &(_, choice)| mask | (1 << choice));
            if best
                .as_ref()
                .is_none_or(|current| choices.len() < current.len())
            {
                best = Some(choices);
            }
        }
        let mut best = best?;
        for (&edge, &count) in &self.edge_counts {
            if count != 1 {
                continue;
            }
            let providers = self
                .edge_providers
                .get(&edge)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&(sector, choice)| compatible[sector] & (1 << choice) != 0)
                .collect::<Vec<_>>();
            if providers.len() < best.len() {
                best = providers;
            }
            if best.is_empty() {
                break;
            }
        }
        if !self.anchor_bounds_hold(&compatible) || !self.degree_bounds_hold(&compatible) {
            return Some(Vec::new());
        }
        best.sort_by_key(|&(sector, choice)| {
            (
                self.excess_after(&self.sectors[sector][choice]),
                self.sectors[sector][choice]
                    .anchor_counts
                    .iter()
                    .map(|(_, count)| count)
                    .sum::<usize>(),
                sector,
                choice,
            )
        });
        Some(best)
    }

    fn anchor_bounds_hold(&self, compatible: &[u64]) -> bool {
        self.anchor_limits.iter().all(|(&anchor, &(min, max))| {
            let mut lower = self.anchor_degrees[&anchor];
            let mut upper = lower;
            for (sector, &mask) in compatible.iter().enumerate() {
                if self.selected[sector].is_some() {
                    continue;
                }
                let counts = self.sectors[sector]
                    .iter()
                    .enumerate()
                    .filter(|(choice, _)| mask & (1 << choice) != 0)
                    .map(|(_, variant)| variant.anchor_count(anchor));
                let Some((sector_min, sector_max)) = min_max(counts) else {
                    return false;
                };
                lower += sector_min;
                upper += sector_max;
            }
            lower <= max && upper >= min
        })
    }

    fn degree_bounds_hold(&self, compatible: &[u64]) -> bool {
        self.vertex_sectors.iter().all(|(&vertex, &owners)| {
            if self.anchor_limits.contains_key(&vertex) || self.ear_touchable.contains(&vertex) {
                return true;
            }
            let mut lower = self
                .vertex_degrees
                .get(&vertex)
                .copied()
                .unwrap_or_default();
            let mut upper = lower;
            for (sector, &mask) in compatible.iter().enumerate() {
                if self.selected[sector].is_some() || owners & (1 << sector) == 0 {
                    continue;
                }
                let counts = self.sectors[sector]
                    .iter()
                    .enumerate()
                    .filter(|(choice, _)| mask & (1 << choice) != 0)
                    .map(|(_, variant)| variant.vertex_count(vertex));
                let Some((sector_min, sector_max)) = min_max(counts) else {
                    return false;
                };
                lower += sector_min;
                upper += sector_max;
            }
            lower <= 7 && upper >= 5
        })
    }

    fn variant_closes(&self, variant: &PreparedVariant) -> bool {
        self.edge_counts.iter().all(|(edge, &count)| {
            count == 2
                || variant
                    .edge_counts
                    .binary_search_by_key(edge, |&(candidate, _)| candidate)
                    .is_ok_and(|index| variant.edge_counts[index].1 == 1)
        }) && variant.edge_counts.iter().all(|&(edge, count)| {
            self.edge_counts.get(&edge).copied().unwrap_or_default() + count == 2
        })
    }

    fn max_ears(&self) -> usize {
        self.anchor_limits
            .values()
            .map(|&(min, max)| max - min)
            .sum()
    }

    fn excess_repairable_after(&self, variant: &PreparedVariant) -> bool {
        self.excess_after(variant) <= self.max_ears()
    }

    fn excess_after(&self, variant: &PreparedVariant) -> usize {
        self.vertex_degrees
            .iter()
            .filter(|(vertex, _)| !self.anchor_limits.contains_key(vertex))
            .map(|(&vertex, &degree)| (degree + variant.vertex_count(vertex)).saturating_sub(7))
            .sum::<usize>()
    }

    fn degrees_repairable_after(&self, variant: &PreparedVariant) -> bool {
        let Some(ears) = self
            .anchor_limits
            .iter()
            .map(|(&anchor, &(target, _))| {
                (self.anchor_degrees[&anchor] + variant.anchor_count(anchor)).checked_sub(target)
            })
            .sum::<Option<usize>>()
        else {
            return false;
        };
        let vertices = self
            .vertex_degrees
            .keys()
            .copied()
            .chain(variant.vertex_counts.iter().map(|&(vertex, _)| vertex))
            .collect::<BTreeSet<_>>();
        let (deficit, excess) = vertices
            .into_iter()
            .filter(|vertex| !self.anchor_limits.contains_key(vertex))
            .fold((0usize, 0usize), |(deficit, excess), vertex| {
                let degree = self
                    .vertex_degrees
                    .get(&vertex)
                    .copied()
                    .unwrap_or_default()
                    + variant.vertex_count(vertex);
                (
                    deficit + 5usize.saturating_sub(degree),
                    excess + degree.saturating_sub(7),
                )
            });
        deficit <= ears * 2 && excess <= ears
    }

    fn variant_fits(&self, sector: usize, variant: &PreparedVariant) -> bool {
        variant
            .triangle_keys
            .iter()
            .all(|triangle| !self.triangle_keys.contains(triangle))
            && variant.edge_counts.iter().all(|&(edge, count)| {
                self.edge_counts.get(&edge).copied().unwrap_or_default() + count <= 2
            })
            && variant.anchor_counts.iter().all(|&(anchor, count)| {
                self.anchor_degrees[&anchor] + count <= self.anchor_limits[&anchor].1
            })
            && self.completed_degrees_hold(sector, variant)
    }

    fn completed_degrees_hold(&self, sector: usize, variant: &PreparedVariant) -> bool {
        let remaining = self
            .selected
            .iter()
            .enumerate()
            .filter(|&(candidate, selected)| candidate != sector && selected.is_none())
            .fold(0u64, |mask, (candidate, _)| mask | (1 << candidate));
        self.vertex_sectors.iter().all(|(&vertex, &owners)| {
            self.anchor_limits.contains_key(&vertex)
                || self.ear_touchable.contains(&vertex)
                || owners & remaining != 0
                || (5..=7).contains(
                    &(self
                        .vertex_degrees
                        .get(&vertex)
                        .copied()
                        .unwrap_or_default()
                        + variant.vertex_count(vertex)),
                )
        })
    }

    fn add_variant(&mut self, variant: &PreparedVariant) {
        self.triangle_keys
            .extend(variant.triangle_keys.iter().copied());
        for &(edge, count) in &variant.edge_counts {
            *self.edge_counts.entry(edge).or_default() += count;
        }
        for &(anchor, count) in &variant.anchor_counts {
            *self.anchor_degrees.get_mut(&anchor).expect("known anchor") += count;
        }
        for &(vertex, count) in &variant.vertex_counts {
            *self.vertex_degrees.entry(vertex).or_default() += count;
        }
    }

    fn remove_variant(&mut self, variant: &PreparedVariant) {
        for triangle in &variant.triangle_keys {
            self.triangle_keys.remove(triangle);
        }
        for &(edge, removed) in &variant.edge_counts {
            let count = self
                .edge_counts
                .get_mut(&edge)
                .expect("selected variant edge must exist");
            *count -= removed;
            if *count == 0 {
                self.edge_counts.remove(&edge);
            }
        }
        for &(anchor, count) in &variant.anchor_counts {
            *self.anchor_degrees.get_mut(&anchor).expect("known anchor") -= count;
        }
        for &(vertex, removed) in &variant.vertex_counts {
            let count = self.vertex_degrees.get_mut(&vertex).expect("known vertex");
            *count -= removed;
            if *count == 0 {
                self.vertex_degrees.remove(&vertex);
            }
        }
    }

    fn evaluate(&mut self) -> SearchStep {
        if self.edge_counts.values().any(|&count| count != 2) {
            return SearchStep::NoSolution;
        }
        let topology_id = self.states as u64;
        let mut mutable = self
            .selected
            .iter()
            .enumerate()
            .flat_map(|(sector, choice)| {
                self.sectors[sector][choice.expect("all sectors selected")]
                    .triangles
                    .clone()
            })
            .collect::<Vec<_>>();
        for triangle in &mut mutable {
            triangle.topology_id = topology_id;
        }
        let mut trial_evidence = self.last.clone();
        trial_evidence.states_examined = self.states;
        let mut ear_states = 0;
        match solve_ears(
            self.source,
            self.stratified,
            self.fixed_anchor_edges,
            self.fixed_final_triangles,
            mutable,
            &mut trial_evidence,
            &mut ear_states,
        ) {
            EarSolve::Solved { triangles, ears } => {
                trial_evidence.selected_ears = ears;
                trial_evidence.ear_states_examined += ear_states;
                let mesh = match materialize(self.source, self.component, &triangles) {
                    Ok(mesh) => mesh,
                    Err(reason) => return SearchStep::Invalid(reason, Box::new(trial_evidence)),
                };
                SearchStep::Closed(Box::new(GlobalExactMergeTrial {
                    mesh,
                    custom_triangles: triangles.into_iter().map(|t| t.vertices).collect(),
                    evidence: trial_evidence,
                }))
            }
            EarSolve::NoSolution => {
                trial_evidence.ear_states_examined += ear_states;
                self.last = trial_evidence;
                SearchStep::NoSolution
            }
            EarSolve::Invalid(reason) => {
                trial_evidence.ear_states_examined += ear_states;
                SearchStep::Invalid(reason, Box::new(trial_evidence))
            }
        }
    }
}

enum EarSolve {
    Solved {
        triangles: Vec<OwnedTopologyTriangle>,
        ears: Vec<GlobalExactSelectedEar>,
    },
    NoSolution,
    Invalid(String),
}

struct EarSearchContext<'a> {
    source: &'a MotherGrid,
    stratified: &'a StratifiedAnnulus,
    fixed_mesh_edges: &'a BTreeSet<(usize, usize)>,
    fixed_final_triangles: &'a [[usize; 3]],
}

type EarSearchKey = (Vec<OwnedTopologyTriangle>, Vec<(usize, usize)>);

fn solve_ears(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    fixed_mesh_edges: &BTreeSet<(usize, usize)>,
    fixed_final_triangles: &[[usize; 3]],
    triangles: Vec<OwnedTopologyTriangle>,
    evidence: &mut GlobalExactMergeEvidence,
    states: &mut usize,
) -> EarSolve {
    let context = EarSearchContext {
        source,
        stratified,
        fixed_mesh_edges,
        fixed_final_triangles,
    };
    solve_ears_inner(
        &context,
        triangles,
        Vec::new(),
        0,
        evidence,
        states,
        &mut BTreeSet::new(),
    )
}

fn solve_ears_inner(
    context: &EarSearchContext<'_>,
    triangles: Vec<OwnedTopologyTriangle>,
    ears: Vec<GlobalExactSelectedEar>,
    depth: usize,
    evidence: &mut GlobalExactMergeEvidence,
    states: &mut usize,
    seen: &mut BTreeSet<EarSearchKey>,
) -> EarSolve {
    if depth
        > context
            .stratified
            .link_contracts
            .len()
            .saturating_mul(MAX_EARS_PER_ANCHOR)
    {
        return EarSolve::Invalid("anchor ear recursion exceeded anchor count".into());
    }
    let mut triangle_key = triangles.clone();
    triangle_key.sort_unstable();
    let ear_counts = ears
        .iter()
        .fold(BTreeMap::new(), |mut counts, ear| {
            *counts.entry(ear.anchor_slot).or_default() += 1;
            counts
        })
        .into_iter()
        .collect();
    if !seen.insert((triangle_key, ear_counts)) {
        return EarSolve::NoSolution;
    }
    let overfull = overfull_anchors(context.stratified, &triangles);
    if overfull.is_empty() {
        let mut final_source_triangles = context.fixed_final_triangles.to_vec();
        final_source_triangles.extend(triangles.iter().map(|triangle| triangle.vertices));
        return if final_gate(
            context.source,
            context.stratified,
            &final_source_triangles,
            evidence,
        )
        .is_ok()
        {
            EarSolve::Solved { triangles, ears }
        } else {
            EarSolve::NoSolution
        };
    }
    let Some(topology_id) = triangles.first().map(|triangle| triangle.topology_id) else {
        return EarSolve::Invalid("global sector topology is empty".into());
    };
    let topology_id = match usize::try_from(topology_id) {
        Ok(topology_id) => topology_id,
        Err(_) => return EarSolve::Invalid("topology id does not fit usize".into()),
    };
    let report = match derive_anchor_ear_candidates_with_fixed_edges(
        context.source,
        context.stratified,
        topology_id,
        &triangles,
        context.fixed_mesh_edges,
    ) {
        Ok(report) => report,
        Err(error) => {
            return EarSolve::Invalid(format!(
                "anchor ear derivation rejected topology {topology_id}: {:?}",
                error.reason
            ))
        }
    };
    let candidates = report
        .candidates
        .into_iter()
        .filter(|candidate| overfull.contains(&candidate.anchor_slot))
        .filter(|candidate| {
            ears.iter()
                .filter(|ear| ear.anchor_slot == candidate.anchor_slot)
                .count()
                < MAX_EARS_PER_ANCHOR
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return EarSolve::NoSolution;
    }
    for candidate in candidates {
        *states += 1;
        let next = match apply_anchor_ear(&triangles, &candidate) {
            Ok(next) => next,
            Err(error) => {
                return EarSolve::Invalid(format!("anchor ear apply rejected: {:?}", error.reason))
            }
        };
        let mut next_ears = ears.clone();
        next_ears.push(GlobalExactSelectedEar {
            anchor_slot: candidate.anchor_slot,
            sector_id: candidate.sector_id,
            removed_neighbour_slot: candidate.removed_neighbour_slot,
            inserted_chord: candidate.inserted_chord,
            owner_sector_ids: candidate.owner_sector_ids.clone(),
        });
        match solve_ears_inner(context, next, next_ears, depth + 1, evidence, states, seen) {
            EarSolve::NoSolution => {}
            terminal => return terminal,
        }
    }
    EarSolve::NoSolution
}

fn overfull_anchors(
    stratified: &StratifiedAnnulus,
    triangles: &[OwnedTopologyTriangle],
) -> BTreeSet<usize> {
    stratified
        .link_contracts
        .iter()
        .filter(|(_, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
        })
        .filter_map(|(&slot, contract)| {
            let degree = anchor_link_edges(slot, contract, triangles).len();
            (degree > usize::from(contract.target_degree_max)).then_some(slot)
        })
        .collect()
}

fn anchor_link_edges(
    anchor_slot: usize,
    contract: &super::VertexLinkContract,
    triangles: &[OwnedTopologyTriangle],
) -> BTreeSet<(usize, usize)> {
    let mut out = contract.fixed_link_edges.clone();
    out.extend(
        triangles
            .iter()
            .filter(|triangle| triangle.vertices.contains(&anchor_slot))
            .map(|triangle| {
                let others = triangle
                    .vertices
                    .into_iter()
                    .filter(|&vertex| vertex != anchor_slot)
                    .collect::<Vec<_>>();
                sorted(others[0], others[1])
            }),
    );
    out
}

fn sector_variants(
    stratified: &StratifiedAnnulus,
) -> Result<Vec<Vec<Vec<OwnedTopologyTriangle>>>, String> {
    let coarse_cycle = stratified
        .coupled
        .coarse_interface
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect::<Vec<_>>();
    stratified
        .probe
        .sector_components
        .iter()
        .enumerate()
        .map(|(sector_id, sector)| {
            let lower_chain = if sector.band_id == 0 {
                contract_chain_to_cycle(&sector.lower_chain, &coarse_cycle)?
            } else {
                sector.lower_chain.clone()
            };
            two_chain_triangulations(sector_id as u64, 0, &lower_chain, &sector.upper_chain)
        })
        .collect()
}

fn contract_chain_to_cycle(chain: &[usize], cycle: &[usize]) -> Result<Vec<usize>, String> {
    let Some(&start) = chain.first() else {
        return Err("sector chain is empty".into());
    };
    let Some(&end) = chain.last() else {
        return Err("sector chain is empty".into());
    };
    let start_index = cycle
        .iter()
        .position(|&vertex| vertex == start)
        .ok_or_else(|| format!("coarse chain start {start} is absent from coarse cycle"))?;
    let end_index = cycle
        .iter()
        .position(|&vertex| vertex == end)
        .ok_or_else(|| format!("coarse chain end {end} is absent from coarse cycle"))?;
    let forward = cycle_path(cycle, start_index, end_index, 1);
    let backward = cycle_path(cycle, start_index, end_index, cycle.len() - 1);
    let mut matches = [forward, backward]
        .into_iter()
        .filter(|path| is_subsequence(path, chain))
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [selected] => Ok(selected.clone()),
        [] => Err(format!(
            "coarse chain {chain:?} has no path on coarse cycle {cycle:?}"
        )),
        _ => Err(format!(
            "coarse chain {chain:?} is ambiguous on coarse cycle {cycle:?}"
        )),
    }
}

fn cycle_path(cycle: &[usize], start: usize, end: usize, step: usize) -> Vec<usize> {
    let mut path = vec![cycle[start]];
    let mut index = start;
    while index != end {
        index = (index + step) % cycle.len();
        path.push(cycle[index]);
    }
    path
}

fn is_subsequence(needle: &[usize], haystack: &[usize]) -> bool {
    let mut next = 0usize;
    for &vertex in haystack {
        if needle.get(next) == Some(&vertex) {
            next += 1;
        }
    }
    next == needle.len()
}

fn two_chain_triangulations(
    sector_id: u64,
    topology_id: u64,
    lower_chain: &[usize],
    upper_chain: &[usize],
) -> Result<Vec<Vec<OwnedTopologyTriangle>>, String> {
    if lower_chain.len() < 2 || upper_chain.len() < 2 {
        return Err("sector chain has fewer than two vertices".into());
    }
    if lower_chain.first() != upper_chain.first() || lower_chain.last() != upper_chain.last() {
        return Err("sector chains do not share endpoints".into());
    }
    let mut polygon = lower_chain.to_vec();
    polygon.extend(upper_chain.iter().rev().skip(1).take(upper_chain.len() - 2));
    if polygon.len() < 3 {
        return Err("sector polygon has fewer than three vertices".into());
    }
    let lower_edges = chain_edges(lower_chain);
    let upper_edges = chain_edges(upper_chain);
    let boundary_edges = polygon_boundary_edges(&polygon);
    let lower_vertices = lower_chain.iter().copied().collect::<BTreeSet<_>>();
    let upper_vertices = upper_chain.iter().copied().collect::<BTreeSet<_>>();
    let mut memo = BTreeMap::new();
    let ctx = TriangulationContext {
        sector_id,
        topology_id,
        polygon: &polygon,
        lower_edges: &lower_edges,
        upper_edges: &upper_edges,
        boundary_edges: &boundary_edges,
        lower_vertices: &lower_vertices,
        upper_vertices: &upper_vertices,
    };
    Ok(triangulate_interval(&ctx, 0, polygon.len() - 1, &mut memo))
}

struct TriangulationContext<'a> {
    sector_id: u64,
    topology_id: u64,
    polygon: &'a [usize],
    lower_edges: &'a BTreeSet<(usize, usize)>,
    upper_edges: &'a BTreeSet<(usize, usize)>,
    boundary_edges: &'a BTreeSet<(usize, usize)>,
    lower_vertices: &'a BTreeSet<usize>,
    upper_vertices: &'a BTreeSet<usize>,
}

fn triangulate_interval(
    ctx: &TriangulationContext<'_>,
    lo: usize,
    hi: usize,
    memo: &mut BTreeMap<(usize, usize), Vec<Vec<OwnedTopologyTriangle>>>,
) -> Vec<Vec<OwnedTopologyTriangle>> {
    if hi <= lo + 1 {
        return vec![Vec::new()];
    }
    if let Some(cached) = memo.get(&(lo, hi)) {
        return cached.clone();
    }
    let mut out = Vec::new();
    for mid in lo + 1..hi {
        let vertices = [ctx.polygon[lo], ctx.polygon[mid], ctx.polygon[hi]];
        if !distinct(vertices) || !triangle_edges_allowed(vertices, ctx) {
            continue;
        }
        let left_choices = triangulate_interval(ctx, lo, mid, memo);
        let right_choices = triangulate_interval(ctx, mid, hi, memo);
        for left in left_choices {
            for right in &right_choices {
                let mut candidate = left.clone();
                candidate.push(OwnedTopologyTriangle {
                    topology_id: ctx.topology_id,
                    sector_id: ctx.sector_id,
                    vertices,
                });
                candidate.extend(right.iter().copied());
                candidate.sort_by_key(|triangle| canonical_triangle(*triangle));
                out.push(candidate);
            }
        }
    }
    out.sort_by_key(|topology| canonical_topology(topology));
    out.dedup_by_key(|topology| canonical_topology(topology));
    memo.insert((lo, hi), out.clone());
    out
}

fn triangle_edges_allowed(vertices: [usize; 3], ctx: &TriangulationContext<'_>) -> bool {
    [
        sorted(vertices[0], vertices[1]),
        sorted(vertices[1], vertices[2]),
        sorted(vertices[2], vertices[0]),
    ]
    .into_iter()
    .all(|edge| {
        ctx.boundary_edges.contains(&edge)
            || ctx.lower_edges.contains(&edge)
            || ctx.upper_edges.contains(&edge)
            || is_cross_chain_edge(edge, ctx.lower_vertices, ctx.upper_vertices)
    })
}

fn fixed_triangles(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<Vec<[usize; 3]>, String> {
    let transition = component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for face in source.mesh.active_triangle_slots() {
        let address = source.triangle_addresses[face]
            .ok_or_else(|| format!("active face {face} has no hierarchy address"))?;
        if !is_descendant_of_any(address, &transition) {
            out.push(source.mesh.triangles()[face]);
        }
    }
    for parent in &component.core_parents {
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid core parent {parent:?}"))?;
        out.retain(|triangle| {
            !children
                .iter()
                .any(|child| source_triangle_matches(source, *child, *triangle))
        });
        out.push(parent_corners(source, *parent)?);
    }
    Ok(out)
}

fn materialize(
    source: &MotherGrid,
    component: &HierarchyComponent,
    custom_triangles: &[OwnedTopologyTriangle],
) -> Result<HierarchyLeafMesh, String> {
    let trial = condense_hierarchy_core(source, &component.core_parents)?;
    let mut leaf_set = trial.leaf_set;
    for parent in &component.transition_parents {
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid transition parent {parent:?}"))?;
        for child in children {
            leaf_set.leaves.remove(&child);
        }
    }
    let custom_parents = component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let custom = custom_triangles
        .iter()
        .map(|triangle| triangle.vertices)
        .collect::<Vec<_>>();
    rebuild_from_leaf_set_with_custom_triangles(source, &leaf_set, &custom_parents, &custom)
}

fn replace_fixed_link_contracts(
    stratified: &mut StratifiedAnnulus,
    fixed_triangles: &[[usize; 3]],
) {
    for (&slot, contract) in &mut stratified.link_contracts {
        contract.fixed_link_edges = final_link_edges(slot, fixed_triangles);
        contract.fixed_link_nodes = contract
            .fixed_link_edges
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .collect();
    }
}

fn final_gate(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    triangles: &[[usize; 3]],
    evidence: &mut GlobalExactMergeEvidence,
) -> Result<(), String> {
    if triangles
        .iter()
        .copied()
        .any(|triangle| !distinct(triangle))
    {
        return Err("degenerate final triangle".into());
    }
    if has_duplicate_triangles(triangles) {
        return Err("duplicate final triangle".into());
    }
    let edge_counts = edge_counts(triangles);
    let degrees = vertex_degrees(triangles);
    let anchors = stratified
        .link_contracts
        .iter()
        .filter(|(_, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
        })
        .map(|(&slot, contract)| (slot, contract))
        .collect::<BTreeMap<_, _>>();
    let vertices = degrees.len();
    let edges = edge_counts.len();
    let faces = triangles.len();
    let euler = vertices as isize - edges as isize + faces as isize;
    let charge = degrees
        .values()
        .map(|degree| 6 - *degree as isize)
        .sum::<isize>();
    evidence.vertex_degrees = degrees.clone();
    evidence.anchor_degrees = anchors
        .keys()
        .filter_map(|slot| degrees.get(slot).map(|degree| (*slot, *degree)))
        .collect();
    evidence.ordinary_degree_histogram.clear();
    for (vertex, degree) in &degrees {
        if !anchors.contains_key(vertex) {
            *evidence
                .ordinary_degree_histogram
                .entry(*degree)
                .or_default() += 1;
        }
    }
    evidence.vertices = vertices;
    evidence.edges = edges;
    evidence.faces = faces;
    evidence.euler = euler;
    evidence.charge = charge;

    if edge_counts.values().any(|&count| count != 2) {
        return Err("final topology has non-closed or non-manifold edges".into());
    }
    for (&slot, contract) in &anchors {
        let link = final_link_edges(slot, triangles);
        if !contract.fixed_link_edges.is_subset(&link) {
            return Err(format!("anchor {slot} lost fixed link edges"));
        }
        let degree = link.len();
        if degree < usize::from(contract.target_degree_min)
            || degree > usize::from(contract.target_degree_max)
        {
            return Err(format!(
                "anchor {slot} degree {degree} violates target contract"
            ));
        }
    }
    for vertex in active_vertices(triangles) {
        if !single_cycle_link(vertex, triangles) {
            return Err(format!("vertex {vertex} link is not one cycle"));
        }
    }
    for (&vertex, &degree) in &degrees {
        if !anchors.contains_key(&vertex) && !(5..=7).contains(&degree) {
            return Err(format!(
                "ordinary vertex {vertex} degree {degree} is outside 5..7"
            ));
        }
    }
    if euler != 2 || charge != 12 {
        return Err(format!(
            "final Euler/charge failed: euler={euler}, charge={charge}"
        ));
    }
    if faces >= source.mesh.triangle_count() || vertices >= source.mesh.vertex_count() {
        return Err("final topology did not reduce faces and vertices".into());
    }
    Ok(())
}

fn evidence(source: &MotherGrid) -> GlobalExactMergeEvidence {
    GlobalExactMergeEvidence {
        source_vertices: source.mesh.vertex_count(),
        source_faces: source.mesh.triangle_count(),
        ..GlobalExactMergeEvidence::default()
    }
}

fn invalid(reason: String, evidence: GlobalExactMergeEvidence) -> GlobalExactMergeOutcome {
    GlobalExactMergeOutcome::InvalidInput { reason, evidence }
}

fn parent_corners(source: &MotherGrid, parent: TriangleAddress) -> Result<[usize; 3], String> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for child in parent
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?
    {
        let slot = source_face_slot(source, child)?;
        for vertex in source.mesh.triangles()[slot] {
            *counts.entry(vertex).or_default() += 1;
        }
    }
    let corners = counts
        .into_iter()
        .filter_map(|(vertex, count)| (count == 1).then_some(vertex))
        .collect::<Vec<_>>();
    if corners.len() != 3 {
        return Err(format!(
            "parent {parent:?} has {} source corners",
            corners.len()
        ));
    }
    Ok([corners[0], corners[1], corners[2]])
}

fn source_face_slot(source: &MotherGrid, address: TriangleAddress) -> Result<usize, String> {
    let dense = address.dense_index(source.subdivision)? + 2;
    if source.triangle_addresses.get(dense).and_then(|x| *x) != Some(address) {
        return Err(format!("source face for {address:?} is missing"));
    }
    Ok(dense)
}

fn source_triangle_matches(
    source: &MotherGrid,
    address: TriangleAddress,
    mut triangle: [usize; 3],
) -> bool {
    source_face_slot(source, address).is_ok_and(|slot| {
        let mut source_triangle = source.mesh.triangles()[slot];
        source_triangle.sort_unstable();
        triangle.sort_unstable();
        source_triangle == triangle
    })
}

fn is_descendant_of_any(address: TriangleAddress, parents: &BTreeSet<TriangleAddress>) -> bool {
    parents.iter().any(|parent| {
        parent
            .children_2_to_1()
            .is_some_and(|children| children.contains(&address))
    })
}

fn final_link_edges(anchor_slot: usize, triangles: &[[usize; 3]]) -> BTreeSet<(usize, usize)> {
    triangles
        .iter()
        .filter(|triangle| triangle.contains(&anchor_slot))
        .map(|triangle| {
            let others = triangle
                .iter()
                .copied()
                .filter(|&vertex| vertex != anchor_slot)
                .collect::<Vec<_>>();
            sorted(others[0], others[1])
        })
        .collect()
}

fn edge_counts(triangles: &[[usize; 3]]) -> BTreeMap<(usize, usize), usize> {
    let mut out = BTreeMap::new();
    for [a, b, c] in triangles.iter().copied() {
        for edge in [sorted(a, b), sorted(b, c), sorted(c, a)] {
            *out.entry(edge).or_default() += 1;
        }
    }
    out
}

fn triangle_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [sorted(a, b), sorted(b, c), sorted(c, a)]
}

fn mesh_edges(triangles: &[[usize; 3]]) -> BTreeSet<(usize, usize)> {
    edge_counts(triangles).into_keys().collect()
}

fn vertex_degrees(triangles: &[[usize; 3]]) -> BTreeMap<usize, usize> {
    let mut links = BTreeMap::<usize, BTreeSet<(usize, usize)>>::new();
    for [a, b, c] in triangles.iter().copied() {
        links.entry(a).or_default().insert(sorted(b, c));
        links.entry(b).or_default().insert(sorted(a, c));
        links.entry(c).or_default().insert(sorted(a, b));
    }
    links
        .into_iter()
        .map(|(vertex, link)| (vertex, link.len()))
        .collect()
}

fn single_cycle_link(vertex: usize, triangles: &[[usize; 3]]) -> bool {
    let edges = triangles
        .iter()
        .filter(|triangle| triangle.contains(&vertex))
        .filter_map(|triangle| {
            let others = triangle
                .iter()
                .copied()
                .filter(|&candidate| candidate != vertex)
                .collect::<Vec<_>>();
            (others.len() == 2).then_some(sorted(others[0], others[1]))
        })
        .collect::<BTreeSet<_>>();
    single_cycle_edges(&edges)
}

fn single_cycle_edges(edges: &BTreeSet<(usize, usize)>) -> bool {
    let mut degrees = BTreeMap::<usize, usize>::new();
    for &(a, b) in edges {
        *degrees.entry(a).or_default() += 1;
        *degrees.entry(b).or_default() += 1;
    }
    if degrees.is_empty() || degrees.values().any(|&degree| degree != 2) {
        return false;
    }
    let start = *degrees.keys().next().expect("non-empty");
    let mut seen = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        for &(a, b) in edges {
            let next = if a == node {
                b
            } else if b == node {
                a
            } else {
                continue;
            };
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    seen.len() == degrees.len()
}

fn active_vertices(triangles: &[[usize; 3]]) -> BTreeSet<usize> {
    triangles.iter().flatten().copied().collect()
}

fn has_duplicate_triangles(triangles: &[[usize; 3]]) -> bool {
    let mut seen = BTreeSet::new();
    triangles.iter().copied().any(|mut triangle| {
        triangle.sort_unstable();
        !seen.insert(triangle)
    })
}

fn chain_edges(chain: &[usize]) -> BTreeSet<(usize, usize)> {
    chain
        .windows(2)
        .map(|edge| sorted(edge[0], edge[1]))
        .collect()
}

fn polygon_boundary_edges(polygon: &[usize]) -> BTreeSet<(usize, usize)> {
    (0..polygon.len())
        .map(|i| sorted(polygon[i], polygon[(i + 1) % polygon.len()]))
        .collect()
}

fn is_cross_chain_edge(
    (a, b): (usize, usize),
    lower_vertices: &BTreeSet<usize>,
    upper_vertices: &BTreeSet<usize>,
) -> bool {
    (lower_vertices.contains(&a) && upper_vertices.contains(&b))
        || (upper_vertices.contains(&a) && lower_vertices.contains(&b))
}

fn canonical_topology(topology: &[OwnedTopologyTriangle]) -> Vec<[usize; 3]> {
    let mut triangles = topology
        .iter()
        .copied()
        .map(canonical_triangle)
        .collect::<Vec<_>>();
    triangles.sort_unstable();
    triangles
}

fn canonical_triangle(triangle: OwnedTopologyTriangle) -> [usize; 3] {
    canonical_vertices(triangle.vertices)
}

fn canonical_vertices(mut vertices: [usize; 3]) -> [usize; 3] {
    vertices.sort_unstable();
    vertices
}

fn distinct([a, b, c]: [usize; 3]) -> bool {
    a != b && a != c && b != c
}

fn sorted(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::n6_legacy_mixed_fixture;

    #[test]
    fn n6_contracted_sector_family_is_stable() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let stratified = build_stratified_annulus(&source, &component).unwrap();
        let variants = sector_variants(&stratified).unwrap();
        assert_eq!(
            variants.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![3, 3, 20, 20, 3, 3, 62, 62, 9, 9, 9, 9, 62, 62]
        );
        assert_eq!(
            variants.iter().map(Vec::len).product::<usize>(),
            3_141_100_312_070_400
        );
    }

    #[test]
    fn n6_budget_exhaustion_stays_unknown() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let outcome = solve_global_exact_merge(
            &source,
            &component,
            GlobalExactMergeLimits { topology_states: 1 },
        );
        let GlobalExactMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
            panic!("one search state must not prove closure or infeasibility");
        };
        assert_eq!(evidence.states_examined, 1);
        assert_eq!(evidence.ear_states_examined, 0);
    }

    #[test]
    #[ignore = "manual exact PR37B proof; about one minute in release mode"]
    fn n6_anchor_ear_family_is_exhaustively_infeasible() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let outcome = solve_global_exact_merge(
            &source,
            &component,
            GlobalExactMergeLimits {
                topology_states: 1_000_000,
            },
        );
        let GlobalExactMergeOutcome::NoAnchorValenceSolution(evidence) = outcome else {
            panic!("frozen N6 generic ear family must be exhausted without closure");
        };
        assert_eq!(evidence.states_examined, 425_879);
        assert!(evidence.ear_states_examined > 0);
        assert_eq!(
            evidence.anchor_degrees,
            BTreeMap::from([(2, 5), (29, 5), (77, 5), (155, 5)])
        );
        assert_eq!((evidence.euler, evidence.charge), (2, 12));
        assert!(evidence
            .ordinary_degree_histogram
            .keys()
            .any(|degree| !(5..=7).contains(degree)));
    }
}
