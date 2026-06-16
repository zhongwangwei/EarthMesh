//! EarthMesh desktop GUI (scaffold).
//!
//! Increment 2: a three-pane eframe shell that loads/saves the configuration
//! through `earthmesh_core` and runs the mesh engine on a background thread via
//! the existing path-based `earthmesh_cli` entry. The UI streams run status back
//! through a channel and never blocks. Per-iteration progress and cancellation
//! arrive once the engine progress seam (Plan 03) lands; the full 92-option form,
//! validation, i18n, and visualization come in later increments.

use earthmesh_core::EarthmeshConfig;
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

const MESH_TYPES: &[&str] = &["landmesh", "oceanmesh", "atmosmesh", "LOCmesh"];
const MODE_GRIDS: &[&str] = &["lonlat", "lambert", "cubical", "tri", "hex", "dbx"];
const OUTPUT_FORMATS: &[&str] = &["CoLM", "FVCOM", "MPAS", "MPAS-Simple"];

fn workspace_root() -> PathBuf {
    // crate dir is <root>/rust/earthmesh_gui
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Extract the raw `&mkrefine ... /` block from a loaded namelist so it can be
/// preserved verbatim when the GUI rewrites the `&mkgrd` block for a run. The GUI
/// only edits `&mkgrd` fields for now, so `&mkrefine` is carried through unchanged.
fn extract_mkrefine_block(raw: &str) -> Option<String> {
    let mut block: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if !inside {
            if trimmed.eq_ignore_ascii_case("&mkrefine")
                || trimmed.to_ascii_lowercase().starts_with("&mkrefine")
            {
                inside = true;
                block.push(line);
            }
        } else {
            block.push(line);
            if trimmed == "/" {
                break;
            }
        }
    }
    if inside {
        Some(block.join("\n"))
    } else {
        None
    }
}

/// Message sent from the background run thread back to the UI thread.
enum RunMsg {
    Done(Result<String, String>),
}

struct EarthMeshApp {
    config: EarthmeshConfig,
    raw_mkrefine: Option<String>,
    loaded_path: Option<PathBuf>,
    status: String,
    running: bool,
    run_rx: Option<Receiver<RunMsg>>,
}

impl Default for EarthMeshApp {
    fn default() -> Self {
        Self {
            config: EarthmeshConfig::default(),
            raw_mkrefine: None,
            loaded_path: None,
            status: "Ready. Load an example namelist to begin.".to_string(),
            running: false,
            run_rx: None,
        }
    }
}

