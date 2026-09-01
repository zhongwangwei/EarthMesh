# Alpha6 CEC rollback find-one solver

PR91 adds a research-only find-one solver for the canonical W2 essential-cycle
problem. It changes neither the Alpha5 fallback nor any Alpha6 product gate.
Even a fully exhausted PR91 run remains `CycleSearchIncomplete`; exact
no-solution authority is reserved for the proof/checkpoint mode in PR92.

## State and propagation

Each candidate edge uses two packed bits (`Undecided`, `Excluded`, or
`Included`). A reversible trail restores edge decisions, selected/undecided
vertex degrees, selected-edge count, and seam parity. A rollback DSU stores
selected path components and their vertex/edge counts; component endpoints are
derived from the trailed degree arrays rather than duplicated in the DSU.

Propagation enforces final degree `0` or `2`:

- degree 2 excludes every remaining incident edge;
- degree 1 with one option includes it;
- degree 1 with no option rejects;
- degree 0 with one option excludes it;
- an `OnSingleInterface` anchor must retain and eventually select exactly two
  incident edges.

At each fixed point the solver also rejects a closed cycle beside another
selected component, an open path that can no longer close through undecided
edges, selected components that can no longer merge, and a coarse-to-fine dual
path made entirely from permanently excluded or non-candidate primal edges.

Branch order is deterministic: open-path endpoints, unsatisfied interface
anchors, dual-seam edges, distance to the middle dual potential, constraint
degree, then canonical edge ID. Inclusion is tried first. Branch rollback does
not clone the full mutable state; a packed state copy is retained only once per
propagated node for exact duplicate detection and unique-state accounting.

## Typed downstream result

`FaceBandPlanEvaluator` returns one of four distinct results: accepted,
exactly rejected, search incomplete, or invalid. The supplied
`FullPolygonPlanEvaluator` maps the existing full-polygon outcomes without
collapsing incomplete/invalid evidence into a topology no-go.

## Frozen N6 find-one gate

The Frozen N6 F0 W2 problem closes deterministically with:

| Measure | Legacy labels | CEC find-one |
| --- | ---: | ---: |
| Raw/unique search states | 45 raw | 4 unique |
| Explicit branch decisions | n/a | 3 |
| Forced propagation events | n/a | 21 |
| Forced includes / excludes | n/a | 17 / 4 |
| Peak selected edges | n/a | 20 |
| Peak rollback records | n/a | 192 |
| Closed / essential cycles evaluated | n/a | 1 / 1 |
| Downstream topology states | 31 | 31 |

The debug-profile integration test completed this gate in roughly 0.3 seconds
on the development machine. Runtime is telemetry, not a correctness threshold;
the stable performance gate is that four unique propagated states are fewer
than the legacy solver's 45 raw states. The emitted evidence also records
elapsed microseconds and propagation events per explicit decision.
