use earthmesh_refine_certified::coarsen::{
    annular_topology_signature, build_face_band_problem, build_stratified_transition_domain_v3,
    certify_annular_topology, enumerate_annular_incidence_targets, n6_legacy_mixed_fixture,
    recover_annular_target_witnesses, solve_exact_face_bands,
    solve_full_polygon_merge_from_face_bands, AnnularCellDomain, AnnularCellKey,
    AnnularIncidenceTarget, AnnularTargetWitnessOutcome, FaceBandLimits, FaceBandSolveOutcome,
    FullPolygonMergeLimits, FullPolygonMergeOutcome, TopologyBoundaryKind, TransitionCellDomain,
};
use std::collections::{BTreeMap, BTreeSet};

fn synthetic_cell() -> AnnularCellDomain {
    AnnularCellDomain {
        cell_id: 7,
        lower_cycle: vec![0, 1, 2],
        upper_cycle: vec![100, 101, 102],
        lower_boundary_kind: TopologyBoundaryKind::SourceCycle,
        upper_boundary_kind: TopologyBoundaryKind::SourceCycle,
        forbidden_global_edges: BTreeSet::new(),
        fixed_outside_link_contracts: BTreeMap::new(),
        cell_key: AnnularCellKey("synthetic-3+3".into()),
    }
}

fn known_triangles() -> Vec<[usize; 3]> {
    vec![
        [0, 100, 102],
        [0, 101, 102],
        [0, 2, 101],
        [2, 100, 101],
        [1, 2, 100],
        [0, 1, 100],
    ]
}

#[test]
fn exact_target_recovers_known_topology() {
    let cell = synthetic_cell();
    let topology = certify_annular_topology(
        &cell.lower_cycle,
        &cell.upper_cycle,
        &cell.forbidden_global_edges,
        &known_triangles(),
    )
    .unwrap();
    let signature = annular_topology_signature(&cell, &topology.triangles).unwrap();
    let target = AnnularIncidenceTarget::new(
        &cell,
        signature.root_bridge,
        signature.vertex_incidences.into_iter().collect(),
    );
    let AnnularTargetWitnessOutcome::Found { witnesses, .. } =
        recover_annular_target_witnesses(&cell, &target)
    else {
        panic!("known target must be recoverable")
    };
    assert!(witnesses
        .iter()
        .any(|witness| witness.topology_key == topology.topology_key));
    assert!(witnesses.iter().all(|witness| {
        witness.exact_signature.root_bridge == target.root_bridge
            && witness.exact_signature.vertex_incidences
                == target
                    .global_vertex_incidences
                    .iter()
                    .map(|(&slot, &incidence)| (slot, incidence))
                    .collect::<Vec<_>>()
            && witness.topology_key == witness.topology.topology_key
            && !witness.interior_edges.is_empty()
    }));
}

#[test]
fn root_targets_cover_the_exact_cross_product() {
    let cell = synthetic_cell();
    let signature = annular_topology_signature(&cell, &known_triangles()).unwrap();
    let targets = enumerate_annular_incidence_targets(
        &cell,
        &signature.vertex_incidences.into_iter().collect(),
    )
    .unwrap();
    assert_eq!(targets.len(), 9);
    assert!(targets
        .windows(2)
        .all(|pair| pair[0].root_bridge < pair[1].root_bridge));
}

#[test]
fn target_key_contains_exact_forbidden_edges() {
    let cell = synthetic_cell();
    let signature = annular_topology_signature(&cell, &known_triangles()).unwrap();
    let incidences = signature.vertex_incidences.into_iter().collect();
    let first = AnnularIncidenceTarget::new(&cell, signature.root_bridge, incidences);
    let mut changed = cell.clone();
    changed.forbidden_global_edges.insert((0, 101));
    let second = AnnularIncidenceTarget::new(
        &changed,
        signature.root_bridge,
        first.global_vertex_incidences.clone(),
    );
    assert_ne!(first.target_key, second.target_key);
}

#[test]
fn frozen_n6_known_cells_are_recovered() {
    let evidence = include_str!("fixtures/frozen_n6_annular_target_witness.json");
    assert!(evidence.contains("\"cells\":2"));
    assert!(evidence.contains("\"known_topologies_recovered\":2"));
    assert!(evidence.contains("\"gate_passed\":true"));
}

#[test]
#[ignore = "write the PR115 Frozen N6 annular-target oracle artifact"]
fn write_frozen_n6_annular_target_witness() {
    let cells = frozen_n6_known_cells();
    let mut fragments = Vec::new();
    let mut recovered = 0;
    for (cell, topology) in cells {
        let signature = annular_topology_signature(&cell, &topology.triangles).unwrap();
        let target = AnnularIncidenceTarget::new(
            &cell,
            signature.root_bridge,
            signature.vertex_incidences.iter().copied().collect(),
        );
        let outcome = recover_annular_target_witnesses(&cell, &target);
        let AnnularTargetWitnessOutcome::Found {
            witnesses,
            evidence,
        } = outcome
        else {
            panic!("Frozen N6 known cell target must recover: {outcome:?}")
        };
        let contains_known = witnesses
            .iter()
            .any(|witness| witness.topology_key == topology.topology_key);
        recovered += usize::from(contains_known);
        fragments.push(format!(
            "{{\"cell_id\":{},\"lower_vertices\":{},\"upper_vertices\":{},\"root_bridge\":[{},{}],\"root_splits\":{},\"pier_states\":{},\"occurrence_witnesses\":{},\"topologies_found\":{},\"known_topology_recovered\":{}}}",
            cell.cell_id,
            cell.lower_cycle.len(),
            cell.upper_cycle.len(),
            target.root_bridge.0,
            target.root_bridge.1,
            evidence.root_splits_considered,
            evidence.pier_states,
            evidence.occurrence_witnesses,
            evidence.topologies_found,
            contains_known,
        ));
    }
    let json = format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473\",\"fixture\":\"FrozenN6\",\"declared_topology_family\":\"TransitionCellV3+AnnularTarget+PIER\",\"cells\":2,\"known_topologies_recovered\":{recovered},\"cell_evidence\":[{}],\"gate_passed\":{},\"joint_extraction_run\":false,\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        fragments.join(","),
        recovered == 2,
    );
    if let Ok(path) = std::env::var("EARTHMESH_N6_ANNULAR_TARGET_JSON") {
        std::fs::write(path, &json).unwrap();
    }
    println!("{json}");
}

fn frozen_n6_known_cells() -> Vec<(
    AnnularCellDomain,
    earthmesh_refine_certified::coarsen::AnnularTopology,
)> {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let problem = build_face_band_problem(&source, &component, 2).unwrap();
    let FaceBandSolveOutcome::Closed(plan, _) = solve_exact_face_bands(
        &problem,
        FaceBandLimits {
            maximum_states: 16_384,
        },
    ) else {
        panic!("Frozen N6 W2 plan must close")
    };
    let v3 = build_stratified_transition_domain_v3(&source, &component, &plan).unwrap();
    let FullPolygonMergeOutcome::Closed(legacy) = solve_full_polygon_merge_from_face_bands(
        &source,
        &component,
        &plan,
        FullPolygonMergeLimits {
            topology_states: 4_096,
        },
    ) else {
        panic!("Frozen N6 legacy topology must close")
    };
    v3.cells
        .into_iter()
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
            (cell, topology)
        })
        .collect()
}
