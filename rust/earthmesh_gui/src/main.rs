//! EarthMesh desktop GUI.
//!
//! Increment 5: a user-centred form. Options are reorganised by task into three
//! tabs — Basics / Refinement / Advanced — with friendly labels, only the mesh
//! shapes the engine actually supports (hex, tri), a prominent Global/Regional
//! choice, mesh-type-filtered refinement criteria, and the import/smoothing
//! plumbing tucked under Advanced. The verbatim namelist mirror is gone.

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;

mod i18n;
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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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
    ("f.mesh_type", Tab::Basics), ("f.mode_grid", Tab::Basics), ("f.nxp", Tab::Basics),
    ("f.output_format", Tab::Basics), ("f.domain_mode", Tab::Basics), ("f.domain_shape", Tab::Basics),
    ("f.domain_prefix", Tab::Basics), ("f.refine_master", Tab::Basics), ("f.threads", Tab::Basics),
    ("f.expnme", Tab::Basics), ("f.base_dir", Tab::Basics),
    ("f.refine_spc", Tab::Refinement), ("f.max_iter_spc", Tab::Refinement),
    ("f.spc_shape", Tab::Refinement), ("f.spc_prefix", Tab::Refinement),
    ("f.refine_cal", Tab::Refinement), ("f.max_iter_cal", Tab::Refinement),
    ("f.cal_shape", Tab::Refinement), ("f.cal_prefix", Tab::Refinement),
    ("f.threshold_dir", Tab::Refinement), ("f.landtype_file", Tab::Refinement),
    ("f.weak_concav", Tab::Refinement), ("f.is_transition", Tab::Refinement),
    ("f.iter_d", Tab::Refinement), ("f.halo", Tab::Refinement), ("f.max_transition", Tab::Refinement),
    ("c.num_landtypes", Tab::Refinement), ("c.area_mainland", Tab::Refinement),
    ("c.lai_m", Tab::Refinement), ("c.lai_s", Tab::Refinement), ("c.slope_m", Tab::Refinement),
    ("c.slope_s", Tab::Refinement), ("c.ks_m", Tab::Refinement), ("c.ks_s", Tab::Refinement),
    ("c.ksol_m", Tab::Refinement), ("c.ksol_s", Tab::Refinement), ("c.tkdry_m", Tab::Refinement),
    ("c.tkdry_s", Tab::Refinement), ("c.tksatf_m", Tab::Refinement), ("c.tksatf_s", Tab::Refinement),
    ("c.tksatu_m", Tab::Refinement), ("c.tksatu_s", Tab::Refinement), ("c.sea_ratio", Tab::Refinement),
    ("c.sst_m", Tab::Refinement), ("c.sst_s", Tab::Refinement), ("c.ssh_m", Tab::Refinement),
    ("c.ssh_s", Tab::Refinement), ("c.eke_m", Tab::Refinement), ("c.eke_s", Tab::Refinement),
    ("c.seaslope_m", Tab::Refinement), ("c.seaslope_s", Tab::Refinement),
    ("c.typhoon_m", Tab::Refinement), ("c.typhoon_s", Tab::Refinement),
    ("f.mode_file", Tab::Advanced), ("f.mode_file_desc", Tab::Advanced), ("f.gridnum", Tab::Advanced),
    ("f.niter", Tab::Advanced), ("f.beta", Tab::Advanced), ("f.relax", Tab::Advanced),
    ("f.niter_refine", Tab::Advanced), ("f.spring_global", Tab::Advanced), ("f.num_rc", Tab::Advanced),
    ("f.set_dis", Tab::Advanced), ("f.spring_regional", Tab::Advanced), ("f.vertex_layers", Tab::Advanced),
    ("f.patch_on", Tab::Advanced), ("f.patch_shape", Tab::Advanced), ("f.patch_prefix", Tab::Advanced),
    ("f.mask_restart", Tab::Advanced), ("f.sea_ratio", Tab::Advanced), ("f.isolated_ocean", Tab::Advanced),
];

fn collect_nml(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_nml(&p, out);
            } else if p.extension().map_or(false, |x| x == "nml") {
                out.push(p);
            }
        }
    }
}

fn bundled_templates() -> Vec<(String, PathBuf)> {
    let examples = workspace_root().join("examples");
    let mut nmls = Vec::new();
    collect_nml(&examples, &mut nmls);
    nmls.sort();
    nmls.into_iter()
        .map(|p| {
            let label = p.strip_prefix(&examples).unwrap_or(&p).to_string_lossy().to_string();
            (label, p)
        })
        .collect()
}

