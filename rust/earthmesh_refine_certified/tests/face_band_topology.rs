use earthmesh_refine_certified::{
    coarsen::{
        build_face_band_problem, build_face_band_problem_with_source_face_rings,
        build_stratified_annulus_from_face_bands, n6_legacy_mixed_fixture, solve_exact_face_bands,
        solve_full_polygon_merge_from_face_bands, AnchorBandPolicy, BandComponentKind,
        FaceBandLimits, FaceBandPlan, FaceBandSolveOutcome, FullPolygonMergeLimits,
        FullPolygonMergeOutcome, HierarchyComponent,
    },
    MotherGrid,
};
use std::{collections::BTreeSet, fs, process::Command};

const FACE_BAND_STATES: u64 = 1_000_000;
const TOPOLOGY_STATES: usize = 1_000;

#[test]
fn frozen_n6_face_band_interfaces_produce_disjoint_polygon_sectors() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan(&source, &component);
    let stratified = build_stratified_annulus_from_face_bands(&source, &component, &plan).unwrap();

    assert_eq!(stratified.traces.len(), 3);
    assert_eq!(stratified.bands.len(), 2);
    assert!(stratified.shared_junctions.is_empty());
    assert!(stratified
        .bands
        .iter()
        .all(|band| matches!(band.kind, BandComponentKind::Annular { .. })));
    assert_eq!(stratified.probe.sector_count, 28);
    assert_eq!(
        stratified
            .probe
            .sector_components
            .iter()
            .filter(|sector| sector.band_id == 0)
            .count(),
        8
    );
    assert_eq!(
        stratified
            .probe
            .sector_components
            .iter()
            .filter(|sector| sector.band_id == 1)
            .count(),
        20
    );
    assert!(stratified.probe.sector_components.iter().all(|sector| {
        sector.lower_chain.len() == 2
            && !sector.upper_chain.is_empty()
            && sector
                .lower_chain
                .iter()
                .collect::<BTreeSet<_>>()
                .is_disjoint(&sector.upper_chain.iter().collect())
    }));
    let actual_interface = stratified.traces[1]
        .directed_edges
        .iter()
        .map(|edge| sorted(edge.from, edge.to))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_interface,
        plan.interface_edges[0].iter().copied().collect()
    );
    let mut stale = (*plan).clone();
    stale.band_face_counts[0] += 1;
    assert!(build_stratified_annulus_from_face_bands(&source, &component, &stale).is_err());
}

#[test]
fn frozen_n6_pinch_free_w2_full_polygon_topology_closes() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan(&source, &component);
    let outcome = solve_full_polygon_merge_from_face_bands(
        &source,
        &component,
        &plan,
        FullPolygonMergeLimits {
            topology_states: TOPOLOGY_STATES,
        },
    );
    let FullPolygonMergeOutcome::Closed(trial) = outcome else {
        panic!("Frozen N6 PF-W2 topology must close: {outcome:?}");
    };
    let global = &trial.global_trial.evidence;
    assert_eq!(trial.evidence.states_examined, 31);
    assert_eq!(trial.evidence.selected_topology_keys.len(), 28);
    assert!(trial.evidence.selected_ears.is_empty());
    assert_eq!(global.anchor_degrees.len(), 4);
    assert!(global.anchor_degrees.values().all(|&degree| degree == 5));
    assert!(global
        .ordinary_degree_histogram
        .keys()
        .all(|degree| (5..=7).contains(degree)));
    assert_eq!(
        global.ordinary_degree_histogram,
        [(5, 21), (6, 303), (7, 13)].into()
    );
    assert_eq!(
        (global.vertices, global.edges, global.faces),
        (341, 1017, 678)
    );
    assert_eq!(global.euler, 2);
    assert_eq!(global.charge, 12);
    assert!(global.faces < global.source_faces);
    assert!(global.vertices < global.source_vertices);
    let subdivisions = trial
        .global_trial
        .mesh
        .triangle_addresses
        .iter()
        .flatten()
        .map(|address| address.n)
        .collect::<BTreeSet<_>>();
    assert!(subdivisions.len() >= 2, "materialized mesh must stay mixed");
}

#[test]
fn face_band_topology_budget_exhaustion_remains_unknown() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan(&source, &component);
    assert!(matches!(
        solve_full_polygon_merge_from_face_bands(
            &source,
            &component,
            &plan,
            FullPolygonMergeLimits { topology_states: 0 },
        ),
        FullPolygonMergeOutcome::SearchBudgetExhausted(_)
    ));
}

#[test]
fn frozen_n6_face_band_topology_search_is_deterministic() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan(&source, &component);
    let solve = || {
        solve_full_polygon_merge_from_face_bands(
            &source,
            &component,
            &plan,
            FullPolygonMergeLimits {
                topology_states: TOPOLOGY_STATES,
            },
        )
    };
    assert_eq!(solve(), solve());
}

