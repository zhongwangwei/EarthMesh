use earthmesh_mesh::MeshState;
use earthmesh_refine_certified::{
    coarsen::{
        apply_anchor_ear, build_anchor_ear_conflict_graph, build_stratified_annulus,
        derive_anchor_ear_candidates, n6_legacy_mixed_fixture, AnchorEarRejectReason,
        OwnedTopologyTriangle, StratifiedAnnulus,
    },
    MotherGrid,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn cross_chain_only_model_proves_anchor_degree_lower_bound_six() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();

    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();

    assert_eq!(
        [2, 29, 77, 155]
            .into_iter()
            .map(|anchor| (anchor, family.initial_anchor_degrees[&anchor]))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([(2, 6), (29, 6), (77, 6), (155, 6)])
    );
}

#[test]
fn n6_anchor_ear_candidates_include_all_four_frozen_witness_chords() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();

    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();

    assert!(family.generated_by_generic_link_discovery);
    assert_eq!(
        witness_chords_by_anchor(&family.candidates),
        BTreeMap::from([
            (2, BTreeSet::from([(9, 72)])),
            (29, BTreeSet::from([(28, 149)])),
            (77, BTreeSet::from([(82, 171)])),
            (155, BTreeSet::from([(154, 160)])),
        ])
    );
    for (anchor, chord) in [
        (29, (28, 149)),
        (77, (82, 171)),
        (2, (9, 72)),
        (155, (154, 160)),
    ] {
        let candidate = candidate_for(&family.candidates, anchor, chord);
        assert_eq!(candidate.removed_triangles.len(), 2);
        assert_eq!(candidate.inserted_triangles.len(), 2);
        assert_eq!(candidate.anchor_initial_link_edges.len(), 6);
        assert_eq!(candidate.anchor_result_link_edges.len(), 5);
        assert!(candidate.degree_delta.contains(&(anchor, -1)));
    }
}

#[test]
fn anchor_ear_flip_reduces_slot29_link_length_from_six_to_five() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();
    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();
    let candidate = candidate_for(&family.candidates, 29, (28, 149));

    let flipped = apply_anchor_ear(&baseline, candidate).unwrap();
    let evidence = derive_anchor_ear_candidates(&source, &stratified, 0, &flipped).unwrap();

    assert_eq!(family.initial_anchor_degrees[&29], 6);
    assert_eq!(evidence.initial_anchor_degrees[&29], 5);
}

#[test]
fn anchor_ear_flip_replaces_two_triangles_with_two_triangles() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();
    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();
    let candidate = candidate_for(&family.candidates, 29, (28, 149));

    let flipped = apply_anchor_ear(&baseline, candidate).unwrap();

    assert_eq!(candidate.removed_triangles.len(), 2);
    assert_eq!(candidate.inserted_triangles.len(), 2);
    assert_eq!(candidate.removed_radial_edge, (29, 147));
    let removed_sector_ids = candidate
        .removed_triangles
        .iter()
        .map(|removed| {
            baseline
                .iter()
                .find(|triangle| canonical_triangle(triangle) == canonical_vertices(*removed))
                .unwrap_or_else(|| panic!("missing removed triangle {removed:?}"))
                .sector_id
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(removed_sector_ids.len(), 1);
    assert_eq!(baseline.len(), flipped.len());
    for triangle in candidate.removed_triangles {
        assert!(!flipped
            .iter()
            .any(|actual| canonical_triangle(actual) == canonical_vertices(triangle)));
    }
    for triangle in candidate.inserted_triangles {
        assert!(flipped
            .iter()
            .any(|actual| canonical_triangle(actual) == canonical_vertices(triangle)));
    }
}

#[test]
fn anchor_ear_flip_preserves_euler_and_total_degree_charge() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();
    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();
    let candidate = candidate_for(&family.candidates, 29, (28, 149));

    let flipped = apply_anchor_ear(&baseline, candidate).unwrap();

    assert_eq!(euler_delta(&baseline, &flipped), 0);
    let degree_delta = candidate
        .degree_delta
        .iter()
        .copied()
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        degree_delta,
        BTreeMap::from([(28, 1), (29, -1), (147, -1), (149, 1)])
    );
    assert_eq!(
        degree_delta
            .values()
            .map(|delta| i16::from(*delta))
            .sum::<i16>(),
        0
    );
}

