# Alpha6 validation-gate governance

PR122 updates the decision matrix after the PR117 topology closure, PR118
geometry probe, and PR121 CEC resume.

```text
N12-Lifted-N6        ResearchTopologyClosed
N12-Interior-Control ResearchExactNoSolution (negative capacity control)
lifted geometry      ContinuousSearchIncomplete
CEC resume           49/49 input shards resumed; 1,233 continuations remain
decision             ContinuousGeometryBlocked
```

The Interior fixture is deliberately capacity-limited. Its exact no-solution
result remains a control result and no longer requires a positive topology
witness before the representative Lifted fixture can advance. The Lifted
fixture nevertheless cannot pass validation: its closed PR117 topology reaches
only `1.337009876734°–173.470265136292°` in the bounded PR118 geometry search.
The PR121 resume also remains incomplete and must not be called no-solution.

Consequently:

- topology search is no longer the current validation blocker.
- continuous strict geometry is the current blocker.
- the N24/N40 research staircase remains locked.
- NXP80 remains locked.
- no product gate or output status changes.

The older mixed-topology reference remains
`39.278499430048°–80.721500570507°`, but it is not evidence for the closed
PR117 topology. Strict `40.2°–79.8°` is not certified.
The governance record is frozen in
`tests/fixtures/n12_validation_gate_governance.json`.
