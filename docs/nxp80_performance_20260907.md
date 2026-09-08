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

Recorded component phase totals were: `topology_search` 808.404 s, preparation
and elastic adjustment 481.119 s, remapping 441.013 s. **The old topology timer
also included previous failed-candidate work, so 808.404 s is not pure topology
search time.** These totals are not additional costs on top of the overall
construction timer. The overall wall-time comparison is unaffected. Peak RSS was
20.9 GiB (`/usr/bin/time -l`); the earlier run only has sampled memory observations.

## Immutable-source remap reuse follow-up

The follow-up keeps one lazily prepared source (Voronoi rings, polygon geometry
and cap index) per CMRC scheduler invocation. An immutable mesh borrow prevents
stale source geometry. Transaction snapshots do not clone the cache, and each
target is rebuilt. All certification gates and sorted overlap accumulation stay
unchanged. Failed candidates now share the phase timer with their caller: only
the unlogged tail is recorded separately, and every topology outcome is timed.

Three manual release samples, each containing six remaps from 64,002 source cells
to 16,002 target cells, gave median totals of **2.814 s uncached vs 1.630 s cached
(42.1% less time)**. Every remap matched exactly. Initial cache preparation is
included; the fresh path ran first in each pair. This is a remapping microbenchmark,
not an end-to-end NXP80 speedup.

The saved NXP10 project completed with byte-identical gridfile, certificate and
pre-export remap CSV; quality and search counters were unchanged. The new timer
recorded nine candidates, seven commits, two failed tails and fourteen topology
outcomes. Related library/integration tests passed (273 tests), along with fmt
and all-targets Clippy. Evidence and rerunnable checks are under
`.omx/artifacts/source-remap-reuse/`.

Memory remains a tradeoff: NXP10 peak RSS was 1.533 GiB versus the historical
1.305 GiB. Its wall time was 599.015 s versus 214.975 s. An initial NXP80 attempt
was stopped before cached remapping: unchanged requirement planning already took
109.374 s versus 26.465 s, and reported used memory was high. That memory reading
includes reclaimable caches and was insufficient to establish memory pressure;
the next attempt monitored actual pressure and system swap counters instead.

### Full-scale verification completed 2026-09-08

The adaptive CoastalOcean NXP80 retry completed in **3,612.985 s (60m13s)**.
Gridfile, certificate and pre-export remap CSV are byte-identical to the 32m46s
historical run; quality metrics and all search counters are unchanged. The new
phase accounting records 133 topology outcomes, 80 candidates, 59 commits and
21 separately timed failed tails.

Peak RSS was **22.948 GiB**, versus 20.898 GiB historically (about 2.05 GiB more).
The memory guard did not trigger. Sampled system swap-out counters increased
by 8.32 GiB during the run; these are machine-wide, not attributable solely to this process.
The untouched planning and geometry stages were also slower. **This run verifies
full-scale correctness and observes memory use; it does not prove an end-to-end
speedup.** Historical wall times are not a controlled same-session comparison.

A separate same-process release A/B test used **4,096,002 source cells** (the
NXP80 mother-grid size) and **1,024,002 regular target cells**. It alternated
fresh/cached order over six pairs, included the first cache preparation and
asserted exact remap equality for every pair:

| Six remaps | Total remapping time |
| --- | ---: |
| Prepare source each time | 55.527 s |
| Reuse prepared source | 29.005 s |

This is **47.8% less remapping time**, not 47.8% less whole-run time. The regular
target is not the adaptive CoastalOcean target; the latter was validated by the
full-run file comparisons above. Reproduce the large isolated test with:

```sh
EARTHMESH_REMAP_BENCH_SUBDIVISION=640 cargo test --release -p earthmesh_refine_certified --lib cached_voronoi_source_benchmark -- --ignored --nocapture
```

