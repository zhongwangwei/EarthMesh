use crate::final_quality_non_negative_usize;
use crate::gridfile_mesh_from_one_based_state;
use crate::method_c_delaunay_mesh_from_unstructured_gridfile;
use crate::method_c_refinement_region_level;
use crate::method_c_spring_iterations;
use crate::native_grid_refinement_depth;
use crate::native_grid_refinement_requested;
use crate::native_initial_delaunay_mesh;
use crate::native_spawn_spring_iterations;
use crate::native_spawn_uses_cartesian_xy;
use crate::read_method_c_calculated_refinement_regions;
use crate::read_method_c_domain_region;
use crate::read_method_c_specified_refinement_regions;
use crate::read_native_grid_deltax;
use crate::read_native_grid_mdomain;
use crate::read_native_grid_refine_controls;
use crate::read_native_grid_refinement_regions;
use crate::read_native_grid_refinement_regions_for_grid;
use crate::read_native_grid_sfcgrid_res_factor;
use crate::read_unstructured_mesh_netcdf;
use crate::run_mkgrd_gridinit_global_namelist;
use crate::validate_native_spawn_mdomain;
use crate::MethodCGridfileMetadataSlices;
use crate::RefinePipelineRunReport;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, RefineConfig};
use earthmesh_mesh::{
    grid_cartesian_xy_to_lonlat_placeholders_one_based_state, grid_xyz2lonlat_one_based_state,
    pcvt_adjust_voronoi_grid_state, voronoi_grid_from_method_c_delaunay_mesh,
    voronoi_grid_from_method_c_delaunay_mesh_cartesian, MethodCDelaunayMesh,
};

use super::outputs::{write_method_c_refined_outputs, MethodCMetadataSlices};

