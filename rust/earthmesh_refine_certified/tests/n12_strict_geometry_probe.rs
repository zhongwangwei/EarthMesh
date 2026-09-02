use earthmesh_refine_certified::coarsen::{
    build_essential_cycle_problem, build_face_band_problem, find_one_sdce_essential_cycle,
    n12_lifted_n6_fixture, solve_elastic_patch_with_max_min_trust_start, ElasticBlockLimits,
    ElasticBlockOutcome, ElasticBlockPhase, ElasticPatch, ElasticTargetMode, GeometryDomainId,
    GeometryFailureWitness, GeometryStartId, RetainedCoreCorridorFamily, SdceCycleFindOneLimits,
    SdceCycleFindOneOutcome, SdcePlanQuantum,
};
use std::{collections::BTreeSet, fs};

const TASKBOOK_SHA256: &str = "65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473";
const TOPOLOGY_LIMITS: SdceCycleFindOneLimits = SdceCycleFindOneLimits {
    maximum_cycle_states: 16_384,
    finalists: 8,
    screening_quantum: SdcePlanQuantum {
        balanced_topologies_per_cell: 8,
        beam_width: 1,
        maximum_flip_depth: 0,
        maximum_joint_pairs: 0,
    },
    finalist_quantum: SdcePlanQuantum {
        balanced_topologies_per_cell: 64,
        beam_width: 32,
        maximum_flip_depth: 64,
        maximum_joint_pairs: 1,
    },
};
const DOMAINS: [GeometryDomainId; 3] = [
    GeometryDomainId::CurrentAnnulus,
    GeometryDomainId::PlusOneOrdinaryRing,
    GeometryDomainId::PlusTwoOrdinaryRings,
];
const STARTS: [GeometryStartId; 6] = [
    GeometryStartId::MaterializedSource,
    GeometryStartId::HierarchySpringEquilibrium,
    GeometryStartId::RingScaleInterpolation,
    GeometryStartId::DegreeAngleEquilibrium,
    GeometryStartId::SignedNormalPlus,
    GeometryStartId::SignedNormalMinus,
];

#[test]
fn frozen_pr118_probe_runs_after_sdce_closure() {
    let topology = include_str!("fixtures/n12_sdce_find_one.json");
    let geometry = include_str!("fixtures/n12_strict_geometry_probe.json");

    assert!(topology.contains("\"topology_closed\":true"));
    assert!(geometry.contains("\"topology_source\":\"PR117_SDCE_FindOne\""));
    assert!(geometry.contains("\"geometry_attempted\":true"));
    assert!(geometry.contains("\"target_mode\":\"HierarchyEdgeAreaDegree\""));
    assert!(geometry.contains("\"solver\":\"MaxMinTangentTrust\""));
    assert!(geometry.contains("\"strict_angle_degrees\":[40.2,79.8]"));
    assert_eq!(geometry.matches("\"phase\":\"Untangle\"").count(), 21);
    assert!(geometry.contains("\"best_angle_range\":[1.337009876734,173.470265136292]"));
    assert!(geometry.contains("\"strict_certified\":false"));
    assert!(geometry.contains("\"n24_n40_nxp80_unlocked\":false"));
    assert!(geometry.contains("\"product_grid_written\":false"));
    assert!(geometry.contains("\"ready_marker_written\":false"));
    assert!(geometry.contains("\"product_gate_changed\":false"));
}

