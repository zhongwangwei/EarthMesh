use earthmesh_refine_certified::coarsen::{
    initial_elastic_phase, n6_legacy_mixed_fixture, solve_elastic_patch, solve_full_polygon_merge,
    ElasticBlockLimits, ElasticBlockOutcome, ElasticBlockPhase, ElasticPatch,
    FullPolygonMergeLimits, FullPolygonMergeOutcome, TransitionTopologyCandidate,
};
use std::{collections::BTreeMap, collections::BTreeSet, fs};

#[test]
fn zero_budget_is_unknown_exhaustion() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits { topology_states: 0 },
    );
    let FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
        panic!("zero budget must not prove no-go: {outcome:?}");
    };
    assert_eq!(evidence.states_examined, 0);
    assert!(evidence.sector_family_counts.contains(&132));
}

#[test]
fn one_budget_is_unknown_exhaustion() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits { topology_states: 1 },
    );
    let FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
        panic!("one budget must not prove no-go: {outcome:?}");
    };
    assert_eq!(evidence.states_examined, 1);
}

#[test]
fn n6_full_polygon_accepts_more_than_64_variants() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits { topology_states: 0 },
    );
    let FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
        panic!("expected budget exhaustion evidence");
    };
    assert_eq!(
        evidence.sector_family_counts,
        vec![5, 5, 132, 132, 5, 5, 132, 132, 14, 14, 14, 14, 132, 132]
    );
}

#[test]
fn implementation_does_not_call_legacy_solver_or_sector_variants() {
    let source = fs::read_to_string("src/coarsen/full_polygon_merge.rs").unwrap();
    assert!(!source.contains("solve_global_exact_merge"));
    assert!(!source.contains("sector_variants"));
    assert!(source.contains("enumerate_stratified_full_polygon_families"));
}

#[test]
fn n6_bounded_smoke_runs_without_false_nogo() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits {
            topology_states: 32,
        },
    );
    match outcome {
        FullPolygonMergeOutcome::Closed(_) | FullPolygonMergeOutcome::SearchBudgetExhausted(_) => {}
        other => panic!("bounded smoke must not report exhausted-family no-go: {other:?}"),
    }
}

#[test]
fn frozen_n6_closes_full_polygon_topology_gate() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits {
            topology_states: 100_000,
        },
    );
    let FullPolygonMergeOutcome::Closed(trial) = outcome else {
        panic!("frozen N6 full-polygon family must close: {outcome:?}");
    };
    let global = &trial.global_trial.evidence;
    assert_eq!(global.anchor_degrees.len(), 4);
    assert!(global.anchor_degrees.values().all(|&degree| degree == 5));
    assert!(global
        .ordinary_degree_histogram
        .keys()
        .all(|degree| (5..=7).contains(degree)));
    assert_eq!(global.euler, 2);
    assert_eq!(global.charge, 12);
    assert!(global.faces < global.source_faces);
    assert!(global.vertices < global.source_vertices);
    assert_eq!(trial.evidence.selected_topology_keys.len(), 14);
    assert!(trial.evidence.states_examined <= 100_000);
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
fn frozen_n6_closed_topology_enters_untangle_not_invalid_patch() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits {
            topology_states: 100_000,
        },
    );
    let FullPolygonMergeOutcome::Closed(trial) = outcome else {
        panic!("frozen N6 full-polygon family must close: {outcome:?}");
    };
    let source_to_compact = trial
        .global_trial
        .mesh
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| source.map(|source| (source, compact)))
        .collect::<BTreeMap<_, _>>();
    let custom_source_vertices = trial
        .global_trial
        .custom_triangles
        .iter()
        .flat_map(|triangle| triangle.iter().copied())
        .collect::<BTreeSet<_>>();
    let anchor_sources = trial
        .global_trial
        .evidence
        .anchor_degrees
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let movable_sources = custom_source_vertices
        .difference(&anchor_sources)
        .copied()
        .collect::<BTreeSet<_>>();
    let movable = movable_sources
        .iter()
        .filter_map(|source| source_to_compact.get(source).copied())
        .filter(|&compact| trial.global_trial.mesh.mesh.is_vertex_live(compact))
        .collect::<BTreeSet<_>>();
    let guard = trial
        .global_trial
        .mesh
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            trial.global_trial.mesh.mesh.triangles()[face]
                .iter()
                .any(|site| movable.contains(site))
        })
        .collect::<BTreeSet<_>>();
    let fixed = guard
        .iter()
        .flat_map(|&face| trial.global_trial.mesh.mesh.triangles()[face])
        .filter(|site| !movable.contains(site))
        .collect::<BTreeSet<_>>();
    let patch = ElasticPatch {
        topology: TransitionTopologyCandidate {
            component_id: 43,
            topology_id: 0,
            core_parents: Vec::new(),
            custom_transition_triangles: BTreeMap::new(),
            source_triangles: trial.global_trial.custom_triangles.clone(),
            source_active_vertices: movable_sources
                .iter()
                .chain(fixed.iter().filter_map(|compact| {
                    trial.global_trial.mesh.source_vertex_slots[*compact].as_ref()
                }))
                .copied()
                .collect(),
            source_degree_forecast: BTreeMap::new(),
        },
        reference_positions: trial.global_trial.mesh.mesh.vertices().to_vec(),
        fixed_compact_vertices: fixed.into_iter().collect(),
        movable_compact_vertices: movable.into_iter().collect(),
        guard_faces: guard.into_iter().collect(),
    };
    assert_eq!(
        initial_elastic_phase(&trial.global_trial.mesh, &patch).unwrap(),
        ElasticBlockPhase::Untangle
    );
    assert!(matches!(
        solve_elastic_patch(
            &trial.global_trial.mesh,
            patch,
            ElasticBlockLimits {
                elastic_iterations: 0,
            },
        ),
        ElasticBlockOutcome::SearchBudgetExhausted { .. }
    ));
}

