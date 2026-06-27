use crate::{
    DomainConfig, ExpertOverrides, MeshCellKind, MeshDomainKind, MeshIntentPreset,
    MeshTargetConfig, ModelFormat, ProjectConfig, ProjectDataLayer, ProjectLayerRole,
    ProjectMetadata, QualityConfig, RefinementRecipe, ResolutionSpec, ThresholdField,
    ViolationPolicy, DEFAULT_MIN_ANGLE_DEG, INTENT_PRESETS,
};

pub const DEPRECATED_ATMOSPHERE_TYPHOON_INTENT_ID: &str = "AtmosphereTyphoonPrecip";

/// Suggested scaffold defaults for a [`MeshIntentPreset`] - the GUI "pick a
/// template" flow. Threshold `criteria` become disabled data-layer entries the
/// user points at files; `extra_roles` are non-threshold inputs (landtype, MERIT,
/// CaMa). Criteria use the engine's real [`ThresholdField`]s; richer criteria
/// (river/coastline plugins) should stay in their workflow-specific config.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetDefaults {
    pub kind: MeshDomainKind,
    pub cell: MeshCellKind,
    pub model_format: ModelFormat,
    pub criteria: Vec<ThresholdField>,
    pub extra_roles: Vec<ProjectLayerRole>,
    pub min_angle_deg: f64,
}

impl MeshIntentPreset {
    pub fn all() -> &'static [MeshIntentPreset] {
        INTENT_PRESETS
    }

    pub fn id(self) -> &'static str {
        match self {
            MeshIntentPreset::Custom => "Custom",
            MeshIntentPreset::HydrologyLand => "HydrologyLand",
            MeshIntentPreset::CarbonLand => "CarbonLand",
            MeshIntentPreset::SnowPermafrostLand => "SnowPermafrostLand",
            MeshIntentPreset::UrbanLand => "UrbanLand",
            MeshIntentPreset::CoastalOcean => "CoastalOcean",
            MeshIntentPreset::Estuary => "Estuary",
            MeshIntentPreset::RiverNetwork => "RiverNetwork",
            MeshIntentPreset::MeritHydroCoast => "MeritHydroCoast",
            MeshIntentPreset::LandOceanCoupled => "LandOceanCoupled",
            MeshIntentPreset::AtmosphereMpas => "AtmosphereMpas",
            MeshIntentPreset::MultiObjectiveBalanced => "MultiObjectiveBalanced",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|preset| preset.id() == id)
            .or(match id {
                DEPRECATED_ATMOSPHERE_TYPHOON_INTENT_ID => Some(Self::AtmosphereMpas),
                _ => None,
            })
    }

    pub fn label(self) -> &'static str {
        match self {
            MeshIntentPreset::Custom => "Land · Basic",
            MeshIntentPreset::HydrologyLand => "Land · Hydrology",
            MeshIntentPreset::CarbonLand => "Land · Carbon",
            MeshIntentPreset::SnowPermafrostLand => "Land · Snow / Permafrost",
            MeshIntentPreset::UrbanLand => "Land · Urban",
            MeshIntentPreset::CoastalOcean => "Ocean · Coastal",
            MeshIntentPreset::Estuary => "Ocean · Estuary",
            MeshIntentPreset::RiverNetwork => "Land · River network",
            MeshIntentPreset::MeritHydroCoast => "Coast · MERIT-Hydro",
            MeshIntentPreset::LandOceanCoupled => "Coupled · Land–Ocean",
            MeshIntentPreset::AtmosphereMpas => "Atmosphere · MPAS",
            MeshIntentPreset::MultiObjectiveBalanced => "Multi-objective balanced",
        }
    }

    /// Suggested defaults (domain / cell / format / criteria / quality) for the
    /// intent - used to scaffold a project.
    pub fn defaults(self) -> PresetDefaults {
        use MeshCellKind::{Hex, Tri};
        use MeshDomainKind::{Atmosphere, Coupled, Land, Ocean};
        use ModelFormat::{CoLM, Fvcom, Mpas};
        use ProjectLayerRole as R;
        use ThresholdField as T;
        let (kind, cell, fmt, criteria, extra_roles): (
            MeshDomainKind,
            MeshCellKind,
            ModelFormat,
            Vec<ThresholdField>,
            Vec<ProjectLayerRole>,
        ) = match self {
            MeshIntentPreset::Custom => (Land, Hex, CoLM, vec![], vec![R::LandType]),
            MeshIntentPreset::HydrologyLand => (
                Land,
                Hex,
                CoLM,
                vec![T::Slope],
                vec![R::LandType, R::MeritHydro],
            ),
            MeshIntentPreset::CarbonLand => {
                (Land, Hex, CoLM, vec![T::Lai, T::Ks], vec![R::LandType])
            }
            MeshIntentPreset::SnowPermafrostLand => (
                Land,
                Hex,
                CoLM,
                vec![T::Slope, T::Tkdry, T::Tksatf],
                vec![R::LandType],
            ),
            MeshIntentPreset::UrbanLand => (Land, Hex, CoLM, vec![T::Lai], vec![R::LandType]),
            // Ocean meshes carve to ocean-only cells from the SAME landcover the
            // land meshes use (sea/land classification), so they need a LandType
            // layer too - otherwise there is nothing to mask the land away with.
            MeshIntentPreset::CoastalOcean => {
                (Ocean, Tri, Fvcom, vec![T::SeaSlope], vec![R::LandType])
            }
            MeshIntentPreset::Estuary => (
                Ocean,
                Tri,
                Fvcom,
                vec![T::SeaSlope],
                vec![R::LandType, R::Cama],
            ),
            MeshIntentPreset::RiverNetwork => (
                Land,
                Hex,
                CoLM,
                vec![T::Slope],
                vec![R::LandType, R::MeritHydro],
            ),
            MeshIntentPreset::MeritHydroCoast => (
                Coupled,
                Hex,
                CoLM,
                vec![T::Slope],
                vec![R::LandType, R::MeritHydro],
            ),
            MeshIntentPreset::LandOceanCoupled => (
                Coupled,
                Hex,
                CoLM,
                vec![T::Lai, T::SeaSlope],
                vec![R::LandType],
            ),
            MeshIntentPreset::AtmosphereMpas => (Atmosphere, Hex, Mpas, vec![], vec![]),
            MeshIntentPreset::MultiObjectiveBalanced => (
                Coupled,
                Hex,
                CoLM,
                vec![T::Lai, T::Slope, T::SeaSlope],
                vec![R::LandType],
            ),
        };
        PresetDefaults {
            kind,
            cell,
            model_format: fmt,
            criteria,
            extra_roles,
            min_angle_deg: DEFAULT_MIN_ANGLE_DEG,
        }
    }
}

