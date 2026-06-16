//! EarthMesh desktop GUI.
//!
//! Increment 3a: the full three-pane workbench — all 92 `&mkgrd`/`&mkrefine`
//! options across five tabs, laid out in aligned grids with section headers and
//! conditional enable/disable, plus the background mesh-engine run from
//! increment 2. Internationalisation (中文/English) lands in increment 3b.

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

const MESH_TYPES: &[&str] = &["landmesh", "oceanmesh", "atmosmesh", "LOCmesh"];
const MODE_GRIDS: &[&str] = &["lonlat", "lambert", "cubical", "tri", "hex", "dbx"];
const REGION_TYPES: &[&str] = &["bbox", "lambert", "close", "circle"];
const SET_DIS_TYPES: &[&str] = &["linear", "nonlinear1", "nonlinear2", "nonlinear3"];
const MODE_FILE_DESCS: &[&str] = &["none", "EarthMesh", "MPAS", "IAP-Ocean", "FVCOM"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Output formats valid for a given mesh type (mirrors `validate_like_read_nl`).
fn output_formats_for(mesh_type: &str) -> &'static [&'static str] {
    match mesh_type {
        "atmosmesh" => &["MPAS", "MPAS-Simple"],
        "oceanmesh" => &["FVCOM"],
        _ => &["CoLM"], // landmesh / earthmesh / LOCmesh
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Basics,
    Spring,
    Mask,
    RefineGeneral,
    RefineCriteria,
}

enum RunMsg {
    Done(Result<String, String>),
}

struct EarthMeshApp {
    mkgrd: EarthmeshConfig,
    refine: RefineConfig,
    loaded_path: Option<PathBuf>,
    tab: Tab,
    status: String,
    running: bool,
    run_rx: Option<Receiver<RunMsg>>,
}

impl Default for EarthMeshApp {
    fn default() -> Self {
        Self {
            mkgrd: EarthmeshConfig::default(),
            refine: RefineConfig::default(),
            loaded_path: None,
            tab: Tab::Basics,
            status: "Ready.".to_string(),
            running: false,
            run_rx: None,
        }
    }
}

impl EarthMeshApp {
    fn load(&mut self, path: PathBuf) {
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(err) => {
                self.status = format!("Read error: {err}");
                return;
            }
        };
        match EarthmeshConfig::from_mkgrd_namelist(&contents) {
            Ok(cfg) => {
                // Parse the refine block with the same mesh/grid; fall back to
                // defaults if this file has no valid &mkrefine.
                self.refine =
                    RefineConfig::from_mkrefine_namelist(&contents, &cfg.mesh_type, &cfg.mode_grid)
                        .unwrap_or_default();
                self.mkgrd = cfg;
                self.status = format!("Loaded {}", path.display());
                self.loaded_path = Some(path);
            }
            Err(err) => self.status = format!("Parse error: {err}"),
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
        let out_hint = format!(
            "{}{}/",
            self.mkgrd.base_dir.trim_end(),
            self.mkgrd.experiment_name.trim()
        );
        thread::spawn(move || {
            let result =
                earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
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
        self.status = "Running… (background thread; UI stays responsive)".to_string();
    }

    fn poll_run(&mut self) {
        if let Some(rx) = &self.run_rx {
            if let Ok(RunMsg::Done(result)) = rx.try_recv() {
                self.status = result.unwrap_or_else(|e| e);
                self.running = false;
                self.run_rx = None;
            }
        }
    }
}

// ---- small widget helpers (two-column grid rows) -------------------------------

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

/// A criterion switch + its single threshold on one grid row.
fn crit_row(ui: &mut egui::Ui, label: &str, on: &mut bool, thr: &mut f64) {
    ui.checkbox(on, label);
    ui.add_enabled(*on, egui::DragValue::new(thr).speed(0.1));
    ui.end_row();
}

/// A criterion switch + its `[f64; 2]` threshold pair on one grid row.
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
        ui.heading("Basics");
        ui.separator();
        egui::Grid::new("basics").num_columns(2).show(ui, |ui| {
            text_row(ui, "Experiment name", &mut self.mkgrd.experiment_name);
            text_row(ui, "Base dir", &mut self.mkgrd.base_dir);
            combo_row(ui, "Mesh type", &mut self.mkgrd.mesh_type, MESH_TYPES);

            // Keep output_format consistent with the chosen mesh type.
            let allowed = output_formats_for(&self.mkgrd.mesh_type);
            if !allowed.contains(&self.mkgrd.output_format.as_str()) {
                self.mkgrd.output_format = allowed[0].to_string();
            }
            combo_row(ui, "Output format", &mut self.mkgrd.output_format, allowed);

            combo_row(ui, "Grid mode", &mut self.mkgrd.mode_grid, MODE_GRIDS);
            int_row(ui, "NXP (resolution)", &mut self.mkgrd.nxp, 1..=100_000);
            int_combo_row(
                ui,
                "Grid pts / degree",
                &mut self.mkgrd.gridnum_perdegree,
                &[(120, "120"), (240, "240")],
            );
            int_row(ui, "OpenMP threads", &mut self.mkgrd.openmp, 1..=1024);
            text_row(ui, "Land-type file", &mut self.mkgrd.landtype_file);
            text_row(ui, "Initial mesh file", &mut self.mkgrd.mode_file);
            combo_row(
                ui,
                "Initial mesh format",
                &mut self.mkgrd.mode_file_description,
                MODE_FILE_DESCS,
            );
        });
    }

    fn tab_spring(&mut self, ui: &mut egui::Ui) {
        ui.heading("Initial Mesh / Spring");
        ui.separator();
        egui::Grid::new("spring").num_columns(2).show(ui, |ui| {
            int_row(ui, "Initial iterations (niter)", &mut self.mkgrd.niter, 0..=1_000_000);
            f32_row(ui, "Spring beta", &mut self.mkgrd.beta);
            f32_row(ui, "Relaxation factor", &mut self.mkgrd.relax);
            int_row(ui, "Refine iterations", &mut self.refine.niter_refine, 0..=1_000_000);
            int_combo_row(
                ui,
                "Spring-Global type",
                &mut self.refine.spring_global_type,
                &[(0, "0 — none"), (1, "1 — OLAM")],
            );
            int_row(ui, "num_rc (Spring-Global)", &mut self.refine.num_rc, 0..=1000);
            combo_row(ui, "Distance function", &mut self.refine.set_dis_type, SET_DIS_TYPES);
            int_combo_row(
                ui,
                "Spring-Regional type",
                &mut self.refine.spring_regional_type,
                &[(0, "0 — none"), (1, "1 — each step"), (2, "2 — final step")],
            );
            int_row(
                ui,
                "Protected vertex layers",
                &mut self.refine.vertex_pretect_layers,
                0..=1000,
            );
        });
    }

    fn tab_mask(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mask & Domain");
        ui.separator();
        egui::Grid::new("mask").num_columns(2).show(ui, |ui| {
            check_row(ui, "Global domain", &mut self.mkgrd.mask_domain_global);
            let regional = !self.mkgrd.mask_domain_global;
            ui.label("Domain shape");
            ui.add_enabled_ui(regional, |ui| {
                egui::ComboBox::from_id_salt("domain_type")
                    .selected_text(self.mkgrd.mask_domain_type.clone())
                    .show_ui(ui, |ui| {
                        for opt in REGION_TYPES {
                            ui.selectable_value(
                                &mut self.mkgrd.mask_domain_type,
                                (*opt).to_string(),
                                *opt,
                            );
                        }
                    });
            });
            ui.end_row();
            ui.label("Domain file prefix");
            ui.add_enabled(
                regional,
                egui::TextEdit::singleline(&mut self.mkgrd.mask_domain_fprefix).desired_width(280.0),
            );
            ui.end_row();

            check_row(ui, "Restart from mask", &mut self.mkgrd.mask_restart);
            f64_row(ui, "Sea/land ratio", &mut self.mkgrd.mask_sea_ratio);
            check_row(ui, "Enable mask patch", &mut self.mkgrd.mask_patch_on);
            let patch = self.mkgrd.mask_patch_on;
            ui.label("Patch shape");
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
            ui.label("Patch file prefix");
            ui.add_enabled(
                patch,
                egui::TextEdit::singleline(&mut self.mkgrd.mask_patch_fprefix).desired_width(280.0),
            );
            ui.end_row();
            check_row(ui, "Remove isolated ocean", &mut self.mkgrd.isolated_ocean);
        });
    }

    fn tab_refine_general(&mut self, ui: &mut egui::Ui) {
        ui.heading("Refinement — General");
        ui.separator();
        ui.checkbox(&mut self.mkgrd.refine, "Perform refinement (master switch)");
        ui.add_space(4.0);
        ui.add_enabled_ui(self.mkgrd.refine, |ui| {
            egui::Grid::new("refine_general").num_columns(2).show(ui, |ui| {
                check_row(ui, "Eliminate weak concavity", &mut self.refine.weak_concav_eliminate);
                check_row(ui, "Use transition rows (Istransition)", &mut self.refine.is_transition);
                check_row(ui, "Iterative-D (iterD)", &mut self.refine.iter_d);
                ui.label("HALO (per level)");
                ui.horizontal(|ui| {
                    for i in 1..=9 {
                        ui.add(egui::DragValue::new(&mut self.refine.halo[i]).speed(1.0));
                    }
                });
                ui.end_row();
                ui.label("Transition rows (per level)");
                ui.horizontal(|ui| {
                    for i in 1..=9 {
                        ui.add(egui::DragValue::new(&mut self.refine.max_transition_row[i]).speed(1.0));
                    }
                });
                ui.end_row();
            });
        });
    }

    fn tab_refine_criteria(&mut self, ui: &mut egui::Ui) {
        ui.heading("Refinement — Specified & Calculated");
        ui.separator();
        let atmos = self.mkgrd.mesh_type == "atmosmesh";
        ui.add_enabled_ui(self.mkgrd.refine, |ui| {
            egui::Grid::new("refine_spc").num_columns(2).show(ui, |ui| {
                check_row(ui, "Specified-region refine (refine_spc)", &mut self.refine.refine_spc);
                let spc = self.refine.refine_spc;
                ui.label("Max specified passes");
                ui.add_enabled(spc, egui::DragValue::new(&mut self.refine.max_iter_spc).range(0..=100));
                ui.end_row();
                ui.label("Specified region shape");
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
                ui.label("Specified region prefix");
                ui.add_enabled(spc, egui::TextEdit::singleline(&mut self.refine.mask_refine_spc_fprefix).desired_width(280.0));
                ui.end_row();
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_enabled(!atmos, egui::Checkbox::new(&mut self.refine.refine_cal, "Threshold-calculated refine (refine_cal)"));
                if atmos {
                    ui.weak("(not available for atmosmesh)");
                }
            });
            let cal = self.refine.refine_cal && !atmos;
            ui.add_enabled_ui(cal, |ui| {
                egui::Grid::new("refine_cal").num_columns(2).show(ui, |ui| {
                    int_row(ui, "Max calculated passes", &mut self.refine.max_iter_cal, 0..=100);
                    combo_row(ui, "Calculated region shape", &mut self.refine.mask_refine_cal_type, REGION_TYPES);
                    text_row(ui, "Calculated region prefix", &mut self.refine.mask_refine_cal_fprefix);
                    text_row(ui, "Threshold dir", &mut self.refine.threshold_dir);
                });
            });

            // Criteria groups (collapsible). Show all; user picks per mesh type.
            ui.add_space(6.0);
            egui::CollapsingHeader::new("Land — one-layer criteria").show(ui, |ui| {
                egui::Grid::new("lnd1").num_columns(2).show(ui, |ui| {
                    ui.checkbox(&mut self.refine.refine_num_landtypes, "Number of land types");
                    ui.add_enabled(self.refine.refine_num_landtypes, egui::DragValue::new(&mut self.refine.th_num_landtypes).range(0..=1000));
                    ui.end_row();
                    crit_row(ui, "Dominant land-type area", &mut self.refine.refine_area_mainland, &mut self.refine.th_area_mainland);
                    crit_row(ui, "LAI mean", &mut self.refine.refine_onelayer_lnd[0], &mut self.refine.th_onelayer_lnd[0]);
                    crit_row(ui, "LAI std", &mut self.refine.refine_onelayer_lnd[1], &mut self.refine.th_onelayer_lnd[1]);
                    crit_row(ui, "Slope mean", &mut self.refine.refine_onelayer_lnd[2], &mut self.refine.th_onelayer_lnd[2]);
                    crit_row(ui, "Slope std", &mut self.refine.refine_onelayer_lnd[3], &mut self.refine.th_onelayer_lnd[3]);
                });
            });
            egui::CollapsingHeader::new("Land — two-layer soil criteria").show(ui, |ui| {
                let names = [
                    "k_s mean", "k_s std", "k_solids mean", "k_solids std", "tkdry mean",
                    "tkdry std", "tksatf mean", "tksatf std", "tksatu mean", "tksatu std",
                ];
                egui::Grid::new("lnd2").num_columns(2).show(ui, |ui| {
                    for i in 0..10 {
                        crit_pair_row(ui, names[i], &mut self.refine.refine_twolayer_lnd[i], &mut self.refine.th_twolayer_lnd[i]);
                    }
                });
            });
            egui::CollapsingHeader::new("Ocean criteria").show(ui, |ui| {
                egui::Grid::new("ocn").num_columns(2).show(ui, |ui| {
                    crit_pair_row(ui, "Sea/land ratio", &mut self.refine.refine_sea_ratio, &mut self.refine.th_sea_ratio);
                    let names = ["SST mean", "SST std", "SSH mean", "SSH std", "EKE mean", "EKE std", "Sea slope mean", "Sea slope std"];
                    for i in 0..8 {
                        crit_row(ui, names[i], &mut self.refine.refine_onelayer_ocn[i], &mut self.refine.th_onelayer_ocn[i]);
                    }
                });
            });
            egui::CollapsingHeader::new("Atmosphere criteria").show(ui, |ui| {
                egui::Grid::new("atm").num_columns(2).show(ui, |ui| {
                    crit_row(ui, "Typhoon freq. mean", &mut self.refine.refine_onelayer_atmos[0], &mut self.refine.th_onelayer_atmos[0]);
                    crit_row(ui, "Typhoon freq. std", &mut self.refine.refine_onelayer_atmos[1], &mut self.refine.th_onelayer_atmos[1]);
                });
            });
        });
    }
}

fn configure_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style.visuals.widgets.noninteractive.bg_stroke.width = 1.0;
    for (_text_style, font) in style.text_styles.iter_mut() {
        font.size *= 1.05;
    }
    ctx.set_style(style);
}

