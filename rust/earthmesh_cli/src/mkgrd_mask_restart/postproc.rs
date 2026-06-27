use std::{io, path::Path};

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::*;

fn select_area_judge_source_window_fortran_order(
    values: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Vec<i32>>> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Area_judge source bounds for selected matrix",
        ));
    }
    let mut selected = Vec::with_capacity(bounds.maxlon_source - bounds.minlon_source + 1);
    for lon_index in bounds.minlon_source..=bounds.maxlon_source {
        let row = values.get(lon_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "source matrix is missing longitude index {lon_index} for selected Area_judge bounds"
                ),
            )
        })?;
        let mut selected_row = Vec::with_capacity(bounds.minlat_source - bounds.maxlat_source + 1);
        for lat_index in bounds.maxlat_source..=bounds.minlat_source {
            selected_row.push(*row.get(lat_index).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "source matrix is missing latitude index {lat_index} for selected Area_judge bounds"
                    ),
                )
            })?);
        }
        selected.push(selected_row);
    }
    Ok(selected)
}

fn require_mask_postproc_plan<'a>(
    plan: Option<&'a MaskPostprocDomainIoPlan>,
    mesh_type: &str,
) -> io::Result<&'a MaskPostprocDomainIoPlan> {
    plan.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("mask_restart Area_judge postproc plan is unavailable for {mesh_type}"),
        )
    })
}

