use crate::apply_workspace_and_mask_operations;
use crate::MkgrdGridinitRunReport;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, MkgrdWorkspacePlan};

/// Run the Rust replacement path for the initial global `mkgrd.x` gridinit branch.
///
/// This mirrors the branch where `mode_grid` is `hex`/`tri` and `mode_file` does
/// not exist: parse the mkgrd namelist, apply the read_nl workspace/mask plan,
/// generate the in-memory global grid, and write
/// `gridfile/gridfile_NXP####_01_<mode_grid>.nc4`. Existing EarthMesh, MPAS,
/// FVCOM and IAP-Ocean mode files use the same import adapters. This is a raw
/// carrier API: final admission belongs to the selected delivery handoff.
pub fn run_mkgrd_gridinit_global_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
) -> io::Result<MkgrdGridinitRunReport> {
    run_mkgrd_gridinit_global(namelist_source.as_ref(), workdir.as_ref(), max_tris, false)
}

/// Only the standalone, unmasked global base handoff sets `final_delivery`.
/// Project, regional extraction and refinement retain their unchecked carriers.
pub(crate) fn run_mkgrd_gridinit_global(
    namelist_source: &Path,
    workdir: &Path,
    max_tris: usize,
    final_delivery: bool,
) -> io::Result<MkgrdGridinitRunReport> {
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let (nxp, _) = super::carrier::gridinit_sizes(&config)?;

    let mut plan = config.read_nl_workspace_plan(None);
    // Inline Project geometry is consumed by the Method-C region adapters and
    // subsequent regional clip; it is not a file prefix for legacy Mask_make.
    plan.mask_operations
        .retain(|operation| !operation.mask_fprefix.trim().starts_with("inline:"));
    let mode_file = PathBuf::from(config.mode_file.trim());
    let mut output_dir = PathBuf::from(config.file_dir());
    let mut delivery = None;
    if final_delivery {
        // This branch publishes the full sphere, not the masked/refined carrier.
        if !config.mask_domain_global {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "final base delivery requires an unmasked global grid",
            ));
        }
        output_dir = crate::workspace_apply::validate_read_nl_workspace_plan(
            &plan,
            namelist_source,
            workdir,
        )?;
        let published = crate::gridfile_output_path(&output_dir, nxp, 1, &config.mode_grid);
        let quality_dir = published
            .parent()
            .unwrap()
            .join("final_quality")
            .join(published.file_stem().unwrap());
        let mut inputs = vec![namelist_source.to_path_buf(), mode_file.clone()];
        let patch_sources = config
            .mask_patch_on
            .then(|| super::patch_delivery::discover_patch_sources(&config))
            .transpose();
        if let Ok(Some(sources)) = &patch_sources {
            inputs.extend(sources.iter().cloned());
            inputs.sort();
            inputs.dedup();
        }
        let input_refs = inputs.iter().map(PathBuf::as_path).collect::<Vec<_>>();
        let mut stage = crate::project_delivery::LegacyDeliveryStage::new(
            &input_refs,
            &[&published],
            &quality_dir,
        )?;
        patch_sources?;
        preserve_final_workspace(&mut plan, &input_refs, &[&published], workdir)?;
        let patch_delivery = if config.mask_patch_on {
            let patch = super::patch_delivery::stage_patch_preprocessing(
                &mut stage,
                &plan,
                &output_dir,
                namelist_source,
                workdir,
                &input_refs,
            )?;
            if let Some(patch) = &patch {
                let patch_outputs = patch
                    .outputs
                    .iter()
                    .map(PathBuf::as_path)
                    .collect::<Vec<_>>();
                preserve_final_workspace(&mut plan, &input_refs, &patch_outputs, workdir)?;
            }
            plan.mask_operations
                .retain(|operation| operation.mask_select != "mask_patch");
            patch
        } else {
            None
        };
        let staged = stage.path(&published)?;
        // Existing import converters append gridfile/<name> to file_dir.
        // Redirect only their output root; workspace and inputs stay canonical.
        output_dir = staged
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        delivery = Some((stage, published, staged, quality_dir, patch_delivery));
    }
    let mut workspace_mask =
        apply_workspace_and_mask_operations(&plan, namelist_source, workdir, 9, false)?;

    let (mut gridfile, runtime_state) =
        super::carrier::generate_gridinit_carrier(&config, &output_dir, max_tris)?;

    if let Some((stage, published, staged, quality_dir, patch_delivery)) = delivery {
        if let Some(patch) = &patch_delivery {
            workspace_mask
                .mask_reports
                .extend(patch.mask_reports.clone());
            workspace_mask.mask_counts = patch.mask_counts.clone();
        }
        fs::rename(&gridfile.output, &staged)?;
        let cell_kind = if config.mode_grid == "tri" {
            earthmesh_project::MeshCellKind::Tri
        } else {
            earthmesh_project::MeshCellKind::Hex
        };
        let quality = crate::project_quality::admit_staged_final_gridfile(
            &crate::project_quality::FinalAdmissionSpec {
                cell_kind,
                expected_euler_characteristic: Some(2),
                thresholds: earthmesh_quality::QualityThresholds::default(),
                repair_level_cap: None,
            },
            &staged,
            &published,
            &quality_dir,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let patch_entries = patch_delivery
            .as_ref()
            .map(|patch| patch.auxiliary_entries())
            .unwrap_or_default();
        let mut auxiliary = BTreeMap::<&str, PathBuf>::new();
        for (key, output) in &patch_entries {
            auxiliary.insert(key.as_str(), output.clone());
        }
        stage.publish(
            serde_json::json!({
                "kind": "earthmesh_legacy_delivery",
                "target": {"cell": cell_kind},
                "capability": if auxiliary.is_empty() { "native_only" } else { "native_and_auxiliary" },
                "patch_preprocessing_only": config.mask_patch_on,
                "patch_applied_to_geometry": false,
                "source_mesh_type": config.mesh_type,
                "source_mode_grid": config.mode_grid,
                "skipped_reason": if config.mask_patch_on {
                    "Native global base grid plus patch preprocessing caches; patch masks are Area_judge inputs and are not applied to base geometry"
                } else {
                    "Native global base grid only; no specialized model adapter was run"
                },
            }),
            &published,
            quality.verdict,
            &BTreeMap::new(),
            &auxiliary,
        )?;
        gridfile.output = published;
    }

    Ok(MkgrdGridinitRunReport {
        config,
        runtime_state,
        workspace_mask,
        raw_output: None,
        gridfile,
        fvcom_2dm: None,
    })
}

/// Preserve delivery and immutable input bytes during last-attempt workspace setup.
pub(super) fn preserve_final_workspace(
    plan: &mut MkgrdWorkspacePlan,
    inputs: &[&Path],
    outputs: &[&Path],
    workdir: &Path,
) -> io::Result<()> {
    plan.remove_existing_file_dir = false;
    plan.remove_filelists = false;
    let saved_namelist = workdir.join(&plan.namelist_save_path);
    for input in inputs.iter().chain(outputs).filter(|path| path.exists()) {
        crate::atomic_output::validate_output_path(input, &saved_namelist)?;
    }
    Ok(())
}
