//! EarthMesh v3 project schema (the L1 "intent" layer).
//!
//! A serde-(de)serializable [`ProjectConfig`] (YAML/JSON) that **lowers** to the
//! engine's [`earthmesh_core::EarthmeshConfig`] + [`earthmesh_core::RefineConfig`]
//! (+ the `&quality` / `&datalayers` blocks), reusing the core lowering built in
//! `earthmesh_core`. This keeps the friendly project layer separate from the 64
//! flat engine fields (audit 03 §2/§9: additive, zero migration).
//!
//! Slice 1: the config spine + serde + `lower()`. Intent presets (03 §2.3) and
//! plugin criteria (03 §3) are layered on in later slices.

use earthmesh_core::{
    DataLayerConfig, DataLayerRole, DataLayersNamelist, EarthmeshConfig, QualityNamelist,
    RefineConfig, ThresholdVar,
};
use serde::{Deserialize, Serialize};

// ----------------------------- top level -----------------------------

/// One project = one reproducible mesh production (audit 03 §2.1).
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

/// The 12 mesh intents of audit 03 §2.3 (+ `Custom`). Slice 1 carries the value;
/// preset-driven defaults (criteria weights / data layers) land in slice 2.
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
    AtmosphereTyphoonPrecip,
    MultiObjectiveBalanced,
}

/// Friendly km, or an explicit engine NXP. km→NXP conversion is not defined in
/// the engine yet (audit), so `ApproxKm` does not set NXP today.
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
            min_angle_deg: 25.0,
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

/// MERIT-Hydro / CaMa river-coast config (audit 03 §3). Carried by the project
/// and routed to the hydro workflow CLI commands — it does not lower into the
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

/// Land-ocean coupling config (audit 03 §3).
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

/// Rough km→NXP estimate. The engine defines no exact formula, so this is
/// anchored on the GUI defaults (≈9 km ↔ NXP 40, i.e. NXP·km ≈ 360) and should
/// be **calibrated per mesh family**. `ApproxKm` in [`ResolutionSpec`] uses it.
pub fn km_to_nxp(km: f64) -> i32 {
    if km <= 0.0 {
        return 1;
    }
    (360.0 / km).round().max(1.0) as i32
}

// ----------------------------- engine mappings -----------------------------

impl MeshDomainKind {
    fn engine_str(self) -> &'static str {
        match self {
            MeshDomainKind::Land => "landmesh",
            MeshDomainKind::Ocean => "oceanmesh",
            MeshDomainKind::Atmosphere => "atmosmesh",
            MeshDomainKind::Coupled => "LOCmesh",
            MeshDomainKind::Earth => "earthmesh",
        }
    }
}

impl MeshCellKind {
    fn engine_str(self) -> &'static str {
        match self {
            MeshCellKind::Hex => "hex",
            MeshCellKind::Tri => "tri",
        }
    }
}

impl ModelFormat {
    fn engine_str(self) -> &'static str {
        match self {
            ModelFormat::CoLM => "CoLM",
            ModelFormat::Mpas => "MPAS",
            ModelFormat::MpasSimple => "MPAS-Simple",
            ModelFormat::Fvcom => "FVCOM",
            ModelFormat::Olam => "OLAM",
        }
    }
}

impl ThresholdField {
    fn to_core(self) -> ThresholdVar {
        match self {
            ThresholdField::Lai => ThresholdVar::Lai,
            ThresholdField::Slope => ThresholdVar::Slope,
            ThresholdField::Ks => ThresholdVar::Ks,
            ThresholdField::KSolids => ThresholdVar::KSolids,
            ThresholdField::Tkdry => ThresholdVar::Tkdry,
            ThresholdField::Tksatf => ThresholdVar::Tksatf,
            ThresholdField::Tksatu => ThresholdVar::Tksatu,
            ThresholdField::Sst => ThresholdVar::Sst,
            ThresholdField::Ssh => ThresholdVar::Ssh,
            ThresholdField::Eke => ThresholdVar::Eke,
            ThresholdField::SeaSlope => ThresholdVar::SeaSlope,
            ThresholdField::Typhoon => ThresholdVar::Typhoon,
        }
    }
}

