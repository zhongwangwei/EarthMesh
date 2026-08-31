//! Effective-width measurements for adjacent stratified transition traces.

use super::{DirectedTrace, RingCycle, StratifiedAnnulus, WorstAngleAtlas};
use crate::mother_grid::MotherGrid;
use earthmesh_mesh::arc_length_unit_sphere;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveBandReport {
    pub logical_band_count: usize,
    pub adjacent_trace_pairs: usize,
    pub shared_vertex_count_by_pair: Vec<usize>,
    pub shared_edge_count_by_pair: Vec<usize>,
    pub minimum_face_strip_width_by_pair: Vec<usize>,
    pub minimum_geodesic_separation_over_target_by_pair: Vec<f64>,
    pub zero_width_pinch_vertices: Vec<usize>,
    pub worst_angle_near_pinch_fraction: f64,
}

pub fn build_effective_band_report(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    atlas: &WorstAngleAtlas,
) -> EffectiveBandReport {
    let mut traces = stratified.traces.iter().collect::<Vec<_>>();
    traces.sort_by_key(|trace| trace.trace_id);
    let face_adjacency = face_adjacency(source);
    let edge_faces = edge_faces(source);
    let ring_scales = source_ring_scales(
        &[
            &stratified.coupled.inner_guard,
            &stratified.coupled.coarse_interface,
            &stratified.coupled.fine_interface,
            &stratified.coupled.outer_guard,
        ],
        &stratified.coupled.intermediate_rings,
    );
    let mut shared_vertex_count_by_pair = Vec::new();
    let mut shared_edge_count_by_pair = Vec::new();
    let mut minimum_face_strip_width_by_pair = Vec::new();
    let mut minimum_geodesic_separation_over_target_by_pair = Vec::new();
    let mut zero_width_pinch_vertices = BTreeSet::new();

    for pair in traces.windows(2) {
        let lower = pair[0];
        let upper = pair[1];
        let shared_vertices = trace_vertices(lower)
            .intersection(&trace_vertices(upper))
            .copied()
            .collect::<BTreeSet<_>>();
        let shared_edges = trace_edges(lower)
            .intersection(&trace_edges(upper))
            .copied()
            .collect::<BTreeSet<_>>();
        zero_width_pinch_vertices.extend(&shared_vertices);
        shared_vertex_count_by_pair.push(shared_vertices.len());
        shared_edge_count_by_pair.push(shared_edges.len());
        minimum_face_strip_width_by_pair.push(
            if shared_vertices.is_empty() && shared_edges.is_empty() {
                minimum_face_strip_width(lower, upper, &edge_faces, &face_adjacency).unwrap_or(0)
            } else {
                0
            },
        );
        minimum_geodesic_separation_over_target_by_pair
            .push(minimum_normalized_separation(source, lower, upper, &ring_scales).unwrap_or(0.0));
    }
    let worst_angle_near_pinch_fraction = if atlas.worst_angles.is_empty() {
        0.0
    } else {
        atlas
            .worst_angles
            .iter()
            .filter(|angle| {
                angle
                    .distance_to_shared_junction
                    .is_some_and(|distance| distance <= 1)
            })
            .count() as f64
            / atlas.worst_angles.len() as f64
    };
    EffectiveBandReport {
        logical_band_count: traces.len().saturating_sub(1),
        adjacent_trace_pairs: traces.len().saturating_sub(1),
        shared_vertex_count_by_pair,
        shared_edge_count_by_pair,
        minimum_face_strip_width_by_pair,
        minimum_geodesic_separation_over_target_by_pair,
        zero_width_pinch_vertices: zero_width_pinch_vertices.into_iter().collect(),
        worst_angle_near_pinch_fraction,
    }
}

pub fn effective_band_report_json(report: &EffectiveBandReport) -> String {
    format!(
        "{{\"logical_band_count\":{},\"adjacent_trace_pairs\":{},\"shared_vertex_count_by_pair\":{},\"shared_edge_count_by_pair\":{},\"minimum_face_strip_width_by_pair\":{},\"minimum_geodesic_separation_over_target_by_pair\":{},\"zero_width_pinch_vertices\":{},\"worst_angle_near_pinch_fraction\":{:.12}}}",
        report.logical_band_count,
        report.adjacent_trace_pairs,
        usize_json(&report.shared_vertex_count_by_pair),
        usize_json(&report.shared_edge_count_by_pair),
        usize_json(&report.minimum_face_strip_width_by_pair),
        f64_json(&report.minimum_geodesic_separation_over_target_by_pair),
        usize_json(&report.zero_width_pinch_vertices),
        report.worst_angle_near_pinch_fraction,
    )
}

fn trace_vertices(trace: &DirectedTrace) -> BTreeSet<usize> {
    trace
        .occurrences
        .iter()
        .map(|occurrence| occurrence.source_slot)
        .collect()
}

fn trace_edges(trace: &DirectedTrace) -> BTreeSet<(usize, usize)> {
    trace
        .directed_edges
        .iter()
        .map(|edge| canonical_edge(edge.from, edge.to))
        .collect()
}