#[test]
fn anchor_ear_flip_rejects_existing_chord() {
    let (source, stratified, mut baseline) = n6_restricted_two_chain_baseline();
    let live = baseline
        .iter()
        .flat_map(|triangle| triangle.vertices)
        .find(|slot| ![28, 29, 149].contains(slot))
        .unwrap();
    baseline.push(OwnedTopologyTriangle {
        topology_id: 0,
        sector_id: 99,
        vertices: [28, 149, live],
    });

    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();

    assert_eq!(
        family.rejections[&29].get(&AnchorEarRejectReason::ChordAlreadyExists),
        Some(&1)
    );
}

#[test]
fn anchor_ear_flip_rejects_fixed_outside_radial_edge() {
    let (mut source, mut stratified, baseline) = n6_restricted_two_chain_baseline();
    let radial = (29, 147);
    let mut vertices = source.mesh.vertices().to_vec();
    let outside_slot = vertices.len();
    vertices.push(vertices[29]);
    let mut triangles = source.mesh.triangles().to_vec();
    let fixed_face = triangles.len();
    triangles.push([radial.0, radial.1, outside_slot]);
    source.mesh = MeshState::from_parts(vertices, triangles).unwrap();
    source.addresses.push(None);
    source.triangle_addresses.push(None);
    stratified.coupled.fixed_outside_face_slots.push(fixed_face);

    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();

    assert!(!family
        .candidates
        .iter()
        .any(|candidate| candidate.removed_radial_edge == radial));
    assert_eq!(
        family.rejections[&29].get(&AnchorEarRejectReason::RadialEdgeNotMutable),
        Some(&1)
    );
}

#[test]
fn anchor_ear_candidates_reject_topology_id_mismatch() {
    let (source, stratified, mut baseline) = n6_restricted_two_chain_baseline();
    baseline[0].topology_id = 1;

    let rejected = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap_err();

    assert_eq!(rejected.reason, AnchorEarRejectReason::TopologyIdMismatch);
}

#[test]
fn anchor_ear_candidates_fail_closed_on_broken_anchor_link() {
    let (source, stratified, mut baseline) = n6_restricted_two_chain_baseline();
    let removed = baseline
        .iter()
        .position(|triangle| triangle.vertices.contains(&29))
        .expect("slot29 must have mutable incident triangles");
    baseline.remove(removed);

    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();

    assert!(!family
        .candidates
        .iter()
        .any(|candidate| candidate.anchor_slot == 29));
    assert_eq!(
        family.rejections[&29].get(&AnchorEarRejectReason::LinkNotSingleCycle),
        Some(&1)
    );
}

#[test]
fn anchor_ear_apply_rejects_a_stale_existing_chord() {
    let (source, stratified, mut baseline) = n6_restricted_two_chain_baseline();
    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();
    let candidate = candidate_for(&family.candidates, 29, (28, 149));
    let live = baseline
        .iter()
        .flat_map(|triangle| triangle.vertices)
        .find(|slot| ![28, 29, 149].contains(slot))
        .unwrap();
    baseline.push(OwnedTopologyTriangle {
        topology_id: 0,
        sector_id: 99,
        vertices: [28, 149, live],
    });

    let rejected = apply_anchor_ear(&baseline, candidate).unwrap_err();

    assert_eq!(rejected.reason, AnchorEarRejectReason::ChordAlreadyExists);
}

#[test]
fn anchor_ear_apply_rejects_a_forged_non_ear_insertion() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();
    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();
    let mut candidate = candidate_for(&family.candidates, 29, (28, 149)).clone();
    candidate.inserted_triangles[1] = [2, 9, 72];

    let rejected = apply_anchor_ear(&baseline, &candidate).unwrap_err();

    assert_eq!(rejected.reason, AnchorEarRejectReason::InvalidEarContract);
}

