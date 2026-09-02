# Alpha6 N12 strict geometry probe

PR118 runs the frozen geometry protocol on the closed PR117 SDCE topology. It
uses `HierarchyEdgeAreaDegree` targets, max-min active-tangent steps, the six
concrete geometry starts plus inherited continuation, and the Current/+1/+2
nested domains.

The bounded screen gives every domain/start pair one iteration, then spends 64
iterations on the best inherited +2-ring witness. All 21 attempts remain in
the `Untangle` phase and exhaust their budgets. The best screened range is
`1.337009876734°–173.470265136292°`; the deepened witness regresses to
`0.004437965593°–179.982388813662°` and is not accepted as the incumbent.

No attempt reaches angle feasibility, so Delaunay/Voronoi and downstream
physical/balance/remap gates are not claimed. The strict `40.2°–79.8°`
internal window and final `40°–80°` product window remain unmet. The existing
mixed-topology baseline `39.278499430048°–80.721500570507°` is unchanged; it is
not a witness for the PR117 topology.

This probe writes no grid or ready marker, does not resume the 49 CEC shards,
and changes no product gate. Frozen evidence is stored in
`tests/fixtures/n12_strict_geometry_probe.json`.
