# Alpha6 Frozen N6 CEC reclassification

PR94 reruns the exact Alpha5 retained-core target definition and classifies its
659 unknown family attempts with CEC proof mode and typed full-polygon results.
This is research evidence only; it does not change the safe fallback or any
product gate.

## Frozen protocol

- retained-core connected candidates: 154
- corridor families: F0–F5
- total family attempts: 924
- legacy face-label bound per family: 16,384 states
- CEC bound per targeted family: 16,384 unique propagated states
- downstream full-polygon bound per cycle: 4,096 states
- taskbook SHA-256:
  `b327b6afdf199abfaf1a77f4e403ef296e4f5bd2483d855b360c08152a10ae53`

The runner exactly reproduces the Alpha5 partition:

```text
265 legacy exact-no-solution
384 legacy closed face bands but unresolved downstream
275 legacy face-label budget exhausted
659 targeted unknowns
```

## CEC result

| Outcome | Families |
| --- | ---: |
| Closed | 0 |
| ExactNoSolution | 43 |
| CycleSearchIncomplete | 553 |
| DownstreamSearchIncomplete | 63 |
| Total | 659 |

The previous 659 unknowns therefore shrink to 616, but none becomes a closed
mixed topology. The 43 exact conclusions are scoped to their complete canonical
cycle problem and downstream contract. The other two incomplete classes remain
separate: 553 stopped at a CEC frontier checkpoint, while 63 exhausted their
cycle family but encountered at least one incomplete or invalid downstream
topology evaluation.

Aggregate search telemetry:

```text
unique propagated states = 9,313,694
explicit decisions        = 9,885,163
closed cycles evaluated   = 3,886,784
exact full-key reuses      = 0
```

PR89 already established that all 924 complete ProblemKeys are unique, so zero
exact duplicate reuse is expected. The first full run cached every large
downstream evidence/trial despite that zero-hit domain and grew to about 13 GB;
it was aborted. The accepted run disables that per-cycle cache for this runner
while retaining PR92's cache API and exact-key outcome reuse. Peak resident
memory stayed below roughly 1 GB. The release probe completed in 1,896 seconds
after compilation.

The complete 659-record evidence is frozen in
`tests/fixtures/frozen_n6_cec_closure.json`. Every incomplete CEC record carries
a typed proof frontier suitable for resume in memory; persistence remains a CLI
responsibility.

## Scientific consequence

No closed topology means geometry was not attempted, as required. This result
does **not** prove Frozen N6 mixed-mesh nonexistence and does not improve the
current best mixed angle range of
`39.278499430048°–80.721500570507°`. The strict internal target remains
`40.2°–79.8°`, so Frozen N6 remains blocked at topology/search rather than being
declared geometrically impossible.
