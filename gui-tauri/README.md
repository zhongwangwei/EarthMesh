# EarthMesh Studio (Tauri shell)

A cross-platform desktop GUI (Windows / macOS / Linux) that uses the redesign
static frontend with a thin **Rust backend** over `earthmesh_project` for the
logic. This replaces the egui GUI, whose immediate-mode styling could not match
the static redesign.

```
gui-tauri/
├── dist/
│   ├── index.html         # static frontend + Tauri invoke bridge
│   └── vendor/
│       ├── openlayers/    # local planar-map runtime
│       └── maplibre/      # MapLibre GL JS 5.24.0 CSP bundle, worker, CSS, license
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

### Map rendering

The embedded result map and the independent map window's **Plane** mode use the
vendored OpenLayers runtime. The independent window also exposes a **Globe**
mode backed by vendored MapLibre GL JS 5.24.0 with the fixed
`vertical-perspective` projection. Switching mode does not replace the inline
map or clone/rebuild the raw mesh payload.

Both renderers consume the same raw mesh/domain/coastal GeoJSON objects received
through patch-based map-window IPC. Their vector-source reference caches skip
OpenLayers parsing and MapLibre `setData` when an object's identity is unchanged;
basemap, projection, opacity, and visibility changes update existing renderer
objects instead of rebuilding the GeoJSON. Classified coastal cells replace the
raw mesh layer rather than being drawn on top of it.

All map JavaScript, CSS, and worker code is self-hosted. `vendor/maplibre/`
contains the 5.24.0 CSP bundle, CSP worker, stylesheet, and upstream license;
Tauri restricts both `script-src` and `worker-src` to `'self'`. Only the configured
ArcGIS raster endpoint is admitted for basemap requests. Plane and globe modes
share the native raw-PNG save command and produce exact-size exports with
EarthMesh attribution; globe export waits for MapLibre to become idle before
capturing its preserved WebGL canvas.

MapLibre vector sources still use Web Mercator tiling, so vector cells wholly
beyond about 85.05° latitude are not drawn in Globe mode; Plane mode remains
the exact polar-data view. A true polar globe layer requires a custom WebGL
layer rather than coordinate clamping, which would misrepresent the mesh.

## Backend commands (`src-tauri/src/lib.rs`)

| command | args | returns |
|---|---|---|
| `list_criteria` | – | one categorical landcover criterion plus independent continuous mean/std criteria `{id,source_stem,statistic,physical_process,label,help,unit,range_min,range_max,default_value}` |
| `project_capabilities` | – | backend-owned intent ids, project defaults, and refinement level limits used by the runtime UI |
| `scaffold_project` | `name, intent, nxp?, approxKm?` | project **YAML** |
| `validate_project` | `yaml` | canonical YAML, or a parse error |
| `set_project_metadata` | `yaml, name, authors, description` | updated **YAML** |
| `preserve_unexposed_project_fields` | `baseYaml, yaml, preserveDomain` | updated **YAML** with opened-project fields the UI does not expose yet |
| `project_summary` | `yaml` | `{name,authors,description,intent,target_kind,cell,model_format,domain,domain_shape,nxp,approx_km,approx_degree,effective_nxp,bbox,sea_ratio,min_angle_deg,auto_refine_batch_cells,on_violation,refine_enabled,threshold_refine_enabled,threshold_criteria:[{id,source_id,statistic,source_enabled,enabled,value}],refinement_backend,refinement_algorithm,method_c_lepp_*,harp_dv_*,hydro_river_width_refine_enabled,hydro_river_upstream_area_refine_enabled,hydro_river_width_threshold_m,hydro_river_upstream_area_threshold_km2,hydro_coast_refine_enabled,hydro_coast_buffer_km,hydro_coast_land_refine_enabled,hydro_coast_ocean_refine_enabled,max_passes,hfield_enabled,layers:[{id,role_kind,source_field,role,path,enabled,threshold_value,wants_folder}]}` |
| `set_layer_path` | `yaml, id, path, enabled` | updated **YAML** |
| `set_threshold_value` | `yaml, id, value?` | updated **YAML** (legacy/shared source threshold; null uses the catalog default) |
| `set_threshold_criterion` | `yaml, id, enabled, value?` | updated **YAML** (one independent `<source>_mean` or `<source>_std` switch/value; source path remains in `data_layers`) |
| `set_hydro_refinement` | `yaml, riverWidthEnabled, riverUpstreamAreaEnabled, coastEnabled, coastBufferKm, coastLandEnabled, coastOceanEnabled, riverWidthThresholdM, riverUpstreamAreaThresholdKm2` | updated **YAML** (independent river-width/upstream-area demand plus the coast-distance demand) |
| `autofill_data_layers_from_folder` | `yaml, folder` | updated **YAML** with matching NetCDF layer paths |
| `set_project_target` | `yaml, kind, modelFormat` | updated **YAML** after enforcing the backend kind/model compatibility matrix |
| `set_target_cell` | `yaml, cell` | updated **YAML** (`hex` or `tri`) |
| `set_domain_global` | `yaml` | updated **YAML** (global domain) |
| `set_domain_bbox` | `yaml, w, e, s, n, seaRatio?` | updated **YAML** (regional bbox) |
| `set_domain_shapefile` | `yaml, path, seaRatio?` | updated **YAML** (watershed SHP domain) |
| `set_domain_close` | `yaml, path, format, seaRatio?` | updated **YAML** (close boundary source) |
| `set_close_boundary` | `yaml, target, mode, iterations?, marginKm?, maxRadiusDeg?, maxSegmentAngleDeg?` | updated **YAML** (expert close boundary mode) |
| `set_quality` | `yaml, minAngleDeg, policy, autoRefineBatchCells` | updated **YAML** (min angle + policy + connected local repair batch) |
| `set_refinement` | `yaml, enabled, thresholdEnabled, maxPasses` | updated **YAML** (independent threshold switch + validated pass count) |
| `set_specified_refinement` | `yaml, enabled, kind?, lon?, lat?, radiusKm?, w?, e?, s?, n?, path?` | updated **YAML** (radius, bbox, or close refinement) |
| `set_refinement_backend` | `yaml, backend` | updated **YAML**; accepts the four UI algorithm ids `method_c`, `lepp_delaunay` (AdaptiveHybrid), `red_green`, and `harp_dv` |
| `set_method_c_algorithm_options` | `yaml` plus the eight LEPP-Delaunay controls | updated **YAML** after validating cycle, tolerance, neighbor-ratio, vertex/insertion/path limits, source-resolution stop, and minimum angle |
| `set_harp_dv_options` | `yaml` plus the nine HARP-DV controls | updated **YAML** after validating cycle, cell-width/budget, patch, neighbor-ratio, separation, degree, and angle limits |
| `set_hfield_refinement` | `yaml, enabled, g?, maxLevel?, baseM?` | updated **YAML** (opt-in canonical H-field; point+radius is the GUI default) |
| `set_expert` | `yaml, nxp?, openmp?, niter?, niterRefine?, maxIterSpc?, maxIterCal?, halo?, maxTransitionRow?, setDisType?, numRc?, vertexPretectLayers?, springGlobalType?, springRegionalType?, beta?, relax?, weakConcavEliminate?, isolatedOcean?` | updated **YAML** (expert overrides; compatibility-only values remain preserved even when not editable in the GUI) |
| `pick_data_file` | – | native file picker → path (or `null`) |
| `pick_data_folder` | – | native folder picker → path (tiled layers) |
| `open_project` | – | native open → `{path, yaml}` (or `null`) |
| `save_project` | `yaml` | native save → path (or `null`) |
| `save_map_png` | raw PNG IPC body | validated PNG → native save path (or `null`) |
| `read_project` | `path` | `{path, yaml}` for recent-project reopen |
| `open_path` | `path` | open output/report path in the OS file browser |
| `run_project` | `yaml, outdir?` | spawn the discovered mesh engine, stream `mkgrd://log` events, return `{ok,code,outdir,gridfile,auto_refine_decisions}`; each decision includes pass, selection reason, selected paths/verdict, structured guarded regressions, and its artifact path |
| `kill_run` | – | terminate the running engine child if one exists |
| `mesh_quality` | `gridfile, kind?` | parsed `quality_summary.json` for the dashboard; `kind` is `tri` or `hex` and maps to report `cell_view` (omitted defaults to `hex`) |
| `mesh_cell_polygons` | `gridfile, kind, maxCells?` | GeoJSON mesh overlay for the map |
| `mesh_merit_cells` | `gridfile, kind, meritRoot, w, e, s, n, stride?, landtypeFile?, r2WidthM, r2UpaKm2, r3WidthM, r3UpaKm2` | final mesh cells with MERIT-Hydro R2/R3 plus land-cover land/ocean/coast fractions; land-cover resolution is inferred from the file |
| `shapefile_boundary_geojson` | `path` | GeoJSON polygon outline for the map |