#[test]
#[ignore = "PR118 deterministic N12 strict-geometry probe"]
fn write_n12_strict_geometry_probe() {
    let screening_iterations = std::env::var("EARTHMESH_CBER_SCREEN_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let deepening_iterations = std::env::var("EARTHMESH_CBER_DEEP_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let fixture = n12_lifted_n6_fixture().unwrap();
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2).unwrap();
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    let SdceCycleFindOneOutcome::Closed { trial, .. } = find_one_sdce_essential_cycle(
        &fixture.source,
        &fixture.component,
        &face_problem,
        &cycle_problem,
        TOPOLOGY_LIMITS,
    ) else {
        panic!("PR117 frozen topology must close before geometry")
    };

    let mut attempts = Vec::new();
    let mut incumbent = None::<GeometryFailureWitness>;
    let mut best_range = None::<(f64, f64)>;
    let mut best_margin = f64::NEG_INFINITY;
    let mut strict_certified = false;
    for domain in DOMAINS {
        let patch = ElasticPatch::from_global_exact_merge_with_domain(
            &fixture.source,
            &fixture.component,
            &trial.global_trial,
            &BTreeSet::new(),
            domain,
        )
        .and_then(|patch| {
            patch.with_hierarchy_targets(
                &fixture.source,
                &trial.global_trial.mesh,
                &fixture.source_levels,
                ElasticTargetMode::HierarchyEdgeAreaDegree,
            )
        })
        .unwrap();

        if let Some(previous) = incumbent.as_ref() {
            record_attempt(
                domain,
                "InheritedLiftedGeometryIfAvailable",
                solve_elastic_patch_with_max_min_trust_start(
                    &previous.mesh,
                    patch.clone(),
                    ElasticBlockLimits {
                        elastic_iterations: screening_iterations,
                    },
                    GeometryStartId::MaterializedSource,
                ),
                &mut attempts,
                &mut incumbent,
                &mut best_range,
                &mut best_margin,
                &mut strict_certified,
            );
        }
        for start in STARTS {
            record_attempt(
                domain,
                start.as_str(),
                solve_elastic_patch_with_max_min_trust_start(
                    &trial.global_trial.mesh,
                    patch.clone(),
                    ElasticBlockLimits {
                        elastic_iterations: screening_iterations,
                    },
                    start,
                ),
                &mut attempts,
                &mut incumbent,
                &mut best_range,
                &mut best_margin,
                &mut strict_certified,
            );
        }
    }

    if !strict_certified {
        let previous = incumbent
            .clone()
            .expect("geometry screen produced no witness");
        let patch = ElasticPatch::from_global_exact_merge_with_domain(
            &fixture.source,
            &fixture.component,
            &trial.global_trial,
            &BTreeSet::new(),
            GeometryDomainId::PlusTwoOrdinaryRings,
        )
        .and_then(|patch| {
            patch.with_hierarchy_targets(
                &fixture.source,
                &trial.global_trial.mesh,
                &fixture.source_levels,
                ElasticTargetMode::HierarchyEdgeAreaDegree,
            )
        })
        .unwrap();
        record_attempt(
            GeometryDomainId::PlusTwoOrdinaryRings,
            "DeepenBestIncumbent",
            solve_elastic_patch_with_max_min_trust_start(
                &previous.mesh,
                patch,
                ElasticBlockLimits {
                    elastic_iterations: deepening_iterations,
                },
                GeometryStartId::MaterializedSource,
            ),
            &mut attempts,
            &mut incumbent,
            &mut best_range,
            &mut best_margin,
            &mut strict_certified,
        );
    }

    let best_range_json = best_range
        .map(|range| format!("[{:.12},{:.12}]", range.0, range.1))
        .unwrap_or_else(|| "null".into());
    let best_margin_json = best_range
        .map(|_| format!("{best_margin:.12}"))
        .unwrap_or_else(|| "null".into());
    let json = format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"{TASKBOOK_SHA256}\",\"fixture\":\"N12-Lifted-N6\",\"topology_source\":\"PR117_SDCE_FindOne\",\"topology_closed\":true,\"target_mode\":\"HierarchyEdgeAreaDegree\",\"solver\":\"MaxMinTangentTrust\",\"strict_angle_degrees\":[40.2,79.8],\"screening_iterations_per_attempt\":{screening_iterations},\"deepening_iterations\":{deepening_iterations},\"domains\":[\"CurrentAnnulus\",\"PlusOneOrdinaryRing\",\"PlusTwoOrdinaryRings\"],\"starts\":[\"MaterializedSource\",\"InheritedLiftedGeometryIfAvailable\",\"HierarchySpringEquilibrium\",\"RingScaleInterpolation\",\"DegreeAngleEquilibrium\",\"SignedNormalPlus\",\"SignedNormalMinus\"],\"incumbent_preserving_continuation\":true,\"attempts\":[{}],\"best_angle_range\":{best_range_json},\"best_signed_margin_deg\":{best_margin_json},\"strict_certified\":{strict_certified},\"delaunay_voronoi_checked\":{strict_certified},\"physical_balance_remap_checked\":false,\"geometry_attempted\":true,\"cec_shards_resumed\":false,\"n24_n40_nxp80_unlocked\":false,\"product_grid_written\":false,\"ready_marker_written\":false,\"product_gate_changed\":false}}",
        attempts.join(","),
    );
    println!("{json}");
    if let Ok(path) = std::env::var("EARTHMESH_N12_STRICT_GEOMETRY_JSON") {
        fs::write(path, json).unwrap();
    }
}

