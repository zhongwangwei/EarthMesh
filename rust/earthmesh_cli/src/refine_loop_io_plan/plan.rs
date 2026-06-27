use std::{io, path::PathBuf};

use earthmesh_core::{EarthmeshConfig, RefineConfig};

use crate::*;

use super::final_quality::{plan_mkgrd_final_quality_check_io, retarget_final_quality_check_step};
use super::paths::{mkgrd_gridfile_path, mkgrd_tmpfile_path};
use super::sources::plan_mkgrd_refine_source_io;

pub fn plan_mkgrd_refine_loop_io(
    config: &EarthmeshConfig,
    refine: &RefineConfig,
) -> io::Result<MkgrdRefineLoopIoPlan> {
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for refine loop",
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let loop_plan = plan_mkgrd_refine_loop(refine)?;
    let file_dir = PathBuf::from(config.file_dir());
    let mesh_type = config.mesh_type.trim().to_string();
    let mode_grid = config.mode_grid.trim().to_string();

    let mut steps = Vec::with_capacity(loop_plan.steps.len());
    for step in &loop_plan.steps {
        let mut sources = Vec::with_capacity(step.sources.len());
        for source in &step.sources {
            sources.push(plan_mkgrd_refine_source_io(
                &file_dir, nxp, &mesh_type, step.step, *source,
            )?);
        }
        steps.push(MkgrdRefineLoopStepIoPlan {
            step: step.step,
            max_transition_row: step.max_transition_row,
            sources,
            refine_loop_input_gridfile: mkgrd_gridfile_path(&file_dir, nxp, step.step, &mode_grid),
            refine_loop_original_tmpfile: mkgrd_tmpfile_path(&file_dir, nxp, step.step, "ori"),
            refine_loop_stage2_tmpfile: mkgrd_tmpfile_path(&file_dir, nxp, step.step, "2"),
            refine_loop_stage5_tmpfile: mkgrd_tmpfile_path(&file_dir, nxp, step.step, "5"),
            refine_loop_output_gridfile: mkgrd_gridfile_path(
                &file_dir,
                nxp,
                step.step + 1,
                &mode_grid,
            ),
            run_refine_loop: step.run_refine_loop,
            stop_after_step: step.stop_after_step,
        });
    }

    let final_step = loop_plan.final_mask_postproc_step;
    let final_quality_check = plan_mkgrd_final_quality_check_io(config, refine, final_step)?;
    let final_mask_postproc_domain =
        matches!(mesh_type.as_str(), "earthmesh" | "landmesh" | "oceanmesh")
            .then(|| {
                plan_mask_postproc_domain_io(
                    &file_dir,
                    nxp,
                    &mode_grid,
                    &mesh_type,
                    config.mask_patch_on,
                )
            })
            .transpose()?;
    Ok(MkgrdRefineLoopIoPlan {
        file_dir: file_dir.clone(),
        nxp,
        mesh_type: mesh_type.clone(),
        mode_grid: mode_grid.clone(),
        max_iter: loop_plan.max_iter,
        steps,
        final_mask_postproc_step: final_step,
        final_get_contain_iter: 0,
        final_domain_gridfile: mkgrd_gridfile_path(&file_dir, nxp, final_step, &mode_grid),
        final_result_gridfile: file_dir
            .join("result")
            .join(format!("gridfile_NXP{nxp:04}_{mode_grid}.nc4")),
        final_domain_contain_output: file_dir.join("contain").join(format!(
            "contain_{mesh_type}_domain_NXP{nxp:04}_{mode_grid}.nc4"
        )),
        final_quality_check,
        final_mask_postproc_domain,
    })
}

pub fn infer_mkgrd_effective_final_step_from_gridfiles(
    plan: &MkgrdRefineLoopIoPlan,
) -> io::Result<usize> {
    let planned_step = plan.final_mask_postproc_step;
    if planned_step != plan.max_iter + 1 || planned_step <= 1 {
        return Ok(planned_step);
    }
    if !plan.final_domain_gridfile.exists() {
        let previous_gridfile =
            mkgrd_gridfile_path(&plan.file_dir, plan.nxp, planned_step - 1, &plan.mode_grid);
        if !previous_gridfile.exists() {
            return Ok(planned_step);
        }
        return Ok(planned_step - 1);
    }
    Ok(planned_step)
}

pub(crate) fn effective_mkgrd_refine_loop_io_plan(
    plan: &MkgrdRefineLoopIoPlan,
) -> io::Result<MkgrdRefineLoopIoPlan> {
    let effective_step = infer_mkgrd_effective_final_step_from_gridfiles(plan)?;
    if effective_step == plan.final_mask_postproc_step {
        return Ok(plan.clone());
    }
    let mut effective = plan.clone();
    effective.final_mask_postproc_step = effective_step;
    effective.final_domain_gridfile = mkgrd_gridfile_path(
        &effective.file_dir,
        effective.nxp,
        effective_step,
        &effective.mode_grid,
    );
    effective.final_quality_check = retarget_final_quality_check_step(
        &effective.final_quality_check,
        &effective.file_dir,
        effective.nxp,
        &effective.mode_grid,
        effective_step,
    );
    Ok(effective)
}
