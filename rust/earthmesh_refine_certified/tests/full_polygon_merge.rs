use earthmesh_refine_certified::coarsen::{
    build_stratified_annulus, frozen_n6_geometry_evidence_json, initial_elastic_phase,
    n6_legacy_mixed_fixture, n6_legacy_mixed_fixture_with_source_levels, solve_elastic_patch,
    solve_full_polygon_merge, solve_full_polygon_merge_free_interface_cber, ElasticBlockLimits,
    ElasticBlockOutcome, ElasticBlockPhase, ElasticPatch, FullPolygonCberLimits,
    FullPolygonMergeLimits, FullPolygonMergeOutcome, RingAnchorKind, TransitionTopologyCandidate,
};
use earthmesh_refine_certified::{remap::ConservativeRemap, SourceLevelField, TargetLevelField};
use std::{collections::BTreeMap, collections::BTreeSet, fs, process::Command};

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
fn free_interface_patch_keeps_remote_guards_fixed_and_moves_interfaces() {
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
    let stratified = build_stratified_annulus(&source, &component).unwrap();
    let patch =
        ElasticPatch::from_full_polygon_merge(&source, &component, &trial, &BTreeSet::new())
            .unwrap();
    let source_slots = &trial.global_trial.mesh.source_vertex_slots;
    let fixed_sources = patch
        .fixed_compact_vertices
        .iter()
        .filter_map(|&compact| source_slots[compact])
        .collect::<BTreeSet<_>>();
    let movable_sources = patch
        .movable_compact_vertices
        .iter()
        .filter_map(|&compact| source_slots[compact])
        .collect::<BTreeSet<_>>();
    let remote_guards = stratified
        .coupled
        .inner_guard
        .vertices
        .iter()
        .chain(&stratified.coupled.outer_guard.vertices)
        .map(|vertex| vertex.source_slot)
        .collect::<BTreeSet<_>>();
    let original_anchors = stratified
        .link_contracts
        .iter()
        .filter_map(|(&source, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
            .then_some(source)
        })
        .collect::<BTreeSet<_>>();
    let fixed_position_sources = stratified
        .coupled
        .inner_guard
        .vertices
        .iter()
        .chain(&stratified.coupled.coarse_interface.vertices)
        .chain(
            stratified
                .coupled
                .intermediate_rings
                .iter()
                .flat_map(|ring| ring.vertices.iter()),
        )
        .chain(&stratified.coupled.fine_interface.vertices)
        .chain(&stratified.coupled.outer_guard.vertices)
        .filter_map(|vertex| vertex.fixed_position.then_some(vertex.source_slot))
        .chain(
            stratified
                .coupled
                .boundary_contracts
                .iter()
                .filter_map(|contract| contract.fixed_position.then_some(contract.source_slot)),
        )
        .collect::<BTreeSet<_>>();
    let interface_sources = stratified
        .traces
        .iter()
        .flat_map(|trace| {
            trace
                .occurrences
                .iter()
                .map(|occurrence| occurrence.source_slot)
        })
        .filter(|source| {
            !remote_guards.contains(source)
                && !original_anchors.contains(source)
                && !fixed_position_sources.contains(source)
        })
        .collect::<BTreeSet<_>>();

    let guard_source_vertices = patch
        .guard_faces
        .iter()
        .flat_map(|&face| trial.global_trial.mesh.mesh.triangles()[face])
        .filter_map(|compact| source_slots[compact])
        .collect::<BTreeSet<_>>();
    assert!(remote_guards
        .intersection(&guard_source_vertices)
        .all(|source| fixed_sources.contains(source)));
    assert!(original_anchors
        .iter()
        .all(|source| !movable_sources.contains(source)));
    assert!(fixed_position_sources
        .iter()
        .all(|source| !movable_sources.contains(source)));
    assert!(fixed_position_sources
        .intersection(&guard_source_vertices)
        .all(|source| fixed_sources.contains(source)));
    assert!(interface_sources
        .difference(&remote_guards)
        .filter(|source| !original_anchors.contains(source))
        .all(|source| movable_sources.contains(source)));
    assert!(fixed_sources.is_disjoint(&movable_sources));
    assert!(patch.guard_faces.iter().all(|&face| {
        trial.global_trial.mesh.mesh.triangles()[face]
            .into_iter()
            .all(|compact| {
                patch.fixed_compact_vertices.contains(&compact)
                    || patch.movable_compact_vertices.contains(&compact)
            })
    }));
}

