use earthmesh_refine_certified::{
    coarsen::{
        build_stratified_annulus, build_stratified_annulus_from_coupled, extract_coupled_annulus,
        n6_legacy_mixed_fixture, plan_hierarchy_components_from_parent_requirements,
        BandComponentKind, BoundaryIncidenceContract, DirectedTrace, ExplicitParentRequirement,
        HierarchyComponent, RingAnchorKind, StratifiedAnnulus, StratifiedAnnulusError,
        VertexLinkContract,
    },
    MotherGrid, TriangleAddress, TriangleOrientation,
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
fn fixture_a_triple_trace_is_typed_unsupported_by_pr36a_regular_gate() {
    let (source, component) = frozen_fixture_a();
    let coupled = extract_coupled_annulus(&source, &component).unwrap();
    let outcome = build_stratified_annulus_from_coupled(&source, &component, coupled);
    assert!(matches!(
        outcome,
        Err(StratifiedAnnulusError::UnsupportedNonDiskBandComponent { .. })
            | Err(StratifiedAnnulusError::UnsupportedMultipleBandIntervals { .. })
            | Err(StratifiedAnnulusError::UnsupportedMultiCycleBandComponent { .. })
            | Err(StratifiedAnnulusError::UnsupportedTripleTraceJunction { .. })
    ));
}

#[test]
fn fixture_b_reports_exact_shared_vertex_sets_between_topology_traces() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = stratified_fixture_b(&source, component);

    assert_eq!(stratified.traces.len(), 3);
    assert_eq!(
        shared_slots(&stratified.traces[0], &stratified.traces[1]),
        slots([24, 29, 75, 77, 151, 169])
    );
    assert_eq!(
        shared_slots(&stratified.traces[1], &stratified.traces[2]),
        slots([15, 26, 73, 86, 148, 153, 164, 172])
    );
    assert!(shared_slots(&stratified.traces[0], &stratified.traces[2]).is_empty());
}

#[test]
fn fixture_b_assigns_every_annulus_face_to_exactly_one_band() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = stratified_fixture_b(&source, component);

    assert_eq!(stratified.probe.band_count, 2);
    assert_eq!(stratified.bands.len(), 14);
    assert_eq!(stratified.probe.sector_count, 14);
    assert_band_partition_closes(&stratified);
    assert!(stratified.bands.iter().all(|band| matches!(
        band.kind,
        BandComponentKind::Annular { .. } | BandComponentKind::SectorDisk { .. }
    )));
}

#[test]
fn fixture_b_freezes_anchor_link_contracts() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = stratified_fixture_b(&source, component);

    assert_anchor_link(&stratified, 29, 11, 2);
    assert_anchor_link(&stratified, 77, 10, 2);
    assert_anchor_link(&stratified, 2, 0, 4);
    assert_anchor_link(&stratified, 155, 2, 4);
}

#[test]
fn fixture_b_shared_junctions_have_single_rotation_port_per_band() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = stratified_fixture_b(&source, component);

    let expected_shared = slots([24, 29, 75, 77, 151, 169, 15, 26, 73, 86, 148, 153, 164, 172]);
    let actual = stratified
        .shared_junctions
        .iter()
        .map(|junction| junction.source_slot)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected_shared);
    assert!(stratified.shared_junctions.iter().all(|junction| {
        junction.ports.len() == 2
            && junction
                .ports
                .iter()
                .all(|port| port.source_slot == junction.source_slot)
    }));
}

