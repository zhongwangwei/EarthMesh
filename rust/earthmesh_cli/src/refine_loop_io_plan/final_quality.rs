use std::{
    io,
    path::{Path, PathBuf},
};

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_mesh::DistanceLayerSpacing;

use crate::*;

use super::paths::mkgrd_gridfile_path;

pub fn plan_mkgrd_final_quality_check_io(
    config: &EarthmeshConfig,
    refine: &RefineConfig,
    step: usize,
) -> io::Result<MkgrdFinalQualityCheckIoPlan> {
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for final quality check",
        ));
    }
    if step == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check step must be one-based",
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let file_dir = PathBuf::from(config.file_dir());
    let mode_grid = config.mode_grid.trim();
    let input_gridfile = mkgrd_gridfile_path(&file_dir, nxp, step, mode_grid);

    if refine.spring_global_type == 0 && refine.spring_regional_type == 0 {
        return Ok(MkgrdFinalQualityCheckIoPlan {
            step,
            run_quality_check: false,
            spring_mode: MkgrdFinalQualitySpringMode::SkippedBothDisabled,
            input_gridfile,
            original_gridfile: None,
            quality_before_spring: None,
            quality_after_spring: None,
            output_gridfile: None,
            regional_set_dis: None,
            global_spring: None,
            regional_spring: None,
            regional_source_mask: None,
        });
    }

    if refine.spring_regional_type == 1 {
        return Ok(MkgrdFinalQualityCheckIoPlan {
            step,
            run_quality_check: false,
            spring_mode: MkgrdFinalQualitySpringMode::SkippedRegionalEachStep,
            input_gridfile,
            original_gridfile: None,
            quality_before_spring: None,
            quality_after_spring: None,
            output_gridfile: None,
            regional_set_dis: None,
            global_spring: None,
            regional_spring: None,
            regional_source_mask: None,
        });
    }

    let spring_mode = if refine.spring_global_type == 1 {
        MkgrdFinalQualitySpringMode::Global
    } else {
        let set_dis = *refine.halo.get(1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Final_Grid_Quality_Check requires halo(1) for regional final spring",
            )
        })?;
        if set_dis <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Final_Grid_Quality_Check regional set_dis halo(1) must be positive",
            ));
        }
        MkgrdFinalQualitySpringMode::RegionalFinal
    };
    let regional_set_dis =
        (spring_mode == MkgrdFinalQualitySpringMode::RegionalFinal).then_some(refine.halo[1]);
    let global_spring = (spring_mode == MkgrdFinalQualitySpringMode::Global)
        .then(|| plan_mkgrd_final_quality_global_spring(config, refine, nxp))
        .transpose()?;
    let regional_spring = (spring_mode == MkgrdFinalQualitySpringMode::RegionalFinal)
        .then(|| plan_mkgrd_final_quality_regional_spring(refine))
        .transpose()?;

    Ok(MkgrdFinalQualityCheckIoPlan {
        step,
        run_quality_check: true,
        spring_mode,
        input_gridfile: input_gridfile.clone(),
        original_gridfile: Some(file_dir.join("gridfile").join(format!(
            "gridfile_NXP{nxp:04}_{step:02}_{mode_grid}_orial.nc4"
        ))),
        quality_before_spring: Some(file_dir.join("result").join(format!(
            "quality_NXP{nxp:04}_{step:02}_global_beforeSpring.nc4"
        ))),
        quality_after_spring: Some(
            file_dir
                .join("result")
                .join(format!("quality_NXP{nxp:04}_{step:02}_global.nc4")),
        ),
        output_gridfile: Some(input_gridfile),
        regional_set_dis,
        global_spring,
        regional_spring,
        regional_source_mask: None,
    })
}

pub(super) fn retarget_final_quality_check_step(
    plan: &MkgrdFinalQualityCheckIoPlan,
    file_dir: &Path,
    nxp: usize,
    mode_grid: &str,
    step: usize,
) -> MkgrdFinalQualityCheckIoPlan {
    let mut retargeted = plan.clone();
    retargeted.step = step;
    if retargeted.run_quality_check {
        let input_gridfile = mkgrd_gridfile_path(file_dir, nxp, step, mode_grid);
        retargeted.input_gridfile = input_gridfile.clone();
        retargeted.original_gridfile = Some(file_dir.join("gridfile").join(format!(
            "gridfile_NXP{nxp:04}_{step:02}_{mode_grid}_orial.nc4"
        )));
        retargeted.quality_before_spring = Some(file_dir.join("result").join(format!(
            "quality_NXP{nxp:04}_{step:02}_global_beforeSpring.nc4"
        )));
        retargeted.quality_after_spring = Some(
            file_dir
                .join("result")
                .join(format!("quality_NXP{nxp:04}_{step:02}_global.nc4")),
        );
        retargeted.output_gridfile = Some(input_gridfile);
    }
    retargeted
}

fn plan_mkgrd_final_quality_regional_spring(
    refine: &RefineConfig,
) -> io::Result<MkgrdFinalQualityRegionalSpringIoPlan> {
    let niter_refine = final_quality_non_negative_usize(
        refine.niter_refine,
        "Final_Grid_Quality_Check niter_refine must be non-negative",
    )?;
    Ok(MkgrdFinalQualityRegionalSpringIoPlan {
        niter_refine,
        radius: earthmesh_core::EARTH_RADIUS_METERS,
    })
}

fn plan_mkgrd_final_quality_global_spring(
    config: &EarthmeshConfig,
    refine: &RefineConfig,
    nxp: usize,
) -> io::Result<MkgrdFinalQualityGlobalSpringIoPlan> {
    let distance_num_rc = final_quality_non_negative_usize(
        refine.num_rc,
        "Final_Grid_Quality_Check num_rc must be non-negative",
    )?;
    let niter_refine = final_quality_non_negative_usize(
        refine.niter_refine,
        "Final_Grid_Quality_Check niter_refine must be non-negative",
    )?;
    let distance_spacing = if distance_num_rc == 0 {
        DistanceLayerSpacing::Linear
    } else {
        final_quality_distance_spacing(&refine.set_dis_type)?
    };
    let base_dists_on_edge =
        f64::from(config.beta) * std::f64::consts::PI * 2.0 * earthmesh_core::EARTH_RADIUS_METERS
            / (5.0 * nxp as f64);
    let base_cellwidth = match config.output_format.trim() {
        "MPAS" | "MPAS-Simple" => Some((7680 / nxp) as f64),
        _ => None,
    };

    Ok(MkgrdFinalQualityGlobalSpringIoPlan {
        base_dists_on_edge,
        base_cellwidth,
        distance_num_rc,
        distance_spacing,
        distance_steps: Vec::new(),
        niter_refine,
        relax: f64::from(config.relax),
        radius: earthmesh_core::EARTH_RADIUS_METERS,
    })
}

pub(crate) fn final_quality_non_negative_usize(value: i32, message: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, message))
}

fn final_quality_distance_spacing(set_dis_type: &str) -> io::Result<DistanceLayerSpacing> {
    match set_dis_type.trim() {
        "linear" => Ok(DistanceLayerSpacing::Linear),
        "nonlinear1" => Ok(DistanceLayerSpacing::Power),
        "nonlinear2" => Ok(DistanceLayerSpacing::Exponential),
        "nonlinear3" => Ok(DistanceLayerSpacing::Logarithmic),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Final_Grid_Quality_Check set_dis_type must be linear, nonlinear1, nonlinear2, or nonlinear3; got {other}"),
        )),
    }
}
