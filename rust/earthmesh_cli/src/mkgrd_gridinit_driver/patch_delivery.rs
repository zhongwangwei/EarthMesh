//! Helpers for final-only `mask_patch_on` preprocessing delivery.
//!
//! Patch masks are legacy Area_judge inputs. They are staged and published as
//! auxiliary caches, but they do not alter standalone base-grid geometry.

use crate::{
    apply_workspace_and_mask_operations, project_delivery::LegacyDeliveryStage, MaskCountState,
    MaskOperationReport,
};
use earthmesh_core::MkgrdWorkspacePlan;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(super) struct PatchPreprocessDelivery {
    pub mask_reports: Vec<MaskOperationReport>,
    pub mask_counts: MaskCountState,
    pub outputs: Vec<PathBuf>,
}

impl PatchPreprocessDelivery {
    pub(super) fn auxiliary_entries(&self) -> Vec<(String, PathBuf)> {
        self.outputs
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, output)| (format!("patch_cache_{index:02}"), output))
            .collect()
    }
}

pub(super) fn discover_patch_sources(
    config: &earthmesh_core::EarthmeshConfig,
) -> io::Result<Vec<PathBuf>> {
    crate::discover_mask_sources(config.mask_patch_fprefix.trim()).map(|discovery| discovery.files)
}

pub(super) fn stage_patch_preprocessing(
    stage: &mut LegacyDeliveryStage,
    plan: &MkgrdWorkspacePlan,
    published_file_dir: &Path,
    namelist_source: &Path,
    workdir: &Path,
    inputs: &[&Path],
) -> io::Result<Option<PatchPreprocessDelivery>> {
    if !plan
        .mask_operations
        .iter()
        .any(|operation| operation.mask_select == "mask_patch")
    {
        return Ok(None);
    }

    let scratch = stage.scratch_dir()?;
    let mut patch_plan = private_patch_plan(plan, &scratch);
    super::global::preserve_final_workspace(&mut patch_plan, inputs, &[], workdir)?;
    let mut report =
        apply_workspace_and_mask_operations(&patch_plan, namelist_source, workdir, 9, false)?;

    let mut published_outputs = Vec::new();
    for mask_report in &report.mask_reports {
        for output in &mask_report.outputs {
            published_outputs.push(published_mask_output(&scratch, published_file_dir, output)?);
        }
    }
    let output_refs = published_outputs
        .iter()
        .map(PathBuf::as_path)
        .collect::<Vec<_>>();
    stage.add_outputs(inputs, &output_refs)?;

    for (private, published) in report
        .mask_reports
        .iter()
        .flat_map(|mask_report| mask_report.outputs.iter())
        .zip(published_outputs.iter())
    {
        fs::copy(private, stage.path(published)?)?;
    }
    remap_mask_reports(&mut report.mask_reports, &scratch, published_file_dir)?;

    Ok(Some(PatchPreprocessDelivery {
        mask_reports: report.mask_reports,
        mask_counts: report.mask_counts,
        outputs: published_outputs,
    }))
}

fn private_patch_plan(plan: &MkgrdWorkspacePlan, scratch: &Path) -> MkgrdWorkspacePlan {
    let file_dir = with_trailing_separator(scratch);
    let mut private = MkgrdWorkspacePlan {
        file_dir: file_dir.clone(),
        remove_existing_file_dir: false,
        remove_filelists: false,
        directories_to_create: ["contain", "gridfile", "patchtype", "result", "tmpfile"]
            .into_iter()
            .map(|subdir| format!("{file_dir}{subdir}/"))
            .collect(),
        namelist_save_path: format!("{file_dir}result/namelist.save"),
        mask_operations: plan
            .mask_operations
            .iter()
            .filter(|operation| operation.mask_select == "mask_patch")
            .cloned()
            .collect(),
    };
    private
        .mask_operations
        .retain(|operation| !operation.mask_fprefix.trim().starts_with("inline:"));
    private
}

fn with_trailing_separator(path: &Path) -> String {
    let mut value = path.display().to_string();
    if !value.ends_with(std::path::MAIN_SEPARATOR) {
        value.push(std::path::MAIN_SEPARATOR);
    }
    value
}

fn published_mask_output(
    scratch: &Path,
    published_file_dir: &Path,
    private_output: &Path,
) -> io::Result<PathBuf> {
    let relative = private_output.strip_prefix(scratch).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "patch output {} was not written under scratch {}",
                private_output.display(),
                scratch.display()
            ),
        )
    })?;
    Ok(published_file_dir.join(relative))
}

fn remap_mask_reports(
    reports: &mut [MaskOperationReport],
    scratch: &Path,
    published_file_dir: &Path,
) -> io::Result<()> {
    for report in reports {
        for output in &mut report.outputs {
            *output = published_mask_output(scratch, published_file_dir, output)?;
        }
    }
    Ok(())
}
