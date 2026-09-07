# CoastalOcean sea-ratio performance check (2026-09-07)

The new `sea_ratio` threshold is independent of landcover class refinement and
domain masking. It uses the same LandType source; the default is disabled with
threshold 0.05 (strictly 5%–95% ocean share when enabled).

## Changes measured

- Select retirement candidates with one incidence sweep instead of repeated
  whole-mesh scans.
- Prepare spherical polygons once per remap, preserving scalar validation,
  overlap weights and certificate ordering.
- Build and validate adjacency from sorted flat edge arrays. Use hash sets only
  where topology gates need membership/count, never candidate traversal order.

No quality gate, search budget, candidate ordering or refinement demand was relaxed.

## Full NXP80 result

Same saved CoastalOcean/100 km project, identical project bytes, unchanged
`input/landtype_igbp_update.nc` (240 samples/degree), same macOS machine:

| Run | Wall time | Change from previous run |
| --- | ---: | ---: |
| Before prepared remapping | 53m37s | — |
| Prepared remapping | 49m33s | -7.6% |
| Flat edge collections and membership sets | 32m46s | -33.9% |

The last run saves 16m47s versus the preceding run and 38.9% versus the first.
Gridfile, certificate and pre-export remap CSV were byte-identical across the
last two runs. Quality metrics were unchanged, including the existing area-CV
warning. Only output-path metadata and elapsed-time records differed.

Gridfile SHA-256:
`88773a99f428f8d3dabdb9fb48c790948e716ab9c490a6034246941865f7e1e0`

Search counters remained 908 topology states, 3,609 elastic iterations and
953,859 interval boxes; 59 components committed, 53 promoted, none exhausted.
The published ocean mesh has 1,201,915 triangles and 628,042 vertices.

Remaining measured component costs: topology search 808.404 s, preparation and
elastic adjustment 481.119 s, remapping 441.013 s. These are phase totals, not
additional costs on top of the overall construction timer. Peak RSS was
20.9 GiB (`/usr/bin/time -l`); the earlier run only has sampled memory observations.

## Verification and limits

Regression coverage includes old ordered-map adjacency/error-order oracles,
hard-gate first-error ordering, scalar/prepared remap equivalence, component
rollback/budget behavior, CLI project lowering and GUI criterion roundtrips.
Run `node scripts/check_gui_js.js`, affected Rust library tests, CLI
`cli_help`/`certified_hidden_cli` tests and Tauri command tests. For phase timing,
set `EARTHMESH_CMRC_TIMING=1` when running `earthmesh_cli --project <saved.yaml>`.

These are single before/after measurements of developer binaries, not repeated
controlled medians or a benchmark of an isolated release checkout. Unrelated
Red-Green drafts were excluded from publication; publication checks also run
without those drafts. Background load and caches can affect elapsed time.
The preceding GUI timer has integer-second resolution; the final run uses
a monotonic CLI wall clock. The NXP10 speedup (48.3%) was not extrapolated to NXP80.
