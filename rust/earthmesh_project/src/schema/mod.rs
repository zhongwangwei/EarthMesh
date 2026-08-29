use earthmesh_core::{
    EarthmeshConfig, DEFAULT_MIN_ANGLE_WARN_DEG, EARTH_RADIUS_METERS, KM_PER_DEGREE_EQUATOR,
};
use serde::{Deserialize, Serialize};

use crate::CloseBoundaryMode;

pub const DEFAULT_MIN_ANGLE_DEG: f64 = DEFAULT_MIN_ANGLE_WARN_DEG;
pub const DEFAULT_AUTO_REFINE_BATCH_CELLS: usize = 1;
pub const PROJECT_SCHEMA_VERSION: &str = "3.0.0";
pub const METHOD_C_MIN_BASE_NXP: i32 = 10;
pub const METHOD_C_MAX_AUTO_REFINE_LEVEL: u8 = 5;
pub const METHOD_C_SPRING_NXP1_KM: f64 =
    std::f64::consts::PI * 2.0 * (EARTH_RADIUS_METERS / 1000.0) / 5.0;

pub fn default_mask_sea_ratio() -> f64 {
    EarthmeshConfig::default().mask_sea_ratio
}

// ----------------------------- top level -----------------------------

/// One project = one reproducible mesh production.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Optional MERIT-Hydro/CaMa closed-loop workflow. Hydro target levels are
    /// applied through the shared HField/Method-C engine and all delivery,
    /// coupling, and quality artifacts are recomputed on the final mesh.
    #[serde(default)]
    pub hydro_coast: Option<HydroCoastConfig>,
    /// Land-ocean coupling options.
    #[serde(default)]
    pub coupling: Option<CoupledMeshConfig>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectMetadata {
    pub name: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: String,
}

