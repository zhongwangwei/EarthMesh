//! Final-only legacy handoff. Candidate/preprocessor callers keep using the
//! unchecked composition helpers; no domain-specific mesh algorithm lives here.
use super::runners::{
    run_mask_postproc_earth_domain_checked, run_mask_postproc_land_domain_checked,
    run_mask_postproc_ocean_domain_checked,
};
use crate::project_delivery::LegacyDeliveryStage;
use crate::{
    MaskPostprocDomainIoPlan, MaskPostprocEarthDomainReport, MaskPostprocEarthRunOptions,
    MaskPostprocLandDomainReport, MaskPostprocLandRunOptions, MaskPostprocOceanDomainReport,
    MaskPostprocOceanRunOptions, UnstructuredMeshWriteReport,
};
use earthmesh_project::{MeshCellKind, MeshDomainKind};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

/// Admit the final Earth mesh before publishing auxiliary files.
/// Supply `Some(2)` for an unmasked global sphere, `None` for regional/masked output.
pub fn run_final_mask_postproc_earth_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocEarthRunOptions<'_>,
    expected_euler_characteristic: Option<isize>,
) -> io::Result<MaskPostprocEarthDomainReport> {
    let (stage, staged_plan, quality_dir) = begin_final_delivery(plan)?;
    let (mut report, quality) =
        run_mask_postproc_earth_domain_checked(&staged_plan, options, |grid| {
            admit(plan, grid, &quality_dir, expected_euler_characteristic)
        })?;
    report.final_gridfile.output = plan.result_gridfile.clone();
    report.patchtype.output = plan
        .patchtype_output
        .clone()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing patchtype output"))?;
    report.earthmesh_info.output = plan.file_dir.join("result/earthmesh_info.nc4");
    record(
        &stage,
        plan,
        &quality,
        &BTreeMap::from([
            ("patchtype", report.patchtype.output.clone()),
            ("earthmesh_info", report.earthmesh_info.output.clone()),
        ]),
    )?;
    Ok(report)
}

/// Admit complete land cells with masked/regional topology before patchtype output.
pub fn run_final_mask_postproc_land_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocLandRunOptions<'_>,
) -> io::Result<MaskPostprocLandDomainReport> {
    let (stage, staged_plan, quality_dir) = begin_final_delivery(plan)?;
    let (mut report, quality) =
        run_mask_postproc_land_domain_checked(&staged_plan, options, |grid| {
            admit(plan, grid, &quality_dir, None)
        })?;
    report.final_gridfile.output = plan.result_gridfile.clone();
    report.patchtype.output = plan
        .patchtype_output
        .clone()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing patchtype output"))?;
    record(
        &stage,
        plan,
        &quality,
        &BTreeMap::from([("patchtype", report.patchtype.output.clone())]),
    )?;
    Ok(report)
}

/// Admit final ocean cells and embedded boundary metadata before OBC sidecars.
pub fn run_final_mask_postproc_ocean_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocOceanRunOptions,
) -> io::Result<MaskPostprocOceanDomainReport> {
    let (stage, staged_plan, quality_dir) = begin_final_delivery(plan)?;
    let (mut report, quality) =
        run_mask_postproc_ocean_domain_checked(&staged_plan, options, |grid| {
            admit(plan, grid, &quality_dir, None)
        })?;
    report.final_gridfile.output = plan.result_gridfile.clone();
    let mut auxiliary = BTreeMap::new();
    for (key, output, published) in [
        (
            "obc",
            report.obc.as_mut().map(|r| &mut r.output),
            &plan.obc_output,
        ),
        (
            "obcv2",
            report.obcv2.as_mut().map(|r| &mut r.output),
            &plan.obcv2_output,
        ),
    ] {
        if let Some(output) = output {
            *output = published.clone().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("missing {key} output"))
            })?;
            auxiliary.insert(key, output.clone());
        }
    }
    record(&stage, plan, &quality, &auxiliary)?;
    Ok(report)
}

fn cell_kind(plan: &MaskPostprocDomainIoPlan) -> io::Result<MeshCellKind> {
    match plan.mode_grid.as_str() {
        "hex" => Ok(MeshCellKind::Hex),
        "tri" => Ok(MeshCellKind::Tri),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "final domain delivery requires tri or hex",
        )),
    }
}

