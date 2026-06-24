//! EarthMesh Studio — Tauri backend.
//!
//! Thin command layer over `earthmesh_project` (the intent schema). The webview
//! frontend (reused from `docs/gui_redesign/prototype.html`) calls these over
//! Tauri IPC via `window.__TAURI__.core.invoke(...)`.
//!
//! Deliberately hdf5-free: this process only builds/validates the project
//! intent and lowers it to a Fortran namelist. Actual mesh generation is left
//! by lowering the project to a namelist here and running `mkgrd.x <mkgrd.nml>`
//! (the CLI's positional input form), so the GUI never links netcdf.

use earthmesh_project::{
    criterion_catalog, DomainConfig, MeshIntentPreset, ProjectConfig, ProjectLayerRole,
    RegionShape, ResolutionSpec, ViolationPolicy,
};
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

/// One selectable intent preset, for the "New project" template gallery.
#[derive(Serialize)]
struct IntentInfo {
    id: String,
    label: String,
}

/// A refinement criterion, flattened for the data-layer / quality UI.
#[derive(Serialize)]
struct CriterionInfo {
    id: String,
    display_name: String,
    physical_process: String,
    label: String,
    help: String,
    unit: String,
    range_min: f64,
    range_max: f64,
    default: f64,
    /// Engine file stem (= the scaffolded layer id) so the UI can match a
    /// template's threshold layers back to their criterion metadata.
    stem: String,
}

/// A loaded project: canonical YAML plus the path it came from.
#[derive(Serialize)]
struct OpenedProject {
    path: String,
    yaml: String,
}

/// One data layer, flattened for the live layer panel.
#[derive(Serialize)]
struct LayerSummary {
    id: String,
    role: String,
    path: String,
    enabled: bool,
    /// True for tiled inputs (MERIT-Hydro, CaMa) that are directories of tiles,
    /// so the UI offers a folder picker instead of a file picker.
    wants_folder: bool,
}

/// A project at a glance — used to reflect a loaded YAML back into the UI.
#[derive(Serialize)]
struct ProjectSummary {
    name: String,
    intent: String,
    domain: String,
    nxp: Option<i32>,
    approx_km: Option<f64>,
    /// `[w, e, s, n]` when the domain is a regional bounding box, else `None`.
    bbox: Option<[f64; 4]>,
    min_angle_deg: f64,
    on_violation: String,
    refine_enabled: bool,
    max_passes: u8,
    layers: Vec<LayerSummary>,
}

/// Outcome of a mesh run: exit status + where the outputs landed.
#[derive(Serialize)]
struct RunResult {
    ok: bool,
    code: Option<i32>,
    outdir: String,
    /// The gridfile the engine reported (`gridfile=<path>` on stdout), so the GUI
    /// can run quality + draw the mesh without re-globbing. None if not seen.
    gridfile: Option<String>,
}

/// The 12 intent presets in catalog order, paired with stable ids that match
/// their serde representation (so `scaffold_project` round-trips cleanly).
const INTENTS: &[(MeshIntentPreset, &str, &str)] = &[
    (MeshIntentPreset::Custom, "Custom", "Custom / blank"),
    (MeshIntentPreset::HydrologyLand, "HydrologyLand", "Land · Hydrology"),
    (MeshIntentPreset::CarbonLand, "CarbonLand", "Land · Carbon"),
    (MeshIntentPreset::SnowPermafrostLand, "SnowPermafrostLand", "Land · Snow / Permafrost"),
    (MeshIntentPreset::UrbanLand, "UrbanLand", "Land · Urban"),
    (MeshIntentPreset::CoastalOcean, "CoastalOcean", "Ocean · Coastal"),
    (MeshIntentPreset::Estuary, "Estuary", "Ocean · Estuary"),
    (MeshIntentPreset::RiverNetwork, "RiverNetwork", "Ocean · River network"),
    (MeshIntentPreset::MeritHydroCoast, "MeritHydroCoast", "Coast · MERIT-Hydro"),
    (MeshIntentPreset::LandOceanCoupled, "LandOceanCoupled", "Coupled · Land–Ocean"),
    (MeshIntentPreset::AtmosphereTyphoonPrecip, "AtmosphereTyphoonPrecip", "Atmosphere · Typhoon / Precip"),
    (MeshIntentPreset::MultiObjectiveBalanced, "MultiObjectiveBalanced", "Multi-objective balanced"),
];

fn parse_intent(s: &str) -> MeshIntentPreset {
    INTENTS
        .iter()
        .find(|(_, id, _)| *id == s)
        .map(|(p, _, _)| *p)
        .unwrap_or(MeshIntentPreset::Custom)
}

/// Reverse of [`parse_intent`]: the stable id for a preset.
fn intent_id(p: MeshIntentPreset) -> String {
    INTENTS
        .iter()
        .find(|(x, _, _)| *x == p)
        .map(|(_, id, _)| id.to_string())
        .unwrap_or_else(|| "Custom".to_string())
}

