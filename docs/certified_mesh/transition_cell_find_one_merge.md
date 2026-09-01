# V3 transition-cell find-one merge

PR107 generalizes the exact global merge boundary from legacy disk sectors to
`TransitionCellTopology`. Annular topology triangles, vertex incidences, link
edges, and canonical keys now enter the same find-one product search.

## Concrete inclusion

`certify_annular_topology()` validates a supplied concrete member against the
PR105 annulus contract:

- exactly `m+n` triangles and `2(m+n)` edges;
- two fixed boundary cycles and Euler characteristic zero;
- manifold edge incidence;
- one path link at every boundary vertex;
- no forbidden edge;
- canonical minimum bridge key.

Because CSAE is complete for that declared family, a topology passing this
validator is a V3 family member without materializing the surrounding Catalan
family.

## Global merge

`solve_transition_cell_find_one()` walks the bounded product of generalized
cell families. For each selection it reuses the existing shared anchor-ear
solver, but supplies V3 link contracts directly. The final gate checks:

- every edge has incidence two;
- every vertex link is one cycle;
- pentagon anchors have degree five;
- ordinary vertices have degree 5--7;
- sphere Euler characteristic is two;
- total charge is twelve.

The first closed topology is materialized and returned. Exhausting a complete
family is exact no-solution; exhausting the topology-state limit is
`SearchIncomplete`.

## Frozen N6 gate

The frozen legacy selected topology partitions into certified V3 annular cells
of sizes 8+20 and 20+28. The two singleton V3 families close in one global
state with no anchor ears and reproduce the legacy topology exactly:

- vertices / edges / faces: `341 / 1017 / 678`;
- Euler / charge: `2 / 12`;
- identical anchor degrees and ordinary degree histogram;
- identical custom triangle set.

This is an inclusion oracle, not a claim that raw N6 or Lifted-N12 concrete
family generation is complete. Geometry and product gates remain unchanged.
