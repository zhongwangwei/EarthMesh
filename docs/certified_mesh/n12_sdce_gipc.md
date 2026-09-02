# Lifted-N12 SDCE global incidence plans

PR113 adds the Global Incidence Plan CSP (GIPC). It chooses final vertex
degrees and per-cell triangle-incidence counts before any concrete annular
triangles are generated.

## Current evidence

- Frozen CEC prefix: 16,384 unique states and 6,838 essential cycles.
- Exact PR112 contracts built: 6,838.
- Zero-ear incidence plans found: 6,838.
- Exact no-plan cycles: 0.
- Search-incomplete cycles: 0.
- Invalid cycles: 0.
- GIPC states: 899,051.
- Cell-sum prunes: 2,334,301.
- Charge prunes: 0.
- Checkpoints required by the frozen run: 0.
- First plan scores: ordinary curvature `0`, incidence roughness `48`.

Every frozen PR111 defect slot is assigned final degree 6 in every selected
plan:

```text
48, 52, 78, 252, 256, 343 -> degree 6 in 6,838 / 6,838 plans
```

This is the first complete-prefix evidence that the ordinary degree 4/8/9
failures were artifacts of concrete balanced-strip incidence allocation rather
than a global linear degree obstruction.

## Declared no-ear family

GIPC uses the PR112 zero post-hoc anchor-ear contract. Each variable is one
global transition vertex and each value is one complete tuple:

```text
(final degree, per-owner cell incidence counts)
```

Original anchors remain degree 5. Ordinary vertices remain degree 5--7. Every
owner count is positive. A shared-interface tuple assigns both cells at once,
so the split cannot become inconsistent later.

## Incidence CSP

Variables use static MRV by exact tuple-domain cardinality. Equal domains are
ordered by caller-supplied research priority, anchor status, shared-interface
status, higher fixed contribution, then canonical source slot. Production code
contains no frozen slot constants; the six PR111 slots are supplied only by the
research audit.

For each cell, every partial state enforces

```text
assigned + remaining_min <= 3 * triangle_count
assigned + remaining_max >= 3 * triangle_count.
```

The same lower/upper-bound propagation is applied to the transition charge.
Value preference is degree 6 first for ordinary vertices, then charge distance,
local incidence roughness, and canonical tuple order. Preference changes only
find-one order; the exact plan family is unchanged.

## Concrete witness scope and completeness

Concrete witness scope remains empty. GIPC does not select root bridges,
polygon ears, diagonals, triangles, or topology pairs.

Within the declared incidence contract, the DFS is finite and complete:

- every contract tuple is branched or soundly removed by a cell-sum/charge
  bound;
- an exhausted frontier is `ExactNoPlan` for that cycle's zero-ear incidence
  family;
- a nonempty frontier at the state limit is `SearchIncomplete`;
- `Found` satisfies every exact cell total and the exact transition charge.

`ExactNoPlan` would not prove the complete CSAE plus repair family impossible.
No audited Lifted-N12 cycle reached that outcome.

## Checkpoint

The checkpoint stores the exact contract/cycle identity, deterministic variable
order, cumulative evidence, and every unvisited DFS frontier state with its
tuple prefix, cell sums, and charge. Resume validates and recomputes each
partial state before continuing. A split run returns the same first plan as a
one-shot run.

## Oracle

Frozen N6 finds a legal two-cell incidence plan before triangles. Unit oracles
cover sound cell-sum and charge pruning, checkpoint/resume equality, value-order
family invariance, typed budget incompleteness, and scoped zero-ear no-plan
semantics.

## Go / No-Go

**Go to PR114 PIER.** Every one of the 6,838 fixed-prefix cycles has a legal
zero-ear global incidence plan. The next question is exact concrete annular
realizability of a selected target, not degree-plan existence.

## Gate impact

- Concrete topology generated: no.
- Topology closure claimed: no.
- Geometry attempted: no.
- Remaining 49 CEC shards resumed: no.
- Product and angle gates changed: no.
- Best mixed angle range remains
  `39.278499430048°--80.721500570507°`; strict `40°--80°` publication and
  `40.2°--79.8°` internal gates remain unmet.

Frozen evidence:
`rust/earthmesh_refine_certified/tests/fixtures/n12_sdce_gipc.json`.
Taskbook SHA-256:
`65f26b64c78dd7dfadaaf2a1099f52d11c6a67461afb0a9558edbbf5941ef473`.