fn edge_faces(source: &MotherGrid) -> BTreeMap<(usize, usize), BTreeSet<usize>> {
    let mut out = BTreeMap::<(usize, usize), BTreeSet<usize>>::new();
    for face in source.mesh.active_triangle_slots() {
        for edge in triangle_edges(source.mesh.triangles()[face]) {
            out.entry(edge).or_default().insert(face);
        }
    }
    out
}

fn face_adjacency(source: &MotherGrid) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for faces in edge_faces(source).into_values() {
        for &left in &faces {
            for &right in &faces {
                if left != right {
                    adjacency.entry(left).or_default().insert(right);
                }
            }
        }
    }
    adjacency
}

fn minimum_face_strip_width(
    lower: &DirectedTrace,
    upper: &DirectedTrace,
    edge_faces: &BTreeMap<(usize, usize), BTreeSet<usize>>,
    face_adjacency: &BTreeMap<usize, BTreeSet<usize>>,
) -> Option<usize> {
    let lower_faces = trace_edges(lower)
        .iter()
        .filter_map(|edge| edge_faces.get(edge))
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    let upper_faces = trace_edges(upper)
        .iter()
        .filter_map(|edge| edge_faces.get(edge))
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    if lower_faces.is_empty() || upper_faces.is_empty() {
        return None;
    }
    let mut distances = lower_faces
        .iter()
        .map(|&face| (face, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut queue = lower_faces.iter().copied().collect::<VecDeque<_>>();
    while let Some(face) = queue.pop_front() {
        let distance = distances[&face];
        if upper_faces.contains(&face) {
            return Some(distance + 1);
        }
        for &next in face_adjacency.get(&face).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(next) {
                entry.insert(distance + 1);
                queue.push_back(next);
            }
        }
    }
    None
}

fn source_ring_scales(
    fixed_rings: &[&RingCycle],
    intermediate_rings: &[RingCycle],
) -> BTreeMap<usize, Vec<f64>> {
    let mut scales = BTreeMap::<usize, Vec<f64>>::new();
    for ring in fixed_rings.iter().copied().chain(intermediate_rings.iter()) {
        for vertex in &ring.vertices {
            scales
                .entry(vertex.source_slot)
                .or_default()
                .push(ring.target_scale);
        }
    }
    scales
}

fn minimum_normalized_separation(
    source: &MotherGrid,
    lower: &DirectedTrace,
    upper: &DirectedTrace,
    ring_scales: &BTreeMap<usize, Vec<f64>>,
) -> Option<f64> {
    let lower_vertices = trace_vertices(lower);
    let upper_vertices = trace_vertices(upper);
    let separation = lower_vertices
        .iter()
        .flat_map(|&left| {
            upper_vertices.iter().map(move |&right| {
                arc_length_unit_sphere(source.mesh.vertices()[left], source.mesh.vertices()[right])
            })
        })
        .filter(|distance| distance.is_finite())
        .min_by(f64::total_cmp)?;
    let lower_scale = trace_scale(&lower_vertices, ring_scales)?;
    let upper_scale = trace_scale(&upper_vertices, ring_scales)?;
    let target = (lower_scale * upper_scale).sqrt();
    (target > 0.0).then_some(separation / target)
}

fn trace_scale(vertices: &BTreeSet<usize>, ring_scales: &BTreeMap<usize, Vec<f64>>) -> Option<f64> {
    let mut values = vertices
        .iter()
        .filter_map(|vertex| ring_scales.get(vertex))
        .flatten()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(values[values.len() / 2])
}

fn triangle_edges([a, b, c]: [usize; 3]) -> [(usize, usize); 3] {
    [
        canonical_edge(a, b),
        canonical_edge(b, c),
        canonical_edge(c, a),
    ]
}

fn canonical_edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}

fn usize_json(values: &[usize]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn f64_json(values: &[f64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("{value:.12}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{DirectedHalfEdge, RingOccurrence, RingOccurrenceId, TraceRole};

    fn trace(trace_id: usize, vertices: &[usize]) -> DirectedTrace {
        DirectedTrace {
            trace_id,
            role: TraceRole::Intermediate,
            directed_edges: vertices
                .windows(2)
                .map(|edge| DirectedHalfEdge {
                    from: edge[0],
                    to: edge[1],
                })
                .collect(),
            occurrences: vertices
                .iter()
                .enumerate()
                .map(|(ordinal, &source_slot)| RingOccurrence {
                    occurrence_id: RingOccurrenceId { trace_id, ordinal },
                    source_slot,
                    role: TraceRole::Intermediate,
                })
                .collect(),
        }
    }

    #[test]
    fn shared_vertices_and_edges_are_exact() {
        let lower = trace(0, &[1, 2, 3]);
        let upper = trace(1, &[2, 3, 4]);
        assert_eq!(
            trace_vertices(&lower)
                .intersection(&trace_vertices(&upper))
                .copied()
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            trace_edges(&lower)
                .intersection(&trace_edges(&upper))
                .copied()
                .collect::<Vec<_>>(),
            vec![(2, 3)]
        );
    }
}
