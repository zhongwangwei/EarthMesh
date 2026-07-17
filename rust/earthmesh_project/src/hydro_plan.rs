use std::path::{Path, PathBuf};

use crate::{
    nxp_to_km, DomainConfig, ProjectConfig, RegionShape, ResolutionSpec,
    METHOD_C_MAX_AUTO_REFINE_LEVEL,
};
use earthmesh_core::KM_PER_DEGREE_EQUATOR;

/// Data-only plan for the post-mesh hydro workflow. Execution belongs to the CLI;
/// GUI callers invoke that same CLI stage instead of reimplementing it.
#[derive(Clone, Debug, PartialEq)]
pub struct HydroExecutionPlan {
    pub merit_root: String,
    pub cama_root: Option<String>,
    /// Exact Project footprint. File-backed shapes are resolved relative to the
    /// Project file by the execution layer, not the process working directory.
    pub domain: RegionShape,
    pub r2_width_m: f64,
    pub r3_width_m: f64,
    pub r2_upa_km2: f64,
    pub r3_upa_km2: f64,
    pub river_refinement_enabled: bool,
    pub river_width_refinement_enabled: bool,
    pub river_upstream_area_refinement_enabled: bool,
    pub river_width_threshold_m: f64,
    pub river_upstream_area_threshold_km2: f64,
    pub coast_refinement_enabled: bool,
    pub coast_buffer_km: f64,
    pub coast_land_refinement_enabled: bool,
    pub coast_ocean_refinement_enabled: bool,
    pub merit_stride: usize,
    pub target_dx_km: f64,
    pub include_classes: Vec<String>,
    pub max_level: u8,
}

/// Stable location for Project hydro artifacts, shared by CLI and GUI callers.
/// Keeping it relative to the final gridfile avoids run-directory policy drift.
pub fn project_hydro_output_dir(gridfile: impl AsRef<Path>) -> PathBuf {
    gridfile
        .as_ref()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("hydro_project")
}

impl ProjectConfig {
    pub fn hydro_execution_plan(&self) -> Result<Option<HydroExecutionPlan>, String> {
        let Some(hydro) = &self.hydro_coast else {
            return Ok(None);
        };
        let domain = match &self.domain {
            DomainConfig::Regional { shape, .. } => shape.clone(),
            _ => return Err("hydro_coast Project routing requires a regional domain".to_string()),
        };
        let target_dx_km = match self.target.resolution {
            ResolutionSpec::Nxp(nxp) => nxp_to_km(nxp),
            ResolutionSpec::ApproxKm(km) => km,
            ResolutionSpec::ApproxDegree(degrees) => degrees * KM_PER_DEGREE_EQUATOR,
        };
        Ok(Some(HydroExecutionPlan {
            merit_root: hydro.merit_root.clone(),
            cama_root: hydro.cama_root.clone(),
            domain,
            r2_width_m: hydro.r2_width_m,
            r3_width_m: hydro.r3_width_m,
            r2_upa_km2: hydro.r2_upa_km2,
            r3_upa_km2: hydro.r3_upa_km2,
            river_refinement_enabled: hydro.river_refinement_enabled,
            river_width_refinement_enabled: hydro.river_width_refinement_active(),
            river_upstream_area_refinement_enabled: hydro.river_upstream_area_refinement_active(),
            river_width_threshold_m: hydro.effective_river_width_threshold_m(),
            river_upstream_area_threshold_km2: hydro.effective_river_upstream_area_threshold_km2(),
            coast_refinement_enabled: hydro.coast_refinement_enabled,
            coast_buffer_km: hydro.coast_buffer_km,
            coast_land_refinement_enabled: hydro.coast_land_refinement_enabled,
            coast_ocean_refinement_enabled: hydro.coast_ocean_refinement_enabled,
            merit_stride: hydro.merit_stride,
            target_dx_km,
            include_classes: vec![
                "R2".to_string(),
                "R3".to_string(),
                "COAST_LAND".to_string(),
                "COAST_OCEAN".to_string(),
            ],
            max_level: if self.refinement.enabled
                && self.refinement.threshold_enabled
                && (hydro.has_river_refinement()
                    || (hydro.coast_refinement_enabled
                        && (hydro.coast_land_refinement_enabled
                            || hydro.coast_ocean_refinement_enabled)))
            {
                self.refinement
                    .max_passes
                    .clamp(1, METHOD_C_MAX_AUTO_REFINE_LEVEL)
            } else {
                0
            },
        }))
    }
}
