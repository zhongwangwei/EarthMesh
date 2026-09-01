# Lifted-N12 plan-native band domains

PR103 builds W2 band topology directly from `FaceBandPlan.labels` and the
declared interface edge sets. It does not construct `CoupledAnnulus`, sectors,
or full-polygon families.

## Fixed-prefix result

- CEC states: 16,384
- essential cycles/plans: 6,838
- plans successfully built: 6,838
- annular bands: 13,676 (two per plan)
- band topology errors: 0
- band 0 contracted coarse boundaries: 6,838

The first canonical plan records 32 source coarse-side edges contracted to 16
topology edges. Each contracted edge owns a deterministic one- or two-edge
source path, and the paths exactly partition the source boundary. The shared
internal and fine source boundaries contain 52 and 56 edges respectively.

All bands have `chi = 0` and two source boundary cycles. This passes the PR103
gate and permits the TransitionCell V3 architecture work. CEC remains
`CycleSearchIncomplete`; no downstream topology, geometry, shard restoration,
product artifact, or gate change is claimed.

Evidence: `tests/fixtures/n12_plan_band_domain.json`. Taskbook SHA-256:
`cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e`.
