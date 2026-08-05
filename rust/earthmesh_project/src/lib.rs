//! EarthMesh v3 project schema (the L1 "intent" layer).
//!
//! A serde-(de)serializable [`ProjectConfig`] (YAML/JSON) that **lowers** to the
//! engine's [`earthmesh_core::EarthmeshConfig`] + [`earthmesh_core::RefineConfig`]
//! (+ the `&quality` / `&datalayers` blocks), reusing the core lowering built in
//! `earthmesh_core`. This keeps the friendly project layer separate from the 64
//! flat engine fields.
//!
//! Owns the project schema, validation, intent presets, criteria catalog, and
//! lowering into engine namelists.

pub use earthmesh_core::KM_PER_DEGREE_EQUATOR;

mod capability_registry;
mod schema;
pub use capability_registry::{
    classify_project_capability, preset_capability_key, project_capability_registry,
    project_domain_classes, project_source_profiles, project_specified_sources,
    project_target_triples, threshold_topology_source_atom, ProjectCapability,
    ProjectCapabilityEntry, ProjectCapabilityKey, ProjectCloseBoundaryMode, ProjectContractId,
    ProjectCoordinateMode, ProjectDomainClass, ProjectOutputDelivery, ProjectParameterizedTestId,
    ProjectRejectionReason, ProjectSourceAtom, ProjectSourceProfile, ProjectSpecifiedSource,
    ProjectTargetTriple, ProjectValidationTestId, PROJECT_DOMAIN_CLASS_COUNT,
    PROJECT_RAW_CAPABILITY_KEY_COUNT, PROJECT_SOURCE_PROFILE_COUNT, PROJECT_TARGET_TRIPLE_COUNT,
    PROJECT_THRESHOLD_FIELDS,
};

pub use schema::{
    auto_refine_level_cap, default_mask_sea_ratio, degree_to_nxp, effective_auto_refine_pass,
    km_to_nxp, next_auto_refine_pass, nxp_to_km, AdaptiveRefinementRecipe, CloseMaskFormat,
    CoupledMeshConfig, DomainConfig, ExpertOverrides, FractionMethod, HfieldRefinementRecipe,
    HydroCoastConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset, MeshTargetConfig,
    ModelFormat, ProjectConfig, ProjectDataLayer, ProjectLayerRole, ProjectMetadata, QualityConfig,
    RefinementRecipe, RegionShape, ResolutionSpec, SpecifiedBboxRefinement,
    SpecifiedCircleRefinement, SpecifiedCircleRefinements, SpecifiedCloseRefinement,
    ThresholdCriterionConfig, ThresholdField, ViolationPolicy, DEFAULT_AUTO_REFINE_BATCH_CELLS,
    DEFAULT_MIN_ANGLE_DEG, INTENT_PRESETS, METHOD_C_MAX_AUTO_REFINE_LEVEL, METHOD_C_MIN_BASE_NXP,
    METHOD_C_SPRING_NXP1_KM, PROJECT_SCHEMA_VERSION,
};
mod auto_refine;
pub use auto_refine::{AutoRefineAction, AutoRefineEvent, AutoRefineState};
mod criteria;
pub use criteria::{
    criteria_for_domain, criterion_by_id, criterion_catalog, threshold_criterion_by_id,
    threshold_criterion_catalog, CriterionGuiSpec, CriterionSpec, EffectiveLandcoverCriterion,
    EffectiveThresholdCriterion, ThresholdCriterionSpec, ThresholdStatistic,
    DEFAULT_LANDCOVER_CLASS_THRESHOLD, LANDCOVER_CRITERION_ID,
};
mod close_boundary;
pub use close_boundary::{
    transform_close_boundary, CloseBoundaryGeometry, CloseBoundaryMode, CloseBoundaryReport,
    CloseBoundaryTransform,
};
mod close_source;
pub use close_source::{
    read_close_mask_nml_points, read_lonlat_text_points, read_shapefile_polygon_rings,
    write_close_mask_nml,
};
mod display;
mod engine_mapping;
mod geometry_ir;
pub use geometry_ir::{
    GeometryIr, GeometryPoint, GeometryPrimitive, GeometryRegion, GeometrySegment,
};
mod hydro_plan;
pub use hydro_plan::{project_hydro_output_dir, HydroExecutionPlan};
mod lowering;
pub use lowering::LoweredProject;
mod presets;
pub use presets::{PresetDefaults, DEPRECATED_ATMOSPHERE_TYPHOON_INTENT_ID};
mod stage_cache;
pub use stage_cache::{content_addressed_stage_key, StageCache};
mod validation;

// ----------------------------- tests -----------------------------

#[cfg(test)]
mod tests;
