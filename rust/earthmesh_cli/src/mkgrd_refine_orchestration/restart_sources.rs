use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshConfig;

use crate::*;

/// Run the migrated restart-refine path from a compact source-state handoff.
///
/// This is the library-owned counterpart to the direct
/// `--run-mask-restart-area-judge-refine` CLI mode: it parses the mkgrd
/// namelist, locates the restart Area_judge grid, reconstructs compact
/// source-state axes, builds final-domain contain/postprocess options, and runs
/// the migrated restart-refine stack with the standard Rust working-state
/// executor.
pub fn run_mkgrd_restart_refine_compact_source_state_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    source_state_path: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    mask_postproc_num_vertex: Option<usize>,
) -> io::Result<MkgrdRestartRefineCompactSourceStateNamelistRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let initial_gridfile = initial_gridfile.as_ref();
    let source_bundle = read_mkgrd_compact_restart_refine_source_state(source_state_path)?;
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let restart_input = PathBuf::from(config.file_dir()).join("result/IsInDmArea_grid.nc4");
    let effective_mask_postproc_num_vertex = match mask_postproc_num_vertex {
        Some(num_vertex) => Some(num_vertex),
        None if config.mesh_type.trim() == "oceanmesh" => {
            maybe_infer_mask_restart_ocean_num_vertex_from_config(&config)?
                .or(Some(source_bundle.source_state.num_vertex))
        }
        None if matches!(config.mesh_type.trim(), "earthmesh" | "landmesh") => {
            maybe_infer_mask_restart_non_ocean_num_vertex_from_config(&config)?
                .or(Some(source_bundle.source_state.num_vertex))
        }
        None if matches!(config.mesh_type.trim(), "atmos" | "atmosmesh") => {
            Some(source_bundle.source_state.num_vertex)
        }
        None => None,
    };
    let final_postproc_requested = effective_mask_postproc_num_vertex.is_some();
    let derived_seaorland =
        final_postproc_requested.then(|| source_bundle.source_state.seaorland.clone());
    let contain_options = if let Some(seaorland) = derived_seaorland.as_ref() {
        restart_refine_final_contain_options(
            &restart_input,
            config.mesh_type.trim(),
            effective_mask_postproc_num_vertex,
            seaorland,
            &source_bundle.axes.lon_vertex,
            &source_bundle.axes.lat_vertex,
            &source_bundle.axes.lon_i,
            &source_bundle.axes.lat_i,
        )?
    } else {
        None
    };
    let selected_land_domain = if final_postproc_requested
        && matches!(config.mesh_type.trim(), "earthmesh" | "landmesh")
    {
        let restart_payload = read_area_judge_grid_netcdf(&restart_input)?;
        Some(selected_land_domain_from_area_judge_grid_payload(
            &restart_payload,
        )?)
    } else {
        None
    };
    let final_postproc_request = restart_refine_final_postproc_request(
        config.mesh_type.trim(),
        effective_mask_postproc_num_vertex,
        config.mask_sea_ratio,
        selected_land_domain.as_ref(),
    )?;
    let postproc_options = match &final_postproc_request {
        Some(MkgrdRestartRefineFinalPostprocRequest::Earth {
            mask_sea_ratio,
            minlon_dm_area,
            maxlat_dm_area,
            nlons_dm_select,
            nlats_dm_select,
        }) => Some(MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid(
            MkgrdFinalDomainEarthAutoPostprocOptions {
                mask_sea_ratio: *mask_sea_ratio,
                minlon_dm_area: *minlon_dm_area,
                maxlat_dm_area: *maxlat_dm_area,
                nlons_dm_select: *nlons_dm_select,
                nlats_dm_select: *nlats_dm_select,
                lon_vertex: &source_bundle.axes.lon_vertex,
                lat_vertex: &source_bundle.axes.lat_vertex,
                lon_i: &source_bundle.axes.lon_i,
                lat_i: &source_bundle.axes.lat_i,
            },
        )),
        Some(MkgrdRestartRefineFinalPostprocRequest::Land(context)) => Some(
            MkgrdFinalDomainPostprocOptions::Land(MaskPostprocLandRunOptions {
                seaorland: &context.selected_seaorland,
                minlon_dm_area: context.minlon_dm_area,
                maxlat_dm_area: context.maxlat_dm_area,
                nlons_dm_select: context.nlons_dm_select,
                nlats_dm_select: context.nlats_dm_select,
                lon_vertex: &source_bundle.axes.lon_vertex,
                lat_vertex: &source_bundle.axes.lat_vertex,
                lon_i: &source_bundle.axes.lon_i,
                lat_i: &source_bundle.axes.lat_i,
            }),
        ),
        Some(MkgrdRestartRefineFinalPostprocRequest::Ocean {
            mask_sea_ratio,
            num_vertex,
        }) => Some(MkgrdFinalDomainPostprocOptions::Ocean(
            MaskPostprocOceanRunOptions {
                mask_sea_ratio: *mask_sea_ratio,
                num_vertex: *num_vertex,
            },
        )),
        Some(MkgrdRestartRefineFinalPostprocRequest::Atmos) => {
            Some(MkgrdFinalDomainPostprocOptions::Atmos {
                output_format: config.output_format.trim(),
            })
        }
        None => None,
    };
    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    let report = run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor_and_final_domain_contain(
        namelist_source,
        workdir,
        source_bundle.area_judge_restart_refine_loop_options(&restart_input, initial_gridfile),
        &mut executor,
        contain_options,
        postproc_options,
    )?;

    Ok(MkgrdRestartRefineCompactSourceStateNamelistRunReport {
        source_bundle,
        report,
    })
}

