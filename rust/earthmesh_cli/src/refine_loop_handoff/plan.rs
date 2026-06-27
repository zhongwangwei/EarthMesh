use std::io;

use crate::*;

fn validate_mkgrd_refine_loop_controls(refine: &RefineConfig, max_iter: usize) -> io::Result<()> {
    if max_iter >= refine.halo.len() || max_iter >= refine.max_transition_row.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "max_iter {max_iter} exceeds one-based refine control table length {}",
                refine.halo.len() - 1
            ),
        ));
    }

    for iter in 1..=max_iter {
        let halo = refine.halo[iter];
        let max_transition_row = refine.max_transition_row[iter];
        if halo < max_transition_row {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("halo({iter}) must be larger than or equal to max_transition_row({iter})"),
            ));
        }
        if halo <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("halo({iter}) must be more than zero"),
            ));
        }
        if max_transition_row <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("max_transition_row({iter}) must be more than zero"),
            ));
        }
    }
    Ok(())
}

/// Fortran computes `max_iter = max(max_iter_cal, max_iter_spc)`, then for each
/// `step` runs the calculated `iter=0` threshold branch while `step <=
/// max_iter_cal`, the specified branch while `step <= max_iter_spc`, and then
/// calls `refine_loop`.  When `exit_loop_step(step)` is pre-seeded, this plan
/// also models the Fortran `all(exit_loop_step(1:max_iter))` early-exit check
/// before incrementing `step`.
pub fn plan_mkgrd_refine_loop(refine: &RefineConfig) -> io::Result<MkgrdRefineLoopPlan> {
    let calculated_enabled = matches!(refine.refine_setting.as_str(), "calculate" | "mixed");
    let specified_enabled = matches!(refine.refine_setting.as_str(), "specified" | "mixed");
    let active_max_iter_cal = if calculated_enabled {
        refine.max_iter_cal
    } else {
        0
    };
    let active_max_iter_spc = if specified_enabled {
        refine.max_iter_spc
    } else {
        0
    };
    let max_iter_i32 = active_max_iter_cal.max(active_max_iter_spc);
    if max_iter_i32 <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_iter must be more than zero like mkgrd.F90 refine loop",
        ));
    }
    let max_iter = usize::try_from(max_iter_i32)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "max_iter must fit usize"))?;
    let max_iter_cal = usize::try_from(active_max_iter_cal.max(0))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "max_iter_cal must fit usize"))?;
    let max_iter_spc = usize::try_from(active_max_iter_spc.max(0))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "max_iter_spc must fit usize"))?;
    validate_mkgrd_refine_loop_controls(refine, max_iter)?;

    let mut steps = Vec::with_capacity(max_iter);
    let mut completed_steps = vec![false; max_iter + 1];
    let mut final_mask_postproc_step = max_iter + 1;
    for step in 1..=max_iter {
        let mut sources = Vec::with_capacity(2);
        if calculated_enabled && step <= max_iter_cal {
            sources.push(MkgrdRefineSource::CalculatedIterZero);
        }
        if specified_enabled && step <= max_iter_spc {
            sources.push(MkgrdRefineSource::SpecifiedStep);
        }

        if refine.exit_loop_step.get(step).copied().unwrap_or(false) {
            completed_steps[step] = true;
        }
        let stop_after_step = (1..=max_iter).all(|idx| completed_steps[idx]);
        if stop_after_step {
            final_mask_postproc_step = step;
        }

        steps.push(MkgrdRefineLoopStepPlan {
            step,
            max_transition_row: usize::try_from(refine.max_transition_row[step]).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("max_transition_row({step}) must fit usize"),
                )
            })?,
            sources,
            run_refine_loop: true,
            stop_after_step,
        });

        if stop_after_step {
            break;
        }
    }

    Ok(MkgrdRefineLoopPlan {
        max_iter,
        steps,
        final_mask_postproc_step,
    })
}