impl eframe::App for EarthMeshApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_run();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.heading("EarthMesh");
                ui.separator();
                if ui.button("📂 Load example").clicked() {
                    self.load(workspace_root().join("examples/default/atmosphere_hex_global.nml"));
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
            ui.add_space(2.0);
        });

        egui::SidePanel::left("nav").resizable(false).default_width(150.0).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading("Sections");
            ui.separator();
            ui.selectable_value(&mut self.tab, Tab::Basics, "Basics");
            ui.selectable_value(&mut self.tab, Tab::Spring, "Initial Mesh");
            ui.selectable_value(&mut self.tab, Tab::Mask, "Mask & Domain");
            ui.selectable_value(&mut self.tab, Tab::RefineGeneral, "Refine — General");
            ui.selectable_value(&mut self.tab, Tab::RefineCriteria, "Refine — Criteria");
        });

        egui::SidePanel::right("run").resizable(true).default_width(280.0).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading("Run");
            ui.separator();
            ui.label(if self.running { "● running" } else { "○ idle" });
            ui.add_space(6.0);
            ui.label(&self.status);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                Tab::Basics => self.tab_basics(ui),
                Tab::Spring => self.tab_spring(ui),
                Tab::Mask => self.tab_mask(ui),
                Tab::RefineGeneral => self.tab_refine_general(ui),
                Tab::RefineCriteria => self.tab_refine_criteria(ui),
            });
        });
    }
}

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "EarthMesh",
        native_options,
        Box::new(|cc| {
            configure_style(&cc.egui_ctx);
            let mut app = EarthMeshApp::default();
            // Start with a real namelist so the form shows meaningful values.
            app.load(workspace_root().join("examples/default/atmosphere_hex_global.nml"));
            Ok(Box::new(app))
        }),
    )
}
