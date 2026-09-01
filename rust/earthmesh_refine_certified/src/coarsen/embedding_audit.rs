//! Geometry-only audit for transferring a certified W2 witness to a W3 topology.

use super::{full_polygon::minor_arc_crossing_strength, FullPolygonTopologyKey, HierarchyLeafMesh};
use crate::certificate::spherical_triangle_angles;
use earthmesh_mesh::{cross, dot, magnitude, orientation_on_sphere, CartesianPoint, Sign};
use std::collections::{BTreeMap, BTreeSet};

const NEAR_DEGENERATE_ANGLE_DEGREES: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationMismatch {
    pub compact_vertex: usize,
    pub source_vertex: Option<usize>,
    pub combinatorial_order: Vec<usize>,
    pub geometric_order: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingAudit {
    pub topology_key: FullPolygonTopologyKey,
    pub common_source_vertices: usize,
    pub added_source_vertices: Vec<usize>,
    pub removed_source_vertices: Vec<usize>,
    pub common_edges: usize,
    pub added_edges: usize,
    pub removed_edges: usize,
    pub common_triangles: usize,
    pub added_triangles: usize,
    pub removed_triangles: usize,
    pub non_positive_triangles: usize,
    pub crossing_pairs: usize,
    pub near_degenerate_triangles: usize,
    pub fixed_only_degenerate_triangles: usize,
    pub rotation_mismatch_vertices: Vec<RotationMismatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingAuditOutcome {
    UsableEmbedding,
    TopologyClosedNoUsableEmbedding,
}

impl EmbeddingAudit {
    pub fn outcome(&self) -> EmbeddingAuditOutcome {
        if self.non_positive_triangles == 0
            && self.crossing_pairs == 0
            && self.near_degenerate_triangles == 0
            && self.fixed_only_degenerate_triangles == 0
            && self.rotation_mismatch_vertices.is_empty()
        {
            EmbeddingAuditOutcome::UsableEmbedding
        } else {
            EmbeddingAuditOutcome::TopologyClosedNoUsableEmbedding
        }
    }
}

pub fn audit_embedding_transfer(
    topology_key: FullPolygonTopologyKey,
    reference: &HierarchyLeafMesh,
    candidate: &HierarchyLeafMesh,
    fixed_compact_vertices: &[usize],
) -> Result<EmbeddingAudit, String> {
    validate_source_slots(reference)?;
    validate_source_slots(candidate)?;

    let reference_sources = source_vertices(reference);
    let candidate_sources = source_vertices(candidate);
    let added_source_vertices = candidate_sources
        .difference(&reference_sources)
        .copied()
        .collect::<Vec<_>>();
    let removed_source_vertices = reference_sources
        .difference(&candidate_sources)
        .copied()
        .collect::<Vec<_>>();

    let reference_edges = source_edges(reference);
    let candidate_edges = source_edges(candidate);
    let common_edges = reference_edges.intersection(&candidate_edges).count();
    let reference_triangles = source_triangles(reference);
    let candidate_triangles = source_triangles(candidate);
    let common_triangles = reference_triangles
        .intersection(&candidate_triangles)
        .count();

    let fixed = fixed_compact_vertices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut non_positive_triangles = 0;
    let mut near_degenerate_triangles = 0;
    let mut fixed_only_degenerate_triangles = 0;
    for face in candidate.mesh.active_triangle_slots() {
        let triangle = candidate.mesh.triangles()[face];
        let points = triangle.map(|site| candidate.mesh.vertices()[site]);
        if orientation_on_sphere(points[0], points[1], points[2]) != Ok(Sign::Positive) {
            non_positive_triangles += 1;
        }
        let near_degenerate = spherical_triangle_angles(points).is_none_or(|angles| {
            angles.into_iter().any(|angle| {
                angle <= NEAR_DEGENERATE_ANGLE_DEGREES
                    || angle >= 180.0 - NEAR_DEGENERATE_ANGLE_DEGREES
            })
        });
        if near_degenerate {
            near_degenerate_triangles += 1;
            if triangle.into_iter().all(|site| fixed.contains(&site)) {
                fixed_only_degenerate_triangles += 1;
            }
        }
    }

    Ok(EmbeddingAudit {
        topology_key,
        common_source_vertices: reference_sources.intersection(&candidate_sources).count(),
        added_source_vertices,
        removed_source_vertices,
        common_edges,
        added_edges: edge_count(candidate).saturating_sub(common_edges),
        removed_edges: edge_count(reference).saturating_sub(common_edges),
        common_triangles,
        added_triangles: candidate
            .mesh
            .triangle_count()
            .saturating_sub(common_triangles),
        removed_triangles: reference
            .mesh
            .triangle_count()
            .saturating_sub(common_triangles),
        non_positive_triangles,
        crossing_pairs: crossing_pairs(candidate),
        near_degenerate_triangles,
        fixed_only_degenerate_triangles,
        rotation_mismatch_vertices: rotation_mismatches(candidate),
    })
}

fn validate_source_slots(mesh: &HierarchyLeafMesh) -> Result<(), String> {
    if mesh.source_vertex_slots.len() != mesh.mesh.vertices().len() {
        return Err("embedding audit source-slot map does not match mesh vertices".into());
    }
    let mut seen = BTreeSet::new();
    if let Some(duplicate) = mesh
        .source_vertex_slots
        .iter()
        .flatten()
        .find(|&&source| !seen.insert(source))
    {
        return Err(format!(
            "embedding audit has duplicate source vertex {duplicate}"
        ));
    }
    Ok(())
}

fn source_vertices(mesh: &HierarchyLeafMesh) -> BTreeSet<usize> {
    mesh.source_vertex_slots.iter().flatten().copied().collect()
}

fn source_edges(mesh: &HierarchyLeafMesh) -> BTreeSet<(usize, usize)> {
    mesh.mesh
        .active_triangle_slots()
        .flat_map(|face| {
            let [a, b, c] = mesh.mesh.triangles()[face];
            [(a, b), (b, c), (c, a)]
        })
        .filter_map(|(a, b)| {
            let a = mesh.source_vertex_slots[a]?;
            let b = mesh.source_vertex_slots[b]?;
            Some(edge(a, b))
        })
        .collect()
}

fn source_triangles(mesh: &HierarchyLeafMesh) -> BTreeSet<[usize; 3]> {
    mesh.mesh
        .active_triangle_slots()
        .filter_map(|face| {
            let triangle = mesh.mesh.triangles()[face].map(|site| mesh.source_vertex_slots[site]);
            let mut triangle = [triangle[0]?, triangle[1]?, triangle[2]?];
            triangle.sort_unstable();
            Some(triangle)
        })
        .collect()
}

fn mesh_edges(mesh: &HierarchyLeafMesh) -> BTreeSet<(usize, usize)> {
    mesh.mesh
        .active_triangle_slots()
        .flat_map(|face| {
            let [a, b, c] = mesh.mesh.triangles()[face];
            [edge(a, b), edge(b, c), edge(c, a)]
        })
        .collect()
}

fn edge_count(mesh: &HierarchyLeafMesh) -> usize {
    mesh_edges(mesh).len()
}

fn crossing_pairs(mesh: &HierarchyLeafMesh) -> usize {
    let edges = mesh_edges(mesh).into_iter().collect::<Vec<_>>();
    let mut count = 0;
    for (index, &(a, b)) in edges.iter().enumerate() {
        for &(c, d) in &edges[index + 1..] {
            if a == c || a == d || b == c || b == d {
                continue;
            }
            if minor_arc_crossing_strength(
                mesh.mesh.vertices()[a],
                mesh.mesh.vertices()[b],
                mesh.mesh.vertices()[c],
                mesh.mesh.vertices()[d],
            ) > 1.0e-14
            {
                count += 1;
            }
        }
    }
    count
}

fn rotation_mismatches(mesh: &HierarchyLeafMesh) -> Vec<RotationMismatch> {
    let mut next = BTreeMap::<usize, BTreeMap<usize, usize>>::new();
    for face in mesh.mesh.active_triangle_slots() {
        let [a, b, c] = mesh.mesh.triangles()[face];
        for (site, from, to) in [(a, b, c), (b, c, a), (c, a, b)] {
            let map = next.entry(site).or_default();
            if map.insert(from, to).is_some_and(|previous| previous != to) {
                map.clear();
            }
        }
    }

    let mut mismatches = Vec::new();
    for (site, links) in next {
        let Some(combinatorial_order) = directed_cycle(&links) else {
            continue;
        };
        let geometric_order = geometric_order(mesh, site, &combinatorial_order);
        if geometric_order.len() != combinatorial_order.len()
            || !cyclically_equal(&combinatorial_order, &geometric_order)
        {
            mismatches.push(RotationMismatch {
                compact_vertex: site,
                source_vertex: mesh.source_vertex_slots[site],
                combinatorial_order,
                geometric_order,
            });
        }
    }
    mismatches
}

fn directed_cycle(links: &BTreeMap<usize, usize>) -> Option<Vec<usize>> {
    let &start = links.keys().next()?;
    if links.len() < 3 || links.values().copied().collect::<BTreeSet<_>>().len() != links.len() {
        return None;
    }
    let mut order = Vec::with_capacity(links.len());
    let mut current = start;
    for _ in 0..links.len() {
        if order.contains(&current) {
            return None;
        }
        order.push(current);
        current = *links.get(&current)?;
    }
    (current == start).then_some(order)
}

fn geometric_order(mesh: &HierarchyLeafMesh, site: usize, neighbours: &[usize]) -> Vec<usize> {
    let Some(normal) = normalized(mesh.mesh.vertices()[site]) else {
        return Vec::new();
    };
    let axis = if normal.z.abs() < 0.9 {
        CartesianPoint::new(0.0, 0.0, 1.0)
    } else {
        CartesianPoint::new(1.0, 0.0, 0.0)
    };
    let Some(x_axis) = normalized(cross(axis, normal)) else {
        return Vec::new();
    };
    let y_axis = cross(normal, x_axis);
    let mut ordered = neighbours
        .iter()
        .copied()
        .filter_map(|neighbour| {
            let point = mesh.mesh.vertices()[neighbour];
            let tangent = CartesianPoint::new(
                point.x - dot(point, normal) * normal.x,
                point.y - dot(point, normal) * normal.y,
                point.z - dot(point, normal) * normal.z,
            );
            (magnitude(tangent) > 1.0e-14)
                .then(|| (dot(tangent, y_axis).atan2(dot(tangent, x_axis)), neighbour))
        })
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
    ordered
        .into_iter()
        .map(|(_, neighbour)| neighbour)
        .collect()
}

fn cyclically_equal(left: &[usize], right: &[usize]) -> bool {
    left.len() == right.len()
        && (left.is_empty()
            || right
                .iter()
                .position(|value| Some(value) == left.first())
                .is_some_and(|offset| {
                    left.iter()
                        .enumerate()
                        .all(|(index, value)| *value == right[(offset + index) % right.len()])
                }))
}

fn normalized(point: CartesianPoint) -> Option<CartesianPoint> {
    let length = magnitude(point);
    (length > 0.0 && length.is_finite())
        .then(|| CartesianPoint::new(point.x / length, point.y / length, point.z / length))
}

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{coarsen::HierarchyLeafMesh, mother_grid::MotherGrid};

    fn fixture() -> HierarchyLeafMesh {
        let grid = MotherGrid::generate(6).unwrap();
        HierarchyLeafMesh {
            source_vertex_slots: (0..grid.mesh.vertices().len()).map(Some).collect(),
            triangle_addresses: grid.triangle_addresses,
            mesh: grid.mesh,
        }
    }

    fn key() -> FullPolygonTopologyKey {
        FullPolygonTopologyKey {
            sector_id: 0,
            triangles: Vec::new(),
        }
    }

    #[test]
    fn w3_near_degenerate_start_is_reported() {
        let reference = fixture();
        let mut candidate = reference.clone();
        let [a, b, _] = candidate.mesh.triangles()[2];
        let pa = candidate.mesh.vertices()[a];
        let pb = candidate.mesh.vertices()[b];
        let point = normalized(CartesianPoint::new(
            pa.x * 0.999_999 + pb.x * 0.000_001,
            pa.y * 0.999_999 + pb.y * 0.000_001,
            pa.z * 0.999_999 + pb.z * 0.000_001,
        ))
        .unwrap();
        candidate.mesh.move_vertex(b, point);
        let audit = audit_embedding_transfer(key(), &reference, &candidate, &[]).unwrap();
        assert!(audit.near_degenerate_triangles > 0);
        assert_eq!(
            audit.outcome(),
            EmbeddingAuditOutcome::TopologyClosedNoUsableEmbedding
        );
    }

    #[test]
    fn w3_rotation_mismatch_is_not_called_angle_failure() {
        let reference = fixture();
        let mut candidate = reference.clone();
        let [_, b, c] = candidate.mesh.triangles()[2];
        let b_position = candidate.mesh.vertices()[b];
        let c_position = candidate.mesh.vertices()[c];
        candidate.mesh.move_vertex(b, c_position);
        candidate.mesh.move_vertex(c, b_position);
        let audit = audit_embedding_transfer(key(), &reference, &candidate, &[]).unwrap();
        assert!(!audit.rotation_mismatch_vertices.is_empty());
        assert_eq!(
            audit.outcome(),
            EmbeddingAuditOutcome::TopologyClosedNoUsableEmbedding
        );
    }

    #[test]
    fn fixed_only_degenerate_triangle_blocks_current_domain() {
        let reference = fixture();
        let mut candidate = reference.clone();
        let triangle = candidate.mesh.triangles()[2];
        candidate
            .mesh
            .move_vertex(triangle[2], candidate.mesh.vertices()[triangle[0]]);
        let audit = audit_embedding_transfer(key(), &reference, &candidate, &triangle).unwrap();
        assert!(audit.fixed_only_degenerate_triangles > 0);
        assert_eq!(
            audit.outcome(),
            EmbeddingAuditOutcome::TopologyClosedNoUsableEmbedding
        );
    }
}