Related production changes are in `remap/mod.rs`,
`coarsen/component_transaction.rs` and `coarsen/scheduler.rs` under
`rust/earthmesh_refine_certified/src/`. Only the ignored benchmark was adjusted
after the full run; production code stayed fixed. Seven remap regressions and
all-targets Clippy/fmt checks passed again. The local GUI sidecar was atomically
updated to the tested engine SHA-256
`244976c601a5521fe748a6672c3ca7b9272a13bfe9409a13b2aaa351b76c4e8a`;
the previous engine is backed up. The combined follow-up below supersedes this engine.
Detailed evidence is in `.omx/artifacts/source-remap-reuse/full-retry/`,
`full-scale-remap/` and `sidecar-stage.json`.

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


## 2026-09-08 follow-up: split candidate costs and accelerate ring membership

This pass measures the remaining cost before changing any solver/certificate policy.

- Replaced the combined `component_prepare_and_elastic` timer with disjoint `install_delta`, `prepare_guard`, `geometry_screen`, `prepare_patch`, `elastic_solve`, and `elastic_apply` events. Rejected elastic candidates now emit their solve time before a tail-only failure event. `install_delta` includes restoring source positions; existing `local_geometry` includes lowering delivered levels; `remap` includes target-level preparation. Labels describe stages, not exclusively one function.
- Added `cmrc_detail phase=rebuild_* elapsed_us=...` for coverage, compaction, orientation and `MeshState` assembly. These are **nested, aggregate-only details**, including initial state, topology and install callers. They must not be summed again with component timings or attributed to a caller without separate evidence.
- The existing mixed-component fixture, scaled to source subdivisions 80 and 160, certified deterministically twice at each size. At subdivision 160, two transactions spent 426 ms in patch preparation and 1,095 ms in elastic solve; these are synthetic measurements, not the adaptive NXP80 project's proportions.
- A 40-second macOS sample at subdivision 320 showed substantial time in full geometry verification inside the elastic stage. Within the sampled base patch builder, ring expansion dominated map construction. Sampling includes main-thread waits for parallel work; it is diagnostic evidence, not a CPU-exclusive benchmark or a reason to remove validation.

The narrowly scoped change keeps the full face scan and ordered `BTreeSet` output, but replaces repeated tree membership queries with a temporary dense boolean mask. Membership updates happen only after a complete ring, preserving synchronous expansion, fixed vertices and missing-source behavior. The mask costs one byte per source-slot entry (about 3.91 MiB at the NXP80 mother-grid size) and is not retained between calls. No adjacency cache, new dependency, topology change, geometry relaxation, or search-budget change was introduced.

### Same-process ring helper A/B

The benchmark extracts the exact production helper and its retained tree-based test reference. Source subdivision 640 has 4,096,002 active cells and 8,192,000 faces. Each case executes six paired calls with alternating order and asserts exact set equality. Mask initialization is included; grid generation/process startup is not.

| Initial movable sites | Baseline median | Dense-mask median | Ring-stage time saved |
| --- | ---: | ---: | ---: |
| 64 | 1.722932 s | 0.320546 s | 81.4% |
| 2,048 | 2.569717 s | 0.309758 s | 87.9% |

These numbers are **not full mesh-generation speedups**. The same-session release component comparison used three process runs per version, with two transactions per run and alternating version order. All 12 transactions certified, with identical mesh fingerprints and phase counts. Patch-preparation run means had medians of 379.5 ms (baseline) and 134.0 ms (candidate), but unchanged topology and installation stages also shifted by about 41%, and the series drifted substantially. These component timings therefore **do not support a causal whole-component speedup claim**; in particular, the observed 12.8% transaction-median reduction is not attributed to this change. The reliable performance claim remains the same-process ring-helper result above.

Raw results, including the noisy observations, are retained in `.omx/artifacts/component-cost-breakdown/`. The subsequent full-scale runs below validate adaptive Tri/Hex outputs, not a controlled end-to-end speedup.

### Verification scope

