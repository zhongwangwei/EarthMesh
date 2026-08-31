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

## PR51 nested-domain embedding evidence

PR51 removes the cold-start ambiguity from the PR49 +2-ring result. It persists
the +1 best mesh by source slot, expands only the movable classification and
guard to `PlusTwoOrdinaryRings`, and initializes the 32 newly released ordinary
vertices at their unchanged materialized coordinates. Connectivity, anchors,
and physical fixed sources are unchanged.

The frozen 500x64 gate reproduced the 76-vertex +1 witness and embedded it into
the 108-vertex +2 domain with bitwise-equal common coordinates:

| source range | embedded range | source margin | embedded margin | common | newly released | gate |
|---:|---:|---:|---:|---:|---:|---|
| 37.340837475907--83.652760215259 | 37.340837475907--83.652760215259 | -3.852760215259 | -3.852760215259 | 76 | 32 | `Go` |

This establishes only exact nested initialization. It does not improve the
angle range or certify 40.2--79.8 degrees; the next gate is monotone +2
continuation that must retain this seed as best-so-far.

## PR52 monotone +2 continuation evidence

PR52 continues the exact PR51 seed through a frozen 64-iteration schedule:
16 new-ring-only iterations, 24 deterministic alternating block iterations,
and 24 joint iterations. Each block uses `ActiveTangentTrust`; a stage that
regresses signed margin is discarded and the exact best coordinates are
restored. The 76 inherited vertices are partitioned into 28 old outer-ring and
48 interface vertices; the +2 halo contributes 32 newly released vertices.

| initial range | best range | initial margin | best margin | improvement | gate |
|---:|---:|---:|---:|---:|---|
| 37.340837475907--83.652760215259 | 38.551143486745--81.453074281139 | -3.852760215259 | -1.653074281139 | +2.199685934119 | `MaterialImprovement` |

The best-so-far margin is monotone and the +2 output cannot be worse than its
+1 seed. The strict window is still not met, so this is not a Frozen N6
certificate. The next gate is the worst-angle/effective-width atlas; simply
adding more iterations is not justified by the observed stage stalls.

## PR53 blocker atlas

The deterministic worst-100 atlas classifies the PR52 +2 witness as
`WidthDominated`:

| metric | +1 best | +2 warm best |
|---|---:|---:|
| within one graph step of a zero-width shared junction | 90% | 85% |
| adjacent to a pentagon or shared junction | 91% | 88% |
| near the fixed guard | 52% | 31% |
| contains a long same-chain/full-polygon diagonal | 8% | 9% |

The current W2 stratification has two logical adjacent-trace pairs. They share
6 and 8 vertices respectively, so both minimum face-strip widths and both
normalized minimum separations were conservatively reported as zero. PR55
later showed that none of these 14 shared vertices is a rotation-aware
`TruePinch`; see the correction below.

## PR55 W3/W4 planning result

The planning-only search widens the transition parent set, sacrifices inner
core parents, retries up to four outward parent rings, re-extracts every nested
trace family that remains valid, and checks every deterministic subsequence.
It does not run full-polygon merge or CBER.

Frozen N6 returns the required typed stop result, `InsufficientAnnulusWidth`:

| mode | best effective bands | adjacent shared vertices | adjacent shared edges | result |
|---|---:|---:|---:|---|
| legacy W2 | 0 | 14 | 0 | diagnostic only |
| W3 | 0 | 24 | 0 | `InsufficientAnnulusWidth` |
| local W4 | — | — | — | not evaluated: global W3 prerequisite failed |

W4 is not treated as a global four-band requirement. Its planner first gates a
three-band ordinary-region subsequence, then measures a fourth band only in
pentagon, seam, and pinch-centred trace slices. The Frozen N6 W3 prerequisite
fails, so the local W4 gate is deliberately not evaluated.

Widening far enough to expose more nominal traces repeatedly puts original
icosahedron anchors 29 or 2 strictly inside the annulus, which the certified
extractor rejects. Shallower valid annuli still have adjacent traces touching
at source vertices. Therefore nominal extra traces cannot be reported as
positive-width bands. This result is scoped to parent-layer extraction plus
four outward rings; it is not a global proof against arbitrary new
anchor-exclusion cycle surgery.

## PR55 rotation-aware width correction

The source-vertex rotation audit distinguishes an actual zero-face wedge from
a shared primal vertex with intervening band faces:

| trace family | shared occurrences | unique vertices | true pinches | one-face wedges | anchor junctions |
|---|---:|---:|---:|---:|---:|
| legacy W2 | 14 | 14 | 0 | 12 | 2 |
| PR54 W3 candidate | 24 | 20 | 0 | 20 | 4 |

Every ordinary occurrence has two separated one-face wedges in its full
rotation; classification uses the minimum contiguous wedge width. Four PR54
occurrences repeat vertices touched by two adjacent trace pairs, which is why
24 adjacent-pair occurrences correspond to 20 unique vertices.

Consequently the PR53 `85%` value means “near a nominal shared junction,” not
“near a proven zero-width pinch.” With no `TruePinch` in the legacy W2 audit,
the corrected worst-100 true-pinch fraction is `0%`. This invalidates the old
zero-width causal label but does not make the current topology angle-feasible.
The next gate is the exact source-face PF-W2 construction defined in
[face_band_contract.md](face_band_contract.md); no topology merge or CBER was
run in this audit.

## PR56 exact PF-W2 face-band result

The first finite ladder step, `F0CurrentTransitionFaces`, closes without
expanding the face complex. The exact deterministic search labels all 88
transition faces in 45 states: band counts are 36 and 52, and their boundary is
one 20-edge/20-vertex simple internal cycle. The interface shares no vertex
with either fixed boundary, both bands are connected annular strips, all four
original anchors lie inside a single band, and no cap, corridor, or coarse-core
sacrifice is used. This is the minimum-cost PF-W2 planning witness; it is not
yet a full-polygon topology or angle certificate. The next gate is PR57
topology closure derived from this interface, still before CBER.
