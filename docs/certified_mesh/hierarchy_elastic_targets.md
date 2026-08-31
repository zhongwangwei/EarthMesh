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


## PR48 active tangent trust evidence

PR48 adds an explicit experiment path for an active-constraint spherical tangent
trust solver. It keeps the PR46 C target mode (`HierarchyEdgeAreaDegree`), the
PR47 deterministic start set, the same topology order, and the same 500x64
budget. The active solve assembles explicit tangent rows `(residual r, projected
Jacobian J, weight W)`, solves deterministic damped normal equations
`(J^T W J + lambda I) delta = -J^T W r`, clamps each movable vertex by its trust
radius, then applies `Exp_x(delta)`. The ordinary PR46/PR47 serializers are
unchanged; this probe reports `solver_mode = ActiveTangentTrust`.

500x64 release comparison, two runs byte-identical:

| start | outcome | attempts | phase counts | best angle range | best signed margin | last angle range | last signed margin |
|---|---|---:|---|---:|---:|---:|---:|
| `MaterializedSource` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.838636866483--92.242446998773 | -12.442446998773 | 27.464343730108--92.540658996499 | -12.740658996499 |
| `HierarchySpringEquilibrium` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.799440175167--92.352813339354 | -12.552813339354 | 27.274794316152--115.534844306902 | -35.734844306902 |
| `RingScaleInterpolation` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.214145946639--92.645476907821 | -12.985854053361 | 27.214145946639--92.645476907821 | -12.985854053361 |
| `DegreeAngleEquilibrium` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.359215515607--92.263380411133 | -12.840784484393 | 26.897879190503--93.030404757847 | -13.302120809497 |
| `SignedNormalPlus` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.395788550308--92.578326257591 | -12.804211449692 | 23.547674393253--99.343006914190 | -19.543006914190 |
| `SignedNormalMinus` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.484757584112--92.178664517782 | -12.715242415888 | 27.294692354321--91.833619206947 | -12.905307645679 |

Best PR48 start: `MaterializedSource`, margin `-12.442446998773`.
This improves the PR46 C baseline by 2.114006015995 degrees, but still misses
the 40.2--79.8 internal angle window. The result remains
`ContinuousSearchIncomplete` in `AngleFeasibility`; it is not a topology no-go
and not a continuous infeasibility proof.

## PR49 domain ladder evidence

PR49 keeps the PR46 C target mode (`HierarchyEdgeAreaDegree`), PR48
`ActiveTangentTrust`, topology order, start (`MaterializedSource`), and 500x64
budget fixed while changing only the movable ordinary-vertex domain:
`CurrentAnnulus`, `PlusOneOrdinaryRing`, and `PlusTwoOrdinaryRings`. The original
twelve icosahedron anchors and explicit physical fixed sources remain fixed.
Current guard vertices are not permanently fixed; each domain expansion rebuilds
the guard/fixed closure after choosing the movable set.

500x64 release comparison, two runs byte-identical. `CurrentAnnulus` reproduces
the PR48 baseline exactly (`MaterializedSource`, best margin
`-12.442446998773`, 27.838636866483--92.242446998773 degrees), which locks the
legacy closure semantics while testing wider domains.

| domain | outcome | attempts | phase counts | best angle range | best signed margin | last angle range | last signed margin |
|---|---|---:|---|---:|---:|---:|---:|
| `CurrentAnnulus` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 27.838636866483--92.242446998773 | -12.442446998773 | 27.464343730108--92.540658996499 | -12.740658996499 |
| `PlusOneOrdinaryRing` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 37.340837475907--83.652760215259 | -3.852760215259 | 35.425253727729--86.277987382141 | -6.477987382141 |
| `PlusTwoOrdinaryRings` | `ContinuousSearchIncomplete` | 16 | `AngleFeasibility: 16` | 33.318463982635--90.088795086042 | -10.288795086042 | 10.749538016364--150.970317722384 | -71.170317722384 |

Best PR49 domain: `PlusOneOrdinaryRing`, margin `-3.852760215259`. This is a
large bounded-search improvement over PR48 `CurrentAnnulus`, but it still misses
the internal 40.2--79.8 angle window. The +2-ring finite 64-iteration result is
worse than +1-ring; this does not contradict the complete-solver monotonicity
claim for the mathematical feasible domain because the bounded local solver is
not complete and may stall differently in a wider domain.

Per-domain best/last failure JSON now carries minimal real failure diagnostics,
not placeholder telemetry: reference-to-candidate movement distribution
(`count`, `min`, `p50`, `p90`, `max`, `sum`), worst violating triangle face-graph
distance to a fixed guard face when reachable, and active-constraint boundary
face numerator/denominator/ratio. This is final/best failure evidence only; it
is not a stored full accepted-step trajectory. The result remains
`ContinuousSearchIncomplete` in `AngleFeasibility`; it is not a topology no-go
and not a continuous infeasibility proof.

## PR50 scoped interval evidence

PR50 persists the actual best PR49 failure mesh and constructs a nonzero
two-coordinate tangent box around each of its 76 movable +1-ring vertices.
Every point is mapped through the spherical exponential map. Outward interval
bounds cover all 666 active faces, positive orientation, angle cosines, signed
margin, and a necessary non-crossing test. Search exhaustion remains `Unknown`.

The deterministic release probe used the frozen 500x64 numerical search,
`MaterializedSource`, `HierarchyEdgeAreaDegree`, `ActiveTangentTrust`,
`PlusOneOrdinaryRing`, a per-coordinate trust radius of `1e-7` radians, and one
interval box. Two runs produced byte-identical JSON.

| numerical angle range | numerical margin | movable vertices | trust radius | interval outcome | boxes | interval upper margin |
|---:|---:|---:|---:|---|---:|---:|
| 37.340837475907--83.652760215259 | -3.852760215259 | 76 | 1e-7 rad | `CertifiedInfeasibleWithinDomain` | 1 | -3.851707976720 |

This proves only that the named topology, fixed/movable assignment, and the
specified local trust box around the numerical witness contain no strict angle
witness. It does **not** prove that the complete +1-ring coordinate domain,
another topology, a wider trust domain, or helper-vertex topology is
infeasible. No 40.2--79.8-degree witness was found, so the PR51 publication gate
and NXP80 remain blocked.
