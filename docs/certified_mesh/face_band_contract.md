# Source-face band contract

This contract replaces shared-vertex set intersection as the definition of
transition width. The existing `band_cycle_planner` remains scoped evidence for
`ParentLayerTraceFamily`; it is not a complete source-graph search.

## Rotation-aware width

For adjacent traces meeting at a source vertex, order all incident source faces
by the vertex rotation and collect the contiguous faces belonging to the band
between those traces.

- no intervening band face: `TruePinch`;
- minimum intervening wedge of one face: `OneFaceWedge`;
- minimum intervening wedge of two or more faces: `MultiFaceWedge`;
- an original degree-5 anchor: `AnchorJunction`, with its wedge width recorded.

Therefore `shared_vertices > 0` is a conservative warning, not an equivalence
to zero combinatorial width.

## Face-band problem

Let `K` be a fixed source-triangle transition complex with coarse boundary
`∂cK` and fine boundary `∂fK`. A `W`-band plan assigns each transition face a
label `b(f) ∈ {0, …, W-1}` with:

1. coarse-boundary faces fixed to `0`;
2. fine-boundary faces fixed to `W-1`;
3. `|b(f)-b(g)| ≤ 1` for edge-adjacent faces;
4. `max(Bv)-min(Bv) ≤ 1` for labels incident to every source vertex;
5. every label non-empty and dual-edge connected.

Internal interface `Ii` consists of source edges separating labels `i-1` and
`i`. Each interface must be one connected degree-2 graph, hence one simple
cycle. Internal interfaces must be vertex-disjoint and must not touch fixed
coarse/fine boundaries without an explicit boundary-anchor contract.

Each band subcomplex must be an annular strip: connected, Euler characteristic
zero, and exactly two boundary cycles.

## Original anchors

An original icosahedron anchor may be:

- inside one band, with all incident transition faces carrying one label;
- on one interface, with only its two adjacent labels and interface degree 2;
- inside a fine cap connected to the fine exterior by a deterministic face
  corridor.

The anchor remains fixed with global degree 5 and one link cycle. Entering the
transition interior is not by itself an error.

## Exact outcomes and scope

`Closed` proves a face-band plan for the named face complex and anchor policy.
`FamilyExhaustedNoSolution` excludes only that finite family.
`SearchBudgetExhausted` is unknown. None of these outcomes may be extrapolated
to a larger face complex, different anchor cap, released coarse core, or added
source vertices.

## Frozen N6 rotation audit

| trace family | adjacent-pair shared occurrences | unique vertices | true pinches | one-face wedges | multi-face wedges | anchor junctions |
|---|---:|---:|---:|---:|---:|---:|
| legacy W2 | 14 | 14 | 0 | 12 | 0 | 2 |
| PR54 W3 candidate (`outward=1`, `width=2`) | 24 | 20 | 0 | 20 | 0 | 4 |

The PR53 statement “85% of the worst 100 angles are near zero-width pinches”
was based on shared-vertex proximity. Under the rotation-aware definition the
current legacy W2 has no `TruePinch`, so the corresponding true-pinch fraction
is `0%`. The 85% remains valid only as proximity to nominal shared junctions.

This correction does not certify the current topology or its angle range. The
next construction gate is exact pinch-free W2 face labeling, with an internal
separator that does not touch either fixed boundary.

## Frozen N6 exact PF-W2 result

The exact PR56 search closes at the first ladder step, `F0`, so no face-ring
expansion, coarse-core sacrifice, or anchor cap is attempted:

| metric | result |
|---|---:|
| transition faces | 88 |
| coarse / fine boundary faces | 16 / 26 |
| exact states | 45 |
| band face counts | 36 / 52 |
| internal interface | one 20-edge, 20-vertex simple cycle |
| true pinches | 0 |
| cap / corridor faces | 0 / 0 |
| core faces sacrificed | 0 |

The interface is disjoint from both fixed boundary vertex sets. Original
anchors 2, 29, 77, and 155 each satisfy `InteriorOfSingleBand`. Both bands are
dual-connected annular strips with Euler characteristic zero and exactly two
boundary cycles.

The solver enumerates the full supplied binary label family in deterministic
MRV order if necessary; the distance potential only orders labels. Exhaustion
is scoped to the supplied face complex, while a state-budget hit remains
unknown. PR56 intentionally rejects `band_count=3` until the PR59 W3-specific
propagation and interface-disjointness gate is implemented.

## Frozen N6 PF-W2 topology result

PR57 converts the exact face labels into three ordered boundary cycles and
derives polygon-sector boundaries from source edges connecting consecutive
cycles. The two annular bands produce 8 and 20 disjoint sectors respectively;
the old parent-layer nominal traces are not used as sector boundaries.

The existing full-polygon enumeration, degree reachability, global merge,
anchor-ear, final-link, edge-incidence, Euler/charge, and hierarchy
materialization gates then close in 31 topology states:

| metric | result |
|---|---:|
| sectors | 28 (8 / 20) |
| selected sector topologies | 28 |
| selected anchor ears | 0 |
| final vertices / edges / faces | 341 / 1017 / 678 |
| anchor degrees | 2, 29, 77, 155 are all 5 |
| ordinary degree histogram | degree 5: 21; degree 6: 303; degree 7: 13 |
| Euler / charge | 2 / 12 |

Every final vertex link is one cycle, every edge has incidence two, the mesh
remains mixed-level, and both vertex and face counts are reduced. This closes
the PF-W2 combinatorial topology gate only. Coordinates are unchanged, so the
best known angle range remains `38.551143486745–81.453074281139°`; PR58 must
transfer that witness and run the strict geometry gate.
