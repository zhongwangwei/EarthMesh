use earthmesh_refine_certified::coarsen::{
    build_essential_cycle_problem, build_face_band_problem, find_one_sdce_essential_cycle,
    n12_lifted_n6_fixture, RetainedCoreCorridorFamily, SdceCycleFindOneLimits,
    SdceCycleFindOneOutcome, SdcePlanQuantum,
};
use std::{collections::BTreeMap, fs};

const LIMITS: SdceCycleFindOneLimits = SdceCycleFindOneLimits {
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

#[test]
fn frozen_lifted_prefix_closes_without_ears() {
    let evidence = include_str!("fixtures/n12_sdce_find_one.json");
    assert!(evidence.contains("\"essential_cycles\":6838"));
    assert!(evidence.contains("\"cycles_screened\":6838"));
    assert!(evidence.contains("\"topology_closed\":true"));
    assert!(evidence.contains("\"selected_ears\":0"));
    assert!(evidence.contains("\"euler\":2"));
    assert!(evidence.contains("\"charge\":12"));
    assert!(evidence.contains(
        "\"critical_final_degrees\":{\"48\":5,\"52\":7,\"78\":7,\"252\":5,\"256\":7,\"343\":7}"
    ));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
    assert!(evidence.contains("\"geometry_attempted\":false"));
}

#[test]
fn zero_finalists_is_invalid() {
    let (fixture, face_problem, cycle_problem) = problem();
    assert!(matches!(
        find_one_sdce_essential_cycle(
            &fixture.source,
            &fixture.component,
            &face_problem,
            &cycle_problem,
            SdceCycleFindOneLimits {
                finalists: 0,
                ..LIMITS
            },
        ),
        SdceCycleFindOneOutcome::InvalidInput(_)
    ));
}

#[test]
#[ignore = "PR117 bounded Lifted-N12 SDCE fixture writer"]
fn write_lifted_prefix_closure() {
    let (fixture, face_problem, cycle_problem) = problem();
    let SdceCycleFindOneOutcome::Closed { trial, evidence } = find_one_sdce_essential_cycle(
        &fixture.source,
        &fixture.component,
        &face_problem,
        &cycle_problem,
        LIMITS,
    ) else {
        panic!("the frozen PR117 budget must close")
    };
    let cec = evidence.cec.as_ref().unwrap();
    let joint = evidence.joint.as_ref().unwrap();
    let global = &trial.global_trial.evidence;
    assert_eq!(cec.essential_cycles, 6_838);
    assert_eq!(evidence.cycles_screened, 6_838);
    assert_eq!(global.euler, 2);
    assert_eq!(global.charge, 12);
    assert!(global.selected_ears.is_empty());
    assert!(evidence
        .critical_final_degrees
        .values()
        .all(|degree| (5..=7).contains(degree)));
    let roots = evidence.selected_roots.unwrap();
    let json = format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"{}\",\"fixture\":\"N12-Lifted-N6\",\"declared_topology_family\":\"W2CanonicalEssentialCycle+TransitionCellV3+ContractDistanceFlip+PIER+JointZeroEar\",\"limits\":{{\"cycle_unique_states\":{},\"finalists\":{},\"screening_topologies_per_cell\":{},\"screening_beam_width\":{},\"screening_flip_depth\":{},\"finalist_topologies_per_cell\":{},\"finalist_beam_width\":{},\"finalist_flip_depth\":{},\"joint_pairs\":{}}},\"unique_states\":{},\"essential_cycles\":{},\"cycles_screened\":{},\"screening_pairs_scored\":{},\"best_screening_distance\":{},\"finalists_selected\":{},\"finalists_examined\":{},\"finalist_best_distances\":{:?},\"closed_cycle_key\":\"{:016x}\",\"closed_plan_key\":\"{}\",\"closed_flip_depth\":{},\"closed_initial_family_counts\":{:?},\"closed_pairs_scored\":{},\"selected_roots\":[[{},{}],[{},{}]],\"lower_witnesses\":{},\"upper_witnesses\":{},\"dynamic_secondary_targets\":{},\"dynamic_forbidden_edges\":{},\"candidate_pairs\":{},\"pairs_examined\":{},\"selected_ears\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"anchor_degrees\":{},\"ordinary_degree_histogram\":{},\"critical_final_degrees\":{},\"topology_closed\":true,\"cec_shards_resumed\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        taskbook_sha256(),
        LIMITS.maximum_cycle_states,
        LIMITS.finalists,
        LIMITS.screening_quantum.balanced_topologies_per_cell,
        LIMITS.screening_quantum.beam_width,
        LIMITS.screening_quantum.maximum_flip_depth,
        LIMITS.finalist_quantum.balanced_topologies_per_cell,
        LIMITS.finalist_quantum.beam_width,
        LIMITS.finalist_quantum.maximum_flip_depth,
        LIMITS.finalist_quantum.maximum_joint_pairs,
        cec.unique_states,
        cec.essential_cycles,
        evidence.cycles_screened,
        evidence.screening_pairs_scored,
        evidence.best_screening_distance.unwrap(),
        evidence.finalists_selected,
        evidence.finalists_examined,
        evidence.finalist_best_distances,
        fnv1a(format!("{:?}", evidence.closed_cycle.as_ref().unwrap()).bytes()),
        evidence.closed_plan.as_ref().unwrap().0,
        evidence.closed_flip_depth.unwrap(),
        evidence.closed_initial_family_counts,
        evidence.closed_pairs_scored,
        roots[0].0,
        roots[0].1,
        roots[1].0,
        roots[1].1,
        joint.lower_witnesses,
        joint.upper_witnesses,
        joint.dynamic_secondary_targets,
        joint.dynamic_forbidden_edges,
        joint.candidate_pairs,
        joint.pairs_examined,
        global.selected_ears.len(),
        global.vertices,
        global.edges,
        global.faces,
        global.euler,
        global.charge,
        json_map(&global.anchor_degrees),
        json_map(&global.ordinary_degree_histogram),
        json_map(&evidence.critical_final_degrees),
    );
    println!("{json}");
    if let Ok(path) = std::env::var("EARTHMESH_N12_SDCE_FIND_ONE_JSON") {
        fs::write(path, &json).unwrap();
    }
}

fn problem() -> (
    earthmesh_refine_certified::coarsen::CertifiedResearchFixture,
    earthmesh_refine_certified::coarsen::FaceBandProblem,
    earthmesh_refine_certified::coarsen::EssentialCycleProblem,
) {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let face_problem = build_face_band_problem(&fixture.source, &fixture.component, 2).unwrap();
    let cycle_problem = build_essential_cycle_problem(
        &fixture.source,
        &face_problem,
        fixture.component.core_parents.iter().copied(),
        RetainedCoreCorridorFamily::F0CurrentSourceFaceCorridor,
    )
    .unwrap();
    (fixture, face_problem, cycle_problem)
}

fn json_map<T: std::fmt::Display>(map: &BTreeMap<usize, T>) -> String {
    format!(
        "{{{}}}",
        map.iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn taskbook_sha256() -> &'static str {
    "65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473"
}