#[test]
fn free_interface_physical_fixed_sources_are_not_movable() {
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
    let base_patch =
        ElasticPatch::from_full_polygon_merge(&source, &component, &trial, &BTreeSet::new())
            .unwrap();
    let source_slots = &trial.global_trial.mesh.source_vertex_slots;
    let physical = base_patch
        .movable_compact_vertices
        .iter()
        .find_map(|&compact| source_slots[compact])
        .expect("base free-interface patch has movable source vertices");
    let physical_fixed_sources = BTreeSet::from([physical]);
    let patch =
        ElasticPatch::from_full_polygon_merge(&source, &component, &trial, &physical_fixed_sources)
            .unwrap();
    let fixed_sources = patch
        .fixed_compact_vertices
        .iter()
        .filter_map(|&compact| source_slots[compact])
        .collect::<BTreeSet<_>>();
    let movable_sources = patch
        .movable_compact_vertices
        .iter()
        .filter_map(|&compact| source_slots[compact])
        .collect::<BTreeSet<_>>();
    assert!(!movable_sources.contains(&physical));
    if patch.guard_faces.iter().any(|&face| {
        trial.global_trial.mesh.mesh.triangles()[face]
            .into_iter()
            .any(|compact| source_slots[compact] == Some(physical))
    }) {
        assert!(fixed_sources.contains(&physical));
    }
}

#[test]
fn free_interface_cber_does_not_change_topology() {
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
    let patch =
        ElasticPatch::from_full_polygon_merge(&source, &component, &trial, &BTreeSet::new())
            .unwrap();
    let input_mesh = trial.global_trial.mesh.clone();
    let triangles = input_mesh.mesh.triangles().to_vec();
    let neighbours = input_mesh.mesh.neighbours().to_vec();
    match solve_elastic_patch(
        &input_mesh,
        patch,
        ElasticBlockLimits {
            elastic_iterations: 1,
        },
    ) {
        ElasticBlockOutcome::Certified(elastic) => {
            assert_eq!(elastic.mesh.mesh.triangles(), triangles);
            assert_eq!(elastic.mesh.mesh.neighbours(), neighbours);
        }
        ElasticBlockOutcome::ElasticNoImprovement { .. }
        | ElasticBlockOutcome::RequiresDifferentTopology { .. }
        | ElasticBlockOutcome::SearchBudgetExhausted { .. } => {}
        other => panic!("free-interface patch must be valid: {other:?}"),
    }
    assert_eq!(input_mesh.mesh.triangles(), triangles);
    assert_eq!(input_mesh.mesh.neighbours(), neighbours);
}

#[test]
fn free_interface_geometry_failure_continues_exact_search() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge_free_interface_cber(
        &source,
        &component,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states: 64,
            elastic_iterations: 1,
        },
    );
    let FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
        panic!("bounded geometry failures should remain unknown, not no-go: {outcome:?}");
    };
    assert_ne!(evidence.geometry_candidates_attempted, 0);
    assert!(evidence.geometry_candidates_attempted > 1);
    assert!(evidence.topology_candidates_closed >= evidence.geometry_candidates_attempted);
    assert!(evidence.ear_degree_feasible_candidates >= evidence.geometry_candidates_attempted);
}

#[test]
fn all_geometry_failures_are_unknown_not_topology_no_solution() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge_free_interface_cber(
        &source,
        &component,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states: 24,
            elastic_iterations: 1,
        },
    );
    let FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
        panic!("geometry failure exhaustion must remain unknown, not topology no-go: {outcome:?}");
    };
    assert!(evidence.geometry_candidates_attempted > 0);
}

#[test]
fn best_failure_tracks_best_signed_margin_independently_of_last() {
    let better = geometry_failure("better", 41.0, 79.0);
    let worse = geometry_failure("worse", 24.0, 96.0);
    assert!(better.signed_margin_degrees().unwrap() > worse.signed_margin_degrees().unwrap());

    let mut evidence = empty_merge_evidence();
    evidence.record_geometry_failure(better);
    evidence.record_geometry_failure(worse);
    assert_eq!(evidence.last_geometry_failure.unwrap().reason, "worse");
    assert_eq!(evidence.best_geometry_failure.unwrap().reason, "better");
}

