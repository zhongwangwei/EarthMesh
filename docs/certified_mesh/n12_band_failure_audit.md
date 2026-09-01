# Lifted-N12 general band-boundary audit

PR102 classifies every legacy `UnsupportedNonDiskBandComponent` without
changing the topology solver. The fixed 16,384-state replay audits both bands
for all 6,838 essential cycles.

## Result

- cycles audited: 6,838
- bands audited: 13,676
- valid topological annuli (`chi = 0`, two 2-regular boundary cycles): 13,676
- band topology contract failures: 0
- band 0: 6,838 `LowerTraceEdgeMismatch` results
- band 1: 6,838 `DirectConnectorCapacityMissing` results
- directed winding violations: none in the frozen examples

Band 0 has a valid 32-edge source boundary but the legacy trace uses the
16-edge contracted coarse boundary. Band 1 matches both boundary cycles, but
the thick band has only 5 of 52 lower vertices directly adjacent to its upper
cycle in the first canonical example. The old thin-strip connector assumption
therefore cannot represent it.

The PR102 conclusion is `LegalAnnuliLegacyRepresentationFailure` (case A), not
a CEC/FaceBandPlan contract failure. PR103 may build plan-native annular band
domains. No geometry, shard restoration, product output, or gate change occurs.

Evidence: `tests/fixtures/n12_band_failure_audit.json`. Taskbook SHA-256:
`cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e`.
