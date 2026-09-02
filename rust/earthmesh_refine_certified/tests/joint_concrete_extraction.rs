use earthmesh_refine_certified::coarsen::{
    annular_topology_signature, build_face_band_problem, build_global_incidence_contract,
    build_stratified_transition_domain_v3, certify_annular_topology, n6_legacy_mixed_fixture,
    solve_exact_face_bands, solve_full_polygon_merge_from_face_bands,
    solve_joint_concrete_extraction, AnnularCellDomain, AnnularIncidenceTarget, EssentialCycleKey,
    FaceBandLimits, FaceBandSolveOutcome, FullPolygonMergeLimits, FullPolygonMergeOutcome,
    GlobalIncidencePlan, GlobalIncidencePlanKey, JointConcreteExtractionOutcome,
    JointConcreteExtractionPlan, JointConcreteLimits, TransitionCellDomain,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn n6_known_topology_is_recovered() {
    let fixture = frozen_n6_joint_fixture();
    let outcome = solve_joint_concrete_extraction(
        &fixture.source,
        &fixture.component,
        &fixture.domain,
        &fixture.plan,
        JointConcreteLimits::default(),
        None,
    );
    let JointConcreteExtractionOutcome::Closed { trial, evidence } = outcome else {
        panic!("Frozen N6 joint target must close: {outcome:?}")
    };
    assert_eq!(evidence.pairs_examined, 1);
    assert!(trial.global_trial.evidence.selected_ears.is_empty());
    assert_eq!(
        closure_tuple(&trial.global_trial.evidence),
        closure_tuple(&fixture.legacy.global_trial.evidence)
    );
    assert_eq!(
        canonical_triangles(&trial.global_trial.custom_triangles),
        canonical_triangles(&fixture.legacy.global_trial.custom_triangles)
    );
}

#[test]
fn pair_checkpoint_resume_equals_one_shot() {
    let fixture = frozen_n6_joint_fixture();
    let one_shot = solve_joint_concrete_extraction(
        &fixture.source,
        &fixture.component,
        &fixture.domain,
        &fixture.plan,
        JointConcreteLimits::default(),
        None,
    );
    let checkpoint = match solve_joint_concrete_extraction(
        &fixture.source,
        &fixture.component,
        &fixture.domain,
        &fixture.plan,
        JointConcreteLimits { maximum_pairs: 0 },
        None,
    ) {
        JointConcreteExtractionOutcome::SearchIncomplete { checkpoint, .. } => checkpoint,
        other => panic!("zero pair budget must checkpoint: {other:?}"),
    };
    let resumed = solve_joint_concrete_extraction(
        &fixture.source,
        &fixture.component,
        &fixture.domain,
        &fixture.plan,
        JointConcreteLimits::default(),
        Some(&checkpoint),
    );
    assert_eq!(one_shot, resumed);
}

#[test]
fn degree_contract_mismatch_is_internal_error() {
    let fixture = frozen_n6_joint_fixture();
    let mut incidence = fixture.plan.incidence_plan.clone();
    *incidence.final_degrees.values_mut().next().unwrap() += 1;
    let plan = JointConcreteExtractionPlan::new(
        incidence,
        fixture.plan.lower_target.clone(),
        fixture.plan.upper_target.clone(),
    );
    assert!(matches!(
        solve_joint_concrete_extraction(
            &fixture.source,
            &fixture.component,
            &fixture.domain,
            &plan,
            JointConcreteLimits::default(),
            None,
        ),
        JointConcreteExtractionOutcome::InvalidInput(reason)
            if reason.contains("degree contract mismatch")
    ));
}

#[test]
fn frozen_n6_joint_oracle_passes() {
    let evidence = include_str!("fixtures/frozen_n6_joint_concrete.json");
    assert!(evidence.contains("\"outcome\":\"Closed\""));
    assert!(evidence.contains("\"selected_ears\":0"));
    assert!(evidence.contains("\"entered_joint_extraction\":true"));
    assert!(evidence.contains("\"gate_passed\":true"));
    assert!(evidence.contains("\"cec_shards_resumed\":false"));
}

#[test]
#[ignore = "write the PR116 Frozen N6 joint concrete oracle artifact"]
fn write_frozen_n6_joint_oracle() {
    let fixture = frozen_n6_joint_fixture();
    let outcome = solve_joint_concrete_extraction(
        &fixture.source,
        &fixture.component,
        &fixture.domain,
        &fixture.plan,
        JointConcreteLimits::default(),
        None,
    );
    let JointConcreteExtractionOutcome::Closed { trial, evidence } = outcome else {
        panic!("Frozen N6 joint oracle must close: {outcome:?}")
    };
    let global = &trial.global_trial.evidence;
    let known = closure_tuple(global) == closure_tuple(&fixture.legacy.global_trial.evidence)
        && canonical_triangles(&trial.global_trial.custom_triangles)
            == canonical_triangles(&fixture.legacy.global_trial.custom_triangles);
    let json = format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473\",\"fixture\":\"FrozenN6\",\"declared_topology_family\":\"TransitionCellV3+GIPC+PIER+JointZeroEar\",\"candidate_pairs\":{},\"pairs_examined\":{},\"dynamic_secondary_targets\":{},\"dynamic_forbidden_edges\":{},\"entered_joint_extraction\":{},\"selected_ears\":{},\"vertices\":{},\"edges\":{},\"faces\":{},\"euler\":{},\"charge\":{},\"anchor_degrees\":{},\"ordinary_degree_histogram\":{},\"known_topology_recovered\":{},\"outcome\":\"Closed\",\"gate_passed\":{},\"cec_shards_resumed\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        evidence.candidate_pairs,
        evidence.pairs_examined,
        evidence.dynamic_secondary_targets,
        evidence.dynamic_forbidden_edges,
        evidence.entered_joint_extraction,
        global.selected_ears.len(),
        global.vertices,
        global.edges,
        global.faces,
        global.euler,
        global.charge,
        map_json(&global.anchor_degrees),
        map_json(&global.ordinary_degree_histogram),
        known,
        known && global.selected_ears.is_empty(),
    );
    if let Ok(path) = std::env::var("EARTHMESH_N6_JOINT_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    println!("{json}");
}

struct FrozenJointFixture {
    source: earthmesh_refine_certified::MotherGrid,
    component: earthmesh_refine_certified::coarsen::HierarchyComponent,
    domain: earthmesh_refine_certified::coarsen::StratifiedTransitionDomainV3,
    legacy: Box<earthmesh_refine_certified::coarsen::FullPolygonMergeTrial>,
    plan: JointConcreteExtractionPlan,
}

fn frozen_n6_joint_fixture() -> FrozenJointFixture {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(face_plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("Frozen N6 W2 plan must close")
    };
    let domain = build_stratified_transition_domain_v3(&source, &component, &face_plan).unwrap();
    let contract = build_global_incidence_contract(&source, &component, &domain).unwrap();
    let FullPolygonMergeOutcome::Closed(legacy) = solve_full_polygon_merge_from_face_bands(
        &source,
        &component,
        &face_plan,
        FullPolygonMergeLimits {
            topology_states: 4_096,
        },
    ) else {
        panic!("Frozen N6 legacy topology must close")
    };
    let known = known_cell_topologies(&domain, &legacy);
    let mut cell_incidences = BTreeMap::new();
    let mut targets = Vec::new();
    for (cell, topology) in &known {
        let signature = annular_topology_signature(cell, &topology.triangles).unwrap();
        let incidences = signature
            .vertex_incidences
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        cell_incidences.insert(cell.cell_id, incidences.clone());
        targets.push(AnnularIncidenceTarget::new(
            cell,
            signature.root_bridge,
            incidences,
        ));
    }
    let final_degrees = contract
        .vertex_domains
        .iter()
        .map(|(&slot, vertex)| {
            let cell_degree = cell_incidences
                .values()
                .filter_map(|incidences| incidences.get(&slot))
                .map(|&incidence| usize::from(incidence))
                .sum::<usize>();
            (
                slot,
                u8::try_from(usize::from(vertex.fixed_degree) + cell_degree).unwrap(),
            )
        })
        .collect();
    let incidence_plan = GlobalIncidencePlan {
        cycle_key: EssentialCycleKey {
            ordered_vertices: Vec::new(),
        },
        final_degrees,
        cell_incidences,
        ordinary_curvature_score: 0,
        incidence_roughness_score: 0,
        plan_key: GlobalIncidencePlanKey("frozen-n6-known-zero-ear".into()),
    };
    let plan =
        JointConcreteExtractionPlan::new(incidence_plan, targets.remove(0), targets.remove(0));
    FrozenJointFixture {
        source,
        component,
        domain,
        legacy,
        plan,
    }
}

fn known_cell_topologies(
    domain: &earthmesh_refine_certified::coarsen::StratifiedTransitionDomainV3,
    legacy: &earthmesh_refine_certified::coarsen::FullPolygonMergeTrial,
) -> Vec<(
    AnnularCellDomain,
    earthmesh_refine_certified::coarsen::AnnularTopology,
)> {
    domain
        .cells
        .iter()
        .enumerate()
        .map(|(cell_index, cell)| {
            let TransitionCellDomain::Annulus(cell) = cell else {
                panic!("Frozen N6 W2 cells must be annular")
            };
            let vertices = cell
                .lower_cycle
                .iter()
                .chain(&cell.upper_cycle)
                .copied()
                .collect::<BTreeSet<_>>();
            let lower = cell.lower_cycle.iter().copied().collect::<BTreeSet<_>>();
            let triangles = legacy
                .global_trial
                .custom_triangles
                .iter()
                .copied()
                .filter(|triangle| triangle.iter().all(|vertex| vertices.contains(vertex)))
                .filter(|triangle| {
                    cell_index == 0 || !triangle.iter().all(|vertex| lower.contains(vertex))
                })
                .collect::<Vec<_>>();
            let topology = certify_annular_topology(
                &cell.lower_cycle,
                &cell.upper_cycle,
                &cell.forbidden_global_edges,
                &triangles,
            )
            .unwrap();
            (cell.clone(), topology)
        })
        .collect()
}

type ClosureTuple = (
    usize,
    usize,
    usize,
    isize,
    isize,
    BTreeMap<usize, usize>,
    BTreeMap<usize, usize>,
);

fn closure_tuple(
    evidence: &earthmesh_refine_certified::coarsen::GlobalExactMergeEvidence,
) -> ClosureTuple {
    (
        evidence.vertices,
        evidence.edges,
        evidence.faces,
        evidence.euler,
        evidence.charge,
        evidence.anchor_degrees.clone(),
        evidence.ordinary_degree_histogram.clone(),
    )
}

fn canonical_triangles(triangles: &[[usize; 3]]) -> BTreeSet<[usize; 3]> {
    triangles
        .iter()
        .copied()
        .map(|mut triangle| {
            triangle.sort_unstable();
            triangle
        })
        .collect()
}

fn map_json(map: &BTreeMap<usize, usize>) -> String {
    format!(
        "{{{}}}",
        map.iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}
