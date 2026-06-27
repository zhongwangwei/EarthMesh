use crate::{fortran_quote, parse_fortran_string, strip_fortran_comment};

/// A NetCDF criterion field that calculated refinement reads from
/// `threshold_dir/<file_stem>.nc`. The stem is the **authoritative engine name**
/// (`earthmesh_cli` `AREA_JUDGE_*_NAMES`); keep this in lock-step with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThresholdVar {
    Lai,
    Slope,
    Ks,
    KSolids,
    Tkdry,
    Tksatf,
    Tksatu,
    Sst,
    Ssh,
    Eke,
    SeaSlope,
    Typhoon,
}

impl ThresholdVar {
    /// Engine file stem / NetCDF base var name (`lai`, `slope_avg`, `k_s`, ...).
    pub fn file_stem(self) -> &'static str {
        match self {
            ThresholdVar::Lai => "lai",
            ThresholdVar::Slope => "slope_avg",
            ThresholdVar::Ks => "k_s",
            ThresholdVar::KSolids => "k_solids",
            ThresholdVar::Tkdry => "tkdry",
            ThresholdVar::Tksatf => "tksatf",
            ThresholdVar::Tksatu => "tksatu",
            ThresholdVar::Sst => "sst",
            ThresholdVar::Ssh => "ssh",
            ThresholdVar::Eke => "eke",
            ThresholdVar::SeaSlope => "sea_slope",
            ThresholdVar::Typhoon => "typhoon",
        }
    }

    /// Two-layer fields expose NetCDF vars `<stem>_l1` / `<stem>_l2`.
    pub fn is_two_layer(self) -> bool {
        matches!(
            self,
            ThresholdVar::Ks
                | ThresholdVar::KSolids
                | ThresholdVar::Tkdry
                | ThresholdVar::Tksatf
                | ThresholdVar::Tksatu
        )
    }

    pub fn from_stem(s: &str) -> Option<ThresholdVar> {
        Some(match s {
            "lai" => ThresholdVar::Lai,
            "slope_avg" => ThresholdVar::Slope,
            "k_s" => ThresholdVar::Ks,
            "k_solids" => ThresholdVar::KSolids,
            "tkdry" => ThresholdVar::Tkdry,
            "tksatf" => ThresholdVar::Tksatf,
            "tksatu" => ThresholdVar::Tksatu,
            "sst" => ThresholdVar::Sst,
            "ssh" => ThresholdVar::Ssh,
            "eke" => ThresholdVar::Eke,
            "sea_slope" => ThresholdVar::SeaSlope,
            "typhoon" => ThresholdVar::Typhoon,
            _ => return None,
        })
    }
}

/// Which engine input a data layer feeds once a project is lowered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataLayerRole {
    /// -> `EarthmeshConfig.landtype_file` (sea/land + landtype).
    LandType,
    /// -> `threshold_dir/<stem>.nc` + the matching `refine_*` switch.
    ThresholdField(ThresholdVar),
    /// -> `mask_refine_spc_fprefix` (specified-region mask).
    SpecifiedMask,
    /// -> MERIT-Hydro workflow root.
    MeritHydroRoot,
    /// -> CaMa reach data.
    CamaReach,
}

impl DataLayerRole {
    pub fn to_token(self) -> String {
        match self {
            DataLayerRole::LandType => "landtype".to_string(),
            DataLayerRole::ThresholdField(v) => format!("threshold:{}", v.file_stem()),
            DataLayerRole::SpecifiedMask => "specified_mask".to_string(),
            DataLayerRole::MeritHydroRoot => "merit_hydro".to_string(),
            DataLayerRole::CamaReach => "cama_reach".to_string(),
        }
    }

    pub fn from_token(s: &str) -> Option<DataLayerRole> {
        if let Some(stem) = s.strip_prefix("threshold:") {
            return ThresholdVar::from_stem(stem).map(DataLayerRole::ThresholdField);
        }
        Some(match s {
            "landtype" => DataLayerRole::LandType,
            "specified_mask" => DataLayerRole::SpecifiedMask,
            "merit_hydro" => DataLayerRole::MeritHydroRoot,
            "cama_reach" => DataLayerRole::CamaReach,
            _ => return None,
        })
    }
}

/// One declarative input layer (the GUI step-3 data-layer manager). Lowered to
/// the engine inputs implied by `role`. **Additive**: carried in an optional
/// `&datalayers` block that existing parsers ignore.
#[derive(Clone, Debug, PartialEq)]
pub struct DataLayerConfig {
    pub id: String,
    pub role: DataLayerRole,
    pub path: String,
    pub var: Option<String>,
    pub enabled: bool,
    pub required: bool,
}

/// The `&datalayers` block: an ordered list of [`DataLayerConfig`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataLayersNamelist {
    pub layers: Vec<DataLayerConfig>,
}

impl DataLayersNamelist {
    /// Parse a `&datalayers` block. Each layer is one
    /// `NL%layer = 'id|role|path|var|enabled|required'` line. Lenient: malformed
    /// or unknown-role lines are skipped; an absent block yields an empty list.
    pub fn from_datalayers_namelist(input: &str) -> Self {
        let mut layers = Vec::new();
        let mut in_block = false;
        for raw_line in input.lines() {
            let line = strip_fortran_comment(raw_line).trim().trim_end_matches(',');
            if line.is_empty() {
                continue;
            }
            if line.starts_with('&') {
                in_block = line.eq_ignore_ascii_case("&datalayers");
                continue;
            }
            if line == "/" {
                in_block = false;
                continue;
            }
            if !in_block {
                continue;
            }
            let Some((left, right)) = line.split_once('=') else {
                continue;
            };
            let Some(field) = left.trim().split_once('%').map(|(_, f)| f.trim()) else {
                continue;
            };
            if field.eq_ignore_ascii_case("layer") {
                let token = parse_fortran_string(right.trim().trim_end_matches(','));
                if let Some(layer) = parse_data_layer_token(&token) {
                    layers.push(layer);
                }
            }
        }
        DataLayersNamelist { layers }
    }

    /// Serialize to a `&datalayers` block; round-trips through the parser.
    pub fn to_datalayers_namelist(&self) -> String {
        let mut out = String::from("&datalayers\n");
        for l in &self.layers {
            out.push_str(&format!(
                "  NL%layer = {}\n",
                fortran_quote(&data_layer_token(l))
            ));
        }
        out.push_str("/\n");
        out
    }
}

fn data_layer_token(l: &DataLayerConfig) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        l.id,
        l.role.to_token(),
        l.path,
        l.var.as_deref().unwrap_or(""),
        if l.enabled { "T" } else { "F" },
        if l.required { "T" } else { "F" },
    )
}

fn parse_data_layer_token(s: &str) -> Option<DataLayerConfig> {
    let p: Vec<&str> = s.split('|').collect();
    if p.len() < 6 {
        return None;
    }
    let role = DataLayerRole::from_token(p[1])?;
    let truthy =
        |x: &str| x.eq_ignore_ascii_case("t") || x == "1" || x.eq_ignore_ascii_case("true");
    Some(DataLayerConfig {
        id: p[0].to_string(),
        role,
        path: p[2].to_string(),
        var: if p[3].is_empty() {
            None
        } else {
            Some(p[3].to_string())
        },
        enabled: truthy(p[4]),
        required: truthy(p[5]),
    })
}