/// Human-readable label for a data-layer role (Threshold variants via serde).
fn role_label(r: &ProjectLayerRole) -> String {
    match r {
        ProjectLayerRole::LandType => "land type".to_string(),
        ProjectLayerRole::SpecifiedMask => "specified mask".to_string(),
        ProjectLayerRole::MeritHydro => "MERIT-Hydro".to_string(),
        ProjectLayerRole::Cama => "CaMa".to_string(),
        ProjectLayerRole::Threshold(f) => format!(
            "threshold · {}",
            serde_json::to_string(f).unwrap_or_default().trim_matches('"')
        ),
    }
}

/// List the intent presets for the template gallery.
#[tauri::command]
fn list_intents() -> Vec<IntentInfo> {
    INTENTS
        .iter()
        .map(|(_, id, label)| IntentInfo {
            id: id.to_string(),
            label: label.to_string(),
        })
        .collect()
}

/// List every registered refinement criterion (self-describing GUI specs).
#[tauri::command]
fn list_criteria() -> Vec<CriterionInfo> {
    criterion_catalog()
        .iter()
        .map(|c| CriterionInfo {
            id: c.id.to_string(),
            display_name: c.display_name.to_string(),
            physical_process: c.physical_process.to_string(),
            label: c.gui.label.to_string(),
            help: c.gui.help.to_string(),
            unit: c.gui.unit.to_string(),
            range_min: c.gui.range.0,
            range_max: c.gui.range.1,
            default: c.gui.default,
            stem: c.field.stem().to_string(),
        })
        .collect()
}

/// Scaffold a project from an intent preset and serialize it to YAML.
/// `domain` is "global" (default) for now; regional domains come from the map UI.
#[tauri::command]
fn scaffold_project(name: String, intent: String, nxp: i32) -> Result<String, String> {
    let domain = domain_or_default(&[]);
    let cfg = ProjectConfig::scaffold(
        &name,
        parse_intent(&intent),
        domain,
        ResolutionSpec::Nxp(nxp.max(1)),
    );
    cfg.to_yaml()
}

/// Parse a project YAML and lower it to the Fortran namelist the engine reads.
#[tauri::command]
fn project_to_namelist(yaml: String) -> Result<String, String> {
    let cfg = ProjectConfig::from_yaml(&yaml)?;
    Ok(cfg.lower().to_namelist())
}

/// Validate a project YAML — returns the canonical re-serialized YAML on success,
/// or a human-readable parse error. Used by the editor's "Validate" action.
#[tauri::command]
fn validate_project(yaml: String) -> Result<String, String> {
    ProjectConfig::from_yaml(&yaml)?.to_yaml()
}

// Slice 1 only handles the global domain; regional bbox/circle comes from the
// map control in a later slice. Kept as a helper so the call sites are explicit.
fn domain_or_default(_args: &[f64]) -> DomainConfig {
    DomainConfig::Global
}

/// Summarize a project YAML for the UI (name, intent, resolution, data layers).
#[tauri::command]
fn project_summary(yaml: String) -> Result<ProjectSummary, String> {
    let cfg = ProjectConfig::from_yaml(&yaml)?;
    let (nxp, approx_km) = match cfg.target.resolution {
        ResolutionSpec::Nxp(n) => (Some(n), None),
        ResolutionSpec::ApproxKm(k) => (None, Some(k)),
    };
    let domain = match &cfg.domain {
        DomainConfig::Global => "global",
        _ => "regional",
    }
    .to_string();
    let bbox = match &cfg.domain {
        DomainConfig::Regional {
            shape: RegionShape::Bbox { w, e, n, s },
            ..
        } => Some([*w, *e, *s, *n]),
        _ => None,
    };
    let on_violation = match cfg.quality.on_violation {
        ViolationPolicy::Block => "block",
        ViolationPolicy::Warn => "warn",
    }
    .to_string();
    let layers = cfg
        .data_layers
        .iter()
        .map(|l| LayerSummary {
            id: l.id.clone(),
            role: role_label(&l.role),
            path: l.path.clone(),
            enabled: l.enabled,
            wants_folder: matches!(
                l.role,
                ProjectLayerRole::MeritHydro | ProjectLayerRole::Cama
            ),
        })
        .collect();
    Ok(ProjectSummary {
        name: cfg.metadata.name.clone(),
        intent: intent_id(cfg.target.intent),
        domain,
        nxp,
        approx_km,
        bbox,
        min_angle_deg: cfg.quality.min_angle_deg,
        on_violation,
        refine_enabled: cfg.refinement.enabled,
        max_passes: cfg.refinement.max_passes,
        layers,
    })
}