#[test]
fn fixture_b_probe_json_is_deterministic_and_machine_readable() {
    let (source, component) = n6_legacy_mixed_fixture().unwrap();
    let stratified = stratified_fixture_b(&source, component);

    let first = stratified.to_record().to_json_string();
    let second = stratified.to_record().to_json_string();

    assert_eq!(first, second);
    assert!(first.contains("\"topology_trace_count\":3"));
    assert!(first.contains("\"band_count\":2"));
    assert!(first.contains("\"sector_count\":14"));
    assert!(first.contains("\"shared_junction_count\":14"));
    assert!(
        first.contains("\"shared_vertex_slots\":[15,24,26,29,73,75,77,86,148,151,153,164,169,172]")
    );
    assert!(!first.contains("\\\""));
    assert!(first.contains("\"trace_pair_shared_slots\":"));
    assert!(first.contains("\"sector_components\":"));
    assert!(first.contains("\"junction_rotation_intervals\":"));

    let record = stratified.to_record();
    assert_eq!(record.trace_pair_shared_slots.len(), 2);
    assert_eq!(
        record.trace_pair_shared_slots[0].source_slots,
        vec![24, 29, 75, 77, 151, 169]
    );
    assert_eq!(
        record.trace_pair_shared_slots[1].source_slots,
        vec![15, 26, 73, 86, 148, 153, 164, 172]
    );
    assert_eq!(record.sector_components.len(), 14);
    assert_eq!(record.junction_rotation_intervals.len(), 28);
    assert!(record.sector_components.iter().all(|sector| {
        sector.lower_chain.first() == Some(&sector.start_junction)
            && sector.lower_chain.last() == Some(&sector.end_junction)
            && sector.upper_chain.first() == Some(&sector.start_junction)
            && sector.upper_chain.last() == Some(&sector.end_junction)
    }));
    assert_eq!(record.anchor_target_degree_ranges.get(&29), Some(&(5, 5)));
    assert_eq!(
        record.anchor_fixed_link_edges.get(&29),
        Some(
            &stratified.link_contracts[&29]
                .fixed_link_edges
                .iter()
                .copied()
                .collect::<Vec<_>>()
        )
    );
}

fn stratified_fixture_b(source: &MotherGrid, component: HierarchyComponent) -> StratifiedAnnulus {
    build_stratified_annulus(source, &component).unwrap()
}

fn assert_band_partition_closes(stratified: &StratifiedAnnulus) {
    let mut seen = BTreeSet::new();
    for band in &stratified.bands {
        assert!(!band.face_slots.is_empty());
        for &face in &band.face_slots {
            assert!(
                seen.insert(face),
                "face {face} was assigned to multiple bands"
            );
        }
    }
    assert_eq!(
        seen,
        stratified
            .coupled
            .annulus_face_slots
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    );
}

fn assert_anchor_link(
    stratified: &StratifiedAnnulus,
    slot: usize,
    base_vertex: u8,
    fixed_edge_count: usize,
) {
    let contract = stratified
        .link_contracts
        .get(&slot)
        .unwrap_or_else(|| panic!("missing link contract for slot {slot}"));
    assert_vertex_link_contract(contract, base_vertex, fixed_edge_count);
    let boundary = stratified
        .coupled
        .boundary_contracts
        .iter()
        .find(|contract| contract.source_slot == slot)
        .unwrap_or_else(|| panic!("missing boundary incidence contract for slot {slot}"));
    assert_boundary_incidence_contract(boundary, slot);
}

fn assert_vertex_link_contract(
    contract: &VertexLinkContract,
    base_vertex: u8,
    fixed_edge_count: usize,
) {
    assert!(matches!(
        contract.anchor_kind,
        RingAnchorKind::IcosahedronPentagon { base_vertex: actual } if actual == base_vertex
    ));
    assert_eq!(contract.fixed_link_edges.len(), fixed_edge_count);
    assert_eq!(contract.target_degree_min, 5);
    assert_eq!(contract.target_degree_max, 5);
}

fn assert_boundary_incidence_contract(contract: &BoundaryIncidenceContract, slot: usize) {
    assert_eq!(contract.source_slot, slot);
    assert!(contract.fixed_position);
    assert_eq!(contract.allowed_global_degree_min, 5);
    assert_eq!(contract.allowed_global_degree_max, 5);
}

fn shared_slots(a: &DirectedTrace, b: &DirectedTrace) -> BTreeSet<usize> {
    trace_slots(a)
        .intersection(&trace_slots(b))
        .copied()
        .collect()
}

fn trace_slots(trace: &DirectedTrace) -> BTreeSet<usize> {
    trace
        .occurrences
        .iter()
        .map(|occurrence| occurrence.source_slot)
        .collect()
}

fn slots<const N: usize>(slots: [usize; N]) -> BTreeSet<usize> {
    slots.into_iter().collect()
}