/// Execute global specified refinement directly through the Method-C
/// Delaunay/Voronoi mesh layer.
pub fn run_refine_pipeline_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
) -> io::Result<RefinePipelineRunReport> {
    let namelist_source = namelist_source.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let is_atmosmesh = matches!(config.mesh_type.trim(), "atmos" | "atmosmesh");
    let native_mdomain = read_native_grid_mdomain(&contents)?;
    let native_deltax = read_native_grid_deltax(&contents)?;
    let native_global_like_domain =
        native_mdomain.map_or(config.mask_domain_global, |mdomain| mdomain < 2);
    let native_surface_global_domain =
        native_mdomain.map_or(config.mask_domain_global, |mdomain| mdomain == 0);
    let native_sfcgrid_res_factor = read_native_grid_sfcgrid_res_factor(&contents)?;
    let native_surface_global_expansion = !is_atmosmesh && native_sfcgrid_res_factor > 1;
    let native_refine_regions_requested =
        native_grid_refinement_requested(&contents, config.mesh_type.trim())?;
    if !config.refine && !native_surface_global_expansion && !native_refine_regions_requested {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C specified refine requires NL%refine=.true.",
        ));
    }
    if !matches!(
        config.mesh_type.trim(),
        "atmos" | "atmosmesh" | "landmesh" | "oceanmesh" | "LOCmesh" | "earthmesh"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C global-source specified refine currently supports atmos, atmosmesh, landmesh, oceanmesh, LOCmesh, and earthmesh",
        ));
    }
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for Method-C specified refine",
        ));
    }
    let hfield_options = crate::hfield_refine::read_hfield_refine_options(&contents)?;
    let hydro_hfield_max_level = hfield_options
        .as_ref()
        .map(crate::hydro_refinement_adapter::hydro_target_max_level)
        .transpose()?
        .unwrap_or(0);
    let has_hydro_hfield_source = hydro_hfield_max_level > 0;
    let uses_existing_mode_file = PathBuf::from(config.mode_file.trim()).exists();
    let native_global_grid_requested = native_mdomain.is_some()
        || native_refine_regions_requested
        || native_surface_global_expansion;
    if native_global_grid_requested
        && native_global_like_domain
        && !uses_existing_mode_file
        && config.nxp % 3 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be divisible by 3 for an Method-C global run",
        ));
    }
    if config.niter < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "niter must be non-negative for Method-C specified refine",
        ));
    }
    let native_atmosphere_regions =
        read_native_grid_refinement_regions_for_grid(&contents, true, native_global_like_domain)?;
    let native_surface_regions = if is_atmosmesh {
        Vec::new()
    } else {
        read_native_grid_refinement_regions_for_grid(&contents, false, native_global_like_domain)?
    };
    if !is_atmosmesh
        && !native_surface_global_domain
        && (native_surface_global_expansion
            || !native_atmosphere_regions.is_empty()
            || !native_surface_regions.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native Method-C surface Method-C grids require a global domain",
        ));
    }
    let native_regions =
        read_native_grid_refinement_regions(&contents, is_atmosmesh, native_global_like_domain)?;
    if !native_regions.is_empty() {
        validate_native_spawn_mdomain(native_mdomain)?;
    }
    let refine = match RefineConfig::from_mkrefine_namelist_with_external_field(
        &contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
        has_hydro_hfield_source,
    ) {
        Ok(refine) => refine,
        Err(_err) if !native_regions.is_empty() || native_surface_global_expansion => {
            read_native_grid_refine_controls(&contents)?
        }
        Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidInput, err)),
    };
    if !refine.refine_spc
        && !refine.refine_cal
        && native_regions.is_empty()
        && !native_surface_global_expansion
        && !has_hydro_hfield_source
    {
        return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C direct path requires refine_spc, refine_cal, or native Method-C ngrids/nsfcgrids to be active.",
            ));
    }
    let max_spc_level = if refine.refine_spc {
        final_quality_non_negative_usize(
            refine.max_iter_spc,
            "Method-C specified refine max_iter_spc must be non-negative",
        )?
    } else {
        0
    };
    let max_cal_level = if refine.refine_cal {
        final_quality_non_negative_usize(
            refine.max_iter_cal,
            "Method-C calculated refine max_iter_cal must be non-negative",
        )?
    } else {
        0
    };
    let max_native_level = native_grid_refinement_depth(&contents, is_atmosmesh)?;
    let max_surface_expansion_level = usize::from(native_surface_global_expansion);
    let max_level = max_spc_level
        .max(max_cal_level)
        .max(max_native_level)
        .max(max_surface_expansion_level)
        .max(hydro_hfield_max_level);
    if refine.refine_spc && !(1..=5).contains(&max_spc_level) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C direct refine max_iter_spc/max_iter_cal must select a level in 1..=5",
        ));
    }
    if refine.refine_cal && !(1..=5).contains(&max_cal_level) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C direct refine max_iter_spc/max_iter_cal must select a level in 1..=5",
        ));
    }

    let native_only_spawn = !native_regions.is_empty() && !refine.refine_spc && !refine.refine_cal;
    let native_cartesian_xy = native_spawn_uses_cartesian_xy(
        native_mdomain,
        config.mask_domain_global,
        native_only_spawn,
    ) || native_mdomain == Some(5);
    let method_c_nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let active_hfield_options = hfield_options.as_ref();
    let use_hfield_regions = active_hfield_options.is_some();
    let mesh_type = config.mesh_type.trim();
    let has_threshold_hfield_sources = use_hfield_regions
        && refine.refine_cal
        && crate::hfield_refine::has_threshold_hfield_sources(&refine, mesh_type);

    let gridinit = run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)?;
    let mut regions = native_regions;
    if refine.refine_spc {
        regions.extend(read_method_c_specified_refinement_regions(
            &refine,
            max_spc_level,
            method_c_nxp,
            !use_hfield_regions,
        )?);
    }
    let calculated_region_prefix = refine.mask_refine_cal_fprefix.trim().trim_end_matches('/');
    let has_configured_calculated_regions =
        !calculated_region_prefix.is_empty() && calculated_region_prefix != "/tmp";
    if refine.refine_cal && (!has_threshold_hfield_sources || has_configured_calculated_regions) {
        regions.extend(read_method_c_calculated_refinement_regions(
            &refine,
            max_cal_level,
        )?);
    }
    if regions.is_empty()
        && !has_threshold_hfield_sources
        && !native_surface_global_expansion
        && !has_hydro_hfield_source
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Method-C direct refine found no region sources",
        ));
    }

    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let mesh = if let Some(mesh) = native_initial_delaunay_mesh(nxp, native_mdomain, native_deltax)?
    {
        mesh
    } else {
        let source_gridfile = read_unstructured_mesh_netcdf(&gridinit.gridfile.output)?;
        let source_levels =
            crate::grid_quality_pipeline::read_gridfile_mesh_points(&gridinit.gridfile.output)?;
        method_c_delaunay_mesh_from_unstructured_gridfile(
            &source_gridfile,
            MethodCGridfileMetadataSlices {
                m_refine_level: (!source_levels.m_refine_level.is_empty())
                    .then_some(source_levels.m_refine_level.as_slice()),
                m_refine_level_orig: (!source_levels.m_refine_level_orig.is_empty())
                    .then_some(source_levels.m_refine_level_orig.as_slice()),
                m_ngr: (!source_levels.m_ngr.is_empty()).then_some(source_levels.m_ngr.as_slice()),
                w_refine_level: (!source_levels.w_refine_level.is_empty())
                    .then_some(source_levels.w_refine_level.as_slice()),
                w_refine_level_orig: (!source_levels.w_refine_level_orig.is_empty())
                    .then_some(source_levels.w_refine_level_orig.as_slice()),
                w_ngr: (!source_levels.w_ngr.is_empty()).then_some(source_levels.w_ngr.as_slice()),
            },
            nxp,
            usize::try_from(config.niter).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "NL%niter must fit usize")
            })?,
            config.beta,
            config.relax,
            max_tris,
        )?
    };
    let spring_nest_iterations = if native_only_spawn {
        if !is_atmosmesh {
            let atmosphere_iterations = if native_atmosphere_regions.is_empty() {
                0
            } else {
                native_spawn_spring_iterations(&refine, true, &config.runtype)?
            };
            let surface_iterations = if native_surface_regions.is_empty() {
                0
            } else {
                native_spawn_spring_iterations(&refine, false, &config.runtype)?
            };
            atmosphere_iterations.max(surface_iterations)
        } else {
            native_spawn_spring_iterations(&refine, is_atmosmesh, &config.runtype)?
        }
    } else if native_surface_global_expansion
        && native_surface_regions.is_empty()
        && !refine.refine_spc
        && !refine.refine_cal
    {
        0
    } else {
        method_c_spring_iterations(&refine, is_atmosmesh)?
    };
    let (mesh, spring_nest_passes) = if !is_atmosmesh
        && (native_only_spawn || native_surface_global_expansion)
        && !refine.refine_spc
        && !refine.refine_cal
    {
        let atmosphere_max_level = native_atmosphere_regions
            .iter()
            .map(method_c_refinement_region_level)
            .max()
            .unwrap_or(0);
        let surface_max_level = native_surface_regions
            .iter()
            .map(method_c_refinement_region_level)
            .max()
            .unwrap_or(0);
        let atmosphere_spring_iterations =
            native_spawn_spring_iterations(&refine, true, &config.runtype)?;
        let surface_spring_iterations =
            native_spawn_spring_iterations(&refine, false, &config.runtype)?;
        let (mesh, atmosphere_spring_passes) = if atmosphere_max_level > 0 {
            if atmosphere_spring_iterations > 0 {
                if native_cartesian_xy {
                    mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                        &native_atmosphere_regions,
                        atmosphere_max_level,
                        MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                        nxp,
                        atmosphere_spring_iterations,
                        native_deltax,
                    )?
                } else {
                    mesh.spawn_nest_with_spring_as_atmosmesh(
                        &native_atmosphere_regions,
                        atmosphere_max_level,
                        nxp,
                        atmosphere_spring_iterations,
                    )?
                }
            } else {
                (
                    if native_cartesian_xy {
                        mesh.spawn_nest_cartesian_xy_with_max_mrows(
                            &native_atmosphere_regions,
                            atmosphere_max_level,
                            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                        )?
                    } else {
                        mesh.spawn_nest_as_atmosmesh(
                            &native_atmosphere_regions,
                            atmosphere_max_level,
                        )?
                    },
                    0,
                )
            }
        } else {
            (mesh, 0)
        };
        let mesh = if native_surface_global_expansion {
            mesh.expand_by_factor(native_sfcgrid_res_factor)?
        } else {
            mesh
        };
        let surface_nxp = nxp.checked_mul(native_sfcgrid_res_factor).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C native nxp_sfc overflows usize",
            )
        })?;
        let (mesh, surface_spring_passes) = if native_surface_regions.is_empty() {
            (mesh, 0)
        } else if surface_spring_iterations > 0 {
            if native_cartesian_xy {
                mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                    &native_surface_regions,
                    surface_max_level,
                    MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                    surface_nxp,
                    surface_spring_iterations,
                    native_deltax,
                )?
            } else {
                mesh.spawn_nest_with_spring(
                    &native_surface_regions,
                    surface_max_level,
                    surface_nxp,
                    surface_spring_iterations,
                )?
            }
        } else {
            (
                if native_cartesian_xy {
                    mesh.spawn_nest_cartesian_xy_with_max_mrows(
                        &native_surface_regions,
                        surface_max_level,
                        MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                    )?
                } else {
                    mesh.spawn_nest_as_surface(&native_surface_regions, surface_max_level)?
                },
                0,
            )
        };
        (mesh, atmosphere_spring_passes + surface_spring_passes)
    } else if let Some(hfield) = active_hfield_options {
        // H-field mode: compose the same specified regions into a
        // gradient-limited cell-width field and let quantized target levels
        // drive Method-C ("split between levels" with legality by
        // construction). Spherical runs sample lon/lat rasters; Cartesian-XY
        // runs sample the same region constraints analytically in x/y meters.
        let base_m = hfield.base_m.unwrap_or_else(|| {
            if native_cartesian_xy {
                native_deltax
            } else {
                2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS
                    / (5.0 * nxp as f64)
            }
        });
        let field_max_level = hfield.max_level.unwrap_or(max_level).clamp(1, 5);
        let max_mrows = if is_atmosmesh {
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS
        } else {
            MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
        };
        if native_cartesian_xy && has_hydro_hfield_source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hydro target-cell h-field requires a spherical lon/lat Method-C run",
            ));
        }
        if native_cartesian_xy {
            let geographic_threshold_field = if has_threshold_hfield_sources {
                hfield.geographic_origin.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cartesian-XY geographic threshold rasters require hfield_origin_lon and hfield_origin_lat",
                    )
                })?;
                Some(crate::hfield_refine::build_composed_hfield(
                    &[],
                    &refine,
                    mesh_type,
                    Some(&config),
                    base_m,
                    hfield,
                    max_cal_level.clamp(1, field_max_level),
                )?)
            } else {
                None
            };
            for region in &regions {
                region.validate_cartesian_xy()?;
            }
            // An explicit h-field is a mkrefine request, not the implicit
            // native ngrids-only path; honor its niter_refine controls instead
            // of forcing Method-C's 5000-iteration native spawn default.
            let hfield_spring_iterations = method_c_spring_iterations(&refine, is_atmosmesh)?;
            mesh.spawn_nest_from_cartesian_xy_target_levels_with_spring_deltax(
                |x, y| {
                    let region_level = crate::hfield_refine::cartesian_hfield_level_at(
                        &regions,
                        x,
                        y,
                        base_m,
                        hfield.g,
                        field_max_level,
                    );
                    let threshold_level = geographic_threshold_field
                        .as_ref()
                        .map(|field| {
                            let (origin_lon, origin_lat) =
                                hfield.geographic_origin.expect("origin checked above");
                            let (lon, lat) = crate::hfield_refine::cartesian_xy_to_lonlat(
                                x, y, origin_lon, origin_lat,
                            );
                            field.level_at(lon, lat, base_m, field_max_level as u8)
                        })
                        .unwrap_or(0);
                    region_level.max(threshold_level)
                },
                field_max_level,
                max_mrows,
                nxp,
                hfield_spring_iterations,
                native_deltax,
            )?
        } else {
            let mut field = crate::hfield_refine::build_composed_hfield(
                &regions,
                &refine,
                mesh_type,
                Some(&config),
                base_m,
                hfield,
                max_cal_level.clamp(1, field_max_level),
            )?;
            crate::hydro_refinement_adapter::apply_hydro_target_to_field(
                &mut field, hfield, base_m,
            )?;
            mesh.spawn_nest_from_target_levels_with_spring(
                |lon, lat| field.level_at(lon, lat, base_m, field_max_level as u8),
                field_max_level,
                max_mrows,
                nxp,
                spring_nest_iterations,
            )?
        }
    } else if spring_nest_iterations > 0 {
        if native_cartesian_xy {
            mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                &regions,
                max_level,
                if is_atmosmesh {
                    MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS
                } else {
                    MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
                },
                nxp,
                spring_nest_iterations,
                native_deltax,
            )?
        } else if is_atmosmesh {
            mesh.spawn_nest_with_spring_and_max_mrows(
                &regions,
                max_level,
                MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                nxp,
                spring_nest_iterations,
            )?
        } else {
            mesh.spawn_nest_with_spring_and_max_mrows(
                &regions,
                max_level,
                MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                nxp,
                spring_nest_iterations,
            )?
        }
    } else if native_cartesian_xy {
        (
            mesh.spawn_nest_cartesian_xy_with_max_mrows(
                &regions,
                max_level,
                if is_atmosmesh {
                    MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS
                } else {
                    MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
                },
            )?,
            0,
        )
    } else if is_atmosmesh {
        (mesh.spawn_nest_as_atmosmesh(&regions, max_level)?, 0)
    } else {
        (mesh.spawn_nest(&regions, max_level)?, 0)
    };
    let transition_faces = mesh.boundary_rows().len();

    let state = if native_cartesian_xy {
        let mut state = voronoi_grid_from_method_c_delaunay_mesh_cartesian(
            &mesh,
            earthmesh_core::EARTH_RADIUS_METERS,
        )?;
        grid_cartesian_xy_to_lonlat_placeholders_one_based_state(&mut state.grid)?;
        state
    } else {
        let mut state =
            voronoi_grid_from_method_c_delaunay_mesh(&mesh, earthmesh_core::EARTH_RADIUS_METERS)?;
        pcvt_adjust_voronoi_grid_state(&mut state)?;
        grid_xyz2lonlat_one_based_state(&mut state.grid)?;
        state
    };

    let file_dir = PathBuf::from(config.file_dir());
    let domain_region = read_method_c_domain_region(&config)?;
    let output_mesh = gridfile_mesh_from_one_based_state(&state.grid, &state.tabs)?;
    let m_refine_levels = method_c_m_refine_levels_zero_based(&state)?;
    let m_refine_levels_orig = method_c_m_refine_levels_orig_zero_based(&state)?;
    let m_ngr = method_c_m_ngr(&state)?;
    let w_refine_levels = method_c_w_refine_levels_zero_based(&state)?;
    let w_refine_levels_orig = method_c_w_refine_levels_orig_zero_based(&state)?;
    let w_ngr = method_c_w_ngr(&state)?;
    let outputs = write_method_c_refined_outputs(
        &contents,
        &config,
        source_gridnum_perdegree,
        &file_dir,
        nxp,
        max_level,
        &output_mesh,
        domain_region.as_ref(),
        Some(MethodCMetadataSlices {
            m_refine_level: &m_refine_levels,
            m_refine_level_orig: &m_refine_levels_orig,
            m_ngr: &m_ngr,
            w_refine_level: &w_refine_levels,
            w_refine_level_orig: &w_refine_levels_orig,
            w_ngr: &w_ngr,
        }),
    )?;

    let mut runtime_state =
        EarthmeshRuntimeState::new(config.clone()).with_refine_config(refine.clone());
    runtime_state.grid = state.grid;
    runtime_state.ijtabs = state.tabs;
    runtime_state
        .record_pentagon_indices_from_icosahedron(state.impent)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    runtime_state
        .record_mesh_counts_for_step(max_level, runtime_state.grid.nma, runtime_state.grid.nwa)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    Ok(RefinePipelineRunReport {
        gridinit,
        refine,
        regions,
        max_level,
        transition_faces,
        spring_nest_passes,
        spring_nest_iterations,
        raw_output: outputs.raw_output,
        landtype_masked_cells: outputs.landtype_masked_cells,
        coupled_outputs: outputs.coupled_outputs,
        output: outputs.output,
        runtime_state,
    })
}

