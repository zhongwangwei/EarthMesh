//! Read the CLI completion record, never infer delivery from directory contents.

use earthmesh_project::{ModelFormat, ProjectConfig, ProjectTargetTriple};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn scoped_file(root: &Path, value: Option<&str>) -> Result<PathBuf, String> {
    let value = value
        .filter(|s| !s.trim().is_empty())
        .ok_or("Project delivery is missing an artifact path")?;
    let path = root
        .join(value)
        .canonicalize()
        .map_err(|e| format!("Project delivery artifact {value}: {e}"))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(format!(
            "Project delivery artifact is not a file within this run: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid Project delivery JSON {}: {e}", path.display()))
}

pub(crate) fn read_project_delivery(
    cfg: &ProjectConfig,
    run_dir: &Path,
    final_gridfile: Option<&str>,
    reported: Option<&str>,
    ok: bool,
) -> Result<Option<Value>, String> {
    // Old engines can complete a native mesh without reporting actual model delivery.
    let Some(reported) = reported.filter(|_| ok) else {
        return Ok(None);
    };
    let root = run_dir
        .canonicalize()
        .map_err(|e| format!("resolve run directory: {e}"))?;
    let path = scoped_file(&root, Some(reported))?;
    let mut report = read_json(&path)?;
    let target = ProjectTargetTriple::from(&cfg.target);
    if report["schema_version"] != 1
        || report["kind"] != "earthmesh_project_delivery"
        || report["target"] != json!(target)
        || report["capability"] != json!(target.output_delivery())
    {
        return Err("Project delivery schema or configured target does not match this run".into());
    }
    let gridfile = scoped_file(&root, report["gridfile"].as_str())?;
    if gridfile != scoped_file(&root, final_gridfile)? {
        return Err("Project delivery does not reference the selected final gridfile".into());
    }
    let quality_path = scoped_file(&root, report["final_quality"]["report"].as_str())?;
    let quality = read_json(&quality_path)?;
    if quality["kind"] != "earthmesh_mesh_quality"
        || !matches!(quality["verdict"].as_str(), Some("pass" | "warn" | "fail"))
        || quality["verdict"] != report["final_quality"]["verdict"]
        || scoped_file(&root, quality["mesh_name"].as_str())? != gridfile
    {
        return Err(
            "Project delivery final quality does not match the selected mesh/verdict".into(),
        );
    }
    let native_only = target.skipped_adapter_reason().is_some()
        || (cfg.target.model_format == ModelFormat::CoLM && cfg.delivery.colm_mesh.is_none());
    let expected_status = if native_only {
        "native_only"
    } else {
        "model_delivered"
    };
    if report["model_delivery_status"] != expected_status
        || (native_only
            && report["skipped_reason"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty()))
        || (!native_only && !report["skipped_reason"].is_null())
    {
        return Err("Project delivery status/reason conflicts with the configured delivery".into());
    }
    let artifacts = report["model_artifacts"]
        .as_object_mut()
        .ok_or("Project delivery model_artifacts must be an object")?;
    let expected: &[&str] = if native_only {
        &[]
    } else {
        match cfg.target.model_format {
            ModelFormat::CoLM => &["colm_mesh_input"],
            ModelFormat::Icon => &["icon_mesh_input"],
            ModelFormat::Fvcom => &["fvcom_mesh_input"],
            ModelFormat::MpasSimple => &["mpas_mesh_input"],
            ModelFormat::Mpas | ModelFormat::MpasOcean => &["mpas_mesh_input", "mpas_graph_info"],
        }
    };
    if artifacts.len() != expected.len() || expected.iter().any(|key| !artifacts.contains_key(*key))
    {
        return Err("Project delivery artifact keys conflict with its target/status".into());
    }
    for value in artifacts.values_mut() {
        *value = json!(scoped_file(&root, value.as_str())?);
    }
    report["gridfile"] = json!(gridfile);
    report["final_quality"]["report"] = json!(quality_path);
    Ok(Some(json!({ "report_path": path, "report": report })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_project::{DomainConfig, MeshIntentPreset, ResolutionSpec};

    #[test]
    fn delivery_record_requires_current_selected_mesh_quality_status_and_scoped_files() {
        let root = std::env::temp_dir().join(format!("gui_delivery_{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut cfg = ProjectConfig::scaffold(
            "delivery",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(3),
        );
        cfg.target.model_format = ModelFormat::CoLM;
        fs::write(root.join("native.nc4"), "native").unwrap();
        fs::write(root.join("other.nc4"), "other").unwrap();
        let quality =
            json!({"kind":"earthmesh_mesh_quality", "mesh_name":"native.nc4", "verdict":"warn"});
        fs::write(root.join("quality.json"), quality.to_string()).unwrap();
        let report = json!({"schema_version":1,"kind":"earthmesh_project_delivery",
            "target":ProjectTargetTriple::from(&cfg.target),"capability":"full","gridfile":"native.nc4",
            "final_quality":{"report":"quality.json","verdict":"warn"},
            "model_delivery_status":"native_only","model_artifacts":{},"skipped_reason":"CoLM mesh raster delivery is not configured"});
        let read = |document: &Value, final_gridfile, ok| {
            fs::write(root.join("delivery.json"), document.to_string()).unwrap();
            read_project_delivery(&cfg, &root, final_gridfile, Some("delivery.json"), ok)
        };
        let bundle = read(&report, Some("native.nc4"), true).unwrap().unwrap();
        assert_eq!(bundle["report"]["gridfile"], json!(root.join("native.nc4")));
        assert!(
            read_project_delivery(&cfg, &root, Some("native.nc4"), None, true)
                .unwrap()
                .is_none()
        );
        assert!(read(&report, Some("native.nc4"), false).unwrap().is_none());
        assert!(read(&report, None, true).is_err());
        assert!(read(&report, Some("other.nc4"), true).is_err());
        for (pointer, value) in [
            ("/schema_version", json!(2)),
            ("/kind", json!("unknown")),
            ("/target/kind", json!("wrong")),
            ("/capability", json!("grid_only")),
            ("/gridfile", json!("missing.nc4")),
            ("/final_quality/report", json!(".")),
            ("/final_quality/verdict", json!("pass")),
            ("/model_delivery_status", json!("model_delivered")),
            ("/model_artifacts", json!({"stale":"other.nc4"})),
            ("/skipped_reason", Value::Null),
        ] {
            let mut invalid = report.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                read(&invalid, Some("native.nc4"), true).is_err(),
                "{pointer}"
            );
        }
        let mut wrong_quality = quality.clone();
        wrong_quality["mesh_name"] = json!("other.nc4");
        fs::write(root.join("quality.json"), wrong_quality.to_string()).unwrap();
        assert!(read(&report, Some("native.nc4"), true).is_err());
        assert!(scoped_file(&root, root.parent().unwrap().to_str()).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::current_exe().unwrap(), root.join("escape"))
                .unwrap();
            assert!(scoped_file(&root, Some("escape")).is_err());
        }
        fs::write(root.join("quality.json"), quality.to_string()).unwrap();
        cfg.target.cell = earthmesh_project::MeshCellKind::Hex;
        cfg.target.model_format = ModelFormat::Mpas;
        let mut delivered = report.clone();
        delivered["target"] = json!(ProjectTargetTriple::from(&cfg.target));
        delivered["model_delivery_status"] = json!("model_delivered");
        delivered["skipped_reason"] = Value::Null;
        delivered["model_artifacts"] =
            json!({"mpas_mesh_input":"other.nc4", "mpas_graph_info":"graph.info"});
        fs::write(root.join("graph.info"), "graph").unwrap();
        fs::write(root.join("delivery.json"), delivered.to_string()).unwrap();
        assert!(read_project_delivery(
            &cfg,
            &root,
            Some("native.nc4"),
            Some("delivery.json"),
            true
        )
        .unwrap()
        .is_some());
        for artifacts in [
            json!({"junk":"native.nc4"}),
            json!({"mpas_graph_info":"graph.info"}),
            json!({"mpas_mesh_input":"missing.nc4", "mpas_graph_info":"graph.info"}),
            json!({"mpas_mesh_input":1, "mpas_graph_info":"graph.info"}),
            json!({"mpas_mesh_input":"other.nc4", "mpas_graph_info":"graph.info", "extra":"native.nc4"}),
        ] {
            delivered["model_artifacts"] = artifacts;
            fs::write(root.join("delivery.json"), delivered.to_string()).unwrap();
            assert!(read_project_delivery(
                &cfg,
                &root,
                Some("native.nc4"),
                Some("delivery.json"),
                true
            )
            .is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
