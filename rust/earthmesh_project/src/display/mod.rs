use crate::{criterion_catalog, ProjectLayerRole, ThresholdField};

impl ThresholdField {
    /// Engine file stem / data-layer id (`lai`, `slope_avg`, ...).
    pub fn stem(self) -> &'static str {
        self.to_core().file_stem()
    }
}

impl ProjectLayerRole {
    pub fn role_kind(self) -> &'static str {
        match self {
            ProjectLayerRole::LandType => "landcover",
            ProjectLayerRole::MeritHydro => "merit",
            ProjectLayerRole::Cama => "cama",
            ProjectLayerRole::Threshold(_) => "threshold",
        }
    }

    pub fn wants_folder(self) -> bool {
        matches!(self, ProjectLayerRole::MeritHydro | ProjectLayerRole::Cama)
    }

    pub fn label(self) -> String {
        match self {
            ProjectLayerRole::LandType => "land type".to_string(),
            ProjectLayerRole::MeritHydro => "MERIT-Hydro".to_string(),
            ProjectLayerRole::Cama => "CaMa".to_string(),
            ProjectLayerRole::Threshold(field) => {
                let label = criterion_catalog()
                    .iter()
                    .find(|criterion| criterion.field == field)
                    .map(|criterion| criterion.gui.label)
                    .unwrap_or_else(|| field.stem());
                format!("threshold · {label}")
            }
        }
    }
}