fn method_c_m_refine_levels_zero_based(
    state: &earthmesh_mesh::VoronoiGridState,
) -> io::Result<Vec<i32>> {
    if state.tabs.m.len() <= state.grid.nma {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Method-C M refinement levels missing from itab_m",
        ));
    }
    (1..=state.grid.nma)
        .map(|im| method_c_level_to_zero_based(state.tabs.m[im].mrlm, "M", im))
        .collect::<io::Result<Vec<_>>>()
}

fn method_c_w_refine_levels_zero_based(
    state: &earthmesh_mesh::VoronoiGridState,
) -> io::Result<Vec<i32>> {
    if state.tabs.w.len() <= state.grid.nwa {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Method-C W refinement levels missing from itab_w",
        ));
    }
    (1..=state.grid.nwa)
        .map(|iw| method_c_level_to_zero_based(state.tabs.w[iw].mrlw, "W", iw))
        .collect::<io::Result<Vec<_>>>()
}

fn method_c_m_refine_levels_orig_zero_based(
    state: &earthmesh_mesh::VoronoiGridState,
) -> io::Result<Vec<i32>> {
    (1..=state.grid.nma)
        .map(|im| method_c_level_to_zero_based(state.tabs.m[im].mrlm_orig, "M orig", im))
        .collect()
}

