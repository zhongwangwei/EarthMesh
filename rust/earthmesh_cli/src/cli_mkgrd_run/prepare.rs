use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use earthmesh_cli::resolve_project_path;
use earthmesh_project::{
    read_lonlat_text_points, read_shapefile_polygon_rings, write_close_mask_nml, CloseMaskFormat,
    DomainConfig, LoweredProject, ProjectConfig, RegionShape,
};

use super::super::cli_args::usage;

static PROJECT_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub(super) struct ProjectRunSpec {
    pub path: PathBuf,
    pub config: ProjectConfig,
}

#[derive(Debug)]
pub(super) struct PreparedMkgrdInput {
    pub namelist: String,
    pub project: Option<ProjectRunSpec>,
    pub project_run_dir: Option<PathBuf>,
    pub cleanup_dir: Option<PathBuf>,
}

struct LoweredNamelist {
    path: String,
    cleanup_dir: PathBuf,
}

pub(super) fn prepare_mkgrd_namelist(
    first: String,
    args: &mut impl Iterator<Item = String>,
) -> Result<PreparedMkgrdInput, String> {
    let mut prepared = if first == "--project" {
        compile_project_arg(args)?
    } else {
        PreparedMkgrdInput {
            namelist: first,
            project: None,
            project_run_dir: None,
            cleanup_dir: None,
        }
    };
    match lower_datalayers_namelist_if_present(&prepared.namelist) {
        Ok(Some(lowered)) => {
            prepared.namelist = lowered.path;
            prepared.cleanup_dir = Some(lowered.cleanup_dir);
        }
        Ok(None) => {}
        Err(err) => {
            if let Some(path) = &prepared.project_run_dir {
                let _ = fs::remove_dir_all(path);
            }
            return Err(err);
        }
    }
    Ok(prepared)
}

fn compile_project_arg(
    args: &mut impl Iterator<Item = String>,
) -> Result<PreparedMkgrdInput, String> {
    let path = args
        .next()
        .ok_or_else(|| usage("--project needs a project.yaml or .json path"))?;
    let text = fs::read_to_string(&path).map_err(|e| format!("read project {path}: {e}"))?;
    let mut project = if path.ends_with(".json") {
        ProjectConfig::from_json(&text)?
    } else {
        ProjectConfig::from_yaml(&text)?
    };
    if project.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine
        && project.refinement.enabled
    {
        let target_nxp = project.try_lower()?.mkgrd.nxp;
        project.refinement.max_passes = earthmesh_project::effective_auto_refine_pass(
            project.refinement.max_passes,
            target_nxp,
        );
    }
    let spec = ProjectRunSpec {
        path: PathBuf::from(path),
        config: project,
    };
    let namelist = compile_project_spec(&spec)?;
    let project_run_dir = Path::new(&namelist).parent().map(Path::to_path_buf);
    Ok(PreparedMkgrdInput {
        namelist,
        project: Some(spec),
        project_run_dir,
        cleanup_dir: None,
    })
}

pub(super) fn compile_project_spec(spec: &ProjectRunSpec) -> Result<String, String> {
    let mut config = spec.config.clone();
    for layer in &mut config.data_layers {
        if !layer.path.trim().is_empty() {
            layer.path = resolve_project_path(&spec.path, &layer.path)
                .to_string_lossy()
                .into_owned();
        }
    }
    if let Some(coupling) = &mut config.coupling {
        if let Some(root) = coupling
            .cama_root
            .as_mut()
            .filter(|root| !root.trim().is_empty())
        {
            *root = resolve_project_path(&spec.path, root)
                .to_string_lossy()
                .into_owned();
        }
    }
    config.validate()?;
    let mut lowered = config.try_lower()?;
    let run_dir = create_project_run_dir(&spec.path)?;
    let result = (|| {
        lowered.mkgrd.base_dir = format!("{}{}", run_dir.display(), std::path::MAIN_SEPARATOR);
        prepare_project_close_sources(&config, &spec.path, &run_dir.join("inputs"), &mut lowered)?;
        if lowered.data_layers.layers.iter().any(|layer| {
            layer.enabled
                && !layer.path.trim().is_empty()
                && matches!(layer.role, earthmesh_core::DataLayerRole::ThresholdField(_))
        }) {
            lowered.refine.threshold_dir =
                run_dir.join("thresholds").to_string_lossy().into_owned();
        }
        let nml_path = run_dir.join("project.nml");
        fs::write(&nml_path, lowered.to_namelist())
            .map_err(|e| format!("write {}: {e}", nml_path.display()))?;
        eprintln!("earthmesh_cli: compiled project -> {}", nml_path.display());
        Ok(nml_path.to_string_lossy().into_owned())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&run_dir);
    }
    result
}

