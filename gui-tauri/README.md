# EarthMesh Studio (Tauri shell)

A cross-platform desktop GUI (Windows / macOS / Linux) that uses the redesign
static frontend with a thin **Rust backend** over `earthmesh_project` for the
logic. This replaces the egui GUI, whose immediate-mode styling could not match
the static redesign.

```
gui-tauri/
├── dist/
│   └── index.html        # static frontend + Tauri invoke bridge
└── src-tauri/
    ├── Cargo.toml        # own workspace; deps: tauri v2 + earthmesh_project (NO hdf5)
    ├── build.rs          # tauri_build::build()
    ├── tauri.conf.json   # frontendDist=../dist, CSP, window, base bundle config
    ├── tauri.bundle.conf.json # release-only engine sidecar overlay
    ├── binaries/         # generated target-suffixed earthmesh_cli sidecar
    ├── capabilities/default.json
    ├── icons/icon@2x.png # 1024px Retina bundle icon
    └── src/
        ├── main.rs       # launcher -> earthmesh_studio_lib::run()
        └── lib.rs        # #[tauri::command]s
```

The look stays close to the redesign while the Tauri bridge binds the static UI
to real project commands. No CSS/colour porting, no immediate-mode constraints.

## Architecture

The frontend is plain static HTML/CSS/JS (no npm, no bundler). `tauri.conf.json`
sets `app.withGlobalTauri: true`, so the page calls Rust over IPC through
`window.__TAURI__.core.invoke(...)`. The Rust side stays **hdf5-free**: it only
builds/validates the project *intent* and exposes project capabilities. Actual
mesh generation is delegated to the discovered CLI through `--project`; the CLI
owns lowering, quality policy, AutoRefine, and hydro orchestration, so the GUI
process never links netcdf/hdf5.

When opened as plain HTML there is no IPC backend, so the page keeps a small set
of display-only fallback values to remain renderable. Inside Tauri,
`project_capabilities` replaces those values before project composition, save,
or run; the browser fallback is not an execution contract.

```
static UI ──invoke()──▶ #[tauri::command] ──▶ earthmesh_project
  (dist)    ◀─JSON/YAML─                          (scaffold / lower / criteria)
```

## Backend commands (`src-tauri/src/lib.rs`)