#[test]
fn best_failure_tie_break_is_order_independent() {
    let a = geometry_failure_with_counts("a", 38.0, 80.0, Some([0, 0, 2, 0]), vec![1]);
    let b = geometry_failure_with_counts("b", 39.0, 79.0, Some([0, 0, 1, 0]), vec![2]);
    let c = geometry_failure_with_counts("c", 42.0, 78.0, Some([1, 0, 0, 0]), vec![0]);
    let forward = best_failure_reason([a.clone(), b.clone(), c.clone()]);
    let reverse = best_failure_reason([c, b, a]);
    assert_eq!(forward, "b");
    assert_eq!(reverse, "b");
}

#[test]
fn frozen_n6_fixture_exposes_truthful_mixed_source_levels() {
    let (source, _component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    assert_eq!(source_levels.len(), source.mesh.vertices().len());
    assert!(source
        .mesh
        .active_vertex_slots()
        .all(|site| source_levels[site].is_some()));
    let histogram = source
        .mesh
        .active_vertex_slots()
        .map(|site| source_levels[site].unwrap())
        .fold(BTreeMap::<usize, usize>::new(), |mut histogram, level| {
            *histogram.entry(level).or_default() += 1;
            histogram
        });
    assert!(histogram.get(&0).copied().unwrap_or_default() > 0);
    assert!(histogram.get(&1).copied().unwrap_or_default() > 0);
}

#[test]
fn frozen_n6_closed_topology_carries_real_final_cell_levels_but_not_remap_before_geometry() {
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
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
    let source_field = SourceLevelField::from_active_voronoi_cells(
        &source.mesh,
        source
            .mesh
            .active_vertex_slots()
            .map(|site| source_levels[site].unwrap())
            .collect(),
    )
    .unwrap();
    let target_field = TargetLevelField::from_active_voronoi_cells(
        &trial.global_trial.mesh.mesh,
        trial
            .global_trial
            .mesh
            .mesh
            .active_vertex_slots()
            .map(|site| {
                let source = trial.global_trial.mesh.source_vertex_slots[site]
                    .expect("target site must preserve source slot evidence");
                source_levels[source].expect("target source slot must have a level")
            })
            .collect(),
    )
    .unwrap();
    assert!(source_field.levels().contains(&0));
    assert!(source_field.levels().contains(&1));
    assert!(target_field.levels().contains(&0));
    assert!(target_field.levels().contains(&1));
    let error =
        ConservativeRemap::between_voronoi_meshes(&source.mesh, &trial.global_trial.mesh.mesh)
            .unwrap_err();
    assert!(
        error.contains("cannot be remapped") && error.contains("no circumcentre"),
        "unexpected remap blocker: {error}"
    );
}

#[test]
fn frozen_n6_strict_cber_reports_bounded_geometry_blocker() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge_free_interface_cber(
        &source,
        &component,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states: 24,
            elastic_iterations: 8,
        },
    );
    let FullPolygonMergeOutcome::SearchBudgetExhausted(evidence) = outcome else {
        panic!(
            "bounded strict CBER must remain honest unknown until geometry certifies: {outcome:?}"
        );
    };
    let failure = evidence
        .last_geometry_failure
        .as_ref()
        .expect("bounded strict CBER must preserve the last geometry failure");
    assert_eq!(evidence.geometry_candidates_attempted, 3);
    assert_eq!(evidence.topology_candidates_closed, 3);
    assert_eq!(evidence.ear_degree_feasible_candidates, 3);
    assert!(failure.elastic_iterations <= 8);
    assert_eq!(failure.final_phase, ElasticBlockPhase::AngleFeasibility);
    assert!(failure.initial_energy.is_finite());
    assert!(failure.final_energy.is_finite());
    let (global_min, global_max) = failure.global_angle_degrees.unwrap();
    let (guard_min, guard_max) = failure.guard_angle_degrees.unwrap();
    assert!(global_min < 40.2 || global_max > 79.8);
    assert!(guard_min < 40.2 || guard_max > 79.8);
    assert!(!failure.reason.is_empty());
    assert_eq!(
        evidence
            .geometry_failure_phase_counts
            .values()
            .sum::<usize>(),
        evidence.geometry_candidates_attempted
    );
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

