use std::collections::BTreeSet;

use earthmesh_refine_certified::{
    coarsen::{
        solve_component_transaction, ComponentTransactionLimits, ComponentTransactionOutcome,
        ComponentTransactionStage, ComponentTransactionState, HierarchyComponent,
    },
    MotherGrid, SourceLevelField, TriangleAddress, TriangleOrientation,
};

const FULL_LIMITS: ComponentTransactionLimits = ComponentTransactionLimits {
    topology_states: 10_000,
    elastic_iterations: 256,
    interval_boxes: 100_000,
    halo_expansions: 0,
};

fn source_levels(grid: &MotherGrid, level: usize) -> SourceLevelField {
    SourceLevelField::from_active_voronoi_cells(
        &grid.mesh,
        vec![level; grid.mesh.active_vertex_slots().count()],
    )
    .unwrap()
}

fn whole_sphere_component(coarse_n: usize) -> HierarchyComponent {
    let parents = MotherGrid::generate(coarse_n)
        .unwrap()
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    HierarchyComponent {
        id: 30,
        parents: parents.clone(),
        boundary_edges: Vec::new(),
        core_parents: parents,
        transition_parents: Vec::new(),
    }
}

fn parent_neighbours(grid: &MotherGrid, parent: TriangleAddress) -> Vec<TriangleAddress> {
    let children = parent
        .children_2_to_1()
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut neighbours = BTreeSet::new();
    for face in grid.mesh.active_triangle_slots() {
        if !grid.triangle_addresses[face].is_some_and(|address| children.contains(&address)) {
            continue;
        }
        for &neighbour in &grid.mesh.neighbours()[face] {
            let other = grid.triangle_addresses[neighbour]
                .and_then(TriangleAddress::parent_2_to_1)
                .unwrap();
            if other != parent {
                neighbours.insert(other);
            }
        }
    }
    neighbours.into_iter().collect()
}

fn mixed_component(grid: &MotherGrid) -> HierarchyComponent {
    let core = TriangleAddress {
        base_face: 0,
        i: 1,
        j: 1,
        n: 4,
        orientation: TriangleOrientation::Down,
    };
    let first_ring = parent_neighbours(grid, core);
    let mut transition = first_ring.clone();
    for parent in &first_ring {
        transition.extend(
            parent_neighbours(grid, *parent)
                .into_iter()
                .filter(|parent| *parent != core && !first_ring.contains(parent)),
        );
    }
    transition.sort_unstable();
    transition.dedup();
    let mut parents = vec![core];
    parents.extend(transition.iter().copied());
    parents.sort_unstable();
    HierarchyComponent {
        id: 31,
        parents,
        boundary_edges: Vec::new(),
        core_parents: vec![core],
        transition_parents: transition,
    }
}

#[test]
fn certified_component_commit_reduces_mesh_and_is_deterministic() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 2);
    let component = whole_sphere_component(4);
    let mut first = ComponentTransactionState::new(&source, 3).unwrap();
    let mut second = first.clone();
    let initial_vertices = first.mesh().mesh.vertex_count();
    let initial_faces = first.mesh().mesh.triangle_count();

    let first_outcome =
        solve_component_transaction(&source, &levels, &mut first, &component, 2, 1, FULL_LIMITS);
    let second_outcome =
        solve_component_transaction(&source, &levels, &mut second, &component, 2, 1, FULL_LIMITS);
    let (
        ComponentTransactionOutcome::Certified(first_report),
        ComponentTransactionOutcome::Certified(second_report),
    ) = (first_outcome, second_outcome)
    else {
        panic!("the exact whole-sphere component must certify")
    };

    assert_eq!(first, second);
    assert_eq!(first.fingerprint(), first_report.after_fingerprint);
    assert_eq!(
        first_report.before_fingerprint,
        second_report.before_fingerprint
    );
    assert_eq!(
        first_report.after_fingerprint,
        second_report.after_fingerprint
    );
    assert_eq!(first_report.local_geometry, second_report.local_geometry);
    assert_eq!(first_report.global_geometry, second_report.global_geometry);
    assert_eq!(
        first_report.final_certificate,
        second_report.final_certificate
    );
    assert_eq!(first_report.remap, second_report.remap);
    assert_eq!(first_report.core_search_states, 0);
    assert_eq!(first_report.topology_states, 0);
    assert!(first_report.interval_boxes > 0);
    assert!(first.mesh().mesh.vertex_count() < initial_vertices);
    assert!(first.mesh().mesh.triangle_count() < initial_faces);
    assert!(first_report.removed_vertices > 0);
    assert!(first_report.removed_faces > 0);
    first_report
        .final_certificate
        .require_final_gates()
        .unwrap();
    assert_eq!(first_report.final_cells.physical_residuals(), 0);
    assert_eq!(first_report.final_cells.balance_residuals(), 0);
    assert_eq!(first_report.remap.negative_weights(), 0);
    assert_eq!(first_report.remap.bad_row_sums(), 0);
    assert_eq!(first_report.remap.bad_lineage_rows(), 0);
    assert!(first_report.remap.constant_closure_error() <= first_report.remap.closure_tolerance());
    assert!(
        first_report.remap.global_area_closure_error() <= first_report.remap.closure_tolerance()
    );
    assert!(first
        .target_levels()
        .unwrap()
        .levels()
        .iter()
        .all(|&level| level == 2));
}

