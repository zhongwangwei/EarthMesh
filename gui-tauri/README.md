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
    ├── tauri.conf.json   # frontendDist=../dist, withGlobalTauri, window, bundle
    ├── capabilities/default.json
    ├── icons/icon.png    # bundle icon
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
builds/validates the project *intent* and lowers it to a Fortran namelist.
Actual mesh generation is delegated to the prebuilt engine: the backend lowers
the project to a namelist and runs the discovered engine with `<mkgrd.nml>` as
its positional input, so the GUI process never links netcdf/hdf5.

```
static UI ──invoke()──▶ #[tauri::command] ──▶ earthmesh_project
  (dist)    ◀─JSON/YAML─                          (scaffold / lower / criteria)
```

## Backend commands (`src-tauri/src/lib.rs`)

| command | args | returns |
|---|---|---|
| `list_criteria` | – | refinement criteria `{physical_process,label,help,unit,stem}` |
| `scaffold_project` | `name, intent, nxp?, approxKm?` | project **YAML** |
| `validate_project` | `yaml` | canonical YAML, or a parse error |
| `set_project_metadata` | `yaml, name, authors, description` | updated **YAML** |
| `preserve_unexposed_project_fields` | `baseYaml, yaml, preserveDomain` | updated **YAML** with opened-project fields the UI does not expose yet |
| `project_summary` | `yaml` | `{name,authors,description,intent,cell,model_format,domain,domain_shape,nxp,approx_km,effective_nxp,bbox,sea_ratio,min_angle_deg,on_violation,refine_enabled,max_passes,layers:[{id,role_kind,role,path,enabled,wants_folder}]}` |
| `set_layer_path` | `yaml, id, path, enabled` | updated **YAML** |
| `set_target_cell` | `yaml, cell` | updated **YAML** (`hex` or `tri`) |
| `set_domain_global` | `yaml` | updated **YAML** (global domain) |
| `set_domain_bbox` | `yaml, w, e, s, n, seaRatio?` | updated **YAML** (regional bbox) |
| `set_domain_shapefile` | `yaml, path, seaRatio?` | updated **YAML** (watershed SHP domain) |
| `set_domain_close` | `yaml, path, format, seaRatio?` | updated **YAML** (close boundary source) |
| `set_quality` | `yaml, minAngleDeg, block` | updated **YAML** (min angle + policy) |
| `set_refinement` | `yaml, enabled, maxPasses` | updated **YAML** (validated pass count) |
| `set_specified_refinement` | `yaml, enabled, kind?, lon?, lat?, radiusKm?, w?, e?, s?, n?, path?` | updated **YAML** (radius, bbox, or close refinement) |
| `set_expert` | `yaml, nxp?, openmp?, niter?, niterRefine?, maxIterSpc?, maxIterCal?, halo?, maxTransitionRow?, setDisType?, numRc?, vertexPretectLayers?, beta?, relax?, weakConcavEliminate?` | updated **YAML** (expert overrides) |
| `pick_data_file` | – | native file picker → path (or `null`) |
| `pick_data_folder` | – | native folder picker → path (tiled layers) |
| `open_project` | – | native open → `{path, yaml}` (or `null`) |
| `save_project` | `yaml` | native save → path (or `null`) |
| `read_project` | `path` | `{path, yaml}` for recent-project reopen |
| `open_path` | `path` | open output/report path in the OS file browser |
| `run_project` | `yaml, outdir?` | spawn the discovered mesh engine, stream `mkgrd://log` events, return `{ok,code,outdir,gridfile}` |
| `kill_run` | – | terminate the running engine child if one exists |
| `mesh_quality` | `gridfile, kind?` | parsed `quality_summary.json` for the dashboard |
| `mesh_cell_polygons` | `gridfile, kind?, maxCells?` | GeoJSON mesh overlay for the map |
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
cd gui-tauri && cargo tauri dev            # run
cd gui-tauri && cargo tauri build          # package installers
```

**System prerequisites** (Tauri webview):
- **Linux**: `webkit2gtk-4.1`, `libgtk-3-dev`, `libsoup-3.0`, `librsvg2-dev`, `build-essential` (see tauri.app prerequisites for your distro).
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

`resolve_mkgrd()` searches, in order: a real `$EARTHMESH_MKGRD` file →
`<repo>/mkgrd.x` → `rust/earthmesh_cli/target/{release,debug}/earthmesh_cli` →
`target/{release,debug}/earthmesh_cli` → the app's own dir → `mkgrd.x` on
`PATH`. When it finds a real file, the backend runs a refreshed temp copy
(`earthmesh_studio_engine.x`) so source-tree C-library quirks do not affect GUI
runs. Set `EARTHMESH_MKGRD` only to override.

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
  engine reports a gridfile.

Known gaps: circle domains remain preserved-but-not-editable in the GUI; polygon
domains need project-schema support first; per-gate thresholds can be exposed
after `QualityConfig` carries them; release bundles still need an explicit
engine sidecar strategy and full platform icon set.

## Caveats

- **Verification here covers syntax, drift checks, and Rust command behavior.**
  `make test-gui` uses Node to parse inline JS and fail fast on GUI/backend
  drift (intent gallery coverage, default template/default values, command docs,
  frontend invokes, text-safe rendering, run-state wiring, placeholders, and i18n
  keys), then exercises the Tauri command layer.
  Packaging still depends on the local Tauri/webview prerequisites listed above.
- **Icons.** Only `icons/icon.png` is included (enough for `cargo run`/dev). For
  release bundles run `cargo tauri icon icons/icon.png` to generate the full
  platform icon set, then list them in `bundle.icon`.
- **Own workspace.** `src-tauri/Cargo.toml` declares an empty `[workspace]`, so
  this app stays out of the engine workspace and never affects
  `cargo test -p earthmesh_*`.