impl ProjectLayerRole {
    fn to_core(self) -> DataLayerRole {
        match self {
            ProjectLayerRole::LandType => DataLayerRole::LandType,
            ProjectLayerRole::Threshold(t) => DataLayerRole::ThresholdField(t.to_core()),
            ProjectLayerRole::SpecifiedMask => DataLayerRole::SpecifiedMask,
            ProjectLayerRole::MeritHydro => DataLayerRole::MeritHydroRoot,
            ProjectLayerRole::Cama => DataLayerRole::CamaReach,
        }
    }
}

// ----------------------------- lowering -----------------------------

/// The L3 engine execution plan produced by [`ProjectConfig::lower`].
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredProject {
    pub mkgrd: EarthmeshConfig,
    pub refine: RefineConfig,
    pub data_layers: DataLayersNamelist,
    pub quality: QualityNamelist,
}

impl LoweredProject {
    /// Emit a runnable namelist (`&mkgrd` + `&mkrefine` + `&quality` + `&datalayers`).
    pub fn to_namelist(&self) -> String {
        // The engine validates &mkrefine whenever the block is present — even for a
        // baseline grid — and that check rejects hex without Istransition and demands
        // refine_spc/cal, both meaningless when refine is off. A baseline grid only
        // needs &mkgrd, so omit &mkrefine when refinement is disabled.
        let mkrefine = if self.mkgrd.refine {
            format!("{}\n", self.refine.to_mkrefine_namelist())
        } else {
            String::new()
        };
        format!(
            "{}\n{}{}\n{}",
            self.mkgrd.to_mkgrd_namelist(),
            mkrefine,
            self.quality.to_quality_namelist(),
            self.data_layers.to_datalayers_namelist()
        )
    }
}

impl ProjectConfig {
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
    pub fn from_yaml(s: &str) -> Result<Self, String> {
        serde_yaml::from_str(s).map_err(|e| e.to_string())
    }
    pub fn to_yaml(&self) -> Result<String, String> {
        serde_yaml::to_string(self).map_err(|e| e.to_string())
    }

    fn data_layers_namelist(&self) -> DataLayersNamelist {
        let layers = self
            .data_layers
            .iter()
            .map(|l| DataLayerConfig {
                id: l.id.clone(),
                role: l.role.to_core(),
                path: l.path.clone(),
                var: None,
                enabled: l.enabled,
                required: matches!(l.role, ProjectLayerRole::LandType),
            })
            .collect();
        DataLayersNamelist { layers }
    }

    fn quality_namelist(&self) -> QualityNamelist {
        let mut q = QualityNamelist::default();
        q.min_angle_warn_deg = self.quality.min_angle_deg;
        q.on_violation = match self.quality.on_violation {
            ViolationPolicy::Block => "block".to_string(),
            ViolationPolicy::Warn => "warn".to_string(),
        };
        q
    }