#[test]
fn interval_budget_failure_restores_the_exact_installed_snapshot() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 2);
    let component = whole_sphere_component(4);
    let mut state = ComponentTransactionState::new(&source, 3).unwrap();
    let snapshot = state.clone();

    let outcome = solve_component_transaction(
        &source,
        &levels,
        &mut state,
        &component,
        2,
        1,
        ComponentTransactionLimits {
            interval_boxes: 0,
            ..FULL_LIMITS
        },
    );
    let ComponentTransactionOutcome::SearchBudgetExhausted(report) = outcome else {
        panic!("zero interval budget must be explicit: {outcome:?}")
    };

    assert_eq!(report.stage, ComponentTransactionStage::LocalGeometry);
    assert!(report.interval_boxes > 0);
    assert_eq!(report.before_fingerprint, report.restored_fingerprint);
    assert_eq!(state, snapshot);
}

#[test]
fn physical_failure_never_reaches_topology_or_elastic_search() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 3);
    let component = whole_sphere_component(4);
    let mut state = ComponentTransactionState::new(&source, 3).unwrap();
    let snapshot = state.clone();

    let outcome =
        solve_component_transaction(&source, &levels, &mut state, &component, 2, 1, FULL_LIMITS);
    let ComponentTransactionOutcome::NotCertifiable(report) = outcome else {
        panic!("physical refusal must remain distinct: {outcome:?}")
    };

    assert_eq!(report.stage, ComponentTransactionStage::Physical);
    assert_eq!(report.topology_states, 0);
    assert_eq!(report.elastic_iterations, 0);
    assert_eq!(report.before_fingerprint, report.restored_fingerprint);
    assert_eq!(state, snapshot);
}

#[test]
fn elastic_budget_exhaustion_rolls_back_a_staged_mixed_topology() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 2);
    let component = mixed_component(&source);
    let mut state = ComponentTransactionState::new(&source, 3).unwrap();
    let snapshot = state.clone();

    let outcome = solve_component_transaction(
        &source,
        &levels,
        &mut state,
        &component,
        2,
        1,
        ComponentTransactionLimits {
            elastic_iterations: 0,
            ..FULL_LIMITS
        },
    );
    let ComponentTransactionOutcome::SearchBudgetExhausted(report) = outcome else {
        panic!("zero CBER budget must be explicit: {outcome:?}")
    };

    assert_eq!(report.stage, ComponentTransactionStage::Elastic);
    assert!(report.topology_states > 0);
    assert_eq!(report.elastic_iterations, 0);
    assert_eq!(report.before_fingerprint, report.restored_fingerprint);
    assert_eq!(state, snapshot);
}

#[test]
fn mixed_component_exhausts_all_uncertifiable_topologies_before_rollback() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 2);
    let component = mixed_component(&source);
    let mut state = ComponentTransactionState::new(&source, 3).unwrap();
    let snapshot = state.clone();

    let outcome =
        solve_component_transaction(&source, &levels, &mut state, &component, 2, 1, FULL_LIMITS);
    let ComponentTransactionOutcome::NotCertifiable(report) = outcome else {
        panic!("all mixed topology candidates are CBER-uncertifiable: {outcome:?}")
    };

    assert_eq!(report.stage, ComponentTransactionStage::Elastic);
    assert_eq!(report.topology_states, 45);
    assert_eq!(report.elastic_iterations, 218);
    assert_eq!(report.before_fingerprint, report.restored_fingerprint);
    assert_eq!(state, snapshot);
}

#[test]
fn pure_core_can_certify_with_zero_topology_budget() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 2);
    let component = whole_sphere_component(4);
    let mut state = ComponentTransactionState::new(&source, 3).unwrap();

    let outcome = solve_component_transaction(
        &source,
        &levels,
        &mut state,
        &component,
        2,
        1,
        ComponentTransactionLimits {
            topology_states: 0,
            ..FULL_LIMITS
        },
    );
    let ComponentTransactionOutcome::Certified(report) = outcome else {
        panic!("pure core should not spend topology-search budget: {outcome:?}")
    };

    assert_eq!(report.topology_states, 0);
    assert!(report.removed_vertices > 0);
}

#[test]
fn committed_parent_claim_survives_candidate_snapshot_commit() {
    let source = MotherGrid::generate(8).unwrap();
    let levels = source_levels(&source, 2);
    let component = whole_sphere_component(4);
    let mut state = ComponentTransactionState::new(&source, 3).unwrap();

    assert!(matches!(
        solve_component_transaction(&source, &levels, &mut state, &component, 2, 1, FULL_LIMITS),
        ComponentTransactionOutcome::Certified(_)
    ));
    let outcome =
        solve_component_transaction(&source, &levels, &mut state, &component, 2, 1, FULL_LIMITS);
    let ComponentTransactionOutcome::InvalidInput(report) = outcome else {
        panic!("committed parents must remain claimed: {outcome:?}")
    };

    assert_eq!(report.stage, ComponentTransactionStage::Preflight);
    assert!(report.reason.contains("already claimed"));
}