/// Set a data layer's path + enabled flag, returning the updated YAML.
#[tauri::command]
fn set_layer_path(yaml: String, id: String, path: String, enabled: bool) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    let mut found = false;
    for l in cfg.data_layers.iter_mut() {
        if l.id == id {
            l.path = path.clone();
            l.enabled = enabled;
            found = true;
        }
    }
    if !found {
        return Err(format!("no data layer with id '{id}'"));
    }
    cfg.to_yaml()
}

/// Set the domain to global, returning the updated YAML.
#[tauri::command]
fn set_domain_global(yaml: String) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.domain = DomainConfig::Global;
    cfg.to_yaml()
}

/// Set the domain to a regional bounding box, returning the updated YAML.
#[tauri::command]
fn set_domain_bbox(
    yaml: String,
    w: f64,
    e: f64,
    s: f64,
    n: f64,
    sea_ratio: Option<f64>,
) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox { w, e, n, s },
        sea_ratio,
    };
    cfg.to_yaml()
}

/// Set the quality gate (min angle + on-violation policy), returning the YAML.
#[tauri::command]
fn set_quality(yaml: String, min_angle_deg: f64, block: bool) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.quality.min_angle_deg = min_angle_deg;
    cfg.quality.on_violation = if block {
        ViolationPolicy::Block
    } else {
        ViolationPolicy::Warn
    };
    cfg.to_yaml()
}

/// Set whether refinement runs and how many passes. `enabled=false` yields a
/// uniform mesh (no source data needed); `max_passes` is clamped to ≥1.
#[tauri::command]
fn set_refinement(yaml: String, enabled: bool, max_passes: u8) -> Result<String, String> {
    let mut cfg = ProjectConfig::from_yaml(&yaml)?;
    cfg.refinement.enabled = enabled;
    cfg.refinement.max_passes = max_passes.max(1);
    cfg.to_yaml()
}

// The three commands below open native dialogs. They are `async` so Tauri runs
// them off the main thread — `blocking_*` on the UI thread would deadlock.

/// Native file picker for a data-layer source. Returns the chosen path, or
/// `None` if the user cancels.
#[tauri::command]
async fn pick_data_file(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter(
            "Geospatial data",
            &["nc", "nc4", "tif", "tiff", "grib", "grib2", "shp", "bin", "dat", "txt"],
        )
        .blocking_pick_file()
        .map(|p| p.to_string())
}

/// Native folder picker for tiled data-layer sources (MERIT-Hydro, CaMa).
#[tauri::command]
async fn pick_data_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

/// Open a project file (YAML or JSON), validate it, return canonical YAML.
#[tauri::command]
async fn open_project(app: tauri::AppHandle) -> Result<Option<OpenedProject>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("EarthMesh project", &["yaml", "yml", "json"])
        .blocking_pick_file();
    let Some(fp) = picked else { return Ok(None) };
    let path = fp.to_string();
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
    let cfg = ProjectConfig::from_yaml(&text)
        .or_else(|_| ProjectConfig::from_json(&text))
        .map_err(|e| format!("parse {path}: {e}"))?;
    let yaml = cfg.to_yaml()?;
    Ok(Some(OpenedProject { path, yaml }))
}

/// Save a validated project YAML via a native dialog. Returns the chosen path.
#[tauri::command]
async fn save_project(app: tauri::AppHandle, yaml: String) -> Result<Option<String>, String> {
    let cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    let yaml = cfg.to_yaml()?;
    let picked = app
        .dialog()
        .file()
        .add_filter("EarthMesh project", &["yaml", "yml"])
        .blocking_save_file();
    let Some(fp) = picked else { return Ok(None) };
    let path = fp.to_string();
    std::fs::write(&path, yaml.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    Ok(Some(path))
}

/// Open a folder/file in the OS file manager (Finder / Explorer / xdg-open).
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).exists() {
        return Err(format!("path does not exist: {path}"));
    }
    #[cfg(target_os = "macos")]
    let prog = "open";
    #[cfg(target_os = "windows")]
    let prog = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let prog = "xdg-open";
    std::process::Command::new(prog)
        .arg(&path)
        .spawn()
        .map_err(|e| format!("open {path}: {e}"))?;
    Ok(())
}

/// Read + validate a project file at a known path (no dialog) — used to reopen
/// a recent project. Returns canonical YAML.
#[tauri::command]
fn read_project(path: String) -> Result<OpenedProject, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))?;
    let cfg = ProjectConfig::from_yaml(&text)
        .or_else(|_| ProjectConfig::from_json(&text))
        .map_err(|e| format!("parse {path}: {e}"))?;
    let yaml = cfg.to_yaml()?;
    Ok(OpenedProject { path, yaml })
}

