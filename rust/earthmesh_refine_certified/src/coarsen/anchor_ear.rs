//! Anchor-ear repair model for shared-sector CAT topology.
//!
//! PR37A is combinatorial only: it derives exact anchor-ear candidates from an
//! existing mutable triangle set and fixed vertex-link contracts. It does not
//! call legacy topology search or CBER.

use super::{RingAnchorKind, StratifiedAnnulus, VertexLinkContract};
use crate::mother_grid::MotherGrid;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OwnedTopologyTriangle {
    pub topology_id: u64,
    pub sector_id: u64,
    pub vertices: [usize; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnchorEarKey {
    pub anchor_slot: usize,
    pub sector_id: u64,
    pub inserted_chord: (usize, usize),
    pub removed_neighbour_slot: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnchorEarRejectReason {
    AnchorBelowTarget,
    AnchorNotOverfull,
    RemovedTrianglesNotMutable,
    RemovedTrianglesNotEar,
    DegenerateAddedTriangle,
    DuplicateTriangle,
    ChordAlreadyExists,
    LinkEdgeDuplicate,
    LinkNotSingleCycle,
    InvalidEarContract,
    RadialEdgeNotMutable,
    TopologyIdMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorEarApplyError {
    pub reason: AnchorEarRejectReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorEarCandidate {
    pub topology_key: AnchorEarKey,
    pub sector_topology_id: usize,
    pub anchor_slot: usize,
    pub sector_id: u64,
    pub removed_triangles: [[usize; 3]; 2],
    pub inserted_triangles: [[usize; 3]; 2],
    pub removed_link_edges: [(usize, usize); 2],
    pub removed_radial_edge: (usize, usize),
    pub inserted_chord: (usize, usize),
    pub predecessor_slot: usize,
    pub removed_neighbour_slot: usize,
    pub successor_slot: usize,
    pub owner_sector_ids: BTreeSet<u64>,
    pub anchor_initial_link_edges: BTreeSet<(usize, usize)>,
    pub anchor_result_link_edges: BTreeSet<(usize, usize)>,
    pub degree_delta: [(usize, i8); 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnchorEarConflictGraph {
    pub candidate_keys: Vec<AnchorEarKey>,
    pub conflicts: BTreeSet<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnchorEarReport {
    pub sector_topology_id: usize,
    pub mutable_triangle_count: usize,
    pub generated_by_generic_link_discovery: bool,
    pub candidates: Vec<AnchorEarCandidate>,
    pub candidate_count: usize,
    pub initial_anchor_degrees: BTreeMap<usize, usize>,
    pub anchor_slots_examined: Vec<usize>,
    pub rejections: BTreeMap<usize, BTreeMap<AnchorEarRejectReason, usize>>,
    pub conflict_graph: AnchorEarConflictGraph,
}

pub fn derive_anchor_ear_candidates(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    sector_topology_id: usize,
    mutable_triangles: &[OwnedTopologyTriangle],
) -> Result<AnchorEarReport, AnchorEarApplyError> {
    let fixed_mesh_edges =
        fixed_outside_mesh_edges(source, &stratified.coupled.fixed_outside_face_slots);
    derive_anchor_ear_candidates_with_fixed_edges(
        source,
        stratified,
        sector_topology_id,
        mutable_triangles,
        &fixed_mesh_edges,
    )
}

pub(super) fn derive_anchor_ear_candidates_with_fixed_edges(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    sector_topology_id: usize,
    mutable_triangles: &[OwnedTopologyTriangle],
    fixed_mesh_edges: &BTreeSet<(usize, usize)>,
) -> Result<AnchorEarReport, AnchorEarApplyError> {
    let mutable = canonical_owned_triangles(mutable_triangles);
    let expected_topology_id = sector_topology_id as u64;
    if mutable
        .iter()
        .any(|triangle| triangle.topology_id != expected_topology_id)
    {
        return reject(AnchorEarRejectReason::TopologyIdMismatch);
    }
    let mut report = AnchorEarReport {
        sector_topology_id,
        mutable_triangle_count: mutable.len(),
        generated_by_generic_link_discovery: true,
        ..AnchorEarReport::default()
    };
    let mut anchors = stratified
        .link_contracts
        .iter()
        .filter_map(|(&slot, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
            .then_some((slot, contract))
        })
        .collect::<Vec<_>>();
    anchors.sort_by_key(|&(slot, _)| slot);
    for (anchor_slot, contract) in anchors {
        report.anchor_slots_examined.push(anchor_slot);
        report.initial_anchor_degrees.insert(
            anchor_slot,
            anchor_link_edges(anchor_slot, contract, &mutable).len(),
        );
        report.candidates.extend(anchor_candidates(
            source,
            anchor_slot,
            contract,
            sector_topology_id,
            &mutable,
            fixed_mesh_edges,
            &mut report.rejections,
        ));
    }
    report
        .candidates
        .sort_by(|a, b| a.topology_key.cmp(&b.topology_key));
    report
        .candidates
        .dedup_by(|a, b| a.topology_key == b.topology_key);
    report.candidate_count = report.candidates.len();
    report.conflict_graph = build_anchor_ear_conflict_graph(report.candidates.clone());
    Ok(report)
}

pub fn apply_anchor_ear(
    mutable_triangles: &[OwnedTopologyTriangle],
    candidate: &AnchorEarCandidate,
) -> Result<Vec<OwnedTopologyTriangle>, AnchorEarApplyError> {
    if !valid_anchor_ear_contract(candidate) {
        return reject(AnchorEarRejectReason::InvalidEarContract);
    }
    let mutable = canonical_owned_triangles(mutable_triangles);
    if mutable
        .iter()
        .any(|triangle| triangle.topology_id != candidate.sector_topology_id as u64)
    {
        return reject(AnchorEarRejectReason::TopologyIdMismatch);
    }
    let removed = candidate
        .removed_triangles
        .iter()
        .copied()
        .map(canonical_vertices)
        .collect::<BTreeSet<_>>();
    if removed.len() != 2 {
        return reject(AnchorEarRejectReason::RemovedTrianglesNotEar);
    }
    if !removed_radial_edge_is_mutable(candidate.removed_radial_edge, &removed) {
        return reject(AnchorEarRejectReason::RadialEdgeNotMutable);
    }
    let mut removed_count = 0usize;
    let mut removed_owners = BTreeSet::new();
    let mut out = Vec::with_capacity(mutable.len());
    for triangle in mutable {
        if removed.contains(&triangle.vertices) {
            removed_count += 1;
            removed_owners.insert(triangle.sector_id);
        } else {
            out.push(triangle);
        }
    }
    if removed_count != 2 || removed_owners != candidate.owner_sector_ids {
        return reject(AnchorEarRejectReason::RemovedTrianglesNotMutable);
    }
    if mesh_edges(&out).contains(&candidate.inserted_chord) {
        return reject(AnchorEarRejectReason::ChordAlreadyExists);
    }
    let radial_incident = mutable_triangles
        .iter()
        .copied()
        .map(canonical_owned)
        .filter(|triangle| triangle_has_edge(triangle.vertices, candidate.removed_radial_edge))
        .collect::<Vec<_>>();
    if radial_incident.len() != 2
        || radial_incident
            .iter()
            .any(|triangle| triangle.topology_id != candidate.sector_topology_id as u64)
        || radial_incident
            .iter()
            .map(|triangle| triangle.sector_id)
            .collect::<BTreeSet<_>>()
            != candidate.owner_sector_ids
    {
        return reject(AnchorEarRejectReason::RadialEdgeNotMutable);
    }
    for vertices in candidate.inserted_triangles {
        if degenerate(vertices) {
            return reject(AnchorEarRejectReason::DegenerateAddedTriangle);
        }
        out.push(canonical_owned(OwnedTopologyTriangle {
            topology_id: candidate.sector_topology_id as u64,
            sector_id: candidate.sector_id,
            vertices,
        }));
    }
    out.sort_unstable();
    if has_duplicate_triangles(&out) {
        return reject(AnchorEarRejectReason::DuplicateTriangle);
    }
    Ok(out)
}

/// Builds local mutation conflicts between candidates produced from one
/// topology. PR37B must still recompute final global links and degree ranges.
pub fn build_anchor_ear_conflict_graph(
    candidates: Vec<AnchorEarCandidate>,
) -> AnchorEarConflictGraph {
    let candidate_keys = candidates
        .iter()
        .map(|candidate| candidate.topology_key.clone())
        .collect::<Vec<_>>();
    let mut conflicts = BTreeSet::new();
    for i in 0..candidates.len() {
        for j in i + 1..candidates.len() {
            if candidates_conflict(&candidates[i], &candidates[j]) {
                conflicts.insert((i, j));
            }
        }
    }
    AnchorEarConflictGraph {
        candidate_keys,
        conflicts,
    }
}

fn anchor_candidates(
    source: &MotherGrid,
    anchor_slot: usize,
    contract: &VertexLinkContract,
    sector_topology_id: usize,
    mutable: &[OwnedTopologyTriangle],
    fixed_mesh_edges: &BTreeSet<(usize, usize)>,
    rejections: &mut BTreeMap<usize, BTreeMap<AnchorEarRejectReason, usize>>,
) -> Vec<AnchorEarCandidate> {
    let target_min = usize::from(contract.target_degree_min);
    let target_max = usize::from(contract.target_degree_max);
    let incident = mutable
        .iter()
        .copied()
        .filter(|triangle| triangle.vertices.contains(&anchor_slot))
        .collect::<Vec<_>>();
    let initial_link = anchor_link_edges(anchor_slot, contract, mutable);
    if !single_cycle_edges(&initial_link) {
        count_rejection(
            rejections,
            anchor_slot,
            AnchorEarRejectReason::LinkNotSingleCycle,
        );
        return Vec::new();
    }
    let degree = initial_link.len();
    if degree < target_min {
        count_rejection(
            rejections,
            anchor_slot,
            AnchorEarRejectReason::AnchorBelowTarget,
        );
        return Vec::new();
    }
    if degree <= target_max {
        count_rejection(
            rejections,
            anchor_slot,
            AnchorEarRejectReason::AnchorNotOverfull,
        );
        return Vec::new();
    }
    let context = CandidateContext {
        source,
        anchor_slot,
        sector_topology_id,
        contract,
        mutable,
        fixed_mesh_edges,
        initial_link: &initial_link,
    };
    let mut out = Vec::new();
    for (w, owners) in radial_edge_owners(anchor_slot, &incident) {
        match candidate_from_radial(&context, w, owners) {
            CandidateResult::Candidate(candidate) => out.push(*candidate),
            CandidateResult::Reject(reason) => count_rejection(rejections, anchor_slot, reason),
            CandidateResult::NotEar => {}
        }
    }
    out
}

fn radial_edge_owners(
    anchor_slot: usize,
    incident: &[OwnedTopologyTriangle],
) -> BTreeMap<usize, Vec<OwnedTopologyTriangle>> {
    let mut out = BTreeMap::<usize, Vec<OwnedTopologyTriangle>>::new();
    for &triangle in incident {
        for vertex in triangle.vertices {
            if vertex != anchor_slot {
                out.entry(vertex).or_default().push(triangle);
            }
        }
    }
    out
}

struct CandidateContext<'a> {
    source: &'a MotherGrid,
    anchor_slot: usize,
    sector_topology_id: usize,
    contract: &'a VertexLinkContract,
    mutable: &'a [OwnedTopologyTriangle],
    fixed_mesh_edges: &'a BTreeSet<(usize, usize)>,
    initial_link: &'a BTreeSet<(usize, usize)>,
}

enum CandidateResult {
    Candidate(Box<AnchorEarCandidate>),
    Reject(AnchorEarRejectReason),
    NotEar,
}

fn candidate_from_radial(
    context: &CandidateContext<'_>,
    w: usize,
    owners: Vec<OwnedTopologyTriangle>,
) -> CandidateResult {
    let source = context.source;
    let anchor_slot = context.anchor_slot;
    let sector_topology_id = context.sector_topology_id;
    let contract = context.contract;
    let mutable = context.mutable;
    let fixed_mesh_edges = context.fixed_mesh_edges;
    let initial_link = context.initial_link;
    let [left, right] = owners.as_slice() else {
        return CandidateResult::NotEar;
    };
    if left.topology_id != right.topology_id {
        return CandidateResult::Reject(AnchorEarRejectReason::TopologyIdMismatch);
    }
    let Some(left_edge) = link_edge_for(left.vertices, anchor_slot) else {
        return CandidateResult::NotEar;
    };
    let Some(right_edge) = link_edge_for(right.vertices, anchor_slot) else {
        return CandidateResult::NotEar;
    };
    if !left_edge_contains(left_edge, w) || !left_edge_contains(right_edge, w) {
        return CandidateResult::NotEar;
    }
    let pred = other_endpoint(left_edge, w);
    let succ = other_endpoint(right_edge, w);
    if pred == succ || pred == anchor_slot || succ == anchor_slot || w == anchor_slot {
        return CandidateResult::NotEar;
    }
    let inserted_chord = sorted_edge(pred, succ);
    let mut inserted = [[anchor_slot, pred, succ], [pred, w, succ]];
    inserted
        .iter_mut()
        .for_each(|triangle| triangle.sort_unstable());
    inserted.sort_unstable();
    if inserted.into_iter().any(degenerate) || !vertices_are_live(source, &inserted) {
        return CandidateResult::Reject(AnchorEarRejectReason::DegenerateAddedTriangle);
    }
    let removed = sorted_triangle_pair(left.vertices, right.vertices);
    let existing_triangles = mutable
        .iter()
        .copied()
        .map(canonical_owned)
        .filter(|triangle| !removed.contains(&triangle.vertices))
        .map(|triangle| triangle.vertices)
        .collect::<BTreeSet<_>>();
    if inserted
        .iter()
        .any(|triangle| existing_triangles.contains(triangle))
    {
        return CandidateResult::Reject(AnchorEarRejectReason::ChordAlreadyExists);
    }
    if mesh_edges(mutable).contains(&inserted_chord) || fixed_mesh_edges.contains(&inserted_chord) {
        return CandidateResult::Reject(AnchorEarRejectReason::ChordAlreadyExists);
    }
    if contract.fixed_link_edges.contains(&inserted_chord) {
        return CandidateResult::Reject(AnchorEarRejectReason::LinkEdgeDuplicate);
    }
    let removed_radial_edge = sorted_edge(anchor_slot, w);
    if fixed_mesh_edges.contains(&removed_radial_edge) {
        return CandidateResult::Reject(AnchorEarRejectReason::RadialEdgeNotMutable);
    }
    let mut result_link = initial_link.clone();
    result_link.remove(&sorted_edge(left_edge.0, left_edge.1));
    result_link.remove(&sorted_edge(right_edge.0, right_edge.1));
    if !result_link.insert(inserted_chord) {
        return CandidateResult::Reject(AnchorEarRejectReason::ChordAlreadyExists);
    }
    if !single_cycle_edges(&result_link) {
        return CandidateResult::Reject(AnchorEarRejectReason::LinkNotSingleCycle);
    }
    let owner_sector_ids = BTreeSet::from([left.sector_id, right.sector_id]);
    let sector_id = *owner_sector_ids.first().expect("ear has two owners");
    CandidateResult::Candidate(Box::new(AnchorEarCandidate {
        topology_key: AnchorEarKey {
            anchor_slot,
            sector_id,
            inserted_chord,
            removed_neighbour_slot: w,
        },
        sector_topology_id,
        anchor_slot,
        sector_id,
        removed_triangles: removed,
        inserted_triangles: inserted,
        removed_link_edges: [
            sorted_edge(left_edge.0, left_edge.1),
            sorted_edge(right_edge.0, right_edge.1),
        ],
        removed_radial_edge,
        inserted_chord,
        predecessor_slot: pred,
        removed_neighbour_slot: w,
        successor_slot: succ,
        owner_sector_ids,
        anchor_initial_link_edges: initial_link.clone(),
        anchor_result_link_edges: result_link,
        degree_delta: [(anchor_slot, -1), (w, -1), (pred, 1), (succ, 1)],
    }))
}

fn count_rejection(
    rejections: &mut BTreeMap<usize, BTreeMap<AnchorEarRejectReason, usize>>,
    anchor_slot: usize,
    reason: AnchorEarRejectReason,
) {
    let anchor = rejections.entry(anchor_slot).or_default();
    *anchor.entry(reason).or_default() += 1;
}

fn reject<T>(reason: AnchorEarRejectReason) -> Result<T, AnchorEarApplyError> {
    Err(AnchorEarApplyError { reason })
}

fn removed_radial_edge_is_mutable(edge: (usize, usize), removed: &BTreeSet<[usize; 3]>) -> bool {
    removed
        .iter()
        .all(|&triangle| triangle_has_edge(triangle, edge))
}

fn valid_anchor_ear_contract(candidate: &AnchorEarCandidate) -> bool {
    let anchor = candidate.anchor_slot;
    let predecessor = candidate.predecessor_slot;
    let removed = candidate.removed_neighbour_slot;
    let successor = candidate.successor_slot;
    let expected_removed =
        sorted_triangle_pair([anchor, predecessor, removed], [anchor, removed, successor]);
    let expected_inserted = sorted_triangle_pair(
        [anchor, predecessor, successor],
        [predecessor, removed, successor],
    );
    let mut expected_result_link = candidate.anchor_initial_link_edges.clone();
    let removed_link_edges = [
        sorted_edge(predecessor, removed),
        sorted_edge(removed, successor),
    ];
    let inserted_chord = sorted_edge(predecessor, successor);
    let links_match = removed_link_edges
        .iter()
        .all(|edge| expected_result_link.remove(edge))
        && expected_result_link.insert(inserted_chord)
        && expected_result_link == candidate.anchor_result_link_edges
        && single_cycle_edges(&candidate.anchor_initial_link_edges)
        && single_cycle_edges(&candidate.anchor_result_link_edges);
    let mut actual_removed_link_edges = candidate.removed_link_edges;
    actual_removed_link_edges.sort_unstable();
    let mut expected_removed_link_edges = removed_link_edges;
    expected_removed_link_edges.sort_unstable();
    candidate.removed_triangles == expected_removed
        && candidate.inserted_triangles == expected_inserted
        && actual_removed_link_edges == expected_removed_link_edges
        && candidate.removed_radial_edge == sorted_edge(anchor, removed)
        && candidate.inserted_chord == inserted_chord
        && candidate.owner_sector_ids.first().copied() == Some(candidate.sector_id)
        && candidate.degree_delta
            == [
                (anchor, -1),
                (removed, -1),
                (predecessor, 1),
                (successor, 1),
            ]
        && candidate.topology_key.anchor_slot == anchor
        && candidate.topology_key.sector_id == candidate.sector_id
        && candidate.topology_key.inserted_chord == inserted_chord
        && candidate.topology_key.removed_neighbour_slot == removed
        && links_match
}

fn candidates_conflict(left: &AnchorEarCandidate, right: &AnchorEarCandidate) -> bool {
    let left_removed = left.removed_triangles.into_iter().collect::<BTreeSet<_>>();
    let left_inserted = left.inserted_triangles.into_iter().collect::<BTreeSet<_>>();
    let right_removed = right.removed_triangles.into_iter().collect::<BTreeSet<_>>();
    let right_inserted = right
        .inserted_triangles
        .into_iter()
        .collect::<BTreeSet<_>>();
    let same_anchor_link_conflict = if left.anchor_slot == right.anchor_slot {
        if left.anchor_initial_link_edges != right.anchor_initial_link_edges {
            true
        } else {
            let mut combined = left.anchor_initial_link_edges.clone();
            let removed = left
                .removed_link_edges
                .into_iter()
                .chain(right.removed_link_edges)
                .all(|edge| combined.remove(&edge));
            let inserted =
                combined.insert(left.inserted_chord) && combined.insert(right.inserted_chord);
            !removed || !inserted || !single_cycle_edges(&combined)
        }
    } else {
        false
    };
    same_anchor_link_conflict
        || !left_removed.is_disjoint(&right_removed)
        || !left_removed.is_disjoint(&right_inserted)
        || !left_inserted.is_disjoint(&right_removed)
        || !left_inserted.is_disjoint(&right_inserted)
        || left.inserted_chord == right.inserted_chord
        || left.inserted_chord == right.removed_radial_edge
        || right.inserted_chord == left.removed_radial_edge
}

fn mutable_anchor_link(
    anchor_slot: usize,
    triangles: &[OwnedTopologyTriangle],
) -> BTreeSet<(usize, usize)> {
    triangles
        .iter()
        .filter_map(|triangle| link_edge_for(triangle.vertices, anchor_slot))
        .collect()
}

fn link_edge_for(triangle: [usize; 3], anchor_slot: usize) -> Option<(usize, usize)> {
    let others = triangle
        .into_iter()
        .filter(|&vertex| vertex != anchor_slot)
        .collect::<Vec<_>>();
    (others.len() == 2).then_some(sorted_edge(others[0], others[1]))
}

fn anchor_link_edges(
    anchor_slot: usize,
    contract: &VertexLinkContract,
    mutable: &[OwnedTopologyTriangle],
) -> BTreeSet<(usize, usize)> {
    let mut out = contract.fixed_link_edges.clone();
    out.extend(mutable_anchor_link(anchor_slot, mutable));
    out
}

fn mesh_edges(triangles: &[OwnedTopologyTriangle]) -> BTreeSet<(usize, usize)> {
    triangles
        .iter()
        .flat_map(|triangle| {
            let [a, b, c] = triangle.vertices;
            [sorted_edge(a, b), sorted_edge(b, c), sorted_edge(c, a)]
        })
        .collect()
}

fn fixed_outside_mesh_edges(source: &MotherGrid, face_slots: &[usize]) -> BTreeSet<(usize, usize)> {
    face_slots
        .iter()
        .filter_map(|&face_slot| source.mesh.triangles().get(face_slot).copied())
        .flat_map(|[a, b, c]| [sorted_edge(a, b), sorted_edge(b, c), sorted_edge(c, a)])
        .collect()
}

fn left_edge_contains(edge: (usize, usize), vertex: usize) -> bool {
    edge.0 == vertex || edge.1 == vertex
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
    let mut stack = vec![start];
    let mut seen = BTreeSet::from([start]);
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

fn canonical_owned_triangles(triangles: &[OwnedTopologyTriangle]) -> Vec<OwnedTopologyTriangle> {
    let mut out = triangles
        .iter()
        .copied()
        .map(canonical_owned)
        .collect::<Vec<_>>();
    out.sort_unstable();
    out
}

fn canonical_owned(mut triangle: OwnedTopologyTriangle) -> OwnedTopologyTriangle {
    triangle.vertices.sort_unstable();
    triangle
}

fn sorted_triangle_pair(mut a: [usize; 3], mut b: [usize; 3]) -> [[usize; 3]; 2] {
    a.sort_unstable();
    b.sort_unstable();
    if a <= b {
        [a, b]
    } else {
        [b, a]
    }
}

fn sorted_edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn other_endpoint(edge: (usize, usize), endpoint: usize) -> usize {
    if edge.0 == endpoint {
        edge.1
    } else {
        edge.0
    }
}

fn degenerate(triangle: [usize; 3]) -> bool {
    triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0]
}

fn vertices_are_live(source: &MotherGrid, triangles: &[[usize; 3]]) -> bool {
    triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .all(|vertex| source.mesh.is_vertex_live(vertex))
}

fn triangle_has_edge(triangle: [usize; 3], edge: (usize, usize)) -> bool {
    [
        sorted_edge(triangle[0], triangle[1]),
        sorted_edge(triangle[1], triangle[2]),
        sorted_edge(triangle[2], triangle[0]),
    ]
    .contains(&edge)
}

fn has_duplicate_triangles(triangles: &[OwnedTopologyTriangle]) -> bool {
    let mut seen = BTreeSet::new();
    triangles.iter().any(|triangle| {
        let triangle = canonical_owned(*triangle);
        !seen.insert((triangle.topology_id, triangle.vertices))
    })
}

fn canonical_vertices(mut vertices: [usize; 3]) -> [usize; 3] {
    vertices.sort_unstable();
    vertices
}
