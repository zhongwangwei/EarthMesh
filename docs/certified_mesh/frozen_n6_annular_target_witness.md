# Frozen N6 annular target witnesses

PR115 connects the PR113 incidence target to the PR114 PIER solver and the
existing exact annular certifier. For a fixed cell and root bridge it:

1. enumerates every positive split of the two duplicated root occurrences;
2. recovers all exact occurrence triangulations with PIER;
3. maps occurrences back to global source slots;
4. glues and certifies the annular topology;
5. recomputes the concrete annular signature and compares the root and every
   vertex incidence with the requested target;
6. deduplicates by the exact topology key.

One failed occurrence witness does not reject the target; all witnesses from
all root splits are checked. Interior edges exclude both boundary cycles so
PR116 can use them as dynamic forbidden edges for the second cell.

## Exact target key

The cache identity uses structural equality over the contract version, lower
cycle, upper cycle, forbidden-edge set, complete incidence vector, and root
bridge. A hash is not used as equality.

## Frozen N6 oracle

The two known TransitionCell V3 annular topologies were converted to exact
incidence targets and recovered independently:

| Cell | Boundaries | Root | Root splits | PIER states | Occurrence witnesses | Topologies | Known key |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| 0 | 8+20 | 20--24 | 8 | 1,597 | 1 | 1 | recovered |
| 1 | 20+28 | 15--20 | 8 | 1,977 | 1 | 1 | recovered |

Both exact known topology keys are present. This closes the PR115 cell-witness
gate and permits PR116 joint two-cell extraction.

## Scope

The two witnesses are not yet claimed as a new jointly selected SDCE result.
PR116 must apply dynamic cross-cell forbidden edges and rerun the complete
global topology gate. Geometry, Lifted-N12 extraction, the remaining CEC
shards, and product generation remain untouched. The best mixed angle range
remains `39.278499430048°--80.721500570507°`; strict `40°--80°` remains unmet.

Frozen evidence:
`rust/earthmesh_refine_certified/tests/fixtures/frozen_n6_annular_target_witness.json`.
Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.
