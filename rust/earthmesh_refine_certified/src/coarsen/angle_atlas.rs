//! Deterministic localization of the worst Frozen N6 angle constraints.

use super::elastic_block::{face_graph_distance_to_any, graph_distances, local_triangle_edges};
use super::{
    ElasticPatch, FullPolygonTopologyKey, GlobalExactSelectedEar, HierarchyLeafMesh,
    RingAnchorKind, StratifiedAnnulus, TraceRole,
};
use crate::{certificate::spherical_triangle_angles, mother_grid::MotherGrid};
use earthmesh_mesh::arc_length_unit_sphere;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdgeClass {
    MotherGridEdge,
    CoarseInterface,
    FineInterface,
    SectorBoundary,
    CrossChainDiagonal,
    SameChainDiagonal,
    AnchorEarChord,
    OtherFullPolygonDiagonal,
}

impl EdgeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MotherGridEdge => "MotherGridEdge",
            Self::CoarseInterface => "CoarseInterface",
            Self::FineInterface => "FineInterface",
            Self::SectorBoundary => "SectorBoundary",
            Self::CrossChainDiagonal => "CrossChainDiagonal",
            Self::SameChainDiagonal => "SameChainDiagonal",
            Self::AnchorEarChord => "AnchorEarChord",
            Self::OtherFullPolygonDiagonal => "OtherFullPolygonDiagonal",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngleWitness {
    pub face: usize,
    pub corner: usize,
    pub corner_source_slot: Option<usize>,
    pub angle_deg: f64,
    pub signed_margin_deg: f64,
    pub topology_key: FullPolygonTopologyKey,
    pub sector_id: Option<u64>,
    pub band_id: Option<usize>,
    pub distance_to_shared_junction: Option<usize>,
    pub distance_to_pentagon_anchor: Option<usize>,
    pub distance_to_fixed_guard_face: Option<usize>,
    pub fixed_vertex_count: usize,
    pub edge_classes: [EdgeClass; 3],
    pub target_edge_log_errors: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleBlockerClassification {
    WidthDominated,
    TopologyDiagonalDominated,
    BoundaryDominated,
    DistributedSolverDominated,
}

impl AngleBlockerClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WidthDominated => "WidthDominated",
            Self::TopologyDiagonalDominated => "TopologyDiagonalDominated",
            Self::BoundaryDominated => "BoundaryDominated",
            Self::DistributedSolverDominated => "DistributedSolverDominated",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorstAngleAtlas {
    pub total_angles: usize,
    pub worst_angles: Vec<AngleWitness>,
    pub adjacent_pentagon_or_junction_fraction: f64,
    pub long_full_polygon_diagonal_fraction: f64,
    pub fixed_guard_neighbourhood_fraction: f64,
}

pub fn build_worst_angle_atlas(
    source: &MotherGrid,
    mesh: &HierarchyLeafMesh,
    patch: &ElasticPatch,
    stratified: &StratifiedAnnulus,
    topology_keys: &[FullPolygonTopologyKey],
    selected_ears: &[GlobalExactSelectedEar],
    limit: usize,
) -> Result<WorstAngleAtlas, String> {
    if topology_keys.is_empty() {
        return Err("angle atlas requires at least one topology key".into());
    }
    if mesh.source_vertex_slots.len() != mesh.mesh.vertices().len() {
        return Err("angle atlas source-slot map does not match mesh vertices".into());
    }

    let source_to_compact = mesh
        .source_vertex_slots
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(compact, source)| source.map(|source| (source, compact)))
        .collect::<BTreeMap<_, _>>();
    let guard_edges = mesh
        .mesh
        .active_triangle_slots()
        .flat_map(|face| local_triangle_edges(mesh.mesh.triangles()[face]))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let shared_sources = stratified
        .shared_junctions
        .iter()
        .map(|junction| junction.source_slot)
        .collect::<BTreeSet<_>>();
    let pentagon_sources = stratified
        .link_contracts
        .iter()
        .filter_map(|(&source, contract)| {
            matches!(
                contract.anchor_kind,
                RingAnchorKind::IcosahedronPentagon { .. }
            )
            .then_some(source)
        })
        .collect::<BTreeSet<_>>();
    let shared_compact = compact_seeds(&shared_sources, &source_to_compact);
    let pentagon_compact = compact_seeds(&pentagon_sources, &source_to_compact);
    let shared_distances = graph_distances(&guard_edges, &shared_compact);
    let pentagon_distances = graph_distances(&guard_edges, &pentagon_compact);
    let fixed = patch
        .fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let fixed_guard_faces = patch
        .guard_faces
        .iter()
        .copied()
        .filter(|&face| {
            mesh.mesh.triangles()[face]
                .iter()
                .any(|site| fixed.contains(site))
        })
        .collect::<BTreeSet<_>>();
    let topology = TopologyIndex::new(stratified, topology_keys, selected_ears);
    let band_vertices = band_vertex_sets(source, stratified);
    let mut witnesses = Vec::new();

    for face in mesh.mesh.active_triangle_slots() {
        let triangle = mesh.mesh.triangles()[face];
        let points = triangle.map(|site| mesh.mesh.vertices()[site]);
        let angles = spherical_triangle_angles(points)
            .ok_or_else(|| format!("angle atlas failed on face {face}"))?;
        let source_triangle = triangle.map(|site| mesh.source_vertex_slots[site]);
        let owner = topology.owner(source_triangle);
        let edge_classes = topology.edge_classes(source_triangle);
        let target_edge_log_errors = target_edge_log_errors(mesh, patch, triangle);
        let band_id = best_band(source_triangle, &band_vertices);
        let fixed_vertex_count = triangle.iter().filter(|site| fixed.contains(site)).count();
        let distance_to_shared_junction = minimum_triangle_distance(triangle, &shared_distances);
        let distance_to_pentagon_anchor = minimum_triangle_distance(triangle, &pentagon_distances);
        let distance_to_fixed_guard_face =
            face_graph_distance_to_any(&mesh.mesh, face, &fixed_guard_faces);
        for (corner, angle_deg) in angles.into_iter().enumerate() {
            witnesses.push(AngleWitness {
                face,
                corner,
                corner_source_slot: source_triangle[corner],
                angle_deg,
                signed_margin_deg: signed_margin(angle_deg),
                topology_key: topology_keys[owner].clone(),
                sector_id: topology.exact_sector(source_triangle),
                band_id,
                distance_to_shared_junction,
                distance_to_pentagon_anchor,
                distance_to_fixed_guard_face,
                fixed_vertex_count,
                edge_classes,
                target_edge_log_errors,
            });
        }
    }
    witnesses.sort_by(|left, right| {
        left.signed_margin_deg
            .total_cmp(&right.signed_margin_deg)
            .then_with(|| left.face.cmp(&right.face))
            .then_with(|| left.corner.cmp(&right.corner))
    });
    let total_angles = witnesses.len();
    witnesses.truncate(limit.min(total_angles));
    let denominator = witnesses.len();
    let fraction = |count| {
        if denominator == 0 {
            0.0
        } else {
            count as f64 / denominator as f64
        }
    };
    let adjacent_pentagon_or_junction_fraction = fraction(
        witnesses
            .iter()
            .filter(|angle| {
                angle
                    .distance_to_shared_junction
                    .is_some_and(|distance| distance <= 1)
                    || angle
                        .distance_to_pentagon_anchor
                        .is_some_and(|distance| distance <= 1)
            })
            .count(),
    );
    let long_threshold = 1.5248_f64.ln();
    let long_full_polygon_diagonal_fraction = fraction(
        witnesses
            .iter()
            .filter(|angle| {
                angle
                    .edge_classes
                    .iter()
                    .zip(angle.target_edge_log_errors)
                    .any(|(class, error)| {
                        matches!(
                            class,
                            EdgeClass::SameChainDiagonal | EdgeClass::OtherFullPolygonDiagonal
                        ) && error > long_threshold
                    })
            })
            .count(),
    );
    let fixed_guard_neighbourhood_fraction = fraction(
        witnesses
            .iter()
            .filter(|angle| {
                angle.fixed_vertex_count > 0
                    || angle
                        .distance_to_fixed_guard_face
                        .is_some_and(|distance| distance <= 1)
            })
            .count(),
    );
    Ok(WorstAngleAtlas {
        total_angles,
        worst_angles: witnesses,
        adjacent_pentagon_or_junction_fraction,
        long_full_polygon_diagonal_fraction,
        fixed_guard_neighbourhood_fraction,
    })
}