#[test]
#[ignore = "explicit Frozen N6 PR57 PF-W2 topology gate"]
fn frozen_n6_pr57_pfw2_topology_probe() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_plan(&source, &component);
    let FullPolygonMergeOutcome::Closed(trial) = solve_full_polygon_merge_from_face_bands(
        &source,
        &component,
        &plan,
        FullPolygonMergeLimits {
            topology_states: TOPOLOGY_STATES,
        },
    ) else {
        panic!("Frozen N6 PF-W2 topology must close");
    };
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".into());
    let global = &trial.global_trial.evidence;
    let anchors = global
        .anchor_degrees
        .iter()
        .map(|(vertex, degree)| format!("\"{vertex}\":{degree}"))
        .collect::<Vec<_>>()
        .join(",");
    let ordinary = global
        .ordinary_degree_histogram
        .iter()
        .map(|(degree, count)| format!("\"{degree}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr57PfW2Topology\",\"commit_sha\":{},\"taskbook_sha256\":\"46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b\",\"face_complex_fingerprint\":{},\"interface_edge_counts\":[20],\"sector_count\":28,\"sector_band_counts\":[8,20],\"topology_states\":{},\"selected_topologies\":{},\"selected_ears\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"anchor_degrees\":{{{}}},\"ordinary_degree_histogram\":{{{}}},\"mixed_levels\":true,\"outcome\":\"Closed\"}}",
        commit,
        plan.face_complex_fingerprint,
        trial.evidence.states_examined,
        trial.evidence.selected_topology_keys.len(),
        trial.evidence.selected_ears.len(),
        global.vertices,
        global.edges,
        global.faces,
        global.euler,
        global.charge,
        anchors,
        ordinary,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit Frozen N6 PR60 W3 topology gate"]
fn frozen_n6_pr60_w3_topology_probe() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let plan = frozen_w3_plan(&source, &component);
    assert_eq!(plan.band_face_counts, [36, 52, 58]);
    assert_eq!(
        plan.interface_edges
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        [20, 28]
    );

    let stratified = build_stratified_annulus_from_face_bands(&source, &component, &plan).unwrap();
    assert_eq!(stratified.traces.len(), 4);
    assert_eq!(stratified.bands.len(), 3);
    assert!(stratified.shared_junctions.is_empty());
    assert!(stratified
        .bands
        .iter()
        .all(|band| matches!(band.kind, BandComponentKind::Annular { .. })));
    assert_eq!(stratified.probe.sector_count, 56);
    assert_eq!(
        (0..3)
            .map(|band| stratified
                .probe
                .sector_components
                .iter()
                .filter(|sector| sector.band_id == band)
                .count())
            .collect::<Vec<_>>(),
        [8, 20, 28]
    );

    let FullPolygonMergeOutcome::Closed(trial) = solve_full_polygon_merge_from_face_bands(
        &source,
        &component,
        &plan,
        FullPolygonMergeLimits {
            topology_states: TOPOLOGY_STATES,
        },
    ) else {
        panic!("Frozen N6 W3 topology must close")
    };
    let global = &trial.global_trial.evidence;
    assert_eq!(trial.evidence.states_examined, 70);
    assert_eq!(trial.evidence.selected_topology_keys.len(), 56);
    assert_eq!(
        global.anchor_degrees,
        [(2, 5), (29, 5), (77, 5), (155, 5)].into()
    );
    assert_eq!(
        global.ordinary_degree_histogram,
        [(5, 25), (6, 295), (7, 17)].into()
    );
    assert_eq!(
        (global.vertices, global.edges, global.faces),
        (341, 1017, 678)
    );
    assert_eq!(global.euler, 2);
    assert_eq!(global.charge, 12);
    let mixed_levels = trial
        .global_trial
        .mesh
        .triangle_addresses
        .iter()
        .flatten()
        .map(|address| address.n)
        .collect::<BTreeSet<_>>()
        .len()
        >= 2;
    assert!(mixed_levels);

    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr60W3Topology\",\"taskbook_sha256\":\"46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b\",\"face_complex_fingerprint\":{},\"band_face_counts\":[36,52,58],\"interface_edge_counts\":[20,28],\"sector_band_counts\":[8,20,28],\"topology_states\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"anchor_degrees\":{{\"2\":5,\"29\":5,\"77\":5,\"155\":5}},\"ordinary_degree_histogram\":{{\"5\":25,\"6\":295,\"7\":17}},\"mixed_levels\":true,\"outcome\":\"Closed\"}}",
        plan.face_complex_fingerprint,
        trial.evidence.states_examined,
        global.vertices,
        global.edges,
        global.faces,
        global.euler,
        global.charge,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

fn frozen_plan(source: &MotherGrid, component: &HierarchyComponent) -> Box<FaceBandPlan> {
    let problem = build_face_band_problem(source, component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: FACE_BAND_STATES,
        },
    ) else {
        panic!("Frozen N6 PF-W2 face plan must close")
    };
    plan
}

fn frozen_w3_plan(source: &MotherGrid, component: &HierarchyComponent) -> Box<FaceBandPlan> {
    let mut problem =
        build_face_band_problem_with_source_face_rings(source, component, 3, 2).unwrap();
    for anchor in [2usize, 155] {
        problem
            .anchor_policies
            .insert(anchor, AnchorBandPolicy::OnSingleInterface);
    }
    let FaceBandSolveOutcome::Closed(plan, evidence) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: FACE_BAND_STATES,
        },
    ) else {
        panic!("Frozen N6 W3 face plan must close")
    };
    assert_eq!(evidence.states_examined, 722_766);
    plan
}

fn sorted(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