#[allow(clippy::too_many_arguments)]
fn record_attempt(
    domain: GeometryDomainId,
    start: &str,
    outcome: ElasticBlockOutcome,
    attempts: &mut Vec<String>,
    incumbent: &mut Option<GeometryFailureWitness>,
    best_range: &mut Option<(f64, f64)>,
    best_margin: &mut f64,
    strict_certified: &mut bool,
) {
    let (kind, iterations, phase, range, certified, witness) = match outcome {
        ElasticBlockOutcome::Certified(trial) => {
            let range = (
                trial.geometry.min_angle_degrees,
                trial.geometry.max_angle_degrees,
            );
            let iterations = trial.report.elastic_iterations;
            (
                "Certified",
                iterations,
                Some(ElasticBlockPhase::Interior),
                Some(range),
                true,
                Some(GeometryFailureWitness {
                    mesh: trial.mesh,
                    patch: trial.patch,
                }),
            )
        }
        ElasticBlockOutcome::ElasticNoImprovement {
            elastic_iterations,
            final_phase,
            global_angle_degrees,
            witness,
            ..
        } => (
            "ElasticNoImprovement",
            elastic_iterations,
            Some(final_phase),
            global_angle_degrees,
            false,
            Some(*witness),
        ),
        ElasticBlockOutcome::SearchBudgetExhausted {
            elastic_iterations,
            final_phase,
            global_angle_degrees,
            witness,
            ..
        } => (
            "SearchBudgetExhausted",
            elastic_iterations,
            Some(final_phase),
            global_angle_degrees,
            false,
            Some(*witness),
        ),
        ElasticBlockOutcome::RequiresDifferentTopology {
            elastic_iterations,
            final_phase,
            global_angle_degrees,
            witness,
            ..
        } => (
            "RequiresDifferentTopology",
            elastic_iterations,
            Some(final_phase),
            global_angle_degrees,
            false,
            Some(*witness),
        ),
        ElasticBlockOutcome::InvalidPatch { .. } => ("InvalidPatch", 0, None, None, false, None),
    };
    let margin = range.map(|(min, max)| (min - 40.2).min(79.8 - max));
    if let (Some(range), Some(margin), Some(witness)) = (range, margin, witness) {
        if margin > *best_margin {
            *best_margin = margin;
            *best_range = Some(range);
            *incumbent = Some(witness);
        }
    }
    *strict_certified |= certified;
    let range_json = range
        .map(|range| format!("[{:.12},{:.12}]", range.0, range.1))
        .unwrap_or_else(|| "null".into());
    let margin_json = margin
        .map(|margin| format!("{margin:.12}"))
        .unwrap_or_else(|| "null".into());
    let phase_json = phase
        .map(|phase| format!("\"{}\"", phase_name(phase)))
        .unwrap_or_else(|| "null".into());
    attempts.push(format!(
        "{{\"domain\":\"{}\",\"start\":\"{start}\",\"iterations\":{iterations},\"outcome\":\"{kind}\",\"phase\":{phase_json},\"angle_range\":{range_json},\"signed_margin_deg\":{margin_json},\"strict_certified\":{certified}}}",
        domain.as_str(),
    ));
}

fn phase_name(phase: ElasticBlockPhase) -> &'static str {
    match phase {
        ElasticBlockPhase::Untangle => "Untangle",
        ElasticBlockPhase::AngleFeasibility => "AngleFeasibility",
        ElasticBlockPhase::DelaunayVoronoiFeasibility => "DelaunayVoronoiFeasibility",
        ElasticBlockPhase::Interior => "Interior",
    }
}
