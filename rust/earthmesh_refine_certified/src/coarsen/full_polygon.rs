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
    pub topology_key: FullPolygonTopologyKey,
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
    enumerate_stratified_full_polygon_families(&stratified, &fixed)
}

pub fn enumerate_full_polygon_family(
    problem: &FullPolygonProblem,
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
        topologies.push(FullPolygonTopology {
            sector_id: problem.sector_id,
            topology_id,
            triangles,
            diagonals: diagonals.clone(),
            vertex_incidences,
            vertex_link_edges,
            geometry_hints,
            topology_key: FullPolygonTopologyKey {
                sector_id: problem.sector_id,
                triangles: canonical,
            },
        });
    }
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

pub fn enumerate_stratified_full_polygon_families(
    stratified: &StratifiedAnnulus,
    fixed_triangles: &[[usize; 3]],
) -> Result<Vec<FullPolygonFamily>, String> {
    let fixed_edges = mesh_edges(fixed_triangles);
    super::full_polygon_reachability::effective_sector_polygons(stratified)?
        .into_iter()
        .map(|sector| enumerate_full_polygon_family(&problem_for_sector(sector, &fixed_edges)))
        .collect()
}

pub(super) fn problem_for_sector(
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
        polygon_vertices: sector.vertices,
        boundary_edges,
        forbidden_global_edges,
        diagonal_hints: BTreeMap::new(),
    }
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
        let full = enumerate_stratified_full_polygon_families(&stratified, &fixed).unwrap();
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
}
