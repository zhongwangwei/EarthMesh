# Lifted-N12 R2 repair-support preflight

PR111 reclassifies the complete post-PR110 balanced topology-pair matrix before
any new anchor-repair solver is introduced. It separates a registered R2 depth
limit from a topology defect that no anchor-ear depth can change.

## Scope

- Frozen CEC prefix: 16,384 unique states and 6,838 essential cycles.
- Retained cell subset: the deterministic balanced annular prefix, at most 64
  topologies per cell.
- Exact retained pair product: 27,697,152.
- Registered repair family: anchor-ear contraction with at most two ears per
  anchor (`R2`).
- Geometry, production gates, the remaining 49 CEC shards, and the repair
  solver are unchanged.
- This is evidence about the retained balanced subset, not a no-solution proof
  for the complete CSAE family.

## V2 semantics

| Class | Meaning |
| --- | --- |
| `DirectNoEarCandidate` | No anchor ear is required; the final gate still decides closure. |
| `RepairDepthR2Candidate` | Anchor degrees fit R2 and the ordinary repair-support preflight passes. |
| `OutsideRepairDepthR2` | At least one anchor needs more than two ears. This is not permanent impossibility. |
| `ExactImpossibleForAllEarDepths` | A defect is invariant under every registered anchor-ear operation. |

The PR110 field `repairable_pairs` is retained only for historical fixture
compatibility. Its formal meaning is now `r2_anchor_necessary_candidates`: it
checks anchor degree and duplicate-triangle conditions only.

In particular, an anchor of degree 8 requires three ears to reach degree 5.
It is therefore outside R2, not mathematically impossible.

## Preflight

For a pair, the conservative ordinary repair support is the union of all
non-anchor vertices in mutable triangles incident to an overfull anchor. Every
current anchor-ear operation changes only the anchor and three vertices in this
set.
Consequently:

1. an ordinary degree outside 5--7 at a vertex outside the support is an exact
   reject;
2. a non-cycle link outside the support is an exact reject;
3. if the affected set is `A`, its initial degree sum is `D_A`, and the fixed
   required ear count is `K`, then a necessary condition is

   ```text
   5|A| <= D_A + K <= 7|A|.
   ```

The aggregate audit stops at the first exact reason per pair. Therefore a zero
count for a later check means no pair reached that check without an earlier
exact reject; it does not assert that the later defect is absent.

## Frozen full-pair result

| Measure | Result |
| --- | ---: |
| cycles audited / domains built | 6,838 / 6,838 |
| concrete cell topologies | 870,400 |
| exact pair product | 27,697,152 |
| direct no-ear candidates | 0 |
| permanent impossible | 4,734,444 (17.09%) |
| outside R2 | 22,962,708 (82.91%) |
| R2 anchor-necessary candidates | 4,734,444 |
| R2 preflight passed | 0 |
| R2 preflight rejected | 4,734,444 |
| pair / cycle / tier accounting | complete |
| audit errors | 0 |

Every R2 anchor-necessary candidate has an ordinary degree defect outside the
anchor-ear support:

| Required ears `K` | Candidates | Preflight passed | Exact rejected |
| ---: | ---: | ---: | ---: |
| 6 | 9,030 | 0 | 9,030 |
| 7 | 14,144 | 0 | 14,144 |
| 8 | 4,711,270 | 0 | 4,711,270 |

The stable primary-reason partition is:

| First unaffected defect | Pairs |
| --- | ---: |
| vertex 48, degree 4 | 31,778 |
| vertex 52, degree 8 | 146,450 |
| vertex 78, degree 8 | 2,629,013 |
| vertex 252, degree 4 | 13,577 |
| vertex 256, degree 8 | 640,444 |
| vertex 343, degree 8 | 123,888 |
| vertex 343, degree 9 | 1,149,294 |

These counts sum exactly to 4,734,444.

## Decision

ASVE-R2 has an empty input portfolio after the exact preflight, so implementing
or running it against this retained subset cannot produce a closure. The
registered next branch is signature-directed concrete extraction: recover
concrete annular topologies whose anchor and ordinary-degree signatures are
compatible before attempting anchor repair.

The 22,962,708 degree-8 pairs remain `OutsideRepairDepthR2`. PR111 does not
promote them to permanent no-solution and does not authorize R3; R3 still
requires its separately registered feasibility audit.

## Gate impact

- New repair solver run: no.
- Search result changed: no.
- Remaining 49 CEC shards resumed: no.
- Geometry attempted: no.
- Product or angle gate changed: no.
- Best mixed angle range remains
  `39.278499430048°--80.721500570507°`; the strict `40°--80°` publication gate
  and `40.2°--79.8°` internal transaction gate remain unmet.

Frozen evidence:
`rust/earthmesh_refine_certified/tests/fixtures/n12_r2_repair_support.json`.
Taskbook SHA-256:
`387311a0c1b2ed52f43515766c9fa785e6849ad819aa05e9c4b78efb65492c24`.
