# Alpha6 canonical problem identity and legacy profile

PR89 canonicalized all 924 Frozen N6 retained-core/corridor attempts without
changing the Alpha5 solver. Exact equality now includes source subdivision,
source-face ring, canonical face/vertex/edge identities, adjacency, boundary
sets, anchor policy, retained parents, corridor family, and both solver
contract versions. Runtime face slots are absent from the key.

## Identity audit

| metric | result |
|---|---:|
| attempts canonicalized | 924 / 924 |
| unique full exact keys | 924 |
| unique address graphs | 672 |
| repeated address-graph attempts | 252 |
| legacy fingerprint buckets | 518 |
| legacy fingerprint buckets containing multiple exact keys | 154 |

The old fingerprint is therefore diagnostic only. It omits source subdivision,
source-face ring, retained parents, anchor policy, adjacency, and contract
versions and cannot key exact cached conclusions.

## Frozen 16,384-state profile

| stage | attempts |
|---|---:|
| face band closed, downstream remained incomplete in PR84 | 384 |
| exact fine-cap precheck rejection | 252 |
| exact propagation rejection | 13 |
| face-band state budget exhausted | 275 |

The 265 exact rejections and the other 659 attempts exactly reproduce the
PR84 accounting. Among those 659 unknowns, 154 use 100–124 transition faces,
154 use 128–152, and 351 use 158–182.

The two-source-ring families dominate legacy work: they consumed 3,245,008 of
4,548,047 raw states. F0 consumed 1,289,375; F1 consumed only 13,664. This
identifies the large F2–F5 complexes, rather than a uniform per-family cost, as
the principal state-explosion location.

The solver created 2,078,938 full-domain clone checkpoints and copied at least
2,832,196,808 bytes of key/domain payload (1,362.33 bytes per checkpoint,
excluding allocator/tree overhead). Only 9.93% of raw states reached leaf
validation. These measurements motivate rollback state; they do not alter any
legacy outcome.

Canonical evidence is stored in
`rust/earthmesh_refine_certified/tests/fixtures/frozen_n6_problem_profile.json`.
