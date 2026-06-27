use earthmesh_core::EarthmeshConfig;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MIN_ANGLE_DEG: f64 = 25.0;

pub fn default_mask_sea_ratio() -> f64 {
    EarthmeshConfig::default().mask_sea_ratio
}

// ----------------------------- top level -----------------------------

/// One project = one reproducible mesh production.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub schema_version: String,
    pub metadata: ProjectMetadata,
    pub domain: DomainConfig,
    pub target: MeshTargetConfig,
    #[serde(default)]
    pub data_layers: Vec<ProjectDataLayer>,
    #[serde(default)]
    pub refinement: RefinementRecipe,
    #[serde(default)]
    pub quality: QualityConfig,
    #[serde(default)]
    pub expert: ExpertOverrides,
    /// MERIT-Hydro / CaMa river-coast (routed to the hydro workflow, not mkgrd).
    #[serde(default)]
    pub hydro_coast: Option<HydroCoastConfig>,
    /// Land-ocean coupling options.
    #[serde(default)]
    pub coupling: Option<CoupledMeshConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: String,
}

// ----------------------------- domain -----------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DomainConfig {
    Global,
    Regional {
        shape: RegionShape,
        #[serde(default)]
        sea_ratio: Option<f64>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RegionShape {
    Bbox { w: f64, e: f64, n: f64, s: f64 },
    Circle { lon: f64, lat: f64, radius_km: f64 },
}

// ----------------------------- target -----------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshTargetConfig {
    pub kind: MeshDomainKind,
    pub cell: MeshCellKind,
    #[serde(default)]
    pub intent: MeshIntentPreset,
    pub resolution: ResolutionSpec,
    pub model_format: ModelFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshDomainKind {
    Land,
    Ocean,
    Atmosphere,
    Coupled,
    Earth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshCellKind {
    Hex,
    Tri,
}

/// Mesh intent presets exposed to project files and the GUI template gallery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshIntentPreset {
    #[default]
    Custom,
    HydrologyLand,
    CarbonLand,
    SnowPermafrostLand,
    UrbanLand,
    CoastalOcean,
    Estuary,
    RiverNetwork,
    MeritHydroCoast,
    LandOceanCoupled,
    #[serde(alias = "AtmosphereTyphoonPrecip")]
    AtmosphereMpas,
    MultiObjectiveBalanced,
}

pub const INTENT_PRESETS: &[MeshIntentPreset] = &[
    MeshIntentPreset::Custom,
    MeshIntentPreset::HydrologyLand,
    MeshIntentPreset::CarbonLand,
    MeshIntentPreset::SnowPermafrostLand,
    MeshIntentPreset::UrbanLand,
    MeshIntentPreset::CoastalOcean,
    MeshIntentPreset::Estuary,
    MeshIntentPreset::RiverNetwork,
    MeshIntentPreset::MeritHydroCoast,
    MeshIntentPreset::LandOceanCoupled,
    MeshIntentPreset::AtmosphereMpas,
    MeshIntentPreset::MultiObjectiveBalanced,
];

/// Friendly km, or an explicit engine NXP. `ApproxKm` lowers through `km_to_nxp`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResolutionSpec {
    ApproxKm(f64),
    Nxp(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFormat {
    CoLM,
    Mpas,
    MpasSimple,
    Fvcom,
    /// Compatibility parse path for old project files. The project layer does
    /// not expose native OLAM output; direct OLAM paths live in the engine/CLI.
    Olam,
}

// ----------------------------- data layers -----------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectDataLayer {
    pub id: String,
    pub role: ProjectLayerRole,
    pub path: String,
    #[serde(default)]
    pub enabled: bool,
}

/// Serde-friendly mirror of [`earthmesh_core::DataLayerRole`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLayerRole {
    LandType,
    Threshold(ThresholdField),
    SpecifiedMask,
    MeritHydro,
    Cama,
}

/// Serde-friendly mirror of [`earthmesh_core::ThresholdVar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThresholdField {
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

// ----------------------------- refinement / quality / expert -----------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RefinementRecipe {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub max_passes: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityConfig {
    pub min_angle_deg: f64,
    #[serde(default)]
    pub on_violation: ViolationPolicy,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            min_angle_deg: DEFAULT_MIN_ANGLE_DEG,
            on_violation: ViolationPolicy::Warn,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationPolicy {
    #[default]
    Warn,
    Block,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExpertOverrides {
    #[serde(default)]
    pub nxp: Option<i32>,
    #[serde(default)]
    pub openmp: Option<i32>,
}

/// MERIT-Hydro / CaMa river-coast config. Carried by the project
/// and routed to the hydro workflow CLI commands - it does not lower into the
/// mkgrd run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HydroCoastConfig {
    pub merit_root: String,
    #[serde(default)]
    pub cama_root: Option<String>,
    #[serde(default = "default_r3_width")]
    pub r3_width_m: f64,
    #[serde(default = "default_r2_width")]
    pub r2_width_m: f64,
}

fn default_r3_width() -> f64 {
    300.0
}

fn default_r2_width() -> f64 {
    50.0
}

/// Land-ocean coupling config.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CoupledMeshConfig {
    #[serde(default)]
    pub fraction_method: FractionMethod,
    #[serde(default)]
    pub identify_coastline: bool,
    #[serde(default)]
    pub identify_river_mouth: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FractionMethod {
    #[default]
    PointSample,
    ConservativeOverlay,
}

/// Rough km->NXP estimate. The engine defines no exact formula, so this is
/// anchored on the GUI defaults (about 9 km <-> NXP 40, i.e. NXP*km about 360)
/// and should be calibrated per mesh family. `ApproxKm` in [`ResolutionSpec`] uses it.
pub fn km_to_nxp(km: f64) -> i32 {
    if km <= 0.0 {
        return 1;
    }
    (360.0 / km).round().max(1.0) as i32
}
