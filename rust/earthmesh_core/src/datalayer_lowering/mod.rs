use std::path::Path;

use crate::{DataLayerRole, DataLayersNamelist, EarthmeshConfig, RefineConfig, ThresholdVar};

/// Which `RefineConfig` switch array a threshold criterion lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefineSwitchArray {
    OneLayerLnd,
    TwoLayerLnd,
    OneLayerOcn,
    OneLayerAtmos,
}

impl ThresholdVar {
    /// (switch array, base index of the *mean* switch; the *std* switch is
    /// base+1) — the authoritative parallel-array layout from the threshold_dir
    /// contract (`core` th_*/refine_* + `cli` AREA_JUDGE_*).
    pub fn switch_slot(self) -> (RefineSwitchArray, usize) {
        match self {
            ThresholdVar::Lai => (RefineSwitchArray::OneLayerLnd, 0),
            ThresholdVar::Slope => (RefineSwitchArray::OneLayerLnd, 2),
            ThresholdVar::Ks => (RefineSwitchArray::TwoLayerLnd, 0),
            ThresholdVar::KSolids => (RefineSwitchArray::TwoLayerLnd, 2),
            ThresholdVar::Tkdry => (RefineSwitchArray::TwoLayerLnd, 4),
            ThresholdVar::Tksatf => (RefineSwitchArray::TwoLayerLnd, 6),
            ThresholdVar::Tksatu => (RefineSwitchArray::TwoLayerLnd, 8),
            ThresholdVar::Sst => (RefineSwitchArray::OneLayerOcn, 0),
            ThresholdVar::Ssh => (RefineSwitchArray::OneLayerOcn, 2),
            ThresholdVar::Eke => (RefineSwitchArray::OneLayerOcn, 4),
            ThresholdVar::SeaSlope => (RefineSwitchArray::OneLayerOcn, 6),
            ThresholdVar::Typhoon => (RefineSwitchArray::OneLayerAtmos, 0),
        }
    }
}

/// Outcome of [`DataLayersNamelist::lower_into`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LowerReport {
    pub landtype_set: bool,
    pub enabled_thresholds: Vec<ThresholdVar>,
    pub specified_mask_set: bool,
    pub warnings: Vec<String>,
}

fn set_refine_switch(refine: &mut RefineConfig, arr: RefineSwitchArray, idx: usize) {
    let slot = match arr {
        RefineSwitchArray::OneLayerLnd => refine.refine_onelayer_lnd.get_mut(idx),
        RefineSwitchArray::TwoLayerLnd => refine.refine_twolayer_lnd.get_mut(idx),
        RefineSwitchArray::OneLayerOcn => refine.refine_onelayer_ocn.get_mut(idx),
        RefineSwitchArray::OneLayerAtmos => refine.refine_onelayer_atmos.get_mut(idx),
    };
    if let Some(s) = slot {
        *s = true;
    }
}

impl DataLayersNamelist {
    /// Apply enabled layers to the engine config (the L1->L3 lowering): set
    /// `landtype_file` from the LandType layer; for each enabled ThresholdField
    /// flip its mean+std `refine_*` switches and enable `refine_cal`; route a
    /// SpecifiedMask to `mask_refine_spc_fprefix`. **Does not touch the mesh
    /// algorithm** — it only fills config fields the engine already consumes.
    ///
    /// A ThresholdField whose path file stem != the engine stem (e.g. `lai`) is
    /// recorded as a warning, since the executor reads `threshold_dir/<stem>.nc`.
    pub fn lower_into(
        &self,
        mkgrd: &mut EarthmeshConfig,
        refine: &mut RefineConfig,
    ) -> LowerReport {
        let mut report = LowerReport::default();
        for l in &self.layers {
            if !l.enabled {
                continue;
            }
            match l.role {
                DataLayerRole::LandType => {
                    mkgrd.landtype_file = l.path.clone();
                    report.landtype_set = true;
                }
                DataLayerRole::ThresholdField(v) => {
                    let stem = v.file_stem();
                    let path_stem = Path::new(&l.path).file_stem().and_then(|s| s.to_str());
                    if let Some(found) = path_stem {
                        if found != stem {
                            report.warnings.push(format!(
                                "layer '{}': path stem '{found}' != engine stem '{stem}' (reads threshold_dir/{stem}.nc)",
                                l.id
                            ));
                        }
                    }
                    let (arr, base) = v.switch_slot();
                    set_refine_switch(refine, arr, base);
                    set_refine_switch(refine, arr, base + 1);
                    refine.refine_cal = true;
                    report.enabled_thresholds.push(v);
                }
                DataLayerRole::SpecifiedMask => {
                    refine.mask_refine_spc_fprefix = l.path.clone();
                    refine.refine_spc = true;
                    report.specified_mask_set = true;
                }
                DataLayerRole::MeritHydroRoot | DataLayerRole::CamaReach => {
                    // Hydro / CaMa roles flow through the dedicated hydro
                    // workflow, not the mkgrd/mkrefine config — nothing here.
                }
            }
        }
        report
    }
}

/// Result of [`lower_datalayers_namelist`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoweredDatalayers {
    /// The rewritten `&mkgrd` + `&mkrefine` namelist with lowered values applied.
    pub namelist: String,
    /// The `threshold_dir` the engine will read `<stem>.nc` from (user's value,
    /// or the fallback when the user left it empty).
    pub threshold_dir: String,
    /// (engine stem, source path) for each enabled ThresholdField layer — the
    /// caller stages these to `threshold_dir/<stem>.nc`.
    pub threshold_files: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

/// Parse a namelist's `&mkgrd` / `&mkrefine` / `&datalayers` blocks, apply the
/// data layers to the config ([`DataLayersNamelist::lower_into`]), and re-emit a
/// `&mkgrd` + `&mkrefine` namelist with the lowered values, so a plain
/// `mkgrd.x`-style run honours the declared layers without a GUI. The engine run
/// path consumes only `&mkgrd`/`&mkrefine`, so other blocks are not re-emitted.
///
/// `threshold_dir_fallback` is used only when the parsed `&mkrefine` leaves
/// `threshold_dir` empty (the caller typically passes `<namelist_dir>/threshold`).
pub fn lower_datalayers_namelist(
    text: &str,
    threshold_dir_fallback: Option<&str>,
) -> Result<LoweredDatalayers, String> {
    let mut mkgrd = EarthmeshConfig::from_mkgrd_namelist(text)?;
    let mut refine = if text.to_ascii_lowercase().contains("&mkrefine") {
        RefineConfig::from_mkrefine_namelist(text, &mkgrd.mesh_type, &mkgrd.mode_grid)?
    } else {
        RefineConfig::default()
    };
    let dl = DataLayersNamelist::from_datalayers_namelist(text);
    let report = dl.lower_into(&mut mkgrd, &mut refine);
    if refine.threshold_dir.trim().is_empty() {
        if let Some(fb) = threshold_dir_fallback {
            refine.threshold_dir = fb.to_string();
        }
    }
    let threshold_files = dl
        .layers
        .iter()
        .filter_map(|l| {
            if !l.enabled || l.path.trim().is_empty() {
                return None;
            }
            match l.role {
                DataLayerRole::ThresholdField(v) => {
                    Some((v.file_stem().to_string(), l.path.clone()))
                }
                _ => None,
            }
        })
        .collect();
    Ok(LoweredDatalayers {
        namelist: format!(
            "{}\n{}",
            mkgrd.to_mkgrd_namelist(),
            refine.to_mkrefine_namelist()
        ),
        threshold_dir: refine.threshold_dir.clone(),
        threshold_files,
        warnings: report.warnings,
    })
}
