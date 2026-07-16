use crate::infer_mask_restart_ocean_num_vertex_from_config;
use crate::maybe_infer_mask_restart_non_ocean_num_vertex_from_config;
use crate::plan_mkgrd_mask_restart_namelist;
use crate::refine_pipeline_refine_dispatch_requested;
use crate::run_mkgrd_gridinit_global_namelist;
use crate::run_mkgrd_mask_restart_area_judge_configured_global_source_namelist;
use crate::run_mkgrd_mask_restart_ocean_namelist;
use crate::run_mkgrd_mask_restart_patch_namelist;
use crate::run_refine_pipeline_namelist;
use crate::MaskPostprocOceanRunOptions;
use crate::MaskRestartAction;
use crate::MkgrdTopLevelDispatchRunReport;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshConfig;

/// Run the current top-level `mkgrd.x` namelist dispatcher for option-free branches.
///
/// This is the Rust replacement for the first branch decision in `mkgrd.F90`:
/// mask-restart namelists must not fall through to the normal gridinit path.
/// Restart variants that require extra source-grid/postprocess options return a typed plan;
/// the patch-preprocess branch is fully executable because it only depends on the namelist.
pub fn run_mkgrd_top_level_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    mask_restart_max_iter: i32,
) -> io::Result<MkgrdTopLevelDispatchRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    if config.mask_restart {
        if config.mask_patch_on {
            let restart_area_grid =
                PathBuf::from(config.file_dir()).join("result/IsInDmArea_grid.nc4");
            if restart_area_grid.exists() {
                return run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
                    namelist_source,
                    workdir,
                    mask_restart_max_iter,
                    None,
                )
                .map(MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge);
            }
            return run_mkgrd_mask_restart_patch_namelist(
                namelist_source,
                workdir,
                mask_restart_max_iter,
            )
            .map(MkgrdTopLevelDispatchRunReport::MaskRestartPatch);
        }
        let plan =
            plan_mkgrd_mask_restart_namelist(namelist_source, workdir, mask_restart_max_iter)?;
        if plan.remask.action == MaskRestartAction::RunMaskPostproc
            && plan.config.mesh_type.trim() == "oceanmesh"
        {
            let num_vertex = infer_mask_restart_ocean_num_vertex_from_config(&plan.config)?;
            return run_mkgrd_mask_restart_ocean_namelist(
                namelist_source,
                workdir,
                mask_restart_max_iter,
                MaskPostprocOceanRunOptions {
                    mask_sea_ratio: plan.config.mask_sea_ratio,
                    num_vertex,
                },
            )
            .map(MkgrdTopLevelDispatchRunReport::MaskRestartOcean);
        }
        if plan.remask.action == MaskRestartAction::ContinueMkgrd
            && !plan.config.mask_patch_on
            && plan.config.mesh_type.trim() != "oceanmesh"
        {
            let postproc_num_vertex =
                maybe_infer_mask_restart_non_ocean_num_vertex_from_config(&plan.config)?;
            return run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
                namelist_source,
                workdir,
                mask_restart_max_iter,
                postproc_num_vertex,
            )
            .map(MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge);
        }
        return Ok(MkgrdTopLevelDispatchRunReport::MaskRestartPlan(plan));
    }

    if refine_pipeline_refine_dispatch_requested(&contents, &config)? {
        return run_refine_pipeline_namelist(namelist_source, workdir, max_tris, None)
            .map(MkgrdTopLevelDispatchRunReport::RefinePipeline);
    }

    run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)
        .map(MkgrdTopLevelDispatchRunReport::Gridinit)
}