impl EarthMeshApp {
    fn load(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(contents) => match EarthmeshConfig::from_mkgrd_namelist(&contents) {
                Ok(config) => {
                    self.config = config;
                    self.raw_mkrefine = extract_mkrefine_block(&contents);
                    self.status = format!("Loaded {}", path.display());
                    self.loaded_path = Some(path);
                }
                Err(err) => self.status = format!("Parse error: {err}"),
            },
            Err(err) => self.status = format!("Read error: {err}"),
        }
    }

    /// The full namelist text to run: the edited `&mkgrd` block plus the
    /// preserved `&mkrefine` block (if one was loaded).
    fn combined_namelist(&self) -> String {
        let mut text = self.config.to_mkgrd_namelist();
        if let Some(refine) = &self.raw_mkrefine {
            text.push('\n');
            text.push_str(refine);
            text.push('\n');
        }
        text
    }

    fn save(&mut self, path: PathBuf) {
        match std::fs::write(&path, self.combined_namelist()) {
            Ok(()) => self.status = format!("Saved {}", path.display()),
            Err(err) => self.status = format!("Write error: {err}"),
        }
    }

    fn start_run(&mut self, ctx: &egui::Context) {
        if self.running {
            return;
        }
        let workdir = workspace_root();
        let nml_path = std::env::temp_dir().join("earthmesh_gui_run.nml");
        if let Err(err) = std::fs::write(&nml_path, self.combined_namelist()) {
            self.status = format!("Could not stage namelist: {err}");
            return;
        }

        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let out_hint = format!("{}{}/", self.config.base_dir.trim_end(), self.config.experiment_name.trim());
        thread::spawn(move || {
            let result = earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
                &nml_path, &workdir, 100_000, 0, None, None, None, 1, None,
            );
            let msg = match result {
                Ok(_) => Ok(format!(
                    "Run finished. Output under {}",
                    workdir.join(out_hint.trim_start_matches("./")).display()
                )),
                Err(err) => Err(format!("Run failed: {err}")),
            };
            let _ = tx.send(RunMsg::Done(msg));
            ctx.request_repaint();
        });

        self.run_rx = Some(rx);
        self.running = true;
        self.status = "Running… (engine on a background thread; UI stays responsive)".to_string();
    }

    fn poll_run(&mut self) {
        if let Some(rx) = &self.run_rx {
            if let Ok(RunMsg::Done(result)) = rx.try_recv() {
                self.status = match result {
                    Ok(message) => message,
                    Err(message) => message,
                };
                self.running = false;
                self.run_rx = None;
            }
        }
    }

    fn combo(ui: &mut egui::Ui, label: &str, value: &mut String, options: &[&str]) {
        ui.horizontal(|ui| {
            ui.label(label);
            egui::ComboBox::from_id_salt(label)
                .selected_text(value.clone())
                .show_ui(ui, |ui| {
                    for option in options {
                        ui.selectable_value(value, (*option).to_string(), *option);
                    }
                });
        });
    }
}

impl eframe::App for EarthMeshApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_run();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("EarthMesh");
                ui.separator();
                if ui.button("📂 Load example").clicked() {
                    let path = workspace_root().join("examples/default/atmosphere_hex_global.nml");
                    self.load(path);
                }
                if ui.button("💾 Save as…").clicked() {
                    let path = self
                        .loaded_path
                        .clone()
                        .map(|p| p.with_extension("saved.nml"))
                        .unwrap_or_else(|| workspace_root().join("earthmesh_gui_out.nml"));
                    self.save(path);
                }
                ui.separator();
                ui.add_enabled_ui(!self.running, |ui| {
                    if ui.button("▶ Run").clicked() {
                        self.start_run(ctx);
                    }
                });
                if self.running {
                    ui.add(egui::Spinner::new());
                }
            });
        });

        egui::SidePanel::left("cases")
            .resizable(true)
            .default_width(160.0)
            .show(ctx, |ui| {
                ui.heading("Cases");
                ui.separator();
                ui.weak("(case/template list — later)");
            });

        egui::SidePanel::right("run")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Run");
                ui.separator();
                if self.running {
                    ui.label("Status: running");
                } else {
                    ui.label("Status: idle");
                }
                ui.separator();
                ui.label(&self.status);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Basics");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Experiment name");
                    ui.text_edit_singleline(&mut self.config.experiment_name);
                });
                ui.horizontal(|ui| {
                    ui.label("Base dir");
                    ui.text_edit_singleline(&mut self.config.base_dir);
                });
                Self::combo(ui, "Mesh type", &mut self.config.mesh_type, MESH_TYPES);
                Self::combo(ui, "Grid mode", &mut self.config.mode_grid, MODE_GRIDS);
                ui.horizontal(|ui| {
                    ui.label("NXP");
                    ui.add(egui::DragValue::new(&mut self.config.nxp).range(1..=100_000));
                });
                Self::combo(
                    ui,
                    "Output format",
                    &mut self.config.output_format,
                    OUTPUT_FORMATS,
                );
                ui.horizontal(|ui| {
                    ui.label("OpenMP threads");
                    ui.add(egui::DragValue::new(&mut self.config.openmp).range(1..=1024));
                });
                ui.checkbox(&mut self.config.refine, "Refine");
            });
        });
    }
}

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "EarthMesh",
        native_options,
        Box::new(|_cc| Ok(Box::new(EarthMeshApp::default()))),
    )
}
