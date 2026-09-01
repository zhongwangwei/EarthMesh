use earthmesh_refine_certified::coarsen::{
    audit_embedding_transfer, build_face_band_problem,
    build_face_band_problem_with_source_face_rings, continue_nested_domain,
    frozen_n6_geometry_evidence_json_with_solver_domain,
    n6_legacy_mixed_fixture_with_source_levels, solve_exact_face_bands,
    solve_full_polygon_merge_free_interface_cber_with_targets_active_trust_starts_and_domain,
    solve_full_polygon_merge_from_face_bands_with_geometry_witness, AnchorBandPolicy,
    DomainContinuationMode, DomainContinuationOutcome, DomainContinuationSchedule,
    ElasticTargetMode, EmbeddingAuditOutcome, FaceBandLimits, FaceBandSolveOutcome,
    FullPolygonCberLimits, FullPolygonMergeEvidence, FullPolygonMergeOutcome, GeometryDomainId,
    GeometryDomainWitness, GeometryFailureWitness, GeometryStartId,
};
use std::{collections::BTreeSet, fs, process::Command};

const TASKBOOK_SHA256: &str = "46d5f8d1ab439ce972186ba50798806b520fe9bdac3f675806d4cd18cff38e2b";
const CLDP_TASKBOOK_SHA256: &str =
    "546a7b60ed8ad94e04f337bfb0c870ef9a2a40ea65545ebe2c256be967a8d722";
const LEGACY_MARGIN_DEG: f64 = -1.653_074_281_139_495_8;
const MATERIAL_IMPROVEMENT_DEG: f64 = 0.25;
const DEFAULT_TOPOLOGY_STATES: usize = 500;
const DEFAULT_ELASTIC_ITERATIONS: usize = 64;
const DEFAULT_W3_TOPOLOGY_STATES: usize = 150;
const STARTS: [GeometryStartId; 6] = [
    GeometryStartId::MaterializedSource,
    GeometryStartId::HierarchySpringEquilibrium,
    GeometryStartId::RingScaleInterpolation,
    GeometryStartId::DegreeAngleEquilibrium,
    GeometryStartId::SignedNormalPlus,
    GeometryStartId::SignedNormalMinus,
];

