//! Abstract full-polygon CAT sector enumeration.
//!
//! This is topology only. Source-geometry visibility is telemetry, never a
//! hard filter.

use super::anchor_ear::OwnedTopologyTriangle;
use super::full_polygon_reachability::SectorPolygon;
use super::global_exact_merge::{fixed_triangles, mesh_edges};
use super::stratified_annulus::StratifiedAnnulus;
use super::transition_topology::triangulations;
use super::{build_stratified_annulus, HierarchyComponent};
use crate::mother_grid::MotherGrid;
use earthmesh_mesh::{arc_length_unit_sphere, cross, dot, magnitude, CartesianPoint};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct FullPolygonProblem {
    pub sector_id: u64,
    pub polygon_vertices: Vec<usize>,
    pub boundary_edges: BTreeSet<(usize, usize)>,
    pub forbidden_global_edges: BTreeSet<(usize, usize)>,
    pub diagonal_hints: BTreeMap<(usize, usize), DiagonalGeometryHint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiagonalGeometryHint {
    pub source_visible: bool,
    pub source_arc_length: f64,
    pub source_crossing_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FullPolygonTopologyKey {
    pub sector_id: u64,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullPolygonTopology {
    pub sector_id: u64,
    pub topology_id: u64,
    pub triangles: Vec<OwnedTopologyTriangle>,
    pub diagonals: BTreeSet<(usize, usize)>,
    pub vertex_incidences: BTreeMap<usize, u8>,
    pub vertex_link_edges: BTreeMap<usize, BTreeSet<(usize, usize)>>,
    pub geometry_hints: BTreeMap<(usize, usize), DiagonalGeometryHint>,
    pub geometry_key: FullPolygonGeometryKey,
    pub topology_key: FullPolygonTopologyKey,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FullPolygonGeometryKey {
    pub non_positive_triangles: usize,
    pub edge_crossings: usize,
    pub invalid_edges: usize,
}

impl FullPolygonGeometryKey {
    pub fn needs_untangle(self) -> bool {
        self.non_positive_triangles > 0 || self.edge_crossings > 0 || self.invalid_edges > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullPolygonFamily {
    pub sector_id: u64,
    pub polygon_size: usize,
    pub topology_count: usize,
    pub topologies: Vec<FullPolygonTopology>,
    pub incidence_domains: BTreeMap<usize, BTreeSet<u8>>,
}

pub fn enumerate_full_polygon_families(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<Vec<FullPolygonFamily>, String> {
    let stratified = build_stratified_annulus(source, component)
        .map_err(|error| format!("stratified annulus rejected component: {error:?}"))?;
    let fixed = fixed_triangles(source, component)?;
    enumerate_stratified_full_polygon_families(source, &stratified, &fixed)
}

pub fn enumerate_full_polygon_family(
    problem: &FullPolygonProblem,
) -> Result<FullPolygonFamily, String> {
    enumerate_full_polygon_family_with_geometry(problem, |_| FullPolygonGeometryKey::default())
}

fn enumerate_full_polygon_family_with_geometry(
    problem: &FullPolygonProblem,
    geometry_key: impl Fn(&[[usize; 3]]) -> FullPolygonGeometryKey,
) -> Result<FullPolygonFamily, String> {
    validate_problem(problem)?;
    let mut candidates = triangulations(&problem.polygon_vertices)
        .into_iter()
        .map(|raw| {
            let mut canonical = raw.into_iter().map(canonical).collect::<Vec<_>>();
            canonical.sort_unstable();
            canonical
        })
        .filter(|canonical| topology_allowed(canonical, problem))
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup();
    let mut topologies = Vec::new();
    for (topology_id, canonical) in candidates.into_iter().enumerate() {
        let topology_id = u64::try_from(topology_id)
            .map_err(|_| "full-polygon topology id exceeds u64".to_string())?;
        let diagonals = diagonals(&canonical, &problem.boundary_edges);
        let vertex_incidences = vertex_incidences(&canonical)?;
        let vertex_link_edges = vertex_link_edges(&canonical);
        if !local_vertex_links_are_paths(&vertex_link_edges) {
            continue;
        }
        let triangles = canonical
            .iter()
            .copied()
            .map(|vertices| OwnedTopologyTriangle {
                sector_id: problem.sector_id,
                topology_id,
                vertices,
            })
            .collect::<Vec<_>>();
        let geometry_hints = diagonals
            .iter()
            .filter_map(|edge| {
                problem
                    .diagonal_hints
                    .get(edge)
                    .copied()
                    .map(|hint| (*edge, hint))
            })
            .collect();
        let geometry_key = geometry_key(&canonical);
        topologies.push(FullPolygonTopology {
            sector_id: problem.sector_id,
            topology_id,
            triangles,
            diagonals: diagonals.clone(),
            vertex_incidences,
            vertex_link_edges,
            geometry_hints,
            geometry_key,
            topology_key: FullPolygonTopologyKey {
                sector_id: problem.sector_id,
                triangles: canonical,
            },
        });
    }
    topologies.sort_by(|left, right| {
        topology_geometry_order(left)
            .cmp(&topology_geometry_order(right))
            .then_with(|| left.topology_key.cmp(&right.topology_key))
    });
    let mut incidence_domains = BTreeMap::<usize, BTreeSet<u8>>::new();
    for topology in &topologies {
        for (&vertex, &count) in &topology.vertex_incidences {
            incidence_domains.entry(vertex).or_default().insert(count);
        }
    }
    Ok(FullPolygonFamily {
        sector_id: problem.sector_id,
        polygon_size: problem.polygon_vertices.len(),
        topology_count: topologies.len(),
        topologies,
        incidence_domains,
    })
}

fn topology_geometry_order(topology: &FullPolygonTopology) -> FullPolygonGeometryKey {
    topology.geometry_key
}

pub fn enumerate_stratified_full_polygon_families(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
    fixed_triangles: &[[usize; 3]],
) -> Result<Vec<FullPolygonFamily>, String> {
    let fixed_edges = mesh_edges(fixed_triangles);
    super::full_polygon_reachability::effective_sector_polygons(stratified)?
        .into_iter()
        .map(|sector| {
            let problem = problem_for_sector(source, sector, &fixed_edges);
            enumerate_full_polygon_family_with_geometry(&problem, |triangles| {
                topology_source_geometry_key(source, &problem.polygon_vertices, triangles)
            })
        })
        .collect()
}

pub(super) fn problem_for_sector(
    source: &MotherGrid,
    sector: SectorPolygon,
    fixed_edges: &BTreeSet<(usize, usize)>,
) -> FullPolygonProblem {
    let boundary_edges = polygon_boundary_edges(&sector.vertices);
    let forbidden_global_edges = fixed_edges
        .difference(&boundary_edges)
        .copied()
        .collect::<BTreeSet<_>>();
    FullPolygonProblem {
        sector_id: sector.id,
        diagonal_hints: diagonal_geometry_hints(source, &sector.vertices, &boundary_edges),
        polygon_vertices: sector.vertices,
        boundary_edges,
        forbidden_global_edges,
    }
}

fn edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}

fn diagonal_geometry_hints(
    source: &MotherGrid,
    vertices: &[usize],
    boundary_edges: &BTreeSet<(usize, usize)>,
) -> BTreeMap<(usize, usize), DiagonalGeometryHint> {
    let mut out = BTreeMap::new();
    for (i, &left) in vertices.iter().enumerate() {
        for &right in &vertices[i + 1..] {
            let edge = edge(left, right);
            if boundary_edges.contains(&edge) {
                continue;
            }
            let source_arc_length = source_arc_length(source, edge).unwrap_or(f64::INFINITY);
            let source_crossing_count = boundary_edges
                .iter()
                .filter(|&&(a, b)| {
                    ![a, b].contains(&left)
                        && ![a, b].contains(&right)
                        && source_edge_crossing_strength(source, edge, (a, b)) > 1.0e-18
                })
                .count();
            out.insert(
                edge,
                DiagonalGeometryHint {
                    source_visible: source_crossing_count == 0 && source_arc_length.is_finite(),
                    source_arc_length,
                    source_crossing_count,
                },
            );
        }
    }
    out
}

fn topology_source_geometry_key(
    source: &MotherGrid,
    polygon_vertices: &[usize],
    triangles: &[[usize; 3]],
) -> FullPolygonGeometryKey {
    let mut key = FullPolygonGeometryKey {
        non_positive_triangles: oriented_triangle_defect_count(source, polygon_vertices, triangles),
        ..Default::default()
    };
    let edges = edge_counts(triangles).into_keys().collect::<Vec<_>>();
    let valid_edges = edges
        .iter()
        .copied()
        .filter(|&edge| {
            let valid = source_arc_length(source, edge).is_some();
            key.invalid_edges += usize::from(!valid);
            valid
        })
        .collect::<Vec<_>>();
    for (i, &(a, b)) in valid_edges.iter().enumerate() {
        for &(c, d) in &valid_edges[i + 1..] {
            if a == c || a == d || b == c || b == d {
                continue;
            }
            if source_edge_crossing_strength(source, (a, b), (c, d)) > 1.0e-18 {
                key.edge_crossings += 1;
            }
        }
    }
    key
}

fn oriented_triangle_defect_count(
    source: &MotherGrid,
    polygon_vertices: &[usize],
    triangles: &[[usize; 3]],
) -> usize {
    let polygon_rank = polygon_vertices
        .iter()
        .copied()
        .enumerate()
        .map(|(rank, site)| (site, rank))
        .collect::<BTreeMap<_, _>>();
    let mut positive = 0usize;
    let mut negative = 0usize;
    let mut degenerate = 0usize;
    for triangle in triangles {
        let Some(oriented) = orient_triangle_by_polygon_order(*triangle, &polygon_rank) else {
            degenerate += 1;
            continue;
        };
        let points = oriented.map(|site| normalized_source_point(source, site));
        let [Some(a), Some(b), Some(c)] = points else {
            degenerate += 1;
            continue;
        };
        let det = dot(a, cross(b, c));
        if !det.is_finite() || det.abs() <= 1.0e-18 {
            degenerate += 1;
        } else if det > 0.0 {
            positive += 1;
        } else {
            negative += 1;
        }
    }
    degenerate + positive.min(negative)
}

fn orient_triangle_by_polygon_order(
    mut triangle: [usize; 3],
    polygon_rank: &BTreeMap<usize, usize>,
) -> Option<[usize; 3]> {
    triangle.sort_by_key(|site| polygon_rank.get(site).copied().unwrap_or(usize::MAX));
    triangle
        .iter()
        .all(|site| polygon_rank.contains_key(site))
        .then_some(triangle)
}

fn source_arc_length(source: &MotherGrid, edge: (usize, usize)) -> Option<f64> {
    if !source.mesh.is_vertex_live(edge.0) || !source.mesh.is_vertex_live(edge.1) {
        return None;
    }
    let length = arc_length_unit_sphere(
        source.mesh.vertices()[edge.0],
        source.mesh.vertices()[edge.1],
    );
    (length.is_finite() && length > 0.0 && length < std::f64::consts::PI - 1.0e-12)
        .then_some(length)
}

fn normalized_source_point(source: &MotherGrid, site: usize) -> Option<CartesianPoint> {
    source
        .mesh
        .is_vertex_live(site)
        .then(|| source.mesh.vertices()[site])
        .and_then(normalized_point)
}

fn source_edge_crossing_strength(
    source: &MotherGrid,
    left: (usize, usize),
    right: (usize, usize),
) -> f64 {
    if [left.0, left.1, right.0, right.1]
        .iter()
        .any(|&site| !source.mesh.is_vertex_live(site))
    {
        return 0.0;
    }
    minor_arc_crossing_strength(
        source.mesh.vertices()[left.0],
        source.mesh.vertices()[left.1],
        source.mesh.vertices()[right.0],
        source.mesh.vertices()[right.1],
    )
}

pub(super) fn minor_arc_crossing_strength(
    a: CartesianPoint,
    b: CartesianPoint,
    c: CartesianPoint,
    d: CartesianPoint,
) -> f64 {
    let Some(a) = normalized_point(a) else {
        return 0.0;
    };
    let Some(b) = normalized_point(b) else {
        return 0.0;
    };
    let Some(c) = normalized_point(c) else {
        return 0.0;
    };
    let Some(d) = normalized_point(d) else {
        return 0.0;
    };
    let ab = cross(a, b);
    let cd = cross(c, d);
    let ab_norm = magnitude(ab);
    let cd_norm = magnitude(cd);
    if ab_norm <= 1.0e-12 || cd_norm <= 1.0e-12 {
        return 0.0;
    }
    if dot(a, b) <= -1.0 + 1.0e-12 || dot(c, d) <= -1.0 + 1.0e-12 {
        return 0.0;
    }
    let s1 = dot(ab, c);
    let s2 = dot(ab, d);
    let s3 = dot(cd, a);
    let s4 = dot(cd, b);
    if ![s1, s2, s3, s4].iter().all(|v| v.is_finite()) || s1 * s2 >= -1.0e-24 || s3 * s4 >= -1.0e-24
    {
        return 0.0;
    }
    let x = cross(ab, cd);
    let Some(x) = normalized_point(x) else {
        return 0.0;
    };
    if ![x, scale_point(x, -1.0)]
        .into_iter()
        .any(|p| point_on_minor_arc(a, b, p) && point_on_minor_arc(c, d, p))
    {
        return 0.0;
    }
    ((-s1 * s2) / (ab_norm * ab_norm) * (-s3 * s4) / (cd_norm * cd_norm)).max(0.0)
}

fn point_on_minor_arc(a: CartesianPoint, b: CartesianPoint, p: CartesianPoint) -> bool {
    angular_distance(a, p) + angular_distance(p, b) <= angular_distance(a, b) + 1.0e-10
}

fn angular_distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    dot(a, b).clamp(-1.0, 1.0).acos()
}

fn normalized_point(point: CartesianPoint) -> Option<CartesianPoint> {
    let length = magnitude(point);
    (length > 0.0 && length.is_finite())
        .then(|| CartesianPoint::new(point.x / length, point.y / length, point.z / length))
}

fn scale_point(point: CartesianPoint, scale: f64) -> CartesianPoint {
    CartesianPoint::new(point.x * scale, point.y * scale, point.z * scale)
}

fn validate_problem(problem: &FullPolygonProblem) -> Result<(), String> {
    let n = problem.polygon_vertices.len();
    if n < 3 {
        return Err("full polygon has fewer than three vertices".into());
    }
    if n > u8::MAX as usize {
        return Err(format!(
            "full polygon has {n} vertices; u8 incidence limit is {}",
            u8::MAX
        ));
    }
    let unique = problem.polygon_vertices.iter().collect::<BTreeSet<_>>();
    if unique.len() != n {
        return Err("full polygon has duplicate boundary vertices".into());
    }
    let expected = polygon_boundary_edges(&problem.polygon_vertices);
    if problem.boundary_edges != expected {
        return Err("full polygon boundary_edges do not match polygon_vertices".into());
    }
    if !problem
        .boundary_edges
        .is_disjoint(&problem.forbidden_global_edges)
    {
        return Err("full polygon marks a boundary edge as forbidden".into());
    }
    Ok(())
}

fn topology_allowed(triangles: &[[usize; 3]], problem: &FullPolygonProblem) -> bool {
    let counts = edge_counts(triangles);
    !has_duplicate_triangles(triangles)
        && triangles.iter().copied().all(distinct)
        && problem
            .boundary_edges
            .iter()
            .all(|edge| counts.get(edge) == Some(&1))
        && counts.into_iter().all(|(edge, count)| {
            !problem.forbidden_global_edges.contains(&edge)
                && if problem.boundary_edges.contains(&edge) {
                    count == 1
                } else {
                    count == 2
                }
        })
}

fn diagonals(
    triangles: &[[usize; 3]],
    boundary_edges: &BTreeSet<(usize, usize)>,
) -> BTreeSet<(usize, usize)> {
    edge_counts(triangles)
        .into_keys()
        .filter(|edge| !boundary_edges.contains(edge))
        .collect()
}

fn vertex_incidences(triangles: &[[usize; 3]]) -> Result<BTreeMap<usize, u8>, String> {
    let mut out = BTreeMap::<usize, u8>::new();
    for triangle in triangles {
        for &vertex in triangle {
            let count = out.entry(vertex).or_default();
            *count = count
                .checked_add(1)
                .ok_or_else(|| format!("vertex {vertex} incidence exceeds u8"))?;
        }
    }
    Ok(out)
}

fn vertex_link_edges(triangles: &[[usize; 3]]) -> BTreeMap<usize, BTreeSet<(usize, usize)>> {
    let mut out = BTreeMap::<usize, BTreeSet<(usize, usize)>>::new();
    for [a, b, c] in triangles.iter().copied() {
        out.entry(a).or_default().insert(sorted(b, c));
        out.entry(b).or_default().insert(sorted(a, c));
        out.entry(c).or_default().insert(sorted(a, b));
    }
    out
}

fn local_vertex_links_are_paths(links: &BTreeMap<usize, BTreeSet<(usize, usize)>>) -> bool {
    links.values().all(|edges| {
        let mut degrees = BTreeMap::<usize, usize>::new();
        for &(a, b) in edges {
            *degrees.entry(a).or_default() += 1;
            *degrees.entry(b).or_default() += 1;
        }
        if degrees.values().filter(|&&degree| degree == 1).count() != 2
            || degrees.values().any(|&degree| degree > 2)
        {
            return false;
        }
        let start = *degrees.keys().next().expect("non-empty polygon link");
        let mut seen = BTreeSet::from([start]);
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            for &(a, b) in edges {
                let next = if a == node {
                    b
                } else if b == node {
                    a
                } else {
                    continue;
                };
                if seen.insert(next) {
                    stack.push(next);
                }
            }
        }
        seen.len() == degrees.len()
    })
}

fn edge_counts(triangles: &[[usize; 3]]) -> BTreeMap<(usize, usize), usize> {
    let mut out = BTreeMap::new();
    for [a, b, c] in triangles.iter().copied() {
        for edge in [sorted(a, b), sorted(b, c), sorted(c, a)] {
            *out.entry(edge).or_default() += 1;
        }
    }
    out
}

fn polygon_boundary_edges(vertices: &[usize]) -> BTreeSet<(usize, usize)> {
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(a, b)| sorted(a, b))
        .collect()
}

fn has_duplicate_triangles(triangles: &[[usize; 3]]) -> bool {
    let mut seen = BTreeSet::new();
    triangles
        .iter()
        .copied()
        .any(|triangle| !seen.insert(canonical(triangle)))
}

fn canonical(mut triangle: [usize; 3]) -> [usize; 3] {
    triangle.sort_unstable();
    triangle
}

fn distinct([a, b, c]: [usize; 3]) -> bool {
    a != b && a != c && b != c
}

fn sorted(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coarsen::{build_stratified_annulus, global_exact_merge, n6_legacy_mixed_fixture};
    use earthmesh_mesh::MeshState;

    fn problem(vertices: Vec<usize>) -> FullPolygonProblem {
        FullPolygonProblem {
            sector_id: 7,
            boundary_edges: polygon_boundary_edges(&vertices),
            polygon_vertices: vertices,
            forbidden_global_edges: BTreeSet::new(),
            diagonal_hints: BTreeMap::new(),
        }
    }

    #[test]
    fn old_two_chain_variants_are_included_in_full_polygon_family() {
        let (source, component) = n6_legacy_mixed_fixture().unwrap();
        let stratified = build_stratified_annulus(&source, &component).unwrap();
        let fixed = global_exact_merge::fixed_triangles(&source, &component).unwrap();
        let full =
            enumerate_stratified_full_polygon_families(&source, &stratified, &fixed).unwrap();
        let old = global_exact_merge::sector_variants(&stratified).unwrap();
        for (sector, old_family) in old.iter().enumerate() {
            let full_keys = full[sector]
                .topologies
                .iter()
                .map(|topology| topology.topology_key.triangles.clone())
                .collect::<BTreeSet<_>>();
            for old_topology in old_family {
                let mut key = old_topology
                    .iter()
                    .map(|triangle| canonical(triangle.vertices))
                    .collect::<Vec<_>>();
                key.sort_unstable();
                assert!(
                    full_keys.contains(&key),
                    "missing old sector {sector} topology {key:?}"
                );
            }
        }
    }

    #[test]
    fn same_chain_diagonal_is_retained() {
        let family = enumerate_full_polygon_family(&problem(vec![0, 1, 2, 3, 4])).unwrap();
        assert!(family
            .topologies
            .iter()
            .any(|topology| topology.diagonals.contains(&(0, 2))));
    }

    #[test]
    fn topology_geometry_key_uses_polygon_order_not_canonical_slot_order() {
        let n = |x, y, z| normalized_point(CartesianPoint::new(x, y, z)).unwrap();
        let mesh = MeshState::from_parts(
            vec![
                CartesianPoint::new(0.0, 0.0, 0.0),
                CartesianPoint::new(0.0, 0.0, 0.0),
                n(-1.0, -1.0, 2.0),
                n(1.0, -1.0, 2.0),
                n(1.0, 1.0, 2.0),
                n(-1.0, 1.0, 2.0),
            ],
            vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 4, 5]],
        )
        .unwrap();
        let source = MotherGrid {
            subdivision: 1,
            mesh,
            addresses: vec![None; 6],
            triangle_addresses: vec![None; 4],
        };
        let canonical_triangles = [[2, 3, 4], [2, 4, 5]];

        let forward = topology_source_geometry_key(&source, &[2, 3, 4, 5], &canonical_triangles);
        let reversed = topology_source_geometry_key(&source, &[5, 4, 3, 2], &canonical_triangles);

        assert_eq!(forward.non_positive_triangles, 0);
        assert_eq!(reversed.non_positive_triangles, 0);
    }

    #[test]
    fn minor_arc_crossing_rejects_opposite_intersections() {
        let p = |degrees: f64, z: f64| {
            let r = (1.0 - z * z).sqrt();
            CartesianPoint::new(
                r * degrees.to_radians().cos(),
                r * degrees.to_radians().sin(),
                z,
            )
        };
        let a = p(10.0, 0.0);
        let b = p(-10.0, 0.0);
        let c = CartesianPoint::new(-0.98, 0.0, 0.2);
        let d = CartesianPoint::new(-0.98, 0.0, -0.2);
        assert_eq!(minor_arc_crossing_strength(a, b, c, d), 0.0);
    }

    #[test]
    fn minor_arc_crossing_strength_changes_under_endpoint_motion() {
        let n = |x, y, z| normalized_point(CartesianPoint::new(x, y, z)).unwrap();
        let a = n(1.0, 0.0, 0.0);
        let b = n(0.0, 1.0, 0.0);
        let c = n(0.7, 0.7, -1.0);
        let d = n(0.7, 0.7, 1.0);
        let moved = n(0.9, 0.1, 1.0);
        let base = minor_arc_crossing_strength(a, b, c, d);
        let changed = minor_arc_crossing_strength(a, b, c, moved);
        assert!(base > 0.0);
        assert_ne!(base, changed);
    }
}
