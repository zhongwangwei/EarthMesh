//! EarthMesh desktop GUI.
//!
//! Increment 5: a user-centred form. Options are reorganised by task into three
//! tabs — Basics / Refinement / Advanced — with friendly labels, only the mesh
//! shapes the engine actually supports (hex, tri), a prominent Global/Regional
//! choice, mesh-type-filtered refinement criteria, and the import/smoothing
//! plumbing tucked under Advanced. The verbatim namelist mirror is gone.

use earthmesh_core::paths::home_dir;
use earthmesh_core::{deg_to_rad, rad_to_deg, EarthmeshConfig, RefineConfig};
use eframe::egui;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

mod components;
mod i18n;
mod theme;
mod ui_helpers;
use i18n::{tr, Lang};

/// Engine value paired with its friendly i18n key.
const MESH_TYPES: &[(&str, &str)] = &[
    ("landmesh", "mesh.land"),
    ("oceanmesh", "mesh.ocean"),
    ("atmosmesh", "mesh.atmos"),
    ("LOCmesh", "mesh.loc"),
    ("earthmesh", "mesh.earth"),
];
// Only hex and tri are implemented by the engine. The per-mesh-type sets below
// encode the domain convention (MPAS atmosphere = hexagonal; FVCOM ocean =
// triangular; land / coupled = either). The engine itself accepts hex or tri for
// any mesh type, so a loaded file keeping an off-convention shape is preserved.
const GRID_HEX: &[(&str, &str)] = &[("hex", "grid.hex")];
const GRID_TRI: &[(&str, &str)] = &[("tri", "grid.tri")];
const GRID_BOTH: &[(&str, &str)] = &[("hex", "grid.hex"), ("tri", "grid.tri")];

fn grid_modes_for(mesh_type: &str) -> &'static [(&'static str, &'static str)] {
    match mesh_type {
        "atmosmesh" => GRID_HEX,
        "oceanmesh" => GRID_TRI,
        _ => GRID_BOTH,
    }
}
const REGION_TYPES: &[&str] = &["bbox", "lambert", "close", "circle"];
const SET_DIS_TYPES: &[&str] = &["linear", "nonlinear1", "nonlinear2", "nonlinear3"];
const MODE_FILE_DESCS: &[&str] = &["none", "EarthMesh", "MPAS", "IAP-Ocean", "FVCOM"];
const SPECIFIED_REFINE_LEVEL_MAX: i32 = 5;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn app_resource_candidates(relative: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(root) = std::env::var("EARTHMESH_RESOURCE_DIR") {
        candidates.push(PathBuf::from(root).join(relative));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../Resources").join(relative));
            candidates.push(dir.join(relative));
        }
    }
    candidates.push(workspace_root().join(relative));
    candidates
}

fn first_existing_resource(relative: &str) -> Option<PathBuf> {
    app_resource_candidates(relative)
        .into_iter()
        .find(|path| path.exists())
}

fn examples_root() -> PathBuf {
    first_existing_resource("examples").unwrap_or_else(|| workspace_root().join("examples"))
}

fn default_example_path() -> PathBuf {
    first_existing_resource("examples/00_quickstart_n16.nml")
        .unwrap_or_else(|| workspace_root().join("examples/00_quickstart_n16.nml"))
}

fn runtime_workdir() -> PathBuf {
    if let Ok(path) = std::env::var("EARTHMESH_WORKDIR") {
        return PathBuf::from(path);
    }
    let dev_root = workspace_root();
    if dev_root.join("examples").exists() {
        return dev_root;
    }
    if let Some(home) = home_dir() {
        return home.join("EarthMesh");
    }
    std::env::current_dir().unwrap_or(dev_root)
}

fn output_root() -> PathBuf {
    runtime_workdir()
}

fn resolve_case_base_dir(base_dir: &str) -> PathBuf {
    let trimmed = base_dir.trim();
    let raw = PathBuf::from(trimmed);
    if raw.is_absolute() {
        raw
    } else {
        output_root().join(trimmed.trim_start_matches("./").trim_end_matches('/'))
    }
}

fn resolve_runtime_file_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    let raw = PathBuf::from(trimmed);
    if raw.is_absolute() {
        raw
    } else {
        runtime_workdir().join(trimmed.trim_start_matches("./"))
    }
}

fn unique_stage_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("earthmesh_gui_{}_{}", std::process::id(), stamp))
}

/// Locate the bundled offline basemap (Protomaps vector PMTiles). Checks, in
/// order: an explicit env override, the macOS `.app` Resources dir (next to the
/// executable), and the dev `assets/` dir. Returns the first that exists so the
/// map works both from a packaged bundle and from `cargo run`.
fn basemap_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("EARTHMESH_BASEMAP") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // EarthMesh.app/Contents/MacOS/<bin> → Contents/Resources/world.pmtiles
            candidates.push(dir.join("../Resources/world.pmtiles"));
            candidates.push(dir.join("world.pmtiles"));
        }
    }
    candidates.extend(app_resource_candidates("world.pmtiles"));
    candidates.push(workspace_root().join("rust/earthmesh_gui/assets/world.pmtiles"));
    candidates.into_iter().find(|p| p.exists())
}

fn output_formats_for(mesh_type: &str) -> &'static [&'static str] {
    match mesh_type {
        "atmosmesh" => &["MPAS", "MPAS-Simple"],
        "oceanmesh" => &["FVCOM"],
        _ => &["CoLM"],
    }
}

fn refinement_supported_for(_mesh_type: &str, _output_format: &str) -> bool {
    true
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Basics,
    Refinement,
    Advanced,
}

fn tab_nav_key(tab: Tab) -> &'static str {
    match tab {
        Tab::Basics => "nav.basics",
        Tab::Refinement => "nav.refinement",
        Tab::Advanced => "nav.advanced",
    }
}

/// Option label key paired with the tab that hosts it — powers the search.
const FIELD_INDEX: &[(&str, Tab)] = &[
    ("f.mesh_type", Tab::Basics),
    ("f.mode_grid", Tab::Basics),
    ("f.nxp", Tab::Basics),
    ("f.output_format", Tab::Basics),
    ("f.domain_mode", Tab::Basics),
    ("f.domain_shape", Tab::Basics),
    ("f.domain_prefix", Tab::Basics),
    ("f.refine_master", Tab::Basics),
    ("f.threads", Tab::Basics),
    ("f.expnme", Tab::Basics),
    ("f.base_dir", Tab::Basics),
    ("f.refine_spc", Tab::Refinement),
    ("f.max_iter_spc", Tab::Refinement),
    ("f.spc_shape", Tab::Refinement),
    ("f.spc_prefix", Tab::Refinement),
    ("f.refine_cal", Tab::Refinement),
    ("f.max_iter_cal", Tab::Refinement),
    ("f.cal_shape", Tab::Refinement),
    ("f.cal_prefix", Tab::Refinement),
    ("f.threshold_dir", Tab::Refinement),
    ("f.landtype_file", Tab::Basics),
    ("f.weak_concav", Tab::Refinement),
    ("f.is_transition", Tab::Refinement),
    ("f.iter_d", Tab::Refinement),
    ("f.halo", Tab::Refinement),
    ("f.max_transition", Tab::Refinement),
    ("c.num_landtypes", Tab::Refinement),
    ("c.area_mainland", Tab::Refinement),
    ("c.lai_m", Tab::Refinement),
    ("c.lai_s", Tab::Refinement),
    ("c.slope_m", Tab::Refinement),
    ("c.slope_s", Tab::Refinement),
    ("c.ks_m", Tab::Refinement),
    ("c.ks_s", Tab::Refinement),
    ("c.ksol_m", Tab::Refinement),
    ("c.ksol_s", Tab::Refinement),
    ("c.tkdry_m", Tab::Refinement),
    ("c.tkdry_s", Tab::Refinement),
    ("c.tksatf_m", Tab::Refinement),
    ("c.tksatf_s", Tab::Refinement),
    ("c.tksatu_m", Tab::Refinement),
    ("c.tksatu_s", Tab::Refinement),
    ("c.sea_ratio", Tab::Refinement),
    ("c.sst_m", Tab::Refinement),
    ("c.sst_s", Tab::Refinement),
    ("c.ssh_m", Tab::Refinement),
    ("c.ssh_s", Tab::Refinement),
    ("c.eke_m", Tab::Refinement),
    ("c.eke_s", Tab::Refinement),
    ("c.seaslope_m", Tab::Refinement),
    ("c.seaslope_s", Tab::Refinement),
    ("c.typhoon_m", Tab::Refinement),
    ("c.typhoon_s", Tab::Refinement),
    ("f.mode_file", Tab::Advanced),
    ("f.mode_file_desc", Tab::Advanced),
    ("f.gridnum", Tab::Advanced),
    ("f.niter", Tab::Advanced),
    ("f.beta", Tab::Advanced),
    ("f.relax", Tab::Advanced),
    ("f.niter_refine", Tab::Advanced),
    ("f.spring_global", Tab::Advanced),
    ("f.num_rc", Tab::Advanced),
    ("f.set_dis", Tab::Advanced),
    ("f.spring_regional", Tab::Advanced),
    ("f.vertex_layers", Tab::Advanced),
    ("f.patch_on", Tab::Advanced),
    ("f.patch_shape", Tab::Advanced),
    ("f.patch_prefix", Tab::Advanced),
    ("f.mask_restart", Tab::Advanced),
    ("f.sea_ratio", Tab::Advanced),
    ("f.isolated_ocean", Tab::Advanced),
];

fn collect_nml(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_nml(&p, out);
            } else if p.extension().is_some_and(|x| x == "nml") {
                out.push(p);
            }
        }
    }
}

fn bundled_templates() -> Vec<(String, PathBuf)> {
    let examples = examples_root();
    let mut nmls = Vec::new();
    collect_nml(&examples, &mut nmls);
    nmls.sort();
    nmls.into_iter()
        .map(|p| {
            let label = p
                .strip_prefix(&examples)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            (label, p)
        })
        .collect()
}

