use std::fs;
use std::io;
use std::path::PathBuf;

use super::{
    global::{final_quality_global_distance_steps, final_quality_global_spring_options},
    paths::{final_quality_file_dir_and_nxp, required_final_quality_path},
    quality::grid_quality_global_from_unstructured_mesh,
    regional::run_final_regional_spring_from_unstructured_mesh,
};
use crate::*;

/// Execute the migrated file-backed subset of `mkgrd.F90:Final_Grid_Quality_Check`.
///
/// This preserves the legacy side-effect order: copy the pre-spring gridfile,
/// write quality diagnostics before spring adjustment, run the selected
/// global/final-regional spring path, write quality diagnostics after spring,
/// then persist the adjusted mesh back to the planned output gridfile.
pub fn run_mkgrd_final_quality_check(plan: &MkgrdFinalQualityCheckIoPlan) -> io::Result<()> {
    if !plan.run_quality_check {
        return Ok(());
    }

    let original_gridfile =
        required_final_quality_path(plan.original_gridfile.as_deref(), "original gridfile")?;
    let quality_before_spring = required_final_quality_path(
        plan.quality_before_spring.as_deref(),
        "quality before spring",
    )?;
    let quality_after_spring =
        required_final_quality_path(plan.quality_after_spring.as_deref(), "quality after spring")?;
    let output_gridfile =
        required_final_quality_path(plan.output_gridfile.as_deref(), "output gridfile")?;

    if let Some(parent) = original_gridfile.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&plan.input_gridfile, original_gridfile)?;

    let raw_mesh = read_unstructured_mesh_netcdf(&plan.input_gridfile)?;
    let mesh = normalize_unstructured_mesh_legacy_placeholders(&raw_mesh)?;
    let before_quality = grid_quality_global_from_unstructured_mesh(&mesh)?;
    write_grid_quality_global_netcdf(quality_before_spring, &before_quality)?;

    let mut restored_global_cellwidth: Option<(PathBuf, Vec<f64>)> = None;
    let adjusted_mesh = match plan.spring_mode {
        MkgrdFinalQualitySpringMode::Global => {
            let (file_dir, nxp) = final_quality_file_dir_and_nxp(plan)?;
            let global_spring = plan.global_spring.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Final_Grid_Quality_Check global spring requires global_spring controls",
                )
            })?;
            let distance_steps = final_quality_global_distance_steps(global_spring);
            let report = run_springjustment_global_from_unstructured_mesh(
                &mesh,
                &file_dir,
                nxp,
                plan.step,
                final_quality_global_spring_options(global_spring, &distance_steps),
            )?;
            if let Some(cellwidth) = report.core.cellwidth.as_ref() {
                restored_global_cellwidth = Some((
                    file_dir
                        .join("result")
                        .join(format!("cellwidth_NXP{nxp:04}_global.nc4")),
                    restore_cellwidth_shape(&raw_mesh, cellwidth)?,
                ));
            }
            report.mesh
        }
        MkgrdFinalQualitySpringMode::RegionalFinal => {
            run_final_regional_spring_from_unstructured_mesh(plan, &mesh)?
        }
        MkgrdFinalQualitySpringMode::SkippedBothDisabled
        | MkgrdFinalQualitySpringMode::SkippedRegionalEachStep => mesh.clone(),
    };

    let after_quality = grid_quality_global_from_unstructured_mesh(&adjusted_mesh)?;
    write_grid_quality_global_netcdf(quality_after_spring, &after_quality)?;
    let output_mesh = restore_unstructured_mesh_shape(&raw_mesh, &adjusted_mesh)?;
    write_unstructured_mesh_netcdf(output_gridfile, &output_mesh)?;
    if let Some((cellwidth_path, cellwidth)) = restored_global_cellwidth {
        write_cellwidth_netcdf(
            cellwidth_path,
            &CellwidthMesh {
                cell_points: raw_mesh.w_points.clone(),
                cellwidth,
            },
        )?;
    }
    Ok(())
}

fn restore_cellwidth_shape(original: &UnstructuredMesh, cellwidth: &[f64]) -> io::Result<Vec<f64>> {
    if cellwidth.len() < original.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth is smaller than original mesh cell shape",
        ));
    }
    let cell_offset = cellwidth.len() - original.w_points.len();
    Ok(cellwidth[cell_offset..].to_vec())
}
