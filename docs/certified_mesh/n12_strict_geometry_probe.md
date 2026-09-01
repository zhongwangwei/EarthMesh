# Alpha6 N12 strict geometry precondition

PR96 freezes the taskbook geometry protocol but does not run it because PR95
found no closed N12 topology. Geometry is conditional on a closed topology;
running the optimizer against an invalid or incomplete combinatorial mesh would
not be scientific evidence.

| Fixture | Topology input | Geometry |
| --- | --- | --- |
| N12-Lifted-N6 | `ResearchCycleSearchIncomplete` | not attempted |
| N12-Interior-Control | `ResearchExactNoSolution` | not attempted |

The frozen, unexecuted protocol retains the `40.2°–79.8°` target,
`HierarchyEdgeAreaDegree` targets, seven prescribed starts, and the
Current/+1/+2 nested domains. Delaunay/Voronoi and
physical/balance/remap research checks are likewise not attempted.

This PR writes no grid or ready marker and changes no product gate. The record
is stored in `tests/fixtures/n12_strict_geometry_probe.json`.

No new angle witness exists. The best known mixed range remains
`39.278499430048°–80.721500570507°`.