| command | args | returns |
|---|---|---|
| `list_criteria` | – | refinement criteria `{physical_process,label,help,unit,range_min,range_max,default_value,stem}` |
| `project_capabilities` | – | backend-owned intent ids, project defaults, and refinement level limits used by the runtime UI |
| `scaffold_project` | `name, intent, nxp?, approxKm?` | project **YAML** |
| `validate_project` | `yaml` | canonical YAML, or a parse error |
| `set_project_metadata` | `yaml, name, authors, description` | updated **YAML** |
| `preserve_unexposed_project_fields` | `baseYaml, yaml, preserveDomain` | updated **YAML** with opened-project fields the UI does not expose yet |
| `project_summary` | `yaml` | `{name,authors,description,intent,cell,model_format,domain,domain_shape,nxp,approx_km,effective_nxp,bbox,sea_ratio,min_angle_deg,auto_refine_batch_cells,on_violation,refine_enabled,threshold_refine_enabled,max_passes,hfield_enabled,layers:[{id,role_kind,role,path,enabled,threshold_value,wants_folder}]}` |
| `set_layer_path` | `yaml, id, path, enabled` | updated **YAML** |
| `set_threshold_value` | `yaml, id, value?` | updated **YAML** (per-criterion threshold; null uses default) |
| `autofill_data_layers_from_folder` | `yaml, folder` | updated **YAML** with matching NetCDF layer paths |
| `set_target_cell` | `yaml, cell` | updated **YAML** (`hex` or `tri`) |
| `set_domain_global` | `yaml` | updated **YAML** (global domain) |
| `set_domain_bbox` | `yaml, w, e, s, n, seaRatio?` | updated **YAML** (regional bbox) |
| `set_domain_shapefile` | `yaml, path, seaRatio?` | updated **YAML** (watershed SHP domain) |
| `set_domain_close` | `yaml, path, format, seaRatio?` | updated **YAML** (close boundary source) |
| `set_close_boundary` | `yaml, target, mode, iterations?, marginKm?, maxRadiusDeg?, maxSegmentAngleDeg?` | updated **YAML** (expert close boundary mode) |
| `set_quality` | `yaml, minAngleDeg, policy, autoRefineBatchCells` | updated **YAML** (min angle + policy + connected local repair batch) |
| `set_refinement` | `yaml, enabled, thresholdEnabled, maxPasses` | updated **YAML** (independent threshold switch + validated pass count) |
| `set_specified_refinement` | `yaml, enabled, kind?, lon?, lat?, radiusKm?, w?, e?, s?, n?, path?` | updated **YAML** (radius, bbox, or close refinement) |
| `set_hfield_refinement` | `yaml, enabled, g?, maxLevel?, baseM?` | updated **YAML** (default h-field; `enabled=false` stores discrete mask mode) |
| `set_expert` | `yaml, nxp?, openmp?, niter?, niterRefine?, maxIterSpc?, maxIterCal?, halo?, maxTransitionRow?, setDisType?, numRc?, vertexPretectLayers?, springGlobalType?, springRegionalType?, beta?, relax?, weakConcavEliminate?` | updated **YAML** (expert overrides) |
| `pick_data_file` | – | native file picker → path (or `null`) |
| `pick_data_folder` | – | native folder picker → path (tiled layers) |
| `open_project` | – | native open → `{path, yaml}` (or `null`) |
| `save_project` | `yaml` | native save → path (or `null`) |
| `read_project` | `path` | `{path, yaml}` for recent-project reopen |
| `open_path` | `path` | open output/report path in the OS file browser |
| `run_project` | `yaml, outdir?` | spawn the discovered mesh engine, stream `mkgrd://log` events, return `{ok,code,outdir,gridfile,auto_refine_decisions}`; each decision includes pass, selection reason, selected paths/verdict, structured guarded regressions, and its artifact path |
| `kill_run` | – | terminate the running engine child if one exists |
| `mesh_quality` | `gridfile, kind?` | parsed `quality_summary.json` for the dashboard; `kind` is `tri` or `hex` and maps to report `cell_view` (omitted defaults to `hex`) |
| `mesh_cell_polygons` | `gridfile, kind, maxCells?` | GeoJSON mesh overlay for the map |
| `mesh_merit_cells` | `gridfile, kind, meritRoot, w, e, s, n, stride?, landtypeFile?` | final mesh cells with MERIT-Hydro R2/R3 plus land-cover land/ocean/coast fractions; land-cover resolution is inferred from the file |
| `shapefile_boundary_geojson` | `path` | GeoJSON polygon outline for the map |

All wired to `earthmesh_project`: `ProjectConfig::scaffold` / `from_yaml` /
`from_json` / `validate` / `to_yaml` / `lower().to_namelist()` /
`criterion_catalog()`. File dialogs use `tauri-plugin-dialog` (Rust side,
`blocking_*` in `async` commands so they run off the UI thread); reads/writes
use `std::fs`.

## Run it

No Node/npm needed to run the static frontend. From `gui-tauri/src-tauri/`:

```bash
cargo run                 # builds backend, embeds dist/, opens the window
```

Or, with the Tauri CLI (adds packaging + dev conveniences):

```bash
cargo install tauri-cli --version "^2.0"   # once
(cd gui-tauri && cargo tauri dev)          # run
make build-gui-bundle                       # stage engine + package installers
```

The Make target applies `tauri.bundle.conf.json` and runs
`scripts/stage_tauri_sidecar.js`, which builds the CLI with
`--locked --features static-netcdf` and copies it to
`src-tauri/binaries/earthmesh_cli-$TARGET_TRIPLE[.exe]`. Tauri v2 then packages
it through `bundle.externalBin`; installed apps discover that bundled
`earthmesh_cli` beside the Studio executable. Node is only needed for packaging
and the existing frontend verification script, not for `cargo run`.

**System prerequisites** (Tauri webview):
- **Linux (Debian/Ubuntu)**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.
- **macOS**: Xcode Command Line Tools.
- **Windows**: WebView2 runtime (preinstalled on Win 11) + MSVC build tools.