fn has_mkrefine_block(contents: &str) -> bool {
    contents
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("&mkrefine"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunResult {
    output_dir: PathBuf,
    note: Option<String>,
    started_at: SystemTime,
}

impl RunResult {
    fn new(output_dir: PathBuf, note: impl Into<String>, started_at: SystemTime) -> Self {
        let note = note.into();
        Self {
            output_dir,
            note: (!note.is_empty()).then_some(note),
            started_at,
        }
    }
}

enum RunMsg {
    Done(Result<RunResult, String>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RefineBboxRegion {
    bounds: [f64; 4],
    level: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RefineCircleRegion {
    circle: [f64; 3],
    level: i32,
}

#[derive(Clone, Debug, PartialEq)]
struct RefineCloseRegion {
    points: Vec<[f64; 2]>,
    level: i32,
}

struct EarthMeshApp {
    mkgrd: EarthmeshConfig,
    refine: RefineConfig,
    loaded_path: Option<PathBuf>,
    tab: Tab,
    lang: Lang,
    search: String,
    recent: Vec<PathBuf>,
    log: Vec<String>,
    status_key: &'static str,
    status_detail: String,
    running: bool,
    run_rx: Option<Receiver<RunMsg>>,
    prog_rx: Option<Receiver<(String, usize, usize)>>,
    cancel_flag: Option<Arc<AtomicBool>>,
    progress: Option<(String, usize, usize)>,
    last_phase: String,
    dom_bbox: [f64; 4],       // west, east, north, south (degrees)
    dom_circle: [f64; 3],     // center lon, center lat, radius (km)
    dom_close: Vec<[f64; 2]>, // close-curve polygon vertices [lon, lat]
    refine_bboxes: Vec<RefineBboxRegion>,
    refine_circles: Vec<RefineCircleRegion>,
    refine_closes: Vec<RefineCloseRegion>,
    results_detached: bool,
    output_files: Vec<PathBuf>,
    mesh_view: Option<earthmesh_cli::GridfileMeshPoints>,
    cell_classes: Vec<i8>,
    tiles: Option<walkers::PmTiles>, // offline Protomaps basemap; None → wireframe fallback
    map_memory: walkers::MapMemory,
    frame_pending: bool, // re-frame the map on the next render, once the widget size is known
    gen_output: bool, // also write the selected model's standard file (MPAS/FVCOM…) after the run
    native_olam_mkgrd: String,
    // R9 GUI workflow / polish (additive; do not affect engine behavior)
    expert_mode: bool,
    theme_dark: bool,
    project_name: String,
    target_template: usize,
    // hydro-workflow trigger (additive; invokes the tested earthmesh_cli::run_hydro_workflow)
    hydro_cells: Option<PathBuf>,
    hydro_corridors: Option<PathBuf>,
    hydro_out_dir: Option<PathBuf>,
    hydro_mesh: Option<PathBuf>,
    hydro_landtype: Option<PathBuf>,
    hydro_wf_running: bool,
    hydro_wf_rx: Option<Receiver<Result<earthmesh_cli::HydroWorkflowReport, String>>>,
}

impl Default for EarthMeshApp {
    fn default() -> Self {
        let mkgrd = EarthmeshConfig {
            openmp: 4,
            ..Default::default()
        };
        let refine = RefineConfig {
            max_iter_spc: SPECIFIED_REFINE_LEVEL_MAX,
            ..Default::default()
        };
        Self {
            mkgrd,
            refine,
            loaded_path: None,
            tab: Tab::Basics,
            lang: Lang::En,
            search: String::new(),
            recent: Vec::new(),
            log: Vec::new(),
            status_key: "status.ready",
            status_detail: String::new(),
            running: false,
            run_rx: None,
            prog_rx: None,
            cancel_flag: None,
            progress: None,
            last_phase: String::new(),
            dom_bbox: [110.0, 120.0, 35.0, 20.0],
            dom_circle: [115.0, 25.0, 500.0],
            dom_close: vec![[110.0, 15.0], [125.0, 15.0], [125.0, 30.0], [110.0, 30.0]],
            refine_bboxes: vec![RefineBboxRegion {
                bounds: [110.0, 120.0, 35.0, 20.0],
                level: SPECIFIED_REFINE_LEVEL_MAX,
            }],
            refine_circles: vec![RefineCircleRegion {
                circle: [115.0, 25.0, 500.0],
                level: SPECIFIED_REFINE_LEVEL_MAX,
            }],
            refine_closes: vec![RefineCloseRegion {
                points: vec![[110.0, 15.0], [125.0, 15.0], [125.0, 30.0], [110.0, 30.0]],
                level: SPECIFIED_REFINE_LEVEL_MAX,
            }],
            results_detached: false,
            output_files: Vec::new(),
            mesh_view: None,
            cell_classes: Vec::new(),
            tiles: None,
            map_memory: walkers::MapMemory::default(),
            frame_pending: false,
            gen_output: false,
            native_olam_mkgrd: String::new(),
            expert_mode: false,
            theme_dark: false,
            project_name: String::new(),
            target_template: 0,
            hydro_cells: None,
            hydro_corridors: None,
            hydro_out_dir: None,
            hydro_mesh: None,
            hydro_landtype: None,
            hydro_wf_running: false,
            hydro_wf_rx: None,
        }
    }
}

const NATIVE_OLAM_MKGRD_FIELDS: &[&str] = &[
    "mdomain",
    "deltax",
    "ngrids",
    "gridplot_base",
    "ngrdll",
    "grdrad",
    "grdlat",
    "grdlon",
    "nsfcgrids",
    "sfcgrid_res_factor",
    "sfcgridplot_base",
    "nsfcgrdll",
    "sfcgrdrad",
    "sfcgrdlat",
    "sfcgrdlon",
];

fn native_olam_mkgrd_lines(input: &str) -> Vec<String> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            !lower.starts_with("&mkgrd") && lower != "/"
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn is_native_olam_mkgrd_line(line: &str) -> bool {
    let compact = line
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    NATIVE_OLAM_MKGRD_FIELDS.iter().any(|field| {
        compact.starts_with(&format!("nl%{field}=")) || compact.starts_with(&format!("nl%{field}("))
    })
}

fn extract_native_olam_mkgrd_lines(contents: &str) -> String {
    let mut in_mkgrd = false;
    let mut lines = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("&mkgrd") {
            in_mkgrd = true;
            continue;
        }
        if in_mkgrd && trimmed == "/" {
            in_mkgrd = false;
            continue;
        }
        if in_mkgrd && is_native_olam_mkgrd_line(trimmed) {
            lines.push(trimmed.to_string());
        }
    }
    lines.join("\n")
}

fn insert_native_olam_mkgrd_lines(mkgrd_namelist: &str, native_lines: &str) -> String {
    let native_lines = native_olam_mkgrd_lines(native_lines);
    if native_lines.is_empty() {
        return mkgrd_namelist.to_string();
    }
    let mut out = String::new();
    for line in mkgrd_namelist.lines() {
        if line.trim() == "/" {
            for native_line in &native_lines {
                out.push_str("  ");
                out.push_str(native_line);
                out.push('\n');
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Draw the mesh cell-centres on a simple equirectangular lon/lat map.
fn draw_mesh_2d(ui: &mut egui::Ui, mesh: &earthmesh_cli::GridfileMeshPoints) {
    let height = (ui.available_height() - 6.0).max(180.0);
    let (rect, _resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(18, 28, 42));
    let norm_lon = |lon: f64| ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    let to_screen = |lon: f64, lat: f64| {
        let x = rect.left() + ((norm_lon(lon) + 180.0) / 360.0) as f32 * rect.width();
        let y = rect.top() + ((90.0 - lat) / 180.0) as f32 * rect.height();
        egui::pos2(x, y)
    };
    let grid = egui::Color32::from_gray(55);
    for lon in (-180..=180).step_by(30) {
        painter.line_segment(
            [to_screen(lon as f64, 90.0), to_screen(lon as f64, -90.0)],
            egui::Stroke::new(0.5, grid),
        );
    }
    for lat in (-90..=90).step_by(30) {
        painter.line_segment(
            [to_screen(-180.0, lat as f64), to_screen(180.0, lat as f64)],
            egui::Stroke::new(0.5, grid),
        );
    }
    // Mesh wireframe: each triangle's three edges (W-vertices, 1-based ids).
    let wn = mesh.w_lon.len();
    let stroke = egui::Stroke::new(0.4, egui::Color32::from_rgb(110, 185, 220));
    let wpos = |idx1: i32| -> Option<egui::Pos2> {
        let i = idx1 as usize;
        (idx1 >= 1 && i <= wn).then(|| to_screen(mesh.w_lon[i - 1], mesh.w_lat[i - 1]))
    };
    let half = rect.width() * 0.5;
    let near = |p: egui::Pos2, q: egui::Pos2| (p.x - q.x).abs() < half;
    for t in mesh.m_to_w.chunks_exact(3) {
        if let (Some(a), Some(b), Some(c)) = (wpos(t[0]), wpos(t[1]), wpos(t[2])) {
            // Skip date-line-wrapping triangles so they don't streak across the map.
            if near(a, b) && near(b, c) && near(a, c) {
                painter.line_segment([a, b], stroke);
                painter.line_segment([b, c], stroke);
                painter.line_segment([c, a], stroke);
            }
        }
    }
}

/// A regional domain used to mask the drawn mesh to the area of interest.
#[derive(Clone)]
enum DomainMask {
    Bbox {
        west: f64,
        east: f64,
        north: f64,
        south: f64,
    },
    Circle {
        lon: f64,
        lat: f64,
        radius_km: f64,
    },
    Close {
        points: Vec<[f64; 2]>,
    },
    Any(Vec<DomainMask>),
}

impl DomainMask {
    /// Whether a cell centre (degrees) falls inside the domain.
    fn contains(&self, lon: f64, lat: f64) -> bool {
        let norm = |x: f64| ((x + 180.0).rem_euclid(360.0)) - 180.0;
        match *self {
            DomainMask::Bbox {
                west,
                east,
                north,
                south,
            } => {
                let (s, n) = (south.min(north), south.max(north));
                let (w, e) = (norm(west), norm(east));
                let lon = norm(lon);
                lat >= s && lat <= n && lon >= w.min(e) && lon <= w.max(e)
            }
            DomainMask::Circle {
                lon: clon,
                lat: clat,
                radius_km,
            } => {
                let r_earth = 6371.0_f64;
                let (la1, la2) = (clat.to_radians(), lat.to_radians());
                let dlat = (lat - clat).to_radians();
                let dlon = (norm(lon) - norm(clon)).to_radians();
                let a =
                    (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
                2.0 * r_earth * a.sqrt().asin() <= radius_km
            }
            DomainMask::Close { ref points } => point_in_close_domain(points, lon, lat),
            DomainMask::Any(ref domains) => domains.iter().any(|domain| domain.contains(lon, lat)),
        }
    }

    fn extent(&self) -> Option<(f64, f64, f64, f64)> {
        match self {
            DomainMask::Bbox {
                west,
                east,
                north,
                south,
            } => Some((*west, *east, *south, *north)),
            DomainMask::Circle {
                lon,
                lat,
                radius_km,
            } => {
                let r_deg = *radius_km / 111.32;
                let r_lon = r_deg / lat.to_radians().cos().abs().max(0.05);
                Some((*lon - r_lon, *lon + r_lon, *lat - r_deg, *lat + r_deg))
            }
            DomainMask::Close { points } => {
                if points.is_empty() {
                    return None;
                }
                let mut west = f64::MAX;
                let mut east = f64::MIN;
                let mut south = f64::MAX;
                let mut north = f64::MIN;
                for [lon, lat] in points {
                    west = west.min(*lon);
                    east = east.max(*lon);
                    south = south.min(*lat);
                    north = north.max(*lat);
                }
                Some((west, east, south, north))
            }
            DomainMask::Any(domains) => {
                let mut iter = domains.iter().filter_map(DomainMask::extent);
                let (mut west, mut east, mut south, mut north) = iter.next()?;
                for (w, e, s, n) in iter {
                    west = west.min(w);
                    east = east.max(e);
                    south = south.min(s);
                    north = north.max(n);
                }
                Some((west, east, south, north))
            }
        }
    }
}

fn point_in_close_domain(points: &[[f64; 2]], lon: f64, lat: f64) -> bool {
    if points.len() < 3 || !lon.is_finite() || !lat.is_finite() {
        return false;
    }
    let norm = |x: f64| ((x + 180.0).rem_euclid(360.0)) - 180.0;
    let lon0 = norm(lon);
    let unwrap = |x: f64| {
        let mut y = norm(x);
        if y - lon0 > 180.0 {
            y -= 360.0;
        } else if y - lon0 < -180.0 {
            y += 360.0;
        }
        y
    };
    let mut inside = false;
    let eps = 1.0e-12;
    for i in 0..points.len() {
        let [alon, alat] = points[i];
        let [blon, blat] = points[(i + 1) % points.len()];
        if !alon.is_finite() || !alat.is_finite() || !blon.is_finite() || !blat.is_finite() {
            return false;
        }
        let ax = unwrap(alon);
        let bx = unwrap(blon);
        let cross = (lon0 - ax) * (blat - alat) - (lat - alat) * (bx - ax);
        let on_segment = cross.abs() <= eps
            && lon0 >= ax.min(bx) - eps
            && lon0 <= ax.max(bx) + eps
            && lat >= alat.min(blat) - eps
            && lat <= alat.max(blat) + eps;
        if on_segment {
            return true;
        }
        if (alat > lat) != (blat > lat) {
            let x_at_lat = ax + (lat - alat) * (bx - ax) / (blat - alat);
            if lon0 < x_at_lat {
                inside = !inside;
            }
        }
    }
    inside
}

/// Draws the mesh triangle wireframe on top of a walkers slippy map, projecting
/// each W-vertex through the map's `Projector` so it tracks pan/zoom. When a
/// `domain` is set, only cells whose centre is inside it are drawn (the rest are
/// masked out), so a regional run shows just its region of interest.
struct MeshOverlay<'m> {
    mesh: &'m earthmesh_cli::GridfileMeshPoints,
    class_codes: &'m [i8],
    domain: Option<DomainMask>,
    /// Draw the hexagonal primal cells (W cells) rather than the triangular dual
    /// (M cells). For an MPAS hex run the cells are hexagons, so this is what the
    /// user expects to see; a tri run draws triangles.
    hex: bool,
}

/// Web-Mercator's usable latitude bound; tiles cover only ±this.
const MERCATOR_MAX_LAT: f64 = 85.05112878;
const MESH_PREVIEW_MAX_LAT: f64 = 80.0;

fn norm_lon_180(lon: f64) -> f64 {
    ((lon + 180.0).rem_euclid(360.0)) - 180.0
}

fn unwrap_lon_around(lon: f64, reference: f64) -> f64 {
    if lon - reference > 180.0 {
        lon - 360.0
    } else if lon - reference < -180.0 {
        lon + 360.0
    } else {
        lon
    }
}

fn unit_xyz_from_lonlat(lon: f64, lat: f64) -> [f64; 3] {
    let lon = deg_to_rad(lon);
    let lat = deg_to_rad(lat);
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn lonlat_from_unit_xyz(point: [f64; 3]) -> (f64, f64) {
    let raxis = point[0].hypot(point[1]);
    (
        rad_to_deg(point[1].atan2(point[0])),
        rad_to_deg(point[2].atan2(raxis)),
    )
}

fn angular_distance_degrees_lonlat(a: (f64, f64), b: (f64, f64)) -> f64 {
    let va = unit_xyz_from_lonlat(a.0, a.1);
    let vb = unit_xyz_from_lonlat(b.0, b.1);
    let dot = (va[0] * vb[0] + va[1] * vb[1] + va[2] * vb[2]).clamp(-1.0, 1.0);
    rad_to_deg(dot.acos())
}

fn preview_cell_is_local(center: (f64, f64), corners: &[(f64, f64)]) -> bool {
    if corners.len() < 3 {
        return false;
    }
    const MAX_PREVIEW_CELL_DISTANCE_DEG: f64 = 30.0;
    corners.iter().all(|&corner| {
        angular_distance_degrees_lonlat(center, corner) <= MAX_PREVIEW_CELL_DISTANCE_DEG
    }) && corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .take(corners.len())
        .all(|(&a, &b)| angular_distance_degrees_lonlat(a, b) <= MAX_PREVIEW_CELL_DISTANCE_DEG)
}

fn preview_segment_orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn preview_segments_intersect(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    const EPS: f64 = 1.0e-12;
    let ab_c = preview_segment_orientation(a, b, c);
    let ab_d = preview_segment_orientation(a, b, d);
    let cd_a = preview_segment_orientation(c, d, a);
    let cd_b = preview_segment_orientation(c, d, b);
    ab_c * ab_d < -EPS && cd_a * cd_b < -EPS
}

fn preview_cell_local_points(center: (f64, f64), corners: &[(f64, f64)]) -> Vec<(f64, f64)> {
    corners
        .iter()
        .map(|&(lon, lat)| (unwrap_lon_around(norm_lon_180(lon), center.0), lat))
        .collect()
}

fn preview_cell_has_self_intersection(center: (f64, f64), corners: &[(f64, f64)]) -> bool {
    let points = preview_cell_local_points(center, corners);
    let n = points.len();
    if n < 4 {
        return false;
    }
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        for j in (i + 1)..n {
            if j == i || j == (i + 1) % n || i == (j + 1) % n {
                continue;
            }
            let c = points[j];
            let d = points[(j + 1) % n];
            if preview_segments_intersect(a, b, c, d) {
                return true;
            }
        }
    }
    false
}

fn order_preview_cell_corners(center: (f64, f64), mut corners: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if corners.len() < 3 {
        return corners;
    }
    let raw_corners = corners.clone();
    let local = preview_cell_local_points(center, &corners);
    let centroid = (
        local.iter().map(|point| point.0).sum::<f64>() / local.len() as f64,
        local.iter().map(|point| point.1).sum::<f64>() / local.len() as f64,
    );
    corners.sort_by(|a, b| {
        let a = (unwrap_lon_around(norm_lon_180(a.0), center.0), a.1);
        let b = (unwrap_lon_around(norm_lon_180(b.0), center.0), b.1);
        let ba = (a.1 - centroid.1).atan2(a.0 - centroid.0);
        let bb = (b.1 - centroid.1).atan2(b.0 - centroid.0);
        ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
    });
    if preview_cell_has_self_intersection(center, &corners)
        && !preview_cell_has_self_intersection(center, &raw_corners)
    {
        return raw_corners;
    }
    corners
}

fn geodesic_edge_points(a: (f64, f64), b: (f64, f64), ref_lon: f64) -> Vec<(f64, f64)> {
    let va = unit_xyz_from_lonlat(a.0, a.1);
    let vb = unit_xyz_from_lonlat(b.0, b.1);
    let dot = (va[0] * vb[0] + va[1] * vb[1] + va[2] * vb[2]).clamp(-1.0, 1.0);
    let angle = dot.acos();
    if angle < 1.0e-12 || (std::f64::consts::PI - angle).abs() < 1.0e-12 {
        return vec![a, b];
    }

    let steps = rad_to_deg(angle).ceil().clamp(1.0, 64.0) as usize;
    let sin_angle = angle.sin();
    let mut points = Vec::with_capacity(steps + 1);
    let mut previous_lon = ref_lon;
    for step in 0..=steps {
        let t = step as f64 / steps as f64;
        let wa = ((1.0 - t) * angle).sin() / sin_angle;
        let wb = (t * angle).sin() / sin_angle;
        let point = [
            wa * va[0] + wb * vb[0],
            wa * va[1] + wb * vb[1],
            wa * va[2] + wb * vb[2],
        ];
        let (lon, lat) = lonlat_from_unit_xyz(point);
        let unwrapped = unwrap_lon_around(norm_lon_180(lon), previous_lon);
        previous_lon = unwrapped;
        points.push((unwrapped, lat));
    }
    points
}

impl walkers::Plugin for MeshOverlay<'_> {
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        response: &egui::Response,
        projector: &walkers::Projector,
        _map_memory: &walkers::MapMemory,
    ) {
        let MeshOverlay {
            mesh,
            class_codes,
            domain,
            hex,
        } = *self;
        let wn = mesh.w_lon.len();
        let mn = mesh.m_lon.len();

        // Clip the mesh to the basemap rectangle, derived straight from the
        // projector so it tracks pan and zoom (Web Mercator only reaches ±85.05°,
        // and the world edges are lon ±180°). Bounded by the map widget itself
        // (`response.rect`) so it never bleeds into the surrounding UI.
        let p = |lon: f64, lat: f64| projector.project(walkers::lon_lat(lon, lat)).to_pos2();
        let x_west = p(-180.0, 0.0).x;
        let x_east = p(180.0, 0.0).x;
        let y_top = p(0.0, MERCATOR_MAX_LAT).y;
        let y_bot = p(0.0, -MERCATOR_MAX_LAT).y;
        let map_rect =
            egui::Rect::from_min_max(egui::pos2(x_west, y_top), egui::pos2(x_east, y_bot))
                .intersect(response.rect);
        let painter = ui.painter().with_clip_rect(map_rect);

        // Draw a closed cell outline from its (normalized lon, lat) corners,
        // unwrapped around `ref_lon`. Pole-cap cells (wide even unwrapped) are
        // dropped; seam-straddling cells get a ±360° copy so the join fills.
        let draw_cell =
            |center: (f64, f64), corners: &[(f64, f64)], ref_lon: f64, stroke: egui::Stroke| {
                if corners.len() < 3 {
                    return;
                }
                if !preview_cell_is_local(center, corners) {
                    return;
                }
                let u: Vec<(f64, f64)> = corners
                    .iter()
                    .map(|&(lo, la)| (unwrap_lon_around(lo, ref_lon), la))
                    .collect();
                if u.iter().any(|(_, lat)| lat.abs() > MESH_PREVIEW_MAX_LAT) {
                    return;
                }
                let umin = u.iter().map(|v| v.0).fold(f64::MAX, f64::min);
                let umax = u.iter().map(|v| v.0).fold(f64::MIN, f64::max);
                if umax - umin > 45.0 {
                    return;
                }
                let draw_at = |offset: f64| {
                    for i in 0..u.len() {
                        let j = (i + 1) % u.len();
                        let arc = geodesic_edge_points(u[i], u[j], ref_lon);
                        for segment in arc.windows(2) {
                            painter.line_segment(
                                [
                                    p(segment[0].0 + offset, segment[0].1),
                                    p(segment[1].0 + offset, segment[1].1),
                                ],
                                stroke,
                            );
                        }
                    }
                };
                draw_at(0.0);
                if umax > 180.0 {
                    draw_at(-360.0);
                }
                if umin < -180.0 {
                    draw_at(360.0);
                }
            };

        if hex && !mesh.w_to_m.is_empty() {
            // Hexagonal primal cells: each W cell's corners are the surrounding
            // M-points (itab_w%im), sorted by their local vertex centroid.
            let width = mesh.w_to_m_width;
            for wi in 0..wn {
                if let Some(dom) = &domain {
                    if !dom.contains(mesh.w_lon[wi], mesh.w_lat[wi]) {
                        continue;
                    }
                }
                let clon = norm_lon_180(mesh.w_lon[wi]);
                let clat = mesh.w_lat[wi];
                let nn = (mesh.n_w.get(wi).copied().unwrap_or(0).max(0) as usize).min(width);
                let mut corners: Vec<(f64, f64)> = Vec::with_capacity(nn);
                for k in 0..nn {
                    let id = mesh.w_to_m[wi * width + k];
                    if let Some(mi) = gridfile_row_index_for_id(id, &mesh.m_lon, &mesh.m_lat) {
                        if mi < mn {
                            corners.push((norm_lon_180(mesh.m_lon[mi]), mesh.m_lat[mi]));
                        }
                    }
                }
                let corners = order_preview_cell_corners((clon, clat), corners);
                draw_cell(
                    (clon, clat),
                    &corners,
                    clon,
                    surface_class_stroke(class_codes.get(wi).copied()),
                );
            }
        } else {
            // Triangular cells: each M cell → its 3 W vertices.
            let vert = |idx1: i32| -> Option<(f64, f64)> {
                gridfile_row_index_for_id(idx1, &mesh.w_lon, &mesh.w_lat)
                    .filter(|&i| i < wn)
                    .map(|i| (norm_lon_180(mesh.w_lon[i]), mesh.w_lat[i]))
            };
            for (ci, t) in mesh.m_to_w.chunks_exact(3).enumerate() {
                if let Some(dom) = &domain {
                    if ci >= mn || !dom.contains(mesh.m_lon[ci], mesh.m_lat[ci]) {
                        continue;
                    }
                }
                let (Some(a), Some(b), Some(c)) = (vert(t[0]), vert(t[1]), vert(t[2])) else {
                    continue;
                };
                let center = (
                    norm_lon_180(mesh.m_lon.get(ci).copied().unwrap_or(a.0)),
                    mesh.m_lat.get(ci).copied().unwrap_or(a.1),
                );
                draw_cell(
                    center,
                    &[a, b, c],
                    a.0,
                    surface_class_stroke(class_codes.get(ci).copied()),
                );
            }
        }
    }
}

fn surface_class_stroke(code: Option<i8>) -> egui::Stroke {
    // Semi-transparent so the basemap underneath stays readable.
    let color = match code.unwrap_or(0) {
        1 => egui::Color32::from_rgba_unmultiplied(46, 160, 67, 180), // LAND
        2 => egui::Color32::from_rgba_unmultiplied(9, 105, 218, 180), // OCEAN
        3 => egui::Color32::from_rgba_unmultiplied(222, 170, 0, 210), // COAST
        _ => egui::Color32::from_rgba_unmultiplied(255, 120, 0, 150),
    };
    egui::Stroke::new(0.75, color)
}

fn surface_class_name(code: i8) -> &'static str {
    match code {
        1 => "LAND",
        2 => "OCEAN",
        3 => "COAST",
        _ => "UNKNOWN",
    }
}

fn surface_class_counts(codes: &[i8]) -> Vec<(i8, usize)> {
    let mut counts = std::collections::BTreeMap::<i8, usize>::new();
    for &code in codes {
        if code != 0 {
            *counts.entry(code).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

fn has_mask_postproc_two_placeholders(lon: &[f64], lat: &[f64]) -> bool {
    lon.len() > 2
        && lat.len() > 2
        && lon[0] == 0.0
        && lat[0] == 0.0
        && lon[1] == 0.0
        && lat[1] == 0.0
}

fn gridfile_row_index_for_id(id: i32, lon: &[f64], lat: &[f64]) -> Option<usize> {
    if id < 1 {
        return None;
    }
    let idx = if has_mask_postproc_two_placeholders(lon, lat) {
        if id < 2 {
            return None;
        }
        id as usize
    } else {
        (id as usize).saturating_sub(1)
    };
    (idx < lon.len() && idx < lat.len()).then_some(idx)
}

fn collect_outputs(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "nc4" || x == "nc") {
                    out.push(p);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

fn preview_output_path(files: &[PathBuf]) -> Option<PathBuf> {
    fn has_component(path: &Path, component: &str) -> bool {
        path.components()
            .any(|part| part.as_os_str() == OsStr::new(component))
    }
    fn is_gridfile(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("gridfile"))
    }

    files
        .iter()
        .find(|path| is_gridfile(path) && has_component(path, "regional"))
        .or_else(|| {
            files
                .iter()
                .find(|path| is_gridfile(path) && has_component(path, "masked"))
        })
        .or_else(|| {
            files
                .iter()
                .find(|path| is_gridfile(path) && has_component(path, "result"))
        })
        .or_else(|| files.iter().find(|path| is_gridfile(path)))
        .or_else(|| files.first())
        .cloned()
}

fn preview_display_files(preview_path: Option<&PathBuf>) -> Vec<PathBuf> {
    preview_path.into_iter().cloned().collect()
}

fn colm_coupling_output_path(files: &[PathBuf]) -> Option<PathBuf> {
    files
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("CoLM_") && name.ends_with("_coupling.nc4"))
        })
        .cloned()
}

fn surface_class_key(lon: f64, lat: f64) -> (i64, i64) {
    let lon = ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    ((lon * 1.0e6).round() as i64, (lat * 1.0e6).round() as i64)
}

fn surface_classes_for_mesh(
    files: &[PathBuf],
    mesh: &earthmesh_cli::GridfileMeshPoints,
    hex: bool,
    landtype: &str,
    gridnum: usize,
) -> Result<Vec<i8>, String> {
    let centers = mesh_center_points(mesh, hex);
    if let Some(path) = colm_coupling_output_path(files) {
        let points = earthmesh_cli::read_colm_surface_class_points_netcdf(&path)
            .map_err(|err| format!("read {}: {err}", path.display()))?;
        let mut by_center = HashMap::with_capacity(points.len());
        for point in points {
            by_center.insert(surface_class_key(point.lon, point.lat), point.code);
        }
        return Ok(centers
            .iter()
            .map(|point| {
                by_center
                    .get(&surface_class_key(point.lon, point.lat))
                    .copied()
                    .unwrap_or(0)
            })
            .collect());
    }
    if !earthmesh_cli::landtype_file_is_real(landtype) {
        return Ok(Vec::new());
    }
    let codes = earthmesh_cli::sample_landtype_surface_class_codes_for_points_fortran_indexed(
        landtype,
        gridnum.max(1),
        &centers,
    )
    .map_err(|err| format!("sample land-type classes: {err}"))?;
    Ok(centers
        .iter()
        .zip(codes)
        .map(|(point, code)| {
            if point.lon == 0.0 && point.lat == 0.0 {
                0
            } else {
                code
            }
        })
        .collect())
}

fn mesh_center_points(
    mesh: &earthmesh_cli::GridfileMeshPoints,
    hex: bool,
) -> Vec<earthmesh_cli::LonLatPoint> {
    let coords = if hex {
        mesh.w_lon.iter().zip(&mesh.w_lat).collect::<Vec<_>>()
    } else {
        mesh.m_lon.iter().zip(&mesh.m_lat).collect::<Vec<_>>()
    };
    coords
        .into_iter()
        .map(|(&lon, &lat)| earthmesh_cli::LonLatPoint { lon, lat })
        .collect()
}

fn collect_outputs_since(dir: &Path, since: SystemTime) -> Vec<PathBuf> {
    collect_outputs(dir)
        .into_iter()
        .filter(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .map(|modified| modified >= since)
                .unwrap_or(false)
        })
        .collect()
}

/// Post-run, pure-Rust outputs: carve a regional gridfile when a bbox/circle/close
/// region is set (so the output drops out-of-region cells), and/or write the
/// selected model's standard file from that (carved, if regional) gridfile.
/// Returns a short summary, or an error for hard failures.
fn produce_outputs(
    out_dir: &str,
    nxp: usize,
    grid: &str,
    fmt: &str,
    gen_output: bool,
    region: Option<&earthmesh_cli::GridRegion>,
    mesh_type: &str,
    landtype: &str,
    gridnum: usize,
) -> Result<String, String> {
    let base = Path::new(out_dir);
    let mut global_gf = None;
    if let Ok(rd) = std::fs::read_dir(base.join("gridfile")) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with("gridfile_NXP") && name.ends_with(".nc4") {
                let matches_grid = name.contains(grid);
                global_gf = Some(p);
                if matches_grid {
                    break;
                }
            }
        }
    }
    let global_gf = global_gf.ok_or_else(|| "gridfile not found in output".to_string())?;
    let global_for_regional = global_gf.clone();
    let mut notes: Vec<String> = Vec::new();

    let landtype_mask = matches!(mesh_type.trim(), "landmesh" | "oceanmesh")
        && earthmesh_cli::landtype_file_is_real(landtype);
    let domain_source_gf = if landtype_mask {
        let mask_dir = base.join("masked");
        std::fs::create_dir_all(&mask_dir).map_err(|e| e.to_string())?;
        let mask_gf = mask_dir.join(format!("gridfile_NXP{nxp:04}_{grid}_{mesh_type}.nc4"));
        let kept = earthmesh_cli::write_landtype_masked_gridfile(
            &global_gf, &mask_gf, landtype, gridnum, grid, mesh_type,
        )
        .map_err(|e| format!("{mesh_type} land-type mask failed: {e}"))?;
        let label = if mesh_type == "landmesh" {
            "land"
        } else {
            "ocean"
        };
        notes.push(format!("{label} land-type masked gridfile ({kept} cells)"));
        mask_gf
    } else {
        global_gf
    };

    // Carve the gridfile to the region so the output drops out-of-region cells.
    let source_gf = if let Some(region) = region {
        let reg_dir = base.join("regional");
        std::fs::create_dir_all(&reg_dir).map_err(|e| e.to_string())?;
        let reg_gf = reg_dir.join(format!("gridfile_NXP{nxp:04}_{grid}_regional.nc4"));
        let kept = earthmesh_cli::write_regional_gridfile(&domain_source_gf, &reg_gf, region, grid)
            .map_err(|e| format!("regional carve failed: {e}"))?;
        notes.push(format!("regional gridfile ({kept} cells)"));
        reg_gf
    } else {
        domain_source_gf
    };

    if gen_output {
        let std_dir = base.join("standard");
        std::fs::create_dir_all(&std_dir).map_err(|e| e.to_string())?;
        let stem = format!("NXP{nxp:04}_{grid}");
        if fmt.starts_with("MPAS") {
            let mesh_out = std_dir.join(format!("MPASOUT_{stem}.nc4"));
            let graph_out = std_dir.join(format!("MPASOUT_{stem}.graph.info"));
            if let (Some(region), "hex") = (region, grid) {
                // Limited-area MPAS: subset the validated global mesh to the
                // region (geometry preserved, connectivity re-indexed, boundary
                // neighbours -> 0). No open-boundary rejection.
                match earthmesh_cli::write_regional_mpas_from_gridfile(
                    &global_for_regional,
                    &mesh_out,
                    &graph_out,
                    region,
                    nxp,
                ) {
                    Ok((_, kept)) => notes.push(format!("regional MPAS ({kept} cells)")),
                    Err(e) => return Err(format!("regional MPAS write failed: {e}")),
                }
            } else {
                match earthmesh_cli::write_standard_mpas_from_gridfile(
                    &source_gf, &mesh_out, &graph_out, nxp,
                ) {
                    Ok(_) => notes.push("standard MPAS".into()),
                    Err(e) => return Err(format!("MPAS write failed: {e}")),
                }
            }
        } else if fmt == "FVCOM" {
            let out_2dm = std_dir.join(format!("FVCOM_{stem}.2dm"));
            match earthmesh_cli::write_standard_fvcom_from_gridfile(&source_gf, &out_2dm) {
                Ok(_) => notes.push("standard FVCOM".into()),
                // A carved patch has boundary cells the writers reject; the
                // regional gridfile is still the usable output.
                Err(e) if region.is_some() => {
                    notes.push(format!("regional FVCOM skipped — open boundary: {e}"))
                }
                Err(e) => return Err(format!("FVCOM write failed: {e}")),
            }
        } else if fmt.eq_ignore_ascii_case("colm") {
            // CoLM surface-data coupling: classify each cell LAND/OCEAN from the
            // land-type grid and write the coupling CSV + NetCDF. River/coast
            // attributes are placeholders until MERIT/CaMa assignment is wired.
            if earthmesh_cli::landtype_file_is_real(landtype) {
                let csv = std_dir.join(format!("CoLM_{stem}_cells.csv"));
                let nc = std_dir.join(format!("CoLM_{stem}_coupling.nc4"));
                let manifest = std_dir.join(format!("CoLM_{stem}_manifest.json"));
                match earthmesh_cli::write_colm_coupling_csv_from_mesh(
                    &source_gf,
                    landtype,
                    gridnum,
                    "earthmesh",
                    grid,
                    &csv,
                ) {
                    Ok(counts) => {
                        let _ = std::fs::write(&manifest, "{}");
                        match earthmesh_cli::write_colm_coupling_netcdf_from_csv(
                            &csv,
                            &nc,
                            "earthmesh",
                            &manifest,
                        ) {
                            Ok(_) => notes.push(format!(
                                "CoLM coupling ({} land / {} ocean cells)",
                                counts.land, counts.ocean
                            )),
                            Err(e) => notes.push(format!("CoLM CSV ok, NetCDF failed: {e}")),
                        }
                    }
                    Err(e) => return Err(format!("CoLM generation failed: {e}")),
                }
            } else {
                notes.push("CoLM needs a land-type file (set Land-type)".into());
            }
        } else {
            notes.push(format!("standard '{fmt}' needs the data pipeline"));
        }
    }
    Ok(notes.join("; "))
}

/// Carve a clean regional ocean FVCOM mesh from the run's global gridfile + the
/// close polygon + a landtype file, without refinement. Writes the `.2dm` under
/// `<out>/standard/`. Returns a short note.
fn clean_ocean_fvcom_for_output(
    out_dir: &str,
    nxp: usize,
    grid: &str,
    close_points: &[earthmesh_cli::LonLatPoint],
    landtype: &str,
    gridnum: usize,
    sea_ratio: f64,
) -> Result<String, String> {
    let base = Path::new(out_dir);
    let mut gridfile = None;
    if let Ok(rd) = std::fs::read_dir(base.join("gridfile")) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with("gridfile_NXP") && name.ends_with(".nc4") {
                let matches_grid = name.contains(grid);
                gridfile = Some(p);
                if matches_grid {
                    break;
                }
            }
        }
    }
    let gridfile = gridfile.ok_or_else(|| "gridfile not found in output".to_string())?;
    let work = base.join("regional_ocean");
    let std_dir = base.join("standard");
    std::fs::create_dir_all(&std_dir).map_err(|e| e.to_string())?;
    let out_2dm = std_dir.join(format!("FVCOM_NXP{nxp:04}_{grid}.2dm"));
    let elements = earthmesh_cli::write_clean_regional_ocean_fvcom(
        &gridfile,
        close_points,
        Path::new(landtype),
        nxp,
        gridnum,
        sea_ratio,
        &work,
        &out_2dm,
    )
    .map_err(|e| format!("clean ocean carve failed: {e}"))?;
    Ok(format!("clean regional FVCOM ({elements} elements)"))
}

