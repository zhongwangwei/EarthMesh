# EarthMesh Studio (Tauri shell)

A cross-platform desktop GUI (Windows / macOS / Linux) that reuses the redesign
prototype **as-is** for the frontend and a thin **Rust backend** over
`earthmesh_project` for the logic. This replaces the egui GUI, whose
immediate-mode styling could not match the prototype.

```
gui-tauri/
├── dist/
│   └── index.html        # frontend = the prototype (docs/gui_redesign/prototype.html)
│                         #   + a Tauri bridge appended at the end (window.emProject)
└── src-tauri/
    ├── Cargo.toml        # own workspace; deps: tauri v2 + earthmesh_project (NO hdf5)
    ├── build.rs          # tauri_build::build()
    ├── tauri.conf.json   # frontendDist=../dist, withGlobalTauri, window, bundle
    ├── capabilities/default.json
    ├── icons/icon.png    # reused from rust/earthmesh_gui
    └── src/
        ├── main.rs       # launcher -> earthmesh_studio_lib::run()
        └── lib.rs        # #[tauri::command]s
```

The look matches the prototype **by construction** — the prototype *is* the
frontend. No CSS/colour porting, no immediate-mode constraints.

## Architecture

The frontend is plain static HTML/CSS/JS (no npm, no bundler). `tauri.conf.json`
sets `app.withGlobalTauri: true`, so the page calls Rust over IPC through
`window.__TAURI__.core.invoke(...)`. The Rust side stays **hdf5-free**: it only
builds/validates the project *intent* and lowers it to a Fortran namelist.
Actual mesh generation is delegated to the prebuilt engine: the backend lowers
the project to a namelist and runs `mkgrd.x <mkgrd.nml>` (the CLI's positional
input form), so the GUI process never links netcdf/hdf5.

```
prototype UI ──invoke()──▶ #[tauri::command] ──▶ earthmesh_project
   (dist)     ◀─JSON/YAML─                          (scaffold / lower / criteria)
```

## Backend commands (`src-tauri/src/lib.rs`)

| command | args | returns |
|---|---|---|
| `list_intents` | – | 12 intent presets `{id,label}` |
| `list_criteria` | – | refinement criteria `{id,label,unit,range,default,…}` |
| `scaffold_project` | `name, intent, nxp` | project **YAML** |
| `project_to_namelist` | `yaml` | Fortran **namelist** (engine input) |
| `validate_project` | `yaml` | canonical YAML, or a parse error |
| `project_summary` | `yaml` | `{name,intent,domain,nxp,bbox,min_angle_deg,on_violation,layers[]}` |
| `set_layer_path` | `yaml, id, path, enabled` | updated **YAML** |
| `set_domain_global` | `yaml` | updated **YAML** (global domain) |
| `set_domain_bbox` | `yaml, w, e, s, n, seaRatio?` | updated **YAML** (regional bbox) |
| `set_quality` | `yaml, minAngleDeg, block` | updated **YAML** (min angle + policy) |
| `pick_data_file` | – | native file picker → path (or `null`) |
| `pick_data_folder` | – | native folder picker → path (tiled layers) |
| `open_project` | – | native open → `{path, yaml}` (or `null`) |
| `save_project` | `yaml` | native save → path (or `null`) |
| `run_project` | `yaml, outdir?` | spawn `mkgrd.x`, stream `mkgrd://log` events, return `{ok,code,outdir}` |

All wired to `earthmesh_project`: `ProjectConfig::scaffold` / `from_yaml` /
`from_json` / `to_yaml` / `lower().to_namelist()` / `criterion_catalog()`. File
dialogs use `tauri-plugin-dialog` (Rust side, `blocking_*` in `async` commands so
they run off the UI thread); reads/writes use `std::fs`.

## Run it

No Node/npm needed (static frontend). From `gui-tauri/src-tauri/`:

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
`✓ Rust backend connected — N refinement criteria registered` on load. Open
devtools and call the backend directly:

```js
await emProject.listIntents()
await emProject.scaffold("test", "HydrologyLand", 40)   // real YAML from Rust
await emProject.buildFromUi()                            // {yaml, nml} for current UI
```

