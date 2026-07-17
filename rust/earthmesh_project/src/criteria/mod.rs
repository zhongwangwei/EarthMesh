use crate::{MeshDomainKind, ProjectConfig, ProjectDataLayer, ProjectLayerRole, ThresholdField};

/// Single categorical land-cover refinement criterion. The LandType data layer
/// remains independently usable as the land/sea mask when this criterion is off.
pub const LANDCOVER_CRITERION_ID: &str = "landcover";
pub const DEFAULT_LANDCOVER_CLASS_THRESHOLD: f64 = 12.0;

/// How the GUI renders a criterion's control (self-describing, so new criteria
/// automatically get GUI metadata from this schema.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CriterionGuiSpec {
    pub label: &'static str,
    pub help: &'static str,
    pub unit: &'static str,
    pub range: (f64, f64),
    pub default: f64,
}

/// A self-describing refinement criterion. The engine refines by threshold, so a
/// criterion maps to a [`ThresholdField`] (the engine input it drives) and
/// carries metadata + GUI spec rather than a cell-level `score()` fn (that stays
/// engine-side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CriterionSpec {
    pub id: &'static str,
    pub physical_process: &'static str,
    pub field: ThresholdField,
    pub applicable: &'static [MeshDomainKind],
    pub gui: CriterionGuiSpec,
}

/// Statistical criterion evaluated from one continuous threshold source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThresholdStatistic {
    Mean,
    Std,
}

impl ThresholdStatistic {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Mean => "mean",
            Self::Std => "std",
        }
    }
}

/// Flattened criterion catalog entry. `source_field` identifies the single
/// NetCDF data source; `statistic` identifies the independent engine switch.
#[derive(Clone, Debug, PartialEq)]
pub struct ThresholdCriterionSpec {
    pub id: String,
    pub label: String,
    pub source_field: ThresholdField,
    pub statistic: ThresholdStatistic,
    pub gui: CriterionGuiSpec,
}

/// Project-specific criterion after applying explicit axis overrides and legacy
/// `ProjectDataLayer::threshold_value` fallback.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveThresholdCriterion {
    pub id: String,
    pub source_layer_id: String,
    /// Whether the shared data source itself is currently enabled. This is
    /// deliberately separate from the criterion switch so disabling a source
    /// does not turn an implicit criterion default into an explicit `false`.
    pub source_enabled: bool,
    pub source_field: ThresholdField,
    pub statistic: ThresholdStatistic,
    /// The criterion's own mean/std switch, independent of source availability.
    pub enabled: bool,
    pub value: f64,
}

/// Project-specific categorical land-cover criterion. Unlike continuous
/// sources, LandType has one engine switch (`refine_num_landtypes`), not
/// separate mean/std axes.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveLandcoverCriterion {
    pub id: &'static str,
    pub source_layer_id: String,
    pub source_enabled: bool,
    pub enabled: bool,
    pub value: f64,
}

impl CriterionSpec {
    /// Build a project data layer for this criterion (role = its ThresholdField).
    pub fn to_data_layer(&self, path: impl Into<String>, enabled: bool) -> ProjectDataLayer {
        ProjectDataLayer {
            id: self.field.stem().to_string(),
            role: ProjectLayerRole::Threshold(self.field),
            path: path.into(),
            enabled,
            threshold_value: None,
        }
    }
}

const ALL_DOMAINS: &[MeshDomainKind] = &[
    MeshDomainKind::Land,
    MeshDomainKind::Ocean,
    MeshDomainKind::Atmosphere,
    MeshDomainKind::Coupled,
    MeshDomainKind::Earth,
];