#[test]
#[ignore = "explicit finite Frozen N6 geometry probe"]
fn frozen_n6_parameterized_geometry_probe() {
    let topology_limit = usize_env("EARTHMESH_FULL_POLYGON_STATES", 500);
    let elastic_iterations = usize_env("EARTHMESH_CBER_ITERATIONS", 64);
    let starts = geometry_starts(
        std::env::var("EARTHMESH_GEOMETRY_START_SET")
            .ok()
            .as_deref(),
    );
    assert_eq!(starts, vec!["MaterializedSource"]);

    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let outcome = solve_full_polygon_merge_free_interface_cber(
        &source,
        &component,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states: topology_limit,
            elastic_iterations,
        },
    );
    let fixture_fingerprint = earthmesh_refine_certified::mesh_fingerprint(&source.mesh);
    let commit_sha = option_env!("EARTHMESH_GIT_SHA")
        .map(str::to_string)
        .or_else(git_head);
    let json = frozen_n6_geometry_evidence_json(
        &outcome,
        fixture_fingerprint,
        topology_limit,
        elastic_iterations,
        commit_sha.as_deref(),
        &starts,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

fn git_head() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn usize_env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .as_deref()
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name} must be usize"))
        })
        .unwrap_or(default)
}

fn geometry_starts(value: Option<&str>) -> Vec<&'static str> {
    match value.unwrap_or("MaterializedSource") {
        "MaterializedSource" | "materialized_source" | "materialized-source" => {
            vec!["MaterializedSource"]
        }
        other => panic!("unsupported EARTHMESH_GEOMETRY_START_SET={other:?}; PR45 freezes MaterializedSource only"),
    }
}

fn empty_merge_evidence() -> earthmesh_refine_certified::coarsen::FullPolygonMergeEvidence {
    earthmesh_refine_certified::coarsen::FullPolygonMergeEvidence {
        family_id: earthmesh_refine_certified::coarsen::TopologyFamilyId::FullPolygonAnchorEar,
        sector_family_counts: Vec::new(),
        retained_topology_counts: Vec::new(),
        reachability: None,
        states_examined: 0,
        states_by_depth: Vec::new(),
        ear_states_examined: 0,
        topology_candidates_closed: 0,
        ear_degree_feasible_candidates: 0,
        geometry_candidates_attempted: 0,
        last_geometry_failure: None,
        best_geometry_failure: None,
        geometry_failure_phase_counts: BTreeMap::new(),
        selected_topology_keys: Vec::new(),
        selected_ears: Vec::new(),
        best_global_evidence: Default::default(),
    }
}

fn best_failure_reason<const N: usize>(
    failures: [earthmesh_refine_certified::coarsen::FullPolygonGeometryFailureEvidence; N],
) -> String {
    let mut evidence = empty_merge_evidence();
    for failure in failures {
        evidence.record_geometry_failure(failure);
    }
    evidence.best_geometry_failure.unwrap().reason
}

fn geometry_failure(
    reason: &str,
    min_angle: f64,
    max_angle: f64,
) -> earthmesh_refine_certified::coarsen::FullPolygonGeometryFailureEvidence {
    geometry_failure_with_counts(reason, min_angle, max_angle, Some([0, 0, 0, 0]), vec![0])
}

fn geometry_failure_with_counts(
    reason: &str,
    min_angle: f64,
    max_angle: f64,
    counts: Option<[usize; 4]>,
    sectors: Vec<u64>,
) -> earthmesh_refine_certified::coarsen::FullPolygonGeometryFailureEvidence {
    let [negative_orientation_count, crossing_count, delaunay_violations, invalid_voronoi_cells] =
        counts.unwrap_or([0, 0, 0, 0]);
    earthmesh_refine_certified::coarsen::FullPolygonGeometryFailureEvidence {
        topology_keys: sectors
            .into_iter()
            .map(
                |sector_id| earthmesh_refine_certified::coarsen::FullPolygonTopologyKey {
                    sector_id,
                    triangles: vec![[1, 2, 3]],
                },
            )
            .collect(),
        start_id: "MaterializedSource",
        elastic_iterations: 1,
        initial_energy: 2.0,
        final_energy: 1.0,
        final_phase: ElasticBlockPhase::AngleFeasibility,
        reason: reason.into(),
        failed_guard_face: None,
        global_angle_degrees: Some((min_angle, max_angle)),
        guard_angle_degrees: Some((min_angle, max_angle)),
        negative_orientation_count: Some(negative_orientation_count),
        crossing_count: Some(crossing_count),
        delaunay_violations: Some(delaunay_violations),
        invalid_voronoi_cells: Some(invalid_voronoi_cells),
    }
}
