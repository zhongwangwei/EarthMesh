//! Coupled-annulus extraction for certified transition development.
//!
//! PR35 is deliberately topology-free: it only partitions source faces, extracts
//! existing source rings, and records boundary incidence contracts.

use super::{core_condensation::source_face_slot, HierarchyComponent};
use crate::mother_grid::{MotherGrid, TriangleAddress, VertexAddress};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingAnchorKind {
    Ordinary,
    IcosahedronPentagon { base_vertex: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingVertexRole {
    FixedInnerGuard,
    CoarseInterface,
    Bridge,
    Intermediate,
    FineInterface,
    FixedOuterGuard,
    OriginalIcosahedronVertex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleOrientation {
    SourceOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingVertex {
    pub source_slot: usize,
    pub address: VertexAddress,
    pub role: RingVertexRole,
    pub anchor_kind: RingAnchorKind,
    pub fixed_position: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryIncidenceContract {
    pub source_slot: usize,
    pub address: VertexAddress,
    pub anchor_kind: RingAnchorKind,
    pub fixed_position: bool,
    pub external_triangle_valence: u8,
    pub allowed_global_degree_min: u8,
    pub allowed_global_degree_max: u8,
    pub required_patch_valence_min: u8,
    pub required_patch_valence_max: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RingCycle {
    pub id: usize,
    pub vertices: Vec<RingVertex>,
    pub orientation: CycleOrientation,
    pub target_scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoupledAnnulus {
    pub component_id: u64,
    pub inner_guard: RingCycle,
    pub coarse_interface: RingCycle,
    pub intermediate_rings: Vec<RingCycle>,
    pub fine_interface: RingCycle,
    pub outer_guard: RingCycle,
    pub boundary_contracts: Vec<BoundaryIncidenceContract>,
    pub annulus_face_slots: Vec<usize>,
    pub fixed_outside_face_slots: Vec<usize>,
    pub anchor_star_guard_face_slots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnulusExtractionError {
    InvalidComponent(String),
    InvalidAnchorIncidence {
        source_slot: usize,
        external_triangle_valence: u8,
    },
    UnsupportedInteriorIcosahedronVertex {
        source_slot: usize,
        address: VertexAddress,
    },
    UnsupportedPentagonHole {
        source_slot: usize,
        address: VertexAddress,
    },
    UnsupportedIntersectingCycles {
        source_slot: usize,
        address: VertexAddress,
    },
    UnsupportedMultiCycleAnnulus {
        boundary: &'static str,
        cycles: usize,
    },
    MissingRing(&'static str),
}

pub fn extract_coupled_annulus(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> Result<CoupledAnnulus, AnnulusExtractionError> {
    let core = set(component.core_parents.iter().copied());
    let transition = set(component.transition_parents.iter().copied());
    validate_component(source, component, &core, &transition)?;

    let parents = set(component.parents.iter().copied());
    let parent_by_face = parent_by_source_face(source)?;
    let parent_graph = parent_graph(source, &parent_by_face)?;
    if let Some(parent) = parents
        .iter()
        .find(|parent| !parent_graph.contains_key(parent))
    {
        return Err(AnnulusExtractionError::InvalidComponent(format!(
            "component parent {parent:?} is not active in the source grid"
        )));
    }
    reject_pentagon_holes(source, &parents, &parent_graph, &parent_by_face)?;

    let mut annulus_faces = BTreeSet::new();
    for parent in &transition {
        annulus_faces.extend(source_face_slots(source, *parent)?);
    }
    if annulus_faces.is_empty() {
        return Err(AnnulusExtractionError::MissingRing(
            "annulus mutable domain",
        ));
    }
    let fixed_outside_faces = source
        .mesh
        .active_triangle_slots()
        .filter(|face| !annulus_faces.contains(face))
        .collect::<BTreeSet<_>>();
    let (annulus_incidence, outside_incidence) =
        vertex_incidence(source, &annulus_faces, &fixed_outside_faces);
    reject_strict_interior_pentagons(source, &annulus_incidence, &outside_incidence)?;

    let layers = parent_layers_from_outside(&parents, &parent_graph)?;
    let max_layer = layers
        .values()
        .copied()
        .max()
        .ok_or(AnnulusExtractionError::MissingRing("inner_guard"))?;
    let max_transition_layer = transition
        .iter()
        .filter_map(|parent| layers.get(parent).copied())
        .max()
        .ok_or(AnnulusExtractionError::MissingRing("intermediate"))?;
    if max_layer < 2 || max_transition_layer == 0 {
        return Err(AnnulusExtractionError::MissingRing("intermediate"));
    }

    let inner_guard_cycle = one_cycle(
        "inner_guard",
        coarse_edges_between(
            source,
            &layer(&layers, max_layer),
            &layer(&layers, max_layer - 1),
        )?,
        true,
    )?;
    let coarse_cycle = one_cycle(
        "coarse_interface",
        coarse_edges_between(source, &core, &transition)?,
        false,
    )?;

    let mut intermediate_rings = Vec::new();
    for min_inside_distance in (1..=max_transition_layer).rev() {
        let inside = layers
            .iter()
            .filter_map(|(&parent, &distance)| (distance >= min_inside_distance).then_some(parent))
            .collect::<BTreeSet<_>>();
        intermediate_rings.push(ring(
            2 + intermediate_rings.len(),
            one_cycle(
                "intermediate",
                fine_boundary_edges(source, &parent_by_face, &inside)?,
                true,
            )?,
            RingVertexRole::Intermediate,
            source,
            false,
        )?);
    }

    let immediate_outside = outside_neighbours(&parents, &parent_graph);
    let component_plus_outside = parents
        .union(&immediate_outside)
        .copied()
        .collect::<BTreeSet<_>>();
    let fine_cycle = one_cycle(
        "fine_interface",
        fine_boundary_edges(source, &parent_by_face, &parents)?,
        true,
    )?;
    let outer_guard_cycle = one_cycle(
        "outer_guard",
        fine_boundary_edges(source, &parent_by_face, &component_plus_outside)?,
        true,
    )?;
    reject_boundary_cycle_intersection(source, &coarse_cycle, &fine_cycle)?;

    let boundary_contracts = boundary_contracts(source, &annulus_incidence, &outside_incidence)?;
    let anchor_slots = inner_guard_cycle
        .iter()
        .chain(&coarse_cycle)
        .chain(
            intermediate_rings
                .iter()
                .flat_map(|ring| ring.vertices.iter().map(|vertex| &vertex.source_slot)),
        )
        .chain(&fine_cycle)
        .chain(&outer_guard_cycle)
        .copied()
        .filter(|&slot| {
            matches!(
                source.addresses.get(slot).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronVertex(_))
            )
        })
        .collect::<BTreeSet<_>>();
    let anchor_star_guard_face_slots = source
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            source.mesh.triangles()[face]
                .iter()
                .any(|slot| anchor_slots.contains(slot))
        })
        .collect::<Vec<_>>();

    let intermediate_count = intermediate_rings.len();
    let annulus = CoupledAnnulus {
        component_id: component.id,
        inner_guard: ring(
            0,
            inner_guard_cycle,
            RingVertexRole::FixedInnerGuard,
            source,
            true,
        )?,
        coarse_interface: ring(
            1,
            coarse_cycle,
            RingVertexRole::CoarseInterface,
            source,
            false,
        )?,
        intermediate_rings,
        fine_interface: ring(
            2 + intermediate_count,
            fine_cycle,
            RingVertexRole::FineInterface,
            source,
            false,
        )?,
        outer_guard: ring(
            3 + intermediate_count,
            outer_guard_cycle,
            RingVertexRole::FixedOuterGuard,
            source,
            true,
        )?,
        boundary_contracts,
        annulus_face_slots: annulus_faces.into_iter().collect(),
        fixed_outside_face_slots: fixed_outside_faces.into_iter().collect(),
        anchor_star_guard_face_slots,
    };
    Ok(annulus)
}

pub(crate) fn expand_coupled_annulus_to_face_complex(
    source: &MotherGrid,
    mut coupled: CoupledAnnulus,
    annulus_faces: &BTreeSet<usize>,
) -> Result<CoupledAnnulus, AnnulusExtractionError> {
    if annulus_faces.is_empty() {
        return Err(AnnulusExtractionError::InvalidComponent(
            "face-band complex must be non-empty".into(),
        ));
    }
    let coarse_vertices = coupled
        .coarse_interface
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect::<BTreeSet<_>>();
    let fine_cycle = exterior_cycle(
        "fine_interface",
        face_boundary_edges(source, annulus_faces),
        &coarse_vertices,
    )?;
    let outside_ring = annulus_faces
        .iter()
        .flat_map(|&face| source.mesh.neighbours()[face])
        .filter(|&face| source.mesh.is_triangle_live(face) && !annulus_faces.contains(&face))
        .collect::<BTreeSet<_>>();
    let guarded_faces = annulus_faces
        .union(&outside_ring)
        .copied()
        .collect::<BTreeSet<_>>();
    let outer_cycle = exterior_cycle(
        "outer_guard",
        face_boundary_edges(source, &guarded_faces),
        &coarse_vertices,
    )?;
    reject_boundary_cycle_intersection(
        source,
        &coupled
            .coarse_interface
            .vertices
            .iter()
            .map(|vertex| vertex.source_slot)
            .collect::<Vec<_>>(),
        &fine_cycle,
    )?;

    let fixed_outside_faces = source
        .mesh
        .active_triangle_slots()
        .filter(|face| !annulus_faces.contains(face))
        .collect::<BTreeSet<_>>();
    let (annulus_incidence, outside_incidence) =
        vertex_incidence(source, annulus_faces, &fixed_outside_faces);
    coupled.fine_interface = ring(
        coupled.intermediate_rings.len() + 2,
        fine_cycle,
        RingVertexRole::FineInterface,
        source,
        false,
    )?;
    coupled.outer_guard = ring(
        coupled.intermediate_rings.len() + 3,
        outer_cycle,
        RingVertexRole::FixedOuterGuard,
        source,
        true,
    )?;
    coupled.boundary_contracts =
        boundary_contracts(source, &annulus_incidence, &outside_incidence)?;
    coupled.annulus_face_slots = annulus_faces.iter().copied().collect();
    coupled.fixed_outside_face_slots = fixed_outside_faces.into_iter().collect();
    Ok(coupled)
}

fn exterior_cycle(
    name: &'static str,
    edges: BTreeSet<Edge>,
    coarse_vertices: &BTreeSet<usize>,
) -> Result<Vec<usize>, AnnulusExtractionError> {
    let mut cycles = cycles_from_edges(name, edges)?
        .into_iter()
        .filter(|cycle| cycle.iter().all(|vertex| !coarse_vertices.contains(vertex)))
        .collect::<Vec<_>>();
    if cycles.len() != 1 {
        return Err(AnnulusExtractionError::UnsupportedMultiCycleAnnulus {
            boundary: name,
            cycles: cycles.len(),
        });
    }
    let mut cycle = cycles.pop().expect("one exterior cycle");
    cycle.reverse();
    rotate_min(&mut cycle);
    Ok(cycle)
}

fn face_boundary_edges(source: &MotherGrid, faces: &BTreeSet<usize>) -> BTreeSet<Edge> {
    faces
        .iter()
        .flat_map(|&face| {
            let triangle = source.mesh.triangles()[face];
            (0..3).filter_map(move |side| {
                (!faces.contains(&source.mesh.neighbours()[face][side]))
                    .then_some(edge(triangle[(side + 1) % 3], triangle[(side + 2) % 3]))
            })
        })
        .collect()
}

fn validate_component(
    source: &MotherGrid,
    component: &HierarchyComponent,
    core: &BTreeSet<TriangleAddress>,
    transition: &BTreeSet<TriangleAddress>,
) -> Result<(), AnnulusExtractionError> {
    if source.subdivision < 2 || !source.subdivision.is_multiple_of(2) {
        return Err(AnnulusExtractionError::InvalidComponent(
            "annulus extraction requires an even source subdivision >= 2".into(),
        ));
    }
    if core.is_empty() || transition.is_empty() {
        return Err(AnnulusExtractionError::InvalidComponent(
            "annulus extraction requires non-empty core and transition parents".into(),
        ));
    }
    if core.len() != component.core_parents.len()
        || transition.len() != component.transition_parents.len()
        || !core.is_disjoint(transition)
    {
        return Err(AnnulusExtractionError::InvalidComponent(
            "component core/transition parents must be unique and disjoint".into(),
        ));
    }
    let parents = set(component.parents.iter().copied());
    if parents != core.union(transition).copied().collect() {
        return Err(AnnulusExtractionError::InvalidComponent(
            "component parents must equal core union transition parents".into(),
        ));
    }
    let expected_n = source.subdivision / 2;
    for parent in parents {
        if parent.n != expected_n {
            return Err(AnnulusExtractionError::InvalidComponent(format!(
                "component parent {:?} is not at subdivision {expected_n}",
                parent
            )));
        }
    }
    Ok(())
}

fn boundary_contracts(
    source: &MotherGrid,
    annulus_incidence: &[u8],
    outside_incidence: &[u8],
) -> Result<Vec<BoundaryIncidenceContract>, AnnulusExtractionError> {
    let mut contracts = Vec::new();
    for slot in source.mesh.active_vertex_slots() {
        if annulus_incidence[slot] == 0 || outside_incidence[slot] == 0 {
            continue;
        }
        let address = source.addresses[slot].clone().ok_or_else(|| {
            AnnulusExtractionError::InvalidComponent(format!("source vertex {slot} has no address"))
        })?;
        let external = outside_incidence[slot];
        let (anchor_kind, fixed_position, global_min, global_max, patch_min, patch_max) =
            match address {
                VertexAddress::IcosahedronVertex(base_vertex) => {
                    if external > 5 {
                        return Err(AnnulusExtractionError::InvalidAnchorIncidence {
                            source_slot: slot,
                            external_triangle_valence: external,
                        });
                    }
                    (
                        RingAnchorKind::IcosahedronPentagon { base_vertex },
                        true,
                        5,
                        5,
                        5 - external,
                        5 - external,
                    )
                }
                _ => {
                    if external > 7 {
                        return Err(AnnulusExtractionError::InvalidAnchorIncidence {
                            source_slot: slot,
                            external_triangle_valence: external,
                        });
                    }
                    (
                        RingAnchorKind::Ordinary,
                        false,
                        5,
                        7,
                        5u8.saturating_sub(external),
                        7 - external,
                    )
                }
            };
        contracts.push(BoundaryIncidenceContract {
            source_slot: slot,
            address,
            anchor_kind,
            fixed_position,
            external_triangle_valence: external,
            allowed_global_degree_min: global_min,
            allowed_global_degree_max: global_max,
            required_patch_valence_min: patch_min,
            required_patch_valence_max: patch_max,
        });
    }
    Ok(contracts)
}

fn ring(
    id: usize,
    vertices: Vec<usize>,
    role: RingVertexRole,
    source: &MotherGrid,
    fixed_guard: bool,
) -> Result<RingCycle, AnnulusExtractionError> {
    let vertices = vertices
        .into_iter()
        .map(|slot| {
            let address = source.addresses[slot].clone().ok_or_else(|| {
                AnnulusExtractionError::InvalidComponent(format!(
                    "source vertex {slot} has no address"
                ))
            })?;
            let anchor_kind = anchor_kind(&address);
            let is_anchor = matches!(anchor_kind, RingAnchorKind::IcosahedronPentagon { .. });
            Ok(RingVertex {
                source_slot: slot,
                address,
                role: if is_anchor {
                    RingVertexRole::OriginalIcosahedronVertex
                } else {
                    role
                },
                anchor_kind,
                fixed_position: fixed_guard || is_anchor,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RingCycle {
        id,
        vertices,
        orientation: CycleOrientation::SourceOrder,
        target_scale: 1.0,
    })
}

fn anchor_kind(address: &VertexAddress) -> RingAnchorKind {
    match *address {
        VertexAddress::IcosahedronVertex(base_vertex) => {
            RingAnchorKind::IcosahedronPentagon { base_vertex }
        }
        _ => RingAnchorKind::Ordinary,
    }
}

fn reject_strict_interior_pentagons(
    source: &MotherGrid,
    annulus_incidence: &[u8],
    outside_incidence: &[u8],
) -> Result<(), AnnulusExtractionError> {
    for (slot, annulus_count) in annulus_incidence.iter().copied().enumerate() {
        if annulus_count == 0 || outside_incidence[slot] != 0 {
            continue;
        }
        if let Some(VertexAddress::IcosahedronVertex(_)) =
            source.addresses.get(slot).and_then(Clone::clone)
        {
            return Err(
                AnnulusExtractionError::UnsupportedInteriorIcosahedronVertex {
                    source_slot: slot,
                    address: source.addresses[slot].clone().expect("matched address"),
                },
            );
        }
    }
    Ok(())
}

fn reject_boundary_cycle_intersection(
    source: &MotherGrid,
    coarse: &[usize],
    fine: &[usize],
) -> Result<(), AnnulusExtractionError> {
    let fine = fine.iter().copied().collect::<BTreeSet<_>>();
    if let Some(source_slot) = coarse.iter().copied().find(|slot| fine.contains(slot)) {
        return Err(AnnulusExtractionError::UnsupportedIntersectingCycles {
            source_slot,
            address: source.addresses[source_slot].clone().ok_or_else(|| {
                AnnulusExtractionError::InvalidComponent(format!(
                    "source vertex {source_slot} has no address"
                ))
            })?,
        });
    }
    Ok(())
}

fn reject_pentagon_holes(
    source: &MotherGrid,
    parents: &BTreeSet<TriangleAddress>,
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
    parent_by_face: &BTreeMap<usize, TriangleAddress>,
) -> Result<(), AnnulusExtractionError> {
    let mut remaining = graph
        .keys()
        .copied()
        .filter(|parent| !parents.contains(parent))
        .collect::<BTreeSet<_>>();
    let mut components = Vec::new();
    while let Some(&seed) = remaining.first() {
        let mut component = BTreeSet::from([seed]);
        let mut queue = VecDeque::from([seed]);
        remaining.remove(&seed);
        while let Some(parent) = queue.pop_front() {
            for &neighbour in &graph[&parent] {
                if remaining.remove(&neighbour) {
                    component.insert(neighbour);
                    queue.push_back(neighbour);
                }
            }
        }
        components.push(component);
    }
    components.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| left.first().cmp(&right.first()))
    });
    if components.len() < 2 || components[0].len() == components[1].len() {
        return Ok(());
    }

    let mut anchors = BTreeMap::<usize, (VertexAddress, BTreeSet<TriangleAddress>)>::new();
    for (&face, &parent) in parent_by_face {
        for source_slot in source.mesh.triangles()[face] {
            if let Some(address @ VertexAddress::IcosahedronVertex(_)) =
                source.addresses[source_slot].as_ref()
            {
                anchors
                    .entry(source_slot)
                    .or_insert_with(|| (address.clone(), BTreeSet::new()))
                    .1
                    .insert(parent);
            }
        }
    }
    for (source_slot, (address, incident)) in anchors {
        if components
            .iter()
            .skip(1)
            .any(|hole| incident.iter().all(|parent| hole.contains(parent)))
        {
            return Err(AnnulusExtractionError::UnsupportedPentagonHole {
                source_slot,
                address,
            });
        }
    }
    Ok(())
}

fn vertex_incidence(
    source: &MotherGrid,
    annulus_faces: &BTreeSet<usize>,
    fixed_outside_faces: &BTreeSet<usize>,
) -> (Vec<u8>, Vec<u8>) {
    let mut annulus = vec![0u8; source.mesh.vertices().len()];
    let mut outside = vec![0u8; source.mesh.vertices().len()];
    for &face in annulus_faces {
        for slot in source.mesh.triangles()[face] {
            annulus[slot] = annulus[slot].saturating_add(1);
        }
    }
    for &face in fixed_outside_faces {
        for slot in source.mesh.triangles()[face] {
            outside[slot] = outside[slot].saturating_add(1);
        }
    }
    (annulus, outside)
}

pub(crate) fn parent_by_source_face(
    source: &MotherGrid,
) -> Result<BTreeMap<usize, TriangleAddress>, AnnulusExtractionError> {
    let mut out = BTreeMap::new();
    for face in source.mesh.active_triangle_slots() {
        let address = source.triangle_addresses[face].ok_or_else(|| {
            AnnulusExtractionError::InvalidComponent(format!("source face {face} has no address"))
        })?;
        let parent = address.parent_2_to_1().ok_or_else(|| {
            AnnulusExtractionError::InvalidComponent(format!(
                "source face {face} has no 2-to-1 parent"
            ))
        })?;
        out.insert(face, parent);
    }
    Ok(out)
}

pub(crate) fn parent_graph(
    source: &MotherGrid,
    parent_by_face: &BTreeMap<usize, TriangleAddress>,
) -> Result<BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>, AnnulusExtractionError> {
    let mut graph = BTreeMap::<TriangleAddress, BTreeSet<TriangleAddress>>::new();
    for parent in parent_by_face.values().copied() {
        graph.entry(parent).or_default();
    }
    for face in source.mesh.active_triangle_slots() {
        let left = parent_by_face[&face];
        for neighbour in source.mesh.neighbours()[face] {
            if neighbour == 0 || !source.mesh.is_triangle_live(neighbour) {
                continue;
            }
            let right = parent_by_face[&neighbour];
            if left != right {
                graph.entry(left).or_default().insert(right);
                graph.entry(right).or_default().insert(left);
            }
        }
    }
    Ok(graph)
}

pub(crate) fn parent_layers_from_outside(
    parents: &BTreeSet<TriangleAddress>,
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
) -> Result<BTreeMap<TriangleAddress, usize>, AnnulusExtractionError> {
    let mut out = BTreeMap::new();
    let mut queue = VecDeque::new();
    for &parent in parents {
        if graph[&parent]
            .iter()
            .any(|neighbour| !parents.contains(neighbour))
        {
            out.insert(parent, 0usize);
            queue.push_back(parent);
        }
    }
    while let Some(parent) = queue.pop_front() {
        let next_distance = out[&parent] + 1;
        for &neighbour in &graph[&parent] {
            if parents.contains(&neighbour) && !out.contains_key(&neighbour) {
                out.insert(neighbour, next_distance);
                queue.push_back(neighbour);
            }
        }
    }
    if out.len() != parents.len() {
        return Err(AnnulusExtractionError::InvalidComponent(
            "component parents are disconnected from the outside boundary".into(),
        ));
    }
    Ok(out)
}

fn layer(layers: &BTreeMap<TriangleAddress, usize>, distance: usize) -> BTreeSet<TriangleAddress> {
    layers
        .iter()
        .filter_map(|(&parent, &actual)| (actual == distance).then_some(parent))
        .collect()
}

fn outside_neighbours(
    parents: &BTreeSet<TriangleAddress>,
    graph: &BTreeMap<TriangleAddress, BTreeSet<TriangleAddress>>,
) -> BTreeSet<TriangleAddress> {
    parents
        .iter()
        .flat_map(|parent| graph[parent].iter().copied())
        .filter(|parent| !parents.contains(parent))
        .collect()
}

type Edge = (usize, usize);

fn set<T: Ord>(items: impl Iterator<Item = T>) -> BTreeSet<T> {
    items.collect()
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn fine_boundary_edges(
    source: &MotherGrid,
    parent_by_face: &BTreeMap<usize, TriangleAddress>,
    inside_parents: &BTreeSet<TriangleAddress>,
) -> Result<BTreeSet<Edge>, AnnulusExtractionError> {
    let mut edges = BTreeSet::new();
    for face in source.mesh.active_triangle_slots() {
        let left = parent_by_face[&face];
        for neighbour in source.mesh.neighbours()[face] {
            if neighbour == 0 || !source.mesh.is_triangle_live(neighbour) || face > neighbour {
                continue;
            }
            let right = parent_by_face[&neighbour];
            if inside_parents.contains(&left) == inside_parents.contains(&right) {
                continue;
            }
            let shared = source.mesh.triangles()[face]
                .iter()
                .copied()
                .filter(|slot| source.mesh.triangles()[neighbour].contains(slot))
                .collect::<Vec<_>>();
            if shared.len() != 2 {
                return Err(AnnulusExtractionError::InvalidComponent(
                    "adjacent source faces did not share exactly one edge".into(),
                ));
            }
            edges.insert(edge(shared[0], shared[1]));
        }
    }
    Ok(edges)
}

fn coarse_edges_between(
    source: &MotherGrid,
    left_parents: &BTreeSet<TriangleAddress>,
    right_parents: &BTreeSet<TriangleAddress>,
) -> Result<BTreeSet<Edge>, AnnulusExtractionError> {
    let mut corners_by_parent = BTreeMap::new();
    for parent in left_parents.iter().chain(right_parents) {
        corners_by_parent.insert(*parent, parent_corners(source, *parent)?);
    }
    let mut edges = BTreeSet::new();
    for left_parent in left_parents {
        let a = corners_by_parent[left_parent];
        for right_parent in right_parents {
            let b = corners_by_parent[right_parent];
            let shared = a
                .iter()
                .copied()
                .filter(|slot| b.contains(slot))
                .collect::<Vec<_>>();
            if shared.len() == 2 {
                edges.insert(edge(shared[0], shared[1]));
            }
        }
    }
    Ok(edges)
}

fn parent_corners(
    source: &MotherGrid,
    parent: TriangleAddress,
) -> Result<[usize; 3], AnnulusExtractionError> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for face in source_face_slots(source, parent)? {
        for slot in source.mesh.triangles()[face] {
            *counts.entry(slot).or_default() += 1;
        }
    }
    let corners = counts
        .into_iter()
        .filter_map(|(slot, count)| (count == 1).then_some(slot))
        .collect::<Vec<_>>();
    <[usize; 3]>::try_from(corners).map_err(|corners| {
        AnnulusExtractionError::InvalidComponent(format!(
            "parent {:?} has {} source corners, expected 3",
            parent,
            corners.len()
        ))
    })
}

fn source_face_slots(
    source: &MotherGrid,
    parent: TriangleAddress,
) -> Result<Vec<usize>, AnnulusExtractionError> {
    parent
        .children_2_to_1()
        .ok_or_else(|| {
            AnnulusExtractionError::InvalidComponent(format!("invalid hierarchy parent {parent:?}"))
        })?
        .into_iter()
        .map(|child| {
            source_face_slot(source, child).map_err(AnnulusExtractionError::InvalidComponent)
        })
        .collect()
}

fn one_cycle(
    name: &'static str,
    edges: BTreeSet<Edge>,
    reverse: bool,
) -> Result<Vec<usize>, AnnulusExtractionError> {
    let cycles = cycles_from_edges(name, edges)?;
    match cycles.len() {
        0 => Err(AnnulusExtractionError::MissingRing(name)),
        1 => {
            let mut cycle = cycles.into_iter().next().expect("one cycle");
            if reverse {
                cycle.reverse();
                rotate_min(&mut cycle);
            }
            Ok(cycle)
        }
        n => Err(AnnulusExtractionError::UnsupportedMultiCycleAnnulus {
            boundary: name,
            cycles: n,
        }),
    }
}

fn cycles_from_edges(
    name: &'static str,
    edges: BTreeSet<Edge>,
) -> Result<Vec<Vec<usize>>, AnnulusExtractionError> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for (a, b) in edges {
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    if adjacency.values().any(|neighbours| neighbours.len() != 2) {
        return Err(AnnulusExtractionError::InvalidComponent(format!(
            "{name} boundary is not a closed 2-regular cycle"
        )));
    }
    let mut seen_vertices = BTreeSet::new();
    let mut cycles = Vec::new();
    for &start in adjacency.keys() {
        if seen_vertices.contains(&start) {
            continue;
        }
        let mut cycle = Vec::new();
        let mut prev = usize::MAX;
        let mut current = start;
        loop {
            if !seen_vertices.insert(current) && current != start {
                return Err(AnnulusExtractionError::InvalidComponent(format!(
                    "{name} boundary repeats vertex {current}"
                )));
            }
            cycle.push(current);
            let next = adjacency[&current]
                .iter()
                .copied()
                .find(|&candidate| candidate != prev)
                .expect("2-regular vertex has a next edge");
            prev = current;
            current = next;
            if current == start {
                break;
            }
        }
        rotate_min(&mut cycle);
        cycles.push(cycle);
    }
    cycles.sort_by_key(|cycle| cycle[0]);
    Ok(cycles)
}

fn rotate_min(cycle: &mut [usize]) {
    let Some((pos, _)) = cycle.iter().enumerate().min_by_key(|(_, slot)| *slot) else {
        return;
    };
    cycle.rotate_left(pos);
}
