use std::collections::HashSet;
use std::path::Path;

use crate::{
    namelist_assignments, DataLayerRole, DataLayersNamelist, EarthmeshConfig, RefineConfig,
    ThresholdVar,
};

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
            ThresholdVar::Dem => (RefineSwitchArray::OneLayerLnd, 4),
            ThresholdVar::SlopeMax => (RefineSwitchArray::OneLayerLnd, 6),
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
    /// flip its mean+std `refine_*` switches and enable `refine_cal`.
    /// **Does not touch the mesh algorithm** — it only fills config fields the
    /// engine already consumes.
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
                    if l.categorical_enabled {
                        refine.refine_num_landtypes = true;
                        refine.refine_cal = true;
                    }
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
                    if l.mean_enabled {
                        set_refine_switch(refine, arr, base);
                    }
                    if l.std_enabled {
                        set_refine_switch(refine, arr, base + 1);
                    }
                    if l.mean_enabled || l.std_enabled {
                        refine.refine_cal = true;
                        report.enabled_thresholds.push(v);
                    }
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
/// `&mkgrd` + `&mkrefine` namelist with the lowered values, while preserving
/// additional execution groups such as `&hfield` and `&quality`. This lets a
/// plain `mkgrd.x`-style run honour declared layers without silently changing
/// the refinement or quality backend selected by the project compiler.
///
/// `threshold_dir_fallback` is used when `RL%threshold_dir` is omitted or
/// explicitly empty (the caller typically passes `<namelist_dir>/threshold`).
pub fn lower_datalayers_namelist(
    text: &str,
    threshold_dir_fallback: Option<&str>,
) -> Result<LoweredDatalayers, String> {
    let threshold_dir_was_explicit = namelist_group_has_field(text, "mkrefine", "threshold_dir")?;
    let mut mkgrd = EarthmeshConfig::from_mkgrd_namelist(text)?;
    let mut refine = if text.to_ascii_lowercase().contains("&mkrefine") {
        RefineConfig::from_mkrefine_namelist(text, &mkgrd.mesh_type, &mkgrd.mode_grid)?
    } else {
        RefineConfig::default()
    };
    let dl = DataLayersNamelist::from_datalayers_namelist(text);
    let mut threshold_fields = HashSet::new();
    for layer in &dl.layers {
        if layer.enabled {
            if let DataLayerRole::ThresholdField(field) = layer.role {
                if !threshold_fields.insert(field.file_stem()) {
                    return Err(format!(
                        "enabled threshold field '{}' is duplicated",
                        field.file_stem()
                    ));
                }
            }
        }
    }
    let report = dl.lower_into(&mut mkgrd, &mut refine);
    if !threshold_dir_was_explicit || refine.threshold_dir.trim().is_empty() {
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
    let native_method_c_assignments = preserve_native_method_c_assignments(text)?;
    let mut mkgrd_namelist = mkgrd.to_mkgrd_namelist();
    if !native_method_c_assignments.is_empty() {
        let insert_at = mkgrd_namelist
            .rfind("/\n")
            .ok_or_else(|| "rewritten &mkgrd group has no terminator".to_string())?;
        mkgrd_namelist.insert_str(insert_at, &native_method_c_assignments);
    }
    let preserved_groups = preserve_unlowered_namelist_groups(text)?;
    let mut namelist = format!("{}\n{}", mkgrd_namelist, refine.to_mkrefine_namelist());
    if !preserved_groups.is_empty() {
        namelist.push('\n');
        namelist.push_str(&preserved_groups);
    }
    Ok(LoweredDatalayers {
        namelist,
        threshold_dir: refine.threshold_dir.clone(),
        threshold_files,
        warnings: report.warnings,
    })
}

fn preserve_native_method_c_assignments(text: &str) -> Result<String, String> {
    let mut out = String::new();
    for assignment in namelist_assignments(text, "mkgrd")? {
        if crate::mkgrd_config::is_native_method_c_field(&assignment.field) {
            out.push_str("  NL%");
            out.push_str(&assignment.field);
            if !assignment.indices.is_empty() {
                out.push('(');
                out.push_str(
                    &assignment
                        .indices
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                );
                out.push(')');
            }
            out.push_str(" = ");
            out.push_str(&assignment.value);
            out.push('\n');
        }
    }
    Ok(out)
}

fn namelist_group_has_field(text: &str, group: &str, wanted_field: &str) -> Result<bool, String> {
    Ok(namelist_assignments(text, group)?
        .iter()
        .any(|assignment| assignment.field.eq_ignore_ascii_case(wanted_field)))
}

fn preserve_unlowered_namelist_groups(text: &str) -> Result<String, String> {
    let mut out = String::new();
    for span in crate::namelist_syntax::namelist_group_spans(text)? {
        if !matches!(
            span.name.to_ascii_lowercase().as_str(),
            "mkgrd" | "mkrefine" | "datalayers"
        ) {
            out.push_str(span.text);
            out.push('\n');
        }
    }
    Ok(out)
}
