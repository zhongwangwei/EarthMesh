//! Topology validator + light repair hooks for EarthMesh v3.
//!
//! The validator turns the mesh connectivity into structured [`TopologyIssue`]s
//! (type / severity / ids / message / suggested fix). Repair hooks operate **only on
//! refinement target levels** (a `Vec<u32>` + neighbor lists) — they never rewrite the
//! mesh structure, so they are a stable entry point for future repair without the
//! large topology-surgery this MVP deliberately avoids. Catastrophic connectivity is
//! `Severity::Fail`; refinement/transition degradation is `Severity::Warn`.

use crate::QualityMeshInput;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Per-category cap so a broken mesh cannot emit unbounded issues.
pub const MAX_ISSUES_PER_TYPE: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Catastrophic invalid connectivity — the mesh cannot be trusted.
    Fail,
    /// Quality degradation — usable but suspicious / repairable.
    Warn,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Fail => "fail",
            Severity::Warn => "warn",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopologyIssueType {
    InvalidVertexIndex,
    InvalidCellIndex,
    CellVertexIncidence,
    DuplicateEdge,
    DanglingEdge,
    MisorientedSharedEdge,
    NeighborDegreeMismatch,
    OrphanCell,
    NonreciprocalNeighbor,
    AbnormalPolygonEdgeCount,
    InvalidRefinementLevel,
    TransitionDiscontinuity,
    DisconnectedMesh,
    NonManifoldVertexFan,
    BoundaryVertexDegree,
    EulerCharacteristicMismatch,
}

impl TopologyIssueType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TopologyIssueType::InvalidVertexIndex => "invalid_vertex_index",
            TopologyIssueType::InvalidCellIndex => "invalid_cell_index",
            TopologyIssueType::CellVertexIncidence => "cell_vertex_incidence",
            TopologyIssueType::DuplicateEdge => "duplicate_edge",
            TopologyIssueType::DanglingEdge => "dangling_edge",
            TopologyIssueType::MisorientedSharedEdge => "misoriented_shared_edge",
            TopologyIssueType::NeighborDegreeMismatch => "neighbor_degree_mismatch",
            TopologyIssueType::OrphanCell => "orphan_cell",
            TopologyIssueType::NonreciprocalNeighbor => "nonreciprocal_neighbor",
            TopologyIssueType::AbnormalPolygonEdgeCount => "abnormal_polygon_edge_count",
            TopologyIssueType::InvalidRefinementLevel => "invalid_refinement_level",
            TopologyIssueType::TransitionDiscontinuity => "transition_discontinuity",
            TopologyIssueType::DisconnectedMesh => "disconnected_mesh",
            TopologyIssueType::NonManifoldVertexFan => "non_manifold_vertex_fan",
            TopologyIssueType::BoundaryVertexDegree => "boundary_vertex_degree",
            TopologyIssueType::EulerCharacteristicMismatch => "euler_characteristic_mismatch",
        }
    }
    /// Connectivity errors are catastrophic (Fail); refinement issues degrade (Warn).
    pub fn default_severity(&self) -> Severity {
        use TopologyIssueType::*;
        match self {
            TransitionDiscontinuity => Severity::Warn,
            _ => Severity::Fail,
        }
    }
}

fn valid_edge_cells(mesh: &QualityMeshInput) -> BTreeMap<(usize, usize), Vec<usize>> {
    let mut edge_cells = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (ci, cell) in mesh.cells.iter().enumerate() {
        for k in 0..cell.vertices.len() {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % cell.vertices.len()];
            if a < mesh.vertices.len() && b < mesh.vertices.len() && a != b {
                edge_cells.entry(edge_key(a, b)).or_default().push(ci);
            }
        }
    }
    edge_cells
}

/// Boundary graph derived from edges with exactly one incident cell.
///
/// Each loop is an open vertex ring (the first vertex is not repeated), starts
/// at its smallest vertex id, and is deterministic. Components containing a
/// vertex whose boundary degree is not two are reported but not emitted as
/// loops.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoundaryTopology {
    pub edge_count: usize,
    pub loops: Vec<Vec<usize>>,
    pub invalid_vertex_degrees: Vec<(usize, usize)>,
}

