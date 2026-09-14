//! Completion records for selected, admitted meshes and their model files.
//! This records current adapter results; it neither validates a mesh nor scans
//! output directories to infer success from historical artifacts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use earthmesh_project::{ModelFormat, ProjectConfig, ProjectTargetTriple};
use earthmesh_quality::QualityLevel;

const LEGACY_DELIVERY_RECORD: &str = "legacy_delivery.json";
const QUALITY_FILES: [&str; 6] = [
    "quality_summary.json",
    "quality_summary.csv",
    "worst_cells.geojson",
    "quality_repair_cells.geojson",
    "quality_repair_plan.json",
    "quality_report.md",
];

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

#[derive(Debug)]
pub(crate) struct LegacyDeliveryStage {
    quality_dir: PathBuf,
    marker: PathBuf,
    staged: BTreeMap<PathBuf, PathBuf>,
    staging_roots: Vec<PathBuf>,
    roots_by_parent: BTreeMap<PathBuf, PathBuf>,
    publication_id: String,
}

impl LegacyDeliveryStage {
    pub(crate) fn new(inputs: &[&Path], outputs: &[&Path], quality_dir: &Path) -> io::Result<Self> {
        let marker = quality_dir.join(LEGACY_DELIVERY_RECORD);
        preflight_marker_before_retire(inputs, outputs, &marker)?;
        retire_delivery_record(&marker)?;

        let mut final_outputs = outputs
            .iter()
            .map(|path| (*path).to_path_buf())
            .collect::<Vec<_>>();
        final_outputs.push(marker.clone());
        preflight_delivery_targets(inputs, &final_outputs, quality_dir)?;

        let publication_id = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let mut stage = Self {
            quality_dir: quality_dir.to_path_buf(),
            marker,
            staged: BTreeMap::new(),
            staging_roots: Vec::new(),
            roots_by_parent: BTreeMap::new(),
            publication_id,
        };
        stage.register_outputs_unchecked(&final_outputs)?;

        Ok(stage)
    }

    pub(crate) fn add_outputs(&mut self, inputs: &[&Path], outputs: &[&Path]) -> io::Result<()> {
        let requested = outputs
            .iter()
            .map(|path| (*path).to_path_buf())
            .collect::<Vec<_>>();
        let mut final_outputs = self.staged.keys().cloned().collect::<Vec<_>>();
        final_outputs.extend(requested.iter().cloned());
        preflight_delivery_targets(inputs, &final_outputs, &self.quality_dir)?;
        self.register_outputs_unchecked(&requested)
    }

    pub(crate) fn scratch_dir(&self) -> io::Result<PathBuf> {
        let marker_root = self
            .staged
            .get(&self.marker)
            .and_then(|path| path.parent())
            .and_then(Path::parent)
            .ok_or_else(|| io::Error::other("legacy delivery marker staging root is unavailable"))?
            .to_path_buf();
        for attempt in 0..128u32 {
            let name = if attempt == 0 {
                "work".to_string()
            } else {
                format!("work-{attempt}")
            };
            let path = marker_root.join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create private legacy delivery work directory",
        ))
    }

    fn register_outputs_unchecked(&mut self, outputs: &[PathBuf]) -> io::Result<()> {
        for output in outputs {
            let parent = output
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            crate::ensure_parent_dir(output)?;
            let canonical_parent = fs::canonicalize(parent)?;
            let root = match self.roots_by_parent.get(&canonical_parent) {
                Some(root) => root.clone(),
                None => {
                    let root = create_staging_root(
                        &canonical_parent,
                        &self.publication_id,
                        self.staging_roots.len(),
                    )?;
                    self.staging_roots.push(root.clone());
                    fs::create_dir(root.join("result"))?;
                    self.roots_by_parent.insert(canonical_parent, root.clone());
                    root
                }
            };
            let file_name = output.file_name().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "delivery output needs a filename",
                )
            })?;
            let staged_path = root.join("result").join(file_name);
            if self.staged.insert(output.clone(), staged_path).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("delivery outputs must be distinct: {}", output.display()),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn path(&self, published: &Path) -> io::Result<PathBuf> {
        self.staged.get(published).cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unregistered legacy delivery output: {}",
                    published.display()
                ),
            )
        })
    }

    pub(crate) fn publish(
        &self,
        metadata: serde_json::Value,
        gridfile: &Path,
        verdict: QualityLevel,
        model_artifacts: &BTreeMap<&str, PathBuf>,
        auxiliary_artifacts: &BTreeMap<&str, PathBuf>,
    ) -> io::Result<()> {
        let staged_marker = self.path(&self.marker)?;
        let quality_report = self.quality_dir.join("quality_summary.json");
        let staged_gridfile = self
            .staged
            .get(gridfile)
            .cloned()
            .unwrap_or_else(|| gridfile.to_path_buf());
        let staged_model = stage_artifact_map(&self.staged, model_artifacts)?;
        let staged_auxiliary = stage_artifact_map(&self.staged, auxiliary_artifacts)?;

        write_delivery_record(
            metadata,
            &staged_gridfile,
            &quality_report,
            verdict,
            &staged_model,
            &staged_auxiliary,
            &staged_marker,
        )?;
        remap_staged_delivery_record(
            &staged_marker,
            gridfile,
            &quality_report,
            model_artifacts,
            auxiliary_artifacts,
        )?;

        let mut publications = Vec::<(&Path, &Path)>::new();
        for (published, staged) in &self.staged {
            if published == &self.marker {
                continue;
            }
            publications.push((staged.as_path(), published.as_path()));
        }
        publications.push((staged_marker.as_path(), self.marker.as_path()));
        crate::atomic_output::publish_artifacts(&publications, &[])
    }
}

