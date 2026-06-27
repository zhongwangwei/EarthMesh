use std::{
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_core::EarthmeshConfig;

use crate::*;

use super::infer::{
    infer_restart_refine_initial_gridfile_from_config, landtype_file_is_real,
    maybe_infer_mask_restart_non_ocean_num_vertex_from_config,
    maybe_infer_mask_restart_ocean_num_vertex_from_config,
    maybe_infer_restart_refine_initial_gridfile_from_config, namelist_sets_landtype_file,
};

fn default_mask_restart_area_judge_postproc_num_vertex(
    config: &EarthmeshConfig,
    explicit_num_vertex: Option<usize>,
) -> io::Result<Option<usize>> {
    if explicit_num_vertex.is_some() {
        return Ok(explicit_num_vertex);
    }
    let mode_num_vertex = mkgrd_mode_grid_num_vertex(config.mode_grid.trim())?;
    if config.mesh_type.trim() == "oceanmesh" {
        return Ok(
            maybe_infer_mask_restart_ocean_num_vertex_from_config(config)?
                .or(Some(mode_num_vertex)),
        );
    }
    let default_num_vertex = if config.mesh_type.trim() == "landmesh" {
        1
    } else {
        mode_num_vertex
    };
    Ok(
        maybe_infer_mask_restart_non_ocean_num_vertex_from_config(config)?
            .or(Some(default_num_vertex)),
    )
}

pub fn infer_default_restart_refine_handoff_from_config(
    config: &EarthmeshConfig,
    namelist_contents: &str,
    has_restart_refine_source_state: bool,
    restart_refine_initial_gridfile: Option<&Path>,
) -> io::Result<Option<MkgrdDefaultRestartRefineHandoff>> {
    if restart_refine_initial_gridfile.is_some() || has_restart_refine_source_state {
        let initial_gridfile = match restart_refine_initial_gridfile {
            Some(path) => path.to_path_buf(),
            None => infer_restart_refine_initial_gridfile_from_config(config)?,
        };
        let source = if has_restart_refine_source_state {
            MkgrdDefaultRestartRefineSource::SourceState
        } else if namelist_sets_landtype_file(namelist_contents)
            && landtype_file_is_real(&config.landtype_file)
        {
            MkgrdDefaultRestartRefineSource::LandtypeFile
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "default restart-refine handoff requires --restart-refine-source-state or NL%landtype_file",
            ));
        };
        return Ok(Some(MkgrdDefaultRestartRefineHandoff {
            source,
            initial_gridfile,
        }));
    }

    if config.mask_restart
        && config.refine
        && namelist_sets_landtype_file(namelist_contents)
        && landtype_file_is_real(&config.landtype_file)
    {
        if let Some(initial_gridfile) =
            maybe_infer_restart_refine_initial_gridfile_from_config(config)?
        {
            return Ok(Some(MkgrdDefaultRestartRefineHandoff {
                source: MkgrdDefaultRestartRefineSource::LandtypeFile,
                initial_gridfile,
            }));
        }
    }

    Ok(None)
}

