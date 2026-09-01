//! Frozen research-only N12 fixtures for Alpha6.

use super::{
    annulus::{parent_by_source_face, parent_graph, parent_layers_from_outside},
    n6_legacy_mixed_fixture, HierarchyComponent, HierarchyEdgeKey,
};
use crate::{mother_grid::analytic_counts, MotherGrid, TriangleAddress, VertexAddress};
use earthmesh_mesh::{arc_length_unit_sphere, spherical_triangle_area_unit};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedResearchFixtureManifest {
    pub name: String,
    pub schema_version: u32,
    pub source_n: usize,
    pub parent_n: usize,
    pub parents: Vec<TriangleAddress>,
    pub core_parents: Vec<TriangleAddress>,
    pub transition_parents: Vec<TriangleAddress>,
    pub expected_source_vertices: usize,
    pub expected_source_edges: usize,
    pub expected_source_faces: usize,
    pub original_anchor_vertices: Vec<VertexAddress>,
    pub physical_region_area: f64,
    pub coarse_core_area: f64,
    pub transition_area: f64,
    pub manifest_key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FixtureRepresentativenessTelemetry {
    pub median_edge_radians: f64,
    pub median_spherical_excess: f64,
    pub minimum_transition_parent_steps: usize,
    pub p50_transition_parent_steps: usize,
    pub p95_transition_parent_steps: usize,
    pub effective_band_count: usize,
    pub original_pentagons_in_transition: usize,
    pub nearest_pentagon_p50_steps: usize,
    pub nearest_pentagon_p95_steps: usize,
    pub degree_five_vertex_fraction: f64,
    pub core_to_transition_area_ratio: f64,
    pub boundary_length_to_core_area_ratio: f64,
    pub transition_vertices_per_core_parent: f64,
    pub fixed_to_movable_vertex_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedResearchFixture {
    pub source: MotherGrid,
    pub component: HierarchyComponent,
    pub source_levels: Vec<Option<usize>>,
    pub manifest: CertifiedResearchFixtureManifest,
    pub telemetry: FixtureRepresentativenessTelemetry,
}

pub fn lift_component_2_to_1(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<HierarchyComponent, String> {
    let lift = |values: &[TriangleAddress]| -> Result<BTreeSet<TriangleAddress>, String> {
        let mut lifted = BTreeSet::new();
        for parent in values {
            lifted.extend(
                parent
                    .children_2_to_1()
                    .ok_or_else(|| format!("invalid parent address {parent:?}"))?,
            );
        }
        Ok(lifted)
    };
    let parents = lift(&component.parents)?;
    let core = lift(&component.core_parents)?;
    let transition = lift(&component.transition_parents)?;
    component_from_sets(source, component.id, parents, core, transition)
}

pub fn n12_lifted_n6_fixture() -> Result<CertifiedResearchFixture, String> {
    let (_, n6) = n6_legacy_mixed_fixture()?;
    let source = MotherGrid::generate(12)?;
    let component = lift_component_2_to_1(&source, &n6)?;
    build_fixture("N12-Lifted-N6", source, component)
}

pub fn n12_interior_control_fixture() -> Result<CertifiedResearchFixture, String> {
    let source = MotherGrid::generate(12)?;
    let component = select_interior_control(&source)?;
    build_fixture("N12-Interior-Control", source, component)
}

pub fn research_fixture_manifest_json(manifest: &CertifiedResearchFixtureManifest) -> String {
    let canonical = manifest_json_without_key(manifest);
    format!(
        "{},\"manifest_key\":\"{}\"}}",
        canonical
            .strip_suffix('}')
            .expect("manifest JSON is an object"),
        manifest.manifest_key
    )
}

pub fn n12_research_fixture_manifests_json() -> Result<String, String> {
    let lifted = n12_lifted_n6_fixture()?;
    let interior = n12_interior_control_fixture()?;
    Ok(format!(
        "{{\"schema_version\":1,\"fixtures\":[{},{}]}}",
        research_fixture_manifest_json(&lifted.manifest),
        research_fixture_manifest_json(&interior.manifest)
    ))
}

pub fn research_fixture_telemetry_json(telemetry: &FixtureRepresentativenessTelemetry) -> String {
    format!(
        "{{\"median_edge_radians\":{:.17e},\"median_spherical_excess\":{:.17e},\"minimum_transition_parent_steps\":{},\"p50_transition_parent_steps\":{},\"p95_transition_parent_steps\":{},\"effective_band_count\":{},\"original_pentagons_in_transition\":{},\"nearest_pentagon_p50_steps\":{},\"nearest_pentagon_p95_steps\":{},\"degree_five_vertex_fraction\":{:.17e},\"core_to_transition_area_ratio\":{:.17e},\"boundary_length_to_core_area_ratio\":{:.17e},\"transition_vertices_per_core_parent\":{:.17e},\"fixed_to_movable_vertex_ratio\":{:.17e}}}",
        telemetry.median_edge_radians,
        telemetry.median_spherical_excess,
        telemetry.minimum_transition_parent_steps,
        telemetry.p50_transition_parent_steps,
        telemetry.p95_transition_parent_steps,
        telemetry.effective_band_count,
        telemetry.original_pentagons_in_transition,
        telemetry.nearest_pentagon_p50_steps,
        telemetry.nearest_pentagon_p95_steps,
        telemetry.degree_five_vertex_fraction,
        telemetry.core_to_transition_area_ratio,
        telemetry.boundary_length_to_core_area_ratio,
        telemetry.transition_vertices_per_core_parent,
        telemetry.fixed_to_movable_vertex_ratio,
    )
}

pub fn n12_research_fixture_report_json() -> Result<String, String> {
    let fixtures = [n12_lifted_n6_fixture()?, n12_interior_control_fixture()?];
    let body = fixtures
        .iter()
        .map(|fixture| {
            format!(
                "{{\"manifest\":{},\"representativeness\":{}}}",
                research_fixture_manifest_json(&fixture.manifest),
                research_fixture_telemetry_json(&fixture.telemetry),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"schema_version\":1,\"research_only\":true,\"fixtures\":[{body}]}}"
    ))
}

pub fn research_fixture_guard_parents(
    source: &MotherGrid,
    component: &HierarchyComponent,
    rings: usize,
) -> Result<BTreeSet<TriangleAddress>, String> {
    let by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let graph = parent_graph(source, &by_face).map_err(|error| format!("{error:?}"))?;
    Ok(guard_set(
        &component.parents.iter().copied().collect(),
        &graph,
        rings,
    ))
}

fn build_fixture(
    name: &str,
    source: MotherGrid,
    component: HierarchyComponent,
) -> Result<CertifiedResearchFixture, String> {
    let source_levels = source_levels(&source, &component.parents)?;
    let expected = analytic_counts(source.subdivision)
        .ok_or_else(|| "N12 analytic counts overflow".to_string())?;
    let anchors = component_anchor_vertices(&source, &component.parents)?;
    let mut manifest = CertifiedResearchFixtureManifest {
        name: name.into(),
        schema_version: 1,
        source_n: source.subdivision,
        parent_n: source.subdivision / 2,
        parents: component.parents.clone(),
        core_parents: component.core_parents.clone(),
        transition_parents: component.transition_parents.clone(),
        expected_source_vertices: expected.0,
        expected_source_edges: expected.1,
        expected_source_faces: expected.2,
        original_anchor_vertices: anchors,
        physical_region_area: parent_area(&component.parents)?,
        coarse_core_area: parent_area(&component.core_parents)?,
        transition_area: parent_area(&component.transition_parents)?,
        manifest_key: String::new(),
    };
    manifest.manifest_key = format!(
        "{:016x}",
        fnv1a(manifest_json_without_key(&manifest).bytes())
    );
    let telemetry = representativeness_telemetry(&source, &component, &manifest)?;
    Ok(CertifiedResearchFixture {
        source,
        component,
        source_levels,
        manifest,
        telemetry,
    })
}

fn component_from_sets(
    source: &MotherGrid,
    id: u64,
    parents: BTreeSet<TriangleAddress>,
    core: BTreeSet<TriangleAddress>,
    transition: BTreeSet<TriangleAddress>,
) -> Result<HierarchyComponent, String> {
    if parents.is_empty() || core.is_empty() || transition.is_empty() {
        return Err("research fixture requires non-empty parent, core, and transition sets".into());
    }
    if !core.is_disjoint(&transition)
        || core.union(&transition).copied().collect::<BTreeSet<_>>() != parents
    {
        return Err("research fixture core and transition must exactly partition parents".into());
    }
    let by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let graph = parent_graph(source, &by_face).map_err(|error| format!("{error:?}"))?;
    if parents.iter().any(|parent| !graph.contains_key(parent)) {
        return Err("research fixture contains a parent outside the source hierarchy".into());
    }
    let boundary_edges = parents
        .iter()
        .flat_map(|left| {
            graph[left]
                .iter()
                .filter(|right| !parents.contains(right))
                .map(move |right| canonical_edge(*left, *right))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(HierarchyComponent {
        id,
        parents: parents.into_iter().collect(),
        boundary_edges,
        core_parents: core.into_iter().collect(),
        transition_parents: transition.into_iter().collect(),
    })
}

fn select_interior_control(source: &MotherGrid) -> Result<HierarchyComponent, String> {
    use crate::TriangleOrientation::{Down as D, Up as U};
    // Frozen result of the initial deterministic radius/width search. Keeping
    // addresses here prevents the control region from drifting with solvers.
    let parents = [
        (0, 0, 1, D),
        (0, 0, 2, D),
        (0, 1, 0, D),
        (0, 1, 1, U),
        (0, 1, 1, D),
        (0, 1, 2, U),
        (0, 1, 2, D),
        (0, 2, 0, D),
        (0, 2, 1, U),
        (0, 2, 1, D),
    ]
    .into_iter()
    .map(|(base_face, i, j, orientation)| TriangleAddress {
        base_face,
        i,
        j,
        n: 6,
        orientation,
    })
    .collect::<BTreeSet<_>>();
    let core = BTreeSet::from([TriangleAddress {
        base_face: 0,
        i: 1,
        j: 1,
        n: 6,
        orientation: D,
    }]);
    let transition = parents.difference(&core).copied().collect();
    component_from_sets(source, 1, parents, core, transition)
}

fn source_levels(
    source: &MotherGrid,
    parents: &[TriangleAddress],
) -> Result<Vec<Option<usize>>, String> {
    let parents = parents.iter().copied().collect::<BTreeSet<_>>();
    let mut levels = vec![None; source.mesh.vertices().len()];
    for face in source.mesh.active_triangle_slots() {
        let parent = source.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .ok_or_else(|| format!("active source face {face} has no parent"))?;
        let level = usize::from(!parents.contains(&parent));
        for site in source.mesh.triangles()[face] {
            levels[site] = Some(levels[site].unwrap_or(0).max(level));
        }
    }
    Ok(levels)
}

fn component_anchor_vertices(
    source: &MotherGrid,
    parents: &[TriangleAddress],
) -> Result<Vec<VertexAddress>, String> {
    let parents = parents.iter().copied().collect::<BTreeSet<_>>();
    let mut anchors = BTreeSet::new();
    for face in source.mesh.active_triangle_slots() {
        let parent = source.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .ok_or_else(|| format!("active source face {face} has no parent"))?;
        if !parents.contains(&parent) {
            continue;
        }
        for site in source.mesh.triangles()[face] {
            if let Some(address @ VertexAddress::IcosahedronVertex(_)) =
                source.addresses[site].clone()
            {
                anchors.insert(address);
            }
        }
    }
    Ok(anchors.into_iter().collect())
}

fn anchor_parents(
    source: &MotherGrid,
    by_face: &BTreeMap<usize, TriangleAddress>,
) -> BTreeSet<TriangleAddress> {
    source
        .mesh
        .active_triangle_slots()
        .filter(|face| {
            source.mesh.triangles()[*face].iter().any(|site| {
                matches!(
                    source.addresses[*site],
                    Some(VertexAddress::IcosahedronVertex(_))
                )
            })
        })
        .map(|face| by_face[&face])
        .collect()
}

fn representativeness_telemetry(
    source: &MotherGrid,
    component: &HierarchyComponent,
    manifest: &CertifiedResearchFixtureManifest,
) -> Result<FixtureRepresentativenessTelemetry, String> {
    let by_face = parent_by_source_face(source).map_err(|error| format!("{error:?}"))?;
    let graph = parent_graph(source, &by_face).map_err(|error| format!("{error:?}"))?;
    let parents = component.parents.iter().copied().collect::<BTreeSet<_>>();
    let transition = component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let layers =
        parent_layers_from_outside(&parents, &graph).map_err(|error| format!("{error:?}"))?;
    let mut transition_steps = transition
        .iter()
        .map(|parent| layers[parent])
        .collect::<Vec<_>>();
    transition_steps.sort_unstable();

    let anchor_parents = anchor_parents(source, &by_face);
    let pentagon_distances = graph_distances(&graph, anchor_parents.iter().copied());
    let mut nearest_pentagon = transition
        .iter()
        .filter_map(|parent| pentagon_distances.get(parent).copied())
        .collect::<Vec<_>>();
    nearest_pentagon.sort_unstable();

    let component_faces = by_face
        .iter()
        .filter_map(|(&face, parent)| parents.contains(parent).then_some(face))
        .collect::<Vec<_>>();
    let transition_faces = by_face
        .iter()
        .filter_map(|(&face, parent)| transition.contains(parent).then_some(face))
        .collect::<Vec<_>>();
    let mut edges = BTreeSet::new();
    let mut excesses = Vec::new();
    let mut vertices = BTreeSet::new();
    let mut transition_vertices = BTreeSet::new();
    for &face in &component_faces {
        let triangle = source.mesh.triangles()[face];
        vertices.extend(triangle);
        edges.insert(sorted_pair(triangle[0], triangle[1]));
        edges.insert(sorted_pair(triangle[1], triangle[2]));
        edges.insert(sorted_pair(triangle[2], triangle[0]));
        excesses.push(spherical_triangle_area_unit(
            triangle.map(|site| source.mesh.vertices()[site]),
        ));
    }
    for face in transition_faces {
        transition_vertices.extend(source.mesh.triangles()[face]);
    }
    let mut edge_lengths = edges
        .iter()
        .map(|&(a, b)| arc_length_unit_sphere(source.mesh.vertices()[a], source.mesh.vertices()[b]))
        .collect::<Vec<_>>();
    edge_lengths.sort_by(f64::total_cmp);
    excesses.sort_by(f64::total_cmp);
    let fixed = vertices
        .iter()
        .filter(|site| {
            matches!(
                source.addresses[**site],
                Some(VertexAddress::IcosahedronVertex(_))
            )
        })
        .count();
    let movable = vertices.len().saturating_sub(fixed);
    let boundary_length = component
        .boundary_edges
        .iter()
        .map(|(left, right)| parent_shared_edge_length(source, &by_face, *left, *right))
        .sum::<Result<f64, _>>()?;

    Ok(FixtureRepresentativenessTelemetry {
        median_edge_radians: median_f64(&edge_lengths),
        median_spherical_excess: median_f64(&excesses),
        minimum_transition_parent_steps: transition_steps.first().copied().unwrap_or(0),
        p50_transition_parent_steps: percentile_usize(&transition_steps, 50),
        p95_transition_parent_steps: percentile_usize(&transition_steps, 95),
        effective_band_count: layers.values().copied().max().unwrap_or(0) + 1,
        original_pentagons_in_transition: component_anchor_vertices(
            source,
            &component.transition_parents,
        )?
        .len(),
        nearest_pentagon_p50_steps: percentile_usize(&nearest_pentagon, 50),
        nearest_pentagon_p95_steps: percentile_usize(&nearest_pentagon, 95),
        degree_five_vertex_fraction: fixed as f64 / vertices.len().max(1) as f64,
        core_to_transition_area_ratio: manifest.coarse_core_area / manifest.transition_area,
        boundary_length_to_core_area_ratio: boundary_length / manifest.coarse_core_area,
        transition_vertices_per_core_parent: transition_vertices.len() as f64
            / component.core_parents.len().max(1) as f64,
        fixed_to_movable_vertex_ratio: fixed as f64 / movable.max(1) as f64,
    })
}

fn parent_shared_edge_length(
    source: &MotherGrid,
    by_face: &BTreeMap<usize, TriangleAddress>,
    left: TriangleAddress,
    right: TriangleAddress,
) -> Result<f64, String> {
    let mut left_edges = BTreeSet::new();
    for (&face, parent) in by_face {
        if *parent == left {
            let [a, b, c] = source.mesh.triangles()[face];
            left_edges.extend([sorted_pair(a, b), sorted_pair(b, c), sorted_pair(c, a)]);
        }
    }
    let mut shared = BTreeSet::new();
    for (&face, parent) in by_face {
        if *parent == right {
            let [a, b, c] = source.mesh.triangles()[face];
            for edge in [sorted_pair(a, b), sorted_pair(b, c), sorted_pair(c, a)] {
                if left_edges.contains(&edge) {
                    shared.insert(edge);
                }
            }
        }
    }
    if shared.is_empty() {
        return Err(format!(
            "boundary parents {left:?} and {right:?} have no shared source edge"
        ));
    }
    Ok(shared
        .into_iter()
        .map(|(a, b)| arc_length_unit_sphere(source.mesh.vertices()[a], source.mesh.vertices()[b]))
        .sum())
}

fn parent_area(parents: &[TriangleAddress]) -> Result<f64, String> {
    if parents.is_empty() {
        return Ok(0.0);
    }
    let n = parents[0].n;
    let grid = MotherGrid::generate(n)?;
    let wanted = parents.iter().copied().collect::<BTreeSet<_>>();
    Ok(grid
        .mesh
        .active_triangle_slots()
        .filter(|face| {
            grid.triangle_addresses[*face].is_some_and(|address| wanted.contains(&address))
        })
        .map(|face| {
            spherical_triangle_area_unit(
                grid.mesh.triangles()[face].map(|site| grid.mesh.vertices()[site]),
            )
        })
        .sum())
}

fn graph_distances(
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    seeds: impl IntoIterator<Item = TriangleAddress>,
) -> BTreeMap<TriangleAddress, usize> {
    let mut distances = BTreeMap::new();
    let mut queue = VecDeque::new();
    for seed in seeds {
        if graph.contains_key(&seed) && distances.insert(seed, 0).is_none() {
            queue.push_back(seed);
        }
    }
    while let Some(parent) = queue.pop_front() {
        let next = distances[&parent] + 1;
        for &neighbour in &graph[&parent] {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(neighbour) {
                entry.insert(next);
                queue.push_back(neighbour);
            }
        }
    }
    distances
}

fn guard_set(
    parents: &BTreeSet<TriangleAddress>,
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    rings: usize,
) -> BTreeSet<TriangleAddress> {
    let distances = graph_distances(graph, parents.iter().copied());
    distances
        .into_iter()
        .filter_map(|(parent, distance)| (distance > 0 && distance <= rings).then_some(parent))
        .collect()
}

fn manifest_json_without_key(manifest: &CertifiedResearchFixtureManifest) -> String {
    format!(
        "{{\"name\":\"{}\",\"schema_version\":{},\"source_n\":{},\"parent_n\":{},\"parents\":{},\"core_parents\":{},\"transition_parents\":{},\"expected_source_vertices\":{},\"expected_source_edges\":{},\"expected_source_faces\":{},\"original_anchor_vertices\":{},\"physical_region_area\":{:.17e},\"coarse_core_area\":{:.17e},\"transition_area\":{:.17e}}}",
        manifest.name,
        manifest.schema_version,
        manifest.source_n,
        manifest.parent_n,
        address_list_json(&manifest.parents),
        address_list_json(&manifest.core_parents),
        address_list_json(&manifest.transition_parents),
        manifest.expected_source_vertices,
        manifest.expected_source_edges,
        manifest.expected_source_faces,
        vertex_list_json(&manifest.original_anchor_vertices),
        manifest.physical_region_area,
        manifest.coarse_core_area,
        manifest.transition_area,
    )
}

fn address_list_json(values: &[TriangleAddress]) -> String {
    let body = values
        .iter()
        .map(|address| {
            format!(
                "[{}, {}, {}, {}, \"{:?}\"]",
                address.base_face, address.i, address.j, address.n, address.orientation
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn vertex_list_json(values: &[VertexAddress]) -> String {
    let body = values
        .iter()
        .map(|address| format!("\"{address:?}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{body}]")
}

fn fnv1a(values: impl IntoIterator<Item = u8>) -> u64 {
    values.into_iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn percentile_usize(values: &[usize], percentile: usize) -> usize {
    if values.is_empty() {
        return 0;
    }
    values[(values.len() - 1) * percentile / 100]
}

fn median_f64(values: &[f64]) -> f64 {
    match values.len() {
        0 => 0.0,
        n if n.is_multiple_of(2) => (values[n / 2 - 1] + values[n / 2]) / 2.0,
        n => values[n / 2],
    }
}

fn canonical_edge(left: TriangleAddress, right: TriangleAddress) -> HierarchyEdgeKey {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn sorted_pair(left: usize, right: usize) -> (usize, usize) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}
