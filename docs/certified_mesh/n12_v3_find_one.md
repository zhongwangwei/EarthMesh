# Lifted-N12 V3 balanced-annulus find-one

PR109 materializes a deterministic balanced-strip subset of each general
annular transition-cell family and sends it through the V3 global merge. The
subset is research-only: exhausting it is `SearchIncomplete`, never a proof
that the complete CSAE family has no solution.

## Frozen 16,384-state result

| Measure | Result |
| --- | ---: |
| essential cycles examined | 6,838 |
| V3 domains built | 6,838 |
| balanced candidates examined | 217,484,556 |
| concrete annular topologies | 870,400 |
| global topology combinations entered | 6,838 |
| bounded anchor-ear states | 1,750,528 |
| topology closed | 0 |
| remaining CEC shards | 49 |

Every cycle reached concrete topology and then exhausted the per-topology
256-state anchor-ear budget on its first global combination. The result is
therefore `DownstreamSearchIncomplete`, not exact no-solution. Restoring the
49 CEC shards would add more cycles without resolving the measured downstream
anchor-ear bottleneck, so PR109 leaves them untouched.

The frozen N6 oracle does close with the same balanced subset, proving that
the enumerator and V3 merge path are live. Lifted-N12 geometry was not run;
the 40°--80° product claim and N24/N40/NXP80 gates remain locked.