    /// Lower the project (L1) to engine config (L3). Reuses the core lowering for
    /// data layers; the mesh algorithm is untouched.
    pub fn lower(&self) -> LoweredProject {
        let mut mkgrd = EarthmeshConfig::default();
        let mut refine = RefineConfig::default();

        mkgrd.experiment_name = self.metadata.name.clone();
        mkgrd.mesh_type = self.target.kind.engine_str().to_string();
        mkgrd.mode_grid = self.target.cell.engine_str().to_string();
        mkgrd.output_format = self.target.model_format.engine_str().to_string();
        match self.target.resolution {
            ResolutionSpec::Nxp(n) => mkgrd.nxp = n,
            ResolutionSpec::ApproxKm(km) => mkgrd.nxp = km_to_nxp(km),
        }

        match &self.domain {
            DomainConfig::Global => mkgrd.mask_domain_global = true,
            DomainConfig::Regional { shape, .. } => {
                mkgrd.mask_domain_global = false;
                mkgrd.mask_domain_type = match shape {
                    RegionShape::Bbox { .. } => "bbox",
                    RegionShape::Circle { .. } => "circle",
                }
                .to_string();
            }
        }

        // Data layers drive landtype_file + refine switches (core lowering).
        let dl = self.data_layers_namelist();
        dl.lower_into(&mut mkgrd, &mut refine);
        // Refinement actually runs only when a threshold (refine_cal) or
        // specified-mask (refine_spc) layer supplies data. Landcover/hydro layers
        // set inputs but DON'T drive refinement — turning `refine` on for them
        // sends a data-less run down the OLAM specified-refine path, which then
        // errors ("requires refine_spc/refine_cal/native…"). That is exactly why a
        // land/ocean mesh with only landcover failed to run. Gate the recipe
        // toggle on a real refinement source so such a mesh runs uniform instead.
        mkgrd.refine = self.refinement.enabled && (refine.refine_cal || refine.refine_spc);

        // The engine runs hex meshes with Istransition=true, and then exactly one of
        // SpringGlobal/SpringRegional may be > 0 (core validate_like_read_nl). Tri
        // meshes keep is_transition=false (the engine then zeroes both spring types).
        if mkgrd.mode_grid != "tri" {
            refine.is_transition = true;
            refine.spring_global_type = if mkgrd.mask_domain_global { 1 } else { 0 };
            refine.spring_regional_type = if mkgrd.mask_domain_global { 0 } else { 1 };
        }

        // Expert overrides win last.
        if let Some(n) = self.expert.nxp {
            mkgrd.nxp = n;
        }
        if let Some(t) = self.expert.openmp {
            mkgrd.openmp = t;
        }

        LoweredProject {
            mkgrd,
            refine,
            data_layers: dl,
            quality: self.quality_namelist(),
        }
    }
}

// ----------------------------- reproducibility manifest -----------------------------

/// A content fingerprint of one input file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputFingerprint {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Auto-generated record for reproducing a run (audit 03 §3, layer L4): tool /
/// schema versions, input file hashes, and the lowered namelist snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReproducibilityManifest {
    pub tool_version: String,
    pub schema_version: String,
    pub project_name: String,
    pub inputs: Vec<InputFingerprint>,
    pub lowered_namelist: String,
}

impl ReproducibilityManifest {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

fn sha256_file(path: &str) -> Option<InputFingerprint> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Some(InputFingerprint {
        path: path.to_string(),
        sha256,
        bytes: bytes.len() as u64,
    })
}

impl ProjectConfig {
    /// Build a reproducibility manifest: hash the enabled data-layer inputs and
    /// snapshot the lowered namelist. Files that can't be read are skipped.
    pub fn reproducibility_manifest(&self) -> ReproducibilityManifest {
        let inputs = self
            .data_layers
            .iter()
            .filter(|l| l.enabled && !l.path.trim().is_empty())
            .filter_map(|l| sha256_file(&l.path))
            .collect();
        ReproducibilityManifest {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: self.schema_version.clone(),
            project_name: self.metadata.name.clone(),
            inputs,
            lowered_namelist: self.lower().to_namelist(),
        }
    }
}

// ----------------------------- intent presets (slice 2) -----------------------------

/// Suggested scaffold defaults for a [`MeshIntentPreset`] — the GUI "pick a
/// template" flow. Threshold `criteria` become disabled data-layer stubs the
/// user points at files; `extra_roles` are non-threshold inputs (landtype, MERIT,
/// CaMa). Criteria use the engine's real [`ThresholdField`]s; richer criteria
/// (river/coastline plugins) land in slice 3.
#[derive(Clone, Debug, PartialEq)]
pub struct PresetDefaults {
    pub kind: MeshDomainKind,
    pub cell: MeshCellKind,
    pub model_format: ModelFormat,
    pub criteria: Vec<ThresholdField>,
    pub extra_roles: Vec<ProjectLayerRole>,
    pub min_angle_deg: f64,
}

impl ThresholdField {
    /// Engine file stem / data-layer id (`lai`, `slope_avg`, ...).
    pub fn stem(self) -> &'static str {
        self.to_core().file_stem()
    }
}

impl ProjectLayerRole {
    fn default_id(self) -> &'static str {
        match self {
            ProjectLayerRole::LandType => "landcover",
            ProjectLayerRole::MeritHydro => "merit",
            ProjectLayerRole::Cama => "cama",
            ProjectLayerRole::SpecifiedMask => "mask",
            ProjectLayerRole::Threshold(_) => "threshold",
        }
    }
}

