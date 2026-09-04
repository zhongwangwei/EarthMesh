# N6 legacy topology evidence

PR #34 adds a proof-only evidence surface for the current per-parent CMRC
transition topology family. It does not change production coarsening.

The evidence is emitted as machine-readable JSON by:

```rust
analyze_legacy_transition_family(
    &source,
    &component,
    topology_budget,
    interval_box_budget_per_topology,
)
    .to_machine_readable_json()
```

The frozen fixture is defined in
[`n6_transition_fixture.md`](n6_transition_fixture.md). The regression test
requires the exact hidden fixture component shape:

```text
source_subdivision = 6
core_parent_count = 10
transition_parent_count = 22
```

## Required conclusion semantics

The family-level outcome must be exactly one of:

```text
CertifiedFeasible
CertifiedInfeasible
UnknownBudgetExhausted
```

`UnknownBudgetExhausted` is an honest terminal-for-this-run result. It must not
be promoted to `CertifiedInfeasible`, and the JSON must not contain
`ProvenInfeasible`.

The frozen PR #34 run uses a topology budget of 2,000, the production halo
depth of two (`maximum_transition_rings - initial_transition_rings`), and one
interval box per emitted topology. It deterministically emits 493 hard-gate
clean legacy topologies and concludes `UnknownBudgetExhausted`: the global
continuous sphere superset is not closed by that interval budget. This is not
a no-go theorem.

```text
topology_family_closed = true
family_topology_count = 493
interval_boxes = 493
best sampled source-geometry margin = -41.917474411461 degrees
interval upper margin = 19.8 degrees
outcome = UnknownBudgetExhausted
```

That `493` count is the historical PR #34 snapshot. The Alpha7 topology gate
also enforces the protected-icosahedron degree contract before emitting a
candidate, so the current regression retains 188 hard-gate-clean topologies
and spends 188 one-box interval checks while preserving the same
`UnknownBudgetExhausted` conclusion.

The numerical margin above is the best emitted source-geometry sample, not a
replacement for PR #33's independently optimized CBER diagnostic. The outward
interval proof starts from `[-1,1]^3` per movable vertex, a conservative
superset of the unit sphere. It may prove infeasibility if every box is pruned;
otherwise it must return unknown.

## PR #34 decision record

- **Constraint:** preserve the strict internal `40.2°–79.8°` window and leave
  production coarsening unchanged.
- **Current-family assumption removed:** numerical CBER failure is no longer
  treated as a mathematical no-go.
- **Mathematical obligation:** feasible needs the full internal certificate;
  infeasible needs complete outward-interval pruning; every other run is
  unknown.
- **Rejected:** larger production budgets and relaxed angle thresholds.
- **Tests:** exact fixture shape, three proof outcomes, no false infeasible,
  deterministic legacy-family evidence.
- **Known limitation:** the one-box-per-topology run does not close the
  continuous domain.
- **Next stage gate:** PR #35 annulus/guard extraction; NXP80 remains blocked
  until the strict N6 gate in PR #38.

## Evidence fields

Each run records:

- topology budget;
- enumerated legacy topology count;
- fixed and movable vertex counts per topology;
- guard face count;
- source-geometry angle range;
- best numerical strict-window margin;
- interval upper margin when available;
- interval box count;
- per-topology outcome;
- family-level outcome and conclusion.

## Local check

```sh
cargo test -p earthmesh_refine_certified --test n6_transition_no_go -- --nocapture
```