enum RunMsg {
    Done(Result<String, String>),
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
    dom_bbox: [f64; 4],   // west, east, north, south (degrees)
    dom_circle: [f64; 3], // center lon, center lat, radius (km)
    results_detached: bool,
    output_files: Vec<PathBuf>,
    mesh_view: Option<earthmesh_cli::GridfileMeshPoints>,
    tiles: Option<walkers::PmTiles>, // offline Protomaps basemap; None → wireframe fallback
    map_memory: walkers::MapMemory,
    frame_pending: bool, // re-frame the map on the next render, once the widget size is known
    gen_mpas: bool,      // atmosmesh: also write a standard MPAS mesh NetCDF after the run
}

impl Default for EarthMeshApp {
    fn default() -> Self {
        Self {
            mkgrd: EarthmeshConfig::default(),
            refine: RefineConfig::default(),
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
            results_detached: false,
            output_files: Vec::new(),
            mesh_view: None,
            tiles: None,
            map_memory: walkers::MapMemory::default(),
            frame_pending: false,
            gen_mpas: false,
        }
    }
}

/// Draw the mesh cell-centres on a simple equirectangular lon/lat map.
fn draw_mesh_2d(ui: &mut egui::Ui, mesh: &earthmesh_cli::GridfileMeshPoints) {
    let height = (ui.available_height() - 6.0).max(180.0);
    let (rect, _resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
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
#[derive(Clone, Copy)]
enum DomainMask {
    Bbox { west: f64, east: f64, north: f64, south: f64 },
    Circle { lon: f64, lat: f64, radius_km: f64 },
}

impl DomainMask {
    /// Whether a cell centre (degrees) falls inside the domain.
    fn contains(&self, lon: f64, lat: f64) -> bool {
        let norm = |x: f64| ((x + 180.0).rem_euclid(360.0)) - 180.0;
        match *self {
            DomainMask::Bbox { west, east, north, south } => {
                let (s, n) = (south.min(north), south.max(north));
                let (w, e) = (norm(west), norm(east));
                let lon = norm(lon);
                lat >= s && lat <= n && lon >= w.min(e) && lon <= w.max(e)
            }
            DomainMask::Circle { lon: clon, lat: clat, radius_km } => {
                let r_earth = 6371.0_f64;
                let (la1, la2) = (clat.to_radians(), lat.to_radians());
                let dlat = (lat - clat).to_radians();
                let dlon = (norm(lon) - norm(clon)).to_radians();
                let a = (dlat / 2.0).sin().powi(2)
                    + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
                2.0 * r_earth * a.sqrt().asin() <= radius_km
            }
        }
    }
}

/// Draws the mesh triangle wireframe on top of a walkers slippy map, projecting
/// each W-vertex through the map's `Projector` so it tracks pan/zoom. When a
/// `domain` is set, only cells whose centre is inside it are drawn (the rest are
/// masked out), so a regional run shows just its region of interest.
struct MeshOverlay<'m> {
    mesh: &'m earthmesh_cli::GridfileMeshPoints,
    domain: Option<DomainMask>,
    /// Draw the hexagonal primal cells (W cells) rather than the triangular dual
    /// (M cells). For an MPAS hex run the cells are hexagons, so this is what the
    /// user expects to see; a tri run draws triangles.
    hex: bool,
}

/// Web-Mercator's usable latitude bound; tiles cover only ±this.
const MERCATOR_MAX_LAT: f64 = 85.05112878;

impl walkers::Plugin for MeshOverlay<'_> {
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        response: &egui::Response,
        projector: &walkers::Projector,
        _map_memory: &walkers::MapMemory,
    ) {
        let MeshOverlay { mesh, domain, hex } = *self;
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
        let map_rect = egui::Rect::from_min_max(egui::pos2(x_west, y_top), egui::pos2(x_east, y_bot))
            .intersect(response.rect);
        let painter = ui.painter().with_clip_rect(map_rect);

        // Semi-transparent so the basemap underneath stays readable.
        let stroke = egui::Stroke::new(0.6, egui::Color32::from_rgba_unmultiplied(255, 120, 0, 150));
        let norm_lon = |lon: f64| ((lon + 180.0).rem_euclid(360.0)) - 180.0;
        // Bring `x` within ±180° of the reference so a cell reads as one contiguous
        // shape instead of wrapping the long way around the globe.
        let unwrap = |x: f64, reference: f64| -> f64 {
            if x - reference > 180.0 {
                x - 360.0
            } else if x - reference < -180.0 {
                x + 360.0
            } else {
                x
            }
        };
        // Draw a closed cell outline from its (normalized lon, lat) corners,
        // unwrapped around `ref_lon`. Pole-cap cells (wide even unwrapped) are
        // dropped; seam-straddling cells get a ±360° copy so the join fills.
        let draw_cell = |corners: &[(f64, f64)], ref_lon: f64| {
            if corners.len() < 3 {
                return;
            }
            let u: Vec<(f64, f64)> = corners.iter().map(|&(lo, la)| (unwrap(lo, ref_lon), la)).collect();
            let umin = u.iter().map(|v| v.0).fold(f64::MAX, f64::min);
            let umax = u.iter().map(|v| v.0).fold(f64::MIN, f64::max);
            if umax - umin > 45.0 {
                return;
            }
            let draw_at = |offset: f64| {
                for i in 0..u.len() {
                    let j = (i + 1) % u.len();
                    painter.line_segment([p(u[i].0 + offset, u[i].1), p(u[j].0 + offset, u[j].1)], stroke);
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
            // M-points (itab_w%im), sorted by bearing around the cell centre.
            let width = mesh.w_to_m_width;
            for wi in 0..wn {
                if let Some(dom) = &domain {
                    if !dom.contains(mesh.w_lon[wi], mesh.w_lat[wi]) {
                        continue;
                    }
                }
                let clon = norm_lon(mesh.w_lon[wi]);
                let clat = mesh.w_lat[wi];
                let nn = (mesh.n_w.get(wi).copied().unwrap_or(0).max(0) as usize).min(width);
                let mut corners: Vec<(f64, f64)> = Vec::with_capacity(nn);
                for k in 0..nn {
                    let id = mesh.w_to_m[wi * width + k];
                    let mi = id as usize;
                    if id >= 1 && mi <= mn {
                        corners.push((norm_lon(mesh.m_lon[mi - 1]), mesh.m_lat[mi - 1]));
                    }
                }
                corners.sort_by(|a, b| {
                    let ba = (a.1 - clat).atan2(unwrap(a.0, clon) - clon);
                    let bb = (b.1 - clat).atan2(unwrap(b.0, clon) - clon);
                    ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
                });
                draw_cell(&corners, clon);
            }
        } else {
            // Triangular cells: each M cell → its 3 W vertices.
            let vert = |idx1: i32| -> Option<(f64, f64)> {
                let i = idx1 as usize;
                (idx1 >= 1 && i <= wn).then(|| (norm_lon(mesh.w_lon[i - 1]), mesh.w_lat[i - 1]))
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
                draw_cell(&[a, b, c], a.0);
            }
        }
    }
}

fn collect_outputs(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().map_or(false, |x| x == "nc4" || x == "nc") {
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

/// Find the run's gridfile and write a standard MPAS mesh (+ graph.info) under
/// `<out>/mpas/`, entirely in Rust. Returns the MPAS path or an error string.
fn write_standard_mpas_for_output(out_dir: &str, nxp: usize, grid: &str) -> Result<String, String> {
    let base = Path::new(out_dir);
    let mut gridfile = None;
    if let Ok(rd) = std::fs::read_dir(base.join("gridfile")) {
        for entry in rd.flatten() {
            let p = entry.path();
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with("gridfile_NXP") && name.ends_with(".nc4") {
                let matches_grid = name.contains(grid);
                gridfile = Some(p);
                if matches_grid {
                    break;
                }
            }
        }
    }
    let gridfile = gridfile.ok_or_else(|| "MPAS: gridfile not found in output".to_string())?;
    let mpas_dir = base.join("mpas");
    std::fs::create_dir_all(&mpas_dir).map_err(|e| e.to_string())?;
    let stem = format!("MPASOUT_NXP{nxp:04}_{grid}");
    let mesh_out = mpas_dir.join(format!("{stem}.nc4"));
    let graph_out = mpas_dir.join(format!("{stem}.graph.info"));
    earthmesh_cli::write_standard_mpas_from_gridfile(&gridfile, &mesh_out, &graph_out, nxp)
        .map_err(|e| format!("MPAS write failed: {e}"))?;
    Ok(mesh_out.display().to_string())
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
                self.refine =
                    RefineConfig::from_mkrefine_namelist(&contents, &cfg.mesh_type, &cfg.mode_grid)
                        .unwrap_or_default();
                self.mkgrd = cfg;
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
            self.mkgrd.to_mkgrd_namelist(),
            self.refine.to_mkrefine_namelist()
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
        workspace_root()
            .join(self.mkgrd.base_dir.trim_start_matches("./").trim_end_matches('/'))
            .join(self.mkgrd.experiment_name.trim())
    }

    fn open_output_dir(&mut self) {
        let dir = self.output_dir();
        let target = if dir.exists() { dir } else { workspace_root() };
        if let Err(err) = open::that(&target) {
            self.set_status("status.write_error", err.to_string());
        }
    }

    /// Author the regional boundary NetCDF from the entered geometry, returning
    /// its path. Returns Ok(None) for shapes the GUI doesn't yet author (close /
    /// lambert), which keep using the user-set boundary file prefix.
    fn generate_domain_file(&self) -> Result<Option<PathBuf>, String> {
        let dir = std::env::temp_dir();
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
            _ => Ok(None),
        }
    }

    fn start_run(&mut self, ctx: &egui::Context) {
        if self.running {
            return;
        }
        // Regional: author the boundary file from the entered geometry first so
        // the namelist points at it.
        if !self.mkgrd.mask_domain_global {
            match self.generate_domain_file() {
                Ok(Some(path)) => {
                    let shown = path.display().to_string();
                    self.mkgrd.mask_domain_fprefix = shown.clone();
                    self.push_log("dom.generated", &shown);
                }
                Ok(None) => {}
                Err(err) => return self.set_status("status.stage_error", err),
            }
        }
        let nml_path = std::env::temp_dir().join("earthmesh_gui_run.nml");
        if let Err(err) = std::fs::write(&nml_path, self.combined_namelist()) {
            return self.set_status("status.stage_error", err.to_string());
        }
        let workdir = workspace_root();
        let (tx, rx) = mpsc::channel();
        let (ptx, prx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_worker = cancel.clone();
        let ctx = ctx.clone();
        let out_hint = self.output_dir().display().to_string();
        let gen_mpas = self.gen_mpas && self.mkgrd.mesh_type == "atmosmesh";
        let nxp = self.mkgrd.nxp.max(0) as usize;
        let grid = self.mkgrd.mode_grid.clone();
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
                Ok(_) if gen_mpas => {
                    // Pure-Rust standard MPAS write — can be slow on big meshes,
                    // so it runs here in the worker, not on the UI thread.
                    let _ = ptx_mpas.send(("mpas".to_string(), 0, 1));
                    match write_standard_mpas_for_output(&out_hint, nxp, &grid) {
                        Ok(_) => Ok(out_hint),
                        Err(err) => Err(err),
                    }
                }
                Ok(_) => Ok(out_hint),
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
            let cancelled = self.cancel_flag.as_ref().map_or(false, |f| f.load(Ordering::Relaxed));
            if cancelled {
                self.set_status("status.cancelled", String::new());
                self.push_log("status.cancelled", "");
            } else {
                match result {
                    Ok(out) => {
                        self.set_status("status.run_done", out.clone());
                        self.push_log("log.run_done", &out);
                        self.output_files = collect_outputs(Path::new(&out));
                        self.mesh_view = self
                            .output_files
                            .iter()
                            .find(|p| p.to_string_lossy().contains("gridfile"))
                            .or_else(|| self.output_files.first())
                            .and_then(|p| earthmesh_cli::read_gridfile_mesh_points(p).ok());
                        self.frame_pending = true;
                    }
                    Err(err) => {
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
    let current = options.iter().find(|(v, _)| *v == *value).map(|(_, t)| *t).unwrap_or("?");
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
            mapped_combo_row(ui, tr(lang, "f.mesh_type"), &mut self.mkgrd.mesh_type, MESH_TYPES, lang);
            ui.label("");
            ui.weak(tr(lang, "mesh.custom_note"));
            ui.end_row();

            let allowed = output_formats_for(&self.mkgrd.mesh_type);
            if !allowed.contains(&self.mkgrd.output_format.as_str()) {
                self.mkgrd.output_format = allowed[0].to_string();
            }
            combo_row(ui, tr(lang, "f.output_format"), &mut self.mkgrd.output_format, allowed);

            // Standard MPAS output is produced in pure Rust from the gridfile.
            if self.mkgrd.mesh_type == "atmosmesh" {
                check_row(ui, tr(lang, "f.gen_mpas"), &mut self.gen_mpas);
            }

            let grids = grid_modes_for(&self.mkgrd.mesh_type);
            mapped_combo_row(ui, tr(lang, "f.mode_grid"), &mut self.mkgrd.mode_grid, grids, lang);

            int_row(ui, tr(lang, "f.nxp"), &mut self.mkgrd.nxp, 1..=100_000);

            // Domain: global vs regional, with conditional boundary options.
            ui.label(tr(lang, "f.domain_mode"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mkgrd.mask_domain_global, true, tr(lang, "opt.global"));
                ui.selectable_value(&mut self.mkgrd.mask_domain_global, false, tr(lang, "opt.regional"));
            });
            ui.end_row();
            if !self.mkgrd.mask_domain_global {
                combo_row(ui, tr(lang, "f.domain_shape"), &mut self.mkgrd.mask_domain_type, REGION_TYPES);
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
                    _ => {
                        ui.label("");
                        ui.weak(tr(lang, "dom.poly_note"));
                        ui.end_row();
                        text_row(ui, tr(lang, "f.domain_prefix"), &mut self.mkgrd.mask_domain_fprefix);
                    }
                }
            }

            check_row(ui, tr(lang, "f.refine_master"), &mut self.mkgrd.refine);
            int_row(ui, tr(lang, "f.threads"), &mut self.mkgrd.openmp, 1..=1024);
        });
    }

    fn tab_refinement(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.refinement"));
        ui.separator();
        if !self.mkgrd.refine {
            ui.weak(tr(lang, "note.refine_off"));
        }
        let mt = self.mkgrd.mesh_type.clone();
        let show_land = mt == "landmesh" || mt == "earthmesh" || mt == "LOCmesh";
        let show_ocean = mt == "oceanmesh" || mt == "earthmesh" || mt == "LOCmesh";
        let show_atmos = mt == "atmosmesh" || mt == "earthmesh" || mt == "LOCmesh";
        let atmos_only = mt == "atmosmesh";

        ui.add_enabled_ui(self.mkgrd.refine, |ui| {
            egui::Grid::new("ref_ctrl").num_columns(2).show(ui, |ui| {
                check_row(ui, tr(lang, "f.refine_spc"), &mut self.refine.refine_spc);
                let spc = self.refine.refine_spc;
                ui.label(tr(lang, "f.max_iter_spc"));
                ui.add_enabled(spc, egui::DragValue::new(&mut self.refine.max_iter_spc).range(0..=100));
                ui.end_row();
                ui.label(tr(lang, "f.spc_shape"));
                ui.add_enabled_ui(spc, |ui| {
                    egui::ComboBox::from_id_salt("spc_type")
                        .selected_text(self.refine.mask_refine_spc_type.clone())
                        .show_ui(ui, |ui| {
                            for opt in REGION_TYPES {
                                ui.selectable_value(&mut self.refine.mask_refine_spc_type, (*opt).to_string(), *opt);
                            }
                        });
                });
                ui.end_row();
                ui.label(tr(lang, "f.spc_prefix"));
                ui.add_enabled(spc, egui::TextEdit::singleline(&mut self.refine.mask_refine_spc_fprefix).desired_width(280.0));
                ui.end_row();
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_enabled(!atmos_only, egui::Checkbox::new(&mut self.refine.refine_cal, tr(lang, "f.refine_cal")));
                if atmos_only {
                    ui.weak(tr(lang, "note.cal_atmos"));
                }
            });
            let cal = self.refine.refine_cal && !atmos_only;
            ui.add_enabled_ui(cal, |ui| {
                egui::Grid::new("ref_cal").num_columns(2).show(ui, |ui| {
                    int_row(ui, tr(lang, "f.max_iter_cal"), &mut self.refine.max_iter_cal, 0..=100);
                    combo_row(ui, tr(lang, "f.cal_shape"), &mut self.refine.mask_refine_cal_type, REGION_TYPES);
                    text_row(ui, tr(lang, "f.cal_prefix"), &mut self.refine.mask_refine_cal_fprefix);
                    text_row(ui, tr(lang, "f.threshold_dir"), &mut self.refine.threshold_dir);
                    text_row(ui, tr(lang, "f.landtype_file"), &mut self.mkgrd.landtype_file);
                });
            });

            ui.add_space(6.0);
            if show_land {
                egui::CollapsingHeader::new(tr(lang, "sec.land_crit")).default_open(true).show(ui, |ui| {
                    egui::Grid::new("lnd1").num_columns(2).show(ui, |ui| {
                        ui.checkbox(&mut self.refine.refine_num_landtypes, tr(lang, "c.num_landtypes"));
                        ui.add_enabled(self.refine.refine_num_landtypes, egui::DragValue::new(&mut self.refine.th_num_landtypes).range(0..=1000));
                        ui.end_row();
                        crit_row(ui, tr(lang, "c.area_mainland"), &mut self.refine.refine_area_mainland, &mut self.refine.th_area_mainland);
                        crit_row(ui, tr(lang, "c.lai_m"), &mut self.refine.refine_onelayer_lnd[0], &mut self.refine.th_onelayer_lnd[0]);
                        crit_row(ui, tr(lang, "c.lai_s"), &mut self.refine.refine_onelayer_lnd[1], &mut self.refine.th_onelayer_lnd[1]);
                        crit_row(ui, tr(lang, "c.slope_m"), &mut self.refine.refine_onelayer_lnd[2], &mut self.refine.th_onelayer_lnd[2]);
                        crit_row(ui, tr(lang, "c.slope_s"), &mut self.refine.refine_onelayer_lnd[3], &mut self.refine.th_onelayer_lnd[3]);
                    });
                    let keys = ["c.ks_m","c.ks_s","c.ksol_m","c.ksol_s","c.tkdry_m","c.tkdry_s","c.tksatf_m","c.tksatf_s","c.tksatu_m","c.tksatu_s"];
                    egui::Grid::new("lnd2").num_columns(2).show(ui, |ui| {
                        for i in 0..10 {
                            crit_pair_row(ui, tr(lang, keys[i]), &mut self.refine.refine_twolayer_lnd[i], &mut self.refine.th_twolayer_lnd[i]);
                        }
                    });
                });
            }
            if show_ocean {
                egui::CollapsingHeader::new(tr(lang, "sec.ocean_crit")).show(ui, |ui| {
                    egui::Grid::new("ocn").num_columns(2).show(ui, |ui| {
                        crit_pair_row(ui, tr(lang, "c.sea_ratio"), &mut self.refine.refine_sea_ratio, &mut self.refine.th_sea_ratio);
                        let keys = ["c.sst_m","c.sst_s","c.ssh_m","c.ssh_s","c.eke_m","c.eke_s","c.seaslope_m","c.seaslope_s"];
                        for i in 0..8 {
                            crit_row(ui, tr(lang, keys[i]), &mut self.refine.refine_onelayer_ocn[i], &mut self.refine.th_onelayer_ocn[i]);
                        }
                    });
                });
            }
            if show_atmos {
                egui::CollapsingHeader::new(tr(lang, "sec.atmos_crit")).show(ui, |ui| {
                    egui::Grid::new("atm").num_columns(2).show(ui, |ui| {
                        crit_row(ui, tr(lang, "c.typhoon_m"), &mut self.refine.refine_onelayer_atmos[0], &mut self.refine.th_onelayer_atmos[0]);
                        crit_row(ui, tr(lang, "c.typhoon_s"), &mut self.refine.refine_onelayer_atmos[1], &mut self.refine.th_onelayer_atmos[1]);
                    });
                });
            }

            egui::CollapsingHeader::new(tr(lang, "sec.adv_refine")).show(ui, |ui| {
                egui::Grid::new("adv_ref").num_columns(2).show(ui, |ui| {
                    check_row(ui, tr(lang, "f.weak_concav"), &mut self.refine.weak_concav_eliminate);
                    check_row(ui, tr(lang, "f.is_transition"), &mut self.refine.is_transition);
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
                            ui.add(egui::DragValue::new(&mut self.refine.max_transition_row[i]).speed(1.0));
                        }
                    });
                    ui.end_row();
                });
            });
        });
    }

    fn tab_advanced(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.advanced"));
        ui.separator();
        egui::CollapsingHeader::new(tr(lang, "sec.import")).show(ui, |ui| {
            egui::Grid::new("import").num_columns(2).show(ui, |ui| {
                text_row(ui, tr(lang, "f.mode_file"), &mut self.mkgrd.mode_file);
                combo_row(ui, tr(lang, "f.mode_file_desc"), &mut self.mkgrd.mode_file_description, MODE_FILE_DESCS);
            });
        });
        egui::CollapsingHeader::new(tr(lang, "sec.smoothing")).show(ui, |ui| {
            egui::Grid::new("smooth").num_columns(2).show(ui, |ui| {
                int_row(ui, tr(lang, "f.niter"), &mut self.mkgrd.niter, 0..=1_000_000);
                f32_row(ui, tr(lang, "f.beta"), &mut self.mkgrd.beta);
                f32_row(ui, tr(lang, "f.relax"), &mut self.mkgrd.relax);
                int_row(ui, tr(lang, "f.niter_refine"), &mut self.refine.niter_refine, 0..=1_000_000);
                int_combo_row(ui, tr(lang, "f.spring_global"), &mut self.refine.spring_global_type, &[(0, tr(lang, "opt.spring_none")), (1, tr(lang, "opt.spring_olam"))]);
                int_row(ui, tr(lang, "f.num_rc"), &mut self.refine.num_rc, 0..=1000);
                combo_row(ui, tr(lang, "f.set_dis"), &mut self.refine.set_dis_type, SET_DIS_TYPES);
                int_combo_row(ui, tr(lang, "f.spring_regional"), &mut self.refine.spring_regional_type, &[(0, tr(lang, "opt.spring_none")), (1, tr(lang, "opt.reg_each")), (2, tr(lang, "opt.reg_final"))]);
                int_row(ui, tr(lang, "f.vertex_layers"), &mut self.refine.vertex_pretect_layers, 0..=1000);
            });
        });
        egui::CollapsingHeader::new(tr(lang, "head.mask")).show(ui, |ui| {
            egui::Grid::new("adv_mask").num_columns(2).show(ui, |ui| {
                int_combo_row(ui, tr(lang, "f.gridnum"), &mut self.mkgrd.gridnum_perdegree, &[(120, "120"), (240, "240")]);
                f64_row(ui, tr(lang, "f.sea_ratio"), &mut self.mkgrd.mask_sea_ratio);
                check_row(ui, tr(lang, "f.mask_restart"), &mut self.mkgrd.mask_restart);
                check_row(ui, tr(lang, "f.isolated_ocean"), &mut self.mkgrd.isolated_ocean);
                check_row(ui, tr(lang, "f.patch_on"), &mut self.mkgrd.mask_patch_on);
                let patch = self.mkgrd.mask_patch_on;
                ui.label(tr(lang, "f.patch_shape"));
                ui.add_enabled_ui(patch, |ui| {
                    egui::ComboBox::from_id_salt("patch_type")
                        .selected_text(self.mkgrd.mask_patch_type.clone())
                        .show_ui(ui, |ui| {
                            for opt in REGION_TYPES {
                                ui.selectable_value(&mut self.mkgrd.mask_patch_type, (*opt).to_string(), *opt);
                            }
                        });
                });
                ui.end_row();
                ui.label(tr(lang, "f.patch_prefix"));
                ui.add_enabled(patch, egui::TextEdit::singleline(&mut self.mkgrd.mask_patch_fprefix).desired_width(280.0));
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
            if ui.button(format!("{}   ·   {}", tr(lang, k), tr(lang, tab_nav_key(tab)))).clicked() {
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
        if self.mesh_view.as_ref().map_or(true, |m| m.m_lon.is_empty()) {
            return;
        }
        // When a regional domain is set, frame to the user's bbox/circle (the mesh
        // is still global, but the view zooms onto the region of interest); for a
        // global domain — or a polygon domain that lives in a file — fit the mesh.
        let regional = !self.mkgrd.mask_domain_global;
        let (lon_min, lon_max, lat_min, lat_max) =
            if regional && self.mkgrd.mask_domain_type == "bbox" {
                let b = self.dom_bbox; // [west, east, north, south]
                (b[0], b[1], b[3], b[2])
            } else if regional && self.mkgrd.mask_domain_type == "circle" {
                let [clon, clat, r_km] = self.dom_circle;
                let r_deg = r_km / 111.32;
                let r_lon = r_deg / clat.to_radians().cos().abs().max(0.05);
                (clon - r_lon, clon + r_lon, clat - r_deg, clat + r_deg)
            } else {
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
            };
        // Tolerate reversed entries (e.g. west > east) and centre on the extent.
        let (lon_min, lon_max) = (lon_min.min(lon_max), lon_min.max(lon_max));
        let (lat_min, lat_max) = (lat_min.min(lat_max), lat_min.max(lat_max));
        let clon = 0.5 * (lon_min + lon_max);
        let clat = 0.5 * (lat_min + lat_max);
        // Pad a region so it isn't drawn edge-to-edge.
        let pad = if regional { 1.3 } else { 1.0 };
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
        egui::ScrollArea::vertical().max_height(72.0).id_salt("files_list").show(ui, |ui| {
            for f in &files {
                let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if ui.button(name).on_hover_text(f.display().to_string()).clicked() {
                    let _ = open::that(f);
                }
            }
        });
        ui.separator();
        if self.mesh_view.is_some() {
            ui.weak(tr(lang, "results.map_hint"));
            // Frame on the first render after a run, now that the map widget's
            // real size is known (the dock and the detached window differ).
            if self.frame_pending {
                let (w, h) = (ui.available_width(), ui.available_height());
                self.frame_mesh_view(w, h);
                self.frame_pending = false;
            }
            // Regional run → mask the drawn mesh to the bbox/circle (a polygon
            // domain lives in a file we can't read here, so it isn't masked).
            let domain = if !self.mkgrd.mask_domain_global {
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
                    _ => None,
                }
            } else {
                None
            };
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
                .with_plugin(MeshOverlay { mesh, domain, hex });
                ui.add(map);
                ui.weak("© OpenStreetMap contributors · Protomaps");
            } else {
                // No bundled basemap → equirectangular wireframe fallback.
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
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert("cjk".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                fam.push("cjk".to_owned());
            }
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                fam.push("cjk".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
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

impl eframe::App for EarthMeshApp {
    // eframe 0.34 requires `ui`; we keep the multi-panel layout in `update`
    // (still invoked by the run loop) and leave `ui` empty.
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    #[allow(deprecated)]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_run();
        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        let lang = self.lang;

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.heading(tr(lang, "app.title"));
                ui.separator();
                ui.add_enabled_ui(!self.running, |ui| {
                    if ui.button(tr(lang, "btn.run")).clicked() {
                        self.start_run(ctx);
                    }
                });
                if ui.add_enabled(self.running, egui::Button::new(tr(lang, "btn.cancel"))).clicked() {
                    self.request_cancel();
                }
                if self.running {
                    ui.add(egui::Spinner::new());
                }
                ui.separator();
                if ui.button(tr(lang, "btn.load")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("namelist", &["nml"]).set_directory(workspace_root()).pick_file() {
                        self.load(path);
                    }
                }
                if ui.button(tr(lang, "btn.save")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("namelist", &["nml"]).set_file_name("earthmesh.nml").set_directory(workspace_root()).save_file() {
                        self.save(path);
                    }
                }
                if ui.button(tr(lang, "btn.open_output")).clicked() {
                    self.open_output_dir();
                }
                ui.separator();
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(tr(lang, "search.placeholder")).desired_width(170.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut self.lang, Lang::Zh, "中文");
                    ui.selectable_value(&mut self.lang, Lang::En, "EN");
                    ui.label(tr(lang, "lang.label"));
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
                if self.results_detached {
                    ui.weak(tr(lang, "results.dock"));
                } else {
                    self.results_ui(ui);
                }
            });

        egui::SidePanel::left("cases").resizable(true).default_width(190.0).show(ctx, |ui| {
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
                        let label = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        if ui.button(label).on_hover_text(path.display().to_string()).clicked() {
                            to_load = Some(path);
                        }
                    }
                }
            });
            if let Some(p) = to_load {
                self.load(p);
            }
        });

        egui::SidePanel::right("run").resizable(true).default_width(300.0).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(tr(lang, "run.title"));
            ui.separator();
            ui.label(tr(lang, if self.running { "run.running" } else { "run.idle" }));
            let status = if self.status_detail.is_empty() {
                tr(lang, self.status_key).to_string()
            } else {
                format!("{} {}", tr(lang, self.status_key), self.status_detail)
            };
            ui.label(status);
            if let Some((phase, done, total)) = &self.progress {
                let frac = if *total > 0 { *done as f32 / *total as f32 } else { 0.0 };
                ui.add(egui::ProgressBar::new(frac).text(format!("{phase} {done}/{total}")));
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new(tr(lang, "run.log")).strong());
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
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
            app.load(workspace_root().join("examples/default/atmosphere_hex_global.nml"));
            Ok(Box::new(app))
        }),
    )
}