// ----------------------------- domain -----------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum DomainConfig {
    Global,
    Regional {
        shape: RegionShape,
        #[serde(default)]
        sea_ratio: Option<f64>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum RegionShape {
    Bbox {
        w: f64,
        e: f64,
        n: f64,
        s: f64,
    },
    Circle {
        lon: f64,
        lat: f64,
        radius_km: f64,
    },
    Shapefile {
        path: String,
    },
    Close {
        path: String,
        format: CloseMaskFormat,
        #[serde(default)]
        boundary: CloseBoundaryMode,
    },
}

impl ProjectConfig {
    /// Euler expectation that is safe before inspecting the final mask topology.
    ///
    /// Only an unmasked global Earth/atmosphere mesh is known a priori to be a
    /// closed sphere. Land, ocean, coupled, and every regional result can gain
    /// boundaries, holes, or multiple components from masking, so they remain
    /// infer-only until final boundary cycles are available.
    pub fn expected_euler_characteristic(&self) -> Option<isize> {
        match (&self.domain, self.target.kind) {
            (DomainConfig::Global, MeshDomainKind::Earth | MeshDomainKind::Atmosphere) => Some(2),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseMaskFormat {
    PolygonShp,
    Nml,
    Netcdf,
    LonLatText,
}

// ----------------------------- target -----------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// The algorithm that turns a refinement request into a finer mesh.
///
/// The two differ in what they do with a request they cannot take as given.
///
/// `MethodC` subdivides a closed region and surrounds it with transition rows
/// (Walko & Avissar 2011). It holds vertex degree to {5, 6, 7}, which is what
/// keeps the hexagonal dual usable -- and it refuses a region it cannot build:
/// its seed lattice steps three cells at a time and its perimeter must be a
/// multiple of three. Named regions are shapes it can build; a region whose
/// shape comes from the data is not, which is why criteria-driven refinement is
/// suspended on it.
///
/// `RedGreen` splits marked triangles into four and closes the seams by halving
/// the neighbours left hanging. Its judge chain *grows* a marking it cannot take
/// as given and never rejects a shape, so an arbitrary coastal region refines.
/// The price is that vertex degree is only bounded by the Lawson flips
/// afterwards; a model reading the triangles directly, as FVCOM does, does not
/// care.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefinementBackend {
    /// Nested regions with transition rows. The default, and the only backend
    /// that reads an h-field -- but criteria-driven refinement is suspended on
    /// it: a region whose shape came from the data is refused rather than
    /// approximated, so `refinement.adaptive` with a criterion enabled fails.
    #[default]
    MethodC,
    /// Split any marked triangle into four and close the seams. Serves named
    /// regions *and* the point+radius criteria, because its judge chain grows a
    /// marking it cannot take as given and never rejects a shape. This is the
    /// backend for a coastline.
    RedGreen,
    /// Re-read the criteria against the cells that exist and change the mesh
    /// locally where they are still unmet.
    ///
    /// Where Method-C and Red-Green turn demand into geometry once and fit a mesh to it,
    /// this measures each Voronoi cell every cycle, so a cell that has become
    /// fine enough stops asking. It refuses any transaction that would leave a
    /// thin triangle, which is what carries its worst angle past Method-C's on
    /// the same request, and it reports what it could not do rather than
    /// quietly serving less.
    ///
    /// It serves named circular regions. A region with a shape -- a bbox, a
    /// polygon -- is refused rather than approximated.
    HarpDv,
    /// Certified Mother-grid Reverse Coarsening (CMRC). Builds a safe global
    /// mother grid and only accepts changes that retain every hard certificate.
    Certified,
}

impl RefinementBackend {
    pub fn owns_quality_repair(self) -> bool {
        matches!(self, Self::HarpDv | Self::Certified)
    }
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

/// Friendly km/degree, or an explicit engine NXP.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ResolutionSpec {
    ApproxKm(f64),
    ApproxDegree(f64),
    Nxp(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFormat {
    CoLM,
    Icon,
    Mpas,
    MpasOcean,
    MpasSimple,
    Fvcom,
}

// ----------------------------- data layers -----------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDataLayer {
    pub id: String,
    pub role: ProjectLayerRole,
    pub path: String,
    #[serde(default)]
    pub enabled: bool,
    /// Optional per-criterion threshold. When absent, the criterion catalog
    /// default is used during lowering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold_value: Option<f64>,
}

/// Optional mean/std criterion override for one continuous threshold data source.
///
/// Continuous criterion ids are the source field stem plus `_mean` or `_std`
/// (for example `lai_mean`, `lai_std`, `k_s_mean`). The categorical LandType
/// criterion uses the single id `landcover`. Source paths remain owned by the
/// matching [`ProjectDataLayer`]. Omitted continuous entries retain legacy
/// mean+std behavior; omitted `landcover` is disabled unless the legacy
/// LandType layer explicitly supplies `threshold_value`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdCriterionConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// Serde-friendly mirror of [`earthmesh_core::DataLayerRole`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLayerRole {
    LandType,
    Threshold(ThresholdField),
    MeritHydro,
    Cama,
}

/// Serde-friendly mirror of [`earthmesh_core::ThresholdVar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThresholdField {
    Lai,
    Slope,
    Dem,
    SlopeMax,
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
#[serde(deny_unknown_fields)]
pub struct RefinementRecipe {
    #[serde(default)]
    pub enabled: bool,
    /// Master switch for calculated refinement from threshold/landcover data.
    /// Data layers remain available to mesh output when this is disabled.
    #[serde(default)]
    pub threshold_enabled: bool,
    #[serde(default)]
    pub max_passes: u8,
    /// Independent mean/std criteria for continuous threshold sources. The
    /// source NetCDF path remains declared once in `data_layers`; these entries
    /// only override statistic-specific enable/value settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threshold_criteria: Vec<ThresholdCriterionConfig>,
    #[serde(default)]
    pub specified_circle: Option<SpecifiedCircleRefinements>,
    #[serde(default)]
    pub specified_bbox: Option<SpecifiedBboxRefinement>,
    #[serde(default)]
    pub specified_close: Option<SpecifiedCloseRefinement>,
    /// Which refinement algorithm builds the mesh.
    #[serde(default)]
    pub backend: RefinementBackend,
    /// Method-C's internal algorithm. `Canonical` preserves the Walko/Avissar
    /// lattice route; `LeppDelaunay` keeps Method-C's demand semantics but lets
    /// LEPP-Delaunay perform the local refinement.
    #[serde(default)]
    pub method_c: MethodCRefinementRecipe,
    /// HARP-DV cycle budgets, candidate spacing, and transaction gates.
    #[serde(default)]
    pub harp_dv: HarpDvRefinementRecipe,
    /// CMRC safe-mother, certification, and search budgets.
    #[serde(default)]
    pub certified: CertifiedRefinementRecipe,
    /// Default refinement backend: ask every enabled criterion again before
    /// each pass and cover what it demands with circles (emits the `&adaptive`
    /// namelist group). Absent means enabled.
    #[serde(default)]
    pub adaptive: Option<AdaptiveRefinementRecipe>,
    /// Red-Green peer backend: compose regions into a gradient-limited
    /// cell-width field and drive Method-C from quantized target levels
    /// (emits the `&hfield` namelist group). Enabling it explicitly turns the
    /// adaptive route off, because a run refines one way or the other.
    #[serde(default)]
    pub hfield: Option<HfieldRefinementRecipe>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertifiedMode {
    #[default]
    SafeMotherOnly,
    ReverseCoarsening,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CertifiedDeliveryMode {
    Tri,
    Hex,
    #[default]
    Coupled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertifiedRefinementRecipe {
    #[serde(default)]
    pub mode: CertifiedMode,
    #[serde(default)]
    pub delivery: CertifiedDeliveryMode,
    #[serde(default = "default_certified_maximum_level")]
    pub maximum_level: u8,
    #[serde(default = "default_certified_maximum_cells")]
    pub maximum_cells: usize,
    #[serde(default = "default_certified_gradation_rings_per_level")]
    pub gradation_rings_per_level: u8,
    #[serde(default = "default_certified_search_budget")]
    pub search_budget: usize,
}

impl Default for CertifiedRefinementRecipe {
    fn default() -> Self {
        Self {
            mode: CertifiedMode::SafeMotherOnly,
            delivery: CertifiedDeliveryMode::Coupled,
            maximum_level: default_certified_maximum_level(),
            maximum_cells: default_certified_maximum_cells(),
            gradation_rings_per_level: default_certified_gradation_rings_per_level(),
            search_budget: default_certified_search_budget(),
        }
    }
}

fn default_certified_maximum_level() -> u8 {
    8
}

fn default_certified_maximum_cells() -> usize {
    10_000_000
}

fn default_certified_gradation_rings_per_level() -> u8 {
    3
}

fn default_certified_search_budget() -> usize {
    100_000
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodCAlgorithm {
    #[default]
    Canonical,
    LeppDelaunay,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodCRefinementRecipe {
    #[serde(default)]
    pub algorithm: MethodCAlgorithm,
    #[serde(default = "default_method_c_lepp_max_cycles")]
    pub max_cycles: usize,
    #[serde(default = "default_method_c_lepp_target_size_tolerance")]
    pub target_size_tolerance: f64,
    #[serde(default = "default_method_c_lepp_maximum_neighbor_size_ratio")]
    pub maximum_neighbor_size_ratio: f64,
    #[serde(default = "default_method_c_lepp_maximum_vertices")]
    pub maximum_vertices: usize,
    #[serde(default = "default_method_c_lepp_maximum_insertions_per_cycle")]
    pub maximum_insertions_per_cycle: usize,
    #[serde(default = "default_method_c_lepp_maximum_path_length")]
    pub maximum_path_length: usize,
    #[serde(default = "default_method_c_lepp_stop_at_source_resolution")]
    pub stop_at_source_resolution: bool,
    #[serde(default = "default_method_c_lepp_minimum_triangle_angle_deg")]
    pub minimum_triangle_angle_deg: f64,
}

impl Default for MethodCRefinementRecipe {
    fn default() -> Self {
        Self {
            algorithm: MethodCAlgorithm::Canonical,
            max_cycles: default_method_c_lepp_max_cycles(),
            target_size_tolerance: default_method_c_lepp_target_size_tolerance(),
            maximum_neighbor_size_ratio: default_method_c_lepp_maximum_neighbor_size_ratio(),
            maximum_vertices: default_method_c_lepp_maximum_vertices(),
            maximum_insertions_per_cycle: default_method_c_lepp_maximum_insertions_per_cycle(),
            maximum_path_length: default_method_c_lepp_maximum_path_length(),
            stop_at_source_resolution:
                earthmesh_core::DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION,
            minimum_triangle_angle_deg: default_method_c_lepp_minimum_triangle_angle_deg(),
        }
    }
}

fn default_method_c_lepp_max_cycles() -> usize {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_MAX_CYCLES
}

fn default_method_c_lepp_target_size_tolerance() -> f64 {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE
}

fn default_method_c_lepp_maximum_neighbor_size_ratio() -> f64 {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO
}

fn default_method_c_lepp_maximum_vertices() -> usize {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES
}

fn default_method_c_lepp_maximum_insertions_per_cycle() -> usize {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE
}

fn default_method_c_lepp_maximum_path_length() -> usize {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH
}

fn default_method_c_lepp_minimum_triangle_angle_deg() -> f64 {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES
}

fn default_method_c_lepp_stop_at_source_resolution() -> bool {
    earthmesh_core::DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarpDvRefinementRecipe {
    #[serde(default = "default_harp_dv_max_cycles")]
    pub max_cycles: u32,
    #[serde(default = "default_harp_dv_minimum_cell_width_m")]
    pub minimum_cell_width_m: f64,
    #[serde(default = "default_harp_dv_maximum_cells")]
    pub maximum_cells: usize,
    #[serde(default = "default_harp_dv_maximum_patch_cells")]
    pub maximum_patch_cells: usize,
    #[serde(default = "default_harp_dv_maximum_neighbor_scale_ratio")]
    pub maximum_neighbor_scale_ratio: f64,
    #[serde(default = "default_harp_dv_minimum_candidate_separation_m")]
    pub minimum_candidate_separation_m: f64,
    #[serde(default = "default_harp_dv_maximum_vertex_degree")]
    pub maximum_vertex_degree: usize,
    #[serde(default = "default_harp_dv_minimum_triangle_angle_deg")]
    pub minimum_triangle_angle_deg: f64,
    /// Optional Ruppert-style angle demand. Zero disables it; the independent
    /// transaction gate above remains active.
    #[serde(default = "default_harp_dv_criterion_minimum_angle_deg")]
    pub criterion_minimum_angle_deg: f64,
}

impl Default for HarpDvRefinementRecipe {
    fn default() -> Self {
        Self {
            max_cycles: default_harp_dv_max_cycles(),
            minimum_cell_width_m: default_harp_dv_minimum_cell_width_m(),
            maximum_cells: default_harp_dv_maximum_cells(),
            maximum_patch_cells: default_harp_dv_maximum_patch_cells(),
            maximum_neighbor_scale_ratio: default_harp_dv_maximum_neighbor_scale_ratio(),
            minimum_candidate_separation_m: default_harp_dv_minimum_candidate_separation_m(),
            maximum_vertex_degree: default_harp_dv_maximum_vertex_degree(),
            minimum_triangle_angle_deg: default_harp_dv_minimum_triangle_angle_deg(),
            criterion_minimum_angle_deg: default_harp_dv_criterion_minimum_angle_deg(),
        }
    }
}

fn default_harp_dv_max_cycles() -> u32 {
    earthmesh_core::DEFAULT_HARP_DV_MAX_CYCLES
}

fn default_harp_dv_minimum_cell_width_m() -> f64 {
    earthmesh_core::DEFAULT_HARP_DV_MINIMUM_CELL_WIDTH_M
}

fn default_harp_dv_maximum_cells() -> usize {
    earthmesh_core::DEFAULT_HARP_DV_MAXIMUM_CELLS
}

fn default_harp_dv_maximum_patch_cells() -> usize {
    earthmesh_core::DEFAULT_HARP_DV_MAXIMUM_PATCH_CELLS
}

fn default_harp_dv_maximum_neighbor_scale_ratio() -> f64 {
    earthmesh_core::DEFAULT_HARP_DV_MAXIMUM_NEIGHBOR_SCALE_RATIO
}

fn default_harp_dv_minimum_candidate_separation_m() -> f64 {
    earthmesh_core::DEFAULT_HARP_DV_MINIMUM_CANDIDATE_SEPARATION_M
}

fn default_harp_dv_maximum_vertex_degree() -> usize {
    earthmesh_core::DEFAULT_HARP_DV_MAXIMUM_VERTEX_DEGREE
}

fn default_harp_dv_minimum_triangle_angle_deg() -> f64 {
    earthmesh_core::DEFAULT_HARP_DV_MINIMUM_TRIANGLE_ANGLE_DEG
}

fn default_harp_dv_criterion_minimum_angle_deg() -> f64 {
    earthmesh_core::DEFAULT_HARP_DV_CRITERION_MINIMUM_ANGLE_DEG
}

/// Point+radius refinement, re-planned before every pass.
///
/// A cell's need for further refinement is a question about that cell, so it
/// cannot be settled before the cell exists. This backend asks the criteria
/// again at each level, over a neighbourhood the size of the cell that level
/// refines away, and covers what they demand with circles. The h-field settles
/// everything up front instead, which is why a resolution-dependent criterion
/// (land-cover heterogeneity) can only be honoured here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdaptiveRefinementRecipe {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 0 = follow the run's max refinement level.
    #[serde(default)]
    pub max_level: u8,
    /// Base cell size in meters. `None` = 2piR/(5*NXP).
    #[serde(default)]
    pub base_m: Option<f64>,
    /// Chase the land/sea boundary as its own criterion.
    #[serde(default = "default_true")]
    pub coastline: bool,
}

impl Default for AdaptiveRefinementRecipe {
    fn default() -> Self {
        Self {
            enabled: true,
            max_level: 0,
            base_m: None,
            coastline: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HfieldRefinementRecipe {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_hfield_g")]
    pub g: f64,
    /// 0 = follow the run's max refinement level.
    #[serde(default)]
    pub max_level: u8,
    #[serde(default)]
    pub base_m: Option<f64>,
    /// Geographic origin used to sample lon/lat threshold rasters from a
    /// native Cartesian-XY mesh. Both values must be set together.
    #[serde(default)]
    pub origin_lon: Option<f64>,
    #[serde(default)]
    pub origin_lat: Option<f64>,
    /// Field raster size. `None` derives it from the target resolution so the
    /// raster resolves the finest cell the run asks for.
    ///
    /// The h-field is sampled per triangle centre and per edge midpoint, so a
    /// raster coarser than the target cell aliases the level map and hands
    /// Method-C a ragged selection. `earthmesh_hfield` states the requirement as
    /// "spacing <= the local h (ideally half)"; the fixed 720x360 default only
    /// satisfies it down to roughly 100 km cells.
    #[serde(default)]
    pub nlon: Option<usize>,
    #[serde(default)]
    pub nlat: Option<usize>,
}

impl Default for HfieldRefinementRecipe {
    fn default() -> Self {
        Self {
            enabled: true,
            g: default_hfield_g(),
            max_level: 0,
            base_m: None,
            origin_lon: None,
            origin_lat: None,
            nlon: None,
            nlat: None,
        }
    }
}

fn default_hfield_g() -> f64 {
    0.2
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecifiedCircleRefinement {
    pub lon: f64,
    pub lat: f64,
    pub radius_km: f64,
}

/// One circle or a chain of them.
///
/// A coastline is not one circle. Reducing a land/sea boundary to point+radius
/// demand yields tens to hundreds of them (19 at one-degree sampling over the
/// South China Sea, 115 at a quarter degree), and until this accepted a list the
/// only way a project could express distributed refinement was the h-field —
/// which is very likely why that became the default backend.
///
/// Existing files keep working: a single mapping still parses, and `null` still
/// means no specified circle refinement.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SpecifiedCircleRefinements {
    One(SpecifiedCircleRefinement),
    Many(Vec<SpecifiedCircleRefinement>),
}

/// Deserialized by hand rather than with `#[serde(untagged)]`.
///
/// Untagged buffers the input and reports "data did not match any variant" when
/// every variant fails, which would turn a typo like `lonn:` into a message that
/// names neither the bad key nor the good ones. Dispatching on map-vs-sequence
/// first means the circle's own `unknown field` error survives intact.
impl<'de> Deserialize<'de> for SpecifiedCircleRefinements {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CirclesVisitor;

        impl<'de> serde::de::Visitor<'de> for CirclesVisitor {
            type Value = SpecifiedCircleRefinements;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a circle mapping or a list of circle mappings")
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                SpecifiedCircleRefinement::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
                .map(SpecifiedCircleRefinements::One)
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                Vec::<SpecifiedCircleRefinement>::deserialize(
                    serde::de::value::SeqAccessDeserializer::new(seq),
                )
                .map(SpecifiedCircleRefinements::Many)
            }
        }

        deserializer.deserialize_any(CirclesVisitor)
    }
}

impl SpecifiedCircleRefinements {
    pub fn as_slice(&self) -> &[SpecifiedCircleRefinement] {
        match self {
            Self::One(circle) => std::slice::from_ref(circle),
            Self::Many(circles) => circles,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecifiedBboxRefinement {
    pub w: f64,
    pub e: f64,
    pub s: f64,
    pub n: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecifiedCloseRefinement {
    pub path: String,
    #[serde(default)]
    pub boundary: CloseBoundaryMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityConfig {
    pub min_angle_deg: f64,
    /// Maximum connected defect cells changed by one AutoRefine pass.
    #[serde(default = "default_auto_refine_batch_cells")]
    pub auto_refine_batch_cells: usize,
    #[serde(default = "default_violation_policy")]
    pub on_violation: ViolationPolicy,
    /// Optional LEPP-Delaunay repair of the completed canonical Method-C mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lepp_post_quality: Option<LeppPostQualityConfig>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeppPostQualityConfig {
    #[serde(default = "default_lepp_post_quality_max_insertions")]
    pub maximum_insertions: usize,
    /// Optional absolute great-circle edge target in kilometres.
    #[serde(default)]
    pub maximum_edge_km: Option<f64>,
}

fn default_lepp_post_quality_max_insertions() -> usize {
    50
}

fn default_auto_refine_batch_cells() -> usize {
    DEFAULT_AUTO_REFINE_BATCH_CELLS
}

fn default_violation_policy() -> ViolationPolicy {
    ViolationPolicy::AutoRefine
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            min_angle_deg: DEFAULT_MIN_ANGLE_DEG,
            auto_refine_batch_cells: DEFAULT_AUTO_REFINE_BATCH_CELLS,
            on_violation: ViolationPolicy::AutoRefine,
            lepp_post_quality: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationPolicy {
    #[serde(alias = "warn")]
    Warn,
    #[serde(alias = "block")]
    Block,
    #[serde(alias = "auto_refine")]
    #[default]
    AutoRefine,
}

impl ViolationPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            ViolationPolicy::Block => "block",
            ViolationPolicy::AutoRefine => "auto_refine",
            ViolationPolicy::Warn => "warn",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpertOverrides {
    #[serde(default)]
    pub nxp: Option<i32>,
    #[serde(default)]
    pub openmp: Option<i32>,
    #[serde(default)]
    pub niter: Option<i32>,
    #[serde(default)]
    pub niter_refine: Option<i32>,
    #[serde(default)]
    pub max_iter_spc: Option<i32>,
    #[serde(default)]
    pub max_iter_cal: Option<i32>,
    #[serde(default)]
    pub halo: Option<Vec<i32>>,
    #[serde(default)]
    pub max_transition_row: Option<Vec<i32>>,
    #[serde(default)]
    pub set_dis_type: Option<String>,
    #[serde(default)]
    pub num_rc: Option<i32>,
    #[serde(default)]
    pub vertex_pretect_layers: Option<i32>,
    #[serde(default)]
    pub spring_global_type: Option<i32>,
    #[serde(default)]
    pub spring_regional_type: Option<i32>,
    #[serde(default)]
    pub beta: Option<f64>,
    #[serde(default)]
    pub relax: Option<f64>,
    #[serde(default)]
    pub weak_concav_eliminate: Option<bool>,
    /// Override the ocean carve's largest-connected-water-body cleanup.
    /// `None` keeps the derived default: on for `oceanmesh`, off otherwise.
    #[serde(default)]
    pub isolated_ocean: Option<bool>,
}

/// MERIT-Hydro inputs used by the post-mesh hydro workflow. Project execution
/// turns the generated gridfile into cell polygons, derives R2/R3 corridors,
/// and runs the shared hydro delivery/refinement workflow. Target levels are
/// converted into the production HField/Method-C input, followed by final-mesh
/// hydro and quality recomputation. When `cama_root` is present, the same stage
/// also exports the native CaMa reach inventory and river-mouth signals for the
/// Project footprint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HydroCoastConfig {
    pub merit_root: String,
    #[serde(default)]
    pub cama_root: Option<String>,
    /// Spatial sampling stride within each MERIT tile. Must remain 1 because
    /// physical river/coast adjacency cannot be reconstructed after sampling.
    #[serde(default = "default_merit_stride")]
    pub merit_stride: usize,
    #[serde(default = "default_r3_width")]
    pub r3_width_m: f64,
    #[serde(default = "default_r2_width")]
    pub r2_width_m: f64,
    #[serde(default = "default_r3_upa")]
    pub r3_upa_km2: f64,
    #[serde(default = "default_r2_upa")]
    pub r2_upa_km2: f64,
    /// River and coast remain available for delivery/coupling even when their
    /// HField refinement demand is disabled.
    #[serde(default = "default_true")]
    pub river_refinement_enabled: bool,
    #[serde(default = "default_true")]
    pub river_width_refinement_enabled: bool,
    #[serde(default = "default_true")]
    pub river_upstream_area_refinement_enabled: bool,
    /// Single user-facing river-width trigger. R2/R3 remain internal
    /// classification thresholds for coupling and map output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub river_width_threshold_m: Option<f64>,
    /// Single user-facing upstream-area trigger, independent of river width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub river_upstream_area_threshold_km2: Option<f64>,
    #[serde(default = "default_true")]
    pub coast_refinement_enabled: bool,
    /// Refinement-only distance band measured from the native MERIT coastline.
    /// A zero value keeps legacy projects on physical coastline cells only.
    #[serde(default)]
    pub coast_buffer_km: f64,
    #[serde(default = "default_true")]
    pub coast_land_refinement_enabled: bool,
    #[serde(default = "default_true")]
    pub coast_ocean_refinement_enabled: bool,
}

impl HydroCoastConfig {
    pub fn effective_river_width_threshold_m(&self) -> f64 {
        self.river_width_threshold_m.unwrap_or(self.r3_width_m)
    }

    pub fn effective_river_upstream_area_threshold_km2(&self) -> f64 {
        self.river_upstream_area_threshold_km2
            .unwrap_or(self.r3_upa_km2)
    }

    pub fn river_width_refinement_active(&self) -> bool {
        self.river_refinement_enabled && self.river_width_refinement_enabled
    }

    pub fn river_upstream_area_refinement_active(&self) -> bool {
        self.river_refinement_enabled && self.river_upstream_area_refinement_enabled
    }

    pub fn has_river_refinement(&self) -> bool {
        self.river_width_refinement_active() || self.river_upstream_area_refinement_active()
    }
}

fn default_r3_width() -> f64 {
    300.0
}

fn default_r2_width() -> f64 {
    50.0
}

fn default_r3_upa() -> f64 {
    50_000.0
}

fn default_r2_upa() -> f64 {
    5_000.0
}

fn default_merit_stride() -> usize {
    1
}

/// Land-ocean coupling config.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoupledMeshConfig {
    #[serde(default)]
    pub fraction_method: FractionMethod,
    #[serde(default)]
    pub identify_coastline: bool,
    #[serde(default)]
    pub identify_river_mouth: bool,
    /// CaMa root used only by coupled river-mouth identification.
    #[serde(default, alias = "river_mouth_cama_root")]
    pub cama_root: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FractionMethod {
    #[default]
    PointSample,
    ConservativeOverlay,
}

impl FractionMethod {
    pub fn engine_str(self) -> &'static str {
        match self {
            Self::PointSample => "point_sample",
            Self::ConservativeOverlay => "conservative_overlay",
        }
    }
}

/// Approximate Method-C global NXP from the spring target spacing `2*pi*R/(5*NXP)`.
pub fn km_to_nxp(km: f64) -> i32 {
    if km <= 0.0 {
        return 1;
    }
    (METHOD_C_SPRING_NXP1_KM / km).round().max(1.0) as i32
}

pub fn nxp_to_km(nxp: i32) -> f64 {
    if nxp <= 0 {
        return METHOD_C_SPRING_NXP1_KM;
    }
    METHOD_C_SPRING_NXP1_KM / f64::from(nxp)
}

pub fn degree_to_nxp(degrees_at_equator: f64) -> i32 {
    km_to_nxp(degrees_at_equator * KM_PER_DEGREE_EQUATOR)
}

pub fn auto_refine_level_cap(target_nxp: i32) -> u8 {
    let mut cap = 1u8;
    while cap < METHOD_C_MAX_AUTO_REFINE_LEVEL
        && METHOD_C_MIN_BASE_NXP.saturating_mul(1_i32 << (cap + 1)) <= target_nxp
    {
        cap += 1;
    }
    cap
}

pub fn next_auto_refine_pass(current: u8, target_nxp: i32) -> Option<u8> {
    let cap = auto_refine_level_cap(target_nxp);
    (current < cap).then_some(current + 1)
}

pub fn effective_auto_refine_pass(requested: u8, target_nxp: i32) -> u8 {
    requested.max(1).min(auto_refine_level_cap(target_nxp))
}