pub fn boundary_topology(mesh: &QualityMeshInput) -> BoundaryTopology {
    let boundary_edges = valid_edge_cells(mesh)
        .into_iter()
        .filter_map(|(edge, cells)| (cells.len() == 1).then_some(edge))
        .collect::<Vec<_>>();
    let mut adjacency = BTreeMap::<usize, Vec<usize>>::new();
    for &(a, b) in &boundary_edges {
        adjacency.entry(a).or_default().push(b);
        adjacency.entry(b).or_default().push(a);
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }

    let invalid_vertex_degrees = adjacency
        .iter()
        .filter_map(|(&vertex, neighbors)| {
            (neighbors.len() != 2).then_some((vertex, neighbors.len()))
        })
        .collect::<Vec<_>>();
    let mut loops = Vec::new();
    let mut seen = BTreeSet::new();
    for &start in adjacency.keys() {
        if seen.contains(&start) {
            continue;
        }
        let mut component = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(vertex) = queue.pop_front() {
            for &next in &adjacency[&vertex] {
                if component.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        seen.extend(component.iter().copied());
        if component.iter().any(|vertex| adjacency[vertex].len() != 2) {
            continue;
        }

        let mut ring = Vec::with_capacity(component.len());
        let mut ring_vertices = BTreeSet::new();
        let mut previous = None;
        let mut current = start;
        loop {
            if !ring_vertices.insert(current) {
                ring.clear();
                break;
            }
            ring.push(current);
            let neighbors = &adjacency[&current];
            let next = neighbors
                .iter()
                .copied()
                .find(|candidate| Some(*candidate) != previous)
                .unwrap_or(neighbors[0]);
            if next == start {
                break;
            }
            previous = Some(current);
            current = next;
        }
        if ring.len() == component.len() {
            loops.push(ring);
        }
    }

    BoundaryTopology {
        edge_count: boundary_edges.len(),
        loops,
        invalid_vertex_degrees,
    }
}

/// Expected Euler characteristic for a manifold genus-zero spherical domain.
///
/// Returns `None` when invalid rings, non-manifold edges/fans, or an open
/// boundary make the boundary count insufficient. For `C` cell components and
/// `B` closed boundary loops, `χ = 2*C - B`; a global closed sphere is therefore
/// `χ = 2`.
pub fn genus_zero_euler_expectation(
    mesh: &QualityMeshInput,
    boundary: &BoundaryTopology,
) -> Option<isize> {
    let edge_cells = valid_edge_cells(mesh);
    if mesh.cells.is_empty()
        || !boundary.invalid_vertex_degrees.is_empty()
        || mesh.cells.iter().any(|cell| {
            let distinct = cell.vertices.iter().copied().collect::<BTreeSet<_>>();
            cell.vertices.len() < 3
                || distinct.len() != cell.vertices.len()
                || distinct.iter().any(|&vertex| vertex >= mesh.vertices.len())
        })
        || edge_cells.values().any(|incidents| incidents.len() > 2)
        || !non_manifold_vertex_fans_from_edges(mesh, &edge_cells).is_empty()
    {
        return None;
    }
    Some(
        2 * connected_component_count_from_edges(mesh, &edge_cells) as isize
            - isize::try_from(boundary.loops.len()).ok()?,
    )
}

/// Euler characteristic of the represented cell complex (`used vertices - edges + cells`).
///
/// This is informational because the expected value depends on whether the mesh is
/// global, regional, holed, or disconnected.
pub fn euler_characteristic(mesh: &QualityMeshInput) -> isize {
    let mut used_vertices = BTreeSet::new();
    let mut valid_cells = 0isize;
    for cell in &mesh.cells {
        let distinct: BTreeSet<_> = cell
            .vertices
            .iter()
            .copied()
            .filter(|&v| v < mesh.vertices.len())
            .collect();
        if distinct.len() >= 3 {
            valid_cells += 1;
            used_vertices.extend(distinct);
        }
    }
    used_vertices.len() as isize - valid_edge_cells(mesh).len() as isize + valid_cells
}

/// Number of edge-connected cell components. Cells touching only at a vertex are
/// deliberately separate: such a vertex is a non-manifold fan junction.
pub fn connected_component_count(mesh: &QualityMeshInput) -> usize {
    connected_component_count_from_edges(mesh, &valid_edge_cells(mesh))
}

fn connected_component_count_from_edges(
    mesh: &QualityMeshInput,
    edge_cells: &BTreeMap<(usize, usize), Vec<usize>>,
) -> usize {
    if mesh.cells.is_empty() {
        return 0;
    }
    let mut adjacency = vec![Vec::<usize>::new(); mesh.cells.len()];
    for cells in edge_cells.values() {
        for &a in cells {
            for &b in cells {
                if a != b && !adjacency[a].contains(&b) {
                    adjacency[a].push(b);
                }
            }
        }
    }
    let mut seen = vec![false; mesh.cells.len()];
    let mut components = 0;
    for start in 0..mesh.cells.len() {
        if seen[start] {
            continue;
        }
        components += 1;
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(ci) = queue.pop_front() {
            for &next in &adjacency[ci] {
                if !seen[next] {
                    seen[next] = true;
                    queue.push_back(next);
                }
            }
        }
    }
    components
}

fn non_manifold_vertex_fans(mesh: &QualityMeshInput) -> Vec<(usize, usize)> {
    let edge_cells = valid_edge_cells(mesh);
    non_manifold_vertex_fans_from_edges(mesh, &edge_cells)
}

fn non_manifold_vertex_fans_from_edges(
    mesh: &QualityMeshInput,
    edge_cells: &BTreeMap<(usize, usize), Vec<usize>>,
) -> Vec<(usize, usize)> {
    let mut incidents = vec![Vec::<usize>::new(); mesh.vertices.len()];
    for (ci, cell) in mesh.cells.iter().enumerate() {
        for &vertex in &cell.vertices {
            if vertex < mesh.vertices.len() && !incidents[vertex].contains(&ci) {
                incidents[vertex].push(ci);
            }
        }
    }
    let mut connections = vec![Vec::<(usize, usize)>::new(); mesh.vertices.len()];
    for (&(a, b), edge_incidents) in edge_cells {
        for (index, &left) in edge_incidents.iter().enumerate() {
            for &right in &edge_incidents[index + 1..] {
                connections[a].push((left, right));
                connections[b].push((left, right));
            }
        }
    }
    let mut broken = Vec::new();
    for (vertex, cells) in incidents.iter().enumerate() {
        if cells.len() <= 1 {
            continue;
        }
        let start = cells[0];
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(ci) = queue.pop_front() {
            for &(left, right) in &connections[vertex] {
                let next = if left == ci {
                    right
                } else if right == ci {
                    left
                } else {
                    continue;
                };
                if seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        if seen.len() != cells.len() {
            broken.push((vertex, start));
        }
    }
    broken
}

pub fn non_manifold_vertex_fan_count(mesh: &QualityMeshInput) -> usize {
    non_manifold_vertex_fans(mesh).len()
}

/// One structured topology problem.
#[derive(Clone, Debug)]
pub struct TopologyIssue {
    pub issue_type: TopologyIssueType,
    pub severity: Severity,
    pub cell_id: Option<usize>,
    pub edge_id: Option<(usize, usize)>,
    pub vertex_id: Option<usize>,
    pub message: String,
    pub suggested_fix: String,
}

impl TopologyIssue {
    fn new(
        issue_type: TopologyIssueType,
        cell_id: Option<usize>,
        edge_id: Option<(usize, usize)>,
        vertex_id: Option<usize>,
        message: impl Into<String>,
        suggested_fix: impl Into<String>,
    ) -> Self {
        Self {
            issue_type,
            severity: issue_type.default_severity(),
            cell_id,
            edge_id,
            vertex_id,
            message: message.into(),
            suggested_fix: suggested_fix.into(),
        }
    }
}

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn push_capped(out: &mut Vec<TopologyIssue>, start: usize, issue: TopologyIssue) {
    if out.len() - start < MAX_ISSUES_PER_TYPE {
        out.push(issue);
    }
}

/// Stateless validator over a [`QualityMeshInput`].
pub struct MeshTopologyValidator<'a> {
    pub mesh: &'a QualityMeshInput,
    /// Max adjacent refinement-level jump tolerated before a transition warning.
    pub max_adjacent_level_jump: u32,
}

impl<'a> MeshTopologyValidator<'a> {
    pub fn new(mesh: &'a QualityMeshInput) -> Self {
        Self {
            mesh,
            max_adjacent_level_jump: 1,
        }
    }

    fn nv(&self) -> usize {
        self.mesh.vertices.len()
    }
    fn nc(&self) -> usize {
        self.mesh.cells.len()
    }

    pub fn validate_indices(&self) -> Vec<TopologyIssue> {
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            for &v in &cell.vertices {
                if v >= self.nv() {
                    push_capped(
                        &mut issues,
                        0,
                        TopologyIssue::new(
                            TopologyIssueType::InvalidVertexIndex,
                            Some(ci),
                            None,
                            Some(v),
                            format!("cell {ci} uses vertex {v} (>= {})", self.nv()),
                            "drop the cell or remap to a valid vertex index",
                        ),
                    );
                }
            }
            for &n in &cell.neighbors {
                if n >= self.nc() {
                    issues.push(TopologyIssue::new(
                        TopologyIssueType::InvalidCellIndex,
                        Some(ci),
                        None,
                        None,
                        format!("cell {ci} lists neighbor {n} (>= {})", self.nc()),
                        "drop the stale neighbor id",
                    ));
                }
            }
        }
        issues
    }

    /// Degenerate / out-of-range edges (broken connectivity).
    pub fn validate_edges(&self) -> Vec<TopologyIssue> {
        self.validate_dangling_edges()
    }

    pub fn validate_dangling_edges(&self) -> Vec<TopologyIssue> {
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let m = cell.vertices.len();
            for k in 0..m {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                if a >= self.nv() || b >= self.nv() || a == b {
                    push_capped(
                        &mut issues,
                        0,
                        TopologyIssue::new(
                            TopologyIssueType::DanglingEdge,
                            Some(ci),
                            Some((a, b)),
                            None,
                            format!("cell {ci} has a degenerate/out-of-range edge ({a},{b})"),
                            "drop the degenerate edge / fix the ring",
                        ),
                    );
                }
            }
        }
        issues
    }

    pub fn validate_duplicate_edges(&self) -> Vec<TopologyIssue> {
        let mut edge_cells: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let m = cell.vertices.len();
            for k in 0..m {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                if a < self.nv() && b < self.nv() && a != b {
                    edge_cells.entry(edge_key(a, b)).or_default().push(ci);
                }
            }
        }
        let mut issues = Vec::new();
        for (edge, cells) in edge_cells {
            if cells.len() > 2 {
                push_capped(
                    &mut issues,
                    0,
                    TopologyIssue::new(
                        TopologyIssueType::DuplicateEdge,
                        cells.first().copied(),
                        Some(edge),
                        None,
                        format!(
                            "edge {edge:?} shared by {} cells (non-manifold)",
                            cells.len()
                        ),
                        "split / rebuild the non-manifold edge",
                    ),
                );
            }
        }
        issues
    }

    pub fn validate_shared_edge_orientation(&self) -> Vec<TopologyIssue> {
        type EdgeKey = (usize, usize);
        type DirectedEdgeUse = (usize, usize, usize);
        let mut edge_orientations: BTreeMap<EdgeKey, Vec<DirectedEdgeUse>> = BTreeMap::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let m = cell.vertices.len();
            for k in 0..m {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                if a < self.nv() && b < self.nv() && a != b {
                    edge_orientations
                        .entry(edge_key(a, b))
                        .or_default()
                        .push((ci, a, b));
                }
            }
        }
        let mut issues = Vec::new();
        for (edge, occ) in edge_orientations {
            if occ.len() == 2 && occ[0].1 == occ[1].1 && occ[0].2 == occ[1].2 {
                push_capped(
                    &mut issues,
                    0,
                    TopologyIssue::new(
                        TopologyIssueType::MisorientedSharedEdge,
                        Some(occ[0].0),
                        Some(edge),
                        None,
                        format!(
                            "edge {edge:?} has the same direction in cells {} and {}",
                            occ[0].0, occ[1].0
                        ),
                        "rewind one incident cell so shared edges are opposite",
                    ),
                );
            }
        }
        issues
    }

    pub fn validate_closed_cell_neighbors(&self) -> Vec<TopologyIssue> {
        let mut edge_cells: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let m = cell.vertices.len();
            for k in 0..m {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                if a < self.nv() && b < self.nv() && a != b {
                    edge_cells.entry(edge_key(a, b)).or_default().push(ci);
                }
            }
        }

        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let m = cell.vertices.len();
            if m < 3 {
                continue;
            }
            let mut derived = Vec::new();
            let mut closed = true;
            for k in 0..m {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                let Some(cells) = edge_cells.get(&edge_key(a, b)) else {
                    closed = false;
                    break;
                };
                if cells.len() != 2 {
                    closed = false;
                    break;
                }
                if let Some(&other) = cells.iter().find(|&&other| other != ci) {
                    if !derived.contains(&other) {
                        derived.push(other);
                    }
                } else {
                    closed = false;
                    break;
                }
            }
            if !closed {
                continue;
            }
            derived.sort_unstable();
            let mut declared: Vec<usize> = cell
                .neighbors
                .iter()
                .copied()
                .filter(|&nb| nb < self.nc() && nb != ci)
                .collect();
            declared.sort_unstable();
            declared.dedup();
            if declared != derived {
                push_capped(
                    &mut issues,
                    0,
                    TopologyIssue::new(
                        TopologyIssueType::NeighborDegreeMismatch,
                        Some(ci),
                        None,
                        None,
                        format!(
                            "cell {ci} declares neighbors {declared:?} but edge topology gives {derived:?}"
                        ),
                        "derive neighbors from shared edges or fix the stale neighbor list",
                    ),
                );
            }
        }
        issues
    }

    pub fn validate_neighbors(&self) -> Vec<TopologyIssue> {
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            for &nb in &cell.neighbors {
                if nb < self.nc() && !self.mesh.cells[nb].neighbors.contains(&ci) {
                    push_capped(
                        &mut issues,
                        0,
                        TopologyIssue::new(
                            TopologyIssueType::NonreciprocalNeighbor,
                            Some(ci),
                            None,
                            None,
                            format!(
                                "cell {ci} lists {nb} as neighbor but {nb} does not reciprocate"
                            ),
                            "make adjacency symmetric (add ci to cell nb's neighbors)",
                        ),
                    );
                }
            }
        }
        issues
    }

    /// Each cell must canonical at least 3 distinct, in-range vertices.
    pub fn validate_cell_vertex_incidence(&self) -> Vec<TopologyIssue> {
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let mut distinct = Vec::new();
            for &v in &cell.vertices {
                if v < self.nv() && !distinct.contains(&v) {
                    distinct.push(v);
                }
            }
            if distinct.len() < 3 {
                push_capped(
                    &mut issues,
                    0,
                    TopologyIssue::new(
                        TopologyIssueType::CellVertexIncidence,
                        Some(ci),
                        None,
                        None,
                        format!(
                            "cell {ci} has only {} distinct valid vertices",
                            distinct.len()
                        ),
                        "drop the degenerate cell",
                    ),
                );
            }
        }
        issues
    }

    pub fn validate_polygon_edge_counts(&self) -> Vec<TopologyIssue> {
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            if cell.vertices.len() < 3 {
                push_capped(
                    &mut issues,
                    0,
                    TopologyIssue::new(
                        TopologyIssueType::AbnormalPolygonEdgeCount,
                        Some(ci),
                        None,
                        None,
                        format!("cell {ci} has {} edges (< 3)", cell.vertices.len()),
                        "drop or rebuild the polygon",
                    ),
                );
            }
        }
        issues
    }

    pub fn validate_orphan_cells(&self) -> Vec<TopologyIssue> {
        let mut edge_cells: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let m = cell.vertices.len();
            for k in 0..m {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                if a < self.nv() && b < self.nv() && a != b {
                    edge_cells.entry(edge_key(a, b)).or_default().push(ci);
                }
            }
        }
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            if cell.vertices.len() < 3 {
                continue;
            }
            let m = cell.vertices.len();
            let shares = (0..m).any(|k| {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % m];
                a < self.nv()
                    && b < self.nv()
                    && a != b
                    && edge_cells
                        .get(&edge_key(a, b))
                        .map(|cs| cs.iter().any(|&o| o != ci))
                        .unwrap_or(false)
            });
            if !shares {
                push_capped(
                    &mut issues,
                    0,
                    TopologyIssue::new(
                        TopologyIssueType::OrphanCell,
                        Some(ci),
                        None,
                        None,
                        format!("cell {ci} shares no edge with any other cell"),
                        "reconnect or drop the isolated cell",
                    ),
                );
            }
        }
        issues
    }

    /// Refinement levels must be consistent (all cells carry a level, or none do).
    pub fn validate_refinement_levels(&self) -> Vec<TopologyIssue> {
        let with = self
            .mesh
            .cells
            .iter()
            .filter(|c| c.refine_level.is_some())
            .count();
        let mut issues = Vec::new();
        if with != 0 && with != self.nc() {
            issues.push(TopologyIssue::new(
                TopologyIssueType::InvalidRefinementLevel,
                None,
                None,
                None,
                format!(
                    "{with}/{} cells carry a refine_level (inconsistent)",
                    self.nc()
                ),
                "assign a level to every cell or none",
            ));
        }
        issues
    }

    pub fn validate_transition_continuity(&self) -> Vec<TopologyIssue> {
        let mut issues = Vec::new();
        for (ci, cell) in self.mesh.cells.iter().enumerate() {
            let Some(la) = cell.refine_level else {
                continue;
            };
            for &nb in &cell.neighbors {
                let Some(other) = self.mesh.cells.get(nb) else {
                    continue;
                };
                let Some(lb) = other.refine_level else {
                    continue;
                };
                if la.abs_diff(lb) > self.max_adjacent_level_jump {
                    push_capped(
                        &mut issues,
                        0,
                        TopologyIssue::new(
                            TopologyIssueType::TransitionDiscontinuity,
                            Some(ci),
                            None,
                            None,
                            format!(
                                "cells {ci}/{nb} differ by {} refinement levels (> {})",
                                la.abs_diff(lb),
                                self.max_adjacent_level_jump
                            ),
                            "insert transition cells or smooth target levels",
                        ),
                    );
                }
            }
        }
        issues
    }

    pub fn validate_connected_components(&self) -> Vec<TopologyIssue> {
        let count = connected_component_count(self.mesh);
        if count <= 1 {
            return Vec::new();
        }
        vec![TopologyIssue::new(
            TopologyIssueType::DisconnectedMesh,
            None,
            None,
            None,
            format!("mesh contains {count} edge-connected cell components"),
            "connect the components or export them as separate meshes",
        )]
    }

    pub fn validate_non_manifold_vertex_fans(&self) -> Vec<TopologyIssue> {
        non_manifold_vertex_fans(self.mesh)
            .into_iter()
            .take(MAX_ISSUES_PER_TYPE)
            .map(|(vertex, cell)| {
                TopologyIssue::new(
                    TopologyIssueType::NonManifoldVertexFan,
                    Some(cell),
                    None,
                    Some(vertex),
                    format!("vertex {vertex} has multiple disconnected incident-cell fans"),
                    "split the vertex so each incident-cell fan has its own vertex id",
                )
            })
            .collect()
    }

    pub fn validate_boundary_loops(&self) -> Vec<TopologyIssue> {
        self.boundary_degree_issues(&boundary_topology(self.mesh))
    }

    fn boundary_degree_issues(&self, boundary: &BoundaryTopology) -> Vec<TopologyIssue> {
        boundary
            .invalid_vertex_degrees
            .iter()
            .copied()
            .take(MAX_ISSUES_PER_TYPE)
            .map(|(vertex, degree)| {
                TopologyIssue::new(
                    TopologyIssueType::BoundaryVertexDegree,
                    None,
                    None,
                    Some(vertex),
                    format!("boundary vertex {vertex} has degree {degree}, expected 2"),
                    "close the boundary chain or split the boundary junction",
                )
            })
            .collect()
    }

    pub fn validate_genus_zero_euler(&self) -> Vec<TopologyIssue> {
        let boundary = boundary_topology(self.mesh);
        self.genus_zero_euler_issues(&boundary)
    }

    fn genus_zero_euler_issues(&self, boundary: &BoundaryTopology) -> Vec<TopologyIssue> {
        let Some(expected) = genus_zero_euler_expectation(self.mesh, boundary) else {
            return Vec::new();
        };
        let actual = euler_characteristic(self.mesh);
        if actual == expected {
            return Vec::new();
        }
        vec![TopologyIssue::new(
            TopologyIssueType::EulerCharacteristicMismatch,
            None,
            None,
            None,
            format!(
                "Euler characteristic {actual} differs from genus-zero boundary expectation {expected} (components={}, boundary_loops={})",
                connected_component_count(self.mesh),
                boundary.loops.len()
            ),
            "repair missing/duplicate cells or boundary connectivity",
        )]
    }

    pub fn validate_boundary_contract(&self) -> Vec<TopologyIssue> {
        let boundary = boundary_topology(self.mesh);
        let mut issues = self.boundary_degree_issues(&boundary);
        issues.extend(self.genus_zero_euler_issues(&boundary));
        issues
    }

    /// Run every validator. Issues are capped per type ([`MAX_ISSUES_PER_TYPE`]).
    pub fn validate_all(&self) -> Vec<TopologyIssue> {
        let mut all = Vec::new();
        all.extend(self.validate_indices());
        all.extend(self.validate_dangling_edges());
        all.extend(self.validate_duplicate_edges());
        all.extend(self.validate_shared_edge_orientation());
        all.extend(self.validate_closed_cell_neighbors());
        all.extend(self.validate_neighbors());
        all.extend(self.validate_cell_vertex_incidence());
        all.extend(self.validate_polygon_edge_counts());
        all.extend(self.validate_orphan_cells());
        all.extend(self.validate_refinement_levels());
        all.extend(self.validate_transition_continuity());
        all.extend(self.validate_connected_components());
        all.extend(self.validate_non_manifold_vertex_fans());
        all.extend(self.validate_boundary_contract());
        all
    }
}