pub fn classify_angle_blockers(
    atlas: &WorstAngleAtlas,
    worst_angle_near_pinch_fraction: f64,
) -> AngleBlockerClassification {
    if worst_angle_near_pinch_fraction >= 0.6 || atlas.adjacent_pentagon_or_junction_fraction >= 0.6
    {
        AngleBlockerClassification::WidthDominated
    } else if atlas.long_full_polygon_diagonal_fraction >= 0.6 {
        AngleBlockerClassification::TopologyDiagonalDominated
    } else if atlas.fixed_guard_neighbourhood_fraction >= 0.6 {
        AngleBlockerClassification::BoundaryDominated
    } else {
        AngleBlockerClassification::DistributedSolverDominated
    }
}

pub fn worst_angle_atlas_json(atlas: &WorstAngleAtlas) -> String {
    let worst = atlas
        .worst_angles
        .iter()
        .map(angle_witness_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"total_angles\":{},\"reported_angles\":{},\"adjacent_pentagon_or_junction_fraction\":{:.12},\"long_full_polygon_diagonal_fraction\":{:.12},\"fixed_guard_neighbourhood_fraction\":{:.12},\"worst_angles\":[{}]}}",
        atlas.total_angles,
        atlas.worst_angles.len(),
        atlas.adjacent_pentagon_or_junction_fraction,
        atlas.long_full_polygon_diagonal_fraction,
        atlas.fixed_guard_neighbourhood_fraction,
        worst,
    )
}

