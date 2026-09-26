//! Rerunning the refine pipeline against a hydro target field.
//!
//! `hydro_refinement_adapter` turns a hydro plan into a target field; this
//! writes the namelist that asks for it and runs the pipeline again. That
//! second half is orchestration, so it lives with the pipeline in the CLI
//! rather than with the field in the input layer.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_hfield::EARTH_RADIUS_METERS;

use crate::hydro_refinement_adapter::*;
use crate::{HfieldRefineOptions, RefinePipelineRunReport};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[derive(Clone, Debug)]
pub struct HydroRefinementAdapterReport {
    pub adapter_namelist: PathBuf,
    pub target: HydroTargetFieldSummary,
    pub pipeline: RefinePipelineRunReport,
}

impl HydroRefinementAdapterReport {
    /// Final production gridfile written by the Method-C rerun.
    pub fn final_gridfile(&self) -> &Path {
        &self.pipeline.output.output
    }
}

pub fn run_hydro_refinement_adapter(
    source_namelist: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
    adapter_namelist: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
) -> io::Result<HydroRefinementAdapterReport> {
    run_refinement_adapter_with_controls(
        source_namelist,
        initial_gridfile,
        cells_geojson,
        target_levels_json,
        adapter_namelist,
        workdir,
        max_tris,
        source_gridnum_perdegree,
        None,
        None,
    )
}

/// Execute one quality-driven local refinement with enough spring relaxation
/// to judge the resulting cell shapes rather than an under-relaxed transition.
/// Unspecified iteration counts remain unspecified so Method-C keeps its
/// canonical 5000-atmosphere / 2000-surface policy.
pub fn run_quality_refinement_adapter(
    source_namelist: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
    adapter_namelist: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
) -> io::Result<HydroRefinementAdapterReport> {
    const MIN_QUALITY_REPAIR_SPRING_ITERATIONS: i32 = 20;
    run_refinement_adapter_with_controls(
        source_namelist,
        initial_gridfile,
        cells_geojson,
        target_levels_json,
        adapter_namelist,
        workdir,
        max_tris,
        source_gridnum_perdegree,
        None,
        Some(MIN_QUALITY_REPAIR_SPRING_ITERATIONS),
    )
}