impl MeshIntentPreset {
    /// Suggested defaults (domain / cell / format / criteria / quality) for the
    /// intent — used to scaffold a project.
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
            MeshIntentPreset::HydrologyLand => {
                (Land, Hex, CoLM, vec![T::Slope], vec![R::LandType, R::MeritHydro])
            }
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
            // layer too — otherwise there is nothing to mask the land away with.
            MeshIntentPreset::CoastalOcean => {
                (Ocean, Tri, Fvcom, vec![T::SeaSlope], vec![R::LandType])
            }
            MeshIntentPreset::Estuary => {
                (Ocean, Tri, Fvcom, vec![T::SeaSlope], vec![R::LandType, R::Cama])
            }
            MeshIntentPreset::RiverNetwork => {
                (Land, Hex, CoLM, vec![T::Slope], vec![R::LandType, R::MeritHydro])
            }
            MeshIntentPreset::MeritHydroCoast => {
                (Coupled, Hex, CoLM, vec![T::Slope], vec![R::LandType, R::MeritHydro])
            }
            MeshIntentPreset::LandOceanCoupled => {
                (Coupled, Hex, CoLM, vec![T::Lai, T::SeaSlope], vec![R::LandType])
            }
            MeshIntentPreset::AtmosphereTyphoonPrecip => {
                (Atmosphere, Hex, Mpas, vec![T::Typhoon], vec![])
            }
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
            min_angle_deg: 25.0,
        }
    }
}

impl ProjectConfig {
    /// Scaffold a project from an intent preset: target + disabled data-layer
    /// stubs (extra roles + threshold criteria) the user then points at files.
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
                max_passes: 3,
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

// ----------------------------- criteria catalog (slice 3) -----------------------------

/// How the GUI renders a criterion's control (self-describing, so new criteria
/// get a GUI for free — audit 03 §3 / principle 5).
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
/// engine-side). `physical_process` is mandatory (principle 2).
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
            id: self.id.to_string(),
            role: ProjectLayerRole::Threshold(self.field),
            path: path.into(),
            enabled,
        }
    }
}

const LAND: &[MeshDomainKind] = &[MeshDomainKind::Land, MeshDomainKind::Coupled];
const OCEAN: &[MeshDomainKind] = &[MeshDomainKind::Ocean, MeshDomainKind::Coupled];
const ATMOS: &[MeshDomainKind] = &[MeshDomainKind::Atmosphere];

