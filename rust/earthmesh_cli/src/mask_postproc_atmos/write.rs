use std::io;
use std::path::Path;

use crate::{
    write_mpas_mesh_from_netcdf_inputs, write_mpas_simple_mesh_from_netcdf_inputs,
    MpasFullMeshPipelineReport, MpasSimpleMeshWriteReport,
};

/// Rust entry point for the `mask_postproc_Atmos` branch when
/// `output_format == 'MPAS-Simple'`.
///
/// This preserves the standard result-file names used by
/// `MPAS_Mesh_Cal_Simple`:
/// `result/gridfile_NXP####_<mode_grid>.nc4`,
/// `result/cellwidth_NXP####_global.nc4`, and
/// `result/MPASOUT_NXP####_global_Simple.nc4`.
pub fn write_mask_postproc_atmos_mpas_simple_netcdf(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
    mesh_type: &str,
    output_format: &str,
) -> io::Result<MpasSimpleMeshWriteReport> {
    if !matches!(mesh_type.trim(), "atmos" | "atmosmesh") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS-Simple mask_postproc writer requires mesh_type atmosmesh",
        ));
    }
    if output_format.trim() != "MPAS-Simple" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS-Simple mask_postproc writer requires output_format MPAS-Simple",
        ));
    }
    let mode_grid = mode_grid.trim();
    if !matches!(mode_grid, "tri" | "hex") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS-Simple mask_postproc writer supports tri or hex mode_grid only",
        ));
    }

    let file_dir = file_dir.as_ref();
    let nxpc = format!("{nxp:04}");
    let gridfile = file_dir
        .join("result")
        .join(format!("gridfile_NXP{nxpc}_{mode_grid}.nc4"));
    let cellwidth = file_dir
        .join("result")
        .join(format!("cellwidth_NXP{nxpc}_global.nc4"));
    let output = file_dir
        .join("result")
        .join(format!("MPASOUT_NXP{nxpc}_global_Simple.nc4"));

    write_mpas_simple_mesh_from_netcdf_inputs(gridfile, cellwidth, output)
}

/// Rust entry point for the `mask_postproc_Atmos` branch when
/// `output_format == 'MPAS'`.
///
/// This preserves the standard result-file names used by `MPAS_Mesh_Cal`:
/// `result/gridfile_NXP####_<mode_grid>.nc4`,
/// `result/cellwidth_NXP####_global.nc4`,
/// `result/MPASOUT_NXP####_global.nc4`, and
/// `result/MPASOUT_NXP####_global.graph.info`.
pub fn write_mask_postproc_atmos_mpas_netcdf(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
    mesh_type: &str,
    output_format: &str,
) -> io::Result<MpasFullMeshPipelineReport> {
    if !matches!(mesh_type.trim(), "atmos" | "atmosmesh") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS mask_postproc writer requires mesh_type atmosmesh",
        ));
    }
    if output_format.trim() != "MPAS" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS mask_postproc writer requires output_format MPAS",
        ));
    }
    let mode_grid = mode_grid.trim();
    if !matches!(mode_grid, "tri" | "hex") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS mask_postproc writer supports tri or hex mode_grid only",
        ));
    }

    let file_dir = file_dir.as_ref();
    let nxpc = format!("{nxp:04}");
    let gridfile = file_dir
        .join("result")
        .join(format!("gridfile_NXP{nxpc}_{mode_grid}.nc4"));
    let cellwidth = file_dir
        .join("result")
        .join(format!("cellwidth_NXP{nxpc}_global.nc4"));
    let mesh_output = file_dir
        .join("result")
        .join(format!("MPASOUT_NXP{nxpc}_global.nc4"));
    let graph_output = file_dir
        .join("result")
        .join(format!("MPASOUT_NXP{nxpc}_global.graph.info"));

    write_mpas_mesh_from_netcdf_inputs(gridfile, cellwidth, mesh_output, graph_output, nxp, step)
}
