//! EarthMesh desktop GUI (scaffold).
//!
//! Increment 1: a three-pane eframe shell that loads/saves the `&mkgrd`
//! configuration through `earthmesh_core` and exposes a handful of fields as
//! editable widgets. The full 92-option form, background run, and visualization
//! arrive in later increments.

use earthmesh_core::EarthmeshConfig;
use eframe::egui;
use std::path::PathBuf;

const MESH_TYPES: &[&str] = &["landmesh", "oceanmesh", "atmosmesh", "LOCmesh"];
const MODE_GRIDS: &[&str] = &["lonlat", "lambert", "cubical", "tri", "hex", "dbx"];
const OUTPUT_FORMATS: &[&str] = &["CoLM", "FVCOM", "MPAS", "MPAS-Simple"];

fn workspace_root() -> PathBuf {
    // crate dir is <root>/rust/earthmesh_gui
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct EarthMeshApp {
    config: EarthmeshConfig,
    loaded_path: Option<PathBuf>,
    status: String,
}

impl Default for EarthMeshApp {
    fn default() -> Self {
        Self {
            config: EarthmeshConfig::default(),
            loaded_path: None,
            status: "Ready. Load an example namelist to begin.".to_string(),
        }
    }
}

impl EarthMeshApp {
    fn load(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(contents) => match EarthmeshConfig::from_mkgrd_namelist(&contents) {
                Ok(config) => {
                    self.config = config;
                    self.status = format!("Loaded {}", path.display());
                    self.loaded_path = Some(path);
                }
                Err(err) => self.status = format!("Parse error: {err}"),
            },
            Err(err) => self.status = format!("Read error: {err}"),
        }
    }

    fn save(&mut self, path: PathBuf) {
        match std::fs::write(&path, self.config.to_mkgrd_namelist()) {
            Ok(()) => self.status = format!("Saved {}", path.display()),
            Err(err) => self.status = format!("Write error: {err}"),
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
                let _ = ui.button("▶ Run").on_hover_text("Background run lands in a later increment");
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
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Run");
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