impl EarthMeshApp {
    fn set_status(&mut self, key: &'static str, detail: String) {
        self.status_key = key;
        self.status_detail = detail;
    }

    fn push_log(&mut self, key: &'static str, detail: &str) {
        let line = if detail.is_empty() {
            tr(self.lang, key).to_string()
        } else {
            format!("{} {}", tr(self.lang, key), detail)
        };
        self.log.push(line);
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    fn load(&mut self, path: PathBuf) {
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(err) => return self.set_status("status.read_error", err.to_string()),
        };
        match EarthmeshConfig::from_mkgrd_namelist(&contents) {
            Ok(cfg) => {
                let mut refine = if has_mkrefine_block(&contents) {
                    match RefineConfig::from_mkrefine_namelist(
                        &contents,
                        &cfg.mesh_type,
                        &cfg.mode_grid,
                    ) {
                        Ok(refine) => refine,
                        Err(err) => return self.set_status("status.parse_error", err),
                    }
                } else {
                    RefineConfig {
                        max_iter_spc: SPECIFIED_REFINE_LEVEL_MAX,
                        ..Default::default()
                    }
                };
                if refine.max_iter_spc <= 0 {
                    refine.max_iter_spc = SPECIFIED_REFINE_LEVEL_MAX;
                }
                self.refine = refine;
                self.mkgrd = cfg;
                self.native_olam_mkgrd = extract_native_olam_mkgrd_lines(&contents);
                let shown = path.display().to_string();
                self.set_status("status.loaded", shown.clone());
                self.push_log("log.loaded", &shown);
                self.recent.retain(|p| p != &path);
                self.recent.insert(0, path.clone());
                self.recent.truncate(8);
                self.loaded_path = Some(path);
            }
            Err(err) => self.set_status("status.parse_error", err),
        }
    }

    fn combined_namelist(&self) -> String {
        format!(
            "{}\n{}",
            insert_native_olam_mkgrd_lines(
                &self.mkgrd.to_mkgrd_namelist(),
                &self.native_olam_mkgrd,
            ),
            self.refine.to_mkrefine_namelist()
        )
    }

    fn run_namelist(&self) -> String {
        let mut mkgrd = self.mkgrd.clone();
        let mut refine = self.refine.clone();
        let base_dir = resolve_case_base_dir(&mkgrd.base_dir);
        mkgrd.base_dir = format!("{}/", base_dir.display());
        if earthmesh_cli::landtype_file_is_real(&mkgrd.landtype_file) {
            mkgrd.landtype_file = resolve_runtime_file_path(&mkgrd.landtype_file)
                .display()
                .to_string();
        }
        if !refinement_supported_for(&mkgrd.mesh_type, &mkgrd.output_format) {
            mkgrd.refine = false;
            refine.refine_spc = false;
            refine.refine_cal = false;
        }
        if mkgrd.mesh_type == "atmosmesh" {
            refine.refine_cal = false;
        }
        if mkgrd.refine && refine.refine_spc && self.gui_authored_specified_refinement() {
            refine.max_iter_spc = self.specified_refine_passes() as i32;
        }
        format!(
            "{}\n{}",
            insert_native_olam_mkgrd_lines(&mkgrd.to_mkgrd_namelist(), &self.native_olam_mkgrd,),
            refine.to_mkrefine_namelist()
        )
    }

    fn save(&mut self, path: PathBuf) {
        match std::fs::write(&path, self.combined_namelist()) {
            Ok(()) => {
                let shown = path.display().to_string();
                self.set_status("status.saved", shown.clone());
                self.push_log("log.saved", &shown);
            }
            Err(err) => self.set_status("status.write_error", err.to_string()),
        }
    }

    fn output_dir(&self) -> PathBuf {
        resolve_case_base_dir(&self.mkgrd.base_dir).join(self.mkgrd.experiment_name.trim())
    }

    fn regional_domain(&self) -> Option<DomainMask> {
        match self.mkgrd.mask_domain_type.as_str() {
            "bbox" => Some(DomainMask::Bbox {
                west: self.dom_bbox[0],
                east: self.dom_bbox[1],
                north: self.dom_bbox[2],
                south: self.dom_bbox[3],
            }),
            "circle" => Some(DomainMask::Circle {
                lon: self.dom_circle[0],
                lat: self.dom_circle[1],
                radius_km: self.dom_circle[2],
            }),
            "close" if self.dom_close.len() >= 3 => Some(DomainMask::Close {
                points: self.dom_close.clone(),
            }),
            _ => None,
        }
    }