fn create_project_run_dir(project_path: &Path) -> Result<PathBuf, String> {
    create_sibling_run_dir(project_path, "earthmesh-run")
}

fn create_sibling_run_dir(source_path: &Path, tag: &str) -> Result<PathBuf, String> {
    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));
    let name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = PROJECT_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dir = parent.join(format!(
        "{name}.{tag}-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    fs::canonicalize(&dir).map_err(|err| format!("resolve {}: {err}", dir.display()))
}

fn prepare_project_close_sources(
    project: &ProjectConfig,
    project_path: &Path,
    stage_dir: &Path,
    lowered: &mut LoweredProject,
) -> Result<(), String> {
    let resolve = |path: &str| resolve_project_path(project_path, path);
    let mut prepared_stage_dir: Option<PathBuf> = None;
    let mut ensure_stage_dir = || -> Result<PathBuf, String> {
        if let Some(dir) = &prepared_stage_dir {
            return Ok(dir.clone());
        }
        fs::create_dir_all(stage_dir)
            .map_err(|err| format!("create {}: {err}", stage_dir.display()))?;
        let dir = fs::canonicalize(stage_dir)
            .map_err(|err| format!("resolve {}: {err}", stage_dir.display()))?;
        prepared_stage_dir = Some(dir.clone());
        Ok(dir)
    };

    if let DomainConfig::Regional { shape, .. } = &project.domain {
        let source = match shape {
            RegionShape::Shapefile { path } => Some((path.as_str(), CloseMaskFormat::PolygonShp)),
            RegionShape::Close { path, format, .. } => Some((path.as_str(), *format)),
            _ => None,
        };
        if let Some((path, format)) = source {
            let source = resolve(path);
            match format {
                CloseMaskFormat::PolygonShp | CloseMaskFormat::LonLatText => {
                    let rings = match format {
                        CloseMaskFormat::PolygonShp => read_shapefile_polygon_rings(&source),
                        CloseMaskFormat::LonLatText => {
                            read_lonlat_text_points(&source).map(|ring| vec![ring])
                        }
                        _ => unreachable!(),
                    }
                    .map_err(|err| format!("convert close domain {}: {err}", source.display()))?;
                    let dir = ensure_stage_dir()?;
                    for (index, ring) in rings.iter().enumerate() {
                        write_close_mask_nml(
                            &dir.join(format!("domain_close_{:03}.nml", index + 1)),
                            ring,
                            0,
                        )
                        .map_err(|err| format!("write close domain NML: {err}"))?;
                    }
                    lowered.mkgrd.mask_domain_fprefix =
                        dir.join("domain_close_").to_string_lossy().into_owned();
                }
                CloseMaskFormat::Nml | CloseMaskFormat::Netcdf => {
                    lowered.mkgrd.mask_domain_fprefix = source.to_string_lossy().into_owned();
                }
            }
        }
    }
    if let Some(close) = &project.refinement.specified_close {
        let source = resolve(&close.path);
        let extension = source
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "shp" | "txt" | "csv") {
            let rings = if extension == "shp" {
                read_shapefile_polygon_rings(&source)
            } else {
                read_lonlat_text_points(&source).map(|ring| vec![ring])
            }
            .map_err(|err| format!("convert specified close {}: {err}", source.display()))?;
            let dir = ensure_stage_dir()?;
            for (index, ring) in rings.iter().enumerate() {
                write_close_mask_nml(
                    &dir.join(format!("specified_close_{:03}.nml", index + 1)),
                    ring,
                    usize::from(project.refinement.max_passes.max(1)),
                )
                .map_err(|err| format!("write specified close NML: {err}"))?;
            }
            lowered.refine.mask_refine_spc_fprefix =
                dir.join("specified_close_").to_string_lossy().into_owned();
        } else {
            lowered.refine.mask_refine_spc_fprefix = source.to_string_lossy().into_owned();
        }
    }
    Ok(())
}