/// The registered refinement criteria (one per engine threshold field).
const CATALOG: &[CriterionSpec] = &[
    CriterionSpec {
        id: "lai",
        physical_process: "vegetation phenology / canopy heterogeneity",
        field: ThresholdField::Lai,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "LAI",
            help: "Refine where leaf-area-index variability exceeds the threshold",
            unit: "m2/m2",
            range: (0.0, 10.0),
            default: 1.0,
        },
    },
    CriterionSpec {
        id: "slope",
        physical_process: "orographic / runoff routing",
        field: ThresholdField::Slope,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "Slope",
            help: "Refine where mean terrain slope exceeds the threshold",
            unit: "deg",
            range: (0.0, 45.0),
            default: 5.0,
        },
    },
    CriterionSpec {
        id: "dem",
        physical_process: "terrain elevation",
        field: ThresholdField::Dem,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "DEM",
            help: "Refine where terrain elevation variability exceeds the threshold",
            unit: "m",
            range: (0.0, 9000.0),
            default: 500.0,
        },
    },
    CriterionSpec {
        id: "slope_max",
        physical_process: "orographic / steep terrain",
        field: ThresholdField::SlopeMax,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "Max slope",
            help: "Refine where maximum terrain slope exceeds the threshold",
            unit: "deg",
            range: (0.0, 90.0),
            default: 15.0,
        },
    },
    CriterionSpec {
        id: "k_s",
        physical_process: "soil hydraulics",
        field: ThresholdField::Ks,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "k_s",
            help: "Refine on saturated hydraulic conductivity heterogeneity",
            unit: "mm/s",
            range: (0.0, 1.0),
            default: 0.01,
        },
    },
    CriterionSpec {
        id: "k_solids",
        physical_process: "soil thermal",
        field: ThresholdField::KSolids,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "k_solids",
            help: "Refine on soil-solids thermal conductivity heterogeneity",
            unit: "W/m/K",
            range: (0.0, 10.0),
            default: 2.0,
        },
    },
    CriterionSpec {
        id: "tkdry",
        physical_process: "soil thermal",
        field: ThresholdField::Tkdry,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "tkdry",
            help: "Refine on dry-soil thermal conductivity heterogeneity",
            unit: "W/m/K",
            range: (0.0, 1.0),
            default: 0.2,
        },
    },
    CriterionSpec {
        id: "tksatf",
        physical_process: "soil thermal (frozen)",
        field: ThresholdField::Tksatf,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "tksatf",
            help: "Refine on frozen saturated thermal conductivity",
            unit: "W/m/K",
            range: (0.0, 5.0),
            default: 2.0,
        },
    },
    CriterionSpec {
        id: "tksatu",
        physical_process: "soil thermal (unfrozen)",
        field: ThresholdField::Tksatu,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "tksatu",
            help: "Refine on unfrozen saturated thermal conductivity",
            unit: "W/m/K",
            range: (0.0, 5.0),
            default: 1.5,
        },
    },
    CriterionSpec {
        id: "sst",
        physical_process: "ocean surface temperature front",
        field: ThresholdField::Sst,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "SST",
            help: "Refine across sea-surface-temperature gradients",
            unit: "degC",
            range: (0.0, 5.0),
            default: 1.0,
        },
    },
    CriterionSpec {
        id: "ssh",
        physical_process: "ocean dynamic height",
        field: ThresholdField::Ssh,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "SSH",
            help: "Refine across sea-surface-height gradients",
            unit: "m",
            range: (0.0, 2.0),
            default: 0.2,
        },
    },
    CriterionSpec {
        id: "eke",
        physical_process: "mesoscale eddies",
        field: ThresholdField::Eke,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "EKE",
            help: "Refine in high eddy-kinetic-energy regions",
            unit: "cm2/s2",
            range: (0.0, 1000.0),
            default: 100.0,
        },
    },
    CriterionSpec {
        id: "sea_slope",
        physical_process: "bathymetric gradient / shelf break",
        field: ThresholdField::SeaSlope,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "Sea slope",
            help: "Refine across steep seafloor slopes / shelf breaks",
            unit: "deg",
            range: (0.0, 30.0),
            default: 3.0,
        },
    },
    CriterionSpec {
        id: "typhoon",
        physical_process: "atmospheric cyclone / precipitation forcing",
        field: ThresholdField::Typhoon,
        applicable: ALL_DOMAINS,
        gui: CriterionGuiSpec {
            label: "Typhoon",
            help: "Refine where typhoon forcing exceeds the threshold",
            unit: "index",
            range: (0.0, 1.0),
            default: 0.5,
        },
    },
];

/// All registered criteria.
pub fn criterion_catalog() -> &'static [CriterionSpec] {
    CATALOG
}