fn begin_final_delivery(
    plan: &MaskPostprocDomainIoPlan,
) -> io::Result<(LegacyDeliveryStage, MaskPostprocDomainIoPlan, PathBuf)> {
    cell_kind(plan)?;
    let quality_dir = plan
        .result_gridfile
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("final_quality")
        .join(plan.result_gridfile.file_stem().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "final gridfile needs a filename",
            )
        })?);
    let earth_info = plan.file_dir.join("result/earthmesh_info.nc4");
    let mut outputs = vec![plan.result_gridfile.as_path()];
    if matches!(plan.mesh_type.as_str(), "earthmesh" | "landmesh") {
        outputs.extend(plan.patchtype_output.as_deref());
    }
    if plan.mesh_type == "earthmesh" {
        outputs.push(&earth_info);
    }
    if plan.mesh_type == "oceanmesh" && plan.mode_grid == "tri" {
        outputs.extend(plan.obc_output.as_deref());
        outputs.extend(plan.obcv2_output.as_deref());
    }
    let stage = LegacyDeliveryStage::new(
        &[&plan.source_gridfile, &plan.contain_domain],
        &outputs,
        &quality_dir,
    )?;
    let mut staged = plan.clone();
    staged.result_gridfile = stage.path(&plan.result_gridfile)?;
    if matches!(plan.mesh_type.as_str(), "earthmesh" | "landmesh") {
        staged.patchtype_output = plan
            .patchtype_output
            .as_ref()
            .map(|p| stage.path(p))
            .transpose()?;
    }
    if plan.mesh_type == "earthmesh" {
        // The existing Earth writer appends result/earthmesh_info.nc4 to file_dir.
        // Redirect only that output root; explicit source/contain inputs stay unchanged.
        staged.file_dir = stage
            .path(&earth_info)?
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::other("invalid Earth delivery staging path"))?
            .to_path_buf();
    }
    if plan.mesh_type == "oceanmesh" && plan.mode_grid == "tri" {
        staged.obc_output = plan
            .obc_output
            .as_ref()
            .map(|p| stage.path(p))
            .transpose()?;
        staged.obcv2_output = plan
            .obcv2_output
            .as_ref()
            .map(|p| stage.path(p))
            .transpose()?;
    }
    Ok((stage, staged, quality_dir))
}

fn admit(
    plan: &MaskPostprocDomainIoPlan,
    grid: &UnstructuredMeshWriteReport,
    quality_dir: &Path,
    expected_euler_characteristic: Option<isize>,
) -> io::Result<earthmesh_quality::MeshQualityReport> {
    let spec = crate::project_quality::FinalAdmissionSpec {
        cell_kind: cell_kind(plan)?,
        expected_euler_characteristic,
        thresholds: earthmesh_quality::QualityThresholds::default(),
        repair_level_cap: None,
    };
    crate::project_quality::admit_staged_final_gridfile(
        &spec,
        &grid.output,
        &plan.result_gridfile,
        quality_dir,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn record(
    stage: &LegacyDeliveryStage,
    plan: &MaskPostprocDomainIoPlan,
    quality: &earthmesh_quality::MeshQualityReport,
    auxiliary: &BTreeMap<&str, PathBuf>,
) -> io::Result<()> {
    let kind = match plan.mesh_type.as_str() {
        "earthmesh" => MeshDomainKind::Earth,
        "landmesh" => MeshDomainKind::Land,
        _ => MeshDomainKind::Ocean,
    };
    // Patch IDs, Earth info and OBC files accompany the native mesh; they are
    // not proof that a CoLM or FVCOM model-format adapter has run.
    stage.publish(
        serde_json::json!({
            "kind": "earthmesh_legacy_delivery",
            "target": {"kind": kind, "cell": cell_kind(plan)?},
            "capability": "native_and_auxiliary",
            "source_mode_grid": plan.mode_grid,
            "skipped_reason": "Native grid and auxiliary files only; no specialized model adapter was run",
        }),
        &plan.result_gridfile,
        quality.verdict,
        &BTreeMap::new(),
        auxiliary,
    )?;
    Ok(())
}
