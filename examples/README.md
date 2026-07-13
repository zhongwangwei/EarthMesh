# EarthMesh v3 examples

Examples fall into two classes. The CI / regression suite only exercises **runnable
templates**; **external-data cases** are documented but require data you provide.

## Runnable templates (no external data)

Work out-of-the-box — no large dataset, no `${EARTHMESH_DATA}`. Used by the smoke /
regression tests.

| Path | Target | Notes |
|------|--------|-------|
| `00_quickstart_n16.nml` | Quickstart global mesh (NXP=16) | Tiny synthetic; default smoke case loaded by the GUI. |
| `projects/quickstart.yaml` | Project-layer quickstart | Parsed, validated, lowered, and namelist-smoked by `earthmesh_project` tests. |
| `projects/auto_refine.yaml` | Regional atmosphere + local quality repair | Full CLI regression: quality report -> HField repair -> Method-C rerun -> recheck. |
| `default/atmosphere_hex_global.nml` | Global hex atmosphere → MPAS | Base-mesh only. |
| `default/land_hex_global.nml` | Global hex land → CoLM | Base-mesh only. |
| `default/ocean_hex_global.nml` | Global hex ocean → FVCOM | Base-mesh only. |

Run one:

```sh
make build
./mkgrd.x examples/00_quickstart_n16.nml
```

The AutoRefine example is also executed end-to-end by the CLI test suite. Copy
it into a work directory before running so generated project artifacts stay out
of the source tree. A repair candidate is rechecked before acceptance; if its
quality regresses, EarthMesh keeps the previous valid mesh and retains the
candidate report for diagnosis. AutoRefine changes one connected defect cell
per pass by default (`quality.auto_refine_batch_cells` can raise the bounded
batch), and writes `auto_refine_decision.json` beside the candidate quality
report with `schema_version: 1`, the selected gridfile, quality-report paths,
acceptance/rejection reason, and a structured `regressions` array. Each
regression records `metric`, its preferred direction (`higher` or `lower`),
the baseline and candidate values, and `delta = candidate - baseline`. Verdict
levels use `pass`, `warn`, and `fail`; the decision keeps the baseline whenever
the candidate is not a strict improvement or any guarded metric regresses.
EarthMesh Studio reads these artifacts from the current run directory and
displays the same candidate-selection audit. When a
project explicitly requests fewer than 20 Method-C spring iterations, quality
repair candidates raise that low override to 20 so acceptance measures a
relaxed mesh rather than a one-iteration transition artifact. Projects without
an explicit override retain the canonical 5000 atmosphere / 2000 surface
iterations.

```sh
mkdir -p /tmp/earthmesh-auto-refine
cp examples/projects/auto_refine.yaml /tmp/earthmesh-auto-refine/project.yaml
(cd /tmp/earthmesh-auto-refine && /path/to/EarthMesh/mkgrd.x --project project.yaml --quiet)
```

## External-data cases (require `${EARTHMESH_DATA}`)

Need real input datasets (MERIT-Hydro tiles, landtype, etc.). Paths use the
`${EARTHMESH_DATA}` placeholder; set it before running. Each case has its own README.

| Path | Needs | Notes |
|------|-------|-------|
| `merit_hydro/gba/` | MERIT-Hydro tiles | Greater Bay Area river/coast refined coupled mesh. See `gba/README.md`. |
| `merit_hydro/yangtze_delta/` | MERIT-Hydro tiles | Yangtze delta estuary mesh. See `yangtze_delta/README.md`. |

```sh
export EARTHMESH_DATA=/path/to/your/datasets
./mkgrd.x examples/merit_hydro/gba/case.nml
```

> The `examples_paths` regression test (in `earthmesh_core`) guards that committed
> examples carry no personal absolute paths, that runnable templates contain no
> `${EARTHMESH_DATA}` placeholder, and that every external-data case ships a README.

Close-domain expert options (`polyline`, spherical Chaikin, and conservative
enclosing cap) are documented in [`docs/close_boundary_modes.md`](../docs/close_boundary_modes.md).