impl Drop for LegacyDeliveryStage {
    fn drop(&mut self) {
        for root in &self.staging_roots {
            let _ = fs::remove_dir_all(root);
        }
    }
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

fn stage_artifact_map<'a>(
    staged: &BTreeMap<PathBuf, PathBuf>,
    artifacts: &'a BTreeMap<&'a str, PathBuf>,
) -> io::Result<BTreeMap<&'a str, PathBuf>> {
    artifacts
        .iter()
        .map(|(&key, path)| {
            staged
                .get(path)
                .cloned()
                .map(|staged| (key, staged))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unregistered legacy delivery artifact: {}", path.display()),
                    )
                })
        })
        .collect()
}

fn remap_staged_delivery_record(
    record: &Path,
    gridfile: &Path,
    quality_report: &Path,
    model_artifacts: &BTreeMap<&str, PathBuf>,
    auxiliary_artifacts: &BTreeMap<&str, PathBuf>,
) -> io::Result<()> {
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(record)?).map_err(io::Error::other)?;
    document["gridfile"] = serde_json::json!(gridfile);
    if let Some(final_quality) = document["final_quality"].as_object_mut() {
        final_quality.insert("report".to_string(), serde_json::json!(quality_report));
    }
    document["model_artifacts"] = serde_json::json!(model_artifacts);
    if auxiliary_artifacts.is_empty() {
        if let Some(object) = document.as_object_mut() {
            object.remove("auxiliary_artifacts");
        }
    } else {
        document["auxiliary_artifacts"] = serde_json::json!(auxiliary_artifacts);
    }
    let bytes = serde_json::to_vec_pretty(&document).map_err(io::Error::other)?;
    fs::write(record, bytes)
}

fn preflight_marker_before_retire(
    inputs: &[&Path],
    outputs: &[&Path],
    marker: &Path,
) -> io::Result<()> {
    crate::atomic_output::validate_output_destination(marker)?;
    for input in inputs.iter().filter(|path| path.exists()) {
        reject_existing_alias(input, marker)?;
    }
    for output in outputs.iter().filter(|path| path.exists()) {
        reject_existing_alias(output, marker)?;
    }
    Ok(())
}

fn preflight_delivery_targets(
    inputs: &[&Path],
    outputs: &[PathBuf],
    quality_dir: &Path,
) -> io::Result<()> {
    let mut targets = outputs.to_vec();
    targets.extend(QUALITY_FILES.iter().map(|name| quality_dir.join(name)));
    let mut resolved = BTreeSet::new();
    let mut previous = Vec::<PathBuf>::new();
    for target in &targets {
        crate::atomic_output::validate_output_destination(target)?;
        let destination = resolved_destination(target)?;
        if !resolved.insert(destination) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("delivery targets must be distinct: {}", target.display()),
            ));
        }
        for input in inputs.iter().filter(|path| path.exists()) {
            reject_existing_alias(input, target)?;
        }
        for earlier in previous.iter().filter(|path| path.exists()) {
            if target.exists() {
                crate::atomic_output::validate_output_path(earlier, target).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("delivery targets must be distinct: {error}"),
                    )
                })?;
            }
        }
        previous.push(target.clone());
    }
    Ok(())
}