    fn specified_refinement_domain(&self) -> Option<DomainMask> {
        if !self.refinement_supported() || !self.mkgrd.refine || !self.refine.refine_spc {
            return None;
        }
        let domains = match self.refine.mask_refine_spc_type.as_str() {
            "bbox" => self
                .refine_bboxes
                .iter()
                .map(|bbox| DomainMask::Bbox {
                    west: bbox.bounds[0],
                    east: bbox.bounds[1],
                    north: bbox.bounds[2],
                    south: bbox.bounds[3],
                })
                .collect::<Vec<_>>(),
            "circle" => self
                .refine_circles
                .iter()
                .map(|circle| DomainMask::Circle {
                    lon: circle.circle[0],
                    lat: circle.circle[1],
                    radius_km: circle.circle[2],
                })
                .collect::<Vec<_>>(),
            "close" => self
                .refine_closes
                .iter()
                .filter(|region| region.points.len() >= 3)
                .map(|region| DomainMask::Close {
                    points: region.points.clone(),
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        match domains.len() {
            0 => None,
            1 => domains.into_iter().next(),
            _ => Some(DomainMask::Any(domains)),
        }
    }

    fn results_draw_domain(&self) -> Option<DomainMask> {
        if self.mkgrd.mask_domain_global {
            None
        } else {
            self.regional_domain()
        }
    }

    fn results_focus_domain(&self) -> Option<DomainMask> {
        if self.mkgrd.mask_domain_global {
            self.specified_refinement_domain()
        } else {
            self.regional_domain()
        }
    }

    fn resolved_landtype_file(&self) -> String {
        if earthmesh_cli::landtype_file_is_real(&self.mkgrd.landtype_file) {
            resolve_runtime_file_path(&self.mkgrd.landtype_file)
                .display()
                .to_string()
        } else {
            self.mkgrd.landtype_file.clone()
        }
    }

    fn landtype_mask_postprocess_required(&self) -> bool {
        matches!(self.mkgrd.mesh_type.as_str(), "landmesh" | "oceanmesh")
            && earthmesh_cli::landtype_file_is_real(&self.mkgrd.landtype_file)
    }

    fn refinement_supported(&self) -> bool {
        refinement_supported_for(&self.mkgrd.mesh_type, &self.mkgrd.output_format)
    }

    fn normalize_refinement_for_mesh(&mut self) {
        if !self.refinement_supported() {
            self.mkgrd.refine = false;
            self.refine.refine_spc = false;
            self.refine.refine_cal = false;
        }
        if self.mkgrd.mesh_type == "atmosmesh" {
            self.refine.refine_cal = false;
        }
    }

    fn refine_level_cap(&self) -> usize {
        SPECIFIED_REFINE_LEVEL_MAX as usize
    }

    fn default_refine_level(&self) -> i32 {
        SPECIFIED_REFINE_LEVEL_MAX
    }

    fn gui_authored_specified_refinement(&self) -> bool {
        matches!(
            self.refine.mask_refine_spc_type.as_str(),
            "bbox" | "circle" | "close"
        )
    }

    fn effective_refine_level(level: i32, cap: usize) -> usize {
        (level.max(1) as usize).min(cap.max(1))
    }

    fn active_refine_degree(&self) -> usize {
        let cap = self.refine_level_cap();
        let max_level = match self.refine.mask_refine_spc_type.as_str() {
            "bbox" => self
                .refine_bboxes
                .iter()
                .map(|region| Self::effective_refine_level(region.level, cap))
                .max(),
            "circle" => self
                .refine_circles
                .iter()
                .map(|region| Self::effective_refine_level(region.level, cap))
                .max(),
            "close" => self
                .refine_closes
                .iter()
                .filter(|region| region.points.len() >= 3)
                .map(|region| Self::effective_refine_level(region.level, cap))
                .max(),
            _ => None,
        };
        max_level.unwrap_or(1)
    }

    fn specified_refine_passes(&self) -> usize {
        let cap = self.refine_level_cap();
        let configured = Self::effective_refine_level(self.refine.max_iter_spc, cap);
        configured.max(self.active_refine_degree()).min(cap)
    }

    fn region_active_at_degree(level: i32, degree: usize, cap: usize) -> bool {
        Self::effective_refine_level(level, cap) >= degree
    }

    fn regional_grid_region(&self) -> Option<earthmesh_cli::GridRegion> {
        if self.mkgrd.mask_domain_global {
            return None;
        }
        match self.mkgrd.mask_domain_type.as_str() {
            "bbox" => Some(earthmesh_cli::GridRegion::Bbox {
                west: self.dom_bbox[0],
                east: self.dom_bbox[1],
                north: self.dom_bbox[2],
                south: self.dom_bbox[3],
            }),
            "circle" => Some(earthmesh_cli::GridRegion::Circle {
                lon: self.dom_circle[0],
                lat: self.dom_circle[1],
                radius_km: self.dom_circle[2],
            }),
            "close" if self.dom_close.len() >= 3 => Some(earthmesh_cli::GridRegion::Close {
                points: self
                    .dom_close
                    .iter()
                    .map(|p| earthmesh_cli::LonLatPoint {
                        lon: p[0],
                        lat: p[1],
                    })
                    .collect(),
            }),
            _ => None,
        }
    }

    fn open_output_dir(&mut self) {
        let dir = self.output_dir();
        let target = if dir.exists() { dir } else { output_root() };
        if let Err(err) = open::that(&target) {
            self.set_status("status.write_error", err.to_string());
        }
    }

    /// Author the regional boundary NetCDF from the entered geometry, returning
    /// its path. Returns Ok(None) for shapes the GUI doesn't yet author
    /// (currently lambert), which keep using the user-set boundary file prefix.
    fn generate_domain_file(&self, dir: &Path) -> Result<Option<PathBuf>, String> {
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        match self.mkgrd.mask_domain_type.as_str() {
            "bbox" => {
                let mask = earthmesh_cli::BBoxMask {
                    refine_degree: 0,
                    points: vec![earthmesh_cli::BBoxPoint {
                        west: self.dom_bbox[0],
                        east: self.dom_bbox[1],
                        north: self.dom_bbox[2],
                        south: self.dom_bbox[3],
                    }],
                };
                let path = dir.join("earthmesh_domain_bbox.nc");
                earthmesh_cli::write_bbox_mask_netcdf(&path, &mask).map_err(|e| e.to_string())?;
                Ok(Some(path))
            }
            "circle" => {
                let mask = earthmesh_cli::CircleMask {
                    refine_degree: 0,
                    points: vec![earthmesh_cli::LonLatPoint {
                        lon: self.dom_circle[0],
                        lat: self.dom_circle[1],
                    }],
                    radius_km: vec![self.dom_circle[2]],
                };
                let path = dir.join("earthmesh_domain_circle.nc");
                earthmesh_cli::write_circle_mask_netcdf(&path, &mask).map_err(|e| e.to_string())?;
                Ok(Some(path))
            }
            "close" => {
                if self.dom_close.len() < 3 {
                    return Err("a close-curve domain needs at least 3 points".to_string());
                }
                let mask = earthmesh_cli::CloseMask {
                    refine_degree: 0,
                    points: self
                        .dom_close
                        .iter()
                        .map(|p| earthmesh_cli::LonLatPoint {
                            lon: p[0],
                            lat: p[1],
                        })
                        .collect(),
                };
                let path = dir.join("earthmesh_domain_close.nc");
                earthmesh_cli::write_close_mask_netcdf(&path, &mask).map_err(|e| e.to_string())?;
                Ok(Some(path))
            }
            _ => Ok(None),
        }
    }

    fn generate_refine_spc_files(&mut self, dir: &Path) -> Result<Option<PathBuf>, String> {
        if !self.refinement_supported() || !self.mkgrd.refine || !self.refine.refine_spc {
            return Ok(None);
        }
        std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        let level_cap = self.refine_level_cap();
        let refine_degree = self.specified_refine_passes();
        match self.refine.mask_refine_spc_type.as_str() {
            "bbox" => {
                if self.refine_bboxes.is_empty() {
                    return Err("specified bbox refinement needs at least one box".to_string());
                }
                let prefix = dir.join("earthmesh_refine_bbox");
                for degree in 1..=refine_degree {
                    let points = self
                        .refine_bboxes
                        .iter()
                        .filter(|region| {
                            Self::region_active_at_degree(region.level, degree, level_cap)
                        })
                        .map(|region| earthmesh_cli::BBoxPoint {
                            west: region.bounds[0],
                            east: region.bounds[1],
                            north: region.bounds[2],
                            south: region.bounds[3],
                        })
                        .collect::<Vec<_>>();
                    let mask = earthmesh_cli::BBoxMask {
                        refine_degree: degree,
                        points,
                    };
                    earthmesh_cli::write_bbox_mask_netcdf(
                        dir.join(format!("earthmesh_refine_bbox_{degree:03}.nc4")),
                        &mask,
                    )
                    .map_err(|err| err.to_string())?;
                }
                self.refine.mask_refine_spc_fprefix = prefix.display().to_string();
                Ok(Some(prefix))
            }
            "circle" => {
                if self.refine_circles.is_empty() {
                    return Err("specified circle refinement needs at least one circle".to_string());
                }
                let prefix = dir.join("earthmesh_refine_circle");
                for degree in 1..=refine_degree {
                    let active_regions = self
                        .refine_circles
                        .iter()
                        .filter(|region| {
                            Self::region_active_at_degree(region.level, degree, level_cap)
                        })
                        .collect::<Vec<_>>();
                    let points = active_regions
                        .iter()
                        .map(|region| earthmesh_cli::LonLatPoint {
                            lon: region.circle[0],
                            lat: region.circle[1],
                        })
                        .collect::<Vec<_>>();
                    let radius_km = active_regions
                        .iter()
                        .map(|region| region.circle[2])
                        .collect::<Vec<_>>();
                    let mask = earthmesh_cli::CircleMask {
                        refine_degree: degree,
                        points,
                        radius_km,
                    };
                    earthmesh_cli::write_circle_mask_netcdf(
                        dir.join(format!("earthmesh_refine_circle_{degree:03}.nc4")),
                        &mask,
                    )
                    .map_err(|err| err.to_string())?;
                }
                self.refine.mask_refine_spc_fprefix = prefix.display().to_string();
                Ok(Some(prefix))
            }
            "close" => {
                if self
                    .refine_closes
                    .iter()
                    .all(|region| region.points.len() < 3)
                {
                    return Err(
                        "specified close refinement needs at least one polygon with 3 points"
                            .to_string(),
                    );
                }
                let prefix = dir.join("earthmesh_refine_close");
                for degree in 1..=refine_degree {
                    let mut polygon_number = 0usize;
                    for region in self
                        .refine_closes
                        .iter()
                        .filter(|region| region.points.len() >= 3)
                        .filter(|region| {
                            Self::region_active_at_degree(region.level, degree, level_cap)
                        })
                    {
                        polygon_number += 1;
                        let mask = earthmesh_cli::CloseMask {
                            refine_degree: degree,
                            points: region
                                .points
                                .iter()
                                .map(|point| earthmesh_cli::LonLatPoint {
                                    lon: point[0],
                                    lat: point[1],
                                })
                                .collect(),
                        };
                        earthmesh_cli::write_close_mask_netcdf(
                            dir.join(format!(
                                "earthmesh_refine_close_{degree:03}_{polygon_number:03}.nc4"
                            )),
                            &mask,
                        )
                        .map_err(|err| err.to_string())?;
                    }
                }
                self.refine.mask_refine_spc_fprefix = prefix.display().to_string();
                Ok(Some(prefix))
            }
            _ => Ok(None),
        }
    }

    fn start_run(&mut self, ctx: &egui::Context) {
        if self.running {
            return;
        }
        self.normalize_refinement_for_mesh();
        let stage_dir = unique_stage_dir();
        // Regional: author the boundary file from the entered geometry first so
        // the namelist points at it.
        if !self.mkgrd.mask_domain_global {
            match self.generate_domain_file(&stage_dir) {
                Ok(Some(path)) => {
                    let shown = path.display().to_string();
                    self.mkgrd.mask_domain_fprefix = shown.clone();
                    self.push_log("dom.generated", &shown);
                }
                Ok(None) => {}
                Err(err) => return self.set_status("status.stage_error", err),
            }
        }
        match self.generate_refine_spc_files(&stage_dir) {
            Ok(Some(prefix)) => self.log.push(format!(
                "Generated refinement region prefix: {}",
                prefix.display()
            )),
            Ok(None) => {}
            Err(err) => return self.set_status("status.stage_error", err),
        }
        if let Err(err) = std::fs::create_dir_all(&stage_dir) {
            return self.set_status("status.stage_error", err.to_string());
        }
        let nml_path = stage_dir.join("earthmesh_gui_run.nml");
        if let Err(err) = std::fs::write(&nml_path, self.run_namelist()) {
            return self.set_status("status.stage_error", err.to_string());
        }
        let workdir = runtime_workdir();
        if let Err(err) = std::fs::create_dir_all(&workdir) {
            return self.set_status("status.stage_error", err.to_string());
        }
        let (tx, rx) = mpsc::channel();
        let (ptx, prx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_worker = cancel.clone();
        let ctx = ctx.clone();
        let output_dir = self.output_dir();
        let started_at = SystemTime::now();
        let out_hint = output_dir.display().to_string();
        let gen_output = self.gen_output;
        let nxp = self.mkgrd.nxp.max(0) as usize;
        let grid = self.mkgrd.mode_grid.clone();
        let fmt = self.mkgrd.output_format.clone();
        let mesh_type = self.mkgrd.mesh_type.clone();
        let region = self.regional_grid_region();
        let landtype_mask = self.landtype_mask_postprocess_required();
        // Clean regional ocean (FVCOM) via the close-curve pipeline: needs a
        // landtype source and a polygon; runs WITHOUT refinement.
        let clean_ocean = self.mkgrd.mesh_type == "oceanmesh"
            && self.mkgrd.output_format == "FVCOM"
            && !self.mkgrd.mask_domain_global
            && self.mkgrd.mask_domain_type == "close"
            && self.dom_close.len() >= 3
            && earthmesh_cli::landtype_file_is_real(&self.mkgrd.landtype_file);
        let close_points: Vec<earthmesh_cli::LonLatPoint> = self
            .dom_close
            .iter()
            .map(|p| earthmesh_cli::LonLatPoint {
                lon: p[0],
                lat: p[1],
            })
            .collect();
        let landtype = self.resolved_landtype_file();
        let gridnum = self.mkgrd.gridnum_perdegree.max(1) as usize;
        let sea_ratio = self.mkgrd.mask_sea_ratio;
        let ptx_mpas = ptx.clone();
        thread::spawn(move || {
            earthmesh_core::progress::set(move |phase, done, total| {
                let _ = ptx.send((phase.to_string(), done, total));
                !cancel_worker.load(Ordering::Relaxed)
            });
            let result =
                earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
                    &nml_path, &workdir, 100_000, 0, None, None, None, 1, None,
                );
            earthmesh_core::progress::clear();
            let msg = match result {
                Ok(_) if clean_ocean => {
                    let _ = ptx_mpas.send(("carve".to_string(), 0, 1));
                    match clean_ocean_fvcom_for_output(
                        &out_hint,
                        nxp,
                        &grid,
                        &close_points,
                        &landtype,
                        gridnum,
                        sea_ratio,
                    ) {
                        Ok(note) => Ok(RunResult::new(output_dir.clone(), note, started_at)),
                        Err(err) => Err(err),
                    }
                }
                Ok(_) if gen_output || region.is_some() || landtype_mask => {
                    // Pure-Rust post-processing (regional carve + standard file)
                    // — can be slow on big meshes, so it runs in the worker.
                    let _ = ptx_mpas.send(("output".to_string(), 0, 1));
                    match produce_outputs(
                        &out_hint,
                        nxp,
                        &grid,
                        &fmt,
                        gen_output,
                        region.as_ref(),
                        &mesh_type,
                        &landtype,
                        gridnum,
                    ) {
                        Ok(note) => Ok(RunResult::new(output_dir.clone(), note, started_at)),
                        Err(err) => Err(err),
                    }
                }
                Ok(_) => Ok(RunResult::new(output_dir.clone(), "", started_at)),
                Err(err) => Err(err.to_string()),
            };
            let _ = tx.send(RunMsg::Done(msg));
            ctx.request_repaint();
        });
        self.run_rx = Some(rx);
        self.prog_rx = Some(prx);
        self.cancel_flag = Some(cancel);
        self.progress = None;
        self.last_phase.clear();
        self.output_files.clear();
        self.mesh_view = None;
        self.cell_classes.clear();
        self.running = true;
        self.set_status("status.running", String::new());
        self.push_log("log.run_start", "");
    }

    fn request_cancel(&mut self) {
        if let Some(flag) = &self.cancel_flag {
            flag.store(true, Ordering::Relaxed);
            self.push_log("log.cancel_req", "");
        }
    }

    fn poll_run(&mut self) {
        self.poll_hydro_workflow();
        let mut updates = Vec::new();
        if let Some(prx) = &self.prog_rx {
            while let Ok(update) = prx.try_recv() {
                updates.push(update);
            }
        }
        for (phase, done, total) in updates {
            if phase != self.last_phase {
                self.last_phase = phase.clone();
                self.log.push(format!("→ {phase}"));
                if self.log.len() > 200 {
                    self.log.remove(0);
                }
            }
            self.progress = Some((phase, done, total));
        }

        let done_msg = self.run_rx.as_ref().and_then(|rx| rx.try_recv().ok());
        if let Some(RunMsg::Done(result)) = done_msg {
            let cancelled = self
                .cancel_flag
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed));
            match result {
                Ok(run) => {
                    let shown = run.output_dir.display().to_string();
                    self.set_status("status.run_done", shown.clone());
                    self.push_log("log.run_done", &shown);
                    if let Some(note) = run.note {
                        self.push_log("log.run_done", &note);
                    }
                    let all_outputs = collect_outputs_since(&run.output_dir, run.started_at);
                    self.log
                        .push(format!("Outputs: {} new NetCDF file(s)", all_outputs.len()));
                    let preview_path = preview_output_path(&all_outputs);
                    self.output_files = preview_display_files(preview_path.as_ref());
                    self.cell_classes.clear();
                    self.mesh_view = match preview_path {
                        Some(path) => match earthmesh_cli::read_gridfile_mesh_points(&path) {
                            Ok(mesh) => {
                                self.log.push(format!("Preview: loaded {}", path.display()));
                                // Generate quality reports into the run dir so the
                                // dashboard has real data (reuses the CLI's gridfile→
                                // quality path; read-only, no schema change).
                                let qinput = earthmesh_cli::quality_input_from_gridfile(&mesh);
                                let qreport = earthmesh_quality::compute(
                                    &qinput,
                                    &earthmesh_quality::QualityThresholds::default(),
                                );
                                match earthmesh_quality::io::write_all(&qreport, &run.output_dir) {
                                    Ok(_) => self.log.push(format!(
                                        "Quality: {} (quality_summary.json written)",
                                        qreport.verdict.as_str()
                                    )),
                                    Err(e) => self.log.push(format!("Quality report failed: {e}")),
                                }
                                match surface_classes_for_mesh(
                                    &all_outputs,
                                    &mesh,
                                    self.mkgrd.mode_grid == "hex",
                                    &self.mkgrd.landtype_file,
                                    self.mkgrd.gridnum_perdegree.max(1) as usize,
                                ) {
                                    Ok(classes) if !classes.is_empty() => {
                                        let counts = surface_class_counts(&classes)
                                            .into_iter()
                                            .map(|(code, count)| {
                                                format!("{}={count}", surface_class_name(code))
                                            })
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        self.log.push(format!("Preview classes: {counts}"));
                                        self.cell_classes = classes;
                                    }
                                    Ok(_) => {}
                                    Err(err) => {
                                        self.log
                                            .push(format!("Preview classes unavailable: {err}"));
                                    }
                                }
                                Some(mesh)
                            }
                            Err(err) => {
                                self.log
                                    .push(format!("Preview failed for {}: {err}", path.display()));
                                None
                            }
                        },
                        None => {
                            self.log
                                .push("Preview: no new NetCDF output found".to_string());
                            None
                        }
                    };
                    self.map_memory = walkers::MapMemory::default();
                    self.frame_pending = true;
                }
                Err(err) => {
                    if cancelled {
                        self.set_status("status.cancelled", String::new());
                        self.push_log("status.cancelled", "");
                    } else {
                        self.set_status("status.run_failed", err.clone());
                        self.push_log("log.run_failed", &err);
                    }
                }
            }
            self.running = false;
            self.run_rx = None;
            self.prog_rx = None;
            self.cancel_flag = None;
            self.progress = None;
        }
    }
}

// ---- grid-row helpers ----------------------------------------------------------

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(280.0));
    ui.end_row();
}
fn int_row(ui: &mut egui::Ui, label: &str, value: &mut i32, range: std::ops::RangeInclusive<i32>) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).range(range).speed(1.0));
    ui.end_row();
}
fn f32_row(ui: &mut egui::Ui, label: &str, value: &mut f32) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).speed(0.01));
    ui.end_row();
}
fn f64_row(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.label(label);
    ui.add(egui::DragValue::new(value).speed(0.01));
    ui.end_row();
}
fn combo_row(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) {
    ui.label(label);
    egui::ComboBox::from_id_salt(label)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for opt in options {
                ui.selectable_value(value, (*opt).to_string(), *opt);
            }
        });
    ui.end_row();
}
/// Combo whose options display a translated label but store an engine value.
fn mapped_combo_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    options: &'static [(&'static str, &'static str)],
    lang: Lang,
) {
    ui.label(label);
    let current = options
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, k)| tr(lang, k))
        .unwrap_or(value.as_str());
    egui::ComboBox::from_id_salt(label)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (v, k) in options {
                ui.selectable_value(value, (*v).to_string(), tr(lang, k));
            }
        });
    ui.end_row();
}
fn int_combo_row(ui: &mut egui::Ui, label: &str, value: &mut i32, options: &[(i32, &str)]) {
    ui.label(label);
    let current = options
        .iter()
        .find(|(v, _)| *v == *value)
        .map(|(_, t)| *t)
        .unwrap_or("?");
    egui::ComboBox::from_id_salt(label)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (v, t) in options {
                ui.selectable_value(value, *v, *t);
            }
        });
    ui.end_row();
}
fn check_row(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    ui.label(label);
    ui.checkbox(value, "");
    ui.end_row();
}
fn crit_row(ui: &mut egui::Ui, label: &str, on: &mut bool, thr: &mut f64) {
    ui.checkbox(on, label);
    ui.add_enabled(*on, egui::DragValue::new(thr).speed(0.1));
    ui.end_row();
}
fn crit_pair_row(ui: &mut egui::Ui, label: &str, on: &mut bool, thr: &mut [f64; 2]) {
    ui.checkbox(on, label);
    ui.horizontal(|ui| {
        ui.add_enabled(*on, egui::DragValue::new(&mut thr[0]).speed(0.1));
        ui.add_enabled(*on, egui::DragValue::new(&mut thr[1]).speed(0.1));
    });
    ui.end_row();
}

