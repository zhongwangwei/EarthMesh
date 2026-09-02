# Lifted-N12 fair topology-pair audit

PR110 is a read-only audit of the post-PR109 bottleneck. It classifies every
balanced topology pair in the frozen 16,384-state Lifted-N12 prefix, records
the current first-pair ear branching, and leaves the production solver and all
publication gates unchanged.

## Current evidence

PR109 built 870,400 concrete cell topologies for 6,838 essential cycles, but
entered only the lexicographically first global pair per cycle. Each first
pair consumed the full 256-state ear budget, so the run stopped after 6,838
global pairs and 1,750,528 ear states with `DownstreamSearchIncomplete`.

## Search scope and exact semantics

- CEC prefix: 16,384 unique states and 6,838 essential cycles.
- Cell subset: the existing deterministic balanced-strip prefix, at most 64
  topologies per annular cell.
- Pair scope: the exact Cartesian product of the two retained cell subsets for
  each cycle.
- Static hard rejects: duplicate triangles or an anchor degree outside the
  current degree-5, at-most-two-ears repair range.
- Geometry is deferred and is neither evaluated nor used as a hard reject.
- `repairable` below means only that the pair passes these anchor necessary
  conditions. It does not mean that ear repair or the final global gate closes.
- Exhausting or rejecting this partial balanced subset is not a CSAE-family
  no-solution proof.

## Frozen pair matrix

| Measure | Result |
| --- | ---: |
| cycles audited / domains built | 6,838 / 6,838 |
| concrete cell topologies | 870,400 |
| exact global pair product | 27,697,152 |
| zero-ear pairs | 0 |
| direct zero-ear closures | 0 |
| anchor-necessary repair candidates | 4,734,444 (17.09%) |
| impossible before ear | 22,962,708 (82.91%) |
| pair accounting complete | true |
| audit errors | 0 |

The repair candidates contain no low-ear case:

| Total required ears `K` | Pairs |
| ---: | ---: |
| 0 | 0 |
| 1 | 0 |
| 2 | 0 |
| 3--5 | 0 |
| 6 | 9,030 |
| 7 | 14,144 |
| 8 | 4,711,270 |

Across the four anchors of every pair, the initial degree histogram is
`6: 91,648`, `7: 70,803,008`, and `8: 39,893,952`. All static rejects are
degree-8 anchors; the frozen subset contains no duplicate-triangle or
below-degree-5 primary reject.

## First pair versus best anchor-ranked pair

The rank uses the PR110 anchor repair score only: pair class, total required
ears, overfull-anchor count, then stable pair indices. Ordinary/link/geometry
terms remain telemetry and are not production scheduling policy in this PR.

| Evidence | Result |
| --- | ---: |
| first-pair rank, minimum | 1 |
| first-pair rank, maximum | 2,625 |
| first-pair rank, mean | 675.318222 |
| first pair repairable | 71 cycles (1.04%) |
| first pair impossible | 6,767 cycles (98.96%) |
| best ranked pair repairable | 3,795 cycles (55.50%) |
| every pair impossible | 3,043 cycles (44.50%) |

Every first pair has zero unmatched edges and zero broken ordinary links, but
it still has 11--22 ordinary degree defects. This proves that PR109 normally
spent its entire ear quantum on a pair already outside the current anchor
repair domain, while a better anchor-ranked pair exists for more than half of
the cycles.

## First-cycle ear trace

The representative current first pair has four degree-7 anchors and therefore
requires `K=8` ears. Initial candidates per anchor are `2, 4, 4, 2`. The
existing DFS examines 256 states, reaches depth 8, and encounters 117 duplicate
seen states. Its nodes by depth are
`1,1,1,1,2,10,50,108,83`; no candidate apply is rejected and no two anchor
candidate supports interact. The final gate reaches ordinary-degree failures
at vertices 73 and 78, each 18 times. The result is `SearchIncomplete`.

This trace is representative branching evidence, not an exact result for that
pair. The PR109 aggregate remains the evidence that all 6,838 first pairs used
the same 256-state quantum.

## Oracle and fairness evidence

The Frozen N6 singleton V3 pair is the identity-ear oracle. The audit counts
one pair, classifies it as zero-ear, sends it through the same final gate as the
old solver, closes it, and reproduces the old `Closed` result. Pair counts also
equal the family Cartesian product in both N6 and Lifted-N12 tests.

PR110 does not change scheduling. It proves the unfairness and supplies the
static evidence required to change it safely: PR109 entered 6,838 of the
27,697,152 pairs now classified, and 98.96% of those first pairs fail the
current anchor necessary condition.

## Go / No-Go

The pre-registered simple zero-ear / `K<=2` scheduler gate does not pass.
However, 4,734,444 pairs remain inside the current per-anchor repair range, so
the balanced subset is not uniformly statically impossible. The frozen result
is therefore `GoAnchorRepairVariants`: next isolate and deduplicate exact local
anchor-star variants (then combine them under the global constraints) instead
of increasing the 256-state first-pair budget. Signature-directed concrete
extraction remains the fallback for the 3,043 cycles whose entire retained
pair subset is statically impossible.

## Gate impact

- Production search result changed: no.
- Remaining 49 CEC shards resumed: no.
- Geometry attempted: no.
- Product or angle contract changed: no.
- Best mixed angle range remains
  `39.278499430048°--80.721500570507°`; strict `40°--80°` publication and
  `40.2°--79.8°` internal gates remain unmet.

The byte-stable evidence is
`rust/earthmesh_refine_certified/tests/fixtures/n12_fair_pair_audit.json`.