## Verify the bridge

Once the window is open, the Run > Log pane shows
`✓ Rust backend connected — N refinement criteria registered` on load.

Clicking **Run** logs the real namelist lowered from the current UI project
config and streams engine stdout/stderr to the Log pane.

## Running a mesh

Clicking **Run** spawns the mesh generator. **No setup needed if you've built the
engine** — `make build` copies the CLI to `<repo>/mkgrd.x`, and the app
auto-discovers it:

```bash
make build                                # produces <repo>/mkgrd.x
cd gui-tauri/src-tauri && cargo run        # Run just works
```

`resolve_mkgrd()` searches, in order: a real `$EARTHMESH_MKGRD` file → the
app's own dir (including the bundled sidecar) → `<repo>/mkgrd.x` →
`rust/earthmesh_cli/target/{release,debug}/earthmesh_cli` →
`target/{release,debug}/earthmesh_cli` → `mkgrd.x` on `PATH`. When it finds a real file, the backend
runs a refreshed temp copy (`earthmesh_studio_engine-$PID-$SOURCE_HASH.x`) so
concurrent runs and different engine sources do not share a stale staged binary,
while source-tree C-library quirks do not affect GUI runs. Set `EARTHMESH_MKGRD`
only to override.

Outputs land in `<base>/<project name>/`, where `<base>` is either a fresh
`earthmesh_run_<ts>` temp dir or the explicit `outdir` passed to `run_project`.
Enabled threshold data layers are copied into the run's `threshold/` directory
under the engine file stems (`slope_avg.nc`, `lai.nc`, etc.) before `mkgrd.nml`
is written. The binary is only launched on an explicit Run click, never
automatically.

## Current behavior

- Project files can be created, opened, saved, validated, summarized, and lowered
  through the shared `earthmesh_project` model.
- Data layers, domain, quality, refinement, target output, and run state are
  reflected from `ProjectConfig` instead of duplicated frontend tables.
- Runs are explicit: the backend stages the engine, writes `mkgrd.nml`, streams
  stdout/stderr to the Log pane, supports kill, and reports the output directory.
- Successful runs load `quality_summary.json` and a map mesh overlay when the
  engine reports a gridfile; quality uses `tri-strict` for triangle targets and
  `hex-cgrid` for hex targets.
- AutoRefine runs scan only their own output tree (without following directory
  symlinks) and show every `auto_refine_decision.json` in pass order. Accepted
  candidates, baseline rollbacks, selected reports, and guarded metric
  regressions are rendered with text-only DOM assignments. Decision schema v1
  is authoritative; legacy artifacts without `schema_version` remain readable
  with a warning, while unknown future versions are skipped rather than decoded
  against an incompatible DTO.
- AutoRefine accepts global, regional bbox/close, and watershed domains. It can
  repair either an already-refined mesh or a uniform pass-zero baseline; its
  generated quality repair remains local and is accepted only when guarded
  quality metrics strictly improve.
- The quality dashboard treats polygon side counts as observed cell makeup, not
  topology failures; failures come from gates and topology issues.

Known gaps: circle domains remain preserved-but-not-editable in the GUI; polygon
domains need project-schema support first; release bundles still need a full
platform icon set.

## Caveats

- **Verification here covers syntax, drift checks, and Rust command behavior.**
  `make test-gui` uses Node to parse inline JS and check frontend-only invariants
  such as capability consumption, text-safe rendering, run-state wiring,
  placeholders, and i18n keys. Rust tests exercise the structured capability
  contract and Tauri command layer directly; the Node check does not scrape Rust
  source text.
  Packaging still depends on the local Tauri/webview prerequisites listed above.
- **Icons.** The 1024px `icons/icon@2x.png` filename marks its Retina density so Tauri can
  generate the macOS ICNS during release bundling. Add generated platform-specific
  icon sets only when publishing native installers for other operating systems.
- **Own workspace.** `src-tauri/Cargo.toml` declares an empty `[workspace]`, so
  this app stays out of the engine workspace and never affects
  `cargo test -p earthmesh_*`.