/// Locate the mesh-generator binary, in priority order:
///   1. `$EARTHMESH_MKGRD` (explicit override),
///   2. well-known build outputs relative to the repo root — `make build` copies
///      the CLI to `<repo>/mkgrd.x`; cargo leaves `earthmesh_cli` in its target
///      dirs — so a freshly built tree "just works" with no configuration,
///   3. next to the running executable (installed / bundled case),
///   4. bare `mkgrd.x`, letting the OS search `PATH`.
fn resolve_mkgrd() -> String {
    // Run the engine from a clean temp dir. The static netcdf/HDF5 build SIGKILLs
    // (OOM) when executed from certain source directories (observed in the dev
    // git-repo root) — an environment-level interaction with the C libraries, not a
    // logic bug. A copy under temp_dir runs reliably, so stage one and return it.
    let found = resolve_mkgrd_path();
    let src = std::path::Path::new(&found);
    if !src.is_file() {
        return found;
    }
    let dst = std::env::temp_dir().join("earthmesh_studio_engine.x");
    let stale = match (std::fs::metadata(src), std::fs::metadata(&dst)) {
        (Ok(s), Ok(d)) => {
            // Refresh when the built engine differs in size OR is newer than the
            // staged copy. A size-only check silently kept a stale engine after a
            // rebuild that happened to land on the same byte count — so a fresh
            // `make build` looked like it "did nothing" in the GUI.
            let src_newer = match (s.modified(), d.modified()) {
                (Ok(sm), Ok(dm)) => sm > dm,
                _ => true,
            };
            s.len() != d.len() || src_newer
        }
        _ => true,
    };
    if stale && std::fs::copy(src, &dst).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755));
        }
    }
    if dst.is_file() {
        return dst.to_string_lossy().into_owned();
    }
    found
}

