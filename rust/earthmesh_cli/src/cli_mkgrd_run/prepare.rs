use std::fs;
use std::path::{Path, PathBuf};

use earthmesh_project::{
    read_lonlat_text_points, read_shapefile_polygon_rings, write_close_mask_nml, CloseMaskFormat,
    DomainConfig, LoweredProject, ProjectConfig, RegionShape,
};

use super::super::cli_args::usage;

#[derive(Clone)]
pub(super) struct ProjectRunSpec {
    pub path: PathBuf,
    pub config: ProjectConfig,
}

pub(super) struct PreparedMkgrdInput {
    pub namelist: String,
    pub project: Option<ProjectRunSpec>,
}

pub(super) fn prepare_mkgrd_namelist(
    first: String,
    args: &mut impl Iterator<Item = String>,
) -> Result<PreparedMkgrdInput, String> {
    let (mut namelist, project) = if first == "--project" {
        let compiled = compile_project_arg(args)?;
        (compiled.namelist, compiled.project)
    } else {
        (first, None)
    };
    if let Some(lowered) = lower_datalayers_namelist_if_present(&namelist)? {
        namelist = lowered;
    }
    Ok(PreparedMkgrdInput { namelist, project })
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
    if project.quality.on_violation == earthmesh_project::ViolationPolicy::AutoRefine {
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
    Ok(PreparedMkgrdInput {
        namelist,
        project: Some(spec),
    })
}

pub(super) fn compile_project_spec(spec: &ProjectRunSpec) -> Result<String, String> {
    spec.config.validate()?;
    let mut lowered = spec.config.try_lower()?;
    prepare_project_close_sources(&spec.config, &spec.path, &mut lowered)?;
    let nml_path = format!("{}.nml", spec.path.display());
    fs::write(&nml_path, lowered.to_namelist()).map_err(|e| format!("write {nml_path}: {e}"))?;
    eprintln!("earthmesh_cli: compiled project -> {nml_path}");
    Ok(nml_path)
}

fn prepare_project_close_sources(
    project: &ProjectConfig,
    project_path: &Path,
    lowered: &mut LoweredProject,
) -> Result<(), String> {
    let parent = project_path.parent().unwrap_or_else(|| Path::new("."));
    let stage_dir = project_path.with_extension("earthmesh-inputs");
    let resolve = |path: &str| {
        let source = PathBuf::from(path);
        if source.is_absolute() {
            source
        } else {
            parent.join(source)
        }
    };
    let mut prepared_stage_dir: Option<PathBuf> = None;
    let mut ensure_stage_dir = || -> Result<PathBuf, String> {
        if let Some(dir) = &prepared_stage_dir {
            return Ok(dir.clone());
        }
        fs::create_dir_all(&stage_dir)
            .map_err(|err| format!("create {}: {err}", stage_dir.display()))?;
        for entry in fs::read_dir(&stage_dir)
            .map_err(|err| format!("read {}: {err}", stage_dir.display()))?
        {
            let path = entry
                .map_err(|err| format!("read {} entry: {err}", stage_dir.display()))?
                .path();
            let generated = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    (name.starts_with("domain_close_") || name.starts_with("specified_close_"))
                        && name.ends_with(".nml")
                });
            if generated {
                fs::remove_file(&path)
                    .map_err(|err| format!("remove stale {}: {err}", path.display()))?;
            }
        }
        let dir = fs::canonicalize(&stage_dir)
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

fn lower_datalayers_namelist_if_present(namelist: &str) -> Result<Option<String>, String> {
    let Ok(text) = fs::read_to_string(namelist) else {
        return Ok(None);
    };
    if !text.to_ascii_lowercase().contains("&datalayers") {
        return Ok(None);
    }

    let fallback = Path::new(namelist)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("threshold");
    let fallback = fallback.display().to_string();
    let lowered = earthmesh_core::lower_datalayers_namelist(&text, Some(fallback.as_str()))?;
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
    let lowered_path = format!("{namelist}.lowered.nml");
    fs::write(&lowered_path, &lowered.namelist)
        .map_err(|e| format!("write lowered namelist {lowered_path}: {e}"))?;
    Ok(Some(lowered_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_project::{
        CloseBoundaryMode, MeshIntentPreset, RefinementRecipe, ResolutionSpec,
        SpecifiedCircleRefinement, SpecifiedCloseRefinement, ViolationPolicy,
    };

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
        let stage_dir = root.join("project.earthmesh-inputs");
        fs::create_dir_all(&stage_dir).unwrap();
        fs::write(stage_dir.join("keep.txt"), "not generated").unwrap();
        let mut args = vec![project_path.to_string_lossy().into_owned()].into_iter();

        let prepared = compile_project_arg(&mut args).unwrap();
        let nml = fs::read_to_string(prepared.namelist).unwrap();
        assert!(stage_dir.join("domain_close_001.nml").is_file());
        assert!(stage_dir.join("specified_close_001.nml").is_file());
        assert!(stage_dir.join("keep.txt").is_file());
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
        let nml = fs::read_to_string(nml_path).unwrap();
        let stage_dir = root.join("project.earthmesh-inputs");
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
