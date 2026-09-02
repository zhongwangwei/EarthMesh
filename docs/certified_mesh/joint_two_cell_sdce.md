# Joint two-cell SDCE

PR116 combines two exact PIER annular witnesses without post-hoc anchor ears.
The cell with fewer witnesses is recovered first. Its non-interface interior
edges become dynamic forbidden edges for the other cell; shared interface
boundary edges remain legal because each adjacent cell must contribute one
triangle. Candidate pairs are ordered by their complete topology keys and the
checkpoint stores the exact plan, candidate order, and next pair index.

Every candidate that survives cross-cell duplicate-edge and duplicate-triangle
checks runs the existing complete final gate over the actual fixed triangles
plus both cells. The gate requires edge incidence two, anchor degree five,
ordinary degree five through seven, cyclic vertex links, Euler characteristic
two, and charge twelve. The selected-ear set must remain empty. A failed pair
does not reject the incidence plan; later concrete pairs are still examined.

## Frozen N6 closed oracle

| Metric | Result |
| --- | ---: |
| Candidate pairs | 1 |
| Pairs examined | 1 |
| Dynamic secondary targets | 1 |
| Dynamic forbidden edges | 28 |
| Selected ears | 0 |
| Vertices / edges / faces | 341 / 1,017 / 678 |
| Euler / charge | 2 / 12 |
| Anchor degrees | four at degree 5 |
| Ordinary degrees | 21 at 5, 303 at 6, 13 at 7 |

The recovered custom triangles and closure tuple equal the known Frozen N6
closed topology.

## Lifted-N12 entry gate

A deterministic bounded flip feasibility probe found a legal incidence plan at
depth 12 from the existing 80-by-256 balanced seed families. PIER recovered one
witness for each cell. Dynamic extraction retained one compatible candidate
pair after adding 56 forbidden edges to the secondary target. A zero pair
budget then returned `SearchIncomplete` with an exact pair checkpoint, proving
entry into joint concrete extraction without claiming Lifted topology closure.

The remaining 49 CEC shards, geometry, and product gates were not run. PR117
must provide fair scheduling across Lifted cycles and plans. The best mixed
geometry remains `39.278499430048°--80.721500570507°`; strict `40°--80°` is
still unmet.

Frozen evidence:

- `rust/earthmesh_refine_certified/tests/fixtures/frozen_n6_joint_concrete.json`
- `rust/earthmesh_refine_certified/tests/fixtures/n12_joint_entry.json`

Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.