impl ProjectConfig {
    /// Scaffold a project from an intent preset: target plus disabled data-layer
    /// entries (extra roles + threshold criteria) the user then points at files.
    pub fn scaffold(
        name: &str,
        intent: MeshIntentPreset,
        domain: DomainConfig,
        resolution: ResolutionSpec,
    ) -> ProjectConfig {
        let d = intent.defaults();
        let mut data_layers = Vec::new();
        for role in &d.extra_roles {
            data_layers.push(ProjectDataLayer {
                id: role.default_id().to_string(),
                role: *role,
                path: String::new(),
                enabled: false,
            });
        }
        for c in &d.criteria {
            data_layers.push(ProjectDataLayer {
                id: c.stem().to_string(),
                role: ProjectLayerRole::Threshold(*c),
                path: String::new(),
                enabled: false,
            });
        }
        ProjectConfig {
            schema_version: "3.0.0".to_string(),
            metadata: ProjectMetadata {
                name: name.to_string(),
                ..Default::default()
            },
            domain,
            target: MeshTargetConfig {
                kind: d.kind,
                cell: d.cell,
                intent,
                resolution,
                model_format: d.model_format,
            },
            data_layers,
            refinement: RefinementRecipe {
                enabled: !d.criteria.is_empty(),
                max_passes: if d.criteria.is_empty() { 0 } else { 3 },
            },
            quality: QualityConfig {
                min_angle_deg: d.min_angle_deg,
                on_violation: ViolationPolicy::Warn,
            },
            expert: ExpertOverrides::default(),
            hydro_coast: None,
            coupling: None,
        }
    }
}