impl EarthMeshApp {
    fn tab_basics(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.basics"));
        ui.separator();
        egui::Grid::new("basics").num_columns(2).show(ui, |ui| {
            // Case identity first.
            text_row(ui, tr(lang, "f.expnme"), &mut self.mkgrd.experiment_name);
            text_row(ui, tr(lang, "f.base_dir"), &mut self.mkgrd.base_dir);
            ui.label("");
            ui.label("");
            ui.end_row();

            // Cascade: mesh type → model (output format) → grid shape.
            mapped_combo_row(
                ui,
                tr(lang, "f.mesh_type"),
                &mut self.mkgrd.mesh_type,
                MESH_TYPES,
                lang,
            );
            ui.label("");
            ui.weak(tr(lang, "mesh.custom_note"));
            ui.end_row();

            let allowed = output_formats_for(&self.mkgrd.mesh_type);
            if !allowed.contains(&self.mkgrd.output_format.as_str()) {
                self.mkgrd.output_format = allowed[0].to_string();
            }
            combo_row(
                ui,
                tr(lang, "f.output_format"),
                &mut self.mkgrd.output_format,
                allowed,
            );
            let refinement_supported = self.refinement_supported();
            if !refinement_supported {
                self.mkgrd.refine = false;
                self.refine.refine_spc = false;
                self.refine.refine_cal = false;
            }

            // The selected model's standard file is produced in pure Rust.
            check_row(ui, tr(lang, "f.gen_mpas"), &mut self.gen_output);

            let grids = grid_modes_for(&self.mkgrd.mesh_type);
            mapped_combo_row(
                ui,
                tr(lang, "f.mode_grid"),
                &mut self.mkgrd.mode_grid,
                grids,
                lang,
            );

            int_row(ui, tr(lang, "f.nxp"), &mut self.mkgrd.nxp, 1..=100_000);

            // Domain: global vs regional, with conditional boundary options.
            ui.label(tr(lang, "f.domain_mode"));
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.mkgrd.mask_domain_global,
                    true,
                    tr(lang, "opt.global"),
                );
                ui.selectable_value(
                    &mut self.mkgrd.mask_domain_global,
                    false,
                    tr(lang, "opt.regional"),
                );
            });
            ui.end_row();
            if !self.mkgrd.mask_domain_global {
                combo_row(
                    ui,
                    tr(lang, "f.domain_shape"),
                    &mut self.mkgrd.mask_domain_type,
                    REGION_TYPES,
                );
                match self.mkgrd.mask_domain_type.as_str() {
                    "bbox" => {
                        f64_row(ui, tr(lang, "dom.west"), &mut self.dom_bbox[0]);
                        f64_row(ui, tr(lang, "dom.east"), &mut self.dom_bbox[1]);
                        f64_row(ui, tr(lang, "dom.north"), &mut self.dom_bbox[2]);
                        f64_row(ui, tr(lang, "dom.south"), &mut self.dom_bbox[3]);
                    }
                    "circle" => {
                        f64_row(ui, tr(lang, "dom.clon"), &mut self.dom_circle[0]);
                        f64_row(ui, tr(lang, "dom.clat"), &mut self.dom_circle[1]);
                        f64_row(ui, tr(lang, "dom.radius"), &mut self.dom_circle[2]);
                    }
                    "close" => {
                        ui.label(tr(lang, "dom.poly_points"));
                        ui.vertical(|ui| {
                            let mut remove = None;
                            for i in 0..self.dom_close.len() {
                                ui.horizontal(|ui| {
                                    let p = &mut self.dom_close[i];
                                    ui.add(
                                        egui::DragValue::new(&mut p[0]).speed(0.1).prefix("lon "),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut p[1]).speed(0.1).prefix("lat "),
                                    );
                                    if self.dom_close.len() > 3 && ui.small_button("✕").clicked()
                                    {
                                        remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove {
                                self.dom_close.remove(i);
                            }
                            if ui.button(tr(lang, "dom.add_point")).clicked() {
                                let last = self.dom_close.last().copied().unwrap_or([115.0, 22.0]);
                                self.dom_close.push(last);
                            }
                        });
                        ui.end_row();
                    }
                    _ => {
                        ui.label("");
                        ui.weak(tr(lang, "dom.poly_note"));
                        ui.end_row();
                        text_row(
                            ui,
                            tr(lang, "f.domain_prefix"),
                            &mut self.mkgrd.mask_domain_fprefix,
                        );
                    }
                }
            }

            // Land/Ocean/coupled meshes need the sea-land source from NetCDF.
            if matches!(
                self.mkgrd.mesh_type.as_str(),
                "landmesh" | "oceanmesh" | "LOCmesh"
            ) {
                ui.label(tr(lang, "f.landtype_file"));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.mkgrd.landtype_file)
                            .desired_width(210.0),
                    );
                    if ui.button(tr(lang, "btn.browse")).clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("NetCDF", &["nc", "nc4"])
                            .pick_file()
                        {
                            self.mkgrd.landtype_file = p.display().to_string();
                        }
                    }
                });
                ui.end_row();
            }

            ui.label(tr(lang, "f.refine_master"));
            ui.add_enabled(
                refinement_supported,
                egui::Checkbox::new(&mut self.mkgrd.refine, ""),
            );
            ui.end_row();
            if !refinement_supported {
                ui.label("");
                ui.weak(tr(lang, "note.refine_unsupported"));
                ui.end_row();
            }
            int_row(ui, tr(lang, "f.threads"), &mut self.mkgrd.openmp, 1..=1024);
        });
    }

    fn refine_regions_ui(&mut self, ui: &mut egui::Ui, enabled: bool) {
        let lang = self.lang;
        let max_level = self.default_refine_level();
        match self.refine.mask_refine_spc_type.as_str() {
            "bbox" => {
                ui.label(tr(lang, "refine.regions"));
                ui.add_enabled_ui(enabled, |ui| {
                    ui.vertical(|ui| {
                        let mut remove = None;
                        for i in 0..self.refine_bboxes.len() {
                            ui.horizontal(|ui| {
                                let bbox = &mut self.refine_bboxes[i];
                                ui.add(
                                    egui::DragValue::new(&mut bbox.bounds[0])
                                        .speed(0.1)
                                        .prefix("W "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut bbox.bounds[1])
                                        .speed(0.1)
                                        .prefix("E "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut bbox.bounds[2])
                                        .speed(0.1)
                                        .prefix("N "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut bbox.bounds[3])
                                        .speed(0.1)
                                        .prefix("S "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut bbox.level)
                                        .range(1..=max_level)
                                        .prefix(format!("{} ", tr(lang, "refine.level"))),
                                );
                                if self.refine_bboxes.len() > 1 && ui.small_button("✕").clicked()
                                {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(index) = remove {
                            self.refine_bboxes.remove(index);
                        }
                        if ui.button(tr(lang, "refine.add_bbox")).clicked() {
                            self.refine_bboxes.push(RefineBboxRegion {
                                bounds: [110.0, 120.0, 35.0, 20.0],
                                level: max_level,
                            });
                        }
                    });
                });
                ui.end_row();
            }
            "circle" => {
                ui.label(tr(lang, "refine.regions"));
                ui.add_enabled_ui(enabled, |ui| {
                    ui.vertical(|ui| {
                        let mut remove = None;
                        for i in 0..self.refine_circles.len() {
                            ui.horizontal(|ui| {
                                let circle = &mut self.refine_circles[i];
                                ui.add(
                                    egui::DragValue::new(&mut circle.circle[0])
                                        .speed(0.1)
                                        .prefix("lon "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut circle.circle[1])
                                        .speed(0.1)
                                        .prefix("lat "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut circle.circle[2])
                                        .speed(1.0)
                                        .prefix("km "),
                                );
                                ui.add(
                                    egui::DragValue::new(&mut circle.level)
                                        .range(1..=max_level)
                                        .prefix(format!("{} ", tr(lang, "refine.level"))),
                                );
                                if self.refine_circles.len() > 1 && ui.small_button("✕").clicked()
                                {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(index) = remove {
                            self.refine_circles.remove(index);
                        }
                        if ui.button(tr(lang, "refine.add_circle")).clicked() {
                            self.refine_circles.push(RefineCircleRegion {
                                circle: [115.0, 25.0, 500.0],
                                level: max_level,
                            });
                        }
                    });
                });
                ui.end_row();
            }
            "close" => {
                ui.label(tr(lang, "refine.regions"));
                ui.add_enabled_ui(enabled, |ui| {
                    ui.vertical(|ui| {
                        let mut remove_polygon = None;
                        for polygon_index in 0..self.refine_closes.len() {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} {}",
                                    tr(lang, "refine.polygon"),
                                    polygon_index + 1
                                ));
                                ui.add(
                                    egui::DragValue::new(
                                        &mut self.refine_closes[polygon_index].level,
                                    )
                                    .range(1..=max_level)
                                    .prefix(format!("{} ", tr(lang, "refine.level"))),
                                );
                            });
                            let mut remove_point = None;
                            let can_remove_point =
                                self.refine_closes[polygon_index].points.len() > 3;
                            for point_index in 0..self.refine_closes[polygon_index].points.len() {
                                ui.horizontal(|ui| {
                                    let point =
                                        &mut self.refine_closes[polygon_index].points[point_index];
                                    ui.add(
                                        egui::DragValue::new(&mut point[0])
                                            .speed(0.1)
                                            .prefix("lon "),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut point[1])
                                            .speed(0.1)
                                            .prefix("lat "),
                                    );
                                    if can_remove_point && ui.small_button("✕").clicked() {
                                        remove_point = Some(point_index);
                                    }
                                });
                            }
                            if let Some(point_index) = remove_point {
                                self.refine_closes[polygon_index].points.remove(point_index);
                            }
                            ui.horizontal(|ui| {
                                if ui.button(tr(lang, "dom.add_point")).clicked() {
                                    self.refine_closes[polygon_index].points.push([115.0, 25.0]);
                                }
                                if self.refine_closes.len() > 1
                                    && ui.button(tr(lang, "refine.remove_polygon")).clicked()
                                {
                                    remove_polygon = Some(polygon_index);
                                }
                            });
                        }
                        if let Some(polygon_index) = remove_polygon {
                            self.refine_closes.remove(polygon_index);
                        }
                        if ui.button(tr(lang, "refine.add_polygon")).clicked() {
                            self.refine_closes.push(RefineCloseRegion {
                                points: vec![
                                    [110.0, 15.0],
                                    [125.0, 15.0],
                                    [125.0, 30.0],
                                    [110.0, 30.0],
                                ],
                                level: max_level,
                            });
                        }
                    });
                });
                ui.end_row();
            }
            _ => {
                ui.label(tr(lang, "f.spc_prefix"));
                ui.add_enabled(
                    enabled,
                    egui::TextEdit::singleline(&mut self.refine.mask_refine_spc_fprefix)
                        .desired_width(280.0),
                );
                ui.end_row();
            }
        }
    }

    fn tab_refinement(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.refinement"));
        ui.separator();
        let refinement_supported = self.refinement_supported();
        if !refinement_supported {
            self.normalize_refinement_for_mesh();
            ui.weak(tr(lang, "note.refine_unsupported"));
        } else if !self.mkgrd.refine {
            ui.weak(tr(lang, "note.refine_off"));
        }
        let mt = self.mkgrd.mesh_type.clone();
        let show_land = mt == "landmesh" || mt == "earthmesh" || mt == "LOCmesh";
        let show_ocean = mt == "oceanmesh" || mt == "earthmesh" || mt == "LOCmesh";
        let show_atmos = mt == "atmosmesh" || mt == "earthmesh" || mt == "LOCmesh";
        let atmos_only = mt == "atmosmesh";

        ui.add_enabled_ui(refinement_supported && self.mkgrd.refine, |ui| {
            egui::Grid::new("ref_ctrl").num_columns(2).show(ui, |ui| {
                check_row(ui, tr(lang, "f.refine_spc"), &mut self.refine.refine_spc);
                let spc = self.refine.refine_spc;
                ui.label(tr(lang, "f.max_iter_spc"));
                ui.add_enabled(
                    spc,
                    egui::DragValue::new(&mut self.refine.max_iter_spc)
                        .range(1..=SPECIFIED_REFINE_LEVEL_MAX),
                );
                ui.end_row();
                ui.label(tr(lang, "f.spc_shape"));
                ui.add_enabled_ui(spc, |ui| {
                    egui::ComboBox::from_id_salt("spc_type")
                        .selected_text(self.refine.mask_refine_spc_type.clone())
                        .show_ui(ui, |ui| {
                            for opt in REGION_TYPES {
                                ui.selectable_value(
                                    &mut self.refine.mask_refine_spc_type,
                                    (*opt).to_string(),
                                    *opt,
                                );
                            }
                        });
                });
                ui.end_row();
                self.refine_regions_ui(ui, spc);
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_enabled(
                    !atmos_only,
                    egui::Checkbox::new(&mut self.refine.refine_cal, tr(lang, "f.refine_cal")),
                );
                if atmos_only {
                    ui.weak(tr(lang, "note.cal_atmos"));
                }
            });
            let cal = self.refine.refine_cal && !atmos_only;
            ui.add_enabled_ui(cal, |ui| {
                egui::Grid::new("ref_cal").num_columns(2).show(ui, |ui| {
                    int_row(
                        ui,
                        tr(lang, "f.max_iter_cal"),
                        &mut self.refine.max_iter_cal,
                        0..=100,
                    );
                    combo_row(
                        ui,
                        tr(lang, "f.cal_shape"),
                        &mut self.refine.mask_refine_cal_type,
                        REGION_TYPES,
                    );
                    text_row(
                        ui,
                        tr(lang, "f.cal_prefix"),
                        &mut self.refine.mask_refine_cal_fprefix,
                    );
                    text_row(
                        ui,
                        tr(lang, "f.threshold_dir"),
                        &mut self.refine.threshold_dir,
                    );
                    text_row(
                        ui,
                        tr(lang, "f.landtype_file"),
                        &mut self.mkgrd.landtype_file,
                    );
                });
            });

            ui.add_space(6.0);
            if show_land {
                egui::CollapsingHeader::new(tr(lang, "sec.land_crit"))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("lnd1").num_columns(2).show(ui, |ui| {
                            ui.checkbox(
                                &mut self.refine.refine_num_landtypes,
                                tr(lang, "c.num_landtypes"),
                            );
                            ui.add_enabled(
                                self.refine.refine_num_landtypes,
                                egui::DragValue::new(&mut self.refine.th_num_landtypes)
                                    .range(0..=1000),
                            );
                            ui.end_row();
                            crit_row(
                                ui,
                                tr(lang, "c.area_mainland"),
                                &mut self.refine.refine_area_mainland,
                                &mut self.refine.th_area_mainland,
                            );
                            crit_row(
                                ui,
                                tr(lang, "c.lai_m"),
                                &mut self.refine.refine_onelayer_lnd[0],
                                &mut self.refine.th_onelayer_lnd[0],
                            );
                            crit_row(
                                ui,
                                tr(lang, "c.lai_s"),
                                &mut self.refine.refine_onelayer_lnd[1],
                                &mut self.refine.th_onelayer_lnd[1],
                            );
                            crit_row(
                                ui,
                                tr(lang, "c.slope_m"),
                                &mut self.refine.refine_onelayer_lnd[2],
                                &mut self.refine.th_onelayer_lnd[2],
                            );
                            crit_row(
                                ui,
                                tr(lang, "c.slope_s"),
                                &mut self.refine.refine_onelayer_lnd[3],
                                &mut self.refine.th_onelayer_lnd[3],
                            );
                        });
                        let keys = [
                            "c.ks_m",
                            "c.ks_s",
                            "c.ksol_m",
                            "c.ksol_s",
                            "c.tkdry_m",
                            "c.tkdry_s",
                            "c.tksatf_m",
                            "c.tksatf_s",
                            "c.tksatu_m",
                            "c.tksatu_s",
                        ];
                        egui::Grid::new("lnd2").num_columns(2).show(ui, |ui| {
                            for i in 0..10 {
                                crit_pair_row(
                                    ui,
                                    tr(lang, keys[i]),
                                    &mut self.refine.refine_twolayer_lnd[i],
                                    &mut self.refine.th_twolayer_lnd[i],
                                );
                            }
                        });
                    });
            }
            if show_ocean {
                egui::CollapsingHeader::new(tr(lang, "sec.ocean_crit")).show(ui, |ui| {
                    egui::Grid::new("ocn").num_columns(2).show(ui, |ui| {
                        crit_pair_row(
                            ui,
                            tr(lang, "c.sea_ratio"),
                            &mut self.refine.refine_sea_ratio,
                            &mut self.refine.th_sea_ratio,
                        );
                        let keys = [
                            "c.sst_m",
                            "c.sst_s",
                            "c.ssh_m",
                            "c.ssh_s",
                            "c.eke_m",
                            "c.eke_s",
                            "c.seaslope_m",
                            "c.seaslope_s",
                        ];
                        for i in 0..8 {
                            crit_row(
                                ui,
                                tr(lang, keys[i]),
                                &mut self.refine.refine_onelayer_ocn[i],
                                &mut self.refine.th_onelayer_ocn[i],
                            );
                        }
                    });
                });
            }
            if show_atmos {
                egui::CollapsingHeader::new(tr(lang, "sec.atmos_crit")).show(ui, |ui| {
                    egui::Grid::new("atm").num_columns(2).show(ui, |ui| {
                        crit_row(
                            ui,
                            tr(lang, "c.typhoon_m"),
                            &mut self.refine.refine_onelayer_atmos[0],
                            &mut self.refine.th_onelayer_atmos[0],
                        );
                        crit_row(
                            ui,
                            tr(lang, "c.typhoon_s"),
                            &mut self.refine.refine_onelayer_atmos[1],
                            &mut self.refine.th_onelayer_atmos[1],
                        );
                    });
                });
            }

            egui::CollapsingHeader::new(tr(lang, "sec.adv_refine")).show(ui, |ui| {
                egui::Grid::new("adv_ref").num_columns(2).show(ui, |ui| {
                    check_row(
                        ui,
                        tr(lang, "f.weak_concav"),
                        &mut self.refine.weak_concav_eliminate,
                    );
                    check_row(
                        ui,
                        tr(lang, "f.is_transition"),
                        &mut self.refine.is_transition,
                    );
                    check_row(ui, tr(lang, "f.iter_d"), &mut self.refine.iter_d);
                    ui.label(tr(lang, "f.halo"));
                    ui.horizontal(|ui| {
                        for i in 1..=9 {
                            ui.add(egui::DragValue::new(&mut self.refine.halo[i]).speed(1.0));
                        }
                    });
                    ui.end_row();
                    ui.label(tr(lang, "f.max_transition"));
                    ui.horizontal(|ui| {
                        for i in 1..=9 {
                            ui.add(
                                egui::DragValue::new(&mut self.refine.max_transition_row[i])
                                    .speed(1.0),
                            );
                        }
                    });
                    ui.end_row();
                });
            });
        });
    }

    fn tab_advanced(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        self.normalize_refinement_for_mesh();
        ui.heading(tr(lang, "head.advanced"));
        ui.separator();
        egui::CollapsingHeader::new(tr(lang, "sec.import")).show(ui, |ui| {
            egui::Grid::new("import").num_columns(2).show(ui, |ui| {
                text_row(ui, tr(lang, "f.mode_file"), &mut self.mkgrd.mode_file);
                combo_row(
                    ui,
                    tr(lang, "f.mode_file_desc"),
                    &mut self.mkgrd.mode_file_description,
                    MODE_FILE_DESCS,
                );
            });
        });
        egui::CollapsingHeader::new(tr(lang, "sec.smoothing")).show(ui, |ui| {
            egui::Grid::new("smooth").num_columns(2).show(ui, |ui| {
                int_row(
                    ui,
                    tr(lang, "f.niter"),
                    &mut self.mkgrd.niter,
                    0..=1_000_000,
                );
                f32_row(ui, tr(lang, "f.beta"), &mut self.mkgrd.beta);
                f32_row(ui, tr(lang, "f.relax"), &mut self.mkgrd.relax);
                int_row(
                    ui,
                    tr(lang, "f.niter_refine"),
                    &mut self.refine.niter_refine,
                    0..=1_000_000,
                );
                int_combo_row(
                    ui,
                    tr(lang, "f.spring_global"),
                    &mut self.refine.spring_global_type,
                    &[
                        (0, tr(lang, "opt.spring_none")),
                        (1, tr(lang, "opt.spring_olam")),
                    ],
                );
                int_row(ui, tr(lang, "f.num_rc"), &mut self.refine.num_rc, 0..=1000);
                combo_row(
                    ui,
                    tr(lang, "f.set_dis"),
                    &mut self.refine.set_dis_type,
                    SET_DIS_TYPES,
                );
                int_combo_row(
                    ui,
                    tr(lang, "f.spring_regional"),
                    &mut self.refine.spring_regional_type,
                    &[
                        (0, tr(lang, "opt.spring_none")),
                        (1, tr(lang, "opt.reg_each")),
                        (2, tr(lang, "opt.reg_final")),
                    ],
                );
                int_row(
                    ui,
                    tr(lang, "f.vertex_layers"),
                    &mut self.refine.vertex_pretect_layers,
                    0..=1000,
                );
            });
        });
        egui::CollapsingHeader::new(tr(lang, "sec.native_olam")).show(ui, |ui| {
            ui.weak(tr(lang, "note.native_olam"));
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.native_olam_mkgrd)
                    .desired_width(f32::INFINITY)
                    .desired_rows(8)
                    .hint_text("NL%ngrids = 2\nNL%ngrdll(2) = 1\nNL%grdrad(2,1) = 2500000.0\nNL%grdlat(2,1) = 25.0\nNL%grdlon(2,1) = 115.0"),
            );
        });
        egui::CollapsingHeader::new(tr(lang, "head.mask")).show(ui, |ui| {
            egui::Grid::new("adv_mask").num_columns(2).show(ui, |ui| {
                int_combo_row(
                    ui,
                    tr(lang, "f.gridnum"),
                    &mut self.mkgrd.gridnum_perdegree,
                    &[(120, "120"), (240, "240")],
                );
                f64_row(ui, tr(lang, "f.sea_ratio"), &mut self.mkgrd.mask_sea_ratio);
                check_row(ui, tr(lang, "f.mask_restart"), &mut self.mkgrd.mask_restart);
                check_row(
                    ui,
                    tr(lang, "f.isolated_ocean"),
                    &mut self.mkgrd.isolated_ocean,
                );
                check_row(ui, tr(lang, "f.patch_on"), &mut self.mkgrd.mask_patch_on);
                let patch = self.mkgrd.mask_patch_on;
                ui.label(tr(lang, "f.patch_shape"));
                ui.add_enabled_ui(patch, |ui| {
                    egui::ComboBox::from_id_salt("patch_type")
                        .selected_text(self.mkgrd.mask_patch_type.clone())
                        .show_ui(ui, |ui| {
                            for opt in REGION_TYPES {
                                ui.selectable_value(
                                    &mut self.mkgrd.mask_patch_type,
                                    (*opt).to_string(),
                                    *opt,
                                );
                            }
                        });
                });
                ui.end_row();
                ui.label(tr(lang, "f.patch_prefix"));
                ui.add_enabled(
                    patch,
                    egui::TextEdit::singleline(&mut self.mkgrd.mask_patch_fprefix)
                        .desired_width(280.0),
                );
                ui.end_row();
            });
        });
    }

    fn render_tab(&mut self, ui: &mut egui::Ui) {
        match self.tab {
            Tab::Basics => self.tab_basics(ui),
            Tab::Refinement => self.tab_refinement(ui),
            Tab::Advanced => self.tab_advanced(ui),
        }
    }

    fn render_search(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "search.results"));
        ui.separator();
        let q = self.search.to_lowercase();
        let matches: Vec<(&'static str, Tab)> = FIELD_INDEX
            .iter()
            .filter(|(k, _)| tr(lang, k).to_lowercase().contains(&q))
            .copied()
            .collect();
        if matches.is_empty() {
            ui.label(tr(lang, "search.none"));
            return;
        }
        let mut goto: Option<Tab> = None;
        for (k, tab) in matches {
            if ui
                .button(format!(
                    "{}   ·   {}",
                    tr(lang, k),
                    tr(lang, tab_nav_key(tab))
                ))
                .clicked()
            {
                goto = Some(tab);
            }
        }
        if let Some(t) = goto {
            self.tab = t;
            self.search.clear();
        }
    }

    /// Centre the basemap on the freshly-loaded mesh and pick a zoom that frames
    /// it within the actual map widget. A global mesh is sized to fill the width
    /// (its poles are stretched/clipped anyway); a regional mesh is fit whole.
    /// Web Mercator screen-x is linear in longitude, so 1° spans `256·2^z / 360`
    /// px; latitude is non-linear but a linear estimate is fine for framing.
    fn frame_mesh_view(&mut self, avail_w: f32, avail_h: f32) {
        if self.mesh_view.as_ref().is_none_or(|m| m.m_lon.is_empty()) {
            return;
        }
        // Frame to the user's region of interest when one exists. That includes
        // explicit regional domains and global grids with specified refinement:
        // the mesh remains global, but the initial view should show the refined
        // area instead of a full-world wireframe.
        let focus_extent = self
            .results_focus_domain()
            .and_then(|domain| domain.extent());
        let focused = focus_extent.is_some();
        let (lon_min, lon_max, lat_min, lat_max) = focus_extent.unwrap_or_else(|| {
            let mesh = self.mesh_view.as_ref().unwrap();
            let (mut lo0, mut lo1) = (f64::MAX, f64::MIN);
            let (mut la0, mut la1) = (f64::MAX, f64::MIN);
            for (&lo, &la) in mesh.m_lon.iter().zip(&mesh.m_lat) {
                let lo = ((lo + 180.0).rem_euclid(360.0)) - 180.0;
                lo0 = lo0.min(lo);
                lo1 = lo1.max(lo);
                la0 = la0.min(la);
                la1 = la1.max(la);
            }
            (lo0, lo1, la0, la1)
        });
        // Tolerate reversed entries (e.g. west > east) and centre on the extent.
        let (lon_min, lon_max) = (lon_min.min(lon_max), lon_min.max(lon_max));
        let (lat_min, lat_max) = (lat_min.min(lat_max), lat_min.max(lat_max));
        let clon = 0.5 * (lon_min + lon_max);
        let clat = 0.5 * (lat_min + lat_max);
        // Pad a region so it isn't drawn edge-to-edge.
        let pad = if focused { 1.3 } else { 1.0 };
        let lon_span = ((lon_max - lon_min) * pad).max(0.5);
        let lat_span = ((lat_max - lat_min) * pad).max(0.5);
        let zoom_w = (avail_w as f64 * 360.0 / (256.0 * lon_span)).log2();
        // Fill the width by default. The leftover available height is unreliable in
        // a short dock (it can read ~0 before the map widget is allocated), so only
        // constrain by height when it is clearly usable — e.g. the detached window.
        let zoom = if avail_h > 60.0 && lon_span < 350.0 {
            let zoom_h = (avail_h as f64 * 360.0 / (256.0 * lat_span)).log2();
            zoom_w.min(zoom_h)
        } else {
            zoom_w
        };
        self.map_memory.center_at(walkers::lon_lat(clon, clat));
        let _ = self.map_memory.set_zoom(zoom.clamp(0.0, 8.0));
    }

    fn results_ui(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        if self.output_files.is_empty() {
            ui.label(tr(lang, "results.empty"));
            return;
        }
        let hex = self.mkgrd.mode_grid == "hex";
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr(lang, "results.files")).strong());
            if let Some(m) = &self.mesh_view {
                // Hex cells are the W points (triangles are the vertices); tri is
                // the other way round.
                let (cells, verts) = if hex {
                    (m.w_lon.len(), m.m_lon.len())
                } else {
                    (m.m_lon.len(), m.w_lon.len())
                };
                if ui.button("Focus").clicked() {
                    let map_w = ui.available_width().max(360.0);
                    self.frame_mesh_view(map_w, 540.0);
                }
                ui.weak(format!(
                    "·  {} {}  ·  {} {}",
                    cells,
                    tr(lang, "results.cells"),
                    verts,
                    tr(lang, "results.vertices"),
                ));
            }
        });
        let files = self.output_files.clone();
        egui::ScrollArea::vertical()
            .max_height(36.0)
            .id_salt("files_list")
            .show(ui, |ui| {
                for f in &files {
                    let name = f
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if ui
                        .button(name)
                        .on_hover_text(f.display().to_string())
                        .clicked()
                    {
                        let _ = open::that(f);
                    }
                }
            });
        let class_counts = surface_class_counts(&self.cell_classes);
        if !class_counts.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Classes");
                for (code, count) in class_counts {
                    let stroke = surface_class_stroke(Some(code));
                    ui.colored_label(
                        stroke.color,
                        format!("{} {count}", surface_class_name(code)),
                    );
                }
            });
        }
        ui.separator();
        if self.mesh_view.is_some() {
            ui.weak(tr(lang, "results.map_hint"));
            let map_w = ui.available_width();
            let map_h = ui.available_height().max(240.0);
            // Frame on the first render after a run, now that the map widget's
            // real size is known (the dock and the detached window differ).
            if self.frame_pending {
                self.frame_mesh_view(map_w, map_h);
                self.frame_pending = false;
            }
            let domain = self.results_draw_domain();
            if let Some(tiles) = &mut self.tiles {
                // Offline Protomaps basemap with the mesh wireframe overlaid.
                let mesh = self.mesh_view.as_ref().unwrap();
                let map = walkers::Map::new(
                    Some(tiles as &mut dyn walkers::Tiles),
                    &mut self.map_memory,
                    walkers::lon_lat(0.0, 0.0),
                )
                // Plain wheel zooms (walkers defaults to ctrl+wheel, treating a
                // bare wheel as a vertical pan); drag still pans.
                .zoom_with_ctrl(false)
                .with_plugin(MeshOverlay {
                    mesh,
                    class_codes: &self.cell_classes,
                    domain,
                    hex,
                });
                ui.add_sized([map_w, map_h], map);
                ui.weak("© OpenStreetMap contributors · Protomaps");
            } else {
                // No bundled basemap -> equirectangular wireframe fallback.
                draw_mesh_2d(ui, self.mesh_view.as_ref().unwrap());
            }
        } else {
            ui.weak(tr(lang, "results.3d_soon"));
        }
    }
}

