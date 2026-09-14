//! Final-only legacy handoff. Candidate/preprocessor callers keep using the
//! unchecked composition helpers; no domain-specific mesh algorithm lives here.
// ponytail: native/sidecar writes are not a transaction; stage a bundle when rollback is added.
use super::runners::{
    run_mask_postproc_earth_domain_checked, run_mask_postproc_land_domain_checked,
    run_mask_postproc_ocean_domain_checked,
};
use crate::{
    MaskPostprocDomainIoPlan, MaskPostprocEarthDomainReport, MaskPostprocEarthRunOptions,
    MaskPostprocLandDomainReport, MaskPostprocLandRunOptions, MaskPostprocOceanDomainReport,
    MaskPostprocOceanRunOptions, UnstructuredMeshWriteReport,
};
use earthmesh_project::{MeshCellKind, MeshDomainKind};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

/// Admit the final Earth mesh before publishing auxiliary files.
/// Supply `Some(2)` for an unmasked global sphere, `None` for regional/masked output.
pub fn run_final_mask_postproc_earth_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocEarthRunOptions<'_>,
    expected_euler_characteristic: Option<isize>,
) -> io::Result<MaskPostprocEarthDomainReport> {
    let quality_dir = begin_final_delivery(plan)?;
    let (report, quality) = run_mask_postproc_earth_domain_checked(plan, options, |grid| {
        admit(plan, grid, &quality_dir, expected_euler_characteristic)
    })?;
    record(
        plan,
        &quality_dir,
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
    let quality_dir = begin_final_delivery(plan)?;
    let (report, quality) = run_mask_postproc_land_domain_checked(plan, options, |grid| {
        admit(plan, grid, &quality_dir, None)
    })?;
    record(
        plan,
        &quality_dir,
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
    let quality_dir = begin_final_delivery(plan)?;
    let (report, quality) = run_mask_postproc_ocean_domain_checked(plan, options, |grid| {
        admit(plan, grid, &quality_dir, None)
    })?;
    let mut auxiliary = BTreeMap::new();
    if let Some(obc) = &report.obc {
        auxiliary.insert("obc", obc.output.clone());
    }
    if let Some(obcv2) = &report.obcv2 {
        auxiliary.insert("obcv2", obcv2.output.clone());
    }
    record(plan, &quality_dir, &quality, &auxiliary)?;
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

fn begin_final_delivery(plan: &MaskPostprocDomainIoPlan) -> io::Result<PathBuf> {
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
    crate::project_delivery::retire_delivery_record(&quality_dir.join("legacy_delivery.json"))?;
    let mut outputs = vec![plan.result_gridfile.clone()];
    outputs.extend(
        [&plan.patchtype_output, &plan.obc_output, &plan.obcv2_output]
            .into_iter()
            .flatten()
            .cloned(),
    );
    if plan.mesh_type == "earthmesh" {
        outputs.push(plan.file_dir.join("result/earthmesh_info.nc4"));
    }
    let mut seen = HashSet::new();
    for (index, output) in outputs.iter().enumerate() {
        for input in [&plan.source_gridfile, &plan.contain_domain] {
            crate::atomic_output::validate_output_path(input, output)?;
        }
        let resolved = fs::canonicalize(
            output
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?
        .join(output.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "delivery output needs a filename",
            )
        })?);
        if !seen.insert(resolved) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "final grid and auxiliary outputs must be distinct",
            ));
        }
        for previous in &outputs[..index] {
            if previous.exists() {
                crate::atomic_output::validate_output_path(previous, output)?;
            }
        }
    }
    Ok(quality_dir)
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
    crate::project_quality::admit_final_gridfile(&spec, &grid.output, quality_dir, None)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn record(
    plan: &MaskPostprocDomainIoPlan,
    quality_dir: &Path,
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
    crate::project_delivery::write_delivery_record(
        serde_json::json!({
            "kind": "earthmesh_legacy_delivery",
            "target": {"kind": kind, "cell": cell_kind(plan)?},
            "capability": "native_and_auxiliary",
            "source_mode_grid": plan.mode_grid,
            "skipped_reason": "Native grid and auxiliary files only; no specialized model adapter was run",
        }),
        &plan.result_gridfile,
        &quality_dir.join("quality_summary.json"),
        quality.verdict,
        &BTreeMap::new(),
        auxiliary,
        &quality_dir.join("legacy_delivery.json"),
    )?;
    Ok(())
}
