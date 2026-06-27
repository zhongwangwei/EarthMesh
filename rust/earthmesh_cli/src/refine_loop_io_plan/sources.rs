use std::{
    io,
    path::{Path, PathBuf},
};

use crate::*;

pub(super) fn plan_mkgrd_refine_source_io(
    file_dir: &Path,
    nxp: usize,
    mesh_type: &str,
    step: usize,
    source: MkgrdRefineSource,
) -> io::Result<MkgrdRefineSourceIoPlan> {
    let stepc = format!("{step:02}");
    match source {
        MkgrdRefineSource::CalculatedIterZero => Ok(MkgrdRefineSourceIoPlan {
            source,
            area_judge_iter: 0,
            get_contain_iter: 0,
            getref_iter: 0,
            area_judge_output: file_dir
                .join("result")
                .join(format!("IsInRfArea_grid_cal_NXP{nxp:04}_{stepc}.nc4")),
            contain_output: file_dir.join("contain").join(format!(
                "contain_{mesh_type}_refine_cal_NXP{nxp:04}_{stepc}_tri.nc4"
            )),
            threshold_outputs: mkgrd_calculated_threshold_outputs(file_dir, nxp, step, mesh_type)?,
            specified_threshold_output: None,
        }),
        MkgrdRefineSource::SpecifiedStep => Ok(MkgrdRefineSourceIoPlan {
            source,
            area_judge_iter: step,
            get_contain_iter: step,
            getref_iter: step,
            area_judge_output: file_dir
                .join("result")
                .join(format!("IsInRfArea_grid_spc_NXP{nxp:04}_{stepc}.nc4")),
            contain_output: file_dir.join("contain").join(format!(
                "contain_{mesh_type}_refine_spc_NXP{nxp:04}_{stepc}_tri.nc4"
            )),
            threshold_outputs: Vec::new(),
            specified_threshold_output: Some(
                file_dir
                    .join("threshold")
                    .join(format!("threshold_specified_NXP{nxp:04}_{stepc}.nc4")),
            ),
        }),
    }
}

fn mkgrd_calculated_threshold_outputs(
    file_dir: &Path,
    nxp: usize,
    step: usize,
    mesh_type: &str,
) -> io::Result<Vec<PathBuf>> {
    let stepc = format!("{step:02}");
    let threshold_dir = file_dir.join("threshold");
    let mut outputs = Vec::new();
    match mesh_type {
        "landmesh" => outputs
            .push(threshold_dir.join(format!("threshold_calculate_land_NXP{nxp:04}_{stepc}.nc4"))),
        "oceanmesh" => outputs
            .push(threshold_dir.join(format!("threshold_calculate_ocean_NXP{nxp:04}_{stepc}.nc4"))),
        "atmos" | "atmosmesh" => outputs
            .push(threshold_dir.join(format!("threshold_calculate_atmos_NXP{nxp:04}_{stepc}.nc4"))),
        "LOCmesh" | "earthmesh" => {
            outputs.push(
                threshold_dir.join(format!("threshold_calculate_land_NXP{nxp:04}_{stepc}.nc4")),
            );
            outputs.push(
                threshold_dir.join(format!("threshold_calculate_ocean_NXP{nxp:04}_{stepc}.nc4")),
            );
            outputs.push(
                threshold_dir.join(format!("threshold_calculate_atmos_NXP{nxp:04}_{stepc}.nc4")),
            );
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported mesh_type {other} for calculated GetRef outputs"),
            ));
        }
    }
    Ok(outputs)
}