#[test]
fn anchor_ear_flip_rejects_conflicting_candidates() {
    let (source, stratified, baseline) = n6_restricted_two_chain_baseline();
    let family = derive_anchor_ear_candidates(&source, &stratified, 0, &baseline).unwrap();
    let first = candidate_for(&family.candidates, 29, (28, 149)).clone();
    let mut second = first.clone();
    second.topology_key.removed_neighbour_slot += 1;

    let graph = build_anchor_ear_conflict_graph(vec![first, second]);

    assert_eq!(graph.conflicts, BTreeSet::from([(0, 1)]));
}

#[test]
fn new_cat_topology_does_not_call_legacy_per_parent_solver() {
    let source = include_str!("../src/coarsen/anchor_ear.rs");

    assert!(!source.contains("solve_transition_topology("));
    assert!(!source.contains("analyze_legacy_transition_family("));
}

fn n6_restricted_two_chain_baseline() -> (MotherGrid, StratifiedAnnulus, Vec<OwnedTopologyTriangle>)
{
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let baseline = restricted_two_chain_baseline(&stratified);
    (source, stratified, baseline)
}

fn restricted_two_chain_baseline(stratified: &StratifiedAnnulus) -> Vec<OwnedTopologyTriangle> {
    let triangulations = stratified
        .probe
        .sector_components
        .iter()
        .enumerate()
        .map(|(sector_id, sector)| {
            restricted_two_chain_triangulations(
                sector_id as u64,
                &sector.lower_chain,
                &sector.upper_chain,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        triangulations.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 1, 15, 15, 1, 1, 6, 6, 1, 1, 1, 1, 6, 6]
    );
    triangulations
        .into_iter()
        .flat_map(|mut choices| {
            choices.sort_by_key(|topology| canonical_topology(topology));
            choices.into_iter().next().unwrap_or_default()
        })
        .collect()
}

fn restricted_two_chain_triangulations(
    sector_id: u64,
    lower_chain: &[usize],
    upper_chain: &[usize],
) -> Vec<Vec<OwnedTopologyTriangle>> {
    let mut polygon = lower_chain.to_vec();
    polygon.extend(
        upper_chain
            .iter()
            .rev()
            .skip(1)
            .take(upper_chain.len().saturating_sub(2)),
    );
    let lower_edges = chain_edges(lower_chain);
    let upper_edges = chain_edges(upper_chain);
    let lower_vertices = lower_chain.iter().copied().collect::<BTreeSet<_>>();
    let upper_vertices = upper_chain.iter().copied().collect::<BTreeSet<_>>();
    let mut memo = BTreeMap::new();
    triangulate_interval(
        sector_id,
        &polygon,
        &lower_edges,
        &upper_edges,
        &lower_vertices,
        &upper_vertices,
        0,
        polygon.len() - 1,
        &mut memo,
    )
}

#[allow(clippy::too_many_arguments)]
fn triangulate_interval(
    sector_id: u64,
    polygon: &[usize],
    lower_edges: &BTreeSet<(usize, usize)>,
    upper_edges: &BTreeSet<(usize, usize)>,
    lower_vertices: &BTreeSet<usize>,
    upper_vertices: &BTreeSet<usize>,
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
        let vertices = [polygon[lo], polygon[mid], polygon[hi]];
        if !distinct(vertices)
            || !triangle_edges_allowed(
                vertices,
                polygon,
                lower_edges,
                upper_edges,
                lower_vertices,
                upper_vertices,
            )
        {
            continue;
        }
        let left_choices = triangulate_interval(
            sector_id,
            polygon,
            lower_edges,
            upper_edges,
            lower_vertices,
            upper_vertices,
            lo,
            mid,
            memo,
        );
        let right_choices = triangulate_interval(
            sector_id,
            polygon,
            lower_edges,
            upper_edges,
            lower_vertices,
            upper_vertices,
            mid,
            hi,
            memo,
        );
        for left in left_choices {
            for right in &right_choices {
                let mut candidate = left.clone();
                candidate.push(OwnedTopologyTriangle {
                    topology_id: 0,
                    sector_id,
                    vertices,
                });
                candidate.extend(right.iter().cloned());
                candidate.sort_by_key(canonical_triangle);
                out.push(candidate);
            }
        }
    }
    out.sort_by_key(|topology| canonical_topology(topology));
    out.dedup_by_key(|topology| canonical_topology(topology));
    memo.insert((lo, hi), out.clone());
    out
}