#[test]
#[ignore = "explicit finite Frozen N6 PR58 inherited PF-W2 geometry gate"]
fn frozen_n6_pr58_pfw2_geometry_probe() {
    let topology_states = usize_env("EARTHMESH_FULL_POLYGON_STATES", DEFAULT_TOPOLOGY_STATES);
    let elastic_iterations = usize_env("EARTHMESH_CBER_ITERATIONS", DEFAULT_ELASTIC_ITERATIONS);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (incumbent, incumbent_range) = pr52_incumbent(&source, &component, &source_levels);
    assert_close(incumbent_range.0, 38.551_143_486_745_16);
    assert_close(incumbent_range.1, 81.453_074_281_139_5);

    let problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("Frozen N6 PF-W2 face-band plan must close")
    };
    let outcome = solve_full_polygon_merge_from_face_bands_with_geometry_witness(
        &source,
        &component,
        &plan,
        &incumbent,
        &BTreeSet::new(),
        FullPolygonCberLimits {
            topology_states,
            elastic_iterations,
        },
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        Some(&source_levels),
        &STARTS,
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    assert!(!matches!(
        outcome,
        FullPolygonMergeOutcome::InvalidInput { .. }
    ));
    let evidence = outcome_evidence(&outcome);
    assert!(evidence.geometry_candidates_attempted > 0);
    let best = evidence.best_geometry_failure.as_ref();
    let best_margin = best.and_then(|failure| failure.signed_margin_degrees());
    let gate = if matches!(outcome, FullPolygonMergeOutcome::Closed(_)) {
        "Certified"
    } else if best_margin
        .is_some_and(|margin| margin >= LEGACY_MARGIN_DEG + MATERIAL_IMPROVEMENT_DEG)
    {
        "MaterialImprovement"
    } else {
        "NoImprovementEnterW3"
    };
    let (common_vertices, fallback_vertices, witness) = best
        .and_then(|failure| failure.witness.as_deref())
        .map(|witness| {
            let inherited = incumbent
                .mesh
                .source_vertex_slots
                .iter()
                .flatten()
                .copied()
                .collect::<BTreeSet<_>>();
            let common = witness
                .mesh
                .source_vertex_slots
                .iter()
                .filter(|slot| slot.is_some_and(|slot| inherited.contains(&slot)))
                .count();
            (
                common,
                witness.mesh.mesh.vertices().len() - common,
                Some(witness),
            )
        })
        .unwrap_or((0, 0, None));
    if let Some(witness) = witness {
        for anchor in [2usize, 29, 77, 155] {
            let compact = witness
                .mesh
                .source_vertex_slots
                .iter()
                .position(|slot| *slot == Some(anchor))
                .expect("original anchor must remain materialized");
            assert!(witness.patch.fixed_compact_vertices.contains(&compact));
        }
    }

    let start_names = STARTS.map(GeometryStartId::as_str);
    let run = frozen_n6_geometry_evidence_json_with_solver_domain(
        &outcome,
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        topology_states,
        elastic_iterations,
        git_head().as_deref(),
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        &start_names,
        "ActiveTangentTrust",
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr58PfW2Geometry\",\"taskbook_sha256\":\"{TASKBOOK_SHA256}\",\"incumbent_angle_degrees\":[{:.12},{:.12}],\"incumbent_signed_margin_deg\":{LEGACY_MARGIN_DEG:.12},\"material_improvement_threshold_deg\":{MATERIAL_IMPROVEMENT_DEG:.12},\"transferred_common_vertices\":{common_vertices},\"safe_fallback_vertices\":{fallback_vertices},\"gate\":\"{gate}\",\"run\":{run}}}",
        incumbent_range.0, incumbent_range.1,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR60 W3 geometry gate"]
fn frozen_n6_pr60_w3_geometry_probe() {
    let topology_states = usize_env("EARTHMESH_FULL_POLYGON_STATES", DEFAULT_W3_TOPOLOGY_STATES);
    let elastic_iterations = usize_env("EARTHMESH_CBER_ITERATIONS", DEFAULT_ELASTIC_ITERATIONS);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (plus_one, plus_two, incumbent_range) =
        pr49_and_pr52_witnesses(&source, &component, &source_levels);
    let mut problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 3, 2).unwrap();
    for anchor in [2usize, 155] {
        problem
            .anchor_policies
            .insert(anchor, AnchorBandPolicy::OnSingleInterface);
    }
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("Frozen N6 W3 face-band plan must close")
    };

    let solve = |domain_id, witness: &GeometryFailureWitness| {
        solve_full_polygon_merge_from_face_bands_with_geometry_witness(
            &source,
            &component,
            &plan,
            witness,
            &BTreeSet::new(),
            FullPolygonCberLimits {
                topology_states,
                elastic_iterations,
            },
            ElasticTargetMode::HierarchyEdgeAreaDegree,
            Some(&source_levels),
            &STARTS,
            domain_id,
        )
    };
    let plus_one_outcome = solve(GeometryDomainId::PlusOneOrdinaryRing, &plus_one);
    let plus_two_outcome = solve(GeometryDomainId::PlusTwoOrdinaryRings, &plus_two);
    for outcome in [&plus_one_outcome, &plus_two_outcome] {
        assert!(!matches!(
            outcome,
            FullPolygonMergeOutcome::InvalidInput { .. }
        ));
        assert!(outcome_evidence(outcome).geometry_candidates_attempted > 0);
    }
    let start_names = STARTS.map(GeometryStartId::as_str);
    let plus_one_run = frozen_n6_geometry_evidence_json_with_solver_domain(
        &plus_one_outcome,
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        topology_states,
        elastic_iterations,
        None,
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        &start_names,
        "ActiveTangentTrust",
        GeometryDomainId::PlusOneOrdinaryRing,
    );
    let plus_two_run = frozen_n6_geometry_evidence_json_with_solver_domain(
        &plus_two_outcome,
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        topology_states,
        elastic_iterations,
        None,
        ElasticTargetMode::HierarchyEdgeAreaDegree,
        &start_names,
        "ActiveTangentTrust",
        GeometryDomainId::PlusTwoOrdinaryRings,
    );
    let gate = if matches!(plus_one_outcome, FullPolygonMergeOutcome::Closed(_))
        || matches!(plus_two_outcome, FullPolygonMergeOutcome::Closed(_))
    {
        "Certified"
    } else {
        "W3GeometryNotCertified"
    };
    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr60W3Geometry\",\"taskbook_sha256\":\"{TASKBOOK_SHA256}\",\"target_trace_scales\":\"H*2^(-k/3), k=0..3\",\"incumbent_angle_degrees\":[{:.12},{:.12}],\"gate\":\"{gate}\",\"plus_one\":{plus_one_run},\"plus_two\":{plus_two_run}}}",
        incumbent_range.0, incumbent_range.1,
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

#[test]
#[ignore = "explicit finite Frozen N6 PR61 W3 embedding audit"]
fn frozen_n6_pr61_w3_embedding_audit_probe() {
    let topology_states = usize_env("EARTHMESH_FULL_POLYGON_STATES", DEFAULT_W3_TOPOLOGY_STATES);
    let (source, component, source_levels) = n6_legacy_mixed_fixture_with_source_levels().unwrap();
    let (plus_one, plus_two, _) = pr49_and_pr52_witnesses(&source, &component, &source_levels);
    let mut problem =
        build_face_band_problem_with_source_face_rings(&source, &component, 3, 2).unwrap();
    for anchor in [2usize, 155] {
        problem
            .anchor_policies
            .insert(anchor, AnchorBandPolicy::OnSingleInterface);
    }
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 1_000_000,
        },
    ) else {
        panic!("Frozen N6 W3 face-band plan must close")
    };

    let solve = |domain_id, witness: &GeometryFailureWitness| {
        solve_full_polygon_merge_from_face_bands_with_geometry_witness(
            &source,
            &component,
            &plan,
            witness,
            &BTreeSet::new(),
            FullPolygonCberLimits {
                topology_states,
                elastic_iterations: 0,
            },
            ElasticTargetMode::HierarchyEdgeAreaDegree,
            Some(&source_levels),
            &STARTS,
            domain_id,
        )
    };
    let plus_one_outcome = solve(GeometryDomainId::PlusOneOrdinaryRing, &plus_one);
    let plus_two_outcome = solve(GeometryDomainId::PlusTwoOrdinaryRings, &plus_two);
    let plus_one_audit = embedding_audit(&plus_one, &plus_one_outcome);
    let plus_two_audit = embedding_audit(&plus_two, &plus_two_outcome);
    for audit in [&plus_one_audit, &plus_two_audit] {
        assert_eq!(
            audit.outcome(),
            EmbeddingAuditOutcome::TopologyClosedNoUsableEmbedding
        );
        assert!(
            audit.non_positive_triangles > 0
                || audit.crossing_pairs > 0
                || audit.near_degenerate_triangles > 0
                || !audit.rotation_mismatch_vertices.is_empty(),
            "W3 failure must be attributed to its transferred embedding"
        );
    }

    let json = format!(
        "{{\"schema_version\":1,\"probe\":\"FrozenN6Pr61W3EmbeddingAudit\",\"taskbook_sha256\":\"{CLDP_TASKBOOK_SHA256}\",\"outcome\":\"TopologyClosedNoUsableEmbedding\",\"w4_started\":false,\"plus_one\":{},\"plus_two\":{}}}",
        embedding_audit_json(&plus_one_audit),
        embedding_audit_json(&plus_two_audit),
    );
    if let Ok(path) = std::env::var("EARTHMESH_GEOMETRY_JSON") {
        fs::write(path, &json).unwrap();
    }
    eprintln!("{json}");
}