fn resolve_mkgrd_path() -> String {
    // Honor an explicit override, but only if it points at a real file — a
    // stale or placeholder $EARTHMESH_MKGRD (e.g. "/path/to/mkgrd.x") must not
    // shadow a real build; fall through to discovery instead.
    if let Ok(p) = std::env::var("EARTHMESH_MKGRD") {
        let p = p.trim();
        if !p.is_empty() && std::path::Path::new(p).is_file() {
            return p.to_string();
        }
    }
    // CARGO_MANIFEST_DIR is <repo>/gui-tauri/src-tauri at build time.
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let mut roots: Vec<std::path::PathBuf> = vec![
        repo.clone(),
        repo.join("rust/earthmesh_cli/target/release"),
        repo.join("rust/earthmesh_cli/target/debug"),
        repo.join("target/release"),
        repo.join("target/debug"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    let names = ["mkgrd.x", "earthmesh_cli", "earthmesh_cli.exe", "mkgrd.exe"];
    for root in &roots {
        for n in &names {
            let cand = root.join(n);
            if cand.is_file() {
                return cand
                    .canonicalize()
                    .unwrap_or(cand)
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }
    "mkgrd.x".to_string()
}

/// Run the mesh generator on a composed project YAML. Streams stdout/stderr to
/// the frontend as `mkgrd://log` events and returns the exit status + output
/// dir. Async so the (blocking) child wait runs off the UI thread; the reader
/// threads emit log lines independently as the process runs.
#[tauri::command]
async fn run_project(
    app: tauri::AppHandle,
    yaml: String,
    outdir: Option<String>,
) -> Result<RunResult, String> {
    // Validate, then lower to the Fortran namelist the engine actually reads:
    // mkgrd.x takes `<mkgrd.nml>` as a positional argument (no `--project` flag),
    // so we do the lowering here rather than relying on the CLI to do it.
    let cfg = ProjectConfig::from_yaml(&yaml).map_err(|e| format!("invalid project: {e}"))?;
    let mut lowered = cfg.lower();
    // Stabilize the spring smoothing. The config default (beta=1.2, relax=0.04) is
    // more aggressive than OLAM's proven-stable values and can OVER-relax: the
    // spring overshoots and folds the mesh locally, leaving overlapping/inverted
    // triangles that render as "fan" artifacts (the gridinit topology stays valid,
    // so nothing catches it). OLAM's ocean defaults (beta=1.0, relax=0.035) relax
    // cleanly. Only override the engine's defaults — a project that set its own
    // values keeps them (the lowering doesn't, today, so this is always applied).
    if lowered.mkgrd.beta == 1.2 {
        lowered.mkgrd.beta = 1.0;
    }
    if lowered.mkgrd.relax == 0.04 {
        lowered.mkgrd.relax = 0.035;
    }
    // The default config seeds several inputs with a "/tmp" placeholder. The
    // engine treats those as real (landtype: opens it as NetCDF; mode_file: a
    // "/tmp" dir "exists", so it tries to ingest an existing mesh). Normalize any
    // non-file path to 'none' so the engine skips landtype and generates a fresh
    // base mesh instead of ingesting a bogus one.
    {
        fn is_real_file(p: &str) -> bool {
            let p = p.trim();
            !p.is_empty()
                && !p.eq_ignore_ascii_case("none")
                && p != "/tmp"
                && std::path::Path::new(p).is_file()
        }
        if !is_real_file(&lowered.mkgrd.landtype_file) {
            lowered.mkgrd.landtype_file = "none".to_string();
        }
        if !is_real_file(&lowered.mkgrd.mode_file) {
            lowered.mkgrd.mode_file = "none".to_string();
        }
    }

    // `outdir` is the BASE output path (the user's choice, or a temp dir). Every
    // file for this run lives in <base>/<project name>/ so outputs are grouped.
    let base = outdir
        .map(|p| p.trim_end_matches('/').to_string())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            std::env::temp_dir()
                .join(format!("earthmesh_run_{ts}"))
                .to_string_lossy()
                .into_owned()
        });
    // Project name -> folder name (sanitized) = the engine's experiment_name.
    let exp: String = {
        let s: String = cfg
            .metadata
            .name
            .trim()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        if s.is_empty() {
            "mesh".to_string()
        } else {
            s
        }
    };
    let run_dir = std::path::Path::new(&base).join(&exp);
    std::fs::create_dir_all(&run_dir).map_err(|e| format!("mkdir {}: {e}", run_dir.display()))?;
    let run_dir_str = run_dir.to_string_lossy().into_owned();

    // The engine CLEARS + recreates its output dir (`file_dir`). Put it in an
    // "output/" SUBfolder of run_dir so the engine never deletes run_dir itself —
    // which holds mkgrd.nml + project.yaml. file_dir = base_dir + experiment_name + "/".
    lowered.mkgrd.base_dir = format!("{run_dir_str}/");
    lowered.mkgrd.experiment_name = "output".to_string();
    let file_dir = run_dir.join("output");
    for sub in ["", "result", "contain", "restart"] {
        let _ = std::fs::create_dir_all(file_dir.join(sub));
    }

    // Regional bbox domain: the engine reads the region from a `.nml` mask file
    // (mask_domain_type='bbox' -> parse_bbox_mask_nml: `bbox_num`/`bbox_refine`
    // then rows of `west east north south`). Generate it from the project's bbox
    // so a regional run needs no external mask file and no netcdf in the GUI.
    if let DomainConfig::Regional {
        shape: RegionShape::Bbox { w, e, n, s },
        ..
    } = &cfg.domain
    {
        let mask_nml = run_dir.join("domain_bbox.nml");
        let body = format!("bbox_num = 1\nbbox_refine = 1\n{w} {e} {n} {s}\n");
        match std::fs::write(&mask_nml, body) {
            Ok(()) => {
                // From-scratch regional CLIP, no refinement. A non-global mask
                // domain (mask_domain_global=.false. + a bbox source) makes the
                // engine subset the base mesh to the box: with refine OFF the run
                // takes the dedicated pure-clip dispatch branch
                // (run_mkgrd_regional_clip_base_namelist), which generates the
                // global base grid and keeps only the in-box cells via the shared
                // write_regional_gridfile writer. One pass, every mesh type, and
                // no netcdf in the GUI — the engine parses this plain-text .nml.
                //
                // refine is forced OFF so the OLAM refine path (which would demand
                // a separate refinement region) is bypassed; refine_spc/cal are
                // cleared for the same reason. NOT mask_restart — that path is a
                // continuation that never clips atmos.
                lowered.mkgrd.mask_domain_global = false;
                lowered.mkgrd.mask_domain_type = "bbox".to_string();
                lowered.mkgrd.mask_domain_fprefix =
                    run_dir.join("domain_bbox").to_string_lossy().into_owned();
                lowered.mkgrd.mask_restart = false;
                lowered.mkgrd.refine = false;
                lowered.refine.refine_spc = false;
                lowered.refine.refine_cal = false;
                let _ = app.emit(
                    "mkgrd://log",
                    format!("✓ regional bbox clip (W {w}, E {e}, N {n}, S {s}) — keeping only in-box cells (no refinement)"),
                );
            }
            Err(err) => {
                let _ = app.emit("mkgrd://log", format!("⚠ could not write bbox mask: {err}"));
            }
        }
    }

    let namelist = lowered.to_namelist();
    // project.yaml (provenance) + mkgrd.nml (engine input) both live in run_dir.
    let yaml_path = run_dir.join("project.yaml");
    std::fs::write(&yaml_path, yaml.as_bytes())
        .map_err(|e| format!("write {}: {e}", yaml_path.display()))?;
    let nml_path = run_dir.join("mkgrd.nml");
    std::fs::write(&nml_path, namelist.as_bytes())
        .map_err(|e| format!("write {}: {e}", nml_path.display()))?;

    let bin = resolve_mkgrd();
    // Surface the staged engine's size + mtime so a stale temp copy (an old engine
    // still being run after a rebuild) is visible at a glance.
    if let Ok(md) = std::fs::metadata(&bin) {
        let mtime = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = app.emit(
            "mkgrd://log",
            format!("engine: {bin}  ({} bytes · mtime {mtime})", md.len()),
        );
    }
    if let Ok(p) = std::env::var("EARTHMESH_MKGRD") {
        let p = p.trim().to_string();
        if !p.is_empty() && !std::path::Path::new(&p).is_file() {
            let _ = app.emit(
                "mkgrd://log",
                format!("note: $EARTHMESH_MKGRD='{p}' is not a file — ignoring it; using '{bin}'."),
            );
        }
    }
    // Pre-run sanity: the engine opens NetCDF inputs, so a placeholder/missing
    // path yields a cryptic `netcdf -51` error. Surface it clearly up front.
    {
        if lowered.mkgrd.landtype_file == "none" {
            let _ = app.emit(
                "mkgrd://log",
                "note: no land-cover file set (landtype_file='none') — fine for atmosphere/uniform \
                 meshes; land & ocean meshes need one (set it in step 3, Data Layers)."
                    .to_string(),
            );
            // Only a GLOBAL mesh with refinement on needs data. A regional run is
            // a pure clip (refine off, no data) and a global uniform run is fine
            // too — so only warn for the global + refine case.
            if lowered.mkgrd.refine && lowered.mkgrd.mask_domain_global {
                let _ = app.emit(
                    "mkgrd://log",
                    "⚠ refinement is on but no source data is set — the engine needs a landtype or \
                     threshold input to refine. For a data-free test, run a uniform mesh (refine off)."
                        .to_string(),
                );
            }
        }
        for l in &cfg.data_layers {
            if l.enabled && l.path.trim().is_empty() {
                let _ = app.emit(
                    "mkgrd://log",
                    format!("⚠ data layer '{}' is enabled but has no file set.", l.id),
                );
            }
        }
        // Regional domains: the engine defines the region from a mask SOURCE FILE
        // (.nml/.nc via mask_domain_fprefix), not from bbox coordinates. Without a
        // real mask file the run fails with "unsupported mask source extension".
        if !lowered.mkgrd.mask_domain_global {
            let mf = lowered.mkgrd.mask_domain_fprefix.trim();
            if mf.is_empty() || mf == "/tmp" || mf.eq_ignore_ascii_case("none") {
                let _ = app.emit(
                    "mkgrd://log",
                    "⚠ regional domain needs a mask source file — the engine derives the region from \
                     .nml/.nc mask files (via mask_domain_fprefix), not from bbox coordinates. None is \
                     set, so this run will fail. Use a global domain, or provide a mask source."
                        .to_string(),
                );
            }
        }
    }
    let _ = app.emit(
        "mkgrd://log",
        format!(
            "--- generated mkgrd.nml (file_dir={}) ---\n{namelist}--- end mkgrd.nml ---",
            lowered.mkgrd.file_dir()
        ),
    );
    let _ = app.emit("mkgrd://log", format!("$ {bin} {}", nml_path.display()));

    let mut child = Command::new(&bin)
        .arg(&nml_path)
        .current_dir(&run_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "could not start '{bin}': {e}. Build mkgrd.x and put it on PATH, \
                 or set EARTHMESH_MKGRD to its full path."
            )
        })?;
    // Record the PID so `kill_run` can stop this engine run on request.
    *RUNNING_CHILD_PID.lock().unwrap() = Some(child.id());

    let out = child.stdout.take();
    let err = child.stderr.take();
    let a1 = app.clone();
    let gridfile_seen = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let gf_capture = gridfile_seen.clone();
    let t1 = std::thread::spawn(move || {
        if let Some(o) = out {
            for line in BufReader::new(o).lines().map_while(Result::ok) {
                // The engine prints `gridfile=<path>` for the mesh it produced.
                if let Some(rest) = line.strip_prefix("gridfile=") {
                    *gf_capture.lock().unwrap() = Some(rest.trim().to_string());
                }
                let _ = a1.emit("mkgrd://log", line);
            }
        }
    });
    let a2 = app.clone();
    let t2 = std::thread::spawn(move || {
        if let Some(e) = err {
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                let _ = a2.emit("mkgrd://log", format!("[stderr] {line}"));
            }
        }
    });

    let status = child.wait().map_err(|e| format!("wait failed: {e}"))?;
    *RUNNING_CHILD_PID.lock().unwrap() = None;
    let _ = t1.join();
    let _ = t2.join();
    let code = status.code();
    let _ = app.emit(
        "mkgrd://log",
        format!(
            "— exited with {}",
            code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into())
        ),
    );
    let gridfile = gridfile_seen.lock().unwrap().clone();
    Ok(RunResult {
        ok: status.success(),
        code,
        outdir: run_dir_str,
        gridfile,
    })
}

