#[test]
fn strict_geometry_protocol_stays_blocked_without_closed_topology() {
    let topology = include_str!("fixtures/n12_cec_topology_probe.json");
    let geometry = include_str!("fixtures/n12_strict_geometry_probe.json");

    assert_eq!(
        topology
            .matches("\"outcome\":\"ResearchTopologyClosed\"")
            .count(),
        0
    );
    assert_eq!(geometry.matches("\"geometry_attempted\":false").count(), 2);
    assert_eq!(geometry.matches("\"geometry_outcome\":null").count(), 2);
    assert!(geometry.contains("\"strict_angle_degrees\":[40.2,79.8]"));
    assert!(geometry.contains("\"product_gate_changed\":false"));
}