pub fn run_mkgrd_mask_restart_area_judge_postproc_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
    options: MkgrdRestartAreaJudgePostprocOptions<'_>,
) -> io::Result<MkgrdRestartAreaJudgePostprocRunReport> {
    let mut restart = run_mkgrd_mask_restart_area_judge_namelist(
        namelist_source,
        workdir,
        max_iter,
        options.area_judge,
    )?;
    let config = &restart.plan.config;
    let mesh_type = config.mesh_type.trim();
    let contain_kind = match mesh_type {
        "earthmesh" => GetContainMeshKind::Loc,
        "landmesh" => GetContainMeshKind::Land,
        "oceanmesh" => GetContainMeshKind::Ocean,
        "atmos" | "atmosmesh" => GetContainMeshKind::Atmos,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mask_restart Area_judge final postproc supports earthmesh/landmesh/oceanmesh/atmos or atmosmesh; got {other}"
                ),
            ));
        }
    };
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for mask_restart Area_judge postproc",
        ));
    }
    let nxp = usize::try_from(config.nxp).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must fit usize for mask_restart Area_judge postproc",
        )
    })?;
    let nxpc = format!("{nxp:04}");
    let mode_grid = config.mode_grid.trim();
    let atmos_source_gridfile = restart
        .plan
        .remask
        .file_dir
        .join("result")
        .join(format!("gridfile_NXP{nxpc}_{mode_grid}.nc4"));
    let atmos_contain_domain = restart.plan.remask.file_dir.join("contain").join(format!(
        "contain_{mesh_type}_domain_NXP{nxpc}_{mode_grid}.nc4"
    ));
    let postproc_plan = if matches!(mesh_type, "atmos" | "atmosmesh") {
        None
    } else {
        Some(plan_mask_postproc_domain_io(
            &restart.plan.remask.file_dir,
            nxp,
            mode_grid,
            mesh_type,
            config.mask_patch_on,
        )?)
    };
    let source_gridfile = postproc_plan
        .as_ref()
        .map(|plan| plan.source_gridfile.as_path())
        .unwrap_or(atmos_source_gridfile.as_path());
    let contain_domain = postproc_plan
        .as_ref()
        .map(|plan| plan.contain_domain.as_path())
        .unwrap_or(atmos_contain_domain.as_path());
    let contain = run_getcontain_refine_file_fortran_indexed(GetContainRefineFileRunConfig {
        gridfile: source_gridfile,
        area_grid_file: &restart.area_write.output,
        output: contain_domain,
        mesh_kind: contain_kind,
        seaorland: &restart.area.seaorland.seaorland,
        lon_vertex: options.area_judge.lon_vertex,
        lat_vertex: options.area_judge.lat_vertex,
        lon_i: options.area_judge.lon_i,
        lat_i: options.area_judge.lat_i,
        num_vertex: options.num_vertex,
    })?;
    restart
        .runtime_state
        .record_mesh_counts_for_step(
            1,
            contain.runtime_counts.current_num_mp_step,
            contain.runtime_counts.current_num_wp_step,
        )
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    if contain.runtime_counts.previous_num_vertex > 0 {
        restart
            .runtime_state
            .record_num_vertex(contain.runtime_counts.previous_num_vertex)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    }

    let bounds = restart.area.domain.bounds;
    let minlon_dm_area = i32::try_from(bounds.minlon_source).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "minlon_source must fit i32 for mask_restart postproc",
        )
    })?;
    let maxlat_dm_area = i32::try_from(bounds.maxlat_source).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "maxlat_source must fit i32 for mask_restart postproc",
        )
    })?;
    let nlons_dm_select = bounds
        .maxlon_source
        .checked_sub(bounds.minlon_source)
        .map(|value| value + 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid restart Area_judge longitude bounds",
            )
        })?;
    let nlats_dm_select = bounds
        .minlat_source
        .checked_sub(bounds.maxlat_source)
        .map(|value| value + 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid restart Area_judge latitude bounds",
            )
        })?;

    let postproc = match mesh_type {
        "earthmesh" => {
            let postproc_plan = require_mask_postproc_plan(postproc_plan.as_ref(), mesh_type)?;
            let source_mesh = read_unstructured_mesh_netcdf(&postproc_plan.source_gridfile)?;
            let num_mp_step = vec![source_mesh.m_points.len()];
            MkgrdFinalDomainPostprocReport::Earth(run_mask_postproc_earth_domain(
                postproc_plan,
                MaskPostprocEarthRunOptions {
                    mask_sea_ratio: config.mask_sea_ratio,
                    minlon_dm_area,
                    maxlat_dm_area,
                    nlons_dm_select,
                    nlats_dm_select,
                    lon_vertex: options.area_judge.lon_vertex,
                    lat_vertex: options.area_judge.lat_vertex,
                    lon_i: options.area_judge.lon_i,
                    lat_i: options.area_judge.lat_i,
                    num_mp_step: &num_mp_step,
                    sjx_points: source_mesh.m_points.len(),
                },
            )?)
        }
        "landmesh" => {
            let postproc_plan = require_mask_postproc_plan(postproc_plan.as_ref(), mesh_type)?;
            let selected_seaorland = select_area_judge_source_window_fortran_order(
                &restart.area.seaorland.seaorland,
                bounds,
            )?;
            MkgrdFinalDomainPostprocReport::Land(run_mask_postproc_land_domain(
                postproc_plan,
                MaskPostprocLandRunOptions {
                    seaorland: &selected_seaorland,
                    minlon_dm_area,
                    maxlat_dm_area,
                    nlons_dm_select,
                    nlats_dm_select,
                    lon_vertex: options.area_judge.lon_vertex,
                    lat_vertex: options.area_judge.lat_vertex,
                    lon_i: options.area_judge.lon_i,
                    lat_i: options.area_judge.lat_i,
                },
            )?)
        }
        "oceanmesh" => {
            let postproc_plan = require_mask_postproc_plan(postproc_plan.as_ref(), mesh_type)?;
            MkgrdFinalDomainPostprocReport::Ocean(run_mask_postproc_ocean_domain(
                postproc_plan,
                MaskPostprocOceanRunOptions {
                    mask_sea_ratio: config.mask_sea_ratio,
                    num_vertex: options.num_vertex,
                },
            )?)
        }
        "atmos" | "atmosmesh" => match config.output_format.trim() {
            "MPAS" => {
                MkgrdFinalDomainPostprocReport::AtmosFull(write_mask_postproc_atmos_mpas_netcdf(
                    &restart.plan.remask.file_dir,
                    nxp,
                    usize::try_from(restart.plan.remask.step).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "mask_restart remask step must fit usize for MPAS postproc",
                        )
                    })?,
                    mode_grid,
                    mesh_type,
                    config.output_format.trim(),
                )?)
            }
            "MPAS-Simple" => {
                MkgrdFinalDomainPostprocReport::Atmos(write_mask_postproc_atmos_mpas_simple_netcdf(
                    &restart.plan.remask.file_dir,
                    nxp,
                    mode_grid,
                    mesh_type,
                    config.output_format.trim(),
                )?)
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "atmosmesh final postproc supports output_format MPAS/MPAS-Simple, got {other}"
                    ),
                ));
            }
        },
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mask_restart Area_judge postproc does not support mesh_type {other}"),
            ));
        }
    };

    Ok(MkgrdRestartAreaJudgePostprocRunReport {
        restart,
        contain,
        postproc,
    })
}