/// The registered refinement criteria (one per engine threshold field).
const CATALOG: &[CriterionSpec] = &[
    CriterionSpec { id: "lai", display_name: "LAI variability", physical_process: "vegetation phenology / canopy heterogeneity", field: ThresholdField::Lai, applicable: LAND,
        gui: CriterionGuiSpec { label: "LAI", help: "Refine where leaf-area-index variability exceeds the threshold", unit: "m2/m2", range: (0.0, 10.0), default: 1.0 } },
    CriterionSpec { id: "slope", display_name: "Terrain slope", physical_process: "orographic / runoff routing", field: ThresholdField::Slope, applicable: LAND,
        gui: CriterionGuiSpec { label: "Slope", help: "Refine where mean terrain slope exceeds the threshold", unit: "deg", range: (0.0, 45.0), default: 5.0 } },
    CriterionSpec { id: "k_s", display_name: "Saturated conductivity", physical_process: "soil hydraulics", field: ThresholdField::Ks, applicable: LAND,
        gui: CriterionGuiSpec { label: "k_s", help: "Refine on saturated hydraulic conductivity heterogeneity", unit: "mm/s", range: (0.0, 1.0), default: 0.01 } },
    CriterionSpec { id: "k_solids", display_name: "Solids conductivity", physical_process: "soil thermal", field: ThresholdField::KSolids, applicable: LAND,
        gui: CriterionGuiSpec { label: "k_solids", help: "Refine on soil-solids thermal conductivity heterogeneity", unit: "W/m/K", range: (0.0, 10.0), default: 2.0 } },
    CriterionSpec { id: "tkdry", display_name: "Dry thermal cond.", physical_process: "soil thermal", field: ThresholdField::Tkdry, applicable: LAND,
        gui: CriterionGuiSpec { label: "tkdry", help: "Refine on dry-soil thermal conductivity heterogeneity", unit: "W/m/K", range: (0.0, 1.0), default: 0.2 } },
    CriterionSpec { id: "tksatf", display_name: "Frozen sat. thermal", physical_process: "soil thermal (frozen)", field: ThresholdField::Tksatf, applicable: LAND,
        gui: CriterionGuiSpec { label: "tksatf", help: "Refine on frozen saturated thermal conductivity", unit: "W/m/K", range: (0.0, 5.0), default: 2.0 } },
    CriterionSpec { id: "tksatu", display_name: "Unfrozen sat. thermal", physical_process: "soil thermal (unfrozen)", field: ThresholdField::Tksatu, applicable: LAND,
        gui: CriterionGuiSpec { label: "tksatu", help: "Refine on unfrozen saturated thermal conductivity", unit: "W/m/K", range: (0.0, 5.0), default: 1.5 } },
    CriterionSpec { id: "sst", display_name: "SST front", physical_process: "ocean surface temperature front", field: ThresholdField::Sst, applicable: OCEAN,
        gui: CriterionGuiSpec { label: "SST", help: "Refine across sea-surface-temperature gradients", unit: "degC", range: (0.0, 5.0), default: 1.0 } },
    CriterionSpec { id: "ssh", display_name: "SSH gradient", physical_process: "ocean dynamic height", field: ThresholdField::Ssh, applicable: OCEAN,
        gui: CriterionGuiSpec { label: "SSH", help: "Refine across sea-surface-height gradients", unit: "m", range: (0.0, 2.0), default: 0.2 } },
    CriterionSpec { id: "eke", display_name: "Eddy kinetic energy", physical_process: "mesoscale eddies", field: ThresholdField::Eke, applicable: OCEAN,
        gui: CriterionGuiSpec { label: "EKE", help: "Refine in high eddy-kinetic-energy regions", unit: "cm2/s2", range: (0.0, 1000.0), default: 100.0 } },
    CriterionSpec { id: "sea_slope", display_name: "Seafloor slope", physical_process: "bathymetric gradient / shelf break", field: ThresholdField::SeaSlope, applicable: OCEAN,
        gui: CriterionGuiSpec { label: "Sea slope", help: "Refine across steep seafloor slopes / shelf breaks", unit: "deg", range: (0.0, 30.0), default: 3.0 } },
    CriterionSpec { id: "typhoon", display_name: "TC track density", physical_process: "tropical-cyclone climatology", field: ThresholdField::Typhoon, applicable: ATMOS,
        gui: CriterionGuiSpec { label: "Typhoon", help: "Refine along high tropical-cyclone track density", unit: "count", range: (0.0, 100.0), default: 10.0 } },
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
    CATALOG.iter().filter(|c| c.applicable.contains(&kind)).collect()
}