#[test]
#[ignore = "explicit finite N6 topology probe"]
fn n6_full_polygon_topology_probe() {
    let limit = std::env::var("EARTHMESH_FULL_POLYGON_STATES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge(
        &source,
        &component,
        FullPolygonMergeLimits {
            topology_states: limit,
        },
    );
    match outcome {
        FullPolygonMergeOutcome::Closed(trial) => eprintln!(
            "limit={limit} outcome=closed topology_states={} depth={:?} edge_closed={} ear_feasible={} ear_states={} retained={:?} anchors={:?} ordinary={:?} vertices={} edges={} faces={} euler={} charge={}",
            trial.evidence.states_examined,
            trial.evidence.states_by_depth,
            trial.evidence.topology_candidates_closed,
            trial.evidence.ear_degree_feasible_candidates,
            trial.evidence.ear_states_examined,
            trial.evidence.retained_topology_counts,
            trial.global_trial.evidence.anchor_degrees,
            trial.global_trial.evidence.ordinary_degree_histogram,
            trial.global_trial.evidence.vertices,
            trial.global_trial.evidence.edges,
            trial.global_trial.evidence.faces,
            trial.global_trial.evidence.euler,
            trial.global_trial.evidence.charge
        ),
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence) => eprintln!(
            "limit={limit} outcome=no-solution topology_states={} ear_states={}",
            evidence.states_examined, evidence.ear_states_examined
        ),
        FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) => eprintln!(
            "limit={limit} outcome=budget-exhausted topology_states={} depth={:?} edge_closed={} ear_feasible={} ear_states={} retained={:?} best_ears={} best_hist={:?} best_euler={} best_charge={}",
            evidence.states_examined,
            evidence.states_by_depth,
            evidence.topology_candidates_closed,
            evidence.ear_degree_feasible_candidates,
            evidence.ear_states_examined,
            evidence.retained_topology_counts,
            evidence.best_global_evidence.selected_ears.len(),
            evidence.best_global_evidence.ordinary_degree_histogram,
            evidence.best_global_evidence.euler,
            evidence.best_global_evidence.charge
        ),
        FullPolygonMergeOutcome::InvalidInput { reason, evidence } => eprintln!(
            "limit={limit} outcome=invalid reason={reason:?} topology_states={} ear_states={}",
            evidence.states_examined, evidence.ear_states_examined
        ),
    }
}
