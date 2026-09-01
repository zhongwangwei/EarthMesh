use earthmesh_refine_certified::coarsen::{
    analyze_annular_signature_domains, analyze_stratified_annular_degree_reachability,
    annular_topology_signature, build_face_band_problem, build_stratified_transition_domain_v3,
    enumerate_annular_degree_signatures, enumerate_canonical_seam_annulus, n6_legacy_mixed_fixture,
    solve_exact_face_bands, solve_full_polygon_merge_from_face_bands, AnnularCellDomain,
    AnnularCellKey, AnnularCellSignatureDomain, AnnularReachabilityLimits,
    AnnularReachabilityOutcome, AnnularSignatureSearchStatus, FaceBandLimits, FaceBandSolveOutcome,
    FullPolygonMergeLimits, FullPolygonMergeOutcome, TopologyBoundaryKind, TransitionCellDomain,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn signature_dp_never_drops_small_csae_topologies() {
    for (lower_len, upper_len) in [(3, 3), (3, 4), (4, 4), (4, 5)] {
        let cell = synthetic_cell(lower_len, upper_len);
        let caps = cell
            .lower_cycle
            .iter()
            .chain(&cell.upper_cycle)
            .map(|&vertex| ((cell.cell_id, vertex), 7))
            .collect();
        let domain = enumerate_annular_degree_signatures(
            &cell,
            &caps,
            AnnularReachabilityLimits {
                maximum_signature_states: 2_000_000,
            },
        )
        .unwrap();
        assert_eq!(
            domain.status,
            AnnularSignatureSearchStatus::ExhaustedNecessaryRelaxation
        );
        let dp = domain
            .signatures
            .iter()
            .map(signature_key)
            .collect::<BTreeSet<_>>();
        let csae = enumerate_canonical_seam_annulus(
            &cell.lower_cycle,
            &cell.upper_cycle,
            &BTreeSet::new(),
        )
        .unwrap();
        for topology in csae.topologies {
            let signature = annular_topology_signature(&cell, &topology.triangles).unwrap();
            assert!(dp.contains(&signature_key(&signature)));
        }
    }
}

#[test]
fn signature_budget_is_typed_incomplete() {
    let cell = synthetic_cell(3, 3);
    let caps = cell
        .lower_cycle
        .iter()
        .chain(&cell.upper_cycle)
        .map(|&vertex| ((cell.cell_id, vertex), 7))
        .collect();
    let domain = enumerate_annular_degree_signatures(
        &cell,
        &caps,
        AnnularReachabilityLimits {
            maximum_signature_states: 0,
        },
    )
    .unwrap();
    assert_eq!(
        domain.status,
        AnnularSignatureSearchStatus::SearchIncomplete
    );
}

#[test]
fn frozen_n6_selected_topology_survives_annular_caps_and_ac3() {
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

    let mut cell_domains = Vec::new();
    for (cell_index, cell) in v3.cells.iter().enumerate() {
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
            // The frozen legacy owner convention assigns triangles wholly on
            // the shared W2 interface to the lower-index annular cell.
            .filter(|triangle| {
                cell_index == 0 || !triangle.iter().all(|vertex| lower.contains(vertex))
            })
            .collect::<Vec<_>>();
        assert_eq!(triangles.len(), vertices.len());
        cell_domains.push(AnnularCellSignatureDomain {
            cell_id: cell.cell_id,
            signatures: vec![annular_topology_signature(cell, &triangles).unwrap()],
            root_bridges_considered: 0,
            states_examined: 1,
            degree_cap_prunes: 0,
            status: AnnularSignatureSearchStatus::ExhaustedNecessaryRelaxation,
        });
    }
    let evidence = analyze_annular_signature_domains(&v3, cell_domains, &BTreeMap::new()).unwrap();
    assert_eq!(
        evidence.outcome,
        AnnularReachabilityOutcome::NecessaryFeasible
    );
    assert_eq!(evidence.cell_signature_counts_after_ac3, vec![1, 1]);
    assert_eq!(evidence.ac3_prunes, 0);
}

#[test]
fn frozen_n6_raw_signature_dp_reports_budget_instead_of_false_no_solution() {
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
    let evidence = analyze_stratified_annular_degree_reachability(
        &v3,
        AnnularReachabilityLimits {
            maximum_signature_states: 4_096,
        },
    )
    .unwrap();
    assert_eq!(
        evidence.outcome,
        AnnularReachabilityOutcome::SearchIncomplete
    );
}

fn synthetic_cell(lower: usize, upper: usize) -> AnnularCellDomain {
    AnnularCellDomain {
        cell_id: 7,
        lower_cycle: (0..lower).collect(),
        upper_cycle: (100..100 + upper).collect(),
        lower_boundary_kind: TopologyBoundaryKind::SourceCycle,
        upper_boundary_kind: TopologyBoundaryKind::SourceCycle,
        forbidden_global_edges: BTreeSet::new(),
        fixed_outside_link_contracts: BTreeMap::new(),
        cell_key: AnnularCellKey("synthetic".into()),
    }
}

fn signature_key(
    signature: &earthmesh_refine_certified::coarsen::AnnularTopologySignature,
) -> String {
    format!(
        "{:?}|{:?}|{:?}",
        signature.vertex_incidences, signature.boundary_link_contributions, signature.root_bridge
    )
}
