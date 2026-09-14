//! Final-only regional base handoff. The mother is an unchecked carrier;
//! regional admission applies only after whole-cell extraction.
use super::{
    carrier::{generate_gridinit_carrier, gridinit_sizes},
    global::preserve_final_workspace,
    landtype::landtype_gridnum_perdegree,
    regional::{
        apply_base_clip_and_carve, base_carve_path, base_raw_parent_path,
        clean_regional_ocean_close_points,
    },
};
use crate::{project_delivery::LegacyDeliveryStage, MkgrdGridinitRunReport};
use earthmesh_core::EarthmeshConfig;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn run_final_base(
    namelist_source: &Path,
    workdir: &Path,
    max_tris: usize,
    config: &EarthmeshConfig,
    carve_landtype: bool,
) -> io::Result<MkgrdGridinitRunReport> {
    let (nxp, _) = gridinit_sizes(config)?;
    let mode_grid = config.mode_grid.trim();
    let mut plan = config.read_nl_workspace_plan(None);
    let file_dir =
        crate::workspace_apply::validate_read_nl_workspace_plan(&plan, namelist_source, workdir)?;
    let published = if carve_landtype {
        base_carve_path(&file_dir, nxp, mode_grid, config.mesh_type.trim())
    } else {
        crate::gridfile_output_path(&file_dir, nxp, 1, mode_grid)
    };
    // Never replace a previously admitted global base with an unchecked mother.
    // Keep ancestry in tmpfile even for a global land/sea-only carve.
    let raw_parent = base_raw_parent_path(&file_dir, nxp, mode_grid);
    let quality_dir = published
        .parent()
        .unwrap()
        .join("final_quality")
        .join(published.file_stem().unwrap());
    let mut inputs = vec![
        namelist_source.to_path_buf(),
        PathBuf::from(config.mode_file.trim()),
    ];
    if carve_landtype {
        inputs.push(PathBuf::from(config.landtype_file.trim()));
    }
    if !config.mask_domain_global && !config.mask_domain_fprefix.trim().starts_with("inline:") {
        inputs.extend(crate::discover_mask_sources(config.mask_domain_fprefix.trim())?.files);
    }
    let inputs = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    // A configured close can become a cap/union. Only the actual polygon owns
    // the clean-ocean boundary outputs. Delay parse errors until Stage retires
    // readiness, without letting malformed geometry overwrite prior artifacts.
    let region = crate::read_method_c_domain_region(config);
    let clean_ocean = clean_regional_ocean_close_points(
        region.as_ref().ok().and_then(Option::as_ref),
        config.mesh_type.trim(),
        mode_grid,
        carve_landtype,
    )
    .is_some();
    let mut auxiliary = BTreeMap::from([("raw_parent", raw_parent.clone())]);
    if clean_ocean {
        auxiliary.insert("obc", crate::obc_boundary_output_path(&file_dir, false));
        auxiliary.insert("obcv2", crate::obcv2_boundary_output_path(&file_dir, false));
    }
    let fvcom =
        (clean_ocean && config.output_format.trim() == "FVCOM" && !config.defer_model_exports)
            .then(|| crate::fvcom_mesh_2dm_output_path(&file_dir));
    let outputs = std::iter::once(published.as_path())
        .chain(auxiliary.values().map(PathBuf::as_path))
        .chain(fvcom.as_deref())
        .collect::<Vec<_>>();
    let stage = LegacyDeliveryStage::new(&inputs, &outputs, &quality_dir)?;
    preserve_final_workspace(&mut plan, &inputs, &outputs, workdir)?;
    let region = region?;
    let landtype_gpd = carve_landtype
        .then(|| landtype_gridnum_perdegree(Path::new(config.landtype_file.trim())))
        .transpose()?;
    if region.is_none() && !carve_landtype {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "final regional base needs a domain or landtype carve",
        ));
    }
    // The typed region above is the input to the shared clipping kernel.
    // Legacy Mask_make caches are redundant here; raw callers still write them.
    plan.mask_operations.clear();
    let workspace_mask =
        crate::apply_workspace_and_mask_operations(&plan, namelist_source, workdir, 9, false)?;
    let staged = stage.path(&published)?;
    let private_dir = staged.parent().and_then(Path::parent).unwrap();
    let (gridfile, runtime_state) = generate_gridinit_carrier(config, private_dir, max_tris)?;
    let mut report = MkgrdGridinitRunReport {
        config: config.clone(),
        runtime_state,
        workspace_mask,
        raw_output: None,
        gridfile,
        fvcom_2dm: None,
    };
    if let Some(close_points) = clean_regional_ocean_close_points(
        region.as_ref(),
        config.mesh_type.trim(),
        mode_grid,
        carve_landtype,
    ) {
        let clean = crate::write_clean_regional_ocean_gridfile(
            &report.gridfile.output,
            close_points,
            Path::new(config.landtype_file.trim()),
            nxp,
            landtype_gpd.expect("clean ocean needs landtype resolution"),
            config.mask_sea_ratio,
            private_dir,
        )?;
        report.raw_output = Some(report.gridfile.clone());
        report.gridfile = crate::unstructured_mesh_write_report_from_file(&clean.result_gridfile)?;
        for (key, source) in [("obc", clean.obc_output), ("obcv2", clean.obcv2_output)] {
            let source = source.expect("clean ocean TRI plan owns both boundary files");
            let destination = stage.path(&auxiliary[key])?;
            if source != destination {
                fs::copy(source, destination)?;
            }
        }
    } else {
        apply_base_clip_and_carve(&mut report, region.as_ref(), landtype_gpd, private_dir)?;
    }
    let raw = report
        .raw_output
        .as_mut()
        .ok_or_else(|| io::Error::other("regional base did not retain its full parent"))?;
    // Copy into each destination's own staging root; parents may be on
    // different filesystems. Final publication still uses same-filesystem rename.
    fs::copy(&raw.output, stage.path(&raw_parent)?)?;
    if report.gridfile.output != staged {
        fs::copy(&report.gridfile.output, &staged)?;
    }
    let cell_kind = if mode_grid == "tri" {
        earthmesh_project::MeshCellKind::Tri
    } else {
        earthmesh_project::MeshCellKind::Hex
    };
    let quality = crate::project_quality::admit_staged_final_gridfile(
        &crate::project_quality::FinalAdmissionSpec {
            cell_kind,
            expected_euler_characteristic: None,
            thresholds: earthmesh_quality::QualityThresholds::default(),
            repair_level_cap: None,
        },
        &staged,
        &published,
        &quality_dir,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut model_artifacts = BTreeMap::new();
    if let Some(output) = fvcom {
        // The clean writer has finished metadata rewrites and embedded OBC.
        // Export exactly this admitted native, never its unchecked mother.
        let mut model = crate::regional_gridfile_writers::write_fvcom_from_final_gridfile(
            &staged,
            &stage.path(&output)?,
        )?;
        model.output = output.clone();
        model_artifacts.insert("fvcom_2dm", output);
        report.fvcom_2dm = Some(model);
    }
    let skipped_reason = if !model_artifacts.is_empty() {
        None
    } else if config.defer_model_exports {
        Some("Model exports deferred; native admission and auxiliary delivery still required")
    } else {
        Some("Native regional base and auxiliaries only; no specialized model adapter was run")
    };
    stage.publish(
        serde_json::json!({
            "kind": "earthmesh_legacy_delivery",
            "target": {"cell": cell_kind},
            "capability": if model_artifacts.is_empty() { "native_and_auxiliary" } else { "full" },
            "requested_output_format": config.output_format,
            "source_mesh_type": config.mesh_type,
            "source_mode_grid": config.mode_grid,
            "skipped_reason": skipped_reason,
        }),
        &published,
        quality.verdict,
        &model_artifacts,
        &auxiliary,
    )?;
    report.gridfile.output = published;
    raw.output = raw_parent;
    Ok(report)
}