New MERIT-Hydro configurations use one 50 km coast-distance threshold with both
sides enabled. Legacy YAML that lacks the distance field loads as 0 km (physical
coastline cells only), so opening an older project cannot silently expand its
refinement footprint.

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
- The embedded map and the independent window's Plane mode use vendored
  OpenLayers. The independent window can switch in place to a vendored MapLibre
  `vertical-perspective` globe while retaining the same raw GeoJSON and map
  settings. Both modes support the five Esri base styles plus a blank view,
  layer visibility and opacity controls, cell details, dateline-safe fitting,
  and exact-size PNG export. Web Mercator/geographic/automatic local UTM views
  and distance/area measurement remain Plane-mode controls.
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
  contract and Tauri command layer directly; the Node check also guards the raw
  PNG save-command registration, locally vendored OpenLayers/MapLibre runtimes,
  strict CSP worker setup, plane/globe switch, fixed globe projection, raw
  GeoJSON identity caching, and both export paths used by the static frontend.
  Packaging still depends on the local Tauri/webview prerequisites listed above.
- **Icons.** The 1024px `icons/icon@2x.png` filename marks its Retina density so Tauri can
  generate the macOS ICNS during release bundling. Add generated platform-specific
  icon sets only when publishing native installers for other operating systems.
- **Own workspace.** `src-tauri/Cargo.toml` declares an empty `[workspace]`, so
  this app stays out of the engine workspace and never affects
  `cargo test -p earthmesh_*`.