pub fn run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    mask_restart_max_iter: i32,
    restart_refine_source_state: Option<&Path>,
    restart_refine_initial_gridfile: Option<&Path>,
    source_gridnum_perdegree: Option<usize>,
    source_first_triangle_id: usize,
    mask_postproc_num_vertex: Option<usize>,
) -> io::Result<MkgrdTopLevelDefaultRestartRefineRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    let Some(handoff) = infer_default_restart_refine_handoff_from_config(
        &config,
        &contents,
        restart_refine_source_state.is_some(),
        restart_refine_initial_gridfile,
    )?
    else {
        let restart_area_grid = PathBuf::from(config.file_dir()).join("result/IsInDmArea_grid.nc4");
        if config.mask_restart && config.mask_patch_on && restart_area_grid.exists() {
            let postproc_num_vertex = default_mask_restart_area_judge_postproc_num_vertex(
                &config,
                mask_postproc_num_vertex,
            )?;
            return run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
                namelist_source,
                workdir,
                mask_restart_max_iter,
                postproc_num_vertex,
            )
            .map(MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge)
            .map(MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch);
        }
        if config.mask_restart && !config.mask_patch_on {
            let plan =
                plan_mkgrd_mask_restart_namelist(namelist_source, workdir, mask_restart_max_iter)?;
            if plan.remask.action == MaskRestartAction::ContinueMkgrd {
                let postproc_num_vertex = default_mask_restart_area_judge_postproc_num_vertex(
                    &config,
                    mask_postproc_num_vertex,
                )?;
                return run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
                    namelist_source,
                    workdir,
                    mask_restart_max_iter,
                    postproc_num_vertex,
                )
                .map(MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge)
                .map(MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch);
            }
        }
        if !config.mask_restart && olam_direct_refine_dispatch_requested(&contents, &config)? {
            let _ = source_first_triangle_id;
            return run_mkgrd_olam_specified_refine_global_source_namelist(
                namelist_source,
                workdir,
                max_tris,
                source_gridnum_perdegree,
            )
            .map(MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource);
        }
        let regional_clip_source = !config.mask_restart && !config.mask_domain_global && {
            let prefix = config.mask_domain_fprefix.trim();
            !prefix.is_empty() && prefix != "none" && prefix != "/tmp"
        };
        let landtype_carve = !config.mask_restart
            && matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh")
            && {
                let lt = config.landtype_file.trim();
                !lt.is_empty() && lt != "none" && lt != "/tmp"
            };
        if regional_clip_source || landtype_carve {
            return run_mkgrd_regional_clip_base_namelist(namelist_source, workdir, max_tris)
                .map(MkgrdTopLevelDispatchRunReport::Gridinit)
                .map(MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch);
        }
        return run_mkgrd_top_level_namelist(
            namelist_source,
            workdir,
            max_tris,
            mask_restart_max_iter,
        )
        .map(MkgrdTopLevelDefaultRestartRefineRunReport::Dispatch);
    };

    match handoff.source {
        MkgrdDefaultRestartRefineSource::SourceState => {
            let source_state = restart_refine_source_state.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "default source-state restart-refine handoff requires a source-state path",
                )
            })?;
            fs::metadata(source_state)?;
            let _ = mask_postproc_num_vertex;
            let rewritten = rewrite_restart_refine_namelist_for_olam_direct(
                namelist_source,
                workdir,
                &handoff.initial_gridfile,
            )?;
            run_mkgrd_olam_specified_refine_global_source_namelist(
                &rewritten,
                workdir,
                max_tris,
                source_gridnum_perdegree,
            )
            .map(MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource)
        }
        MkgrdDefaultRestartRefineSource::LandtypeFile => {
            let _ = source_first_triangle_id;
            let _ = mask_postproc_num_vertex;
            let rewritten = rewrite_restart_refine_namelist_for_olam_direct(
                namelist_source,
                workdir,
                &handoff.initial_gridfile,
            )?;
            run_mkgrd_olam_specified_refine_global_source_namelist(
                &rewritten,
                workdir,
                max_tris,
                source_gridnum_perdegree,
            )
            .map(MkgrdTopLevelDefaultRestartRefineRunReport::OlamRefineGlobalSource)
        }
    }
}

fn rewrite_restart_refine_namelist_for_olam_direct(
    namelist_source: &Path,
    workdir: &Path,
    initial_gridfile: &Path,
) -> io::Result<PathBuf> {
    fs::metadata(initial_gridfile)?;
    let contents = fs::read_to_string(namelist_source)?;
    let mut saw_mode_file = false;
    let mut saw_mode_file_description = false;
    let mut in_mkgrd = false;
    let initial_gridfile = initial_gridfile.display().to_string();
    let mut rewritten = Vec::new();
    for line in contents.lines() {
        let trimmed_lower = line.trim_start().to_ascii_lowercase();
        if trimmed_lower.starts_with("&mkgrd") {
            in_mkgrd = true;
            rewritten.push(line.to_string());
            continue;
        }
        if in_mkgrd && line.trim() == "/" {
            if !saw_mode_file {
                rewritten.push(format!("  NL%mode_file='{initial_gridfile}'"));
            }
            if !saw_mode_file_description {
                rewritten.push("  NL%mode_file_description='EarthMesh'".to_string());
            }
            in_mkgrd = false;
            rewritten.push(line.to_string());
            continue;
        }
        rewritten.push(if trimmed_lower.starts_with("nl%mask_restart") {
            "  NL%mask_restart=.false.".to_string()
        } else if trimmed_lower.starts_with("nl%mode_file_description") {
            saw_mode_file_description = true;
            "  NL%mode_file_description='EarthMesh'".to_string()
        } else if trimmed_lower.starts_with("nl%mode_file") {
            saw_mode_file = true;
            format!("  NL%mode_file='{initial_gridfile}'")
        } else {
            line.to_string()
        });
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let path = workdir.join(format!(
        "earthmesh_olam_default_restart_refine_{}_{}.nml",
        std::process::id(),
        stamp
    ));
    fs::write(&path, format!("{}\n", rewritten.join("\n")))?;
    Ok(path)
}