/// PID of the mesh-generator child currently running (if any). `run_project` sets
/// it on spawn and clears it on exit, so `kill_run` can terminate the run.
static RUNNING_CHILD_PID: std::sync::Mutex<Option<u32>> = std::sync::Mutex::new(None);

/// Terminate the running mesh-generator process, if one is active. Returns whether
/// a process was signalled. Kills by PID — SIGKILL on unix, `taskkill /F /T` on
/// Windows (which also reaps any child threads/processes).
#[tauri::command]
fn kill_run() -> Result<bool, String> {
    let pid = *RUNNING_CHILD_PID.lock().unwrap();
    let Some(pid) = pid else {
        return Ok(false);
    };
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .status();
    }
    *RUNNING_CHILD_PID.lock().unwrap() = None;
    Ok(true)
}

/// Distribution summary for a metric (mean ± std over [min, max]).
#[derive(Serialize, Default, Clone, Copy)]
struct Stat {
    min: f64,
    max: f64,
    mean: f64,
    std: f64,
    cv: f64,
}

/// One quality gate: a metric value + its pass/warn/fail level.
#[derive(Serialize)]
struct Gate {
    metric: String,
    value: f64,
    level: String,
}

/// A mesh-quality summary for the dashboard, parsed from `quality_summary.json`.
#[derive(Serialize)]
struct MeshQuality {
    verdict: String,
    cell_count: i64,
    vertex_count: i64,
    edge_count: i64,
    min_angle_deg: f64,
    max_angle_deg: f64,
    // Per-metric distribution summaries (for the box/range charts).
    cell_area: Stat,
    edge_length_km: Stat,
    aspect_ratio: Stat,
    compactness: Stat,
    // Degeneracy counts.
    zero_area: i64,
    negative_area: i64,
    self_intersection: i64,
    invalid_polygon: i64,
    max_adjacent_resolution_ratio: f64,
    /// (name, count) for each topology issue counter.
    topology: Vec<(String, i64)>,
    gates: Vec<Gate>,
    /// Gate metrics + topology issues at warn/fail level, as "name [level]".
    warnings: Vec<String>,
    report_path: Option<String>,
    worst_cells_path: Option<String>,
}

