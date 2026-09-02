# Lifted-N12 fixed-prefix SDCE closure

PR117 gives every one of the existing 6,838 Lifted-N12 essential cycles the
same bounded screening quantum. Screening starts from eight deterministic
annular seeds per cell and ranks the resulting pairs by their exact distance
to the `GlobalIncidenceContract`; it does not accept a balanced strip merely
because it was generated. The eight best cycles are then searched through
legal diagonal flips. A zero-distance pair is converted to a complete
`GlobalIncidencePlan` and must still pass exact PIER recovery, dynamic
two-cell extraction, and the existing final topology gate.

The frozen budget closes the first finalist at flip depth 40:

| Metric | Result |
| --- | ---: |
| CEC unique states | 16,384 |
| Essential cycles screened | 6,838 |
| Screening pairs scored | 437,632 |
| Best screening distance | 21 |
| Finalists retained / examined | 8 / 1 |
| Closed-plan pairs scored | 220,984 |
| PIER witnesses, lower / upper | 1 / 1 |
| Dynamic forbidden edges | 67 |
| Joint pairs examined | 1 |
| Selected post-hoc ears | 0 |
| Vertices / edges / faces | 1,293 / 3,873 / 2,582 |
| Euler / charge | 2 / 12 |

All four original anchors finish at degree five. The PR111 defect slots finish
at degrees `48:5`, `52:7`, `78:7`, `252:5`, `256:7`, and `343:7`; no ordinary
vertex is outside degree five through seven. This closes the PR117 topology
gate without resuming any of the remaining 49 CEC shards.

This is a deterministic find-one result, not an exhaustive proof of the flip
graph or the complete CSAE family. Geometry was deliberately not run. The
previous best mixed geometry remains `39.278499430048°--80.721500570507°`, so
strict `40°--80°` is not yet claimed; PR118 may now evaluate the frozen closed
topology under the `40.2°--79.8°` internal window.

Frozen evidence:

- `rust/earthmesh_refine_certified/tests/fixtures/n12_sdce_find_one.json`

Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.
