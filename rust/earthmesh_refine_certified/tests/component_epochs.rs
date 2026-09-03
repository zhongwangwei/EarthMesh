use std::collections::{BTreeMap, BTreeSet};

use earthmesh_refine_certified::{
    coarsen::{
        run_elastic_component_epochs, ComponentOutcomeKind, ElasticCmrcConfig, ElasticCmrcOutcome,
    },
    AngleContractId, MotherGrid, SourceLevelField, TriangleAddress,
};

fn source_levels(grid: &MotherGrid, levels_by_site: &[usize]) -> SourceLevelField {
    SourceLevelField::from_active_voronoi_cells(
        &grid.mesh,
        grid.mesh
            .active_vertex_slots()
            .map(|site| levels_by_site[site])
            .collect(),
    )
    .unwrap()
}

fn full_config(max_level: usize) -> ElasticCmrcConfig {
    ElasticCmrcConfig {
        angle_contract: AngleContractId::LegacyStrict40To80,
        max_level,
        max_adjacent_level_delta: 1,
        initial_transition_rings: 1,
        maximum_transition_rings: 3,
        topology_states_per_component: 10_000,
        elastic_iterations_per_topology: 256,
        interval_boxes_per_component: 100_000,
        total_transition_states: 100_000,
        allow_safe_fallback: false,
    }
}

fn parent_neighbours(grid: &MotherGrid, parent: TriangleAddress) -> BTreeSet<TriangleAddress> {
    let children = parent
        .children_2_to_1()
        .unwrap()
        .into_iter()
        .collect::<BTreeSet<_>>();
    grid.mesh
        .active_triangle_slots()
        .filter(|&face| {
            grid.triangle_addresses[face].is_some_and(|child| children.contains(&child))
        })
        .flat_map(|face| grid.mesh.neighbours()[face])
        .filter_map(|face| grid.triangle_addresses[face].and_then(TriangleAddress::parent_2_to_1))
        .filter(|&other| other != parent)
        .collect()
}

fn parent_ball(
    grid: &MotherGrid,
    center: TriangleAddress,
    rings: usize,
) -> BTreeSet<TriangleAddress> {
    let mut ball = BTreeSet::from([center]);
    let mut frontier = ball.clone();
    for _ in 0..rings {
        let next = frontier
            .iter()
            .flat_map(|&parent| parent_neighbours(grid, parent))
            .filter(|parent| !ball.contains(parent))
            .collect::<BTreeSet<_>>();
        ball.extend(next.iter().copied());
        frontier = next;
    }
    ball
}

fn lower_patch_requirements(
    grid: &MotherGrid,
    parents: &BTreeSet<TriangleAddress>,
    required: &mut [usize],
) {
    for face in grid.mesh.active_triangle_slots() {
        if grid.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .is_some_and(|parent| parents.contains(&parent))
        {
            for site in grid.mesh.triangles()[face] {
                required[site] = 2;
            }
        }
    }
}

#[test]
fn uniform_requirement_runs_exactly_three_to_two_to_one_to_zero() {
    let source = MotherGrid::generate(8).unwrap();
    let required = vec![0; source.mesh.vertices().len()];
    let levels = source_levels(&source, &required);

    let outcome = run_elastic_component_epochs(
        source.clone(),
        &source.mesh,
        &levels,
        &required,
        &full_config(3),
    );
    let ElasticCmrcOutcome::Completed(result) = outcome else {
        panic!("the exact hierarchy must complete: {outcome:?}")
    };

    assert_eq!(
        result
            .report
            .levels
            .iter()
            .map(|level| (level.source_level, level.target_level))
            .collect::<Vec<_>>(),
        [(3, 2), (2, 1), (1, 0)]
    );
    assert!(result
        .report
        .levels
        .iter()
        .all(|level| level.components_committed == 1));
    assert_eq!(result.report.components_committed, 3);
    assert_eq!(result.report.components_promoted, 0);
    assert_eq!(result.report.total_topology_states, 0);
    assert_eq!(result.report.final_faces, 20);
    assert_eq!(result.report.final_vertices, 12);
    assert_eq!(result.report.delivered_histogram, BTreeMap::from([(0, 12)]));
    assert_eq!(
        result.state.mesh().mesh,
        MotherGrid::generate(1).unwrap().mesh
    );
}

#[test]
fn global_topology_budget_is_shared_across_same_level_components() {
    let source = MotherGrid::generate(8).unwrap();
    let coarse = MotherGrid::generate(4).unwrap();
    let centers = [0, 10].map(|base_face| {
        coarse
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .find(|address| address.base_face == base_face)
            .unwrap()
    });
    let patches = centers.map(|center| parent_ball(&source, center, 2));
    assert!(patches[0].is_disjoint(&patches[1]));

    let mut required = vec![3; source.mesh.vertices().len()];
    for patch in &patches {
        lower_patch_requirements(&source, patch, &mut required);
    }
    let levels = source_levels(&source, &required);
    let mut config = full_config(3);
    config.topology_states_per_component = 10;
    config.total_transition_states = 2;
    config.elastic_iterations_per_topology = 0;

    let outcome =
        run_elastic_component_epochs(source.clone(), &source.mesh, &levels, &required, &config);
    let ElasticCmrcOutcome::Completed(result) = outcome else {
        panic!("bounded component scheduling must complete: {outcome:?}")
    };

    assert_eq!(result.report.levels[0].components_total, 2);
    assert_eq!(result.report.total_topology_states, 2);
    assert_eq!(result.report.components.len(), 2);
    assert!(result
        .report
        .components
        .iter()
        .all(|component| component.topology_states == 1));
    assert!(result
        .report
        .components
        .iter()
        .all(|component| { component.outcome == ComponentOutcomeKind::SearchBudgetExhausted }));
}
