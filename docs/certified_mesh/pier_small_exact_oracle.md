# PIER exact polygon incidence recovery

PR114 adds Polygon Incidence Ear Reconstruction (PIER). Given a canonical
seam-cut annulus and an exact incidence count for every cut occurrence, PIER
recovers polygon triangulations without choosing geometry.

## Exact recurrence

For an incidence-one occurrence, PIER removes its ear triangle and decrements
the two neighbouring incidences. Every live state preserves

```text
sum(remaining incidences) = 3 * (remaining occurrences - 2).
```

The three-occurrence base accepts only `(1, 1, 1)`. The search rejects
degenerate global triangles, forbidden diagonals, duplicate non-root global
edges, and cross-boundary edges smaller than the canonical root bridge.
Equivalent states are keyed by the remaining occurrence sequence, remaining
incidences, and inserted global edges.

## Checkpoint semantics

A budgeted run returns `SearchIncomplete` with the complete DFS frontier and
seen-state set. Resume verifies the target identity, cyclic occurrence order,
incidence consumption, triangle history, and diagonal history. Frontier
exhaustion is `ExactNoWitness`; it is not reported from a nonempty frontier.

## Small exact CSAE oracle

Existing complete CSAE families were grouped by canonical root bridge and
global vertex-incidence vector. PIER enumerated every positive split of the two
root occurrences and recovered exactly the topology-key set in each group.

| Annulus | Targets | CSAE topologies | PIER topologies | PIER states | Equal |
| --- | ---: | ---: | ---: | ---: | --- |
| 3+3 | 20 | 21 | 21 | 585 | yes |
| 3+4 | 128 | 132 | 132 | 5,837 | yes |
| 4+4 | 826 | 844 | 844 | 57,723 | yes |
| 4+5 | 4,110 | 4,180 | 4,180 | 428,101 | yes |

A sum-valid relaxed occurrence signature with no legal ear sequence also
returns `ExactNoWitness`, so PIER does not manufacture a concrete topology
from a reachability-only signature.

## Gate impact

The PR114 grouped-family gate passes. PR115 may now glue a selected SDCE target
and run the Frozen N6 known-topology oracle. This PR does not run Lifted-N12
joint extraction, geometry, the remaining CEC shards, or product generation.
The best mixed angle range remains
`39.278499430048°--80.721500570507°`; strict `40°--80°` remains unmet.

Frozen evidence:
`rust/earthmesh_refine_certified/tests/fixtures/pier_small_exact_oracle.json`.
Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.
