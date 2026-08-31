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
