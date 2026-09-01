# Alpha6 validation-gate governance

PR97 applies the taskbook decision matrix to the frozen PR94–PR96 evidence.

```text
N12-Lifted-N6       ResearchCycleSearchIncomplete
N12-Interior-Control ResearchExactNoSolution
geometry             not attempted
decision             TopologySolverBlocked
```

The Lifted fixture still has 49 resumable CEC shards, so the result cannot be
called a topology no-go. The Interior result is exact only inside its frozen
canonical family. No closed N12 topology exists for the geometry protocol.

Consequently:

- N6 remains the existing safety/existence gate; it is not reclassified as a
  mere stress fixture.
- N12 is not a mixed-existence gate.
- the N24/N40 research staircase remains locked.
- NXP80 remains locked.
- no product gate or output status changes.

The best known mixed range remains
`39.278499430048°–80.721500570507°`; strict `40.2°–79.8°` is not certified.
The governance record is frozen in
`tests/fixtures/n12_validation_gate_governance.json`.
