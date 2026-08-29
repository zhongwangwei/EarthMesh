use std::collections::BTreeMap;

use earthmesh_refine_certified::{
    coarsen::{run_elastic_component_epochs, ElasticCmrcConfig, ElasticCmrcOutcome},
    MotherGrid, SourceLevelField,
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
