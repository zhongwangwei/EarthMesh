use crate::{MeshDomainKind, ProjectDataLayer, ProjectLayerRole, ThresholdField};

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
    pub display_name: &'static str,
    pub physical_process: &'static str,
    pub field: ThresholdField,
    pub applicable: &'static [MeshDomainKind],
    pub gui: CriterionGuiSpec,
}

impl CriterionSpec {
    /// Build a project data layer for this criterion (role = its ThresholdField).
    pub fn to_data_layer(&self, path: impl Into<String>, enabled: bool) -> ProjectDataLayer {
        ProjectDataLayer {
            id: self.field.stem().to_string(),
            role: ProjectLayerRole::Threshold(self.field),
            path: path.into(),
            enabled,
        }
    }
}

const LAND: &[MeshDomainKind] = &[MeshDomainKind::Land, MeshDomainKind::Coupled];
const OCEAN: &[MeshDomainKind] = &[MeshDomainKind::Ocean, MeshDomainKind::Coupled];

/// The registered refinement criteria (one per engine threshold field).
const CATALOG: &[CriterionSpec] = &[
    CriterionSpec {
        id: "lai",
        display_name: "LAI variability",
        physical_process: "vegetation phenology / canopy heterogeneity",
        field: ThresholdField::Lai,
        applicable: LAND,
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
        display_name: "Terrain slope",
        physical_process: "orographic / runoff routing",
        field: ThresholdField::Slope,
        applicable: LAND,
        gui: CriterionGuiSpec {
            label: "Slope",
            help: "Refine where mean terrain slope exceeds the threshold",
            unit: "deg",
            range: (0.0, 45.0),
            default: 5.0,
        },
    },
    CriterionSpec {
        id: "k_s",
        display_name: "Saturated conductivity",
        physical_process: "soil hydraulics",
        field: ThresholdField::Ks,
        applicable: LAND,
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
        display_name: "Solids conductivity",
        physical_process: "soil thermal",
        field: ThresholdField::KSolids,
        applicable: LAND,
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
        display_name: "Dry thermal cond.",
        physical_process: "soil thermal",
        field: ThresholdField::Tkdry,
        applicable: LAND,
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
        display_name: "Frozen sat. thermal",
        physical_process: "soil thermal (frozen)",
        field: ThresholdField::Tksatf,
        applicable: LAND,
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
        display_name: "Unfrozen sat. thermal",
        physical_process: "soil thermal (unfrozen)",
        field: ThresholdField::Tksatu,
        applicable: LAND,
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
        display_name: "SST front",
        physical_process: "ocean surface temperature front",
        field: ThresholdField::Sst,
        applicable: OCEAN,
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
        display_name: "SSH gradient",
        physical_process: "ocean dynamic height",
        field: ThresholdField::Ssh,
        applicable: OCEAN,
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
        display_name: "Eddy kinetic energy",
        physical_process: "mesoscale eddies",
        field: ThresholdField::Eke,
        applicable: OCEAN,
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
        display_name: "Seafloor slope",
        physical_process: "bathymetric gradient / shelf break",
        field: ThresholdField::SeaSlope,
        applicable: OCEAN,
        gui: CriterionGuiSpec {
            label: "Sea slope",
            help: "Refine across steep seafloor slopes / shelf breaks",
            unit: "deg",
            range: (0.0, 30.0),
            default: 3.0,
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

/// Criteria applicable to a mesh domain (for the GUI refinement step).
pub fn criteria_for_domain(kind: MeshDomainKind) -> Vec<&'static CriterionSpec> {
    CATALOG
        .iter()
        .filter(|c| c.applicable.contains(&kind))
        .collect()
}
