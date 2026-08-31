# Hierarchy elastic targets contract

This is the PR46 gate contract, not an implementation claim.

Future elastic target work must derive edge, area, and degree-angle targets from
hierarchy levels instead of treating failed trial geometry as the only target.
Cross-level edge targets should use geometric interpolation between level
scales. Invalid reference Voronoi areas must not block the angle phase.

Go requires deterministic A/B evidence against the PR45 Frozen N6 baseline:

- same topology order;
- same start set;
- same iteration budget;
- improved `best_signed_margin_deg`, or no default switch;
- no regression in orientation/crossing evidence;
- no regression for already certified simple fixtures.

## PR46 bounded A/B/C evidence

The 500x64 comparison probe is deterministic: two release runs produced
byte-identical JSON.

| arm | outcome | attempts | phase counts | best angle range | best signed margin | last angle range | last signed margin |
|---|---|---:|---|---:|---:|---:|---:|
| A `TrialReference` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.198463901923--94.632376343608 | -14.832376343608 | 24.041664860656--95.723625877804 | -16.158335139344 |
| B `HierarchyEdge` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.175880100685--94.356463865148 | -14.556463865148 | 24.000189278313--95.452426861064 | -16.199810721687 |
| C `HierarchyEdgeAreaDegree` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.175763981707--94.356453014768 | -14.556453014768 | 24.000134741903--95.452491201711 | -16.199865258097 |

C is the best PR46 arm by `best_signed_margin_deg`, improving the PR45 baseline
by 0.275923328840 degrees. This is a solver-recovery improvement, not a strict
certificate. All three arms still stop in `AngleFeasibility`; NXP80 remains
blocked.

The existing default `solve_full_polygon_merge_free_interface_cber` remains
`TrialReference` because that API does not carry source-level evidence. Callers
with truthful hierarchy levels can use the explicit target-mode entry point.

## PR47 deterministic start evidence

PR47 adds an explicit experiment path for deterministic geometry starts using the
PR46 C target mode (`HierarchyEdgeAreaDegree`). The ordinary PR46 entry point is
kept on the previous MaterializedSource/energy path because the PR47 margin-start
experiment did not strictly improve the PR46 C baseline.

500x64 release comparison, two runs byte-identical:

| start | outcome | attempts | phase counts | best angle range | best signed margin | last angle range | last signed margin |
|---|---|---:|---|---:|---:|---:|---:|
| `MaterializedSource` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 10.749066796838--109.387948419340 | -29.587948419340 | 7.558601197972--112.489289203794 | -32.689289203794 |
| `HierarchySpringEquilibrium` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 24.436419106635--95.874725710625 | -16.074725710625 | 22.971299427781--99.028301504132 | -19.228301504132 |
| `RingScaleInterpolation` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 22.877055305236--101.674159373161 | -21.874159373161 | 15.973615026109--107.215622703793 | -27.415622703793 |
| `DegreeAngleEquilibrium` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 9.008621258011--111.024637107932 | -31.224637107932 | 5.782981754393--114.175002680903 | -34.417018245607 |
| `SignedNormalPlus` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 8.411864079377--111.542746694046 | -31.788135920623 | 5.601496455435--114.688578138123 | -34.888578138123 |
| `SignedNormalMinus` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 19.316522088976--101.844982329437 | -22.044982329437 | 12.004295572221--111.742237537669 | -31.942237537669 |

Best PR47 start: `HierarchySpringEquilibrium`, margin `-16.074725710625`.
This is worse than the PR46 C baseline margin `-14.556453014768`, so PR47 does
not switch the default. The result remains a continuous-search incomplete
failure, not a topology no-go.
