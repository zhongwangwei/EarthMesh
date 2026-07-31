use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::io;

use crate::MaskPostprocLayout;

/// Product-specific component retention after regional, land/ocean, or basin
/// clipping. Hard source demand always has higher priority than this policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentRetentionPolicy {
    /// Retain every edge-connected component with at least two cells.
    KeepAllNonSingletons,
    /// Retain only the component(s) containing an explicit seed.
    KeepSeedConnected,
    /// Retain components whose summed cell area reaches the configured floor.
    KeepAllAboveArea,
    /// Retain only components containing immutable hard source demand.
    KeepDemandAnchored,
}

/// Immutable inputs for deterministic post-mask component cleanup.
pub struct MaskedTopologyCleanupInput<'a> {
    pub layout: &'a MaskPostprocLayout,
    /// Product cells that are legal to restore while connecting hard demand.
    pub allowed_before_cleanup: &'a [i32],
    /// Provisional mask after compatibility cleanup.
    pub provisional_active: &'a [i32],
    /// Exact hard source-demand coverage per one-based center id.
    pub hard_demand: &'a [bool],
    /// Optional one-based component seeds.
    pub seeds: &'a [bool],
    /// Optional one-based cell areas. Required by `KeepAllAboveArea`.
    pub cell_areas: &'a [f64],
    pub minimum_component_area: f64,
    pub retention: ComponentRetentionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaskedTopologyCleanupReport {
    pub active: Vec<i32>,
    pub removed_cells: Vec<usize>,
    pub restored_hard_demand_cells: Vec<usize>,
    pub connector_cells: Vec<usize>,
    pub retained_component_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainTopologyFailureKind {
    HardDemandOutsideProductSupport,
    RequiredComponentCannotBeConnected,
    NoRetainedCells,
    CompatibilityCleanupDidNotConverge,
}

#[derive(Debug)]
pub struct DomainTopologyFailure {
    kind: DomainTopologyFailureKind,
    center_id: Option<usize>,
    message: String,
}

impl DomainTopologyFailure {
    pub fn kind(&self) -> DomainTopologyFailureKind {
        self.kind
    }

    pub fn center_id(&self) -> Option<usize> {
        self.center_id
    }
}

impl fmt::Display for DomainTopologyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DomainTopologyFailure {}

pub fn domain_topology_failure(error: &io::Error) -> Option<&DomainTopologyFailure> {
    error.get_ref()?.downcast_ref::<DomainTopologyFailure>()
}

pub(crate) fn domain_topology_error(
    kind: DomainTopologyFailureKind,
    center_id: Option<usize>,
    message: impl Into<String>,
) -> io::Error {
    io::Error::other(DomainTopologyFailure {
        kind,
        center_id,
        message: message.into(),
    })
}

/// Resolve clipped-cell components without confusing actual refinement level
/// with immutable source demand.
///
/// The routine is deterministic:
///
/// 1. build exact edge adjacency from the unmodified product support;
/// 2. restore every missing hard-demand cell;
/// 3. connect required singleton/disconnected demand through the shortest
///    legal path (cell id breaks ties);
/// 4. apply the selected component policy, with hard demand taking priority;
/// 5. reject, rather than publish, any required singleton that has no legal
///    edge-connected neighbour.
pub fn cleanup_masked_topology_one_based(
    input: MaskedTopologyCleanupInput<'_>,
) -> io::Result<MaskedTopologyCleanupReport> {
    validate_input(&input)?;
    let cell_count = input.layout.ustr_points;
    let allowed = mask_to_bools(input.allowed_before_cleanup, cell_count);
    let mut active = mask_to_bools(input.provisional_active, cell_count);
    let adjacency = build_allowed_edge_adjacency(input.layout, &allowed)?;

    for center_id in 2..cell_count {
        if active[center_id] && !allowed[center_id] {
            return Err(invalid(format!(
                "provisional masked cell {center_id} is outside the legal product support"
            )));
        }
        if input.hard_demand[center_id] && !allowed[center_id] {
            return Err(domain_topology_error(
                DomainTopologyFailureKind::HardDemandOutsideProductSupport,
                Some(center_id),
                format!(
                    "immutable hard demand at center {center_id} lies outside the selected product support"
                ),
            ));
        }
    }

    let mut restored_hard_demand_cells = Vec::new();
    for center_id in 2..cell_count {
        if input.hard_demand[center_id] && !active[center_id] {
            active[center_id] = true;
            restored_hard_demand_cells.push(center_id);
        }
    }

    let mut connector_cells = BTreeSet::new();
    match input.retention {
        ComponentRetentionPolicy::KeepSeedConnected => {
            connect_hard_demand_to_seed_components(
                &mut active,
                &allowed,
                &adjacency,
                input.hard_demand,
                input.seeds,
                &mut connector_cells,
            )?;
        }
        _ => {
            connect_required_singletons(
                &mut active,
                &allowed,
                &adjacency,
                input.hard_demand,
                &mut connector_cells,
            )?;
        }
    }

    let components = active_components(&active, &adjacency);
    let mut keep_component = vec![false; components.len()];
    for (component_index, component) in components.iter().enumerate() {
        let has_hard_demand = component
            .iter()
            .any(|&center_id| input.hard_demand[center_id]);
        let policy_keeps = match input.retention {
            ComponentRetentionPolicy::KeepAllNonSingletons => component.len() >= 2,
            ComponentRetentionPolicy::KeepSeedConnected => {
                component.iter().any(|&center_id| input.seeds[center_id])
            }
            ComponentRetentionPolicy::KeepAllAboveArea => {
                compensated_component_area(component, input.cell_areas)
                    >= input.minimum_component_area
            }
            ComponentRetentionPolicy::KeepDemandAnchored => has_hard_demand,
        };
        keep_component[component_index] = has_hard_demand || policy_keeps;
    }

    let mut removed_cells = Vec::new();
    for (component_index, component) in components.iter().enumerate() {
        if keep_component[component_index] {
            continue;
        }
        for &center_id in component {
            active[center_id] = false;
            removed_cells.push(center_id);
        }
    }

    // A retained singleton is not simulation-ready. Non-required singletons
    // are removed; hard/seed/area-required singletons must be connected or
    // fail explicitly.
    loop {
        let components = active_components(&active, &adjacency);
        let Some(component) = components.iter().find(|component| component.len() == 1) else {
            break;
        };
        let center_id = component[0];
        let required = input.hard_demand[center_id]
            || matches!(input.retention, ComponentRetentionPolicy::KeepSeedConnected)
                && input.seeds[center_id]
            || matches!(input.retention, ComponentRetentionPolicy::KeepAllAboveArea)
                && input.cell_areas[center_id] >= input.minimum_component_area;
        if !required {
            active[center_id] = false;
            removed_cells.push(center_id);
            continue;
        }
        let path = shortest_path_to_other_cell(center_id, &allowed, &active, &adjacency)
            .ok_or_else(|| {
                domain_topology_error(
                    DomainTopologyFailureKind::RequiredComponentCannotBeConnected,
                    Some(center_id),
                    format!(
                        "required masked component at center {center_id} has no legal edge-connected path"
                    ),
                )
            })?;
        activate_path(&path, &mut active, &mut connector_cells);
    }

    for center_id in 2..cell_count {
        if input.hard_demand[center_id] && !active[center_id] {
            return Err(domain_topology_error(
                DomainTopologyFailureKind::RequiredComponentCannotBeConnected,
                Some(center_id),
                format!("immutable hard demand at center {center_id} was not retained"),
            ));
        }
    }
    let retained_components = active_components(&active, &adjacency);
    if retained_components.is_empty() {
        return Err(domain_topology_error(
            DomainTopologyFailureKind::NoRetainedCells,
            None,
            "masked topology cleanup retained no simulation cells",
        ));
    }
    if let Some(component) = retained_components
        .iter()
        .find(|component| component.len() == 1)
    {
        return Err(domain_topology_error(
            DomainTopologyFailureKind::RequiredComponentCannotBeConnected,
            component.first().copied(),
            format!(
                "masked topology cleanup retained orphan center {}",
                component[0]
            ),
        ));
    }

    removed_cells.sort_unstable();
    removed_cells.dedup();
    restored_hard_demand_cells.sort_unstable();
    let mut active_mask = input.provisional_active[..cell_count].to_vec();
    for center_id in 2..cell_count {
        active_mask[center_id] = if active[center_id] { 1 } else { -1 };
    }
    Ok(MaskedTopologyCleanupReport {
        active: active_mask,
        removed_cells,
        restored_hard_demand_cells,
        connector_cells: connector_cells.into_iter().collect(),
        retained_component_count: retained_components.len(),
    })
}

fn validate_input(input: &MaskedTopologyCleanupInput<'_>) -> io::Result<()> {
    let cell_count = input.layout.ustr_points;
    for (role, len) in [
        ("allowed product mask", input.allowed_before_cleanup.len()),
        ("provisional product mask", input.provisional_active.len()),
        ("hard-demand mask", input.hard_demand.len()),
        ("seed mask", input.seeds.len()),
    ] {
        if len < cell_count {
            return Err(invalid(format!(
                "{role} length {len} must cover {cell_count} one-based centers"
            )));
        }
    }
    if input.layout.center_neighbors.len() < cell_count
        || input.layout.center_neighbor_counts.len() < cell_count
    {
        return Err(invalid(
            "mask-postproc layout does not cover every center".to_string(),
        ));
    }
    if matches!(input.retention, ComponentRetentionPolicy::KeepAllAboveArea) {
        if input.cell_areas.len() < cell_count {
            return Err(invalid(format!(
                "cell-area length {} must cover {cell_count} one-based centers",
                input.cell_areas.len()
            )));
        }
        if !input.minimum_component_area.is_finite() || input.minimum_component_area < 0.0 {
            return Err(invalid(
                "minimum retained component area must be finite and nonnegative".to_string(),
            ));
        }
        if input.cell_areas[2..cell_count]
            .iter()
            .any(|area| !area.is_finite() || *area < 0.0)
        {
            return Err(invalid(
                "masked topology cell areas must be finite and nonnegative".to_string(),
            ));
        }
    }
    Ok(())
}

fn mask_to_bools(mask: &[i32], cell_count: usize) -> Vec<bool> {
    let mut active = vec![false; cell_count];
    for center_id in 2..cell_count {
        active[center_id] = mask[center_id] == 1;
    }
    active
}

pub(crate) fn build_allowed_edge_adjacency(
    layout: &MaskPostprocLayout,
    allowed: &[bool],
) -> io::Result<Vec<Vec<usize>>> {
    let mut edge_owner = BTreeMap::<(usize, usize), usize>::new();
    let mut adjacency = vec![Vec::new(); layout.ustr_points];
    for center_id in 2..layout.ustr_points {
        if !allowed[center_id] {
            continue;
        }
        let count = layout.center_neighbor_counts[center_id];
        let vertices = &layout.center_neighbors[center_id];
        if count < 3 || count > vertices.len() {
            return Err(invalid(format!(
                "masked cell {center_id} must contain at least three valid vertices"
            )));
        }
        let mut seen_edges = BTreeSet::new();
        for slot in 0..count {
            let a = vertices[slot];
            let b = vertices[(slot + 1) % count];
            if a <= 1 || b <= 1 || a == b {
                return Err(invalid(format!(
                    "masked cell {center_id} contains an invalid edge {a}-{b}"
                )));
            }
            let edge = (a.min(b), a.max(b));
            if !seen_edges.insert(edge) {
                return Err(invalid(format!(
                    "masked cell {center_id} repeats edge {}-{}",
                    edge.0, edge.1
                )));
            }
            if let Some(&other) = edge_owner.get(&edge) {
                if other == center_id {
                    continue;
                }
                adjacency[center_id].push(other);
                adjacency[other].push(center_id);
            } else {
                edge_owner.insert(edge, center_id);
            }
        }
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    Ok(adjacency)
}

fn active_components(active: &[bool], adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; active.len()];
    let mut components = Vec::new();
    for start in 2..active.len() {
        if !active[start] || seen[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        seen[start] = true;
        while let Some(center_id) = queue.pop_front() {
            component.push(center_id);
            for &neighbor in &adjacency[center_id] {
                if active[neighbor] && !seen[neighbor] {
                    seen[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components.sort();
    components
}

fn connect_hard_demand_to_seed_components(
    active: &mut [bool],
    allowed: &[bool],
    adjacency: &[Vec<usize>],
    hard_demand: &[bool],
    seeds: &[bool],
    connector_cells: &mut BTreeSet<usize>,
) -> io::Result<()> {
    let seed_targets = (2..active.len())
        .filter(|&center_id| active[center_id] && seeds[center_id])
        .collect::<BTreeSet<_>>();
    if seed_targets.is_empty() {
        return Err(domain_topology_error(
            DomainTopologyFailureKind::RequiredComponentCannotBeConnected,
            None,
            "KeepSeedConnected requires at least one active legal seed",
        ));
    }
    for center_id in 2..active.len() {
        if !hard_demand[center_id] {
            continue;
        }
        let active_seed_component = component_from(center_id, active, adjacency)
            .iter()
            .any(|member| seed_targets.contains(member));
        if active_seed_component {
            continue;
        }
        let path = shortest_path_to_targets(center_id, allowed, &seed_targets, adjacency)
            .ok_or_else(|| {
                domain_topology_error(
                    DomainTopologyFailureKind::RequiredComponentCannotBeConnected,
                    Some(center_id),
                    format!(
                        "hard-demand center {center_id} cannot reach a retained seed component"
                    ),
                )
            })?;
        activate_path(&path, active, connector_cells);
    }
    Ok(())
}

fn connect_required_singletons(
    active: &mut [bool],
    allowed: &[bool],
    adjacency: &[Vec<usize>],
    hard_demand: &[bool],
    connector_cells: &mut BTreeSet<usize>,
) -> io::Result<()> {
    for center_id in 2..active.len() {
        if !hard_demand[center_id] {
            continue;
        }
        let component = component_from(center_id, active, adjacency);
        if component.len() >= 2 {
            continue;
        }
        let path = shortest_path_to_other_cell(center_id, allowed, active, adjacency).ok_or_else(
            || {
                domain_topology_error(
                    DomainTopologyFailureKind::RequiredComponentCannotBeConnected,
                    Some(center_id),
                    format!("hard-demand center {center_id} has no legal edge-connected neighbour"),
                )
            },
        )?;
        activate_path(&path, active, connector_cells);
    }
    Ok(())
}

fn component_from(start: usize, active: &[bool], adjacency: &[Vec<usize>]) -> Vec<usize> {
    if !active[start] {
        return Vec::new();
    }
    let mut seen = vec![false; active.len()];
    let mut queue = VecDeque::from([start]);
    let mut component = Vec::new();
    seen[start] = true;
    while let Some(center_id) = queue.pop_front() {
        component.push(center_id);
        for &neighbor in &adjacency[center_id] {
            if active[neighbor] && !seen[neighbor] {
                seen[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    component
}

fn shortest_path_to_other_cell(
    start: usize,
    allowed: &[bool],
    active: &[bool],
    adjacency: &[Vec<usize>],
) -> Option<Vec<usize>> {
    let active_targets = (2..active.len())
        .filter(|&center_id| center_id != start && active[center_id])
        .collect::<BTreeSet<_>>();
    if !active_targets.is_empty() {
        if let Some(path) = shortest_path_to_targets(start, allowed, &active_targets, adjacency) {
            return Some(path);
        }
    }
    let legal_targets = (2..allowed.len())
        .filter(|&center_id| center_id != start && allowed[center_id])
        .collect::<BTreeSet<_>>();
    shortest_path_to_targets(start, allowed, &legal_targets, adjacency)
}

fn shortest_path_to_targets(
    start: usize,
    allowed: &[bool],
    targets: &BTreeSet<usize>,
    adjacency: &[Vec<usize>],
) -> Option<Vec<usize>> {
    if start >= allowed.len() || !allowed[start] || targets.is_empty() {
        return None;
    }
    let mut parent = vec![usize::MAX; allowed.len()];
    let mut queue = VecDeque::from([start]);
    parent[start] = start;
    let mut found = None;
    while let Some(center_id) = queue.pop_front() {
        if center_id != start && targets.contains(&center_id) {
            found = Some(center_id);
            break;
        }
        for &neighbor in &adjacency[center_id] {
            if allowed[neighbor] && parent[neighbor] == usize::MAX {
                parent[neighbor] = center_id;
                queue.push_back(neighbor);
            }
        }
    }
    let mut current = found?;
    let mut path = vec![current];
    while current != start {
        current = parent[current];
        path.push(current);
    }
    path.reverse();
    Some(path)
}

fn activate_path(path: &[usize], active: &mut [bool], connector_cells: &mut BTreeSet<usize>) {
    for &center_id in path {
        if !active[center_id] {
            active[center_id] = true;
            connector_cells.insert(center_id);
        }
    }
}

fn compensated_component_area(component: &[usize], cell_areas: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for &center_id in component {
        let value = cell_areas[center_id] - correction;
        let next = sum + value;
        correction = (next - sum) - value;
        sum = next;
    }
    sum
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LonLatPoint;

    fn chain_layout(cell_count: usize) -> MaskPostprocLayout {
        let ustr_points = cell_count + 2;
        let mut center_neighbors = vec![Vec::new(); ustr_points];
        let mut center_neighbor_counts = vec![0; ustr_points];
        // Each consecutive pair shares edge (11+i, 100).
        for offset in 0..cell_count {
            let center_id = offset + 2;
            center_neighbors[center_id] = vec![10 + offset, 11 + offset, 100];
            center_neighbor_counts[center_id] = 3;
        }
        MaskPostprocLayout {
            ustr_points,
            ustr_bounds: 200 + cell_count,
            center_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; ustr_points],
            vertex_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 200 + cell_count],
            center_neighbors,
            vertex_neighbors: vec![Vec::new(); 200 + cell_count],
            center_neighbor_counts,
            vertex_neighbor_counts: vec![0; 200 + cell_count],
        }
    }

    fn input<'a>(
        layout: &'a MaskPostprocLayout,
        allowed: &'a [i32],
        active: &'a [i32],
        hard: &'a [bool],
        retention: ComponentRetentionPolicy,
        seeds: &'a [bool],
        areas: &'a [f64],
        minimum_area: f64,
    ) -> MaskedTopologyCleanupInput<'a> {
        MaskedTopologyCleanupInput {
            layout,
            allowed_before_cleanup: allowed,
            provisional_active: active,
            hard_demand: hard,
            seeds,
            cell_areas: areas,
            minimum_component_area: minimum_area,
            retention,
        }
    }

    #[test]
    fn non_demand_orphan_is_removed() {
        let layout = chain_layout(3);
        let allowed = [0, 0, 1, 1, 1];
        let active = [0, 0, 1, -1, 1];
        let report = cleanup_masked_topology_one_based(input(
            &layout,
            &allowed,
            &active,
            &[false; 5],
            ComponentRetentionPolicy::KeepAllNonSingletons,
            &[false; 5],
            &[],
            0.0,
        ))
        .expect_err("all active cells are non-required singletons");
        assert_eq!(
            domain_topology_failure(&report).unwrap().kind(),
            DomainTopologyFailureKind::NoRetainedCells
        );
    }

    #[test]
    fn hard_orphan_uses_shortest_legal_connector() {
        let layout = chain_layout(4);
        let allowed = [0, 0, 1, 1, 1, 1];
        let active = [0, 0, 1, -1, -1, 1];
        let hard = [false, false, true, false, false, false];
        let report = cleanup_masked_topology_one_based(input(
            &layout,
            &allowed,
            &active,
            &hard,
            ComponentRetentionPolicy::KeepAllNonSingletons,
            &[false; 6],
            &[],
            0.0,
        ))
        .unwrap();
        assert_eq!(report.connector_cells, vec![3, 4]);
        assert_eq!(report.active[2..], [1, 1, 1, 1]);
    }

    #[test]
    fn hard_orphan_without_legal_neighbour_is_typed_failure() {
        let mut layout = chain_layout(2);
        layout.center_neighbors[3] = vec![20, 21, 22];
        let error = cleanup_masked_topology_one_based(input(
            &layout,
            &[0, 0, 1, -1],
            &[0, 0, 1, -1],
            &[false, false, true, false],
            ComponentRetentionPolicy::KeepDemandAnchored,
            &[false; 4],
            &[],
            0.0,
        ))
        .unwrap_err();
        let failure = domain_topology_failure(&error).unwrap();
        assert_eq!(
            failure.kind(),
            DomainTopologyFailureKind::RequiredComponentCannotBeConnected
        );
        assert_eq!(failure.center_id(), Some(2));
    }

    #[test]
    fn seed_policy_connects_hard_demand_to_seed_component() {
        let layout = chain_layout(4);
        let allowed = [0, 0, 1, 1, 1, 1];
        let active = [0, 0, 1, -1, -1, 1];
        let hard = [false, false, true, false, false, false];
        let seeds = [false, false, false, false, false, true];
        let report = cleanup_masked_topology_one_based(input(
            &layout,
            &allowed,
            &active,
            &hard,
            ComponentRetentionPolicy::KeepSeedConnected,
            &seeds,
            &[],
            0.0,
        ))
        .unwrap();
        assert_eq!(report.connector_cells, vec![3, 4]);
        assert_eq!(report.retained_component_count, 1);
    }

    #[test]
    fn area_policy_keeps_large_component_and_drops_small_component() {
        let mut layout = chain_layout(5);
        layout.center_neighbors[5] = vec![30, 31, 32];
        layout.center_neighbors[6] = vec![31, 32, 33];
        let allowed = [0, 0, 1, 1, 1, 1, 1];
        let active = allowed;
        let report = cleanup_masked_topology_one_based(input(
            &layout,
            &allowed,
            &active,
            &[false; 7],
            ComponentRetentionPolicy::KeepAllAboveArea,
            &[false; 7],
            &[0.0, 0.0, 2.0, 2.0, 2.0, 1.0, 1.0],
            5.0,
        ))
        .unwrap();
        assert_eq!(report.active[2..], [1, 1, 1, -1, -1]);
        assert_eq!(report.removed_cells, vec![5, 6]);
    }

    #[test]
    fn demand_priority_overrides_area_policy() {
        let mut layout = chain_layout(5);
        layout.center_neighbors[5] = vec![30, 31, 32];
        layout.center_neighbors[6] = vec![31, 32, 33];
        let allowed = [0, 0, 1, 1, 1, 1, 1];
        let active = allowed;
        let hard = [false, false, false, false, false, true, false];
        let report = cleanup_masked_topology_one_based(input(
            &layout,
            &allowed,
            &active,
            &hard,
            ComponentRetentionPolicy::KeepAllAboveArea,
            &[false; 7],
            &[0.0, 0.0, 2.0, 2.0, 2.0, 1.0, 1.0],
            5.0,
        ))
        .unwrap();
        assert_eq!(report.active[5..], [1, 1]);
        assert_eq!(report.retained_component_count, 2);
    }
}