/// Parse `quality_summary.json` text into a [`MeshQuality`]. `dir` is the report dir,
/// used to locate the .md / worst-cells artifacts written alongside it.
fn parse_quality_summary(text: &str, dir: &std::path::Path) -> Result<MeshQuality, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("parse quality json: {e}"))?;
    let geom = &v["geometry"];
    let mut warnings = Vec::new();
    if let Some(gates) = v["gates"].as_array() {
        for g in gates {
            let level = g["level"].as_str().unwrap_or("");
            if level == "warn" || level == "fail" {
                warnings.push(format!("{} [{level}]", g["metric"].as_str().unwrap_or("?")));
            }
        }
    }
    if let Some(issues) = v["topology_issues"].as_array() {
        for it in issues {
            let sev = it["severity"].as_str().unwrap_or("");
            if sev == "warn" || sev == "fail" {
                warnings.push(format!(
                    "topology: {} [{sev}]",
                    it["issue_type"].as_str().unwrap_or("?")
                ));
            }
        }
    }
    let exists = |name: &str| {
        let p = dir.join(name);
        p.exists().then(|| p.to_string_lossy().into_owned())
    };
    let stat = |key: &str| -> Stat {
        let s = &geom[key];
        Stat {
            min: s["min"].as_f64().unwrap_or(0.0),
            max: s["max"].as_f64().unwrap_or(0.0),
            mean: s["mean"].as_f64().unwrap_or(0.0),
            std: s["std"].as_f64().unwrap_or(0.0),
            cv: s["cv"].as_f64().unwrap_or(0.0),
        }
    };
    let gates = v["gates"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|g| Gate {
                    metric: g["metric"].as_str().unwrap_or("?").to_string(),
                    value: g["value"].as_f64().unwrap_or(0.0),
                    level: g["level"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let topo = &v["topology"];
    let topology = [
        "duplicate_edge_count",
        "dangling_edge_count",
        "orphan_cell_count",
        "neighbor_reciprocity_failure_count",
        "abnormal_polygon_edge_count",
        "isolated_refined_cell_count",
        "transition_continuity_warning_count",
        "invalid_vertex_index_count",
        "invalid_cell_index_count",
    ]
    .iter()
    .map(|k| (k.trim_end_matches("_count").to_string(), topo[*k].as_i64().unwrap_or(0)))
    .collect::<Vec<_>>();
    Ok(MeshQuality {
        verdict: v["verdict"].as_str().unwrap_or("unknown").to_string(),
        cell_count: geom["cell_count"].as_i64().unwrap_or(0),
        vertex_count: geom["vertex_count"].as_i64().unwrap_or(0),
        edge_count: geom["edge_count"].as_i64().unwrap_or(0),
        min_angle_deg: geom["min_angle_deg"].as_f64().unwrap_or(0.0),
        max_angle_deg: geom["max_angle_deg"].as_f64().unwrap_or(0.0),
        cell_area: stat("cell_area"),
        edge_length_km: stat("edge_length_km"),
        aspect_ratio: stat("aspect_ratio"),
        compactness: stat("compactness"),
        zero_area: geom["zero_area_cell_count"].as_i64().unwrap_or(0),
        negative_area: geom["negative_area_cell_count"].as_i64().unwrap_or(0),
        self_intersection: geom["self_intersection_count"].as_i64().unwrap_or(0),
        invalid_polygon: geom["invalid_polygon_count"].as_i64().unwrap_or(0),
        max_adjacent_resolution_ratio: topo["max_adjacent_resolution_ratio"].as_f64().unwrap_or(0.0),
        topology,
        gates,
        warnings,
        report_path: exists("quality_report.md"),
        worst_cells_path: exists("worst_cells.geojson"),
    })
}

/// Run `mkgrd.x --mesh-quality <gridfile> <dir>` and parse the resulting
/// `quality_summary.json` for the Quality dashboard.
#[tauri::command]
fn mesh_quality(gridfile: String, kind: Option<String>) -> Result<MeshQuality, String> {
    let gf = std::path::Path::new(&gridfile);
    if !gf.is_file() {
        return Err(format!("gridfile not found: {gridfile}"));
    }
    let dir = gf
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    // Measure hexagon cells for hex/atmos (MPAS) meshes, triangles for FVCOM —
    // matching the cell view the map renders, so the reported angles are the real
    // cell angles (≈120° for hexagons), not the dual triangles (≈60°).
    let kind = if kind.as_deref() == Some("tri") { "tri" } else { "hex" };
    let bin = resolve_mkgrd();
    let out = Command::new(&bin)
        .arg("--mesh-quality")
        .arg(&gridfile)
        .arg(&dir)
        .arg("--kind")
        .arg(kind)
        .output()
        .map_err(|e| format!("run --mesh-quality ({bin}): {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "mesh-quality failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let json_path = dir.join("quality_summary.json");
    let text = std::fs::read_to_string(&json_path)
        .map_err(|e| format!("read {}: {e}", json_path.display()))?;
    parse_quality_summary(&text, &dir)
}

/// Run `mkgrd.x --gridfile-cell-polygons <gridfile> <out.geojson> --kind <hex|tri>`
/// and return the GeoJSON text for the frontend to overlay on the map.
#[tauri::command]
fn mesh_cell_polygons(
    gridfile: String,
    kind: String,
    max_cells: Option<u32>,
) -> Result<String, String> {
    let gf = std::path::Path::new(&gridfile);
    if !gf.is_file() {
        return Err(format!("gridfile not found: {gridfile}"));
    }
    let dir = gf
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let out_geojson = dir.join("mesh_cells.geojson");
    let kind = if kind == "tri" { "tri" } else { "hex" };
    let bin = resolve_mkgrd();
    let mut cmd = Command::new(&bin);
    cmd.arg("--gridfile-cell-polygons")
        .arg(&gridfile)
        .arg(&out_geojson)
        .arg("--kind")
        .arg(kind);
    if let Some(mc) = max_cells {
        cmd.arg("--max-cells").arg(mc.to_string());
    }
    let res = cmd
        .output()
        .map_err(|e| format!("run --gridfile-cell-polygons ({bin}): {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "cell-polygons failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    std::fs::read_to_string(&out_geojson)
        .map_err(|e| format!("read {}: {e}", out_geojson.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quality_summary_fields_and_warnings() {
        let json = r#"{
            "verdict": "warn",
            "geometry": { "cell_count": 1200, "vertex_count": 640, "edge_count": 1830, "min_angle_deg": 22.5 },
            "gates": [
                { "metric": "min_angle_deg", "value": 22.5, "level": "warn" },
                { "metric": "aspect_ratio", "value": 2.0, "level": "pass" }
            ],
            "topology_issues": [
                { "issue_type": "duplicate_edge", "severity": "fail", "message": "x" }
            ]
        }"#;
        let q = parse_quality_summary(json, std::path::Path::new("/no/such/dir")).unwrap();
        assert_eq!(q.verdict, "warn");
        assert_eq!(q.cell_count, 1200);
        assert_eq!(q.vertex_count, 640);
        assert_eq!(q.min_angle_deg, 22.5);
        assert!(q.warnings.iter().any(|w| w.contains("min_angle_deg [warn]")));
        assert!(q
            .warnings
            .iter()
            .any(|w| w.contains("topology: duplicate_edge [fail]")));
        // pass-level gate must not show up as a warning
        assert!(!q.warnings.iter().any(|w| w.contains("aspect_ratio")));
        assert!(q.report_path.is_none());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_intents,
            list_criteria,
            scaffold_project,
            project_to_namelist,
            validate_project,
            project_summary,
            set_layer_path,
            set_domain_global,
            set_domain_bbox,
            set_quality,
            set_refinement,
            pick_data_file,
            pick_data_folder,
            open_project,
            save_project,
            read_project,
            open_path,
            run_project,
            kill_run,
            mesh_quality,
            mesh_cell_polygons
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
