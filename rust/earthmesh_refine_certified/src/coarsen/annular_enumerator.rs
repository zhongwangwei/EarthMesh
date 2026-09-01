//! Canonical seam-cut enumeration for annular cells without interior vertices.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Edge = (usize, usize);
type Triangle = [usize; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VertexOccurrenceId {
    pub global_source_slot: usize,
    pub occurrence_ordinal: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OccurrenceEdge {
    pub a: VertexOccurrenceId,
    pub b: VertexOccurrenceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutAnnulusPolygon {
    pub root_bridge: Edge,
    pub occurrences: Vec<VertexOccurrenceId>,
    pub boundary_edges: Vec<OccurrenceEdge>,
    pub occurrence_to_global: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnnularTopologyKey {
    pub triangles: Vec<Triangle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnularTopology {
    pub root_bridge: Edge,
    pub triangles: Vec<Triangle>,
    pub topology_key: AnnularTopologyKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnnularEnumerationEvidence {
    pub root_bridges_considered: u64,
    pub cut_polygon_states: u64,
    pub glued_topologies: u64,
    pub duplicate_topologies: u64,
    pub glue_rejects: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullAnnularFamily {
    pub lower_vertices: usize,
    pub upper_vertices: usize,
    pub topologies: Vec<AnnularTopology>,
    pub evidence: AnnularEnumerationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnularEnumerationError {
    BoundaryTooShort,
    BoundaryIntersection,
    DuplicateBoundaryVertex,
    EmptyFamily,
    InvalidTopology(String),
}

pub fn cut_annulus_polygon(
    lower: &[usize],
    upper: &[usize],
    lower_root: usize,
    upper_root: usize,
) -> Result<CutAnnulusPolygon, AnnularEnumerationError> {
    validate_boundaries(lower, upper)?;
    let lower_root = lower_root % lower.len();
    let upper_root = upper_root % upper.len();
    let lower_slot = lower[lower_root];
    let upper_slot = upper[upper_root];
    let occurrence = |slot, ordinal| VertexOccurrenceId {
        global_source_slot: slot,
        occurrence_ordinal: ordinal,
    };
    let mut occurrences = vec![occurrence(lower_slot, 0)];
    occurrences.extend(
        (1..lower.len()).map(|offset| occurrence(lower[(lower_root + offset) % lower.len()], 0)),
    );
    occurrences.push(occurrence(lower_slot, 1));
    occurrences.push(occurrence(upper_slot, 1));
    occurrences.extend(
        (1..upper.len())
            .map(|offset| occurrence(upper[(upper_root + upper.len() - offset) % upper.len()], 0)),
    );
    occurrences.push(occurrence(upper_slot, 0));
    let boundary_edges = occurrences
        .iter()
        .copied()
        .zip(occurrences.iter().copied().cycle().skip(1))
        .take(occurrences.len())
        .map(|(a, b)| occurrence_edge(a, b))
        .collect();
    let occurrence_to_global = occurrences
        .iter()
        .map(|occurrence| occurrence.global_source_slot)
        .collect();
    Ok(CutAnnulusPolygon {
        root_bridge: edge(lower_slot, upper_slot),
        occurrences,
        boundary_edges,
        occurrence_to_global,
    })
}

pub fn enumerate_canonical_seam_annulus(
    lower: &[usize],
    upper: &[usize],
    forbidden_global_edges: &BTreeSet<Edge>,
) -> Result<FullAnnularFamily, AnnularEnumerationError> {
    validate_boundaries(lower, upper)?;
    let polygon_topologies = polygon_triangulations(lower.len() + upper.len() + 2);
    let mut evidence = AnnularEnumerationEvidence::default();
    let mut unique = BTreeMap::<AnnularTopologyKey, AnnularTopology>::new();
    for lower_root in 0..lower.len() {
        for upper_root in 0..upper.len() {
            evidence.root_bridges_considered += 1;
            let cut = cut_annulus_polygon(lower, upper, lower_root, upper_root)?;
            for occurrence_triangles in &polygon_topologies {
                evidence.cut_polygon_states += 1;
                match glue_cut_topology(
                    lower,
                    upper,
                    &cut,
                    occurrence_triangles,
                    forbidden_global_edges,
                ) {
                    Ok(topology) => {
                        evidence.glued_topologies += 1;
                        if unique
                            .insert(topology.topology_key.clone(), topology)
                            .is_some()
                        {
                            evidence.duplicate_topologies += 1;
                        }
                    }
                    Err(reason) => *evidence.glue_rejects.entry(reason).or_default() += 1,
                }
            }
        }
    }
    if unique.is_empty() {
        return Err(AnnularEnumerationError::EmptyFamily);
    }
    Ok(FullAnnularFamily {
        lower_vertices: lower.len(),
        upper_vertices: upper.len(),
        topologies: unique.into_values().collect(),
        evidence,
    })
}

pub fn brute_force_flip_annulus_keys(
    lower: &[usize],
    upper: &[usize],
) -> Result<BTreeSet<AnnularTopologyKey>, AnnularEnumerationError> {
    let seed = enumerate_canonical_seam_annulus(lower, upper, &BTreeSet::new())?
        .topologies
        .into_iter()
        .next()
        .ok_or(AnnularEnumerationError::EmptyFamily)?;
    let boundary = boundary_edges(lower, upper);
    let mut seen = BTreeSet::from([seed.topology_key.clone()]);
    let mut queue = VecDeque::from([seed.triangles]);
    while let Some(triangles) = queue.pop_front() {
        let incidence = edge_triangle_incidence(&triangles);
        for (&shared, owners) in &incidence {
            if boundary.contains(&shared) || owners.len() != 2 {
                continue;
            }
            let first = owners[0];
            let second = owners[1];
            let Some(a) = opposite_vertex(triangles[first], shared) else {
                continue;
            };
            let Some(b) = opposite_vertex(triangles[second], shared) else {
                continue;
            };
            let replacement = edge(a, b);
            if a == b || (replacement != shared && incidence.contains_key(&replacement)) {
                continue;
            }
            let mut candidate = triangles.clone();
            candidate[first] = triangle(a, b, shared.0);
            candidate[second] = triangle(a, b, shared.1);
            candidate.sort_unstable();
            if validate_global_annular_topology(lower, upper, &candidate, &BTreeSet::new()).is_err()
            {
                continue;
            }
            let key = AnnularTopologyKey {
                triangles: candidate.clone(),
            };
            if seen.insert(key) {
                queue.push_back(candidate);
            }
        }
    }
    Ok(seen)
}

pub fn certify_annular_topology(
    lower: &[usize],
    upper: &[usize],
    forbidden_global_edges: &BTreeSet<Edge>,
    triangles: &[Triangle],
) -> Result<AnnularTopology, AnnularEnumerationError> {
    validate_boundaries(lower, upper)?;
    let mut triangles = triangles.to_vec();
    triangles
        .iter_mut()
        .for_each(|triangle| triangle.sort_unstable());
    triangles.sort_unstable();
    validate_global_annular_topology(lower, upper, &triangles, forbidden_global_edges)
        .map_err(AnnularEnumerationError::InvalidTopology)?;
    let lower_set = lower.iter().copied().collect::<BTreeSet<_>>();
    let upper_set = upper.iter().copied().collect::<BTreeSet<_>>();
    let root_bridge = topology_edges(&triangles)
        .into_iter()
        .find(|(a, b)| {
            (lower_set.contains(a) && upper_set.contains(b))
                || (lower_set.contains(b) && upper_set.contains(a))
        })
        .ok_or_else(|| AnnularEnumerationError::InvalidTopology("annulus has no bridge".into()))?;
    let topology_key = AnnularTopologyKey {
        triangles: triangles.clone(),
    };
    Ok(AnnularTopology {
        root_bridge,
        triangles,
        topology_key,
    })
}

pub fn annular_small_exact_oracle_json() -> Result<String, AnnularEnumerationError> {
    let mut fixtures = Vec::new();
    let mut all_equal = true;
    for (m, n) in [(3, 3), (3, 4), (4, 4), (4, 5)] {
        let lower = (0..m).collect::<Vec<_>>();
        let upper = (100..100 + n).collect::<Vec<_>>();
        let csae = enumerate_canonical_seam_annulus(&lower, &upper, &BTreeSet::new())?;
        let csae_keys = csae
            .topologies
            .iter()
            .map(|topology| topology.topology_key.clone())
            .collect::<BTreeSet<_>>();
        let flip_keys = brute_force_flip_annulus_keys(&lower, &upper)?;
        let equal = csae_keys == flip_keys;
        all_equal &= equal;
        fixtures.push(format!(
            "{{\"lower\":{m},\"upper\":{n},\"csae_topologies\":{},\"flip_topologies\":{},\"root_bridges_considered\":{},\"cut_polygon_states\":{},\"families_equal\":{equal}}}",
            csae_keys.len(),
            flip_keys.len(),
            csae.evidence.root_bridges_considered,
            csae.evidence.cut_polygon_states,
        ));
    }
    Ok(format!(
        "{{\"schema_version\":1,\"taskbook_sha256\":\"cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e\",\"declared_topology_family\":\"FixedTwoBoundaryAnnulusNoInteriorVertices\",\"fixtures\":[{}],\"all_families_equal\":{all_equal},\"geometry_attempted\":false,\"product_gate_changed\":false}}",
        fixtures.join(",")
    ))
}

fn glue_cut_topology(
    lower: &[usize],
    upper: &[usize],
    cut: &CutAnnulusPolygon,
    occurrence_triangles: &[Triangle],
    forbidden_global_edges: &BTreeSet<Edge>,
) -> Result<AnnularTopology, String> {
    let mut triangles = Vec::with_capacity(occurrence_triangles.len());
    let mut occurrence_edges = BTreeMap::<Edge, BTreeSet<OccurrenceEdge>>::new();
    for &indices in occurrence_triangles {
        let occurrences = [
            cut.occurrences[indices[0]],
            cut.occurrences[indices[1]],
            cut.occurrences[indices[2]],
        ];
        let global = triangle(
            occurrences[0].global_source_slot,
            occurrences[1].global_source_slot,
            occurrences[2].global_source_slot,
        );
        if global[0] == global[1] || global[1] == global[2] {
            return Err("DegenerateGlobalTriangle".into());
        }
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            occurrence_edges
                .entry(edge(
                    occurrences[a].global_source_slot,
                    occurrences[b].global_source_slot,
                ))
                .or_default()
                .insert(occurrence_edge(occurrences[a], occurrences[b]));
        }
        triangles.push(global);
    }
    triangles.sort_unstable();
    if triangles.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("DuplicateGlobalTriangle".into());
    }
    if occurrence_edges
        .iter()
        .any(|(global, occurrences)| *global != cut.root_bridge && occurrences.len() != 1)
    {
        return Err("NonRootDuplicateGlobalEdge".into());
    }
    if occurrence_edges
        .get(&cut.root_bridge)
        .is_none_or(|occurrences| occurrences.len() != 2)
    {
        return Err("RootBridgeOccurrenceCount".into());
    }
    validate_global_annular_topology(lower, upper, &triangles, forbidden_global_edges)?;
    let bridges = topology_edges(&triangles)
        .into_iter()
        .filter(|(a, b)| {
            (lower.contains(a) && upper.contains(b)) || (lower.contains(b) && upper.contains(a))
        })
        .collect::<BTreeSet<_>>();
    if bridges.first() != Some(&cut.root_bridge) {
        return Err("NonCanonicalRootBridge".into());
    }
    let topology_key = AnnularTopologyKey {
        triangles: triangles.clone(),
    };
    Ok(AnnularTopology {
        root_bridge: cut.root_bridge,
        triangles,
        topology_key,
    })
}

fn validate_global_annular_topology(
    lower: &[usize],
    upper: &[usize],
    triangles: &[Triangle],
    forbidden_global_edges: &BTreeSet<Edge>,
) -> Result<(), String> {
    let vertex_count = lower.len() + upper.len();
    if triangles.len() != vertex_count {
        return Err("TriangleCount".into());
    }
    if triangles
        .iter()
        .any(|triangle| triangle[0] == triangle[1] || triangle[1] == triangle[2])
        || triangles.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err("InvalidTriangles".into());
    }
    let boundary = boundary_edges(lower, upper);
    let incidence = edge_triangle_incidence(triangles);
    if incidence
        .keys()
        .any(|edge| forbidden_global_edges.contains(edge))
        || boundary
            .iter()
            .any(|edge| incidence.get(edge).is_none_or(|owners| owners.len() != 1))
        || incidence.iter().any(|(edge, owners)| {
            !boundary.contains(edge) && owners.len() != 2
                || boundary.contains(edge) && owners.len() != 1
        })
    {
        return Err("EdgeIncidence".into());
    }
    if incidence.len() != 2 * vertex_count {
        return Err("EdgeCount".into());
    }
    if vertex_count as isize - incidence.len() as isize + triangles.len() as isize != 0 {
        return Err("Euler".into());
    }
    if !connected_graph(lower, upper, incidence.keys().copied()) {
        return Err("Disconnected".into());
    }
    let mut links = BTreeMap::<usize, BTreeSet<Edge>>::new();
    for triangle in triangles {
        for corner in 0..3 {
            let vertex = triangle[corner];
            if !links
                .entry(vertex)
                .or_default()
                .insert(edge(triangle[(corner + 1) % 3], triangle[(corner + 2) % 3]))
            {
                return Err("DuplicateLinkEdge".into());
            }
        }
    }
    if links.len() != vertex_count || links.values().any(|link| !is_single_path(link)) {
        return Err("BoundaryLinkNotPath".into());
    }
    Ok(())
}

fn polygon_triangulations(vertices: usize) -> Vec<Vec<Triangle>> {
    fn interval(
        first: usize,
        last: usize,
        memo: &mut BTreeMap<(usize, usize), Vec<Vec<Triangle>>>,
    ) -> Vec<Vec<Triangle>> {
        if last <= first + 1 {
            return vec![Vec::new()];
        }
        if let Some(cached) = memo.get(&(first, last)) {
            return cached.clone();
        }
        let mut out = Vec::new();
        for middle in first + 1..last {
            for left in interval(first, middle, memo) {
                for right in interval(middle, last, memo) {
                    let mut topology = left.clone();
                    topology.extend(right.iter().copied());
                    topology.push([first, middle, last]);
                    out.push(topology);
                }
            }
        }
        memo.insert((first, last), out.clone());
        out
    }
    interval(0, vertices - 1, &mut BTreeMap::new())
}

fn validate_boundaries(lower: &[usize], upper: &[usize]) -> Result<(), AnnularEnumerationError> {
    if lower.len() < 3 || upper.len() < 3 {
        return Err(AnnularEnumerationError::BoundaryTooShort);
    }
    let lower_set = lower.iter().copied().collect::<BTreeSet<_>>();
    let upper_set = upper.iter().copied().collect::<BTreeSet<_>>();
    if lower_set.len() != lower.len() || upper_set.len() != upper.len() {
        return Err(AnnularEnumerationError::DuplicateBoundaryVertex);
    }
    if !lower_set.is_disjoint(&upper_set) {
        return Err(AnnularEnumerationError::BoundaryIntersection);
    }
    Ok(())
}

fn connected_graph(
    lower: &[usize],
    upper: &[usize],
    edges: impl IntoIterator<Item = Edge>,
) -> bool {
    let vertices = lower.iter().chain(upper).copied().collect::<BTreeSet<_>>();
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for (a, b) in edges {
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    let Some(&seed) = vertices.first() else {
        return false;
    };
    let mut reached = BTreeSet::from([seed]);
    let mut queue = VecDeque::from([seed]);
    while let Some(vertex) = queue.pop_front() {
        for &next in adjacency.get(&vertex).into_iter().flatten() {
            if reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    reached == vertices
}

fn is_single_path(edges: &BTreeSet<Edge>) -> bool {
    if edges.is_empty() {
        return false;
    }
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in edges {
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    if adjacency.values().any(|neighbours| neighbours.len() > 2)
        || adjacency
            .values()
            .filter(|neighbours| neighbours.len() == 1)
            .count()
            != 2
    {
        return false;
    }
    let seed = *adjacency.keys().next().unwrap();
    let mut reached = BTreeSet::from([seed]);
    let mut queue = VecDeque::from([seed]);
    while let Some(vertex) = queue.pop_front() {
        for &next in &adjacency[&vertex] {
            if reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    reached.len() == adjacency.len()
}

fn edge_triangle_incidence(triangles: &[Triangle]) -> BTreeMap<Edge, Vec<usize>> {
    let mut incidence = BTreeMap::<Edge, Vec<usize>>::new();
    for (index, triangle) in triangles.iter().enumerate() {
        for edge in triangle_edges(*triangle) {
            incidence.entry(edge).or_default().push(index);
        }
    }
    incidence
}

fn topology_edges(triangles: &[Triangle]) -> BTreeSet<Edge> {
    triangles
        .iter()
        .flat_map(|triangle| triangle_edges(*triangle))
        .collect()
}

fn boundary_edges(lower: &[usize], upper: &[usize]) -> BTreeSet<Edge> {
    cycle_edges(lower).chain(cycle_edges(upper)).collect()
}

fn cycle_edges(cycle: &[usize]) -> impl Iterator<Item = Edge> + '_ {
    cycle
        .iter()
        .copied()
        .zip(cycle.iter().copied().cycle().skip(1))
        .take(cycle.len())
        .map(|(a, b)| edge(a, b))
}

fn opposite_vertex(triangle: Triangle, edge: Edge) -> Option<usize> {
    triangle
        .into_iter()
        .find(|vertex| *vertex != edge.0 && *vertex != edge.1)
}

fn triangle(a: usize, b: usize, c: usize) -> Triangle {
    let mut triangle = [a, b, c];
    triangle.sort_unstable();
    triangle
}

fn triangle_edges(triangle: Triangle) -> [Edge; 3] {
    [
        edge(triangle[0], triangle[1]),
        edge(triangle[1], triangle[2]),
        edge(triangle[2], triangle[0]),
    ]
}

fn occurrence_edge(a: VertexOccurrenceId, b: VertexOccurrenceId) -> OccurrenceEdge {
    if a < b {
        OccurrenceEdge { a, b }
    } else {
        OccurrenceEdge { a: b, b: a }
    }
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
