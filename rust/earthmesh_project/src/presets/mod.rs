use crate::{
    criterion_catalog, DomainConfig, ExpertOverrides, MeshCellKind, MeshDomainKind,
    MeshIntentPreset, MeshTargetConfig, ModelFormat, ProjectConfig, ProjectDataLayer,
    ProjectLayerRole, ProjectMetadata, QualityConfig, RefinementBackend, RefinementRecipe,
    ResolutionSpec, ThresholdField, ViolationPolicy, DEFAULT_MIN_ANGLE_DEG, INTENT_PRESETS,
};

pub const DEPRECATED_ATMOSPHERE_TYPHOON_INTENT_ID: &str = "AtmosphereTyphoonPrecip";

/// Suggested scaffold defaults for a [`MeshIntentPreset`] - the GUI "pick a
/// template" flow. Every registered threshold and non-threshold input is
/// scaffolded; `extra_roles` only records which non-threshold inputs the preset
/// historically recommended by default.
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
            MeshIntentPreset::MeritHydroCoast => "Coastal zone · MERIT-Hydro",
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
        let (kind, cell, fmt, extra_roles): (
            MeshDomainKind,
            MeshCellKind,
            ModelFormat,
            Vec<ProjectLayerRole>,
        ) = match self {
            MeshIntentPreset::Custom => (Land, Hex, CoLM, vec![R::LandType]),
            MeshIntentPreset::HydrologyLand => (Land, Hex, CoLM, vec![R::LandType, R::MeritHydro]),
            MeshIntentPreset::CarbonLand => (Land, Hex, CoLM, vec![R::LandType]),
            MeshIntentPreset::SnowPermafrostLand => (Land, Hex, CoLM, vec![R::LandType]),
            MeshIntentPreset::UrbanLand => (Land, Hex, CoLM, vec![R::LandType]),
            // Ocean meshes carve to ocean-only cells from the SAME landcover the
            // land meshes use (sea/land classification), so they need a LandType
            // layer too - otherwise there is nothing to mask the land away with.
            MeshIntentPreset::CoastalOcean => (Ocean, Tri, Fvcom, vec![R::LandType]),
            MeshIntentPreset::Estuary => (Ocean, Tri, Fvcom, vec![R::LandType, R::Cama]),
            MeshIntentPreset::RiverNetwork => (Land, Hex, CoLM, vec![R::LandType, R::MeritHydro]),
            MeshIntentPreset::MeritHydroCoast => {
                (Coupled, Hex, CoLM, vec![R::LandType, R::MeritHydro])
            }
            MeshIntentPreset::LandOceanCoupled => (Coupled, Hex, CoLM, vec![R::LandType]),
            MeshIntentPreset::AtmosphereMpas => (Atmosphere, Hex, Mpas, vec![R::LandType]),
            MeshIntentPreset::MultiObjectiveBalanced => (Coupled, Hex, CoLM, vec![R::LandType]),
        };
        PresetDefaults {
            kind,
            cell,
            model_format: fmt,
            criteria: criterion_catalog()
                .iter()
                .map(|criterion| criterion.field)
                .collect(),
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
        for role in [
            ProjectLayerRole::LandType,
            ProjectLayerRole::MeritHydro,
            ProjectLayerRole::Cama,
        ] {
            let is_default_landtype = role == ProjectLayerRole::LandType
                && d.extra_roles.contains(&ProjectLayerRole::LandType);
            data_layers.push(ProjectDataLayer {
                id: role.role_kind().to_string(),
                role,
                path: if is_default_landtype {
                    "input/landtype_igbp_update.nc".to_string()
                } else {
                    String::new()
                },
                enabled: is_default_landtype,
                threshold_value: None,
            });
        }
        for c in &d.criteria {
            data_layers.push(ProjectDataLayer {
                id: c.stem().to_string(),
                role: ProjectLayerRole::Threshold(*c),
                path: String::new(),
                enabled: false,
                threshold_value: None,
            });
        }
        ProjectConfig {
            schema_version: crate::PROJECT_SCHEMA_VERSION.to_string(),
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
                backend: RefinementBackend::default(),
                enabled: false,
                threshold_enabled: false,
                max_passes: 0,
                threshold_criteria: Vec::new(),
                method_c: Default::default(),
                harp_dv: Default::default(),
                adaptive: None,
                specified_circle: None,
                specified_bbox: None,
                specified_close: None,
                hfield: None,
            },
            quality: QualityConfig {
                min_angle_deg: d.min_angle_deg,
                auto_refine_batch_cells: crate::DEFAULT_AUTO_REFINE_BATCH_CELLS,
                on_violation: ViolationPolicy::AutoRefine,
                lepp_post_quality: None,
            },
            expert: ExpertOverrides::default(),
            hydro_coast: None,
            coupling: None,
        }
    }
}