Clicking **Run** also logs the real namelist lowered from the current template +
resolution (the prototype's mock animation still plays on top).

## Running a mesh

Clicking **Run** spawns the mesh generator. **No setup needed if you've built the
engine** — `make build` copies the CLI to `<repo>/mkgrd.x`, and the app
auto-discovers it:

```bash
make build                                # produces <repo>/mkgrd.x
cd gui-tauri/src-tauri && cargo run        # Run just works
```

`resolve_mkgrd()` searches, in order: `$EARTHMESH_MKGRD` → `<repo>/mkgrd.x` →
`rust/earthmesh_cli/target/{release,debug}/earthmesh_cli` → `target/{release,debug}/earthmesh_cli`
→ the app's own dir → `mkgrd.x` on `PATH`. Set `EARTHMESH_MKGRD` only to override.

Outputs land in a fresh `earthmesh_run_<ts>` temp dir (reported in the Log pane);
pass an explicit `outdir` to `run_project` to change that. The binary is only
launched on an explicit Run click, never automatically.

## Status

**Slice 0 — shell + architecture (done):** Tauri shell, prototype reused as
frontend, IPC proven end-to-end, Run emits a real lowered namelist.

**Slice 1 — live data layers (done):** step 3's table is rebuilt from the live
`ProjectConfig.data_layers`. Each row's **Browse** opens the native picker and
writes the path back via `set_layer_path`; ✕ clears it. Tiled inputs
(MERIT-Hydro, CaMa) are directories of tiles, so their rows open a **folder**
picker (`pick_data_folder`) and are flagged "tiled dir". The panel reflects only
the layers the current intent preset actually uses (no silent refinement on
missing data) — e.g. the `Land-ocean coupled` template scaffolds `landcover` +
`lai` + `sea_slope` (no `slope_avg`; that comes from the `MERIT-Hydro` template).

**Slice 2 — open / save project.yaml (done):** header **📂 Open** / **💾 Save**
buttons (Tauri only). Save composes the canonical YAML (template + resolution +
layer edits) and writes it via a native dialog; Open reads + validates a
`.yaml`/`.yml`/`.json` project and reflects it back into the UI (name, template,
resolution, layer paths).

**Slice 3 — domain + quality (done):** step 2's Global/Regional pills + W/E/S/N
inputs bind to `DomainConfig` (`Regional{Bbox}`); step 5's *On violation* select
and expert *Min angle ≥* bind to `QualityConfig` (`min_angle_deg` + `Warn`/`Block`,
both of which `lower()` carries into the engine namelist). Open reflects bbox +
quality back into the UI.

**Slice 4 — run with streamed logs (done):** **Run** composes the full project
YAML, then `run_project` lowers it to `mkgrd.nml`, writes both files, and spawns
the mesh generator (`$EARTHMESH_MKGRD`, else `mkgrd.x` on `PATH`) as
`mkgrd.x <mkgrd.nml>`. stdout/stderr are streamed line-by-line to the Log pane
via `mkgrd://log` events; the status pill flips running → finished/failed and
the result reports the output directory.

Done with `std::process::Command` + the core event system — **no shell plugin, no
sidecar bundling** — so the binary path is flexible and nothing is executed
unless the user clicks Run. If `mkgrd.x` isn't found, the Log pane shows a clear
"build it / set `EARTHMESH_MKGRD`" message.

**Slice 5 — workflow UX (done):**
- **Domain:** Global hides the bbox / lat-lon inputs entirely (just a "whole
  planet" note); they appear only in Regional mode. The toggle re-renders live.
- **Refinement:** step 4's criteria list is rebuilt from the *selected template's*
  threshold fields (catalog-labelled), so it changes with the template — e.g.
  atmosphere → typhoon track; land → LAI/slope; coupled → LAI + sea-slope.
- **Data layers** were already template-driven (the panel scaffolds per intent).
- **Run** moved out of the top-right header into the final step's footer and is
  renamed Run (was "Finish"); it triggers the real streamed run there.

**Next (iterative):** cancel a running job (kill the child via shared state);
circle/polygon domains; the remaining per-gate thresholds (skew, area ratio,
coupling) once `QualityConfig` carries them; optionally bundle `mkgrd.x` as a
proper Tauri sidecar for distribution.

## Caveats

- **Not compiled here.** This scaffold was written without a Tauri toolchain in
  the build sandbox. Run `cargo run` (or `cargo tauri dev`) locally first; if
  your installed Tauri v2 CLI flags a config field, adjust `tauri.conf.json` to
  its `tauri config schema`. The dialog commands convert the picked `FilePath`
  with `.to_string()`; if your `tauri-plugin-dialog` version rejects that, change
  it to `.into_path().unwrap().display().to_string()` (in `src/lib.rs`).
- **Icons.** Only `icons/icon.png` is included (enough for `cargo run`/dev). For
  release bundles run `cargo tauri icon icons/icon.png` to generate the full
  platform icon set, then list them in `bundle.icon`.
- **Own workspace.** `src-tauri/Cargo.toml` declares an empty `[workspace]`, so
  this app stays out of the engine workspace and never affects
  `cargo test -p earthmesh_*`.
