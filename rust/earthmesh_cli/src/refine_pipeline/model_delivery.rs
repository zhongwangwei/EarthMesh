//! Model-format delivery for standalone refined final grids.
//!
//! The raw refine pipeline stays native-first.  This helper is called only
//! after a private final gridfile has already passed the shared spherical final
//! admission, so it chooses only model adapters that can consume that admitted
//! native file without guessing missing producer metadata.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_core::EarthmeshConfig;
use earthmesh_project::{MeshCellKind, MeshDomainKind, ModelFormat, ProjectTargetTriple};
use earthmesh_quality::MeshQualityReport;

pub(super) fn write_available_models(
    config: &EarthmeshConfig,
    gridfile: &Path,
    parent: Option<&Path>,
    quality: &MeshQualityReport,
) -> io::Result<(BTreeMap<&'static str, PathBuf>, Option<&'static str>)> {
    if legacy_unset(config.output_format.trim()) {
        return Ok((
            BTreeMap::new(),
            Some("No model output_format was requested; native final admission still completed"),
        ));
    }
    let target = target_triple(config)?;
    if config.defer_model_exports {
        return Ok((
            BTreeMap::new(),
            Some("Model exports deferred; native final admission still completed"),
        ));
    }
    if let Some(reason) = target.skipped_adapter_reason() {
        return Ok((BTreeMap::new(), Some(reason)));
    }

    match (target.cell, target.model_format) {
        (MeshCellKind::Hex, ModelFormat::Mpas | ModelFormat::MpasOcean | ModelFormat::MpasSimple) => {
            write_mpas_models(gridfile, parent, quality, target.model_format)
        }
        (MeshCellKind::Tri, ModelFormat::Fvcom) => write_fvcom_model(gridfile, quality),
        (MeshCellKind::Tri, ModelFormat::Icon) => write_icon_model(config, gridfile, parent, quality),
        (_, ModelFormat::CoLM) => Ok((
            BTreeMap::new(),
            Some("CoLM standalone refined delivery has no raster delivery specification; native grid and coupling metadata only"),
        )),
        _ => Ok((BTreeMap::new(), target.skipped_adapter_reason())),
    }
}

fn write_mpas_models(
    gridfile: &Path,
    parent: Option<&Path>,
    quality: &MeshQualityReport,
    format: ModelFormat,
) -> io::Result<(BTreeMap<&'static str, PathBuf>, Option<&'static str>)> {
    if crate::mpas_gridfile_context::read_mpas_gridfile_context(gridfile)?.is_none() {
        return Ok((
            BTreeMap::new(),
            Some("MPAS model export skipped because the selected native gridfile has no persisted MPAS width context"),
        ));
    }
    let regional = quality.topology.boundary_edge_count > 0;
    let parent = if regional {
        parent.filter(|candidate| *candidate != gridfile)
    } else {
        None
    };
    if regional && parent.is_none() {
        return Ok((
            BTreeMap::new(),
            Some(
                "MPAS regional model export skipped because no closed parent gridfile was provided",
            ),
        ));
    }
    if let Some(parent) = parent {
        if crate::mpas_gridfile_context::read_mpas_gridfile_context(parent)?.is_none() {
            return Ok((
                BTreeMap::new(),
                Some("MPAS regional model export skipped because the parent gridfile has no persisted MPAS width context"),
            ));
        }
    }

    let output = standard_output_dir(gridfile, "MPAS")?;
    let (mesh, graph) = if let Some(parent) = parent {
        crate::mpas_gridfile_writers::write_mpas_from_final_gridfile_with_parent(
            gridfile, parent, &output, format,
        )?
    } else {
        crate::mpas_gridfile_writers::write_mpas_from_final_gridfile(gridfile, &output, format)?
    };
    let mut artifacts = BTreeMap::from([("mpas_mesh_input", mesh)]);
    if let Some(graph) = graph {
        artifacts.insert("mpas_graph_info", graph);
    }
    Ok((artifacts, None))
}