/// Run the migrated restart-refine path from `NL%landtype_file` without
/// CLI-side orchestration.
///
/// This is the library-owned counterpart to
/// `--run-mask-restart-area-judge-refine-landtype-source`: it parses mkgrd
/// config, reads `data_preprocess` landtype axes/landtypes, locates the restart
/// Area_judge grid, builds final-domain contain/postprocess options, and runs
/// the migrated restart-refine stack with the standard Rust working-state
/// executor.
pub fn run_mkgrd_restart_refine_landtype_source_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    initial_gridfile: impl AsRef<Path>,
    source_gridnum_perdegree: Option<usize>,
    source_first_triangle_id: usize,
    mask_postproc_num_vertex: Option<usize>,
) -> io::Result<MkgrdRestartRefineLandtypeSourceNamelistRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let initial_gridfile = initial_gridfile.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let gridnum_perdegree = match source_gridnum_perdegree {
        Some(value) => value,
        None => usize::try_from(config.gridnum_perdegree).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "NL%gridnum_perdegree must be positive for restart-refine landtype source, got {}",
                    config.gridnum_perdegree
                ),
            )
        })?,
    };
    let preprocess = read_landtype_data_preprocess_fortran_indexed(
        Path::new(config.landtype_file.trim()),
        gridnum_perdegree,
    )?;
    let source_grid = preprocess.refine_prepare_source_grid(source_first_triangle_id);
    let restart_input = PathBuf::from(config.file_dir()).join("result/IsInDmArea_grid.nc4");
    let mode_num_vertex = mkgrd_mode_grid_num_vertex(config.mode_grid.trim())?;
    let effective_mask_postproc_num_vertex = match mask_postproc_num_vertex {
        Some(num_vertex) => Some(num_vertex),
        None if config.mesh_type.trim() == "oceanmesh" => {
            maybe_infer_mask_restart_ocean_num_vertex_from_config(&config)?
                .or(Some(mode_num_vertex))
        }
        None if matches!(config.mesh_type.trim(), "earthmesh" | "landmesh") => {
            let default_num_vertex = if config.mesh_type.trim() == "landmesh" {
                1
            } else {
                mode_num_vertex
            };
            maybe_infer_mask_restart_non_ocean_num_vertex_from_config(&config)?
                .or(Some(default_num_vertex))
        }
        None if config.mesh_type.trim() == "atmosmesh" => Some(mode_num_vertex),
        None => None,
    };
    let final_postproc_requested = effective_mask_postproc_num_vertex.is_some();
    let derived_seaorland = final_postproc_requested
        .then(|| seaorland_from_landtypes_global_fortran_indexed(&preprocess.landtypes_global));
    let contain_options = if let Some(seaorland) = derived_seaorland.as_ref() {
        restart_refine_final_contain_options(
            &restart_input,
            config.mesh_type.trim(),
            effective_mask_postproc_num_vertex,
            seaorland,
            &preprocess.lon_vertex,
            &preprocess.lat_vertex,
            &preprocess.lon_i,
            &preprocess.lat_i,
        )?
    } else {
        None
    };
    let selected_land_domain = if final_postproc_requested
        && matches!(config.mesh_type.trim(), "earthmesh" | "landmesh")
    {
        let restart_payload = read_area_judge_grid_netcdf(&restart_input)?;
        Some(selected_land_domain_from_area_judge_grid_payload(
            &restart_payload,
        )?)
    } else {
        None
    };
    let final_postproc_request = restart_refine_final_postproc_request(
        config.mesh_type.trim(),
        effective_mask_postproc_num_vertex,
        config.mask_sea_ratio,
        selected_land_domain.as_ref(),
    )?;
    let postproc_options = match &final_postproc_request {
        Some(MkgrdRestartRefineFinalPostprocRequest::Earth {
            mask_sea_ratio,
            minlon_dm_area,
            maxlat_dm_area,
            nlons_dm_select,
            nlats_dm_select,
        }) => Some(MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid(
            MkgrdFinalDomainEarthAutoPostprocOptions {
                mask_sea_ratio: *mask_sea_ratio,
                minlon_dm_area: *minlon_dm_area,
                maxlat_dm_area: *maxlat_dm_area,
                nlons_dm_select: *nlons_dm_select,
                nlats_dm_select: *nlats_dm_select,
                lon_vertex: &preprocess.lon_vertex,
                lat_vertex: &preprocess.lat_vertex,
                lon_i: &preprocess.lon_i,
                lat_i: &preprocess.lat_i,
            },
        )),
        Some(MkgrdRestartRefineFinalPostprocRequest::Land(context)) => Some(
            MkgrdFinalDomainPostprocOptions::Land(MaskPostprocLandRunOptions {
                seaorland: &context.selected_seaorland,
                minlon_dm_area: context.minlon_dm_area,
                maxlat_dm_area: context.maxlat_dm_area,
                nlons_dm_select: context.nlons_dm_select,
                nlats_dm_select: context.nlats_dm_select,
                lon_vertex: &preprocess.lon_vertex,
                lat_vertex: &preprocess.lat_vertex,
                lon_i: &preprocess.lon_i,
                lat_i: &preprocess.lat_i,
            }),
        ),
        Some(MkgrdRestartRefineFinalPostprocRequest::Ocean {
            mask_sea_ratio,
            num_vertex,
        }) => Some(MkgrdFinalDomainPostprocOptions::Ocean(
            MaskPostprocOceanRunOptions {
                mask_sea_ratio: *mask_sea_ratio,
                num_vertex: *num_vertex,
            },
        )),
        Some(MkgrdRestartRefineFinalPostprocRequest::Atmos) => {
            Some(MkgrdFinalDomainPostprocOptions::Atmos {
                output_format: config.output_format.trim(),
            })
        }
        None => None,
    };
    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    let report = run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor_and_final_domain_contain(
        namelist_source,
        workdir,
        MkgrdAreaJudgeRestartRefineLoopOptions {
            restart_input: &restart_input,
            initial_gridfile,
            source_grid,
            landtypes_global: &preprocess.landtypes_global,
            num_vertex: mode_num_vertex,
            maxlc: preprocess.maxlc,
        },
        &mut executor,
        contain_options,
        postproc_options,
    )?;

    Ok(MkgrdRestartRefineLandtypeSourceNamelistRunReport { preprocess, report })
}
