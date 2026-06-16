//! EarthMesh desktop GUI.
//!
//! Increment 3b: the full three-pane workbench (all 92 `&mkgrd`/`&mkrefine`
//! options across five tabs) plus a built-in 中文/English switch. A small static
//! translation table keeps the UI strings bilingual without pulling in an i18n
//! macro crate. Visual polish (spacing/theme) is deferred.

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

mod i18n;
use i18n::{tr, Lang};

const MESH_TYPES: &[&str] = &["landmesh", "oceanmesh", "atmosmesh", "LOCmesh"];
const MODE_GRIDS: &[&str] = &["lonlat", "lambert", "cubical", "tri", "hex", "dbx"];
const REGION_TYPES: &[&str] = &["bbox", "lambert", "close", "circle"];
const SET_DIS_TYPES: &[&str] = &["linear", "nonlinear1", "nonlinear2", "nonlinear3"];
const MODE_FILE_DESCS: &[&str] = &["none", "EarthMesh", "MPAS", "IAP-Ocean", "FVCOM"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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
    lang: Lang,
    status_key: &'static str,
    status_detail: String,
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
            lang: Lang::En,
            status_key: "status.ready",
            status_detail: String::new(),
            running: false,
            run_rx: None,
        }
    }
}

impl EarthMeshApp {
    fn set_status(&mut self, key: &'static str, detail: String) {
        self.status_key = key;
        self.status_detail = detail;
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
                self.set_status("status.loaded", path.display().to_string());
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
            Ok(()) => self.set_status("status.saved", path.display().to_string()),
            Err(err) => self.set_status("status.write_error", err.to_string()),
        }
    }

    fn start_run(&mut self, ctx: &egui::Context) {
        if self.running {
            return;
        }
        let workdir = workspace_root();
        let nml_path = std::env::temp_dir().join("earthmesh_gui_run.nml");
        if let Err(err) = std::fs::write(&nml_path, self.combined_namelist()) {
            return self.set_status("status.stage_error", err.to_string());
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
                Ok(_) => Ok(workdir.join(out_hint.trim_start_matches("./")).display().to_string()),
                Err(err) => Err(err.to_string()),
            };
            let _ = tx.send(RunMsg::Done(msg));
            ctx.request_repaint();
        });
        self.run_rx = Some(rx);
        self.running = true;
        self.set_status("status.running", String::new());
    }

    fn poll_run(&mut self) {
        if let Some(rx) = &self.run_rx {
            if let Ok(RunMsg::Done(result)) = rx.try_recv() {
                match result {
                    Ok(out) => self.set_status("status.run_done", out),
                    Err(err) => self.set_status("status.run_failed", err),
                }
                self.running = false;
                self.run_rx = None;
            }
        }
    }
}