// ----------------------------- tests -----------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProjectConfig {
        ProjectConfig {
            schema_version: "3.0.0".into(),
            metadata: ProjectMetadata {
                name: "gba".into(),
                authors: vec!["SYSU".into()],
                description: "GBA coupled mesh".into(),
            },
            domain: DomainConfig::Regional {
                shape: RegionShape::Bbox {
                    w: 112.0,
                    e: 115.5,
                    n: 23.5,
                    s: 21.5,
                },
                sea_ratio: Some(0.5),
            },
            target: MeshTargetConfig {
                kind: MeshDomainKind::Coupled,
                cell: MeshCellKind::Hex,
                intent: MeshIntentPreset::MeritHydroCoast,
                resolution: ResolutionSpec::Nxp(40),
                model_format: ModelFormat::CoLM,
            },
            data_layers: vec![
                ProjectDataLayer {
                    id: "lc".into(),
                    role: ProjectLayerRole::LandType,
                    path: "./in/landtype.nc".into(),
                    enabled: true,
                },
                ProjectDataLayer {
                    id: "lai".into(),
                    role: ProjectLayerRole::Threshold(ThresholdField::Lai),
                    path: "./th/lai.nc".into(),
                    enabled: true,
                },
            ],
            refinement: RefinementRecipe {
                enabled: true,
                max_passes: 3,
            },
            quality: QualityConfig {
                min_angle_deg: 28.0,
                on_violation: ViolationPolicy::Block,
            },
            expert: ExpertOverrides {
                nxp: None,
                openmp: Some(8),
            },
            hydro_coast: None,
            coupling: None,
        }
    }

    #[test]
    fn json_round_trips() {
        let p = sample();
        let s = p.to_json().expect("to json");
        let back = ProjectConfig::from_json(&s).expect("from json");
        assert_eq!(p, back);
    }

    #[test]
    fn yaml_round_trips() {
        let p = sample();
        let s = p.to_yaml().expect("to yaml");
        let back = ProjectConfig::from_yaml(&s).expect("from yaml");
        assert_eq!(p, back);
    }

    #[test]
    fn lower_maps_to_engine_config() {
        let lowered = sample().lower();
        assert_eq!(lowered.mkgrd.mesh_type, "LOCmesh");
        assert_eq!(lowered.mkgrd.mode_grid, "hex");
        assert_eq!(lowered.mkgrd.output_format, "CoLM");
        assert_eq!(lowered.mkgrd.nxp, 40);
        assert!(!lowered.mkgrd.mask_domain_global);
        assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
        assert_eq!(lowered.mkgrd.experiment_name, "gba");
        assert_eq!(lowered.mkgrd.openmp, 8); // expert override

        // landcover → landtype_file; lai → refine switch + refine_cal
        assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
        assert!(lowered.refine.refine_onelayer_lnd[0] && lowered.refine.refine_onelayer_lnd[1]);
        assert!(lowered.refine.refine_cal);

        // quality
        assert_eq!(lowered.quality.min_angle_warn_deg, 28.0);
        assert_eq!(lowered.quality.on_violation, "block");

        // runnable namelist with all four blocks
        let nml = lowered.to_namelist();
        assert!(nml.contains("&mkgrd"));
        assert!(nml.contains("&mkrefine"));
        assert!(nml.contains("&quality"));
        assert!(nml.contains("&datalayers"));
    }

    #[test]
    fn hex_mesh_lowers_with_is_transition_and_one_spring() {
        // The engine rejects hex meshes unless Istransition=true with exactly one
        // spring type > 0 (core validate_like_read_nl).
        let lowered = sample().lower();
        assert_eq!(lowered.mkgrd.mode_grid, "hex");
        assert!(lowered.refine.is_transition, "hex needs is_transition=true");
        let g = lowered.refine.spring_global_type > 0;
        let r = lowered.refine.spring_regional_type > 0;
        assert!(
            g ^ r,
            "exactly one spring type > 0, got global={} regional={}",
            lowered.refine.spring_global_type,
            lowered.refine.spring_regional_type
        );
    }

    #[test]
    fn baseline_grid_without_refinement_omits_mkrefine() {
        // A baseline grid (refine off) must omit &mkrefine — the engine would
        // validate it and reject the hex+no-Istransition / no-refine_spc combo.
        let mut lowered = sample().lower();
        lowered.mkgrd.refine = false;
        let nml = lowered.to_namelist();
        assert!(nml.contains("&mkgrd"), "{nml}");
        assert!(
            !nml.contains("&mkrefine"),
            "baseline grid must omit &mkrefine:\n{nml}"
        );
    }

    #[test]
    fn preset_defaults_pick_sensible_criteria() {
        let h = MeshIntentPreset::HydrologyLand.defaults();
        assert_eq!(h.kind, MeshDomainKind::Land);
        assert!(h.criteria.contains(&ThresholdField::Slope));
        assert!(h.extra_roles.contains(&ProjectLayerRole::MeritHydro));

        let a = MeshIntentPreset::AtmosphereTyphoonPrecip.defaults();
        assert_eq!(a.kind, MeshDomainKind::Atmosphere);
        assert_eq!(a.model_format, ModelFormat::Mpas);
        assert!(a.criteria.contains(&ThresholdField::Typhoon));

        let c = MeshIntentPreset::CoastalOcean.defaults();
        assert_eq!(c.kind, MeshDomainKind::Ocean);
        assert_eq!(c.cell, MeshCellKind::Tri);
    }

    #[test]
    fn scaffold_builds_lowerable_project() {
        let p = ProjectConfig::scaffold(
            "hydro_test",
            MeshIntentPreset::HydrologyLand,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        assert_eq!(p.target.kind, MeshDomainKind::Land);
        assert_eq!(p.target.intent, MeshIntentPreset::HydrologyLand);
        // landcover + merit + slope stubs (disabled, no path yet)
        assert!(p.data_layers.iter().any(|l| l.id == "landcover"));
        assert!(p.data_layers.iter().any(|l| l.id == "slope_avg"));
        assert!(p.data_layers.iter().all(|l| !l.enabled));

        // round-trips through yaml, and lowers to engine config
        let back = ProjectConfig::from_yaml(&p.to_yaml().expect("yaml")).expect("from yaml");
        assert_eq!(p, back);
        assert_eq!(p.lower().mkgrd.mesh_type, "landmesh");
    }

    #[test]
    fn criterion_catalog_is_complete_and_self_describing() {
        let cat = criterion_catalog();
        assert_eq!(cat.len(), 12, "one criterion per engine threshold field");
        for c in cat {
            assert!(
                !c.physical_process.is_empty(),
                "{} missing physical_process",
                c.id
            );
            assert!(!c.gui.label.is_empty());
            assert!(c.gui.range.0 <= c.gui.range.1);
        }
        let mut ids: Vec<&str> = cat.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 12, "criterion ids are unique");
    }

    #[test]
    fn criterion_lookup_and_domain_filter() {
        let slope = criterion_by_id("slope").expect("slope criterion");
        assert_eq!(slope.field, ThresholdField::Slope);
        assert!(criterion_by_id("nope").is_none());

        let ocean = criteria_for_domain(MeshDomainKind::Ocean);
        assert!(ocean.iter().any(|c| c.id == "sea_slope"));
        assert!(ocean.iter().all(|c| c.id != "lai")); // land-only excluded

        let layer = slope.to_data_layer("./th/slope_avg.nc", true);
        assert_eq!(
            layer.role,
            ProjectLayerRole::Threshold(ThresholdField::Slope)
        );
        assert!(layer.enabled);
    }

    #[test]
    fn km_resolution_and_hydro_coupling_round_trip() {
        assert_eq!(km_to_nxp(9.0), 40); // anchored on the GUI default
        assert_eq!(km_to_nxp(0.0), 1); // guard
        assert!(km_to_nxp(60.0) >= 1 && km_to_nxp(60.0) < 40);

        let mut p = sample();
        p.target.resolution = ResolutionSpec::ApproxKm(9.0);
        p.hydro_coast = Some(HydroCoastConfig {
            merit_root: "/data/merit".into(),
            cama_root: Some("/data/cama".into()),
            r3_width_m: 300.0,
            r2_width_m: 50.0,
        });
        p.coupling = Some(CoupledMeshConfig {
            fraction_method: FractionMethod::ConservativeOverlay,
            identify_coastline: true,
            identify_river_mouth: true,
        });

        let back = ProjectConfig::from_yaml(&p.to_yaml().expect("yaml")).expect("from yaml");
        assert_eq!(p, back);
        assert_eq!(p.lower().mkgrd.nxp, 40); // ApproxKm(9) → NXP 40
    }

    #[test]
    fn reproducibility_manifest_hashes_inputs() {
        let dir = std::env::temp_dir().join(format!("em_repro_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let f = dir.join("lai.nc");
        std::fs::write(&f, b"hello").expect("write input");

        let mut p = sample();
        for l in p.data_layers.iter_mut() {
            if l.id == "lai" {
                l.path = f.display().to_string();
                l.enabled = true;
            }
        }
        let m = p.reproducibility_manifest();
        assert_eq!(m.schema_version, "3.0.0");
        assert!(!m.tool_version.is_empty());
        assert!(m.lowered_namelist.contains("&mkgrd"));

        let lai = m
            .inputs
            .iter()
            .find(|i| i.path.ends_with("lai.nc"))
            .expect("lai input hashed");
        assert_eq!(lai.bytes, 5);
        // sha256("hello")
        assert_eq!(
            lai.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert!(m.to_json().expect("json").contains("sha256"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
