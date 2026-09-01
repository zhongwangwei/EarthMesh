use earthmesh_refine_certified::{
    coarsen::{
        n12_interior_control_fixture, n12_lifted_n6_fixture, n12_research_fixture_manifests_json,
        n12_research_fixture_report_json, research_fixture_guard_parents,
    },
    TriangleAddress, VertexAddress,
};
use std::collections::BTreeSet;

#[test]
fn n12_counts_are_exact() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    let vertices = fixture.source.mesh.active_vertex_slots().count();
    let faces = fixture.source.mesh.active_triangle_slots().count();
    let edges = fixture
        .source
        .mesh
        .active_triangle_slots()
        .flat_map(|face| {
            let [a, b, c] = fixture.source.mesh.triangles()[face];
            [pair(a, b), pair(b, c), pair(c, a)]
        })
        .collect::<BTreeSet<_>>()
        .len();
    assert_eq!((vertices, edges, faces), (1442, 4320, 2880));
    assert_eq!(fixture.manifest.expected_source_vertices, vertices);
    assert_eq!(fixture.manifest.expected_source_edges, edges);
    assert_eq!(fixture.manifest.expected_source_faces, faces);
}

#[test]
fn lifted_parent_union_matches_n6_region() {
    let fixture = n12_lifted_n6_fixture().unwrap();
    assert_eq!(fixture.component.parents.len(), 128);
    assert_eq!(fixture.component.core_parents.len(), 40);
    assert_eq!(fixture.component.transition_parents.len(), 88);
    let n6 = earthmesh_refine_certified::coarsen::n6_legacy_mixed_fixture()
        .unwrap()
        .1;
    assert_eq!(
        lift(&n6.parents),
        fixture.component.parents.iter().copied().collect()
    );
    assert_eq!(
        lift(&n6.core_parents),
        fixture.component.core_parents.iter().copied().collect()
    );
    assert_eq!(
        lift(&n6.transition_parents),
        fixture
            .component
            .transition_parents
            .iter()
            .copied()
            .collect()
    );
}

#[test]
fn lifted_fixture_is_stable() {
    assert_eq!(
        n12_research_fixture_manifests_json().unwrap(),
        include_str!("fixtures/n12_research_fixtures.json").trim()
    );
}

#[test]
fn representativeness_telemetry_is_research_only_and_finite() {
    let report = n12_research_fixture_report_json().unwrap();
    assert!(report.contains("\"research_only\":true"));
    assert!(report.contains("\"N12-Lifted-N6\""));
    assert!(report.contains("\"N12-Interior-Control\""));
    assert!(!report.contains("NaN"));
    assert!(!report.contains("inf"));
}

#[test]
fn interior_fixture_has_no_icosahedron_vertex_in_guard() {
    let fixture = n12_interior_control_fixture().unwrap();
    assert!(fixture
        .component
        .parents
        .iter()
        .all(|parent| parent.base_face == fixture.component.parents[0].base_face));
    assert!(fixture.manifest.original_anchor_vertices.is_empty());
    assert_eq!(fixture.telemetry.original_pentagons_in_transition, 0);
    let guards = research_fixture_guard_parents(&fixture.source, &fixture.component, 2).unwrap();
    for face in fixture.source.mesh.active_triangle_slots() {
        let parent = fixture.source.triangle_addresses[face]
            .and_then(TriangleAddress::parent_2_to_1)
            .unwrap();
        if guards.contains(&parent) {
            assert!(fixture.source.mesh.triangles()[face].iter().all(|site| {
                !matches!(
                    fixture.source.addresses[*site],
                    Some(VertexAddress::IcosahedronVertex(_))
                )
            }));
        }
    }
    assert!(
        fixture
            .source
            .addresses
            .iter()
            .flatten()
            .filter(|address| matches!(address, VertexAddress::IcosahedronVertex(_)))
            .count()
            == 12
    );
}

#[test]
#[ignore = "prints the canonical snapshot when the fixture definition intentionally changes"]
fn print_n12_fixture_manifest_snapshot() {
    println!("{}", n12_research_fixture_manifests_json().unwrap());
}

fn lift(values: &[TriangleAddress]) -> BTreeSet<TriangleAddress> {
    values
        .iter()
        .flat_map(|parent| parent.children_2_to_1().unwrap())
        .collect()
}

fn pair(left: usize, right: usize) -> (usize, usize) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}
