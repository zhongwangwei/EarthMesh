use std::io;
use std::path::Path;

use crate::*;

/// Execute the direct ocean `mask_restart` postprocess branch while recovering
/// the legacy `num_vertex` boundary from the persisted contain file.
pub fn run_mkgrd_mask_restart_ocean_inferred_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
) -> io::Result<MkgrdMaskRestartOceanRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let plan = plan_mkgrd_mask_restart_namelist(namelist_source, workdir, max_iter)?;
    let num_vertex = infer_mask_restart_ocean_num_vertex_from_config(&plan.config)?;
    run_mkgrd_mask_restart_ocean_namelist(
        namelist_source,
        workdir,
        max_iter,
        MaskPostprocOceanRunOptions {
            mask_sea_ratio: plan.config.mask_sea_ratio,
            num_vertex,
        },
    )
}

pub fn run_mkgrd_mask_restart_ocean_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
    options: MaskPostprocOceanRunOptions,
) -> io::Result<MkgrdMaskRestartOceanRunReport> {
    let plan = plan_mkgrd_mask_restart_namelist(namelist_source, workdir, max_iter)?;
    if plan.remask.action != MaskRestartAction::RunMaskPostproc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart execution is only migrated for oceanmesh without mask_patch_on; got action {:?}",
                plan.remask.action
            ),
        ));
    }
    if plan.config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for mask_restart postproc",
        ));
    }
    let nxp = usize::try_from(plan.config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let postproc_plan = plan_mask_postproc_domain_io(
        &plan.remask.file_dir,
        nxp,
        &plan.config.mode_grid,
        &plan.config.mesh_type,
        plan.config.mask_patch_on,
    )?;
    let postproc = run_mask_postproc_ocean_domain(&postproc_plan, options)?;
    let runtime_state = plan.runtime_state.clone();

    Ok(MkgrdMaskRestartOceanRunReport {
        plan,
        runtime_state,
        postproc,
    })
}
