use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

use earthmesh_project::{MeshCellKind, MeshDomainKind, ModelFormat, ProjectTargetTriple};

use crate::project_delivery::LegacyDeliveryStage;
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

    let quality_dir = atmos_quality_dir(&gridfile, output_format);
    let stage = LegacyDeliveryStage::new(&[&gridfile, &cellwidth], &[&output], &quality_dir)?;
    let quality = admit_atmos_final_gridfile(&gridfile, &quality_dir)?;
    let mut report =
        write_mpas_simple_mesh_from_netcdf_inputs(&gridfile, &cellwidth, stage.path(&output)?)?;
    report.output = output;
    record_atmos_delivery(
        &stage,
        &gridfile,
        mode_grid,
        ModelFormat::MpasSimple,
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

    let quality_dir = atmos_quality_dir(&gridfile, output_format);
    let stage = LegacyDeliveryStage::new(
        &[&gridfile, &cellwidth],
        &[&mesh_output, &graph_output],
        &quality_dir,
    )?;
    let quality = admit_atmos_final_gridfile(&gridfile, &quality_dir)?;
    let mut report = write_mpas_mesh_from_netcdf_inputs(
        &gridfile,
        &cellwidth,
        stage.path(&mesh_output)?,
        stage.path(&graph_output)?,
        nxp,
        step,
    )?;
    report.mesh.output = mesh_output;
    report.graph_info.output = graph_output;
    record_atmos_delivery(
        &stage,
        &gridfile,
        mode_grid,
        ModelFormat::Mpas,
        quality.verdict,
        &BTreeMap::from([
            ("mpas_mesh_input", report.mesh.output.clone()),
            ("mpas_graph_info", report.graph_info.output.clone()),
        ]),
    )?;
    Ok(report)
}

fn atmos_quality_dir(gridfile: &Path, output_format: &str) -> PathBuf {
    gridfile
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("final_quality")
        .join(output_format.trim())
}

fn admit_atmos_final_gridfile(
    gridfile: &Path,
    out_dir: &Path,
) -> io::Result<earthmesh_quality::MeshQualityReport> {
    // Legacy mode_grid selects a source filename; BOTH MPAS adapters export
    // physical dual-W polygons, even from a source labelled tri. Do not check
    // M triangles here, and do not broaden Project's TRI+MPAS capabilities.
    let spec = crate::project_quality::FinalAdmissionSpec {
        cell_kind: MeshCellKind::Hex,
        expected_euler_characteristic: Some(2),
        thresholds: earthmesh_quality::QualityThresholds::default(),
        repair_level_cap: None, // This delivery adapter does not run AutoRefine.
    };
    crate::project_quality::admit_final_gridfile(&spec, gridfile, out_dir, None)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn record_atmos_delivery(
    stage: &LegacyDeliveryStage,
    gridfile: &Path,
    source_mode_grid: &str,
    format: ModelFormat,
    verdict: earthmesh_quality::QualityLevel,
    artifacts: &BTreeMap<&str, PathBuf>,
) -> io::Result<()> {
    let target = ProjectTargetTriple {
        kind: MeshDomainKind::Atmosphere,
        cell: MeshCellKind::Hex,
        model_format: format,
    };
    stage.publish(
        serde_json::json!({
            "kind": "earthmesh_legacy_delivery",
            "target": target,
            "capability": target.output_delivery(),
            "source_mode_grid": source_mode_grid,
            "scope": "closed_sphere",
            "skipped_reason": null,
        }),
        gridfile,
        verdict,
        artifacts,
        &BTreeMap::new(),
    )?;
    Ok(())
}
