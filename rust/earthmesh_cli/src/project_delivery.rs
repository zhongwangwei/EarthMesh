//! Completion records for selected, admitted meshes and their model files.
//! This records current adapter results; it neither validates a mesh nor scans
//! output directories to infer success from historical artifacts.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_project::{ModelFormat, ProjectConfig, ProjectTargetTriple};
use earthmesh_quality::QualityLevel;

pub fn write_project_delivery_report(
    config: &ProjectConfig,
    gridfile: &Path,
    quality_report: &Path,
    verdict: QualityLevel,
    model_artifacts: &BTreeMap<&str, PathBuf>,
) -> io::Result<(PathBuf, &'static str)> {
    let target = ProjectTargetTriple::from(&config.target);
    let skipped_reason = if model_artifacts.is_empty() {
        Some(
            target
                .skipped_adapter_reason()
                .or_else(|| {
                    (config.target.model_format == ModelFormat::CoLM
                        && config.delivery.colm_mesh.is_none())
                    .then_some("CoLM mesh raster delivery is not configured")
                })
                .ok_or_else(|| {
                    io::Error::other("required Project model adapter returned no artifacts")
                })?,
        )
    } else {
        None
    };
    let output = quality_report
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("project_delivery.json");
    write_delivery_record(
        serde_json::json!({
            "kind": "earthmesh_project_delivery",
            "target": target,
            "capability": target.output_delivery(),
            "skipped_reason": skipped_reason,
        }),
        gridfile,
        quality_report,
        verdict,
        model_artifacts,
        &BTreeMap::new(),
        &output,
    )
}

/// Common artifact checks and serialization; each entry point supplies its
/// actual target metadata and keeps its own adapter capability policy.
pub(crate) fn write_delivery_record(
    mut document: serde_json::Value,
    gridfile: &Path,
    quality_report: &Path,
    verdict: QualityLevel,
    model_artifacts: &BTreeMap<&str, PathBuf>,
    auxiliary_artifacts: &BTreeMap<&str, PathBuf>,
    output: &Path,
) -> io::Result<(PathBuf, &'static str)> {
    let status = if model_artifacts.is_empty() {
        "native_only"
    } else {
        "model_delivered"
    };
    let paths = [gridfile, quality_report]
        .into_iter()
        .chain(model_artifacts.values().map(PathBuf::as_path))
        .chain(auxiliary_artifacts.values().map(PathBuf::as_path));
    for path in paths.clone() {
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("delivery artifact is missing: {}", path.display()),
            ));
        }
    }
    for path in paths {
        crate::atomic_output::validate_output_path(path, output)?;
    }
    document["schema_version"] = 1.into();
    document["gridfile"] = serde_json::json!(gridfile);
    document["final_quality"] =
        serde_json::json!({ "report": quality_report, "verdict": verdict.as_str() });
    document["model_delivery_status"] = status.into();
    document["model_artifacts"] = serde_json::json!(model_artifacts);
    if !auxiliary_artifacts.is_empty() {
        document["auxiliary_artifacts"] = serde_json::json!(auxiliary_artifacts);
    }
    let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
    crate::atomic_output::atomic_write(output, |temporary| fs::write(temporary, &bytes))?;
    Ok((output.to_path_buf(), status))
}

/// A previous completion is not evidence for a new final attempt.
pub(crate) fn retire_delivery_record(completion: &Path) -> io::Result<()> {
    match fs::symlink_metadata(completion) {
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "legacy delivery record must be a regular file",
        )),
        Ok(_) => fs::remove_file(completion),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_project::{DomainConfig, MeshIntentPreset, ResolutionSpec};

    #[test]
    fn failed_delivery_record_does_not_publish_or_replace_inputs() {
        let root =
            std::env::temp_dir().join(format!("project_delivery_record_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let gridfile = root.join("native.nc4");
        let quality = root.join("quality_summary.json");
        let output = root.join("project_delivery.json");
        fs::write(&gridfile, b"native").unwrap();
        fs::write(&quality, b"quality").unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(output.join("keep"), b"unrelated").unwrap();
        let mut config = ProjectConfig::scaffold(
            "record",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(3),
        );
        config.target.model_format = ModelFormat::CoLM;
        let error = write_project_delivery_report(
            &config,
            &gridfile,
            &quality,
            QualityLevel::Pass,
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("directory"));
        assert_eq!(fs::read(&gridfile).unwrap(), b"native");
        assert_eq!(fs::read(&quality).unwrap(), b"quality");
        assert_eq!(fs::read(output.join("keep")).unwrap(), b"unrelated");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
        config.target.model_format = ModelFormat::Mpas;
        config.target.cell = earthmesh_project::MeshCellKind::Hex;
        assert!(write_project_delivery_report(
            &config,
            &gridfile,
            &quality,
            QualityLevel::Pass,
            &BTreeMap::new()
        )
        .unwrap_err()
        .to_string()
        .contains("no artifacts"));
        let missing = BTreeMap::from([("mpas_mesh_input", root.join("missing.nc4"))]);
        assert_eq!(
            write_project_delivery_report(
                &config,
                &gridfile,
                &quality,
                QualityLevel::Pass,
                &missing
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::NotFound
        );
        fs::remove_dir_all(root).unwrap();
    }
}
