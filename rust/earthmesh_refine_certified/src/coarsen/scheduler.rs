use super::{
    component_transaction::solve_component_transaction_at_level,
    plan_hierarchy_components_from_parent_requirements, ComponentRollbackReport,
    ComponentTransactionLimits, ComponentTransactionOutcome, ComponentTransactionState,
    ExplicitParentRequirement, HierarchyComponent,
};
use crate::{
    certificate::Certificate,
    fingerprint::mesh_fingerprint,
    mother_grid::{mother_cell_count, MotherGrid, TriangleAddress},
    outcome::FinalCertificationEvidence,
    remap::ConservativeRemap,
    requirement::{certify_final_cell_requirements_with_remap, SourceLevelField},
};
use earthmesh_mesh::MeshState;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElasticCmrcConfig {
    pub max_level: usize,
    pub max_adjacent_level_delta: usize,
    pub initial_transition_rings: usize,
    pub maximum_transition_rings: usize,
    pub topology_states_per_component: usize,
    pub elastic_iterations_per_topology: usize,
    pub interval_boxes_per_component: usize,
    pub total_transition_states: usize,
    pub allow_safe_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentOutcomeKind {
    Certified,
    NoTopology,
    ElasticNoImprovement,
    SearchBudgetExhausted,
    RequiresWiderHalo,
    NotCertifiable,
    PromotedToFine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticComponentRecord {
    pub component_id: u64,
    pub source_level: usize,
    pub target_level: usize,
    pub parent_count: usize,
    pub core_parent_count: usize,
    pub transition_parent_count: usize,
    pub core_vertices_removed: usize,
    pub topology_states: usize,
    pub elastic_iterations: usize,
    pub interval_boxes: usize,
    pub transition_ring_width: usize,
    pub outcome: ComponentOutcomeKind,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticLevelReport {
    pub source_level: usize,
    pub target_level: usize,
    pub components_total: usize,
    pub components_committed: usize,
    pub components_promoted: usize,
    pub components_exhausted: usize,
    pub delivered_histogram: BTreeMap<usize, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElasticCmrcReport {
    pub initial_faces: usize,
    pub final_faces: usize,
    pub initial_vertices: usize,
    pub final_vertices: usize,
    pub requested_histogram: BTreeMap<usize, usize>,
    pub delivered_histogram: BTreeMap<usize, usize>,
    pub components_total: usize,
    pub components_committed: usize,
    pub components_promoted: usize,
    pub components_exhausted: usize,
    pub total_topology_states: usize,
    pub total_elastic_iterations: usize,
    pub total_interval_boxes: usize,
    pub core_vertices_removed: usize,
    pub search_complete: bool,
    pub levels: Vec<ElasticLevelReport>,
    pub components: Vec<ElasticComponentRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElasticCmrcResult {
    pub state: ComponentTransactionState,
    pub report: ElasticCmrcReport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElasticCmrcOutcome {
    Completed(Box<ElasticCmrcResult>),
    NotCertifiable { reason: String },
    InvalidInput { reason: String },
}

pub fn run_elastic_component_epochs(
    grid: MotherGrid,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    required_levels: &[usize],
    config: &ElasticCmrcConfig,
) -> ElasticCmrcOutcome {
    if let Err(reason) = validate_inputs(&grid, source_mesh, source_levels, required_levels, config)
    {
        return ElasticCmrcOutcome::InvalidInput { reason };
    }
    let mut state = match ComponentTransactionState::new(&grid, config.max_level) {
        Ok(state) => state,
        Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
    };
    let initial_faces = grid.mesh.triangle_count();
    let initial_vertices = grid.mesh.vertex_count();
    let mut remaining_topology_states = config.total_transition_states;
    let mut next_component_id = 0u64;
    let mut report = ElasticCmrcReport {
        initial_faces,
        final_faces: initial_faces,
        initial_vertices,
        final_vertices: initial_vertices,
        requested_histogram: histogram(
            grid.mesh
                .active_vertex_slots()
                .map(|site| required_levels[site]),
        ),
        delivered_histogram: histogram(state.target_levels().unwrap().levels().iter().copied()),
        components_total: 0,
        components_committed: 0,
        components_promoted: 0,
        components_exhausted: 0,
        total_topology_states: 0,
        total_elastic_iterations: 0,
        total_interval_boxes: 0,
        core_vertices_removed: 0,
        search_complete: true,
        levels: Vec::new(),
        components: Vec::new(),
    };

    for source_level in (1..=config.max_level).rev() {
        let target_level = source_level - 1;
        let shift = config.max_level - source_level;
        let fine_n = grid.subdivision >> shift;
        let owned_level_grid;
        let level_grid = if fine_n == grid.subdivision {
            &grid
        } else {
            owned_level_grid = match MotherGrid::generate(fine_n) {
                Ok(grid) => grid,
                Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
            };
            &owned_level_grid
        };
        let level_source_slots = match state.level_source_slots(level_grid) {
            Ok(slots) => slots,
            Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
        };
        let requirements =
            match explicit_parent_requirements(&grid, &state, level_grid, required_levels) {
                Ok(requirements) => requirements,
                Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
            };
        let mut plan = match plan_hierarchy_components_from_parent_requirements(
            level_grid,
            &requirements,
            target_level,
            config.initial_transition_rings,
        ) {
            Ok(plan) => plan,
            Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
        };
        sort_components(
            &mut plan.components,
            &plan.parent_requirements,
            target_level,
        );
        let components_total = plan.components.len();
        let mut transition_components_remaining = plan
            .components
            .iter()
            .filter(|component| !component.transition_parents.is_empty())
            .count();
        let mut committed = 0usize;
        let mut promoted = 0usize;
        let mut exhausted = 0usize;

        for mut component in plan.components {
            component.id = next_component_id;
            next_component_id = next_component_id.saturating_add(1);
            let topology_states = if component.transition_parents.is_empty() {
                0
            } else {
                let fair_share =
                    remaining_topology_states.div_ceil(transition_components_remaining);
                transition_components_remaining -= 1;
                config.topology_states_per_component.min(fair_share)
            };
            let limits = ComponentTransactionLimits {
                topology_states,
                elastic_iterations: config.elastic_iterations_per_topology,
                interval_boxes: config.interval_boxes_per_component,
                halo_expansions: config
                    .maximum_transition_rings
                    .saturating_sub(config.initial_transition_rings),
            };
            let outcome = solve_component_transaction_at_level(
                &grid,
                source_levels,
                &mut state,
                level_grid,
                &level_source_slots,
                &component,
                target_level,
                config.max_adjacent_level_delta,
                limits,
            );
            let component_record = match outcome {
                ComponentTransactionOutcome::Certified(commit) => {
                    committed += 1;
                    report.components_committed += 1;
                    report.core_vertices_removed += commit.core_vertices_removed;
                    report.total_topology_states += commit.topology_states;
                    report.total_elastic_iterations += commit.elastic_iterations;
                    report.total_interval_boxes += commit.interval_boxes;
                    remaining_topology_states =
                        remaining_topology_states.saturating_sub(commit.topology_states);
                    ElasticComponentRecord {
                        component_id: component.id,
                        source_level,
                        target_level,
                        parent_count: component.parents.len(),
                        core_parent_count: component.core_parents.len(),
                        transition_parent_count: component.transition_parents.len(),
                        core_vertices_removed: commit.core_vertices_removed,
                        topology_states: commit.topology_states,
                        elastic_iterations: commit.elastic_iterations,
                        interval_boxes: commit.interval_boxes,
                        transition_ring_width: config.initial_transition_rings
                            + commit.halo_expansions,
                        outcome: ComponentOutcomeKind::Certified,
                        reason: None,
                    }
                }
                ComponentTransactionOutcome::InvalidInput(rollback) => {
                    return ElasticCmrcOutcome::InvalidInput {
                        reason: format!(
                            "component {} at {source_level}->{target_level}: {}",
                            component.id, rollback.reason
                        ),
                    };
                }
                ComponentTransactionOutcome::NoTopology(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::NoTopology,
                    rollback,
                ),
                ComponentTransactionOutcome::ElasticNoImprovement(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::ElasticNoImprovement,
                    rollback,
                ),
                ComponentTransactionOutcome::SearchBudgetExhausted(rollback) => {
                    exhausted += 1;
                    report.components_exhausted += 1;
                    report.search_complete = false;
                    rollback_record(
                        &component,
                        source_level,
                        target_level,
                        config.initial_transition_rings,
                        ComponentOutcomeKind::SearchBudgetExhausted,
                        rollback,
                    )
                }
                ComponentTransactionOutcome::RequiresWiderHalo(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::RequiresWiderHalo,
                    rollback,
                ),
                ComponentTransactionOutcome::NotCertifiable(rollback) => rollback_record(
                    &component,
                    source_level,
                    target_level,
                    config.initial_transition_rings,
                    ComponentOutcomeKind::NotCertifiable,
                    rollback,
                ),
            };
            if component_record.outcome != ComponentOutcomeKind::Certified {
                promoted += 1;
                report.components_promoted += 1;
                report.total_topology_states += component_record.topology_states;
                report.total_elastic_iterations += component_record.elastic_iterations;
                report.total_interval_boxes += component_record.interval_boxes;
                remaining_topology_states =
                    remaining_topology_states.saturating_sub(component_record.topology_states);
            }
            report.components.push(component_record);
        }
        report.components_total += components_total;
        if let Err(reason) = certify_stage(
            &grid,
            source_levels,
            &state,
            config.max_adjacent_level_delta,
        ) {
            return ElasticCmrcOutcome::NotCertifiable {
                reason: format!("stage {source_level}->{target_level}: {reason}"),
            };
        }
        let delivered_histogram = match state.target_levels() {
            Ok(levels) => histogram(levels.levels().iter().copied()),
            Err(reason) => return ElasticCmrcOutcome::InvalidInput { reason },
        };
        report.levels.push(ElasticLevelReport {
            source_level,
            target_level,
            components_total,
            components_committed: committed,
            components_promoted: promoted,
            components_exhausted: exhausted,
            delivered_histogram: delivered_histogram.clone(),
        });
        report.delivered_histogram = delivered_histogram;
    }

    report.final_faces = state.mesh().mesh.triangle_count();
    report.final_vertices = state.mesh().mesh.vertex_count();
    ElasticCmrcOutcome::Completed(Box::new(ElasticCmrcResult { state, report }))
}

fn validate_inputs(
    grid: &MotherGrid,
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    required_levels: &[usize],
    config: &ElasticCmrcConfig,
) -> Result<(), String> {
    if mesh_fingerprint(&grid.mesh) != mesh_fingerprint(source_mesh) {
        return Err("CMRC grid and source mesh fingerprints differ".into());
    }
    if source_levels.active_sites()
        != grid
            .mesh
            .active_vertex_slots()
            .collect::<Vec<_>>()
            .as_slice()
    {
        return Err("source level field does not match the source grid".into());
    }
    if required_levels.len() != grid.mesh.vertices().len() {
        return Err("required level slots must match source vertices".into());
    }
    if config.maximum_transition_rings < config.initial_transition_rings {
        return Err("maximum transition rings must not be smaller than initial rings".into());
    }
    let factor = 1usize
        .checked_shl(config.max_level as u32)
        .ok_or_else(|| "CMRC maximum level overflows hierarchy scale".to_string())?;
    if !grid.subdivision.is_multiple_of(factor) {
        return Err(format!(
            "source subdivision {} is not divisible by 2^{}",
            grid.subdivision, config.max_level
        ));
    }
    Ok(())
}

fn explicit_parent_requirements(
    source: &MotherGrid,
    state: &ComponentTransactionState,
    level_grid: &MotherGrid,
    required_levels: &[usize],
) -> Result<Vec<ExplicitParentRequirement>, String> {
    let coarse_n = level_grid.subdivision / 2;
    let parent_count =
        mother_cell_count(coarse_n).ok_or_else(|| "hierarchy parent count overflow".to_string())?;
    let mut requirements = vec![None; parent_count];
    for child in level_grid.triangle_addresses.iter().flatten().copied() {
        let parent = child
            .parent_2_to_1()
            .ok_or_else(|| format!("level child {child:?} has no parent"))?;
        let dense = parent.dense_index(coarse_n)?;
        if requirements[dense].is_some() {
            continue;
        }
        let children = parent
            .children_2_to_1()
            .ok_or_else(|| format!("invalid hierarchy parent {parent:?}"))?;
        let available = children
            .iter()
            .all(|child| state.leaf_set().leaves.contains(child));
        let mut maximum_required_level = 0usize;
        visit_source_descendant_faces(source, parent, &mut |face| {
            for site in source.mesh.triangles()[face] {
                maximum_required_level = maximum_required_level.max(
                    *required_levels
                        .get(site)
                        .ok_or_else(|| format!("source site {site} has no required level"))?,
                );
            }
            Ok(())
        })?;
        requirements[dense] = Some(ExplicitParentRequirement {
            parent,
            maximum_required_level,
            available,
        });
    }
    requirements
        .into_iter()
        .enumerate()
        .map(|(dense, requirement)| {
            requirement.ok_or_else(|| format!("hierarchy parent slot {dense} was not populated"))
        })
        .collect()
}

fn visit_source_descendant_faces(
    source: &MotherGrid,
    address: TriangleAddress,
    visit: &mut impl FnMut(usize) -> Result<(), String>,
) -> Result<(), String> {
    if address.n == source.subdivision {
        let face = address
            .dense_index(source.subdivision)?
            .checked_add(2)
            .ok_or_else(|| format!("source face slot overflow for {address:?}"))?;
        return visit(face);
    }
    if address.n == 0
        || address.n > source.subdivision
        || !source.subdivision.is_multiple_of(address.n)
        || !(source.subdivision / address.n).is_power_of_two()
    {
        return Err(format!(
            "hierarchy address {address:?} is not an ancestor of source subdivision {}",
            source.subdivision
        ));
    }
    for child in address
        .children_2_to_1()
        .ok_or_else(|| format!("invalid hierarchy address {address:?}"))?
    {
        visit_source_descendant_faces(source, child, visit)?;
    }
    Ok(())
}

fn sort_components(
    components: &mut [HierarchyComponent],
    requirements: &[super::ParentRequirement],
    coarse_level: usize,
) {
    let coarse_n = components
        .iter()
        .flat_map(|component| component.parents.first())
        .map(|parent| parent.n)
        .next()
        .unwrap_or(1);
    let margin = |component: &HierarchyComponent| {
        let maximum = component
            .parents
            .iter()
            .filter_map(|parent| parent.dense_index(coarse_n).ok())
            .filter_map(|dense| requirements.get(dense))
            .map(|requirement| requirement.maximum_required_level)
            .max()
            .unwrap_or(coarse_level);
        coarse_level.saturating_sub(maximum)
    };
    components.sort_by(|left, right| {
        right
            .core_parents
            .len()
            .cmp(&left.core_parents.len())
            .then_with(|| left.boundary_edges.len().cmp(&right.boundary_edges.len()))
            .then_with(|| {
                right
                    .transition_parents
                    .is_empty()
                    .cmp(&left.transition_parents.is_empty())
            })
            .then_with(|| {
                left.transition_parents
                    .len()
                    .cmp(&right.transition_parents.len())
            })
            .then_with(|| margin(right).cmp(&margin(left)))
            .then_with(|| left.parents.cmp(&right.parents))
    });
}

fn rollback_record(
    component: &HierarchyComponent,
    source_level: usize,
    target_level: usize,
    initial_transition_rings: usize,
    outcome: ComponentOutcomeKind,
    rollback: ComponentRollbackReport,
) -> ElasticComponentRecord {
    ElasticComponentRecord {
        component_id: component.id,
        source_level,
        target_level,
        parent_count: component.parents.len(),
        core_parent_count: component.core_parents.len(),
        transition_parent_count: component.transition_parents.len(),
        core_vertices_removed: 0,
        topology_states: rollback.topology_states,
        elastic_iterations: rollback.elastic_iterations,
        interval_boxes: rollback.interval_boxes,
        transition_ring_width: initial_transition_rings + rollback.halo_expansions,
        outcome,
        reason: Some(rollback.reason),
    }
}

fn certify_stage(
    source: &MotherGrid,
    source_levels: &SourceLevelField,
    state: &ComponentTransactionState,
    max_adjacent_level_delta: usize,
) -> Result<(), String> {
    Certificate::internal()
        .verify_geometry(&state.mesh().mesh)
        .map_err(|error| format!("internal geometry: {error:?}"))?;
    Certificate::final_delivery()
        .verify_geometry(&state.mesh().mesh)
        .map_err(|error| format!("final geometry: {error:?}"))?;
    let target_levels = state.target_levels()?;
    let remap = ConservativeRemap::between_voronoi_meshes(&source.mesh, &state.mesh().mesh)?;
    let final_cells = certify_final_cell_requirements_with_remap(
        &source.mesh,
        source_levels,
        &state.mesh().mesh,
        &target_levels,
        max_adjacent_level_delta,
        &remap,
    )
    .map_err(|error| format!("final cells: {error:?}"))?;
    let remap_certificate =
        remap.certify_spherical_overlap(source_levels.levels().len(), target_levels.levels().len());
    FinalCertificationEvidence::from_final_cells(&final_cells, remap_certificate)
        .map_err(|reason| format!("remap: {reason}"))?;
    Ok(())
}

fn histogram(levels: impl IntoIterator<Item = usize>) -> BTreeMap<usize, usize> {
    let mut histogram = BTreeMap::new();
    for level in levels {
        *histogram.entry(level).or_default() += 1;
    }
    histogram
}
