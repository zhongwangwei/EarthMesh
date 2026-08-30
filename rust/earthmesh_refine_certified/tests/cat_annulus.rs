use earthmesh_refine_certified::{
    coarsen::{
        extract_coupled_annulus, n6_legacy_mixed_fixture,
        plan_hierarchy_components_from_parent_requirements, AnnulusExtractionError,
        BoundaryIncidenceContract, CoupledAnnulus, ExplicitParentRequirement, HierarchyComponent,
        RingAnchorKind, RingVertexRole,
    },
    MotherGrid, TriangleAddress, TriangleOrientation, VertexAddress,
};
use std::collections::BTreeSet;

fn frozen_fixture_a() -> (MotherGrid, HierarchyComponent) {
    use TriangleOrientation::{Down as D, Up as U};
    let source = MotherGrid::generate(12).unwrap();
    let eligible = [
        (0, 0, 1, U),
        (0, 0, 1, D),
        (0, 0, 2, U),
        (0, 0, 2, D),
        (0, 0, 3, U),
        (0, 0, 3, D),
        (0, 0, 4, U),
        (0, 1, 0, U),
        (0, 1, 0, D),
        (0, 1, 1, U),
        (0, 1, 1, D),
        (0, 1, 2, U),
        (0, 1, 2, D),
        (0, 1, 3, U),
        (0, 1, 3, D),
        (0, 1, 4, U),
        (0, 2, 0, U),
        (0, 2, 0, D),
        (0, 2, 1, U),
        (0, 2, 1, D),
        (0, 2, 2, U),
        (0, 2, 2, D),
        (0, 2, 3, U),
        (0, 3, 0, U),
        (0, 3, 1, U),
        (0, 3, 2, U),
        (1, 1, 0, D),
        (1, 2, 0, U),
        (1, 2, 0, D),
        (1, 3, 0, U),
        (1, 3, 0, D),
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
    let requirements = MotherGrid::generate(6)
        .unwrap()
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .map(|parent| ExplicitParentRequirement {
            parent,
            maximum_required_level: usize::from(!eligible.contains(&parent)),
            available: true,
        })
        .collect::<Vec<_>>();
    let mut components =
        plan_hierarchy_components_from_parent_requirements(&source, &requirements, 0, 2)
            .unwrap()
            .components;
    assert_eq!(components.len(), 1);
    (source, components.pop().unwrap())
}

#[test]
fn fixture_a_extracts_plain_nested_annulus_without_pentagon_anchors() {
    let (source, component) = frozen_fixture_a();
    let annulus = extract_coupled_annulus(&source, &component).unwrap();

    assert_eq!(component.parents.len(), 31);
    assert_eq!(component.core_parents.len(), 4);
    assert_eq!(component.transition_parents.len(), 27);
    assert_partition_closes(&source, &annulus);
    assert_cycles_are_source_edges(&source, &annulus);
    assert!(!annulus.boundary_contracts.iter().any(is_anchor));
    assert!(!annulus.coarse_interface.vertices.is_empty());
    assert_eq!(annulus.intermediate_rings.len(), 2);
    assert!(annulus.intermediate_rings[0].vertices.len() > annulus.coarse_interface.vertices.len());
    assert!(
        annulus.intermediate_rings[1].vertices.len() > annulus.intermediate_rings[0].vertices.len()
    );
    assert!(annulus.fine_interface.vertices.len() > annulus.intermediate_rings[1].vertices.len());
    assert!(annulus
        .coarse_interface
        .vertices
        .iter()
        .all(|vertex| !vertex.fixed_position));
    assert!(annulus
        .fine_interface
        .vertices
        .iter()
        .all(|vertex| !vertex.fixed_position));
    assert!(annulus
        .inner_guard
        .vertices
        .iter()
        .chain(&annulus.outer_guard.vertices)
        .all(|vertex| vertex.fixed_position));
    assert!(annulus
        .coarse_interface
        .vertices
        .iter()
        .chain(
            annulus
                .intermediate_rings
                .iter()
                .flat_map(|ring| ring.vertices.iter())
        )
        .chain(annulus.fine_interface.vertices.iter())
        .any(
            |vertex| matches!(vertex.address, VertexAddress::IcosahedronEdge { .. })
                && matches!(vertex.anchor_kind, RingAnchorKind::Ordinary)
                && !vertex.fixed_position
        ));
}

#[test]
fn fixture_b_freezes_boundary_pentagon_anchor_contracts() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let annulus = extract_coupled_annulus(&source, &component).unwrap();

    assert_eq!(component.parents.len(), 32);
    assert_eq!(component.core_parents.len(), 10);
    assert_eq!(component.transition_parents.len(), 22);
    assert_partition_closes(&source, &annulus);
    assert_cycles_are_source_edges(&source, &annulus);
    assert_eq!(annulus.annulus_face_slots.len(), 88);
    assert_eq!(annulus.fixed_outside_face_slots.len(), 632);

    let anchors = annulus
        .boundary_contracts
        .iter()
        .filter(|contract| is_anchor(contract))
        .collect::<Vec<_>>();
    assert_eq!(anchors.len(), 4);
    assert_anchor(anchors.as_slice(), 29, 11, 2, 3);
    assert_anchor(anchors.as_slice(), 77, 10, 2, 3);
    assert_anchor(anchors.as_slice(), 2, 0, 4, 1);
    assert_anchor(anchors.as_slice(), 155, 2, 4, 1);

    assert_eq!(slots(&annulus.inner_guard), vec![99, 106, 162, 101]);
    assert_eq!(
        slots(&annulus.coarse_interface),
        vec![24, 29, 151, 162, 169, 77, 75, 99]
    );
    assert_eq!(
        slots(&annulus.intermediate_rings[0]),
        vec![
            15, 93, 73, 74, 75, 81, 86, 82, 77, 171, 172, 337, 169, 167, 164, 159, 153, 152, 151,
            327, 148, 149, 29, 28, 26, 25, 24, 20,
        ]
    );
    assert_eq!(
        slots(&annulus.fine_interface),
        vec![
            2, 72, 73, 79, 84, 85, 86, 177, 172, 336, 339, 342, 164, 160, 155, 154, 153, 325, 329,
            330, 148, 147, 26, 22, 17, 16, 15, 9,
        ]
    );
    assert_eq!(
        slots(&annulus.outer_guard),
        vec![
            2, 51, 52, 83, 84, 88, 91, 89, 86, 182, 183, 178, 172, 173, 174, 335, 339, 341, 277,
            278, 155, 248, 252, 328, 329, 332, 143, 146, 148, 145, 141, 144, 26, 23, 19, 18, 17,
            11, 4, 3,
        ]
    );
    assert!(annulus.coarse_interface.vertices.iter().all(|vertex| {
        !matches!(vertex.address, VertexAddress::IcosahedronEdge { step, .. } if step % 2 == 1)
    }));
    let boundary_anchor_slots = annulus
        .coarse_interface
        .vertices
        .iter()
        .chain(&annulus.intermediate_rings[0].vertices)
        .chain(&annulus.fine_interface.vertices)
        .filter(|vertex| matches!(vertex.role, RingVertexRole::OriginalIcosahedronVertex))
        .map(|vertex| vertex.source_slot)
        .collect::<BTreeSet<_>>();
    assert_eq!(boundary_anchor_slots, BTreeSet::from([2, 29, 77, 155]));
    assert!(annulus
        .coarse_interface
        .vertices
        .iter()
        .chain(&annulus.intermediate_rings[0].vertices)
        .chain(&annulus.fine_interface.vertices)
        .filter(|vertex| matches!(vertex.anchor_kind, RingAnchorKind::Ordinary))
        .all(|vertex| !vertex.fixed_position));
    assert_eq!(annulus.anchor_star_guard_face_slots.len(), 20);
    for &slot in &[2, 29, 77, 155] {
        assert_eq!(
            annulus
                .anchor_star_guard_face_slots
                .iter()
                .filter(|&&face| source.mesh.triangles()[face].contains(&slot))
                .count(),
            5
        );
    }
}

