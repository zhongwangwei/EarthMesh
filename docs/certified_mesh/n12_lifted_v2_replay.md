# Lifted-N12 Adapter V2 fixed-prefix replay

PR101 replays the first 16,384 canonical essential-cycle states with the
TopologyDomain V2 adapter and the unchanged 4,096-state downstream budget.
The Frozen N6 V1/V2 oracle from PR100 remains the promotion prerequisite.

## Result

- essential cycles: 6,838
- legacy `inner_guard` rejects: 0
- `DomainAdapter` rejects: 0
- `StratifiedSectorization` rejects: 6,838
- reason: `UnsupportedNonDiskBandComponent { band_id: 0 }`
- downstream topology states: 0
- downstream exact rejects/incomplete/closed: 0/0/0
- CEC outcome: `ResearchCycleSearchIncomplete` with the existing resumable frontier

The old plan-independent geometry-guard bug is fixed. The new failure is the
plan-dependent stratified band decomposition, but no cycle reaches degree
reachability or full-polygon enumeration. This is not an exact topology
no-solution result.

The PR101 gate therefore fails. PR102 must not restore the 49 shards, geometry
must not run, and N24/N40/NXP80 remain locked. No product, ready marker, angle
contract, or validation gate changes.

Evidence: `tests/fixtures/n12_lifted_v2_replay.json`. The taskbook SHA-256 is
`63215a9043f5aa87092a78b2910d0c779da3c10e2c749bb4e11d5b0e5b207c5d`.
