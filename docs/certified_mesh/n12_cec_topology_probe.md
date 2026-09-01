# Alpha6 N12 CEC topology probe

PR95 applies the canonical essential-cycle solver and the existing exact
full-polygon contract to both frozen N12 fixtures. This is research-only: it
does not run geometry, write a grid or ready marker, or change a product gate.

## Frozen limits

- CEC: 16,384 unique propagated states per fixture
- downstream full polygon: 4,096 states per cycle
- taskbook SHA-256:
  `b327b6afdf199abfaf1a77f4e403ef296e4f5bd2483d855b360c08152a10ae53`

## Result

| Fixture | Result | Unique states | Essential cycles | Detail |
| --- | --- | ---: | ---: | --- |
| N12-Lifted-N6 | `ResearchCycleSearchIncomplete` | 16,384 | 6,838 | 49 resumable shards remain |
| N12-Interior-Control | `ResearchExactNoSolution` | 1 | 0 | exact CEC family exhausted |

Every one of the 6,838 essential cycles reached for N12-Lifted-N6 was rejected
by the downstream evaluator as invalid before topology search. The recorded
reason is:

```text
inner_guard boundary is not a closed 2-regular cycle
```

The CEC frontier was not exhausted, so this is a solver-blocked result, not a
nonexistence proof. Neither fixture produced a closed full-polygon topology;
therefore degree/link/Euler/charge evidence is unavailable and the taskbook's
strict geometry protocol must not run.

The deterministic report is frozen at
`tests/fixtures/n12_cec_topology_probe.json`.

## Scientific consequence

PR95 supplies no new angle witness. The best known mixed witness remains
`39.278499430048°–80.721500570507°`, below the internal strict target
`40.2°–79.8°`. The N12 result does not justify N24, N40, or NXP80.
