use crate::apply_workspace_and_mask_operations;
use crate::build_area_judge_restart_one_based;
use crate::plan_mkgrd_mask_restart_namelist;
use crate::write_area_judge_selected_grid_report;
use crate::AreaJudgePatchConfig;
use crate::MaskRestartAction;
use crate::MkgrdRestartAreaJudgeOptions;
use crate::MkgrdRestartAreaJudgeRunReport;
use std::{io, path::Path};

pub fn run_mkgrd_mask_restart_area_judge_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
    options: MkgrdRestartAreaJudgeOptions<'_>,
) -> io::Result<MkgrdRestartAreaJudgeRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let plan = plan_mkgrd_mask_restart_namelist(namelist_source, workdir, max_iter)?;
    if plan.remask.action != MaskRestartAction::ContinueMkgrd {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart Area_judge continuation requires ContinueMkgrd; got action {:?}",
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
    let mask_patch = plan.config.mask_patch_on.then_some(AreaJudgePatchConfig {
        mask_patch_type: &plan.config.mask_patch_type,
        mask_patch_ndm: workspace_mask.mask_counts.mask_patch_ndm[0],
    });
    let area_output = plan.remask.file_dir.join("result/IsInDmArea_grid.nc4");
    let area = build_area_judge_restart_one_based(
        &plan.remask.file_dir,
        &area_output,
        mask_patch,
        plan.remask.refine,
        None,
        options.lon_vertex,
        options.lat_vertex,
        options.lon_i,
        options.lat_i,
        options.gridnum_perdegree,
        options.nlons_source,
        options.nlats_source,
    )?;
    let area_write = write_area_judge_selected_grid_report(
        &area_output,
        &area.domain.is_in_domain,
        Some(&area.seaorland.seaorland),
        options.lon_i,
        options.lat_i,
        area.domain.bounds,
    )?;

    let runtime_state = plan.runtime_state.clone();

    Ok(MkgrdRestartAreaJudgeRunReport {
        plan,
        runtime_state,
        workspace_mask,
        area,
        area_write,
        refine_write: None,
    })
}
