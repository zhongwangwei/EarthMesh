# CMRC requirement-layer provenance

CMRC now records `requirement_layers` in both `certified_certificate.json` and
`certified_resources.json`. This is **diagnostic separation, not a relaxation of
requirements**. The policy remains `effective_raster_remains_hard`: construction,
final-cell certification, scheduling, defaults and all legacy residual fields
continue to use their previous inputs.

| Layer | Population | Meaning |
| --- | --- | --- |
| `raw_source_raster` | Source raster samples | Canonical specified/calculated region footprints, quantized before gradient limiting. Not an exact analytic-region coverage certificate. |
| `effective_source_raster` | The same raster samples | Composed and gradient-limited requirements, including conservative global bounds. Still hard-certified. |
| `graph_scheduling_target` | Initial mother-grid Voronoi cells | Effective raster projected by cell overlap, then graph-graded for scheduling. Not the original source histogram or the final cell count. |

Raw decomposition is available for region-only and no-source runs. If threshold
or hydro sources contribute, its status is `unavailable_threshold_or_hydro`, with
null raw histogram and null expansion count. A region-only subset is never
reported as the full raw requirement of such a run. Their effective requirements
remain intact.

`raised_samples_over_raw` counts effective levels above the raw levels on the
same raster. It is not an extra-mesh-cell count or a causal attribution to
smoothing alone. `conservative_global_bound=true` separately identifies the
existing sub-raster-region fallback; do not confuse a zero raw sampled footprint
with absence of a required region. `graph_scheduling_target.status=not_applied`
and a null histogram mean this run did not use mixed graph grading, not zero
graph requirements.

## Checks and remaining work

- HField regression locks the A3 level histogram and a pre-change continuous
  fingerprint rounded to 1 mm, while comparing every continuous value bitwise
  against the shared raw builder followed by the existing limiter.
- CLI regression covers no-source, region-only graph grading, threshold/hydro
  unavailable provenance, and both specified/calculated sub-raster bounds.
- A3 acceptance artifacts: `.omx/artifacts/cmrc-requirement-layers-20260909/`
  in the primary checkout. A3 remains 63,810 cells; gridfile, MPAS, graph, remap
  and ready bytes match the frozen five-ring baseline. Legacy certificate fields
  are unchanged. Independent MPAS/topology/circle audits pass, with no source-cap
  cell below level 2. The measured 12.41 s is one run, not a speed comparison.

The source raster has 5,298 raw level-2 samples versus 6,448 after smoothing,
plus 2,920 new level-1 samples. The graph histogram instead describes 655,362
initial mother Voronoi cells (20,765 at level 2, 8,854 at level 1); its population
must not be compared directly with the 259,200 raster samples.

This step does not reduce cell counts or promote an alternate g/ring setting.
Making transition preferences optional still requires separate final-cell source
coverage and hex-quality acceptance; a smaller mesh alone is not sufficient.