fn install_fonts(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Kailasa.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    let mut loaded = Vec::new();
    for (index, path) in CANDIDATES.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            let name = format!("earthmesh_fallback_{index}");
            fonts.font_data.insert(
                name.clone(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            loaded.push(name);
        }
    }
    if loaded.is_empty() {
        return;
    }
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(fam) = fonts.families.get_mut(&family) {
            for name in loaded.iter().rev() {
                fam.insert(0, name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    for (_s, font) in style.text_styles.iter_mut() {
        font.size *= 1.05;
    }
    ctx.set_global_style(style);
}

impl EarthMeshApp {
    fn theme(&self) -> theme::EarthMeshTheme {
        theme::EarthMeshTheme {
            dark: self.theme_dark,
        }
    }

    /// Apply a target template preset to the config (additive convenience; the user
    /// can still edit every field afterwards).
    fn apply_template(&mut self, t: ui_helpers::TargetTemplate) {
        self.mkgrd.mesh_type = t.mesh_type.to_string();
        self.mkgrd.mode_grid = t.mode_grid.to_string();
        self.mkgrd.output_format = t.output_format.to_string();
        self.mkgrd.nxp = t.default_nxp;
        self.mkgrd.mask_domain_global = t.global;
        self.mkgrd.refine = t.refine;
        self.log.push(format!("template: {}", t.id));
    }

    /// Render the quality dashboard from the run output dir's existing artifacts
    /// (read-only: quality_summary.json / run_manifest.json / worst_cells.geojson /
    /// quality_report.md). No schema change.
    fn render_quality_dashboard(
        &self,
        ui: &mut egui::Ui,
        theme: &theme::EarthMeshTheme,
        lang: Lang,
    ) {
        let dir = self
            .output_files
            .iter()
            .find_map(|p| p.parent().map(|d| d.to_path_buf()));
        let Some(dir) = dir else {
            components::empty_state(ui, tr(lang, "dash.empty"));
            return;
        };
        let d = ui_helpers::QualityDashboard::from_dir(&dir);

        ui.horizontal(|ui| {
            ui.label(tr(lang, "dash.verdict"));
            components::status_badge(ui, theme, &d.verdict, &d.verdict.to_uppercase());
            if let Some(status) = &d.manifest_status {
                ui.separator();
                ui.label(format!("{}: {}", tr(lang, "dash.run_status"), status));
            }
        })
        .response
        .on_hover_text(ui_helpers::tooltip("quality_status"));

        if !d.headline.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for (k, v) in &d.headline {
                    ui.label(format!("{k}: {v}"));
                    ui.separator();
                }
            });
        }

        if !d.top_warnings.is_empty() {
            components::status_message(
                ui,
                theme,
                components::MessageKind::Warning,
                tr(lang, "dash.warnings"),
            );
            for w in d.top_warnings.iter().take(8) {
                ui.label(format!("• {w}"));
            }
        }
        for w in d.manifest_warnings.iter().take(4) {
            components::status_message(ui, theme, components::MessageKind::Warning, w);
        }

        for s in &d.next_steps {
            components::status_message(ui, theme, components::MessageKind::Info, s);
        }

        if let Some(p) = &d.worst_cells_path {
            ui.label(format!("{}: {p}", tr(lang, "dash.worst_cells")));
        }
        if let Some(p) = &d.quality_report_path {
            if ui.button(tr(lang, "dash.open_report")).clicked() {
                let _ = open::that(p);
            }
        }
    }

    /// Result dir for the coupling/refinement panels: the hydro-workflow output dir if a
    /// workflow has run, else the mesh run's output dir.
    fn results_dir(&self) -> Option<PathBuf> {
        self.hydro_out_dir.clone().or_else(|| {
            self.output_files
                .iter()
                .find_map(|p| p.parent().map(|d| d.to_path_buf()))
        })
    }

    /// Render the R7 land/ocean coupling-quality summary from the result dir's
    /// `coupling_quality.json` (read-only; produced by `--coupling-quality-from-mesh` or
    /// `--hydro-workflow --mesh … --landtype …`).
    fn render_coupling_quality(
        &self,
        ui: &mut egui::Ui,
        theme: &theme::EarthMeshTheme,
        lang: Lang,
    ) {
        let s = self
            .results_dir()
            .as_deref()
            .map(ui_helpers::CouplingQualitySummary::from_dir)
            .unwrap_or_default();
        if !s.present {
            components::empty_state(ui, tr(lang, "coupling.empty"));
            return;
        }
        ui.horizontal(|ui| {
            ui.label(tr(lang, "dash.verdict"));
            components::status_badge(ui, theme, &s.verdict, &s.verdict.to_uppercase());
        });
        ui.horizontal_wrapped(|ui| {
            for (k, v) in &s.fields {
                ui.label(format!("{k}: {v}"));
                ui.separator();
            }
        });
    }

    /// Render the R8 refinement-plan summary from the result dir's
    /// `refinement_plan.json` (read-only; produced by `--plan-refinement-from-hydro` /
    /// `--hydro-workflow`).
    fn render_refinement_plan(&self, ui: &mut egui::Ui, lang: Lang) {
        let s = self
            .results_dir()
            .as_deref()
            .map(ui_helpers::RefinementPlanSummary::from_dir)
            .unwrap_or_default();
        if !s.present {
            components::empty_state(ui, tr(lang, "refine_plan.empty"));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for (k, v) in &s.fields {
                ui.label(format!("{k}: {v}"));
                ui.separator();
            }
        });
    }

    /// File pickers + a Run button that triggers the hydro workflow
    /// (`earthmesh_cli::run_hydro_workflow`): cells × corridors -> intersections +
    /// CoLM coupling CSV + R8 refinement plan + manifest, into a chosen out dir. Optional
    /// mesh + land-type add the R7 coupling-quality step. Runs on a background thread, so
    /// the slow NetCDF (mesh+land-type) path does not block the UI. The coupling /
    /// refinement panels above read the result. Additive: invokes the tested library fn.
    fn render_hydro_workflow_controls(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        lang: Lang,
    ) {
        let none = tr(lang, "hydro_wf.none");
        let path_label = |p: &Option<PathBuf>| {
            p.as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| none.to_string())
        };
        let pick_geojson = |ui: &mut egui::Ui, label: &str, slot: &mut Option<PathBuf>| {
            ui.horizontal(|ui| {
                if ui.button(label).clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("geojson", &["geojson", "json"])
                        .pick_file()
                    {
                        *slot = Some(p);
                    }
                }
                ui.label(path_label(slot));
            });
        };
        pick_geojson(ui, tr(lang, "hydro_wf.pick_cells"), &mut self.hydro_cells);
        pick_geojson(
            ui,
            tr(lang, "hydro_wf.pick_corridors"),
            &mut self.hydro_corridors,
        );
        ui.horizontal(|ui| {
            if ui.button(tr(lang, "hydro_wf.pick_out")).clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.hydro_out_dir = Some(p);
                }
            }
            ui.label(path_label(&self.hydro_out_dir));
        });
        // optional R7 mesh + land-type (NetCDF) — slow, hence the background thread.
        ui.horizontal(|ui| {
            if ui.button(tr(lang, "hydro_wf.pick_mesh")).clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("netcdf", &["nc", "nc4"])
                    .pick_file()
                {
                    self.hydro_mesh = Some(p);
                }
            }
            ui.label(path_label(&self.hydro_mesh));
        });
        ui.horizontal(|ui| {
            if ui.button(tr(lang, "hydro_wf.pick_landtype")).clicked() {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("netcdf", &["nc", "nc4"])
                    .pick_file()
                {
                    self.hydro_landtype = Some(p);
                }
            }
            ui.label(path_label(&self.hydro_landtype));
        });
        let ready = !self.running
            && !self.hydro_wf_running
            && self.hydro_cells.is_some()
            && self.hydro_corridors.is_some();
        ui.horizontal(|ui| {
            ui.add_enabled_ui(ready, |ui| {
                if ui.button(tr(lang, "hydro_wf.run")).clicked() {
                    self.run_hydro_workflow_now(ctx);
                }
            });
            if self.hydro_wf_running {
                ui.add(egui::Spinner::new());
                ui.label(tr(lang, "hydro_wf.running"));
            }
        });
    }

    /// Spawn the hydro workflow on a background thread (the mesh+land-type NetCDF path is
    /// slow); the result arrives via `hydro_wf_rx` and is drained in `poll_run`.
    fn run_hydro_workflow_now(&mut self, ctx: &egui::Context) {
        if self.hydro_wf_running {
            return;
        }
        let (Some(cells), Some(corridors)) =
            (self.hydro_cells.clone(), self.hydro_corridors.clone())
        else {
            self.log
                .push("hydro workflow: pick cells + corridors GeoJSON first".into());
            return;
        };
        if self.hydro_mesh.is_some() != self.hydro_landtype.is_some() {
            self.log
                .push("hydro workflow: pick both mesh + land-type, or neither".into());
            return;
        }
        let out_dir = self
            .hydro_out_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("earthmesh_hydro_workflow"));
        let mesh = self.hydro_mesh.clone();
        let landtype = self.hydro_landtype.clone();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let res = earthmesh_cli::run_hydro_workflow(
                &cells,
                &corridors,
                &out_dir,
                &["R2".to_string(), "R3".to_string()],
                0.0,
                false,
                None,
                3,
                None,
                mesh.as_deref(),
                landtype.as_deref(),
                120,
            )
            .map_err(|e| e.to_string());
            let _ = tx.send(res);
            ctx.request_repaint();
        });
        self.hydro_wf_rx = Some(rx);
        self.hydro_wf_running = true;
        self.log.push("hydro workflow: running…".into());
    }

    /// Drain a finished background hydro workflow (called from `poll_run`).
    fn poll_hydro_workflow(&mut self) {
        let Some(result) = self.hydro_wf_rx.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return;
        };
        self.hydro_wf_running = false;
        self.hydro_wf_rx = None;
        match result {
            Ok(r) => {
                if let Some(d) = r.manifest_path.parent() {
                    self.hydro_out_dir = Some(d.to_path_buf());
                }
                let coupling = r
                    .coupling_quality_verdict
                    .map(|v| format!(", coupling={v}"))
                    .unwrap_or_default();
                self.log.push(format!(
                    "hydro workflow done: {} intersection cells, {} coupling rows, {} refined{}",
                    r.intersection_cells, r.coupling_rows, r.cells_refined, coupling
                ));
            }
            Err(e) => self.log.push(format!("hydro workflow failed: {e}")),
        }
    }
}