/// Worst severity among issues mapped to a coarse pass/warn/fail string.
pub fn worst_severity(issues: &[TopologyIssue]) -> Option<Severity> {
    if issues.iter().any(|i| i.severity == Severity::Fail) {
        Some(Severity::Fail)
    } else if issues.iter().any(|i| i.severity == Severity::Warn) {
        Some(Severity::Warn)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Repair hooks — operate on refinement target levels only (no mesh-structure rewrite).
// ---------------------------------------------------------------------------

/// What a repair pass changed; serialize with [`RepairReport::to_json`] / `to_md`.
#[derive(Clone, Debug, Default)]
pub struct RepairReport {
    /// `(action, cells_changed)` per hook.
    pub actions: Vec<(String, usize)>,
    /// Issues that level-repair cannot fix (catastrophic connectivity).
    pub unrepairable: Vec<TopologyIssue>,
}

impl RepairReport {
    pub fn total_changed(&self) -> usize {
        self.actions.iter().map(|(_, n)| n).sum()
    }

    pub fn to_json(&self) -> String {
        let mut s = String::from("{\n  \"kind\": \"earthmesh_repair_report\",\n  \"actions\": [\n");
        for (i, (a, n)) in self.actions.iter().enumerate() {
            let comma = if i + 1 < self.actions.len() { "," } else { "" };
            s.push_str(&format!(
                "    {{\"action\": \"{a}\", \"cells_changed\": {n}}}{comma}\n"
            ));
        }
        s.push_str(&format!(
            "  ],\n  \"total_changed\": {},\n  \"unrepairable_count\": {}\n}}\n",
            self.total_changed(),
            self.unrepairable.len()
        ));
        s
    }

    pub fn to_md(&self) -> String {
        let mut s = String::from("## Repair report\n\n");
        for (a, n) in &self.actions {
            s.push_str(&format!("- {a}: {n} cell(s) changed\n"));
        }
        s.push_str(&format!("\n- total changed: {}\n", self.total_changed()));
        if !self.unrepairable.is_empty() {
            s.push_str(&format!(
                "- **unrepairable (catastrophic): {}**\n",
                self.unrepairable.len()
            ));
            for issue in &self.unrepairable {
                s.push_str(&format!(
                    "  - {}: {}\n",
                    issue.issue_type.as_str(),
                    issue.message
                ));
            }
        }
        s
    }
}

/// Downgrade refined cells whose every neighbor is strictly coarser (isolated refinement).
/// Returns the number of cells changed.
pub fn remove_isolated_refined_cells(levels: &mut [u32], neighbors: &[Vec<usize>]) -> usize {
    let snapshot = levels.to_vec();
    let mut changed = 0;
    for ci in 0..levels.len() {
        let la = snapshot[ci];
        let nbs = neighbors.get(ci).map(|v| v.as_slice()).unwrap_or(&[]);
        if la > 0
            && !nbs.is_empty()
            && nbs
                .iter()
                .all(|&nb| snapshot.get(nb).copied().map(|lb| lb < la).unwrap_or(false))
        {
            let max_nb = nbs
                .iter()
                .filter_map(|&nb| snapshot.get(nb).copied())
                .max()
                .unwrap_or(0);
            levels[ci] = max_nb;
            changed += 1;
        }
    }
    changed
}

/// One smoothing pass: clamp each level to `max(neighbor) + 1`. Returns cells changed.
pub fn smooth_target_levels(levels: &mut [u32], neighbors: &[Vec<usize>]) -> usize {
    let snapshot = levels.to_vec();
    let mut changed = 0;
    for ci in 0..levels.len() {
        let nbs = neighbors.get(ci).map(|v| v.as_slice()).unwrap_or(&[]);
        if let Some(max_nb) = nbs.iter().filter_map(|&nb| snapshot.get(nb).copied()).max() {
            let cap = max_nb + 1;
            if snapshot[ci] > cap {
                levels[ci] = cap;
                changed += 1;
            }
        }
    }
    changed
}

/// Iteratively lower levels so no adjacent pair differs by more than `max_jump`.
/// Returns cells changed.
pub fn enforce_max_adjacent_level_jump(
    levels: &mut [u32],
    neighbors: &[Vec<usize>],
    max_jump: u32,
) -> usize {
    let mut changed = 0;
    let mut iterations = 0;
    loop {
        let snapshot = levels.to_vec();
        let mut any = false;
        for ci in 0..levels.len() {
            let nbs = neighbors.get(ci).map(|v| v.as_slice()).unwrap_or(&[]);
            let min_nb = nbs.iter().filter_map(|&nb| snapshot.get(nb).copied()).min();
            if let Some(min_nb) = min_nb {
                if snapshot[ci] > min_nb + max_jump {
                    levels[ci] = min_nb + max_jump;
                    changed += 1;
                    any = true;
                }
            }
        }
        iterations += 1;
        if !any || iterations > levels.len() + 1 {
            break;
        }
    }
    changed
}

/// Issues that level-repair cannot fix (broken connectivity needs mesh surgery).
pub fn mark_unrepairable(issues: &[TopologyIssue]) -> Vec<TopologyIssue> {
    use TopologyIssueType::*;
    issues
        .iter()
        .filter(|i| {
            matches!(
                i.issue_type,
                InvalidVertexIndex
                    | InvalidCellIndex
                    | CellVertexIncidence
                    | DuplicateEdge
                    | DanglingEdge
                    | OrphanCell
                    | NonreciprocalNeighbor
                    | AbnormalPolygonEdgeCount
                    | DisconnectedMesh
                    | NonManifoldVertexFan
                    | BoundaryVertexDegree
                    | EulerCharacteristicMismatch
            )
        })
        .cloned()
        .collect()
}

/// Apply the level-repair hooks in order and emit a [`RepairReport`]. Mutates `levels`.
pub fn run_repair_hooks(
    levels: &mut [u32],
    neighbors: &[Vec<usize>],
    issues: &[TopologyIssue],
    max_jump: u32,
) -> RepairReport {
    let mut report = RepairReport::default();
    let n = remove_isolated_refined_cells(levels, neighbors);
    report
        .actions
        .push(("remove_isolated_refined_cells".to_string(), n));
    let n = smooth_target_levels(levels, neighbors);
    report.actions.push(("smooth_target_levels".to_string(), n));
    let n = enforce_max_adjacent_level_jump(levels, neighbors, max_jump);
    report
        .actions
        .push(("enforce_max_adjacent_level_jump".to_string(), n));
    report.unrepairable = mark_unrepairable(issues);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{QualityCell, QualityMeshInput};
    use earthmesh_geometry::Point;

    fn two_square_mesh() -> QualityMeshInput {
        QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(0.0, 1.0),
                Point::new(2.0, 0.0),
                Point::new(2.0, 1.0),
            ],
            cells: vec![
                QualityCell {
                    vertices: vec![0, 1, 2, 3],
                    refine_level: Some(0),
                    neighbors: vec![1],
                },
                QualityCell {
                    vertices: vec![1, 4, 5, 2],
                    refine_level: Some(0),
                    neighbors: vec![0],
                },
            ],
        }
    }

    fn square_ring_mesh() -> QualityMeshInput {
        let vertices = (0..4)
            .flat_map(|y| (0..4).map(move |x| Point::new(f64::from(x), f64::from(y))))
            .collect();
        let vertex = |x: usize, y: usize| y * 4 + x;
        let cells = (0..3)
            .flat_map(|y| {
                (0..3).filter_map(move |x| {
                    (x != 1 || y != 1).then_some(QualityCell {
                        vertices: vec![
                            vertex(x, y),
                            vertex(x + 1, y),
                            vertex(x + 1, y + 1),
                            vertex(x, y + 1),
                        ],
                        refine_level: Some(0),
                        neighbors: Vec::new(),
                    })
                })
            })
            .collect();
        QualityMeshInput { vertices, cells }
    }

    fn two_component_mesh() -> QualityMeshInput {
        let left = two_square_mesh();
        let mut vertices = left.vertices.clone();
        vertices.extend(
            left.vertices
                .iter()
                .map(|point| Point::new(point.x + 4.0, point.y)),
        );
        let offset = left.vertices.len();
        let mut cells = left.cells.clone();
        cells.extend(left.cells.iter().cloned().map(|mut cell| {
            cell.vertices
                .iter_mut()
                .for_each(|vertex| *vertex += offset);
            cell.neighbors
                .iter_mut()
                .for_each(|neighbor| *neighbor += 2);
            cell
        }));
        QualityMeshInput { vertices, cells }
    }

    #[test]
    fn boundary_contract_enumerates_region_hole_and_multipart_loops() {
        let cases = [
            (two_square_mesh(), 1, 1, 1),
            (square_ring_mesh(), 1, 2, 0),
            (two_component_mesh(), 2, 2, 2),
        ];

        for (mesh, components, loops, expected_euler) in cases {
            let boundary = boundary_topology(&mesh);
            assert!(boundary.invalid_vertex_degrees.is_empty());
            assert_eq!(boundary.loops.len(), loops);
            assert_eq!(connected_component_count(&mesh), components);
            assert_eq!(
                genus_zero_euler_expectation(&mesh, &boundary),
                Some(expected_euler)
            );
            assert_eq!(euler_characteristic(&mesh), expected_euler);
        }
    }

    #[test]
    fn boundary_contract_keeps_closed_sphere_euler_two() {
        let mesh = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
                Point::new(-1.0, -1.0),
            ],
            cells: vec![
                QualityCell {
                    vertices: vec![0, 2, 1],
                    ..QualityCell::default()
                },
                QualityCell {
                    vertices: vec![0, 1, 3],
                    ..QualityCell::default()
                },
                QualityCell {
                    vertices: vec![1, 2, 3],
                    ..QualityCell::default()
                },
                QualityCell {
                    vertices: vec![2, 0, 3],
                    ..QualityCell::default()
                },
            ],
        };

        let boundary = boundary_topology(&mesh);
        assert_eq!(boundary.edge_count, 0);
        assert!(boundary.loops.is_empty());
        assert_eq!(genus_zero_euler_expectation(&mesh, &boundary), Some(2));
        assert_eq!(euler_characteristic(&mesh), 2);
    }

    #[test]
    fn boundary_contract_rejects_non_spherical_closed_genus() {
        let vertex = |x: usize, y: usize| (y % 3) * 3 + (x % 3);
        let mesh = QualityMeshInput {
            vertices: (0..3)
                .flat_map(|y| (0..3).map(move |x| Point::new(x as f64, y as f64)))
                .collect(),
            cells: (0..3)
                .flat_map(|y| {
                    (0..3).map(move |x| QualityCell {
                        vertices: vec![
                            vertex(x, y),
                            vertex(x + 1, y),
                            vertex(x + 1, y + 1),
                            vertex(x, y + 1),
                        ],
                        ..QualityCell::default()
                    })
                })
                .collect(),
        };

        let boundary = boundary_topology(&mesh);
        assert_eq!(boundary.edge_count, 0);
        assert_eq!(euler_characteristic(&mesh), 0);
        assert_eq!(genus_zero_euler_expectation(&mesh, &boundary), Some(2));
        assert!(MeshTopologyValidator::new(&mesh)
            .validate_genus_zero_euler()
            .iter()
            .any(|issue| {
                issue.issue_type == TopologyIssueType::EulerCharacteristicMismatch
                    && issue.severity == Severity::Fail
            }));
    }

    #[test]
    fn boundary_contract_rejects_open_chain_and_vertex_junction() {
        let open = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 99],
                ..QualityCell::default()
            }],
        };
        let open_boundary = boundary_topology(&open);
        assert_eq!(open_boundary.invalid_vertex_degrees, vec![(0, 1), (2, 1)]);
        assert_eq!(genus_zero_euler_expectation(&open, &open_boundary), None);
        assert!(MeshTopologyValidator::new(&open)
            .validate_boundary_loops()
            .iter()
            .all(|issue| issue.issue_type == TopologyIssueType::BoundaryVertexDegree));

        let junction = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
                Point::new(-1.0, 0.0),
                Point::new(0.0, -1.0),
            ],
            cells: vec![
                QualityCell {
                    vertices: vec![0, 1, 2],
                    ..QualityCell::default()
                },
                QualityCell {
                    vertices: vec![0, 3, 4],
                    ..QualityCell::default()
                },
            ],
        };
        let junction_boundary = boundary_topology(&junction);
        assert_eq!(junction_boundary.invalid_vertex_degrees, vec![(0, 4)]);
        assert_eq!(
            genus_zero_euler_expectation(&junction, &junction_boundary),
            None
        );
    }

    #[test]
    fn valid_mesh_has_no_issues() {
        let m = two_square_mesh();
        let v = MeshTopologyValidator::new(&m);
        assert!(v.validate_all().is_empty());
        assert_eq!(worst_severity(&v.validate_all()), None);
    }

    #[test]
    fn invalid_vertex_index_detected_as_fail() {
        let mut m = two_square_mesh();
        m.cells[0].vertices = vec![0, 1, 2, 99];
        let issues = MeshTopologyValidator::new(&m).validate_indices();
        assert!(issues
            .iter()
            .any(|i| i.issue_type == TopologyIssueType::InvalidVertexIndex
                && i.severity == Severity::Fail
                && i.vertex_id == Some(99)));
    }

    #[test]
    fn duplicate_edge_detected() {
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.5, 1.0),
                Point::new(0.5, -1.0),
                Point::new(0.5, 2.0),
            ],
            cells: vec![
                QualityCell {
                    vertices: vec![0, 1, 2],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
                QualityCell {
                    vertices: vec![0, 1, 3],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
                QualityCell {
                    vertices: vec![0, 1, 4],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
            ],
        };
        let issues = MeshTopologyValidator::new(&m).validate_duplicate_edges();
        assert!(issues
            .iter()
            .any(|i| i.issue_type == TopologyIssueType::DuplicateEdge));
    }

    #[test]
    fn nonreciprocal_neighbor_detected() {
        let mut m = two_square_mesh();
        m.cells[1].neighbors = vec![];
        let issues = MeshTopologyValidator::new(&m).validate_neighbors();
        assert!(issues
            .iter()
            .any(|i| i.issue_type == TopologyIssueType::NonreciprocalNeighbor));
    }

    #[test]
    fn isolated_refined_cell_repaired_and_reported() {
        // cell 1 is refined (level 2) but its only neighbor (0) is coarser (level 0).
        let mut levels = vec![0u32, 2u32];
        let neighbors = vec![vec![1usize], vec![0usize]];
        let removed = remove_isolated_refined_cells(&mut levels, &neighbors);
        assert_eq!(removed, 1);
        assert_eq!(levels[1], 0);
    }

    #[test]
    fn level_jump_too_large_warns_and_enforced() {
        let mut m = two_square_mesh();
        m.cells[0].refine_level = Some(0);
        m.cells[1].refine_level = Some(3);
        let issues = MeshTopologyValidator::new(&m).validate_transition_continuity();
        assert!(issues.iter().any(
            |i| i.issue_type == TopologyIssueType::TransitionDiscontinuity
                && i.severity == Severity::Warn
        ));

        let mut levels = vec![0u32, 3u32];
        let neighbors = vec![vec![1usize], vec![0usize]];
        let changed = enforce_max_adjacent_level_jump(&mut levels, &neighbors, 1);
        assert!(changed >= 1);
        assert!(levels[0].abs_diff(levels[1]) <= 1);
    }

    #[test]
    fn repair_hook_emits_report() {
        let mut levels = vec![0u32, 3u32, 0u32];
        let neighbors = vec![vec![1usize], vec![0usize, 2usize], vec![1usize]];
        let issues = vec![TopologyIssue::new(
            TopologyIssueType::InvalidVertexIndex,
            Some(0),
            None,
            Some(99),
            "x",
            "y",
        )];
        let report = run_repair_hooks(&mut levels, &neighbors, &issues, 1);
        assert_eq!(report.actions.len(), 3);
        assert_eq!(report.unrepairable.len(), 1); // invalid index not level-repairable
        let json = report.to_json();
        assert!(json.contains("earthmesh_repair_report"));
        assert!(json.contains("enforce_max_adjacent_level_jump"));
        assert!(report.to_md().contains("Repair report"));
    }
}
