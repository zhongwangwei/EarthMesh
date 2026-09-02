# Lifted-N12 SDCE incidence contract

PR112 starts signature-directed concrete extraction (SDCE) by building the
global degree/link contract before selecting any annular triangles. It does not
run an incidence-plan CSP or concrete topology search.

## Current evidence

- Frozen CEC prefix: 16,384 unique states and 6,838 essential cycles.
- Exact fixed topology source: `fixed_triangles_for_face_complex(...)` after
  coarse-core lowering.
- Contracts built: 6,838 / 6,838.
- Transition vertex domains: 892,213.
- Legal owner tuples: 7,420,459.
- Empty vertex domains: 0.
- Invalid cell incidence sums: 0.
- Fixed-link adapter mismatches: 0.
- Transition charge target: 4 for every audited cycle.

Frozen N6 independently builds the same two-cell contract and confirms that
every cell incidence sum is three times its annular triangle count.

## Declared no-ear family

The contract is the zero post-hoc anchor-ear family. Original icosahedron
anchors have final-degree domain `{5}`; ordinary transition vertices have
`{5,6,7}`. Each cell owner contributes a positive triangle-incidence count.
For shared interface vertices, all positive owner tuples summing to the chosen
final degree minus the exact fixed degree are retained.

The six frozen PR111 defect slots are telemetry only. Production contract code
contains no slot-specific branch. Across all 6,838 cycles they are ordinary,
single-cell vertices with three legal owner tuples:

| Source slot | Exact fixed degree |
| ---: | ---: |
| 48 | 2 |
| 52 | 4 |
| 78 | 2 |
| 252 | 2 |
| 256 | 2 |
| 343 | 3 |

This explains why the balanced-strip results could produce degree 4/8/9: those
concrete strips selected the wrong owner incidence. The global contract itself
does not require those defects.

## Incidence and link contract

For an annular cell with boundary sizes `m` and `n`, the contract records
`m+n` triangles and exactly `3(m+n)` vertex incidences. Fixed-only ordinary
vertices must already have degree 5--7 and fixed-only anchors degree 5.

Every transition vertex has exactly two link-path providers:

- fixed topology plus one cell at an exterior boundary; or
- the two cells at their shared interface.

Provider endpoints must agree. Fixed degrees and paths come from actual final
fixed triangles; `VertexLinkContract.fixed_link_edges.len()` is not used as a
degree oracle.

The necessary charge equation is frozen as

```text
sum_transition(6 - final_degree)
  = 12 - sum_fixed_only(6 - fixed_degree).
```

## Concrete witness scope and completeness

Concrete witness scope in PR112 is empty. No annular triangulation, balanced
strip, ear repair, or geometry state is generated.

The contract builder is complete for its declared linear domains: it enumerates
every positive composition of `final_degree - fixed_degree` across the actual
cell owners. It does **not** claim that every tuple has a concrete annular
triangulation; PR113 will solve global cell sums and charge, and later PIER work
will decide concrete realizability.

## Reachability audit

The existing annular reachability path stores:

- incidence signatures;
- boundary link-path signatures; and
- member counts.

It stores neither concrete witnesses nor backpointers and remains a necessary
relaxation where glue-invalid states may survive. It therefore cannot directly
extract an SDCE witness.

## Exact/incomplete distinction and checkpoint

An invalid fixed-only degree, malformed path, provider mismatch, or adapter
mismatch is an exact input-contract error. An empty owner-tuple domain is a
scoped zero-ear contract no-plan reason. Passing PR112 only establishes
nonempty exact domains; it is not a topology closure.

No new checkpoint exists because PR112 performs no search. The remaining 49
CEC shards stay locked.

## Go / No-Go

**Go to PR113 GIPC.** All 6,838 audited cycles have nonempty vertex domains,
exact cell sums, and zero adapter mismatches.

## Gate impact

- Concrete topology search: not run.
- Geometry: not attempted.
- Remaining CEC shards: not resumed.
- Product and angle gates: unchanged.
- Best mixed angle range remains
  `39.278499430048°--80.721500570507°`; the strict `40°--80°` publication gate
  and `40.2°--79.8°` internal gate remain unmet.

Frozen evidence:
`rust/earthmesh_refine_certified/tests/fixtures/n12_sdce_incidence_contract.json`.
Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.