impl eframe::App for EarthMeshApp {
    // eframe 0.34 requires `ui`; we keep the multi-panel layout in `update`
    // (still invoked by the run loop) and leave `ui` empty.
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    #[allow(deprecated)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_run();
        if self.running || self.hydro_wf_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        let lang = self.lang;

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.heading(tr(lang, "app.title"));
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.project_name)
                        .hint_text(tr(lang, "project.name"))
                        .desired_width(130.0),
                );
                ui.separator();
                ui.add_enabled_ui(!self.running, |ui| {
                    if ui.button(tr(lang, "btn.run")).clicked() {
                        self.start_run(ctx);
                    }
                });
                if ui
                    .add_enabled(self.running, egui::Button::new(tr(lang, "btn.cancel")))
                    .clicked()
                {
                    self.request_cancel();
                }
                if self.running {
                    ui.add(egui::Spinner::new());
                }
                ui.separator();
                if ui.button(tr(lang, "btn.load")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("namelist", &["nml"])
                        .set_directory(examples_root())
                        .pick_file()
                    {
                        self.load(path);
                    }
                }
                if ui.button(tr(lang, "btn.save")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("namelist", &["nml"])
                        .set_file_name("earthmesh.nml")
                        .set_directory(runtime_workdir())
                        .save_file()
                    {
                        self.save(path);
                    }
                }
                if ui.button(tr(lang, "btn.open_output")).clicked() {
                    self.open_output_dir();
                }
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(tr(lang, "search.placeholder"))
                        .desired_width(170.0),
                );
                ui.separator();
                // Target template selector (applies a preset to the config).
                let templates = ui_helpers::target_templates();
                let current = templates
                    .get(self.target_template)
                    .map(|t| tr(lang, t.name_key))
                    .unwrap_or("—");
                egui::ComboBox::from_id_salt("target_template")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (i, t) in templates.iter().enumerate() {
                            if ui
                                .selectable_label(self.target_template == i, tr(lang, t.name_key))
                                .on_hover_text(tr(lang, t.help_key))
                                .clicked()
                            {
                                self.target_template = i;
                                self.apply_template(*t);
                            }
                        }
                    })
                    .response
                    .on_hover_text(tr(lang, "tpl.tooltip"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut self.lang, Lang::Zh, "中文");
                    ui.selectable_value(&mut self.lang, Lang::En, "EN");
                    ui.label(tr(lang, "lang.label"));
                    ui.separator();
                    if ui
                        .selectable_label(self.theme_dark, "🌓")
                        .on_hover_text(tr(lang, "theme.toggle"))
                        .clicked()
                    {
                        self.theme_dark = !self.theme_dark;
                        self.theme().apply(ctx);
                    }
                    ui.checkbox(&mut self.expert_mode, tr(lang, "mode.expert"))
                        .on_hover_text(tr(lang, "mode.expert.help"));
                });
            });
            ui.add_space(2.0);
        });

        // Results dock with a detach-to-window control (egui multi-viewport: a
        // real, resizable/maximizable OS window).
        if self.results_detached {
            let mut close = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("results_window"),
                egui::ViewportBuilder::default()
                    .with_title(tr(lang, "results.window"))
                    .with_inner_size([760.0, 540.0]),
                |vctx, _class| {
                    egui::CentralPanel::default().show(vctx, |ui| {
                        ui.heading(tr(lang, "results.title"));
                        ui.separator();
                        self.results_ui(ui);
                    });
                    if vctx.input(|i| i.viewport().close_requested()) {
                        close = true;
                    }
                },
            );
            if close {
                self.results_detached = false;
                self.frame_pending = true; // re-frame for the dock's size
            }
        }

        // Cap the dock at half the window so it can never swallow the form (and
        // a persisted oversized height gets clamped back). Big-map viewing is the
        // detached window's job.
        let dock_max = (ctx.screen_rect().height() * 0.5).max(200.0);
        egui::TopBottomPanel::bottom("results_dock")
            .resizable(true)
            .default_height(220.0)
            .min_height(120.0)
            .max_height(dock_max)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.heading(tr(lang, "results.title"));
                    if ui.button(tr(lang, "results.detach")).clicked() {
                        self.results_detached = true;
                        self.frame_pending = true; // re-frame for the window's size
                    }
                });
                ui.separator();
                egui::CollapsingHeader::new(tr(lang, "dash.title"))
                    .default_open(true)
                    .show(ui, |ui| {
                        let theme = self.theme();
                        self.render_quality_dashboard(ui, &theme, lang);
                    });
                egui::CollapsingHeader::new(tr(lang, "coupling.title"))
                    .default_open(false)
                    .show(ui, |ui| {
                        let theme = self.theme();
                        self.render_coupling_quality(ui, &theme, lang);
                    });
                egui::CollapsingHeader::new(tr(lang, "refine_plan.title"))
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_refinement_plan(ui, lang);
                    });
                egui::CollapsingHeader::new(tr(lang, "hydro_wf.title"))
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_hydro_workflow_controls(ui, ctx, lang);
                    });
                ui.separator();
                if self.results_detached {
                    ui.weak(tr(lang, "results.dock"));
                } else {
                    self.results_ui(ui);
                }
            });

        egui::SidePanel::left("cases")
            .resizable(true)
            .default_width(190.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.heading(tr(lang, "cases.title"));
                ui.separator();
                let templates = bundled_templates();
                let recent = self.recent.clone();
                let mut to_load: Option<PathBuf> = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(egui::RichText::new(tr(lang, "cases.templates")).strong());
                    for (label, path) in templates {
                        if ui.button(label).clicked() {
                            to_load = Some(path);
                        }
                    }
                    if !recent.is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(tr(lang, "cases.recent")).strong());
                        for path in recent {
                            let label = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ui
                                .button(label)
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                to_load = Some(path);
                            }
                        }
                    }
                });
                if let Some(p) = to_load {
                    self.load(p);
                }
            });

        egui::SidePanel::right("run")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.heading(tr(lang, "run.title"));
                ui.separator();
                ui.label(tr(
                    lang,
                    if self.running {
                        "run.running"
                    } else {
                        "run.idle"
                    },
                ));
                let status = if self.status_detail.is_empty() {
                    tr(lang, self.status_key).to_string()
                } else {
                    format!("{} {}", tr(lang, self.status_key), self.status_detail)
                };
                ui.label(status);
                if let Some((phase, done, total)) = &self.progress {
                    let frac = if *total > 0 {
                        *done as f32 / *total as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(frac).text(format!("{phase} {done}/{total}")));
                }
                ui.add_space(8.0);
                ui.label(egui::RichText::new(tr(lang, "run.log")).strong());
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.log {
                            ui.monospace(line);
                        }
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.search.trim().is_empty() {
                egui::ScrollArea::vertical().show(ui, |ui| self.render_search(ui));
                return;
            }
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Basics, tr(lang, "nav.basics"));
                ui.selectable_value(&mut self.tab, Tab::Refinement, tr(lang, "nav.refinement"));
                ui.selectable_value(&mut self.tab, Tab::Advanced, tr(lang, "nav.advanced"));
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| self.render_tab(ui));
        });
    }
}

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1040.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "EarthMesh",
        native_options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            configure_style(&cc.egui_ctx);
            let mut app = EarthMeshApp::default();
            // Offline vector basemap, if bundled. Missing file → wireframe fallback.
            // tile_size 256 makes the map zoom equal the source tile zoom and,
            // crucially, zeroes walkers' tile-size zoom adjustment — its default
            // 1024 subtracts 2 from the zoom in mercator::tile_id, which underflows
            // (panics) for any map zoom below 2, e.g. a framed global mesh.
            app.tiles = basemap_path().map(|p| {
                walkers::PmTiles::with_style(
                    p,
                    walkers::Style::protomaps_light(),
                    cc.egui_ctx.clone(),
                )
                .with_tile_size(256)
            });
            app.load(default_example_path());
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn results_dir_prefers_hydro_workflow_out_dir() {
        let mut app = EarthMeshApp::default();
        // no inputs -> no result dir
        assert_eq!(app.results_dir(), None);
        // a mesh run output dir is used when present
        app.output_files = vec![PathBuf::from("/tmp/run42/preview.geojson")];
        assert_eq!(app.results_dir(), Some(PathBuf::from("/tmp/run42")));
        // a hydro-workflow out dir takes precedence
        app.hydro_out_dir = Some(PathBuf::from("/tmp/wf99"));
        assert_eq!(app.results_dir(), Some(PathBuf::from("/tmp/wf99")));
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "earthmesh_gui_{name}_{}_{}",
            std::process::id(),
            stamp
        ))
    }

    fn sample_mkgrd_with_invalid_mkrefine() -> String {
        r#"
&mkgrd
  NL%EXPNME = 'ATMOS_hex_N64_refine2_global'
  NL%base_dir = './cases/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'hex'
  NL%mode_file = 'none'
  NL%mode_file_description = 'none'
  NL%NXP = 64
  NL%refine = .TRUE.
  NL%gridnum_perdegree = 120
  NL%niter = 5000
  NL%beta = 1.0
  NL%relax = 0.035
  NL%openmp = 8
  NL%landtype_file = './input/landtype_usgs_update.nc'
  NL%mask_domain_global = .TRUE.
  NL%mask_domain_type = 'circle'
  NL%mask_domain_fprefix = 'none'
  NL%mask_restart = .FALSE.
  NL%mask_sea_ratio = 0.5
  NL%mask_patch_on = .FALSE.
  NL%mask_patch_type = 'close'
  NL%mask_patch_fprefix = 'none'
  NL%output_format = 'MPAS'
/
&mkrefine
  RL%refine_spc = definitely_not_bool
/
"#
        .to_string()
    }

    #[test]
    fn new_gui_case_defaults_threads_to_four() {
        let app = EarthMeshApp::default();

        assert_eq!(app.mkgrd.openmp, 4);
        assert!(app.combined_namelist().contains("  NL%openmp = 4\n"));
    }

    #[test]
    fn startup_default_example_keeps_threads_at_four() {
        let mut app = EarthMeshApp::default();

        app.load(default_example_path());

        assert_eq!(app.status_key, "status.loaded");
        assert_eq!(app.mkgrd.experiment_name, "quickstart_n16");
        assert!(!app.mkgrd.refine);
        assert_eq!(app.mkgrd.openmp, 4);
    }

    #[test]
    fn geodesic_preview_edges_subdivide_polar_segments() {
        let points = geodesic_edge_points(
            (-52.728481739687766, -83.7458016915313),
            (-23.228535450465927, -83.77011542296827),
            -38.0,
        );

        assert!(
            points.len() > 2,
            "polar mesh edges should be drawn as great-circle arcs"
        );
        for pair in points.windows(2) {
            assert!(
                (pair[1].0 - pair[0].0).abs() < 20.0,
                "subdivided preview segment still jumps in longitude: {pair:?}"
            );
        }
    }

    #[test]
    fn mesh_preview_rejects_nonlocal_hex_cell_corners() {
        let center = (5.1, 19.8);
        let corners = [
            (-33.84918607072149, -32.83221841300302),
            (10.086714743994946, 23.254218642233624),
            (9.5, 21.0),
        ];

        assert!(
            !preview_cell_is_local(center, &corners),
            "a preview cell must not connect far-away corners into map-crossing lines"
        );
    }

    #[test]
    fn mesh_preview_accepts_local_hex_cell_corners() {
        let center = (71.666, 23.889);
        let corners = [
            (68.892, 23.218),
            (69.356, 25.499),
            (71.607, 26.351),
            (72.399, 28.904),
            (74.354, 24.492),
            (74.366, 22.972),
        ];

        assert!(preview_cell_is_local(center, &corners));
    }

    #[test]
    fn mesh_preview_orders_refined_transition_cell_without_self_crossing() {
        let center: (f64, f64) = (104.0225705594479, 28.852722704937452);
        let mut corners: Vec<(f64, f64)> = vec![
            (104.34186468158608, 29.03451747503522),
            (102.25229182018321, 30.15930433803884),
            (103.50121563350347, 31.12392094450185),
            (105.81342373990657, 30.31926388990762),
            (106.24945523792911, 28.97939087675153),
            (106.04174299857882, 28.73257819900902),
        ];

        corners.sort_by(|a, b| {
            let ba = (a.1 - center.1).atan2(unwrap_lon_around(a.0, center.0) - center.0);
            let bb = (b.1 - center.1).atan2(unwrap_lon_around(b.0, center.0) - center.0);
            ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
        });
        assert!(
            preview_cell_has_self_intersection(center, &corners),
            "this regression sample demonstrates the old W-center sort crossing itself"
        );

        let ordered = order_preview_cell_corners(center, corners);

        assert!(
            !preview_cell_has_self_intersection(center, &ordered),
            "preview ordering must not draw tangled polygons in refined transition cells"
        );
    }

    #[test]
    fn global_specified_circle_refinement_focuses_without_masking_results_preview() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.mask_domain_global = true;
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.mask_refine_spc_type = "circle".to_string();
        app.refine_circles = vec![RefineCircleRegion {
            circle: [115.0, 25.0, 500.0],
            level: 1,
        }];

        let focus = app
            .results_focus_domain()
            .expect("global refinement should focus the refined region");

        assert!(focus.contains(115.0, 25.0));
        assert!(
            app.results_draw_domain().is_none(),
            "global+refinement is still a global mesh and must not mask away the rest of the mesh"
        );
        assert!(
            !focus.contains(0.0, 0.0),
            "the initial view should still zoom to the specified refinement region"
        );
    }

    #[test]
    fn frame_global_refinement_preview_centers_on_specified_region() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.mask_domain_global = true;
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.mask_refine_spc_type = "circle".to_string();
        app.refine_circles = vec![RefineCircleRegion {
            circle: [115.0, 25.0, 500.0],
            level: 1,
        }];
        app.mesh_view = Some(earthmesh_cli::GridfileMeshPoints {
            m_lon: vec![-180.0, 0.0, 180.0],
            m_lat: vec![-80.0, 0.0, 80.0],
            w_lon: vec![-180.0, 0.0, 180.0],
            w_lat: vec![-80.0, 0.0, 80.0],
            m_to_w: Vec::new(),
            w_to_m: Vec::new(),
            n_w: Vec::new(),
            w_to_m_width: 0,
        });

        app.frame_mesh_view(760.0, 540.0);

        let center = app.map_memory.detached().expect("map should be centered");
        assert!(
            (center.x() - 115.0).abs() < 1.0,
            "center lon={}",
            center.x()
        );
        assert!((center.y() - 25.0).abs() < 1.0, "center lat={}", center.y());
        assert!(
            app.map_memory.zoom() > 5.0,
            "global+refinement preview should start zoomed into the refined region, got {}",
            app.map_memory.zoom()
        );
    }

    #[test]
    fn changed_experiment_name_targets_a_new_case_dir_and_run_namelist() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.base_dir = "./cases/".to_string();
        app.mkgrd.experiment_name = "case_from_loaded_nml".to_string();

        app.mkgrd.experiment_name = "case_from_current_run_button".to_string();

        assert!(app
            .output_dir()
            .ends_with(Path::new("cases/case_from_current_run_button")));
        assert!(app
            .run_namelist()
            .contains("NL%EXPNME = 'case_from_current_run_button'"));
        assert!(!app.run_namelist().contains("case_from_loaded_nml"));
    }

    #[test]
    fn land_and_ocean_meshes_request_landtype_masked_outputs_without_standard_output() {
        let mut app = EarthMeshApp::default();
        app.gen_output = false;
        app.mkgrd.landtype_file = "./input/landtype_usgs_update.nc".to_string();

        app.mkgrd.mesh_type = "landmesh".to_string();
        assert!(app.landtype_mask_postprocess_required());

        app.mkgrd.mesh_type = "oceanmesh".to_string();
        assert!(app.landtype_mask_postprocess_required());

        app.mkgrd.mesh_type = "atmosmesh".to_string();
        assert!(!app.landtype_mask_postprocess_required());

        app.mkgrd.mesh_type = "landmesh".to_string();
        app.mkgrd.landtype_file = "none".to_string();
        assert!(!app.landtype_mask_postprocess_required());
    }

    #[test]
    fn run_namelist_resolves_relative_landtype_file_for_land_and_ocean_meshes() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.mesh_type = "landmesh".to_string();
        app.mkgrd.landtype_file = "./input/landtype_usgs_update.nc".to_string();

        let nml = app.run_namelist();
        let resolved = runtime_workdir().join("input/landtype_usgs_update.nc");

        assert!(nml.contains(&format!("NL%landtype_file = '{}'", resolved.display())));
    }

    #[test]
    fn mpas_mesh_preserves_specified_refinement_but_disables_calculated_refinement() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.mesh_type = "atmosmesh".to_string();
        app.mkgrd.output_format = "MPAS".to_string();
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.refine_cal = true;

        let nml = app.run_namelist();

        assert!(nml.contains("NL%refine = .TRUE."));
        assert!(nml.contains("RL%refine_spc = .TRUE."));
        assert!(nml.contains("RL%refine_cal = .FALSE."));
    }

    #[test]
    fn global_specified_atmos_refinement_preserves_final_global_spring() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.mesh_type = "atmosmesh".to_string();
        app.mkgrd.mask_domain_global = true;
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.refine_cal = true;
        app.refine.spring_global_type = 1;
        app.refine.spring_regional_type = 0;

        let nml = app.run_namelist();

        assert!(nml.contains("RL%refine_spc = .TRUE."));
        assert!(nml.contains("RL%refine_cal = .FALSE."));
        assert!(
            nml.contains("RL%SpringGlobal_type = 1"),
            "fixed global spring must be preserved in generated namelists"
        );

        app.normalize_refinement_for_mesh();
        assert_eq!(app.refine.spring_global_type, 1);
        assert!(!app.refine.refine_cal);
    }

    #[test]
    fn specified_circle_refinement_respects_per_region_levels() {
        let dir = unique_temp_dir("circle_refine_levels");
        let mut app = EarthMeshApp::default();
        app.mkgrd.mesh_type = "landmesh".to_string();
        app.mkgrd.output_format = "CoLM".to_string();
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.max_iter_spc = 3;
        app.refine.mask_refine_spc_type = "circle".to_string();
        app.refine_circles = vec![
            RefineCircleRegion {
                circle: [100.0, 20.0, 50.0],
                level: 1,
            },
            RefineCircleRegion {
                circle: [110.0, 25.0, 75.0],
                level: 3,
            },
        ];

        let prefix = app
            .generate_refine_spc_files(&dir)
            .expect("generate circle refine mask")
            .expect("circle refine prefix");
        let mask =
            earthmesh_cli::read_circle_mask_netcdf(dir.join("earthmesh_refine_circle_001.nc4"))
                .expect("read first generated circle mask");
        let mid_mask =
            earthmesh_cli::read_circle_mask_netcdf(dir.join("earthmesh_refine_circle_002.nc4"))
                .expect("read second generated circle mask");
        let final_mask =
            earthmesh_cli::read_circle_mask_netcdf(dir.join("earthmesh_refine_circle_003.nc4"))
                .expect("read final generated circle mask");

        assert_eq!(prefix, dir.join("earthmesh_refine_circle"));
        assert_eq!(
            app.refine.mask_refine_spc_fprefix,
            prefix.display().to_string()
        );
        assert_eq!(mask.refine_degree, 1);
        assert_eq!(mid_mask.refine_degree, 2);
        assert_eq!(final_mask.refine_degree, 3);
        assert_eq!(mask.points.len(), 2);
        assert_eq!(mask.points[1].lon, 110.0);
        assert_eq!(mask.radius_km, vec![50.0, 75.0]);
        assert_eq!(mid_mask.points.len(), 1);
        assert_eq!(mid_mask.points[0].lon, 110.0);
        assert_eq!(mid_mask.radius_km, vec![75.0]);
        assert_eq!(final_mask.points.len(), 1);
        assert_eq!(final_mask.points[0].lon, 110.0);
        assert_eq!(final_mask.radius_km, vec![75.0]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn specified_refinement_defaults_to_five_independent_region_levels() {
        let mut app = EarthMeshApp::default();

        assert_eq!(app.refine.max_iter_spc, 5);
        assert_eq!(app.default_refine_level(), 5);

        app.refine.max_iter_spc = 2;

        assert_eq!(
            app.default_refine_level(),
            5,
            "region level choices stay 1..5 even when a loaded namelist has max_iter_spc=2"
        );
    }

    #[test]
    fn staging_specified_refine_files_does_not_rewrite_gui_max_passes() {
        let dir = unique_temp_dir("circle_refine_preserve_max_passes");
        let mut app = EarthMeshApp::default();
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.max_iter_spc = 5;
        app.refine.mask_refine_spc_type = "circle".to_string();
        app.refine_circles = vec![
            RefineCircleRegion {
                circle: [115.0, 25.0, 500.0],
                level: 2,
            },
            RefineCircleRegion {
                circle: [90.0, 25.0, 500.0],
                level: 2,
            },
        ];

        let prefix = app
            .generate_refine_spc_files(&dir)
            .expect("generate circle refine mask")
            .expect("circle refine prefix");

        assert_eq!(prefix, dir.join("earthmesh_refine_circle"));
        assert_eq!(
            app.refine.max_iter_spc, 5,
            "staging masks for level-2 regions must not change the visible GUI maximum"
        );
        let empty_third =
            earthmesh_cli::read_circle_mask_netcdf(dir.join("earthmesh_refine_circle_003.nc4"))
                .expect("read third generated circle mask");
        let empty_final =
            earthmesh_cli::read_circle_mask_netcdf(dir.join("earthmesh_refine_circle_005.nc4"))
                .expect("read final generated circle mask");
        assert_eq!(empty_third.refine_degree, 3);
        assert!(
            empty_third.points.is_empty(),
            "level-2 regions should produce an empty degree-3 mask under the global cap"
        );
        assert_eq!(empty_final.refine_degree, 5);
        assert!(
            empty_final.points.is_empty(),
            "level-2 regions should produce an empty degree-5 mask under the global cap"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn run_namelist_preserves_specified_refine_max_passes() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.max_iter_spc = 5;
        app.refine.mask_refine_spc_type = "circle".to_string();
        app.refine_circles = vec![
            RefineCircleRegion {
                circle: [115.0, 25.0, 500.0],
                level: 2,
            },
            RefineCircleRegion {
                circle: [90.0, 25.0, 500.0],
                level: 2,
            },
        ];

        let nml = app.run_namelist();

        assert!(
            nml.contains("RL%max_iter_spc = 5"),
            "run namelist must preserve the global specified-refine pass cap"
        );
    }

    #[test]
    fn native_olam_mkgrd_lines_are_inserted_into_generated_namelist() {
        let mut app = EarthMeshApp::default();
        app.native_olam_mkgrd = "&mkgrd\nNL%ngrids = 2\nNL%ngrdll(2) = 1\nNL%grdrad(2,1) = 2500000.0\nNL%grdlat(2,1) = 25.0\nNL%grdlon(2,1) = 115.0\n/".to_string();

        let nml = app.run_namelist();

        assert_eq!(nml.matches("&mkgrd").count(), 1);
        assert!(nml.contains("  NL%ngrids = 2\n"));
        assert!(nml.contains("  NL%grdrad(2,1) = 2500000.0\n"));
        assert!(nml.contains("  NL%grdlon(2,1) = 115.0\n/\n"));
    }

    #[test]
    fn native_olam_mkgrd_lines_are_extracted_from_loaded_namelist() {
        let nml = "&mkgrd\n  NL%EXPNME='case_native'\n  NL%NXP=6\n  NL%ngrids=2\n  NL%ngrdll(2)=1\n  NL%grdrad(2,1)=2500000.0\n  NL%grdlat(2,1)=25.0\n  NL%grdlon(2,1)=115.0\n  NL%mask_domain_global=.true.\n/\n";

        let native = extract_native_olam_mkgrd_lines(nml);

        assert!(native.contains("NL%ngrids=2"));
        assert!(native.contains("NL%ngrdll(2)=1"));
        assert!(native.contains("NL%grdrad(2,1)=2500000.0"));
        assert!(!native.contains("NL%mask_domain_global"));
    }

    #[test]
    fn specified_bbox_refinement_respects_per_region_levels() {
        let dir = unique_temp_dir("bbox_refine_levels");
        let mut app = EarthMeshApp::default();
        app.mkgrd.mesh_type = "LOCmesh".to_string();
        app.mkgrd.output_format = "CoLM".to_string();
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.max_iter_spc = 3;
        app.refine.mask_refine_spc_type = "bbox".to_string();
        app.refine_bboxes = vec![
            RefineBboxRegion {
                bounds: [100.0, 110.0, 30.0, 20.0],
                level: 1,
            },
            RefineBboxRegion {
                bounds: [120.0, 130.0, 25.0, 15.0],
                level: 3,
            },
        ];

        let prefix = app
            .generate_refine_spc_files(&dir)
            .expect("generate bbox refine mask")
            .expect("bbox refine prefix");
        let mask = earthmesh_cli::read_bbox_mask_netcdf(dir.join("earthmesh_refine_bbox_001.nc4"))
            .expect("read first generated bbox mask");
        let mid_mask =
            earthmesh_cli::read_bbox_mask_netcdf(dir.join("earthmesh_refine_bbox_002.nc4"))
                .expect("read second generated bbox mask");
        let final_mask =
            earthmesh_cli::read_bbox_mask_netcdf(dir.join("earthmesh_refine_bbox_003.nc4"))
                .expect("read final generated bbox mask");

        assert_eq!(prefix, dir.join("earthmesh_refine_bbox"));
        assert_eq!(
            app.refine.mask_refine_spc_fprefix,
            prefix.display().to_string()
        );
        assert_eq!(mask.refine_degree, 1);
        assert_eq!(mid_mask.refine_degree, 2);
        assert_eq!(final_mask.refine_degree, 3);
        assert_eq!(mask.points.len(), 2);
        assert_eq!(mask.points[1].west, 120.0);
        assert_eq!(mask.points[1].south, 15.0);
        assert_eq!(mid_mask.points.len(), 1);
        assert_eq!(mid_mask.points[0].west, 120.0);
        assert_eq!(mid_mask.points[0].south, 15.0);
        assert_eq!(final_mask.points.len(), 1);
        assert_eq!(final_mask.points[0].west, 120.0);
        assert_eq!(final_mask.points[0].south, 15.0);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn specified_close_refinement_respects_per_polygon_levels() {
        let dir = unique_temp_dir("close_refine_levels");
        let mut app = EarthMeshApp::default();
        app.mkgrd.mesh_type = "landmesh".to_string();
        app.mkgrd.output_format = "CoLM".to_string();
        app.mkgrd.refine = true;
        app.refine.refine_spc = true;
        app.refine.max_iter_spc = 4;
        app.refine.mask_refine_spc_type = "close".to_string();
        app.refine_closes = vec![
            RefineCloseRegion {
                points: vec![[100.0, 10.0], [110.0, 10.0], [110.0, 20.0], [100.0, 20.0]],
                level: 1,
            },
            RefineCloseRegion {
                points: vec![[120.0, 30.0], [130.0, 30.0], [130.0, 40.0], [120.0, 40.0]],
                level: 4,
            },
        ];

        let prefix = app
            .generate_refine_spc_files(&dir)
            .expect("generate close refine masks")
            .expect("close refine prefix");
        let first =
            earthmesh_cli::read_close_mask_netcdf(dir.join("earthmesh_refine_close_001_001.nc4"))
                .expect("read first close mask");
        let second =
            earthmesh_cli::read_close_mask_netcdf(dir.join("earthmesh_refine_close_001_002.nc4"))
                .expect("read second close mask");
        let mid_first =
            earthmesh_cli::read_close_mask_netcdf(dir.join("earthmesh_refine_close_002_001.nc4"))
                .expect("read second-pass close mask");
        let final_first =
            earthmesh_cli::read_close_mask_netcdf(dir.join("earthmesh_refine_close_004_001.nc4"))
                .expect("read final first close mask");

        assert_eq!(prefix, dir.join("earthmesh_refine_close"));
        assert_eq!(
            app.refine.mask_refine_spc_fprefix,
            prefix.display().to_string()
        );
        assert_eq!(first.refine_degree, 1);
        assert_eq!(second.refine_degree, 1);
        assert_eq!(mid_first.refine_degree, 2);
        assert_eq!(final_first.refine_degree, 4);
        assert_eq!(first.points[0].lon, 100.0);
        assert_eq!(second.points[0].lon, 120.0);
        assert_eq!(mid_first.points[0].lon, 120.0);
        assert_eq!(final_first.points[0].lon, 120.0);
        assert!(!dir.join("earthmesh_refine_close_002_002.nc4").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn close_domain_builds_a_grid_region_for_output_carving() {
        let mut app = EarthMeshApp::default();
        app.mkgrd.mask_domain_global = false;
        app.mkgrd.mask_domain_type = "close".to_string();
        app.dom_close = vec![[100.0, 10.0], [110.0, 10.0], [110.0, 20.0], [100.0, 20.0]];

        let Some(earthmesh_cli::GridRegion::Close { points }) = app.regional_grid_region() else {
            panic!("close domain should create a close GridRegion");
        };

        assert_eq!(points.len(), 4);
        assert!(DomainMask::Close {
            points: app.dom_close.clone()
        }
        .contains(105.0, 15.0));
        assert!(!DomainMask::Close {
            points: app.dom_close.clone()
        }
        .contains(120.0, 15.0));
    }

    #[test]
    fn quickstart_gridfile_can_populate_results_preview() {
        let dir = unique_temp_dir("results_preview");
        let grid_dir = dir.join("gridfile");
        fs::create_dir_all(&grid_dir).expect("create gridfile dir");
        let gridfile = grid_dir.join("gridfile_NXP0002_01_hex.nc4");
        let mesh = earthmesh_cli::UnstructuredMesh {
            m_points: vec![
                earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
                earthmesh_cli::LonLatPoint {
                    lon: 10.0,
                    lat: 20.0,
                },
            ],
            w_points: vec![
                earthmesh_cli::LonLatPoint { lon: 0.0, lat: 0.0 },
                earthmesh_cli::LonLatPoint {
                    lon: 30.0,
                    lat: 40.0,
                },
                earthmesh_cli::LonLatPoint {
                    lon: 50.0,
                    lat: 60.0,
                },
            ],
            m_to_w: vec![[1, 1, 1], [2, 3, 1]],
            w_to_m: vec![vec![1], vec![2, 1, 1, 1, 1], vec![2, 1, 1, 1, 1, 1]],
            n_w_to_m: vec![1, 5, 6],
        };
        earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh).expect("write gridfile");

        let files = collect_outputs(&dir);
        let preview = files
            .iter()
            .find(|p| p.to_string_lossy().contains("gridfile"))
            .or_else(|| files.first())
            .and_then(|p| earthmesh_cli::read_gridfile_mesh_points(p).ok());

        assert!(
            preview
                .as_ref()
                .is_some_and(|m| !m.m_lon.is_empty() && !m.w_lon.is_empty()),
            "result output should expose a readable gridfile preview"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn default_gui_run_writes_preview_gridfile() {
        let dir = unique_temp_dir("default_run");
        fs::create_dir_all(&dir).expect("create temp dir");
        let nml = dir.join("earthmesh_gui_run.nml");
        let mut app = EarthMeshApp::default();
        app.load(default_example_path());
        app.mkgrd.experiment_name = format!(
            "quickstart_gui_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        fs::write(&nml, app.run_namelist()).expect("write staged namelist");

        let run = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
            &nml,
            runtime_workdir(),
            100_000,
            0,
            None,
            None,
            None,
            1,
            None,
        )
        .expect("run default GUI namelist");
        assert!(
            matches!(
                run,
                earthmesh_cli::MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch(
                    earthmesh_cli::MkgrdTopLevelDispatchRunReport::Gridinit(_)
                )
            ),
            "default GUI namelist should run gridinit, got {run:?}"
        );

        let files = collect_outputs(&app.output_dir());
        assert!(
            files.iter().any(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("gridfile_NXP"))
            }),
            "default GUI run should leave a gridfile under {}",
            app.output_dir().display()
        );

        let _ = fs::remove_dir_all(app.output_dir());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_reports_invalid_mkrefine_without_overwriting_current_refine() {
        let dir = unique_temp_dir("invalid_refine");
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("case.nml");
        fs::write(&path, sample_mkgrd_with_invalid_mkrefine()).expect("write sample");

        let mut app = EarthMeshApp::default();
        app.refine.refine_spc = true;
        let previous_refine = app.refine.clone();

        app.load(path);

        assert_eq!(app.status_key, "status.parse_error");
        assert_eq!(app.refine, previous_refine);
        assert!(app.loaded_path.is_none());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn completed_run_uses_structured_output_dir_when_note_is_present() {
        let dir = unique_temp_dir("run_result");
        fs::create_dir_all(&dir).expect("create output dir");
        let mesh_file = dir.join("gridfile.nc");
        fs::write(&mesh_file, "not a real netcdf, only checking discovery").expect("write nc");

        let (tx, rx) = mpsc::channel();
        tx.send(RunMsg::Done(Ok(RunResult {
            output_dir: dir.clone(),
            note: Some("post output written".to_string()),
            started_at: UNIX_EPOCH,
        })))
        .expect("send run result");

        let mut app = EarthMeshApp::default();
        app.running = true;
        app.run_rx = Some(rx);
        app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));

        app.poll_run();

        assert_eq!(app.status_key, "status.run_done");
        assert_eq!(app.status_detail, dir.display().to_string());
        assert_eq!(app.output_files, vec![mesh_file]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn completed_run_lists_only_outputs_modified_after_run_start() {
        let dir = unique_temp_dir("run_result_since");
        fs::create_dir_all(&dir).expect("create output dir");
        let old_file = dir.join("old.nc");
        fs::write(&old_file, "old").expect("write old nc");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let started_at = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let new_file = dir.join("new.nc");
        fs::write(&new_file, "new").expect("write new nc");

        let (tx, rx) = mpsc::channel();
        tx.send(RunMsg::Done(Ok(RunResult {
            output_dir: dir.clone(),
            note: None,
            started_at,
        })))
        .expect("send run result");

        let mut app = EarthMeshApp::default();
        app.output_files = vec![old_file.clone()];
        app.running = true;
        app.run_rx = Some(rx);
        app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));

        app.poll_run();

        assert_eq!(app.output_files, vec![new_file]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn preview_prefers_landtype_masked_gridfile_over_global_gridfile() {
        let base = PathBuf::from("/tmp/earthmesh_case");
        let global = base.join("gridfile/gridfile_NXP0016_hex.nc4");
        let masked = base.join("masked/gridfile_NXP0016_hex_landmesh.nc4");
        let files = vec![global, masked.clone()];

        assert_eq!(
            preview_output_path(&files).as_deref(),
            Some(masked.as_path())
        );
    }

    #[test]
    fn preview_prefers_regional_gridfile_over_masked_gridfile() {
        let base = PathBuf::from("/tmp/earthmesh_case");
        let global = base.join("gridfile/gridfile_NXP0016_hex.nc4");
        let masked = base.join("masked/gridfile_NXP0016_hex_landmesh.nc4");
        let regional = base.join("regional/gridfile_NXP0016_hex_regional.nc4");
        let files = vec![global, masked, regional.clone()];

        assert_eq!(
            preview_output_path(&files).as_deref(),
            Some(regional.as_path())
        );
    }

    #[test]
    fn preview_prefers_final_result_gridfile_over_refine_step_gridfile() {
        let base = PathBuf::from("/tmp/earthmesh_case");
        let step = base.join("gridfile/gridfile_NXP0016_01_hex.nc4");
        let final_grid = base.join("result/gridfile_NXP0016_hex.nc4");
        let files = vec![step, final_grid.clone()];

        assert_eq!(
            preview_output_path(&files).as_deref(),
            Some(final_grid.as_path()),
            "refined runs should preview the final result gridfile, not the first refine step"
        );
    }

    #[test]
    fn displayed_outputs_keep_only_current_preview_netcdf() {
        let base = PathBuf::from("/tmp/earthmesh_case");
        let global = base.join("gridfile/gridfile_NXP0128_01_hex.nc4");
        let regional = base.join("regional/gridfile_NXP0128_hex_regional.nc4");
        let coupling = base.join("standard/CoLM_NXP0128_hex_coupling.nc4");
        let files = vec![global, regional.clone(), coupling];
        let preview = preview_output_path(&files);

        assert_eq!(
            preview_display_files(preview.as_ref()),
            vec![regional],
            "Results should list only the mesh NetCDF currently used by the preview"
        );
    }

    #[test]
    fn surface_classes_match_colm_coupling_points_to_hex_cells_with_placeholders() {
        let dir = unique_temp_dir("surface_classes");
        let standard = dir.join("standard");
        fs::create_dir_all(&standard).expect("create standard dir");
        let csv = standard.join("cells.csv");
        let nc = standard.join("CoLM_NXP0002_hex_coupling.nc4");
        let manifest = standard.join("manifest.json");
        fs::write(&manifest, "{}").expect("write manifest");
        fs::write(
            &csv,
            "cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell\n\
case_1,1,110.000000,20.000000,LAND,false,none,0.0,0.0,false,none,0.0,0.0,0.0\n\
case_2,2,111.000000,21.000000,OCEAN,false,none,0.0,0.0,false,none,0.0,0.0,0.0\n\
case_3,3,112.000000,22.000000,COAST,false,none,0.0,0.0,true,COAST,0.5,0.0,0.0\n",
        )
        .expect("write csv");
        earthmesh_cli::write_colm_coupling_netcdf_from_csv(&csv, &nc, "case", &manifest)
            .expect("write coupling");
        let mesh = earthmesh_cli::GridfileMeshPoints {
            m_lon: vec![0.0],
            m_lat: vec![0.0],
            w_lon: vec![0.0, 110.0, 111.0, 112.0],
            w_lat: vec![0.0, 20.0, 21.0, 22.0],
            m_to_w: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
        };
        let classes = surface_classes_for_mesh(&[nc], &mesh, true, "", 1).expect("read classes");

        assert_eq!(classes, vec![0, 1, 2, 3]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn preview_id_mapping_handles_mask_postproc_two_placeholder_rows() {
        let lon = vec![0.0, 0.0, 10.0, 20.0];
        let lat = vec![0.0, 0.0, 30.0, 40.0];

        assert_eq!(gridfile_row_index_for_id(1, &lon, &lat), None);
        assert_eq!(gridfile_row_index_for_id(2, &lon, &lat), Some(2));
        assert_eq!(gridfile_row_index_for_id(3, &lon, &lat), Some(3));
    }

    #[test]
    fn preview_id_mapping_keeps_standard_one_based_gridfile_rows() {
        let lon = vec![10.0, 20.0, 30.0];
        let lat = vec![40.0, 50.0, 60.0];

        assert_eq!(gridfile_row_index_for_id(1, &lon, &lat), Some(0));
        assert_eq!(gridfile_row_index_for_id(3, &lon, &lat), Some(2));
    }
}
