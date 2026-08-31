use earthmesh_refine_certified::coarsen::{
    analyze_full_polygon_degree_reachability, enumerate_full_polygon_families,
    enumerate_full_polygon_family, n6_legacy_mixed_fixture, DiagonalGeometryHint,
    FullPolygonProblem,
};
use std::collections::{BTreeMap, BTreeSet};

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn boundary(vertices: &[usize]) -> BTreeSet<(usize, usize)> {
    vertices
        .iter()
        .copied()
        .zip(vertices.iter().copied().cycle().skip(1))
        .take(vertices.len())
        .map(|(a, b)| edge(a, b))
        .collect()
}

fn problem(n: usize) -> FullPolygonProblem {
    let polygon_vertices = (0..n).collect::<Vec<_>>();
    FullPolygonProblem {
        sector_id: 0,
        boundary_edges: boundary(&polygon_vertices),
        polygon_vertices,
        forbidden_global_edges: BTreeSet::new(),
        diagonal_hints: BTreeMap::new(),
    }
}

#[test]
fn catalan_counts_n3_to_n9() {
    for (n, expected) in [(3, 1), (4, 2), (5, 5), (6, 14), (7, 42), (8, 132), (9, 429)] {
        assert_eq!(
            enumerate_full_polygon_family(&problem(n))
                .unwrap()
                .topology_count,
            expected
        );
    }
}

#[test]
fn brute_force_diagonal_subsets_match_n3_to_n7() {
    for n in 3..=7 {
        let family = enumerate_full_polygon_family(&problem(n)).unwrap();
        let got = family
            .topologies
            .iter()
            .map(|topology| topology.diagonals.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(got, brute_force_triangulation_diagonals(n), "n={n}");
    }
}

#[test]
fn source_invisible_diagonal_is_not_removed_from_exact_family() {
    let mut p = problem(5);
    p.diagonal_hints.insert(
        edge(0, 2),
        DiagonalGeometryHint {
            source_visible: false,
            source_arc_length: 1.0,
            source_crossing_count: 1,
        },
    );
    let family = enumerate_full_polygon_family(&p).unwrap();
    let topology = family
        .topologies
        .iter()
        .find(|topology| topology.diagonals.contains(&edge(0, 2)))
        .expect("invisible diagonal was hard-filtered");
    assert!(!topology.geometry_hints[&edge(0, 2)].source_visible);
}

#[test]
fn fixed_outside_nonboundary_edge_is_rejected() {
    let mut p = problem(5);
    p.forbidden_global_edges.insert(edge(0, 2));
    let family = enumerate_full_polygon_family(&p).unwrap();
    assert!(family.topology_count < 5);
    assert!(family
        .topologies
        .iter()
        .all(|topology| !topology.diagonals.contains(&edge(0, 2))));
}

#[test]
fn n6_full_polygon_sector_counts_match_taskbook() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let families = enumerate_full_polygon_families(&source, &component).unwrap();
    assert_eq!(
        families
            .iter()
            .map(|family| family.topology_count)
            .collect::<Vec<_>>(),
        vec![5, 5, 132, 132, 5, 5, 132, 132, 14, 14, 14, 14, 132, 132]
    );
}

#[test]
fn n6_full_polygon_source_geometry_hints_are_populated() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let families = enumerate_full_polygon_families(&source, &component).unwrap();
    let hinted = families
        .iter()
        .flat_map(|family| &family.topologies)
        .filter(|topology| !topology.diagonals.is_empty())
        .count();
    assert!(hinted > 0);
    assert!(families
        .iter()
        .flat_map(|family| &family.topologies)
        .all(|topology| topology.geometry_hints.len() == topology.diagonals.len()));
    assert!(families
        .iter()
        .flat_map(|family| family.topologies.windows(2))
        .all(|pair| pair[0].geometry_key <= pair[1].geometry_key));
    assert!(families
        .iter()
        .flat_map(|family| &family.topologies)
        .any(|topology| topology.geometry_key.needs_untangle()));
}

#[test]
fn pr39_incidence_domains_match_full_polygon_enumerator() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let families = enumerate_full_polygon_families(&source, &component).unwrap();
    let evidence = analyze_full_polygon_degree_reachability(&source, &component).unwrap();
    let mut actual = BTreeMap::new();
    for family in families {
        for (vertex, domain) in family.incidence_domains {
            actual.insert((family.sector_id, vertex), domain);
        }
    }
    assert_eq!(actual, evidence.incidence_domains);
}

fn brute_force_triangulation_diagonals(n: usize) -> BTreeSet<BTreeSet<(usize, usize)>> {
    let b = boundary(&(0..n).collect::<Vec<_>>());
    let diagonals = (0..n)
        .flat_map(|i| (i + 1..n).map(move |j| edge(i, j)))
        .filter(|e| !b.contains(e))
        .collect::<Vec<_>>();
    let mut out = BTreeSet::new();
    choose(
        &diagonals,
        n.saturating_sub(3),
        0,
        &mut BTreeSet::new(),
        &mut out,
    );
    out
}

fn choose(
    diagonals: &[(usize, usize)],
    need: usize,
    start: usize,
    current: &mut BTreeSet<(usize, usize)>,
    out: &mut BTreeSet<BTreeSet<(usize, usize)>>,
) {
    if current.len() == need {
        out.insert(current.clone());
        return;
    }
    for i in start..diagonals.len() {
        let d = diagonals[i];
        if current.iter().all(|&e| !crosses(e, d)) {
            current.insert(d);
            choose(diagonals, need, i + 1, current, out);
            current.remove(&d);
        }
    }
}

fn crosses((a, b): (usize, usize), (c, d): (usize, usize)) -> bool {
    a != c
        && a != d
        && b != c
        && b != d
        && ((a < c && c < b && (d < a || b < d)) || (c < a && a < d && (b < c || d < b)))
}