- Tree-reference equivalence regression passed before and after implementation, covering 96 combinations of grid size, empty/sparse/all-site bases, fixed sites, mapping holes, out-of-mesh source slots and zero-to-three expansion rings.
- 294 related tests passed: library 245, transactions 10, core condensation 6, full polygon merge 19, topology 14.
- Subprocess timing checks cover disabled timing, successful candidates, failed elastic candidates, terminal topology outcomes and non-double-counted nested detail.
- All-target Clippy (`-D warnings`), workspace formatting and diff checks passed. Fresh candidate-binary subprocess checks passed for timing disabled, successful certification, and eight failed elastic candidates followed by a terminal topology outcome.
- Full adaptive NXP80 Tri/Hex exports subsequently passed the clean-checkout validation below; the tested engine is now staged for the local GUI.


### Clean alpha7 Tri/Hex validation

A detached checkout of `e7d657d4062fd2c8f1633ee2cb60a5706e696094` contained only
this document and the six certified-backend source/test files in this follow-up.
Unrelated local Red-Green, mesh and CLI drafts were excluded. The release build
used `--locked --features static-netcdf --target aarch64-apple-darwin`.
Tri and Hex ran sequentially with their original saved project bytes and unchanged
240-samples/degree LandType input; no budgets or quality gates were relaxed.

| Full adaptive NXP80 case | Wall time | Peak RSS |
| --- | ---: | ---: |
| CoastalOcean / Tri | 2,110.404 s (35m10s) | 24.616 GiB |
| AtmosphereMpas / Hex | 1,945.157 s (32m25s) | 25.125 GiB |

For each case, gridfile, certificate, remap CSV, quality summary CSV, repair plan,
repair cells, worst cells and readiness marker match that case's baseline bytes
exactly. Quality JSON, Markdown report, auto-refine decision, manifest and resource
counters also match after removing only verified output-path changes, elapsed time
and the checked manifest-file byte-length difference. All 112 components retain
the same outcomes: 59 committed, 53 promoted, none exhausted; 908 topology states,
3,609 elastic iterations and 953,859 interval boxes. Existing quality warnings
are unchanged, not repaired by these performance changes.

Tri retains the gridfile hash reported above. Hex gridfile SHA-256:
`5e52e8be1c0c33c7643bac5ccca269b2b6821760b545b109d084ff099698efed`.
The Tri ocean subset and Hex global dual have different export scopes; their
elapsed-time difference is not a like-for-like topology comparison.

Current **disjoint** component-stage totals expose the remaining costs:

| Stage | Tri | Hex |
| --- | ---: | ---: |
| Topology search | 682.246 s | 680.604 s |
| Elastic solve | 556.678 s | 522.207 s |
| Remapping | 306.778 s | 309.081 s |
| Install delta | 73.641 s | 72.718 s |
| Patch preparation | 37.169 s | 37.334 s |

Other component and export stages are omitted from this table; it is not a total.
Nested construction/rebuild timers must not be added to these stage totals.
Patch preparation is now about 1.8–1.9% of wall time; topology search and elastic
solve are the larger remaining measured stages, without implying that either can
be shortened safely by relaxing validation.

**These wall times are observational, not a causal speedup claim.** Background
applications/builds and source-cache conditions differed from historical runs;
the earlier best Tri time (32m46s) is still below this run. System-wide swap-outs
increased by zero during Tri and about 255 MiB during Hex, not attributable solely
to the engine. Neither run triggered the 32 GiB RSS or memory-pressure guard.
The reliable isolated speedup claims remain the same-process helper/remap tests.

The exact tested binary was atomically staged into the local GUI sidecar, with
its predecessor backed up. Engine SHA-256:
`1c6561efc932443dc63947b4c1601b4fcdab517f5584851b21c3c83fc9c92a8c`.
Protocol is `earthmesh-studio-engine/3`; NetCDF is statically linked. Source-checkout
GUI runs select this sidecar and refresh their temporary copy on the next launch.
Build provenance, source hashes, logs, memory samples and per-file comparison
results are retained in `.omx/artifacts/ring-full-validation/`.