fn method_c_w_refine_levels_orig_zero_based(
    state: &earthmesh_mesh::VoronoiGridState,
) -> io::Result<Vec<i32>> {
    (1..=state.grid.nwa)
        .map(|iw| method_c_level_to_zero_based(state.tabs.w[iw].mrlw_orig, "W orig", iw))
        .collect()
}

fn method_c_m_ngr(state: &earthmesh_mesh::VoronoiGridState) -> io::Result<Vec<i32>> {
    method_c_ngr_values((1..=state.grid.nma).map(|im| state.tabs.m[im].ngr), "M")
}

fn method_c_w_ngr(state: &earthmesh_mesh::VoronoiGridState) -> io::Result<Vec<i32>> {
    method_c_ngr_values((1..=state.grid.nwa).map(|iw| state.tabs.w[iw].ngr), "W")
}

fn method_c_ngr_values(values: impl Iterator<Item = i32>, role: &str) -> io::Result<Vec<i32>> {
    values
        .enumerate()
        .map(|(row, value)| {
            if row == 0 && value <= 0 {
                Ok(0)
            } else if value > 0 {
                Ok(value)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C {role} ngr at row {} must be positive, got {value}",
                        row + 1
                    ),
                ))
            }
        })
        .collect()
}

fn method_c_level_to_zero_based(level: i32, role: &str, index: usize) -> io::Result<i32> {
    if index == 1 && level <= 0 {
        return Ok(0);
    }
    if level <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Method-C {role} refinement level at row {index} must be one-based and positive, got {level}"),
        ));
    }
    Ok(level - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_core::{GridMemory, IjTabs, ItabM, ItabW};

    fn minimal_state(mrlm: i32, mrlw: i32) -> earthmesh_mesh::VoronoiGridState {
        earthmesh_mesh::VoronoiGridState {
            grid: GridMemory {
                nma: 2,
                nwa: 2,
                ..GridMemory::default()
            },
            tabs: IjTabs {
                m: vec![
                    ItabM::default(),
                    ItabM::default(),
                    ItabM {
                        mrlm,
                        ..ItabM::default()
                    },
                ],
                v: Vec::new(),
                w: vec![
                    ItabW::default(),
                    ItabW::default(),
                    ItabW {
                        mrlw,
                        ..ItabW::default()
                    },
                ],
            },
            impent: [0; 12],
        }
    }

    #[test]
    fn method_c_refine_level_export_rejects_non_positive_one_based_levels() {
        let bad_m = minimal_state(0, 1);
        let err = method_c_m_refine_levels_zero_based(&bad_m)
            .expect_err("zero Method-C M level must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        let bad_w = minimal_state(1, 0);
        let err = method_c_w_refine_levels_zero_based(&bad_w)
            .expect_err("zero Method-C W level must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
