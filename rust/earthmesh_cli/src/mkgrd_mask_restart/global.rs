use std::{fs, io, path::Path};

use earthmesh_core::EarthmeshConfig;

use crate::*;

pub fn run_mkgrd_mask_restart_area_judge_global_source_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    postproc_num_vertex: Option<usize>,
) -> io::Result<MkgrdRestartAreaJudgeGlobalSourceRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let effective_postproc_num_vertex = match postproc_num_vertex {
        Some(num_vertex) => Some(num_vertex),
        None => {
            let contents = fs::read_to_string(namelist_source)?;
            let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            if config.mesh_type.trim() == "oceanmesh" {
                maybe_infer_mask_restart_ocean_num_vertex_from_config(&config)?
            } else {
                maybe_infer_mask_restart_non_ocean_num_vertex_from_config(&config)?
            }
        }
    };
    let axes =
        build_global_source_axes_fortran_indexed(gridnum_perdegree, nlons_source, nlats_source)?;
    let area_judge = axes.restart_area_judge_options();
    if let Some(num_vertex) = effective_postproc_num_vertex {
        let postproc = run_mkgrd_mask_restart_area_judge_postproc_namelist(
            namelist_source,
            workdir,
            max_iter,
            MkgrdRestartAreaJudgePostprocOptions {
                area_judge,
                num_vertex,
            },
        )?;
        let restart = postproc.restart.clone();
        Ok(MkgrdRestartAreaJudgeGlobalSourceRunReport {
            restart,
            postproc: Some(postproc),
        })
    } else {
        let restart = run_mkgrd_mask_restart_area_judge_namelist(
            namelist_source,
            workdir,
            max_iter,
            area_judge,
        )?;
        Ok(MkgrdRestartAreaJudgeGlobalSourceRunReport {
            restart,
            postproc: None,
        })
    }
}

pub fn run_mkgrd_mask_restart_area_judge_configured_global_source_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
    postproc_num_vertex: Option<usize>,
) -> io::Result<MkgrdRestartAreaJudgeGlobalSourceRunReport> {
    let namelist_source = namelist_source.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let gridnum_perdegree = usize::try_from(config.gridnum_perdegree).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "NL%gridnum_perdegree must be positive for configured global source axes, got {}",
                config.gridnum_perdegree
            ),
        )
    })?;
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%gridnum_perdegree must be positive for configured global source axes",
        ));
    }
    let nlons_source = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "configured global-source longitude count overflowed usize",
        )
    })?;
    let nlats_source = gridnum_perdegree.checked_mul(180).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "configured global-source latitude count overflowed usize",
        )
    })?;
    let effective_postproc_num_vertex = match postproc_num_vertex {
        Some(num_vertex) => Some(num_vertex),
        None if config.mesh_type.trim() != "oceanmesh" => {
            maybe_infer_mask_restart_non_ocean_num_vertex_from_config(&config)?
        }
        None => None,
    };
    run_mkgrd_mask_restart_area_judge_global_source_namelist(
        namelist_source,
        workdir,
        max_iter,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        effective_postproc_num_vertex,
    )
}
