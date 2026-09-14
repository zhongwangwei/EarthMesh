use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_project::{MeshCellKind, MeshDomainKind, ModelFormat, ProjectTargetTriple};

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

    let (quality_dir, quality) = admit_atmos_final_gridfile(&gridfile, output_format)?;
    let report = write_mpas_simple_mesh_from_netcdf_inputs(&gridfile, cellwidth, output)?;
    record_atmos_delivery(
        &gridfile,
        mode_grid,
        ModelFormat::MpasSimple,
        &quality_dir,
        quality.verdict,
        &BTreeMap::from([("mpas_mesh_input", report.output.clone())]),
    )?;
    Ok(report)
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

    let (quality_dir, quality) = admit_atmos_final_gridfile(&gridfile, output_format)?;
    let report = write_mpas_mesh_from_netcdf_inputs(
        &gridfile,
        cellwidth,
        mesh_output,
        graph_output,
        nxp,
        step,
    )?;
    record_atmos_delivery(
        &gridfile,
        mode_grid,
        ModelFormat::Mpas,
        &quality_dir,
        quality.verdict,
        &BTreeMap::from([
            ("mpas_mesh_input", report.mesh.output.clone()),
            ("mpas_graph_info", report.graph_info.output.clone()),
        ]),
    )?;
    Ok(report)
}

fn admit_atmos_final_gridfile(
    gridfile: &Path,
    output_format: &str,
) -> io::Result<(PathBuf, earthmesh_quality::MeshQualityReport)> {
    let out_dir = gridfile
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("final_quality")
        .join(output_format.trim());
    // A prior success is not evidence for this attempt, including failures
    // while reading the input or in the downstream cellwidth/model adapter.
    let completion = out_dir.join("legacy_delivery.json");
    match fs::symlink_metadata(&completion) {
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "legacy delivery record must be a regular file",
            ));
        }
        Ok(_) => fs::remove_file(&completion)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    // Legacy mode_grid selects a source filename; BOTH MPAS adapters export
    // physical dual-W polygons, even from a source labelled tri. Do not check
    // M triangles here, and do not broaden Project's TRI+MPAS capabilities.
    let spec = crate::project_quality::FinalAdmissionSpec {
        cell_kind: MeshCellKind::Hex,
        expected_euler_characteristic: Some(2),
        thresholds: earthmesh_quality::QualityThresholds::default(),
        repair_level_cap: None, // This delivery adapter does not run AutoRefine.
    };
    let report = crate::project_quality::admit_final_gridfile(&spec, gridfile, &out_dir, None)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok((out_dir, report))
}

fn record_atmos_delivery(
    gridfile: &Path,
    source_mode_grid: &str,
    format: ModelFormat,
    quality_dir: &Path,
    verdict: earthmesh_quality::QualityLevel,
    artifacts: &BTreeMap<&str, PathBuf>,
) -> io::Result<()> {
    let target = ProjectTargetTriple {
        kind: MeshDomainKind::Atmosphere,
        cell: MeshCellKind::Hex,
        model_format: format,
    };
    crate::project_delivery::write_delivery_record(
        serde_json::json!({
            "kind": "earthmesh_legacy_delivery",
            "target": target,
            "capability": target.output_delivery(),
            "source_mode_grid": source_mode_grid,
            "scope": "closed_sphere",
            "skipped_reason": null,
        }),
        gridfile,
        &quality_dir.join("quality_summary.json"),
        verdict,
        artifacts,
        &quality_dir.join("legacy_delivery.json"),
    )?;
    Ok(())
}