fn create_staging_root(parent: &Path, publication_id: &str, index: usize) -> io::Result<PathBuf> {
    for attempt in 0..128u32 {
        let root = parent.join(format!(
            ".earthmesh-delivery-{publication_id}-{index}-{attempt}"
        ));
        match fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create private legacy delivery staging root",
    ))
}

fn resolved_destination(path: &Path) -> io::Result<PathBuf> {
    crate::ensure_parent_dir(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(
        fs::canonicalize(parent)?.join(path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "delivery output needs a filename",
            )
        })?),
    )
}

fn reject_existing_alias(input: &Path, output: &Path) -> io::Result<()> {
    if !input.exists() || !output.exists() {
        return Ok(());
    }
    crate::atomic_output::validate_output_path(input, output)
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

    #[test]
    fn legacy_stage_missing_staged_output_restores_previous_outputs_and_withdraws_marker() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-rollback-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let result = root.join("result");
        let quality = result.join("final_quality/native");
        fs::create_dir_all(&quality).unwrap();
        let input = root.join("input.nc4");
        let grid = result.join("native.nc4");
        let sidecar = result.join("sidecar.nc4");
        let marker = quality.join("legacy_delivery.json");
        fs::write(&input, b"input").unwrap();
        fs::write(&grid, b"old grid").unwrap();
        fs::write(&sidecar, b"old sidecar").unwrap();
        fs::write(&marker, b"old ready").unwrap();
        fs::write(quality.join("quality_summary.json"), b"quality").unwrap();

        let stage = LegacyDeliveryStage::new(&[&input], &[&grid, &sidecar], &quality).unwrap();
        fs::write(stage.path(&grid).unwrap(), b"new grid").unwrap();

        let error = stage
            .publish(
                serde_json::json!({"kind": "test_delivery"}),
                &grid,
                QualityLevel::Pass,
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .expect_err("missing staged sidecar must fail publication");

        assert!(
            error.to_string().contains("missing")
                || error.to_string().contains("No such file")
                || error.to_string().contains("os error 2"),
            "unexpected error: {error}"
        );
        assert_eq!(fs::read(&grid).unwrap(), b"old grid");
        assert_eq!(fs::read(&sidecar).unwrap(), b"old sidecar");
        assert!(!marker.exists(), "readiness marker must stay withdrawn");
        drop(stage);
        assert!(fs::read_dir(&result).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".earthmesh-delivery")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_stage_groups_staging_by_output_parent_and_preserves_filenames() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-parents-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let input = root.join("input.nc4");
        let a = root.join("a/native.nc4");
        let b = root.join("b/obc.nc4");
        let quality = root.join("q/final");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fs::create_dir_all(&quality).unwrap();
        fs::write(&input, b"input").unwrap();

        let stage = LegacyDeliveryStage::new(&[&input], &[&a, &b], &quality).unwrap();
        let staged_a = stage.path(&a).unwrap();
        let staged_b = stage.path(&b).unwrap();
        let staged_marker = stage.path(&quality.join("legacy_delivery.json")).unwrap();
        assert_eq!(staged_a.file_name().unwrap(), "native.nc4");
        assert_eq!(staged_b.file_name().unwrap(), "obc.nc4");
        assert_ne!(
            staged_a.parent().unwrap().parent(),
            staged_b.parent().unwrap().parent()
        );
        assert_eq!(staged_marker.file_name().unwrap(), "legacy_delivery.json");
        drop(stage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn blocked_output_parent_retires_marker_without_touching_parent_file() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-blocked-parent-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let input = root.join("input.nc4");
        let quality = root.join("quality");
        let blocked_parent = root.join("result");
        let output = blocked_parent.join("native.nc4");
        fs::create_dir_all(&quality).unwrap();
        fs::write(&input, b"input").unwrap();
        fs::write(&blocked_parent, b"sentinel").unwrap();
        let marker = quality.join("legacy_delivery.json");
        fs::write(&marker, b"old ready").unwrap();

        assert!(LegacyDeliveryStage::new(&[&input], &[&output], &quality).is_err());
        assert_eq!(fs::read(&blocked_parent).unwrap(), b"sentinel");
        assert!(
            !marker.exists(),
            "ordinary output failure must retire stale readiness"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn marker_output_hardlink_alias_rejects_before_retiring_marker() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-marker-alias-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let input = root.join("input.nc4");
        let quality = root.join("quality");
        let output = root.join("result/native.nc4");
        fs::create_dir_all(&quality).unwrap();
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&input, b"input").unwrap();
        let marker = quality.join("legacy_delivery.json");
        fs::write(&marker, b"old ready").unwrap();
        fs::hard_link(&marker, &output).unwrap();

        let error = LegacyDeliveryStage::new(&[&input], &[&output], &quality).unwrap_err();

        assert!(
            error.to_string().contains("alias") || error.to_string().contains("same"),
            "{error}"
        );
        assert_eq!(fs::read(&marker).unwrap(), b"old ready");
        assert_eq!(fs::read(&output).unwrap(), b"old ready");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dynamic_outputs_can_be_registered_after_scratch_generation() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-dynamic-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let input = root.join("input.nc4");
        let quality = root.join("quality");
        let grid = root.join("result/final.nc4");
        fs::create_dir_all(&quality).unwrap();
        fs::write(&input, b"input").unwrap();
        let marker = quality.join("legacy_delivery.json");
        fs::write(&marker, b"old ready").unwrap();

        let mut stage = LegacyDeliveryStage::new(&[&input], &[], &quality).unwrap();
        assert!(!marker.exists(), "constructor retires stale readiness");
        let scratch = stage.scratch_dir().unwrap();
        assert!(scratch.is_dir());
        assert!(scratch
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("work"));
        assert!(scratch.starts_with(
            stage
                .path(&marker)
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
        ));

        stage.add_outputs(&[&input], &[&grid]).unwrap();
        let staged_grid = stage.path(&grid).unwrap();
        assert_eq!(staged_grid.file_name().unwrap(), "final.nc4");
        fs::write(&staged_grid, b"new grid").unwrap();
        fs::write(quality.join("quality_summary.json"), b"quality").unwrap();
        stage
            .publish(
                serde_json::json!({"kind": "test_dynamic_delivery"}),
                &grid,
                QualityLevel::Pass,
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .unwrap();

        assert_eq!(fs::read(&grid).unwrap(), b"new grid");
        assert!(marker.is_file());
        drop(stage);
        assert_no_stage_dirs(&root);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dynamic_output_registration_rejects_quality_collision_and_duplicate() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-dynamic-collision-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let input = root.join("input.nc4");
        let quality = root.join("quality");
        fs::create_dir_all(&quality).unwrap();
        fs::write(&input, b"input").unwrap();
        let marker = quality.join("legacy_delivery.json");
        fs::write(&marker, b"old ready").unwrap();

        let mut stage = LegacyDeliveryStage::new(&[&input], &[], &quality).unwrap();
        let quality_summary = quality.join("quality_summary.json");
        let error = stage
            .add_outputs(&[&input], &[&quality_summary])
            .unwrap_err()
            .to_string();
        assert!(error.contains("distinct"), "{error}");
        assert!(
            !marker.exists(),
            "late validation must not restore stale readiness"
        );

        let output = root.join("result/final.nc4");
        stage.add_outputs(&[&input], &[&output]).unwrap();
        let duplicate = stage
            .add_outputs(&[&input], &[&output])
            .unwrap_err()
            .to_string();
        assert!(duplicate.contains("distinct"), "{duplicate}");
        drop(stage);
        assert_no_stage_dirs(&root);
        let _ = fs::remove_dir_all(root);
    }

    fn assert_no_stage_dirs(root: &Path) {
        if !root.exists() {
            return;
        }
        for entry in fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            assert!(
                !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".earthmesh-delivery-"),
                "staging path leaked: {}",
                path.display()
            );
            if path.is_dir() {
                assert_no_stage_dirs(&path);
            }
        }
    }

    #[test]
    fn invalid_nonmarker_output_retires_previous_legacy_marker() {
        let root = std::env::temp_dir().join(format!(
            "legacy-delivery-stage-invalid-output-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let input = root.join("input.nc4");
        let quality = root.join("quality");
        let output_dir = root.join("result/native.nc4");
        fs::create_dir_all(&quality).unwrap();
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(&input, b"input").unwrap();
        let marker = quality.join("legacy_delivery.json");
        fs::write(&marker, b"old ready").unwrap();

        let error = LegacyDeliveryStage::new(&[&input], &[&output_dir], &quality).unwrap_err();

        assert!(error.to_string().contains("directory"), "{error}");
        assert!(
            !marker.exists(),
            "invalid output must retire stale readiness"
        );
        let _ = fs::remove_dir_all(root);
    }
}