#[test]
fn strict_interior_original_pentagon_is_typed_unsupported() {
    let source = MotherGrid::generate(4).unwrap();
    let vertex_slot = source
        .addresses
        .iter()
        .position(|address| matches!(address, Some(VertexAddress::IcosahedronVertex(0))))
        .unwrap();
    let mut transition_parents = source
        .mesh
        .active_triangle_slots()
        .filter(|&face| source.mesh.triangles()[face].contains(&vertex_slot))
        .map(|face| {
            source.triangle_addresses[face]
                .unwrap()
                .parent_2_to_1()
                .unwrap()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    transition_parents.sort();
    let core = TriangleAddress {
        base_face: 0,
        i: 1,
        j: 0,
        n: 2,
        orientation: TriangleOrientation::Up,
    };
    assert!(!transition_parents.contains(&core));
    let mut parents = transition_parents.clone();
    parents.push(core);
    parents.sort();
    let component = HierarchyComponent {
        id: 7,
        parents,
        boundary_edges: Vec::new(),
        core_parents: vec![core],
        transition_parents,
    };
    assert!(matches!(
        extract_coupled_annulus(&source, &component),
        Err(AnnulusExtractionError::UnsupportedInteriorIcosahedronVertex {
            source_slot,
            address: VertexAddress::IcosahedronVertex(0),
        }) if source_slot == vertex_slot
    ));
}

#[test]
fn pentagon_inside_a_parent_graph_hole_is_typed_unsupported() {
    use TriangleOrientation::{Down as D, Up as U};
    let source = MotherGrid::generate(6).unwrap();
    let excluded = [
        // Five-parent hole around IcosahedronVertex(0).
        (0, 0, 0, U),
        (1, 0, 0, U),
        (2, 0, 0, U),
        (3, 0, 0, U),
        (4, 0, 0, U),
        // Larger outside component around IcosahedronVertex(5).
        (0, 0, 1, D),
        (0, 0, 2, U),
        (1, 1, 0, D),
        (1, 2, 0, U),
        (5, 1, 0, D),
        (5, 2, 0, U),
        (6, 0, 0, U),
        (6, 0, 0, D),
        (15, 0, 1, D),
        (15, 0, 2, U),
    ]
    .into_iter()
    .map(|(base_face, i, j, orientation)| TriangleAddress {
        base_face,
        i,
        j,
        n: 3,
        orientation,
    })
    .collect::<BTreeSet<_>>();
    let requirements = MotherGrid::generate(3)
        .unwrap()
        .triangle_addresses
        .iter()
        .flatten()
        .copied()
        .map(|parent| ExplicitParentRequirement {
            parent,
            maximum_required_level: usize::from(excluded.contains(&parent)),
            available: true,
        })
        .collect::<Vec<_>>();
    let plan =
        plan_hierarchy_components_from_parent_requirements(&source, &requirements, 0, 1).unwrap();
    assert_eq!(plan.components.len(), 1);
    assert_eq!(plan.components[0].parents.len(), 165);
    assert!(matches!(
        extract_coupled_annulus(&source, &plan.components[0]),
        Err(AnnulusExtractionError::UnsupportedPentagonHole {
            source_slot: 2,
            address: VertexAddress::IcosahedronVertex(0),
        })
    ));
}

fn is_anchor(contract: &BoundaryIncidenceContract) -> bool {
    matches!(
        contract.anchor_kind,
        RingAnchorKind::IcosahedronPentagon { .. }
    )
}

fn assert_anchor(
    anchors: &[&BoundaryIncidenceContract],
    slot: usize,
    base_vertex: u8,
    external: u8,
    required_patch: u8,
) {
    let contract = anchors
        .iter()
        .find(|contract| contract.source_slot == slot)
        .unwrap_or_else(|| panic!("missing anchor source slot {slot}"));
    assert_eq!(
        contract.address,
        VertexAddress::IcosahedronVertex(base_vertex)
    );
    assert!(contract.fixed_position);
    assert_eq!(contract.external_triangle_valence, external);
    assert_eq!(contract.allowed_global_degree_min, 5);
    assert_eq!(contract.allowed_global_degree_max, 5);
    assert_eq!(contract.required_patch_valence_min, required_patch);
    assert_eq!(contract.required_patch_valence_max, required_patch);
}

fn slots(cycle: &earthmesh_refine_certified::coarsen::RingCycle) -> Vec<usize> {
    cycle
        .vertices
        .iter()
        .map(|vertex| vertex.source_slot)
        .collect()
}

fn assert_cycles_are_source_edges(source: &MotherGrid, annulus: &CoupledAnnulus) {
    let mut neighbours = vec![BTreeSet::new(); source.mesh.vertices().len()];
    for face in source.mesh.active_triangle_slots() {
        let triangle = source.mesh.triangles()[face];
        for side in 0..3 {
            neighbours[triangle[side]].insert(triangle[(side + 1) % 3]);
            neighbours[triangle[(side + 1) % 3]].insert(triangle[side]);
        }
    }
    for (cycle, max_hops) in std::iter::once((&annulus.inner_guard, 2))
        .chain(std::iter::once((&annulus.coarse_interface, 2)))
        .chain(annulus.intermediate_rings.iter().map(|ring| (ring, 1)))
        .chain(std::iter::once((&annulus.fine_interface, 1)))
        .chain(std::iter::once((&annulus.outer_guard, 1)))
    {
        let slots = slots(cycle);
        assert!(slots.len() >= 3);
        assert_eq!(
            slots.iter().copied().collect::<BTreeSet<_>>().len(),
            slots.len()
        );
        for (&a, &b) in slots
            .iter()
            .zip(slots.iter().cycle().skip(1))
            .take(slots.len())
        {
            assert!(
                neighbours[a].contains(&b)
                    || (max_hops == 2
                        && neighbours[a]
                            .iter()
                            .any(|middle| neighbours[*middle].contains(&b)))
            );
        }
    }
    assert!(slots(&annulus.coarse_interface)
        .into_iter()
        .collect::<BTreeSet<_>>()
        .is_disjoint(
            &slots(&annulus.fine_interface)
                .into_iter()
                .collect::<BTreeSet<_>>()
        ));
}

fn assert_partition_closes(source: &MotherGrid, annulus: &CoupledAnnulus) {
    let annulus_faces = annulus
        .annulus_face_slots
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let fixed_faces = annulus
        .fixed_outside_face_slots
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(annulus_faces.is_disjoint(&fixed_faces));
    let active = source.mesh.active_triangle_slots().collect::<BTreeSet<_>>();
    assert_eq!(
        annulus_faces
            .union(&fixed_faces)
            .copied()
            .collect::<BTreeSet<_>>(),
        active
    );
}