fn triangle_edges_allowed(
    [a, b, c]: [usize; 3],
    polygon: &[usize],
    lower_edges: &BTreeSet<(usize, usize)>,
    upper_edges: &BTreeSet<(usize, usize)>,
    lower_vertices: &BTreeSet<usize>,
    upper_vertices: &BTreeSet<usize>,
) -> bool {
    [sorted(a, b), sorted(b, c), sorted(c, a)]
        .into_iter()
        .all(|edge| {
            polygon_boundary_edges(polygon).contains(&edge)
                || lower_edges.contains(&edge)
                || upper_edges.contains(&edge)
                || is_cross_chain_edge(edge, lower_vertices, upper_vertices)
        })
}

fn is_cross_chain_edge(
    (a, b): (usize, usize),
    lower_vertices: &BTreeSet<usize>,
    upper_vertices: &BTreeSet<usize>,
) -> bool {
    let a_is_strict_lower = lower_vertices.contains(&a) && !upper_vertices.contains(&a);
    let b_is_strict_lower = lower_vertices.contains(&b) && !upper_vertices.contains(&b);
    let a_is_strict_upper = upper_vertices.contains(&a) && !lower_vertices.contains(&a);
    let b_is_strict_upper = upper_vertices.contains(&b) && !lower_vertices.contains(&b);
    (a_is_strict_lower && b_is_strict_upper) || (a_is_strict_upper && b_is_strict_lower)
}

fn chain_edges(chain: &[usize]) -> BTreeSet<(usize, usize)> {
    chain
        .windows(2)
        .map(|edge| sorted(edge[0], edge[1]))
        .collect()
}

fn polygon_boundary_edges(polygon: &[usize]) -> BTreeSet<(usize, usize)> {
    polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1))
        .take(polygon.len())
        .map(|(a, b)| sorted(a, b))
        .collect()
}

fn canonical_topology(topology: &[OwnedTopologyTriangle]) -> Vec<[usize; 3]> {
    let mut triangles = topology.iter().map(canonical_triangle).collect::<Vec<_>>();
    triangles.sort();
    triangles
}

fn canonical_triangle(triangle: &OwnedTopologyTriangle) -> [usize; 3] {
    canonical_vertices(triangle.vertices)
}

fn canonical_vertices(mut vertices: [usize; 3]) -> [usize; 3] {
    vertices.sort();
    vertices
}

fn witness_chords_by_anchor(
    candidates: &[earthmesh_refine_certified::coarsen::AnchorEarCandidate],
) -> BTreeMap<usize, BTreeSet<(usize, usize)>> {
    let mut out = BTreeMap::<usize, BTreeSet<(usize, usize)>>::new();
    for candidate in candidates {
        out.entry(candidate.anchor_slot)
            .or_default()
            .insert(candidate.inserted_chord);
    }
    out
}

fn candidate_for(
    candidates: &[earthmesh_refine_certified::coarsen::AnchorEarCandidate],
    anchor_slot: usize,
    chord: (usize, usize),
) -> &earthmesh_refine_certified::coarsen::AnchorEarCandidate {
    candidates
        .iter()
        .find(|candidate| candidate.anchor_slot == anchor_slot && candidate.inserted_chord == chord)
        .unwrap_or_else(|| panic!("missing anchor {anchor_slot} chord {chord:?}"))
}

fn euler_delta(before: &[OwnedTopologyTriangle], after: &[OwnedTopologyTriangle]) -> isize {
    euler(after) - euler(before)
}

fn euler(triangles: &[OwnedTopologyTriangle]) -> isize {
    let vertices = triangles
        .iter()
        .flat_map(|triangle| triangle.vertices)
        .collect::<BTreeSet<_>>();
    let edges = triangles
        .iter()
        .flat_map(|triangle| {
            let [a, b, c] = triangle.vertices;
            [sorted(a, b), sorted(b, c), sorted(c, a)]
        })
        .collect::<BTreeSet<_>>();
    vertices.len() as isize - edges.len() as isize + triangles.len() as isize
}

fn sorted(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn distinct([a, b, c]: [usize; 3]) -> bool {
    a != b && b != c && c != a
}
