use crate::apply_workspace_and_mask_operations;
use crate::MaskRestartAction;
use crate::MaskRestartRemaskPlan;
use crate::MkgrdMaskRestartPatchRunReport;
use crate::MkgrdMaskRestartPlanReport;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState};

/// Plan the top-level `mkgrd.F90` mask-restart branch without running
/// `MOD_mask_postproc.F90:mask_postproc`.
pub fn plan_mkgrd_mask_restart_namelist(
    namelist_source: impl AsRef<Path>,
    _workdir: impl AsRef<Path>,
    max_iter: i32,
) -> io::Result<MkgrdMaskRestartPlanReport> {
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    if !config.mask_restart {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_restart branch requires NL%mask_restart=.true.",
        ));
    }

    let workspace_plan = config.read_nl_workspace_plan(None);
    let action = if config.mesh_type == "oceanmesh" && !config.mask_patch_on {
        MaskRestartAction::RunMaskPostproc
    } else {
        MaskRestartAction::ContinueMkgrd
    };
    let remask = MaskRestartRemaskPlan {
        file_dir: PathBuf::from(config.file_dir()),
        mesh_type: config.mesh_type.clone(),
        step: max_iter + 1,
        refine: false,
        action,
    };
    if remask.step <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart runtime step must be positive after max_iter + 1, got {}",
                remask.step
            ),
        ));
    }

    let mut runtime_config = config.clone();
    runtime_config.refine = false;
    let runtime_state = EarthmeshRuntimeState::new(runtime_config).with_step(remask.step as usize);

    Ok(MkgrdMaskRestartPlanReport {
        config,
        runtime_state,
        workspace_plan,
        remask,
    })
}

/// Execute the current `mkgrd.F90:read_nl` mask-restart branch that runs
/// `Mask_make('mask_patch', ...)` and then returns to the normal mkgrd flow.
pub fn run_mkgrd_mask_restart_patch_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
) -> io::Result<MkgrdMaskRestartPatchRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let plan = plan_mkgrd_mask_restart_namelist(namelist_source, workdir, max_iter)?;
    if !plan.config.mask_patch_on {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_restart patch execution requires NL%mask_patch_on=.true.",
        ));
    }
    if plan.remask.action != MaskRestartAction::ContinueMkgrd {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart patch execution must continue mkgrd; got action {:?}",
                plan.remask.action
            ),
        ));
    }
    let workspace_mask = apply_workspace_and_mask_operations(
        &plan.workspace_plan,
        namelist_source,
        workdir,
        0,
        false,
    )?;

    let runtime_state = plan.runtime_state.clone();

    Ok(MkgrdMaskRestartPatchRunReport {
        plan,
        runtime_state,
        workspace_mask,
    })
}