/// Look up a criterion by id (`lai`, `slope`, `sea_slope`, ...).
pub fn criterion_by_id(id: &str) -> Option<&'static CriterionSpec> {
    CATALOG.iter().find(|c| c.id == id)
}

/// Every continuous source expanded into its independent mean/std criteria.
pub fn threshold_criterion_catalog() -> Vec<ThresholdCriterionSpec> {
    CATALOG
        .iter()
        .flat_map(|source| {
            [ThresholdStatistic::Mean, ThresholdStatistic::Std].map(move |statistic| {
                ThresholdCriterionSpec {
                    id: format!("{}_{}", source.field.stem(), statistic.suffix()),
                    label: format!("{} {}", source.gui.label, statistic.suffix()),
                    source_field: source.field,
                    statistic,
                    gui: source.gui,
                }
            })
        })
        .collect()
}

pub fn threshold_criterion_by_id(id: &str) -> Option<ThresholdCriterionSpec> {
    threshold_criterion_catalog()
        .into_iter()
        .find(|criterion| criterion.id == id)
}

impl ProjectConfig {
    /// Resolve the categorical land-cover criterion independently from the
    /// LandType mask/source toggle. Legacy projects that set only
    /// `ProjectDataLayer::threshold_value` retain their previous behavior;
    /// an explicit criterion entry always wins and can disable that fallback.
    pub fn effective_landcover_criterion(&self) -> Option<EffectiveLandcoverCriterion> {
        let source = self
            .data_layers
            .iter()
            .find(|layer| layer.enabled && layer.role == ProjectLayerRole::LandType)
            .or_else(|| {
                self.data_layers
                    .iter()
                    .find(|layer| layer.role == ProjectLayerRole::LandType)
            })?;
        let explicit = self
            .refinement
            .threshold_criteria
            .iter()
            .find(|criterion| criterion.id == LANDCOVER_CRITERION_ID);
        Some(EffectiveLandcoverCriterion {
            id: LANDCOVER_CRITERION_ID,
            source_layer_id: source.id.clone(),
            source_enabled: source.enabled,
            enabled: explicit.map_or(source.threshold_value.is_some(), |criterion| {
                criterion.enabled
            }),
            value: explicit.map_or_else(
                || {
                    source
                        .threshold_value
                        .unwrap_or(DEFAULT_LANDCOVER_CLASS_THRESHOLD)
                },
                |criterion| criterion.value.unwrap_or(DEFAULT_LANDCOVER_CLASS_THRESHOLD),
            ),
        })
    }

    /// Resolve one mean/std criterion without duplicating its data-source path.
    /// Explicit `refinement.threshold_criteria` entries are self-contained:
    /// their blank value means the catalog default. Only an omitted entry falls
    /// back to the legacy shared `threshold_value`.
    pub fn effective_threshold_criterion(
        &self,
        field: ThresholdField,
        statistic: ThresholdStatistic,
    ) -> Option<EffectiveThresholdCriterion> {
        let source = self
            .data_layers
            .iter()
            .find(|layer| layer.enabled && layer.role == ProjectLayerRole::Threshold(field))
            .or_else(|| {
                self.data_layers
                    .iter()
                    .find(|layer| layer.role == ProjectLayerRole::Threshold(field))
            })?;
        let catalog = CATALOG.iter().find(|criterion| criterion.field == field)?;
        let id = format!("{}_{}", field.stem(), statistic.suffix());
        let explicit = self
            .refinement
            .threshold_criteria
            .iter()
            .find(|criterion| criterion.id == id);
        Some(EffectiveThresholdCriterion {
            id,
            source_layer_id: source.id.clone(),
            source_enabled: source.enabled,
            source_field: field,
            statistic,
            enabled: explicit.is_none_or(|criterion| criterion.enabled),
            value: explicit.map_or_else(
                || source.threshold_value.unwrap_or(catalog.gui.default),
                |criterion| criterion.value.unwrap_or(catalog.gui.default),
            ),
        })
    }
}

/// Criteria available to a mesh domain (currently the full shared catalog).
pub fn criteria_for_domain(kind: MeshDomainKind) -> Vec<&'static CriterionSpec> {
    CATALOG
        .iter()
        .filter(|c| c.applicable.contains(&kind))
        .collect()
}