fn embedding_audit(
    reference: &GeometryFailureWitness,
    outcome: &FullPolygonMergeOutcome,
) -> earthmesh_refine_certified::coarsen::EmbeddingAudit {
    let failure = outcome_evidence(outcome)
        .best_geometry_failure
        .as_ref()
        .expect("W3 embedding audit requires a geometry candidate");
    let candidate = failure
        .witness
        .as_deref()
        .expect("W3 embedding audit requires candidate coordinates");
    let topology_key = failure
        .topology_keys
        .first()
        .cloned()
        .expect("W3 geometry candidate must identify its topology");
    audit_embedding_transfer(
        topology_key,
        &reference.mesh,
        &candidate.mesh,
        &candidate.patch.fixed_compact_vertices,
    )
    .unwrap()
}

fn embedding_audit_json(audit: &earthmesh_refine_certified::coarsen::EmbeddingAudit) -> String {
    format!(
        "{{\"common_source_vertices\":{},\"added_source_vertices\":{:?},\"removed_source_vertices\":{:?},\"common_edges\":{},\"added_edges\":{},\"removed_edges\":{},\"common_triangles\":{},\"added_triangles\":{},\"removed_triangles\":{},\"non_positive_triangles\":{},\"crossing_pairs\":{},\"near_degenerate_triangles\":{},\"fixed_only_degenerate_triangles\":{},\"rotation_mismatch_vertices\":{}}}",
        audit.common_source_vertices,
        audit.added_source_vertices,
        audit.removed_source_vertices,
        audit.common_edges,
        audit.added_edges,
        audit.removed_edges,
        audit.common_triangles,
        audit.added_triangles,
        audit.removed_triangles,
        audit.non_positive_triangles,
        audit.crossing_pairs,
        audit.near_degenerate_triangles,
        audit.fixed_only_degenerate_triangles,
        audit.rotation_mismatch_vertices.len(),
    )
}

