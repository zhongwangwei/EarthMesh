//! Face-band topology domains and deferred geometry guards.

use super::{
    annulus::{
        boundary_contracts_for_face_complex, coarse_edges_between, parent_by_source_face, ring,
    },
    BoundaryIncidenceContract, CoupledAnnulus, FaceBandPlan, HierarchyComponent, RingVertexRole,
};
use crate::{MotherGrid, VertexAddress};
use std::collections::{BTreeMap, BTreeSet};

type Edge = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCycle {
    pub ordered_vertices: Vec<usize>,
    pub edges: BTreeSet<Edge>,
    pub cycle_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionTopologyDomainKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionTopologyDomain {
    pub component_id: u64,
    pub annulus_face_slots: BTreeSet<usize>,
    pub coarse_interface: BoundaryCycle,
    pub internal_interfaces: Vec<BoundaryCycle>,
    pub fine_interface: BoundaryCycle,
    pub fixed_outside_face_slots: BTreeSet<usize>,
    pub boundary_contracts: Vec<BoundaryIncidenceContract>,
    pub source_face_labels: BTreeMap<usize, u8>,
    pub topology_key: TransitionTopologyDomainKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyDomainError {
    InvalidPlan(String),
    MissingCoarseInterface,
    MultipleCoarseInterfaces { count: usize },
    MissingFineInterface,
    MultipleFineInterfaces { count: usize },
    InterfaceMismatch,
    BoundaryIntersection,
    InvalidOutsideIncidence(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryGraph {
    pub edges: BTreeSet<Edge>,
    pub connected_components: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryGuardRegionKey(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryGuardRegion {
    pub inner_fixed_faces: BTreeSet<usize>,
    pub outer_fixed_faces: BTreeSet<usize>,
    pub fixed_source_vertices: BTreeSet<usize>,
    pub movable_source_vertices: BTreeSet<usize>,
    pub guard_face_slots: BTreeSet<usize>,
    pub inner_boundary_components: Vec<BoundaryGraph>,
    pub outer_boundary_components: Vec<BoundaryGraph>,
    pub guard_key: GeometryGuardRegionKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryGuardError {
    MovableFixedOverlap,
    OriginalAnchorMovable,
    MissingGuardFaces,
    InvalidPhysicalFixedPoint,
}

pub fn build_transition_topology_domain_from_face_bands(
    source: &MotherGrid,
    component: &HierarchyComponent,
    plan: &FaceBandPlan,
) -> Result<TransitionTopologyDomain, TopologyDomainError> {
    if plan.band_count < 2 || plan.interface_edges.len() + 1 != plan.band_count {
        return Err(TopologyDomainError::InvalidPlan(
            "face-band plan has inconsistent band/interface counts".into(),
        ));
    }
    let annulus_face_slots = plan.labels.keys().copied().collect::<BTreeSet<_>>();
    if annulus_face_slots.is_empty()
        || plan
            .labels
            .values()
            .any(|&label| usize::from(label) >= plan.band_count)
    {
        return Err(TopologyDomainError::InvalidPlan(
            "face-band labels are empty or outside the declared bands".into(),
        ));
    }
    let mut face_counts = vec![0usize; plan.band_count];
    for &label in plan.labels.values() {
        face_counts[usize::from(label)] += 1;
    }
    let expected_internal = expected_interface_edges(source, plan);
    let supplied_internal = plan
        .interface_edges
        .iter()
        .map(|edges| {
            edges
                .iter()
                .map(|&(a, b)| edge(a, b))
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    if face_counts != plan.band_face_counts || expected_internal != supplied_internal {
        return Err(TopologyDomainError::InterfaceMismatch);
    }

    let parent_by_face = parent_by_source_face(source)
        .map_err(|error| TopologyDomainError::InvalidPlan(format!("{error:?}")))?;
    let core = component
        .core_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let transition = component
        .transition_parents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let coarse_edges = coarse_edges_between(source, &core, &transition)
        .map_err(|error| TopologyDomainError::InvalidPlan(format!("{error:?}")))?;
    let mut fine_edges = BTreeSet::new();
    for (&face, &label) in &plan.labels {
        let triangle = source.mesh.triangles()[face];
        for side in 0..3 {
            let neighbour = source.mesh.neighbours()[face][side];
            if plan.labels.contains_key(&neighbour) {
                continue;
            }
            let boundary = edge(triangle[(side + 1) % 3], triangle[(side + 2) % 3]);
            if usize::from(label) + 1 == plan.band_count
                && !parent_by_face
                    .get(&neighbour)
                    .is_some_and(|parent| core.contains(parent))
            {
                fine_edges.insert(boundary);
            }
        }
    }
    let coarse_interface = single_boundary_cycle(
        coarse_edges,
        TopologyDomainError::MissingCoarseInterface,
        |count| TopologyDomainError::MultipleCoarseInterfaces { count },
    )?;
    let fine_interface = single_boundary_cycle(
        fine_edges,
        TopologyDomainError::MissingFineInterface,
        |count| TopologyDomainError::MultipleFineInterfaces { count },
    )?;
    if coarse_interface
        .ordered_vertices
        .iter()
        .any(|vertex| fine_interface.ordered_vertices.contains(vertex))
    {
        return Err(TopologyDomainError::BoundaryIntersection);
    }
    let internal_interfaces = supplied_internal
        .into_iter()
        .map(|edges| {
            single_boundary_cycle(edges, TopologyDomainError::InterfaceMismatch, |_| {
                TopologyDomainError::InterfaceMismatch
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (fixed_outside, boundary_contracts) =
        boundary_contracts_for_face_complex(source, &annulus_face_slots)
            .map_err(|error| TopologyDomainError::InvalidOutsideIncidence(format!("{error:?}")))?;
    let fixed_outside_face_slots = fixed_outside.into_iter().collect::<BTreeSet<_>>();
    let topology_key = TransitionTopologyDomainKey(format!(
        "{:016x}",
        fnv1a(
            format!(
                "{}|{:?}|{:?}|{:?}|{:?}",
                component.id,
                annulus_face_slots,
                coarse_interface.edges,
                internal_interfaces
                    .iter()
                    .map(|cycle| &cycle.edges)
                    .collect::<Vec<_>>(),
                fine_interface.edges,
            )
            .bytes(),
        )
    ));
    Ok(TransitionTopologyDomain {
        component_id: component.id,
        annulus_face_slots,
        coarse_interface,
        internal_interfaces,
        fine_interface,
        fixed_outside_face_slots,
        boundary_contracts,
        source_face_labels: plan.labels.clone(),
        topology_key,
    })
}

pub fn build_geometry_guard_region(
    source: &MotherGrid,
    domain: &TransitionTopologyDomain,
    physical_fixed_sources: &BTreeSet<usize>,
) -> Result<GeometryGuardRegion, GeometryGuardError> {
    if physical_fixed_sources
        .iter()
        .any(|slot| !source.mesh.is_vertex_live(*slot))
    {
        return Err(GeometryGuardError::InvalidPhysicalFixedPoint);
    }
    let anchors = source
        .mesh
        .active_vertex_slots()
        .filter(|slot| {
            matches!(
                source.addresses.get(*slot).and_then(Option::as_ref),
                Some(VertexAddress::IcosahedronVertex(_))
            )
        })
        .collect::<BTreeSet<_>>();
    let mut movable_source_vertices = domain
        .annulus_face_slots
        .iter()
        .flat_map(|face| source.mesh.triangles()[*face])
        .collect::<BTreeSet<_>>();
    movable_source_vertices
        .retain(|slot| !anchors.contains(slot) && !physical_fixed_sources.contains(slot));
    if movable_source_vertices
        .iter()
        .any(|slot| anchors.contains(slot))
    {
        return Err(GeometryGuardError::OriginalAnchorMovable);
    }
    let fixed_source_vertices = source
        .mesh
        .active_vertex_slots()
        .filter(|slot| !movable_source_vertices.contains(slot))
        .collect::<BTreeSet<_>>();
    if !movable_source_vertices.is_disjoint(&fixed_source_vertices) {
        return Err(GeometryGuardError::MovableFixedOverlap);
    }
    let guard_face_slots = source
        .mesh
        .active_triangle_slots()
        .filter(|face| {
            source.mesh.triangles()[*face]
                .iter()
                .any(|slot| movable_source_vertices.contains(slot))
        })
        .collect::<BTreeSet<_>>();
    if movable_source_vertices.iter().any(|vertex| {
        source.mesh.active_triangle_slots().any(|face| {
            source.mesh.triangles()[face].contains(vertex) && !guard_face_slots.contains(&face)
        })
    }) {
        return Err(GeometryGuardError::MissingGuardFaces);
    }
    let inner_fixed_faces = fixed_faces_touching_cycle(source, domain, &domain.coarse_interface);
    let outer_fixed_faces = fixed_faces_touching_cycle(source, domain, &domain.fine_interface);
    let guard_key = GeometryGuardRegionKey(format!(
        "{:016x}",
        fnv1a(
            format!(
                "{:?}|{:?}|{:?}",
                movable_source_vertices, guard_face_slots, physical_fixed_sources
            )
            .bytes(),
        )
    ));
    Ok(GeometryGuardRegion {
        inner_fixed_faces,
        outer_fixed_faces,
        fixed_source_vertices,
        movable_source_vertices,
        guard_face_slots,
        inner_boundary_components: vec![BoundaryGraph {
            edges: domain.coarse_interface.edges.clone(),
            connected_components: 1,
        }],
        outer_boundary_components: vec![BoundaryGraph {
            edges: domain.fine_interface.edges.clone(),
            connected_components: 1,
        }],
        guard_key,
    })
}

pub(super) fn coupled_annulus_from_topology_domain(
    source: &MotherGrid,
    domain: &TransitionTopologyDomain,
) -> Result<CoupledAnnulus, TopologyDomainError> {
    let make_ring = |id, vertices: Vec<usize>, role, fixed| {
        ring(id, vertices, role, source, fixed)
            .map_err(|error| TopologyDomainError::InvalidOutsideIncidence(format!("{error:?}")))
    };
    Ok(CoupledAnnulus {
        component_id: domain.component_id,
        inner_guard: make_ring(
            0,
            domain.coarse_interface.ordered_vertices.clone(),
            RingVertexRole::FixedInnerGuard,
            true,
        )?,
        coarse_interface: make_ring(
            1,
            domain.coarse_interface.ordered_vertices.clone(),
            RingVertexRole::CoarseInterface,
            false,
        )?,
        intermediate_rings: Vec::new(),
        fine_interface: make_ring(
            2,
            domain.fine_interface.ordered_vertices.clone(),
            RingVertexRole::FineInterface,
            false,
        )?,
        outer_guard: make_ring(
            3,
            domain.fine_interface.ordered_vertices.clone(),
            RingVertexRole::FixedOuterGuard,
            true,
        )?,
        boundary_contracts: domain.boundary_contracts.clone(),
        annulus_face_slots: domain.annulus_face_slots.iter().copied().collect(),
        fixed_outside_face_slots: domain.fixed_outside_face_slots.iter().copied().collect(),
        anchor_star_guard_face_slots: Vec::new(),
    })
}

fn expected_interface_edges(source: &MotherGrid, plan: &FaceBandPlan) -> Vec<BTreeSet<Edge>> {
    let mut interfaces = vec![BTreeSet::new(); plan.band_count.saturating_sub(1)];
    for (&face, &label) in &plan.labels {
        let triangle = source.mesh.triangles()[face];
        for side in 0..3 {
            let neighbour = source.mesh.neighbours()[face][side];
            let Some(&other) = plan.labels.get(&neighbour) else {
                continue;
            };
            if label.abs_diff(other) == 1 {
                interfaces[usize::from(label.min(other))]
                    .insert(edge(triangle[(side + 1) % 3], triangle[(side + 2) % 3]));
            }
        }
    }
    interfaces
}

fn single_boundary_cycle(
    edges: BTreeSet<Edge>,
    missing: TopologyDomainError,
    multiple: impl Fn(usize) -> TopologyDomainError,
) -> Result<BoundaryCycle, TopologyDomainError> {
    if edges.is_empty() {
        return Err(missing);
    }
    let cycles = cycles_from_edges(&edges).ok_or_else(|| multiple(0))?;
    if cycles.len() != 1 {
        return Err(multiple(cycles.len()));
    }
    let ordered_vertices = cycles.into_iter().next().expect("one cycle");
    let cycle_key = format!("{:016x}", fnv1a(format!("{ordered_vertices:?}").bytes()));
    Ok(BoundaryCycle {
        ordered_vertices,
        edges,
        cycle_key,
    })
}

pub(super) fn cycles_from_edges(edges: &BTreeSet<Edge>) -> Option<Vec<Vec<usize>>> {
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in edges {
        if a == b {
            return None;
        }
        adjacency.entry(a).or_default().insert(b);
        adjacency.entry(b).or_default().insert(a);
    }
    if adjacency.values().any(|neighbours| neighbours.len() != 2) {
        return None;
    }
    let mut unseen = adjacency.keys().copied().collect::<BTreeSet<_>>();
    let mut cycles = Vec::new();
    while let Some(&start) = unseen.first() {
        let mut cycle = vec![start];
        let mut previous = usize::MAX;
        let mut current = start;
        loop {
            let next = adjacency[&current]
                .iter()
                .copied()
                .find(|next| *next != previous)?;
            if next == start {
                break;
            }
            if !unseen.contains(&next) {
                return None;
            }
            cycle.push(next);
            previous = current;
            current = next;
        }
        for vertex in &cycle {
            unseen.remove(vertex);
        }
        canonicalize_cycle(&mut cycle);
        cycles.push(cycle);
    }
    cycles.sort();
    Some(cycles)
}

fn canonicalize_cycle(cycle: &mut Vec<usize>) {
    let minimum = *cycle.iter().min().expect("cycle is non-empty");
    let forward = cycle.iter().position(|slot| *slot == minimum).unwrap();
    cycle.rotate_left(forward);
    let mut reverse = cycle.clone();
    reverse.reverse();
    let reverse_minimum = reverse.iter().position(|slot| *slot == minimum).unwrap();
    reverse.rotate_left(reverse_minimum);
    if reverse < *cycle {
        *cycle = reverse;
    }
}

fn fixed_faces_touching_cycle(
    source: &MotherGrid,
    domain: &TransitionTopologyDomain,
    cycle: &BoundaryCycle,
) -> BTreeSet<usize> {
    domain
        .fixed_outside_face_slots
        .iter()
        .copied()
        .filter(|face| {
            let triangle = source.mesh.triangles()[*face];
            [
                edge(triangle[0], triangle[1]),
                edge(triangle[1], triangle[2]),
                edge(triangle[2], triangle[0]),
            ]
            .iter()
            .any(|edge| cycle.edges.contains(edge))
        })
        .collect()
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
