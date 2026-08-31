# Frozen N6 geometry baseline after PR44

PR45 freezes the reproducible evidence for the Frozen N6 strict mixed fixture.
It does not change the geometry solver.

## Current factual baseline

- Topology: closed for the full-polygon mixed fixture after PR44.
- Geometry: not certified yet.
- Internal solver window: 40.2--79.8 degrees.
- Publication window: 40--80 degrees.
- Current solver mode: `FiniteDifferenceElastic` with `TrialReference` targets.
- Current start set: `MaterializedSource` only.
- Current domain: `CurrentAnnulus`.
- NXP80 remains blocked until Frozen N6 strict geometry is certified.

A bounded continuous search failure is reported as `ContinuousSearchIncomplete`.
It is not a topology no-go and not a proof of continuous infeasibility.

## Reproduction

```sh
EARTHMESH_FULL_POLYGON_STATES=500 \
EARTHMESH_CBER_ITERATIONS=64 \
EARTHMESH_GEOMETRY_START_SET=MaterializedSource \
EARTHMESH_GEOMETRY_JSON=/tmp/n6-500x64.json \
cargo test --release \
  -p earthmesh_refine_certified \
  --test full_polygon_merge \
  frozen_n6_parameterized_geometry_probe \
  -- --ignored --exact --nocapture
```

The JSON records both `last_failure` and `best_failure`. Use
`best_signed_margin_deg` to compare bounded geometry searches; do not compare
only the final printed angle range.


## Frozen 500x64 result

Two release runs with the reproduction command produced byte-identical JSON.
The outcome is `ContinuousSearchIncomplete`.

| field | value |
|---|---:|
| topology limit | 500 |
| elastic iterations | 64 |
| topology candidates closed | 72 |
| geometry candidates attempted | 16 |
| phase counts | `AngleFeasibility: 16` |
| best global angle range | 27.198463901923--94.632376343608 degrees |
| best signed margin | -14.832376343608 degrees |
| last global angle range | 24.041664860656--95.723625877804 degrees |
| last signed margin | -16.158335139344 degrees |
| best start | `MaterializedSource` |

`last_failure` is the final candidate attempted. `best_failure` is selected by
fewer known orientation/crossing failures, larger signed angle margin, fewer
known Delaunay/Voronoi failures, then topology/start order. In this run all
recorded failures stopped in `AngleFeasibility`. Orientation, crossing,
Delaunay, and Voronoi counts are `null` because PR45 does not add true per-state
instrumentation; they are not zero claims.

## Typed outcomes

- `Certified`: strict Frozen N6 geometry certified.
- `ContinuousSearchIncomplete`: bounded continuous geometry search ended without
  a certificate.
- `RequiresDifferentTopology`: topology-family evidence, not a generic result
  for angle solver exhaustion.
- `InvalidPatch`: invalid fixture or patch evidence.

## Known limitation

PR45 only freezes evidence and documentation. It intentionally does not add
hierarchy-derived targets, multistart, active constraints, domain laddering, or
interval proof.