// ---- grid-row helpers (label already translated by the caller) -----------------

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
            text_row(ui, tr(lang, "f.expnme"), &mut self.mkgrd.experiment_name);
            text_row(ui, tr(lang, "f.base_dir"), &mut self.mkgrd.base_dir);
            combo_row(ui, tr(lang, "f.mesh_type"), &mut self.mkgrd.mesh_type, MESH_TYPES);

            let allowed = output_formats_for(&self.mkgrd.mesh_type);
            if !allowed.contains(&self.mkgrd.output_format.as_str()) {
                self.mkgrd.output_format = allowed[0].to_string();
            }
            combo_row(ui, tr(lang, "f.output_format"), &mut self.mkgrd.output_format, allowed);

            combo_row(ui, tr(lang, "f.mode_grid"), &mut self.mkgrd.mode_grid, MODE_GRIDS);
            int_row(ui, tr(lang, "f.nxp"), &mut self.mkgrd.nxp, 1..=100_000);
            int_combo_row(
                ui,
                tr(lang, "f.gridnum"),
                &mut self.mkgrd.gridnum_perdegree,
                &[(120, "120"), (240, "240")],
            );
            int_row(ui, tr(lang, "f.openmp"), &mut self.mkgrd.openmp, 1..=1024);
            text_row(ui, tr(lang, "f.landtype_file"), &mut self.mkgrd.landtype_file);
            text_row(ui, tr(lang, "f.mode_file"), &mut self.mkgrd.mode_file);
            combo_row(ui, tr(lang, "f.mode_file_desc"), &mut self.mkgrd.mode_file_description, MODE_FILE_DESCS);
        });
    }

    fn tab_spring(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.spring"));
        ui.separator();
        egui::Grid::new("spring").num_columns(2).show(ui, |ui| {
            int_row(ui, tr(lang, "f.niter"), &mut self.mkgrd.niter, 0..=1_000_000);
            f32_row(ui, tr(lang, "f.beta"), &mut self.mkgrd.beta);
            f32_row(ui, tr(lang, "f.relax"), &mut self.mkgrd.relax);
            int_row(ui, tr(lang, "f.niter_refine"), &mut self.refine.niter_refine, 0..=1_000_000);
            int_combo_row(
                ui,
                tr(lang, "f.spring_global"),
                &mut self.refine.spring_global_type,
                &[(0, tr(lang, "opt.spring_none")), (1, tr(lang, "opt.spring_olam"))],
            );
            int_row(ui, tr(lang, "f.num_rc"), &mut self.refine.num_rc, 0..=1000);
            combo_row(ui, tr(lang, "f.set_dis"), &mut self.refine.set_dis_type, SET_DIS_TYPES);
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
            int_row(ui, tr(lang, "f.vertex_layers"), &mut self.refine.vertex_pretect_layers, 0..=1000);
        });
    }

    fn tab_mask(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.mask"));
        ui.separator();
        egui::Grid::new("mask").num_columns(2).show(ui, |ui| {
            check_row(ui, tr(lang, "f.domain_global"), &mut self.mkgrd.mask_domain_global);
            let regional = !self.mkgrd.mask_domain_global;
            ui.label(tr(lang, "f.domain_shape"));
            ui.add_enabled_ui(regional, |ui| {
                egui::ComboBox::from_id_salt("domain_type")
                    .selected_text(self.mkgrd.mask_domain_type.clone())
                    .show_ui(ui, |ui| {
                        for opt in REGION_TYPES {
                            ui.selectable_value(&mut self.mkgrd.mask_domain_type, (*opt).to_string(), *opt);
                        }
                    });
            });
            ui.end_row();
            ui.label(tr(lang, "f.domain_prefix"));
            ui.add_enabled(regional, egui::TextEdit::singleline(&mut self.mkgrd.mask_domain_fprefix).desired_width(280.0));
            ui.end_row();

            check_row(ui, tr(lang, "f.mask_restart"), &mut self.mkgrd.mask_restart);
            f64_row(ui, tr(lang, "f.sea_ratio"), &mut self.mkgrd.mask_sea_ratio);
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
            check_row(ui, tr(lang, "f.isolated_ocean"), &mut self.mkgrd.isolated_ocean);
        });
    }

    fn tab_refine_general(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.refine_general"));
        ui.separator();
        ui.checkbox(&mut self.mkgrd.refine, tr(lang, "f.refine_master"));
        ui.add_space(4.0);
        ui.add_enabled_ui(self.mkgrd.refine, |ui| {
            egui::Grid::new("refine_general").num_columns(2).show(ui, |ui| {
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
    }

    fn tab_refine_criteria(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.heading(tr(lang, "head.refine_criteria"));
        ui.separator();
        let atmos = self.mkgrd.mesh_type == "atmosmesh";
        ui.add_enabled_ui(self.mkgrd.refine, |ui| {
            egui::Grid::new("refine_spc").num_columns(2).show(ui, |ui| {
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
                ui.add_enabled(!atmos, egui::Checkbox::new(&mut self.refine.refine_cal, tr(lang, "f.refine_cal")));
                if atmos {
                    ui.weak(tr(lang, "note.cal_atmos"));
                }
            });
            let cal = self.refine.refine_cal && !atmos;
            ui.add_enabled_ui(cal, |ui| {
                egui::Grid::new("refine_cal").num_columns(2).show(ui, |ui| {
                    int_row(ui, tr(lang, "f.max_iter_cal"), &mut self.refine.max_iter_cal, 0..=100);
                    combo_row(ui, tr(lang, "f.cal_shape"), &mut self.refine.mask_refine_cal_type, REGION_TYPES);
                    text_row(ui, tr(lang, "f.cal_prefix"), &mut self.refine.mask_refine_cal_fprefix);
                    text_row(ui, tr(lang, "f.threshold_dir"), &mut self.refine.threshold_dir);
                });
            });

            ui.add_space(6.0);
            egui::CollapsingHeader::new(tr(lang, "g.lnd1")).show(ui, |ui| {
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
            });
            egui::CollapsingHeader::new(tr(lang, "g.lnd2")).show(ui, |ui| {
                let keys = ["c.ks_m","c.ks_s","c.ksol_m","c.ksol_s","c.tkdry_m","c.tkdry_s","c.tksatf_m","c.tksatf_s","c.tksatu_m","c.tksatu_s"];
                egui::Grid::new("lnd2").num_columns(2).show(ui, |ui| {
                    for i in 0..10 {
                        crit_pair_row(ui, tr(lang, keys[i]), &mut self.refine.refine_twolayer_lnd[i], &mut self.refine.th_twolayer_lnd[i]);
                    }
                });
            });
            egui::CollapsingHeader::new(tr(lang, "g.ocn")).show(ui, |ui| {
                egui::Grid::new("ocn").num_columns(2).show(ui, |ui| {
                    crit_pair_row(ui, tr(lang, "c.sea_ratio"), &mut self.refine.refine_sea_ratio, &mut self.refine.th_sea_ratio);
                    let keys = ["c.sst_m","c.sst_s","c.ssh_m","c.ssh_s","c.eke_m","c.eke_s","c.seaslope_m","c.seaslope_s"];
                    for i in 0..8 {
                        crit_row(ui, tr(lang, keys[i]), &mut self.refine.refine_onelayer_ocn[i], &mut self.refine.th_onelayer_ocn[i]);
                    }
                });
            });
            egui::CollapsingHeader::new(tr(lang, "g.atmos")).show(ui, |ui| {
                egui::Grid::new("atm").num_columns(2).show(ui, |ui| {
                    crit_row(ui, tr(lang, "c.typhoon_m"), &mut self.refine.refine_onelayer_atmos[0], &mut self.refine.th_onelayer_atmos[0]);
                    crit_row(ui, tr(lang, "c.typhoon_s"), &mut self.refine.refine_onelayer_atmos[1], &mut self.refine.th_onelayer_atmos[1]);
                });
            });
        });
    }
}

/// Register a CJK-capable font as a fallback so 中文 renders instead of tofu
/// boxes. egui's bundled fonts are Latin-only. We load the first available
/// system font from a per-OS candidate list and append it after the defaults,
/// so Latin keeps the default look and only missing (CJK) glyphs fall back.
/// (Embedding a font would make this self-contained cross-platform; deferred.)
fn install_fonts(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        // macOS
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        // Linux
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        // Windows
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
    ];
    let mut fonts = egui::FontDefinitions::default();
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
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
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    for (_s, font) in style.text_styles.iter_mut() {
        font.size *= 1.05;
    }
    ctx.set_style(style);
}

impl eframe::App for EarthMeshApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_run();
        let lang = self.lang;

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.heading(tr(lang, "app.title"));
                ui.separator();
                if ui.button(tr(lang, "btn.load")).clicked() {
                    self.load(workspace_root().join("examples/default/atmosphere_hex_global.nml"));
                }
                if ui.button(tr(lang, "btn.save")).clicked() {
                    let path = self
                        .loaded_path
                        .clone()
                        .map(|p| p.with_extension("saved.nml"))
                        .unwrap_or_else(|| workspace_root().join("earthmesh_gui_out.nml"));
                    self.save(path);
                }
                ui.separator();
                ui.add_enabled_ui(!self.running, |ui| {
                    if ui.button(tr(lang, "btn.run")).clicked() {
                        self.start_run(ctx);
                    }
                });
                if self.running {
                    ui.add(egui::Spinner::new());
                }
                // language switch (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut self.lang, Lang::Zh, "中文");
                    ui.selectable_value(&mut self.lang, Lang::En, "EN");
                    ui.label(tr(lang, "lang.label"));
                });
            });
            ui.add_space(2.0);
        });

        egui::SidePanel::left("nav").resizable(false).default_width(160.0).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(tr(lang, "nav.title"));
            ui.separator();
            ui.selectable_value(&mut self.tab, Tab::Basics, tr(lang, "nav.basics"));
            ui.selectable_value(&mut self.tab, Tab::Spring, tr(lang, "nav.spring"));
            ui.selectable_value(&mut self.tab, Tab::Mask, tr(lang, "nav.mask"));
            ui.selectable_value(&mut self.tab, Tab::RefineGeneral, tr(lang, "nav.refine_general"));
            ui.selectable_value(&mut self.tab, Tab::RefineCriteria, tr(lang, "nav.refine_criteria"));
        });

        egui::SidePanel::right("run").resizable(true).default_width(280.0).show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(tr(lang, "run.title"));
            ui.separator();
            ui.label(tr(lang, if self.running { "run.running" } else { "run.idle" }));
            ui.add_space(6.0);
            let status = if self.status_detail.is_empty() {
                tr(lang, self.status_key).to_string()
            } else {
                format!("{}\n{}", tr(lang, self.status_key), self.status_detail)
            };
            ui.label(status);
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
            install_fonts(&cc.egui_ctx);
            configure_style(&cc.egui_ctx);
            let mut app = EarthMeshApp::default();
            app.load(workspace_root().join("examples/default/atmosphere_hex_global.nml"));
            Ok(Box::new(app))
        }),
    )
}