fn lower_datalayers_namelist_if_present(namelist: &str) -> Result<Option<LoweredNamelist>, String> {
    let Ok(text) = fs::read_to_string(namelist) else {
        return Ok(None);
    };
    if !text.to_ascii_lowercase().contains("&datalayers") {
        return Ok(None);
    }

    let stage_dir = create_sibling_run_dir(Path::new(namelist), "earthmesh-lowered")?;
    let result = (|| {
        let fallback = stage_dir.join("thresholds");
        let fallback = fallback.to_string_lossy();
        let lowered = earthmesh_core::lower_datalayers_namelist(&text, Some(&fallback))?;
        if !lowered.threshold_files.is_empty() {
            let th_dir = PathBuf::from(&lowered.threshold_dir);
            fs::create_dir_all(&th_dir)
                .map_err(|e| format!("create threshold dir {}: {e}", th_dir.display()))?;
            for (stem, src) in &lowered.threshold_files {
                let dst = th_dir.join(format!("{stem}.nc"));
                fs::copy(src, &dst)
                    .map_err(|e| format!("stage threshold {src} -> {}: {e}", dst.display()))?;
            }
        }
        for warning in &lowered.warnings {
            eprintln!("earthmesh_cli: warning: {warning}");
        }
        let lowered_path = stage_dir.join("lowered.nml");
        fs::write(&lowered_path, &lowered.namelist)
            .map_err(|e| format!("write lowered namelist {}: {e}", lowered_path.display()))?;
        Ok(LoweredNamelist {
            path: lowered_path.to_string_lossy().into_owned(),
            cleanup_dir: stage_dir.clone(),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage_dir);
    }
    result.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_project::{
        CloseBoundaryMode, HfieldRefinementRecipe, MeshIntentPreset, RefinementRecipe,
        ResolutionSpec, SpecifiedCircleRefinement, SpecifiedCloseRefinement, ThresholdField,
        ViolationPolicy,
    };

    #[test]
    fn project_prepare_preserves_hfield_and_quality_after_datalayer_lowering() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_project_groups_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut project = ProjectConfig::scaffold(
            "project_groups",
            MeshIntentPreset::CoastalOcean,
            DomainConfig::Regional {
                shape: RegionShape::Bbox {
                    w: 100.0,
                    e: 102.0,
                    s: 10.0,
                    n: 12.0,
                },
                sea_ratio: None,
            },
            ResolutionSpec::Nxp(40),
        );
        project.refinement.enabled = true;
        project.refinement.max_passes = 2;
        project.refinement.specified_circle = Some(SpecifiedCircleRefinement {
            lon: 101.0,
            lat: 11.0,
            radius_km: 50.0,
        });
        project.refinement.hfield = Some(HfieldRefinementRecipe {
            g: 0.15,
            max_level: 2,
            ..HfieldRefinementRecipe::default()
        });
        project.quality.min_angle_deg = 31.0;
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();

        let prepared = prepare_mkgrd_namelist("--project".to_string(), &mut args).unwrap();
        let nml = fs::read_to_string(&prepared.namelist).unwrap();

        assert!(prepared.namelist.ends_with("lowered.nml"));
        assert!(nml.contains("&hfield"), "{nml}");
        assert!(nml.contains("NL%hfield_g = 0.15"), "{nml}");
        assert!(nml.contains("&quality"), "{nml}");
        assert!(nml.contains("NL%min_angle_warn_deg = 31"), "{nml}");
        assert!(!nml.contains("&datalayers"), "{nml}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_prepare_keeps_threshold_master_switch_off_and_landtype() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_threshold_off_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let landtype = root.join("landtype.nc");
        let lai = root.join("lai.nc");
        fs::write(&landtype, []).unwrap();
        fs::write(&lai, []).unwrap();
        let mut project = ProjectConfig::scaffold(
            "threshold_off",
            MeshIntentPreset::CarbonLand,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.data_layers = vec![
            earthmesh_project::ProjectDataLayer {
                id: "landtype".to_string(),
                role: earthmesh_project::ProjectLayerRole::LandType,
                path: landtype.to_string_lossy().into_owned(),
                enabled: true,
                threshold_value: None,
            },
            earthmesh_project::ProjectDataLayer {
                id: "lai".to_string(),
                role: earthmesh_project::ProjectLayerRole::Threshold(ThresholdField::Lai),
                path: lai.to_string_lossy().into_owned(),
                enabled: true,
                threshold_value: None,
            },
        ];
        project.refinement.enabled = true;
        project.refinement.threshold_enabled = false;
        project.refinement.max_passes = 1;
        project.refinement.specified_circle = Some(SpecifiedCircleRefinement {
            lon: 0.0,
            lat: 0.0,
            radius_km: 50.0,
        });
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();

        let prepared = prepare_mkgrd_namelist("--project".to_string(), &mut args).unwrap();
        let nml = fs::read_to_string(prepared.namelist).unwrap();
        let mkgrd = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&nml).unwrap();
        let refine = earthmesh_core::RefineConfig::from_mkrefine_namelist(
            &nml,
            &mkgrd.mesh_type,
            &mkgrd.mode_grid,
        )
        .unwrap();
        assert_eq!(mkgrd.landtype_file, landtype.to_string_lossy());
        assert!(!refine.refine_cal);
        assert!(!refine.refine_num_landtypes);
        assert!(!root.join("project.earthmesh-thresholds").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_prepare_keeps_auto_refine_uniform_baseline_unrefined() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_auto_refine_uniform_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut project = ProjectConfig::scaffold(
            "auto_refine_uniform",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(16),
        );
        project.quality.on_violation = earthmesh_project::ViolationPolicy::AutoRefine;
        assert!(!project.refinement.enabled);
        assert_eq!(project.refinement.max_passes, 0);
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();

        let prepared = prepare_mkgrd_namelist("--project".to_string(), &mut args).unwrap();
        let nml = fs::read_to_string(prepared.namelist).unwrap();
        let mkgrd = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&nml).unwrap();
        assert!(!mkgrd.refine);
        assert!(nml.contains("NL%on_violation = 'auto_refine'"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_thresholds_resolve_relative_to_project_and_isolate_runs() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_threshold_stage_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let input_dir = root.join("input");
        fs::create_dir_all(&input_dir).unwrap();
        let source = input_dir.join("typhoon.nc");
        fs::write(&source, b"threshold").unwrap();
        let mut project = ProjectConfig::scaffold(
            "threshold_stage",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.data_layers = vec![earthmesh_project::ProjectDataLayer {
            id: "typhoon".to_string(),
            role: earthmesh_project::ProjectLayerRole::Threshold(ThresholdField::Typhoon),
            path: "input/typhoon.nc".to_string(),
            enabled: true,
            threshold_value: None,
        }];
        project.refinement.enabled = true;
        project.refinement.max_passes = 1;
        project.refinement.threshold_enabled = true;
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let run = || {
            let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();
            prepare_mkgrd_namelist("--project".to_string(), &mut args).unwrap()
        };

        let first = run();
        let second = run();
        assert_ne!(first.namelist, second.namelist);
        let canonical_root = fs::canonicalize(&root).unwrap();
        let mut output_dirs = Vec::new();
        for prepared in [first, second] {
            let nml = fs::read_to_string(&prepared.namelist).unwrap();
            let mkgrd = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&nml).unwrap();
            let refine = earthmesh_core::RefineConfig::from_mkrefine_namelist(
                &nml,
                &mkgrd.mesh_type,
                &mkgrd.mode_grid,
            )
            .unwrap();
            let threshold_dir = PathBuf::from(refine.threshold_dir);
            assert!(threshold_dir.starts_with(&canonical_root));
            assert_eq!(
                fs::read(threshold_dir.join("typhoon.nc")).unwrap(),
                b"threshold"
            );
            let output_dir = PathBuf::from(mkgrd.file_dir());
            assert!(output_dir.starts_with(&canonical_root));
            output_dirs.push(output_dir);
        }
        assert_ne!(output_dirs[0], output_dirs[1]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_coupling_cama_root_resolves_relative_to_project() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_coupling_path_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("cama")).unwrap();
        let mut project = ProjectConfig::scaffold(
            "coupling_path",
            MeshIntentPreset::MeritHydroCoast,
            DomainConfig::Regional {
                shape: RegionShape::Bbox {
                    w: 100.0,
                    e: 102.0,
                    s: 10.0,
                    n: 12.0,
                },
                sea_ratio: None,
            },
            ResolutionSpec::Nxp(40),
        );
        project.coupling = Some(earthmesh_project::CoupledMeshConfig {
            identify_river_mouth: true,
            cama_root: Some("cama".into()),
            ..earthmesh_project::CoupledMeshConfig::default()
        });
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let spec = ProjectRunSpec {
            path: project_path,
            config: project,
        };

        let namelist = compile_project_spec(&spec).unwrap();
        let text = fs::read_to_string(namelist).unwrap();
        let mkgrd = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&text).unwrap();
        assert_eq!(PathBuf::from(mkgrd.coupling_cama_root), root.join("cama"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_project_lowering_removes_unique_run_directory() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_project_cleanup_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut project = ProjectConfig::scaffold(
            "cleanup",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.data_layers = vec![earthmesh_project::ProjectDataLayer {
            id: "typhoon".into(),
            role: earthmesh_project::ProjectLayerRole::Threshold(ThresholdField::Typhoon),
            path: "missing.nc".into(),
            enabled: true,
            threshold_value: None,
        }];
        project.refinement.enabled = true;
        project.refinement.max_passes = 1;
        project.refinement.threshold_enabled = true;
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();

        let error = prepare_mkgrd_namelist("--project".into(), &mut args).unwrap_err();
        assert!(error.contains("stage threshold"));
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("earthmesh-run")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn raw_namelist_explicit_threshold_dir_is_honored() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_threshold_explicit_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("typhoon.nc");
        let explicit = root.join("chosen-thresholds");
        fs::write(&source, b"threshold").unwrap();
        let mut project = ProjectConfig::scaffold(
            "threshold_explicit",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.data_layers = vec![earthmesh_project::ProjectDataLayer {
            id: "typhoon".to_string(),
            role: earthmesh_project::ProjectLayerRole::Threshold(ThresholdField::Typhoon),
            path: source.to_string_lossy().into_owned(),
            enabled: true,
            threshold_value: None,
        }];
        project.refinement.enabled = true;
        project.refinement.max_passes = 1;
        project.refinement.threshold_enabled = true;
        let mut lowered = project.try_lower().unwrap();
        lowered.refine.threshold_dir = explicit.to_string_lossy().into_owned();
        let namelist = root.join("raw.nml");
        fs::write(&namelist, lowered.to_namelist()).unwrap();

        lower_datalayers_namelist_if_present(namelist.to_str().unwrap()).unwrap();
        assert_eq!(fs::read(explicit.join("typhoon.nc")).unwrap(), b"threshold");
        assert!(!root.join("raw.nml.thresholds").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn raw_namelist_lowering_uses_unique_staging_directories() {
        let root =
            std::env::temp_dir().join(format!("earthmesh_cli_raw_lowering_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("typhoon.nc");
        fs::write(&source, b"threshold").unwrap();
        let mut project = ProjectConfig::scaffold(
            "raw_lowering",
            MeshIntentPreset::AtmosphereMpas,
            DomainConfig::Global,
            ResolutionSpec::Nxp(40),
        );
        project.data_layers = vec![earthmesh_project::ProjectDataLayer {
            id: "typhoon".into(),
            role: earthmesh_project::ProjectLayerRole::Threshold(ThresholdField::Typhoon),
            path: source.to_string_lossy().into_owned(),
            enabled: true,
            threshold_value: None,
        }];
        project.refinement.enabled = true;
        project.refinement.max_passes = 1;
        project.refinement.threshold_enabled = true;
        let mut lowered = project.try_lower().unwrap();
        lowered.refine.threshold_dir.clear();
        let namelist = root.join("raw.nml");
        fs::write(&namelist, lowered.to_namelist()).unwrap();

        let first = lower_datalayers_namelist_if_present(namelist.to_str().unwrap())
            .unwrap()
            .unwrap();
        let second = lower_datalayers_namelist_if_present(namelist.to_str().unwrap())
            .unwrap()
            .unwrap();

        assert_ne!(first.path, second.path);
        assert_ne!(first.cleanup_dir, second.cleanup_dir);
        assert_eq!(
            fs::read(first.cleanup_dir.join("thresholds/typhoon.nc")).unwrap(),
            b"threshold"
        );
        assert_eq!(
            fs::read(second.cleanup_dir.join("thresholds/typhoon.nc")).unwrap(),
            b"threshold"
        );
        assert!(!root.join("raw.nml.lowered.nml").exists());
        assert!(!root.join("raw.nml.thresholds").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_cli_stages_domain_and_specified_close_text_as_nml() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_project_close_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("region.txt"), "100,10\n102,10\n102,12\n100,12\n").unwrap();
        let mut project = ProjectConfig::scaffold(
            "cli_close",
            MeshIntentPreset::CoastalOcean,
            DomainConfig::Regional {
                shape: RegionShape::Close {
                    path: "region.txt".to_string(),
                    format: CloseMaskFormat::LonLatText,
                    boundary: CloseBoundaryMode::Polyline,
                },
                sea_ratio: None,
            },
            ResolutionSpec::Nxp(40),
        );
        project.refinement = RefinementRecipe {
            enabled: true,
            max_passes: 2,
            specified_close: Some(SpecifiedCloseRefinement {
                path: "region.txt".to_string(),
                boundary: CloseBoundaryMode::Polyline,
            }),
            ..RefinementRecipe::default()
        };
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();

        let prepared = compile_project_arg(&mut args).unwrap();
        let stage_dir = Path::new(&prepared.namelist)
            .parent()
            .unwrap()
            .join("inputs");
        let nml = fs::read_to_string(prepared.namelist).unwrap();
        assert!(stage_dir.join("domain_close_001.nml").is_file());
        assert!(stage_dir.join("specified_close_001.nml").is_file());
        assert!(nml.contains(&stage_dir.join("domain_close_").display().to_string()));
        assert!(nml.contains(&stage_dir.join("specified_close_").display().to_string()));
    }

    #[test]
    fn project_cli_clamps_initial_auto_refine_pass_to_resolution_cap() {
        let root =
            std::env::temp_dir().join(format!("earthmesh_cli_auto_cap_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut project = ProjectConfig::scaffold(
            "auto_cap",
            MeshIntentPreset::CoastalOcean,
            DomainConfig::Regional {
                shape: RegionShape::Bbox {
                    w: 0.0,
                    e: 2.0,
                    s: 0.0,
                    n: 2.0,
                },
                sea_ratio: None,
            },
            ResolutionSpec::Nxp(40),
        );
        project.refinement.enabled = true;
        project.refinement.max_passes = 3;
        project.refinement.specified_circle = Some(SpecifiedCircleRefinement {
            lon: 1.0,
            lat: 1.0,
            radius_km: 50.0,
        });
        project.quality.on_violation = ViolationPolicy::AutoRefine;
        let path = root.join("project.yaml");
        fs::write(&path, project.to_yaml().unwrap()).unwrap();
        let mut args = vec![path.to_string_lossy().into_owned()].into_iter();
        let prepared = compile_project_arg(&mut args).unwrap();
        assert_eq!(prepared.project.unwrap().config.refinement.max_passes, 2);
    }

    #[test]
    fn project_cli_stages_watershed_shapefile_as_close_domain() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_project_watershed_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_test_polygon_shp(
            &root.join("watershed.shp"),
            &[(100.0, 10.0), (102.0, 10.0), (102.0, 12.0), (100.0, 12.0)],
        );
        let mut project = ProjectConfig::scaffold(
            "watershed",
            MeshIntentPreset::CoastalOcean,
            DomainConfig::Regional {
                shape: RegionShape::Shapefile {
                    path: "watershed.shp".to_string(),
                },
                sea_ratio: None,
            },
            ResolutionSpec::Nxp(40),
        );
        project.refinement.enabled = false;
        project.refinement.max_passes = 0;
        let project_path = root.join("project.yaml");
        fs::write(&project_path, project.to_yaml().unwrap()).unwrap();

        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();
        let nml_path = compile_project_arg(&mut args).unwrap().namelist;
        let stage_dir = Path::new(&nml_path).parent().unwrap().join("inputs");
        let nml = fs::read_to_string(nml_path).unwrap();
        assert!(stage_dir.join("domain_close_001.nml").is_file());
        assert!(nml.contains("mask_domain_type = 'close'"));
        assert!(nml.contains(&stage_dir.join("domain_close_").display().to_string()));
    }

    fn write_test_polygon_shp(path: &Path, ring: &[(f64, f64)]) {
        let mut points = ring.to_vec();
        points.push(ring[0]);
        let content_bytes = 48 + points.len() * 16;
        let file_bytes = 108 + content_bytes;
        let mut out = Vec::with_capacity(file_bytes);

        out.extend(9994_i32.to_be_bytes());
        out.extend([0_u8; 20]);
        out.extend(((file_bytes / 2) as i32).to_be_bytes());
        out.extend(1000_i32.to_le_bytes());
        out.extend(5_i32.to_le_bytes());
        for value in [100.0_f64, 10.0, 102.0, 12.0, 0.0, 0.0, 0.0, 0.0] {
            out.extend(value.to_le_bytes());
        }
        out.extend(1_i32.to_be_bytes());
        out.extend(((content_bytes / 2) as i32).to_be_bytes());
        out.extend(5_i32.to_le_bytes());
        for value in [100.0_f64, 10.0, 102.0, 12.0] {
            out.extend(value.to_le_bytes());
        }
        out.extend(1_i32.to_le_bytes());
        out.extend((points.len() as i32).to_le_bytes());
        out.extend(0_i32.to_le_bytes());
        for (x, y) in points {
            out.extend(x.to_le_bytes());
            out.extend(y.to_le_bytes());
        }
        fs::write(path, out).unwrap();
    }
}
