# Lifted-N12 downstream uniform-rejection audit

PR98 replays the PR95 budget without changing the solver and records every
downstream rejection by stage, reason, and first canonical cycle.

## Frozen result

- adapter contract: V1 legacy coupled-annulus extraction
- cycle budget: 16,384 unique states
- full-polygon budget: 4,096 states per cycle
- essential cycles: 6,838
- `DomainAdapter` rejects: 6,838
- distinct reject reasons: 1
- downstream topology states: 0

The one reason is the legacy requirement that `inner_guard` be a single closed
2-regular cycle. A plan-independent preflight reproduces it before any
candidate cycle is supplied and classifies the stage as `GeometryGuardOnly`.
The first rejected cycle is frozen in the JSON evidence rather than using a
last-reason sample as a distribution.

The refined governance state is therefore
`DownstreamAnnulusContractBlocked`, with CEC incompleteness secondary. This is
not 6,838 topology no-go results.

Evidence: `tests/fixtures/n12_downstream_reject_audit.json`. The taskbook
SHA-256 is
`63215a9043f5aa87092a78b2910d0c779da3c10e2c749bb4e11d5b0e5b207c5d`.

No geometry or product artifact is produced and no gate changes.
