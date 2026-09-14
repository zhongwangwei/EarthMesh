//! Final-only simple clip/carve handoff. The mother is an unchecked carrier;
//! regional admission applies only after whole-cell extraction.
use super::{
    carrier::{generate_gridinit_carrier, gridinit_sizes},
    global::preserve_final_workspace,
    landtype::landtype_gridnum_perdegree,
    regional::{apply_base_clip_and_carve, base_carve_path, base_raw_parent_path},
};
use crate::{project_delivery::LegacyDeliveryStage, MkgrdGridinitRunReport};
use earthmesh_core::EarthmeshConfig;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn run_simple_final_base(
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
    let outputs = [&published as &Path, &raw_parent];
    let stage = LegacyDeliveryStage::new(&inputs, &outputs, &quality_dir)?;
    preserve_final_workspace(&mut plan, &inputs, &outputs, workdir)?;
    let region = crate::read_method_c_domain_region(config)?;
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
    apply_base_clip_and_carve(&mut report, region.as_ref(), landtype_gpd, private_dir)?;
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
    stage.publish(
        serde_json::json!({
            "kind": "earthmesh_legacy_delivery",
            "target": {"cell": cell_kind},
            "capability": "native_and_auxiliary",
            "source_mesh_type": config.mesh_type,
            "source_mode_grid": config.mode_grid,
            "skipped_reason": "Native clipped/carved base only; raw_parent is unchecked ancestry, not a model adapter result",
        }),
        &published, quality.verdict, &BTreeMap::new(),
        &BTreeMap::from([("raw_parent", raw_parent.clone())]),
    )?;
    report.gridfile.output = published;
    raw.output = raw_parent;
    Ok(report)
}