fn angle_witness_json(angle: &AngleWitness) -> String {
    let edge_classes = angle
        .edge_classes
        .iter()
        .map(|class| format!("\"{}\"", class.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let mut json = String::new();
    write!(
        json,
        "{{\"face\":{},\"corner\":{},\"corner_source_slot\":{},\"angle_deg\":{:.12},\"signed_margin_deg\":{:.12},\"topology_key\":{},\"sector_id\":{},\"band_id\":{},\"distance_to_shared_junction\":{},\"distance_to_pentagon_anchor\":{},\"distance_to_fixed_guard_face\":{},\"fixed_vertex_count\":{},\"edge_classes\":[{}],\"target_edge_log_errors\":[{:.12},{:.12},{:.12}]}}",
        angle.face,
        angle.corner,
        option_usize(angle.corner_source_slot),
        angle.angle_deg,
        angle.signed_margin_deg,
        topology_key_json(&angle.topology_key),
        option_u64(angle.sector_id),
        option_usize(angle.band_id),
        option_usize(angle.distance_to_shared_junction),
        option_usize(angle.distance_to_pentagon_anchor),
        option_usize(angle.distance_to_fixed_guard_face),
        angle.fixed_vertex_count,
        edge_classes,
        angle.target_edge_log_errors[0],
        angle.target_edge_log_errors[1],
        angle.target_edge_log_errors[2],
    )
    .unwrap();
    json
}

struct TopologyIndex {
    vertex_sets: Vec<BTreeSet<usize>>,
    exact_triangles: BTreeMap<[usize; 3], usize>,
    boundary_edges: BTreeSet<(usize, usize)>,
    topology_edges: BTreeSet<(usize, usize)>,
    coarse_edges: BTreeSet<(usize, usize)>,
    fine_edges: BTreeSet<(usize, usize)>,
    trace_memberships: BTreeMap<usize, BTreeSet<usize>>,
    ear_chords: BTreeSet<(usize, usize)>,
    sector_ids: Vec<u64>,
}

impl TopologyIndex {
    fn new(
        stratified: &StratifiedAnnulus,
        topology_keys: &[FullPolygonTopologyKey],
        selected_ears: &[GlobalExactSelectedEar],
    ) -> Self {
        let mut exact_triangles = BTreeMap::new();
        let mut boundary_edges = BTreeSet::new();
        let mut topology_edges = BTreeSet::new();
        let mut vertex_sets = Vec::new();
        let mut sector_ids = Vec::new();
        for (index, key) in topology_keys.iter().enumerate() {
            let mut counts = BTreeMap::<(usize, usize), usize>::new();
            let mut vertices = BTreeSet::new();
            for triangle in &key.triangles {
                let triangle = canonical_triangle(*triangle);
                exact_triangles.insert(triangle, index);
                vertices.extend(triangle);
                for edge in source_edges(triangle) {
                    *counts.entry(edge).or_default() += 1;
                    topology_edges.insert(edge);
                }
            }
            boundary_edges.extend(
                counts
                    .into_iter()
                    .filter_map(|(edge, count)| (count == 1).then_some(edge)),
            );
            vertex_sets.push(vertices);
            sector_ids.push(key.sector_id);
        }
        let mut coarse_edges = BTreeSet::new();
        let mut fine_edges = BTreeSet::new();
        let mut trace_memberships = BTreeMap::<usize, BTreeSet<usize>>::new();
        for trace in &stratified.traces {
            for occurrence in &trace.occurrences {
                trace_memberships
                    .entry(occurrence.source_slot)
                    .or_default()
                    .insert(trace.trace_id);
            }
            let out = match trace.role {
                TraceRole::CoarseInterface => &mut coarse_edges,
                TraceRole::FineInterface => &mut fine_edges,
                TraceRole::Intermediate => continue,
            };
            out.extend(
                trace
                    .directed_edges
                    .iter()
                    .map(|edge| source_edge(edge.from, edge.to)),
            );
        }
        Self {
            vertex_sets,
            exact_triangles,
            boundary_edges,
            topology_edges,
            coarse_edges,
            fine_edges,
            trace_memberships,
            ear_chords: selected_ears
                .iter()
                .map(|ear| source_edge(ear.inserted_chord.0, ear.inserted_chord.1))
                .collect(),
            sector_ids,
        }
    }

    fn owner(&self, triangle: [Option<usize>; 3]) -> usize {
        if let Some(triangle) = complete_triangle(triangle) {
            if let Some(&owner) = self.exact_triangles.get(&canonical_triangle(triangle)) {
                return owner;
            }
            return self
                .vertex_sets
                .iter()
                .enumerate()
                .max_by_key(|(index, vertices)| {
                    (
                        triangle
                            .iter()
                            .filter(|site| vertices.contains(site))
                            .count(),
                        usize::MAX - index,
                    )
                })
                .map(|(index, _)| index)
                .unwrap_or(0);
        }
        0
    }

    fn exact_sector(&self, triangle: [Option<usize>; 3]) -> Option<u64> {
        complete_triangle(triangle)
            .and_then(|triangle| self.exact_triangles.get(&canonical_triangle(triangle)))
            .map(|&owner| self.sector_ids[owner])
    }

    fn edge_classes(&self, triangle: [Option<usize>; 3]) -> [EdgeClass; 3] {
        let Some(triangle) = complete_triangle(triangle) else {
            return [EdgeClass::MotherGridEdge; 3];
        };
        source_edges(triangle).map(|edge| self.edge_class(edge))
    }

    fn edge_class(&self, edge: (usize, usize)) -> EdgeClass {
        if self.coarse_edges.contains(&edge) {
            EdgeClass::CoarseInterface
        } else if self.fine_edges.contains(&edge) {
            EdgeClass::FineInterface
        } else if self.ear_chords.contains(&edge) {
            EdgeClass::AnchorEarChord
        } else if self.boundary_edges.contains(&edge) {
            EdgeClass::SectorBoundary
        } else if self.topology_edges.contains(&edge) {
            let left = self.trace_memberships.get(&edge.0);
            let right = self.trace_memberships.get(&edge.1);
            if left.is_some_and(|left| right.is_some_and(|right| !left.is_disjoint(right))) {
                EdgeClass::SameChainDiagonal
            } else if left.is_some() && right.is_some() {
                EdgeClass::CrossChainDiagonal
            } else {
                EdgeClass::OtherFullPolygonDiagonal
            }
        } else {
            EdgeClass::MotherGridEdge
        }
    }
}

fn target_edge_log_errors(
    mesh: &HierarchyLeafMesh,
    patch: &ElasticPatch,
    triangle: [usize; 3],
) -> [f64; 3] {
    local_triangle_edges(triangle).map(|edge| {
        let actual =
            arc_length_unit_sphere(mesh.mesh.vertices()[edge.0], mesh.mesh.vertices()[edge.1]);
        let target = patch
            .target_field
            .target_edge_lengths
            .get(&edge)
            .copied()
            .unwrap_or_else(|| {
                arc_length_unit_sphere(
                    patch.reference_positions[edge.0],
                    patch.reference_positions[edge.1],
                )
            });
        if actual > 0.0 && target > 0.0 {
            (actual / target).ln()
        } else {
            0.0
        }
    })
}

fn band_vertex_sets(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut sets = BTreeMap::<usize, BTreeSet<usize>>::new();
    for label in &stratified.band_face_labels {
        sets.entry(label.band_id)
            .or_default()
            .extend(source.mesh.triangles()[label.face_slot]);
    }
    sets
}

fn best_band(
    triangle: [Option<usize>; 3],
    band_vertices: &BTreeMap<usize, BTreeSet<usize>>,
) -> Option<usize> {
    let triangle = complete_triangle(triangle)?;
    band_vertices
        .iter()
        .max_by_key(|(band, vertices)| {
            (
                triangle
                    .iter()
                    .filter(|site| vertices.contains(site))
                    .count(),
                usize::MAX - **band,
            )
        })
        .and_then(|(&band, vertices)| {
            triangle
                .iter()
                .any(|site| vertices.contains(site))
                .then_some(band)
        })
}

fn compact_seeds(
    sources: &BTreeSet<usize>,
    source_to_compact: &BTreeMap<usize, usize>,
) -> BTreeSet<usize> {
    sources
        .iter()
        .filter_map(|source| source_to_compact.get(source).copied())
        .collect()
}

fn minimum_triangle_distance(
    triangle: [usize; 3],
    distances: &BTreeMap<usize, usize>,
) -> Option<usize> {
    triangle
        .into_iter()
        .filter_map(|site| distances.get(&site).copied())
        .min()
}

fn signed_margin(angle: f64) -> f64 {
    (angle - 40.2).min(79.8 - angle)
}

fn complete_triangle(triangle: [Option<usize>; 3]) -> Option<[usize; 3]> {
    Some([triangle[0]?, triangle[1]?, triangle[2]?])
}

fn canonical_triangle(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn source_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [source_edge(a, b), source_edge(b, c), source_edge(c, a)]
}

fn source_edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}

fn option_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn option_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn topology_key_json(key: &FullPolygonTopologyKey) -> String {
    let triangles = key
        .triangles
        .iter()
        .map(|triangle| format!("[{},{},{}]", triangle[0], triangle[1], triangle[2]))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"sector_id\":{},\"triangles\":[{}]}}",
        key.sector_id, triangles
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atlas(width: f64, diagonal: f64, boundary: f64) -> WorstAngleAtlas {
        WorstAngleAtlas {
            total_angles: 0,
            worst_angles: Vec::new(),
            adjacent_pentagon_or_junction_fraction: width,
            long_full_polygon_diagonal_fraction: diagonal,
            fixed_guard_neighbourhood_fraction: boundary,
        }
    }

    #[test]
    fn blocker_gate_uses_frozen_priority_and_threshold() {
        assert_eq!(
            classify_angle_blockers(&atlas(0.6, 1.0, 1.0), 0.0),
            AngleBlockerClassification::WidthDominated
        );
        assert_eq!(
            classify_angle_blockers(&atlas(0.0, 0.6, 1.0), 0.0),
            AngleBlockerClassification::TopologyDiagonalDominated
        );
        assert_eq!(
            classify_angle_blockers(&atlas(0.0, 0.0, 0.6), 0.0),
            AngleBlockerClassification::BoundaryDominated
        );
        assert_eq!(
            classify_angle_blockers(&atlas(0.59, 0.59, 0.59), 0.59),
            AngleBlockerClassification::DistributedSolverDominated
        );
    }

    #[test]
    fn empty_atlas_json_is_stable() {
        let atlas = atlas(0.0, 0.0, 0.0);
        assert_eq!(
            worst_angle_atlas_json(&atlas),
            worst_angle_atlas_json(&atlas)
        );
    }
}
