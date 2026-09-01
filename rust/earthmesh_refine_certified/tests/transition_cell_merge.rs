use earthmesh_refine_certified::coarsen::{
    build_face_band_problem, build_stratified_transition_domain_v3, certify_annular_topology,
    enumerate_balanced_annular_strips, n6_legacy_mixed_fixture, solve_exact_face_bands,
    solve_full_polygon_merge_from_face_bands, solve_transition_cell_find_one,
    AnnularEnumerationEvidence, AnnularTransitionCellFamily, FaceBandLimits, FaceBandSolveOutcome,
    FullAnnularFamily, FullPolygonMergeLimits, FullPolygonMergeOutcome, TransitionCellDomain,
    TransitionCellFamily, TransitionCellMergeLimits, TransitionCellMergeOutcome,
};
use std::collections::BTreeSet;

#[test]
fn legacy_selected_topology_is_in_v3() {
    let (source, component, v3, legacy, families) = frozen_n6_v3_families();
    let outcome = solve_transition_cell_find_one(
        &source,
        &component,
        &v3,
        &families,
        TransitionCellMergeLimits {
            topology_states: 1,
            ear_states_per_topology: usize::MAX,
        },
    );
    let TransitionCellMergeOutcome::Closed(trial) = outcome else {
        panic!("Frozen N6 V3 inclusion must close: {outcome:?}")
    };
    assert_eq!(trial.evidence.states_examined, 1);
    assert_eq!(trial.evidence.selected_annular_keys.len(), 2);
    assert!(trial.global_trial.evidence.selected_ears.is_empty());
    assert_eq!(
        trial.global_trial.evidence.anchor_degrees,
        legacy.global_trial.evidence.anchor_degrees
    );
    assert_eq!(
        trial.global_trial.evidence.ordinary_degree_histogram,
        legacy.global_trial.evidence.ordinary_degree_histogram
    );
    assert_eq!(
        (
            trial.global_trial.evidence.vertices,
            trial.global_trial.evidence.edges,
            trial.global_trial.evidence.faces,
            trial.global_trial.evidence.euler,
            trial.global_trial.evidence.charge,
        ),
        (
            legacy.global_trial.evidence.vertices,
            legacy.global_trial.evidence.edges,
            legacy.global_trial.evidence.faces,
            legacy.global_trial.evidence.euler,
            legacy.global_trial.evidence.charge,
        )
    );
    assert_eq!(
        canonical_triangles(&trial.global_trial.custom_triangles),
        canonical_triangles(&legacy.global_trial.custom_triangles)
    );
}

#[test]
fn zero_global_states_is_typed_incomplete() {
    let (source, component, v3, _, families) = frozen_n6_v3_families();
    assert!(matches!(
        solve_transition_cell_find_one(
            &source,
            &component,
            &v3,
            &families,
            TransitionCellMergeLimits {
                topology_states: 0,
                ear_states_per_topology: usize::MAX,
            },
        ),
        TransitionCellMergeOutcome::SearchIncomplete(_)
    ));
}

#[test]
fn balanced_annular_subset_finds_frozen_n6_topology() {
    let (source, component, v3, _, _) = frozen_n6_v3_families();
    let families = v3
        .cells
        .iter()
        .map(|cell| {
            let TransitionCellDomain::Annulus(cell) = cell else {
                panic!("Frozen N6 W2 cells must be annular")
            };
            let search = enumerate_balanced_annular_strips(
                &cell.lower_cycle,
                &cell.upper_cycle,
                &cell.forbidden_global_edges,
                64,
            )
            .unwrap();
            TransitionCellFamily::Annulus(AnnularTransitionCellFamily {
                cell_id: cell.cell_id,
                family: search.family,
            })
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        solve_transition_cell_find_one(
            &source,
            &component,
            &v3,
            &families,
            TransitionCellMergeLimits {
                topology_states: 4_096,
                ear_states_per_topology: 256,
            },
        ),
        TransitionCellMergeOutcome::Closed(_)
    ));
}

fn frozen_n6_v3_families() -> (
    earthmesh_refine_certified::MotherGrid,
    earthmesh_refine_certified::coarsen::HierarchyComponent,
    earthmesh_refine_certified::coarsen::StratifiedTransitionDomainV3,
    Box<earthmesh_refine_certified::coarsen::FullPolygonMergeTrial>,
    Vec<TransitionCellFamily>,
) {
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
    let families = v3
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
            TransitionCellFamily::Annulus(AnnularTransitionCellFamily {
                cell_id: cell.cell_id,
                family: FullAnnularFamily {
                    lower_vertices: cell.lower_cycle.len(),
                    upper_vertices: cell.upper_cycle.len(),
                    topologies: vec![topology],
                    evidence: AnnularEnumerationEvidence::default(),
                },
            })
        })
        .collect();
    (source, component, v3, legacy, families)
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
