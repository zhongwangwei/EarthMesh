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

## Frozen N6 PF-W2 geometry continuation

PR58 transfers the PR52 `PlusTwoOrdinaryRings` witness by source-vertex identity
onto every exact PF-W2 topology candidate. In the frozen run, 333 vertices
inherit their incumbent coordinates and 10 vertices retain safe materialized
coordinates. Original anchors and the rebuilt PF-W2 guard closure remain fixed.
Topology ordering uses the inherited coordinates, but exact-family membership
is unchanged.

The deterministic 500-topology-state, 64-iteration active-tangent-trust run
uses all six registered starts. It closes 447 topology candidates and starts
geometry on 182 of them. Its best PF-W2 range is
`37.462020125507–83.823954752680°` (signed margin `-4.023954752680°`) from
`HierarchySpringEquilibrium`. This is worse than the retained PR52 incumbent
`38.551143486745–81.453074281139°` (margin `-1.653074281139°`), so the
incumbent is not replaced. The strict `40.2–79.8°` gate remains unmet and the
pre-registered result is `NoImprovementEnterW3`, not a PF-W2 impossibility
proof.

## Frozen N6 exact W3 result

PR59 extends the same exact label solver to `b(f) in {0,1,2}` and adds
arc-consistent propagation for adjacent-face Lipschitz and incident-vertex
no-skip constraints. Source-face expansion is outward only; it never consumes
the non-empty coarse core. The registered ladder closes as follows:

| step | transition faces | result | branch states |
|---|---:|---|---:|
| F0 current corridor | 88 | `FamilyExhaustedNoSolution` | 0 |
| F1 +1 source-face ring | 116 | `FamilyExhaustedNoSolution` | 0 |
| F2 +2 source-face rings | 146 | `Closed` | 58 |

F0 and F1 require no branching because propagation empties a domain, which is
an exact contradiction rather than a budget result. The first closed plan has
band face counts `36/50/60` and two simple interfaces with `20/26` edges and
vertices. The interfaces are vertex-disjoint and avoid both fixed boundaries;
all vertex label spans are at most one and all four original anchors remain
inside one band. No coarse-core sacrifice, fine cap, or corridor is used.
PR59 establishes the W3 face-label plan only; topology and geometry remain the
PR60 gate.

## PR60 W3 anchor-policy escalation and topology closure

The first F2 plan leaves anchors 2 and 155 strictly inside the fine-side band.
The current full-polygon family triangulates trace-bounded polygons and cannot
retain a strict-interior source vertex, so materializing that plan would remove
both anchors. This is the registered failure condition for escalating from
`InteriorOfSingleBand` to `OnSingleInterface`; it is not an angle relaxation.

Re-solving F2 with anchors 2 and 155 on exactly one internal interface closes
in 722766 states. The resulting bands contain `36/52/58` faces, the interfaces
contain `20/28` edges, and the 56 topology sectors split `8/20/28` by band. The
global exact merge closes in 70 states with V/E/F `341/1017/678`. All four
affected original anchors have degree 5, ordinary degrees are `{5:25, 6:295,
7:17}`, every edge has incidence two, every vertex link is one cycle, Euler is
2, charge is 12, and the materialized mesh remains mixed-level.

The bounded PR60 geometry gate examines 150 topology states and seven geometry
candidates in each of the +1 and +2 coordinate domains, with 64 iterations and
all six deterministic starts. The best +1 result is
`0.016745201577--179.918785972103` degrees; the best +2 result is
`0.051225122152--179.845918790626` degrees. Both stop in `Untangle`, so this W3
family does not certify `40.2--79.8` degrees. Because failure is global rather
than confined to otherwise-feasible anchor neighbourhoods, local W4 is not
entered. This is a bounded solver result, not a proof that every continuous W3
embedding is impossible.
