# Lifted-N12 V3 prefix replay

PR108 replays the frozen 16,384-state CEC prefix through plan-native V3
annular reachability. It does not restore any of the 49 CEC frontier shards.

## Frozen result

| Measure | Result |
| --- | ---: |
| CEC unique states | 16,384 |
| essential cycles | 6,838 |
| V3 domains built | 6,838 |
| cycles reaching annular reachability | 6,838 |
| annular reachability exact rejects | 0 |
| annular reachability incomplete | 6,838 |
| downstream invalid | 0 |
| topology closed | 0 |
| remaining resumable shards | 49 |

Every cycle enters the general annular signature DP. No cycle calls legacy
sectorization and no cycle fails in `BandDomain`. With 4,096 signature states
per cycle, all 6,838 stop as typed `AnnularSignatureSearchIncomplete`:

- root bridges considered: 13,676;
- signature states: 56,016,896;
- early degree-cap prunes: 59,435,896;
- downstream incomplete: 6,838.

The three PR108 hard gates therefore pass:

1. no domain-adapter or legacy sectorization rejection;
2. at least one cycle reaches annular reachability (all 6,838 do);
3. meaningful downstream evidence exists (`downstream_incomplete=6,838`).

This result proves the new downstream path is active, not that a Lifted-N12
topology exists. Concrete topology count and geometry attempts remain zero.
PR109 may resume the 49 CEC shards or run a more targeted find-one search, but
the 40°--80° product claim and higher-scale gates remain locked.