fn write_fvcom_model(
    gridfile: &Path,
    quality: &MeshQualityReport,
) -> io::Result<(BTreeMap<&'static str, PathBuf>, Option<&'static str>)> {
    if quality.topology.boundary_edge_count > 0
        && crate::obc_boundary_io::read_gridfile_obc_order(gridfile)?.is_none()
    {
        return Ok((
            BTreeMap::new(),
            Some(
                "FVCOM model export skipped because a bounded TRI mesh needs embedded OBC context",
            ),
        ));
    }
    let output = standard_output_file(gridfile, "FVCOM", "2dm")?;
    let report =
        crate::regional_gridfile_writers::write_fvcom_from_final_gridfile(gridfile, &output)?;
    Ok((BTreeMap::from([("fvcom_mesh_input", report.output)]), None))
}

fn write_icon_model(
    config: &EarthmeshConfig,
    gridfile: &Path,
    parent: Option<&Path>,
    quality: &MeshQualityReport,
) -> io::Result<(BTreeMap<&'static str, PathBuf>, Option<&'static str>)> {
    let regional = quality.topology.boundary_edge_count > 0;
    let parent = if regional {
        parent.filter(|candidate| *candidate != gridfile)
    } else {
        None
    };
    if regional && parent.is_none() {
        return Ok((
            BTreeMap::new(),
            Some(
                "ICON regional model export skipped because no closed parent gridfile was provided",
            ),
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .ok()
        .filter(|&nxp| nxp > 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "ICON model export requires a positive NXP, got {}",
                    config.nxp
                ),
            )
        })?;
    let output = standard_output_file(gridfile, "ICON", "nc4")?;
    let report = match parent {
        Some(parent) => {
            crate::write_icon_from_final_gridfile_with_parent(gridfile, parent, &output, nxp)?
        }
        None => crate::write_icon_from_final_gridfile(gridfile, &output, nxp)?,
    };
    Ok((BTreeMap::from([("icon_mesh_input", report.output)]), None))
}

fn target_triple(config: &EarthmeshConfig) -> io::Result<ProjectTargetTriple> {
    Ok(ProjectTargetTriple {
        kind: domain_kind(config.mesh_type.trim())?,
        cell: cell_kind(config.mode_grid.trim())?,
        model_format: model_format(config.output_format.trim())?,
    })
}

fn domain_kind(value: &str) -> io::Result<MeshDomainKind> {
    match value {
        "landmesh" => Ok(MeshDomainKind::Land),
        "oceanmesh" => Ok(MeshDomainKind::Ocean),
        "atmos" | "atmosmesh" => Ok(MeshDomainKind::Atmosphere),
        "LOCmesh" => Ok(MeshDomainKind::Coupled),
        "earthmesh" => Ok(MeshDomainKind::Earth),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported mesh_type for refined model delivery: {other}"),
        )),
    }
}

fn cell_kind(value: &str) -> io::Result<MeshCellKind> {
    match value {
        "" | "/tmp" | "hex" => Ok(MeshCellKind::Hex),
        "tri" => Ok(MeshCellKind::Tri),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported mode_grid for refined model delivery: {other}"),
        )),
    }
}

fn legacy_unset(value: &str) -> bool {
    matches!(value, "" | "/tmp")
}

fn model_format(value: &str) -> io::Result<ModelFormat> {
    match value {
        "CoLM" => Ok(ModelFormat::CoLM),
        "ICON" => Ok(ModelFormat::Icon),
        "MPAS" => Ok(ModelFormat::Mpas),
        "MPAS-Ocean" => Ok(ModelFormat::MpasOcean),
        "MPAS-Simple" => Ok(ModelFormat::MpasSimple),
        "FVCOM" => Ok(ModelFormat::Fvcom),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported output_format for refined model delivery: {other}"),
        )),
    }
}