fn pr52_incumbent(
    source: &earthmesh_refine_certified::MotherGrid,
    component: &earthmesh_refine_certified::coarsen::HierarchyComponent,
    source_levels: &[Option<usize>],
) -> (GeometryFailureWitness, (f64, f64)) {
    let (_, plus_two, range) = pr49_and_pr52_witnesses(source, component, source_levels);
    (plus_two, range)
}

fn pr49_and_pr52_witnesses(
    source: &earthmesh_refine_certified::MotherGrid,
    component: &earthmesh_refine_certified::coarsen::HierarchyComponent,
    source_levels: &[Option<usize>],
) -> (GeometryFailureWitness, GeometryFailureWitness, (f64, f64)) {
    let outcome =
        solve_full_polygon_merge_free_interface_cber_with_targets_active_trust_starts_and_domain(
            source,
            component,
            &BTreeSet::new(),
            FullPolygonCberLimits {
                topology_states: DEFAULT_TOPOLOGY_STATES,
                elastic_iterations: DEFAULT_ELASTIC_ITERATIONS,
            },
            ElasticTargetMode::HierarchyEdgeAreaDegree,
            Some(source_levels),
            &[GeometryStartId::MaterializedSource],
            GeometryDomainId::PlusOneOrdinaryRing,
        );
    let failure = outcome_evidence(&outcome)
        .best_geometry_failure
        .as_ref()
        .expect("PR52 reconstruction requires the PR49 +1 witness");
    let witness = failure
        .witness
        .as_deref()
        .expect("PR52 reconstruction requires coordinates");
    let plus_one = witness.clone();
    let inherited = GeometryDomainWitness::from_failure(
        earthmesh_refine_certified::mesh_fingerprint(&source.mesh),
        failure.topology_keys.clone(),
        GeometryDomainId::PlusOneOrdinaryRing,
        witness,
        failure.global_angle_degrees.unwrap(),
        failure.guard_angle_degrees.unwrap(),
    )
    .unwrap();
    let target_patch = inherited
        .expanded_patch(
            source,
            source_levels,
            &BTreeSet::new(),
            GeometryDomainId::PlusTwoOrdinaryRings,
        )
        .unwrap();
    let DomainContinuationOutcome::Completed(continuation) = continue_nested_domain(
        &inherited,
        target_patch,
        DomainContinuationSchedule::frozen_n6(),
        DomainContinuationMode::InheritedBestMonotone,
    ) else {
        panic!("PR52 continuation must complete")
    };
    (
        plus_one,
        continuation.best_witness.as_ref().clone(),
        continuation.best_angle_range_deg,
    )
}

fn outcome_evidence(outcome: &FullPolygonMergeOutcome) -> &FullPolygonMergeEvidence {
    match outcome {
        FullPolygonMergeOutcome::Closed(trial) => &trial.evidence,
        FullPolygonMergeOutcome::TopologyFamilyExhaustedNoSolution(evidence)
        | FullPolygonMergeOutcome::SearchBudgetExhausted(evidence)
        | FullPolygonMergeOutcome::InvalidInput { evidence, .. } => evidence,
    }
}

fn usize_env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
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

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-10,
        "{actual} != {expected}"
    );
}