/// Execute the hydro adapter while optionally tightening (never loosening) the
/// source HField gradation. The closed-loop quality gate uses this for one
/// bounded physical retry when a deep refinement transition is too abrupt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_hydro_refinement_adapter_with_gradation_cap(
    source_namelist: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
    adapter_namelist: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
    hfield_g_cap: Option<f64>,
) -> io::Result<HydroRefinementAdapterReport> {
    run_refinement_adapter_with_controls(
        source_namelist,
        initial_gridfile,
        cells_geojson,
        target_levels_json,
        adapter_namelist,
        workdir,
        max_tris,
        source_gridnum_perdegree,
        hfield_g_cap,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_refinement_adapter_with_controls(
    source_namelist: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
    adapter_namelist: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
    hfield_g_cap: Option<f64>,
    min_spring_iterations: Option<i32>,
) -> io::Result<HydroRefinementAdapterReport> {
    let source = fs::read_to_string(source_namelist.as_ref())?;
    let initial_gridfile = fs::canonicalize(initial_gridfile.as_ref())?;
    let mut config = EarthmeshConfig::from_mkgrd_namelist(&source)
        .map_err(|error| invalid(format!("hydro adapter parse mkgrd: {error}")))?;
    let mut refine = if config.refine && source.to_ascii_lowercase().contains("&mkrefine") {
        RefineConfig::from_mkrefine_namelist_with_external_field(
            &source,
            &config.mesh_type,
            &config.mode_grid,
            true,
        )
        .map_err(|error| invalid(format!("hydro adapter parse mkrefine: {error}")))?
    } else {
        RefineConfig::default()
    };
    apply_minimum_spring_iterations(&mut refine, min_spring_iterations);
    if config.mask_restart {
        return Err(invalid(
            "hydro refinement adapter cannot run through a mask_restart namelist",
        ));
    }
    let adapter_namelist = adapter_namelist.as_ref();
    let isolated_engine_root = adapter_namelist
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("engine");
    fs::create_dir_all(&isolated_engine_root)?;
    config.base_dir = format!("{}/", isolated_engine_root.display());
    config.experiment_name = "hydro_refined".to_string();
    // The plan was measured on this exact parent mesh. Rebuilding a new coarse
    // grid from NXP/niter invalidates cell identity and degrades the far field.
    config.mode_file = initial_gridfile.display().to_string();
    config.mode_file_description = "EarthMesh".to_string();
    config.refine_backend = "method_c".to_string();
    config.refine = true;
    if config.mode_grid.trim() != "tri" {
        refine.is_transition = true;
    }
    if config.mask_domain_global {
        refine.spring_global_type = 1;
        refine.spring_regional_type = 0;
    } else {
        refine.spring_global_type = 0;
        refine.spring_regional_type = 1;
    }

    let old_hfield = crate::hfield_refine::read_hfield_refine_options(&source)?;
    let source_g = old_hfield.as_ref().map_or(0.2, |options| options.g);
    let g = hfield_g_cap.map_or(source_g, |cap| source_g.min(cap));
    let base_m = old_hfield.as_ref().and_then(|options| options.base_m);
    let nlon = old_hfield.as_ref().map_or(720, |options| options.nlon);
    let nlat = old_hfield.as_ref().map_or(360, |options| options.nlat);
    let origin_lines = old_hfield
        .as_ref()
        .and_then(|options| options.geographic_origin)
        .map(|(lon, lat)| {
            format!("  NL%hfield_origin_lon = {lon}\n  NL%hfield_origin_lat = {lat}\n")
        })
        .unwrap_or_default();
    let mut cells_geojson = fs::canonicalize(cells_geojson.as_ref())?;
    let mut target_levels_json = fs::canonicalize(target_levels_json.as_ref())?;
    if let Some((old_cells, old_levels)) = old_hfield
        .as_ref()
        .and_then(HfieldRefineOptions::hydro_target_paths)
    {
        let old_cells = fs::canonicalize(old_cells)?;
        let old_levels = fs::canonicalize(old_levels)?;
        if (old_cells.as_path(), old_levels.as_path())
            != (cells_geojson.as_path(), target_levels_json.as_path())
        {
            (cells_geojson, target_levels_json) = combine_target_sources(
                &old_cells,
                &old_levels,
                &cells_geojson,
                &target_levels_json,
                &adapter_namelist
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("composed_hfield_inputs"),
            )?;
            cells_geojson = fs::canonicalize(cells_geojson)?;
            target_levels_json = fs::canonicalize(target_levels_json)?;
        }
    }
    let target = load_hydro_target_field(
        &cells_geojson,
        &target_levels_json,
        base_m.unwrap_or_else(|| {
            2.0 * std::f64::consts::PI * EARTH_RADIUS_METERS / (5.0 * f64::from(config.nxp))
        }),
        g,
        nlon,
        nlat,
    )?;
    let engine_max_level = usize::from(target.summary.max_level)
        .max(
            old_hfield
                .as_ref()
                .and_then(|options| options.max_level)
                .unwrap_or(0),
        )
        .max(refine.max_iter_spc.max(0) as usize)
        .max(refine.max_iter_cal.max(0) as usize)
        .clamp(1, 5);
    let base_line = base_m
        .map(|value| format!("  NL%hfield_base_m = {value}\n"))
        .unwrap_or_default();
    let hfield = format!(
        "&hfield\n  NL%hfield_on = .true.\n  NL%hfield_g = {g}\n  NL%hfield_max_level = {}\n{base_line}{origin_lines}  NL%hfield_nlon = {nlon}\n  NL%hfield_nlat = {nlat}\n  NL%hfield_target_cells_geojson = {}\n  NL%hfield_target_levels_json = {}\n/\n",
        engine_max_level,
        quote_path(&cells_geojson)?,
        quote_path(&target_levels_json)?,
    );
    // Target levels are absolute, so replaying realized sources is idempotent.
    // Keeping them enabled also covers cells moved across an original target
    // boundary by the repair pass's spring relaxation.
    let text = format!(
        "{}\n{}\n{}",
        config.to_mkgrd_namelist(),
        refine.to_mkrefine_namelist(),
        hfield
    );
    crate::ensure_parent_dir(adapter_namelist)?;
    fs::write(adapter_namelist, text)?;
    let pipeline = crate::run_refine_pipeline_namelist(
        adapter_namelist,
        workdir,
        max_tris,
        source_gridnum_perdegree,
    )?;
    Ok(HydroRefinementAdapterReport {
        adapter_namelist: adapter_namelist.to_path_buf(),
        target: target.summary,
        pipeline,
    })
}

fn apply_minimum_spring_iterations(refine: &mut RefineConfig, minimum: Option<i32>) {
    if let Some(minimum) = minimum.filter(|_| refine.niter_refine_specified) {
        refine.niter_refine = refine.niter_refine.max(minimum);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_repair_enforces_a_minimum_spring_budget_without_reducing_overrides() {
        let mut canonical = RefineConfig::default();
        apply_minimum_spring_iterations(&mut canonical, Some(20));
        assert!(!canonical.niter_refine_specified);
        assert_eq!(
            crate::refinement_spring_iterations(&canonical, true).unwrap(),
            5000
        );
        assert_eq!(
            crate::refinement_spring_iterations(&canonical, false).unwrap(),
            2000
        );

        let mut under_relaxed = RefineConfig {
            niter_refine: 1,
            niter_refine_specified: true,
            ..RefineConfig::default()
        };
        apply_minimum_spring_iterations(&mut under_relaxed, Some(20));
        assert_eq!(under_relaxed.niter_refine, 20);
        assert!(under_relaxed.niter_refine_specified);

        let mut expert = RefineConfig {
            niter_refine: 40,
            niter_refine_specified: true,
            ..RefineConfig::default()
        };
        apply_minimum_spring_iterations(&mut expert, Some(20));
        assert_eq!(expert.niter_refine, 40);
    }
}