fn standard_output_dir(gridfile: &Path, prefix: &str) -> io::Result<PathBuf> {
    let output = standard_dir(gridfile).join(format!("{prefix}_{}", stem(gridfile)));
    fs::create_dir_all(&output)?;
    Ok(output)
}

fn standard_output_file(gridfile: &Path, prefix: &str, extension: &str) -> io::Result<PathBuf> {
    let output = standard_dir(gridfile).join(format!("{prefix}_{}.{}", stem(gridfile), extension));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(output)
}

fn standard_dir(gridfile: &Path) -> PathBuf {
    gridfile
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("standard")
}

fn stem(gridfile: &Path) -> String {
    gridfile
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("gridfile")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(mode_grid: &str, output_format: &str) -> EarthmeshConfig {
        EarthmeshConfig {
            mesh_type: "atmosmesh".to_string(),
            mode_grid: mode_grid.to_string(),
            output_format: output_format.to_string(),
            nxp: 3,
            ..EarthmeshConfig::default()
        }
    }

    fn quality(boundary_edges: usize) -> MeshQualityReport {
        use earthmesh_quality::{GeometryMetrics, QualityLevel, TopologyMetrics};
        MeshQualityReport {
            mesh_name: String::new(),
            cell_view: "hex".to_string(),
            tool_version: String::new(),
            geometry: GeometryMetrics::default(),
            topology: TopologyMetrics {
                boundary_edge_count: boundary_edges,
                ..TopologyMetrics::default()
            },
            refine_level_groups: Vec::new(),
            hfield: None,
            adaptive: None,
            gates: Vec::new(),
            worst_cells: Vec::new(),
            repair_cells: Vec::new(),
            topology_issues: Vec::new(),
            verdict: QualityLevel::Pass,
        }
    }

    #[test]
    fn legacy_atmos_alias_and_unset_grid_defaults_are_native_only_when_format_unset() {
        let mut config = config("/tmp", "/tmp");
        config.mesh_type = "atmos".to_string();
        let (artifacts, reason) =
            write_available_models(&config, Path::new("/no/such/native.nc4"), None, &quality(0))
                .unwrap();
        assert!(artifacts.is_empty());
        assert_eq!(
            reason,
            Some("No model output_format was requested; native final admission still completed")
        );
        assert_eq!(
            domain_kind(config.mesh_type.trim()).unwrap(),
            MeshDomainKind::Atmosphere
        );
        assert_eq!(
            cell_kind(config.mode_grid.trim()).unwrap(),
            MeshCellKind::Hex
        );
    }

    #[test]
    fn defer_model_exports_returns_native_only_reason_without_touching_gridfile() {
        let mut config = config("hex", "MPAS");
        config.defer_model_exports = true;
        let (artifacts, reason) =
            write_available_models(&config, Path::new("/no/such/native.nc4"), None, &quality(0))
                .unwrap();
        assert!(artifacts.is_empty());
        assert_eq!(
            reason,
            Some("Model exports deferred; native final admission still completed")
        );
    }

    #[test]
    fn unsupported_target_uses_project_capability_registry_reason() {
        let (artifacts, reason) = write_available_models(
            &config("tri", "MPAS"),
            Path::new("/no/such/native.nc4"),
            None,
            &quality(0),
        )
        .unwrap();
        assert!(artifacts.is_empty());
        assert_eq!(
            reason,
            Some("MPAS specialized export requires hexagonal cells")
        );
    }

    #[test]
    fn mpas_missing_context_is_native_only_not_uniform_guess() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh-refine-model-delivery-missing-context-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let gridfile = root.join("native.nc4");
        netcdf::create(&gridfile).unwrap().close().unwrap();
        let (artifacts, reason) =
            write_available_models(&config("hex", "MPAS"), &gridfile, None, &quality(0)).unwrap();
        assert!(artifacts.is_empty());
        assert_eq!(
            reason,
            Some("MPAS model export skipped because the selected native gridfile has no persisted MPAS width context")
        );
        assert!(!root.join("standard").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
