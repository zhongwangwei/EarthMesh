use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, RefineConfig};
use earthmesh_mesh::{
    grid_cartesian_xy_to_lonlat_placeholders_fortran_indexed_state,
    grid_xyz2lonlat_fortran_indexed_state, pcvt_adjust_voronoi_grid_state,
    voronoi_grid_from_olam_delaunay_mesh, voronoi_grid_from_olam_delaunay_mesh_cartesian,
    OlamDelaunayMesh,
};

use super::outputs::write_olam_refined_outputs;
use crate::*;

/// Execute global specified refinement directly through the OLAM
/// Delaunay/Voronoi mesh layer.
pub fn run_mkgrd_olam_specified_refine_global_source_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
    source_gridnum_perdegree: Option<usize>,
) -> io::Result<MkgrdOlamSpecifiedRefineRunReport> {
    let namelist_source = namelist_source.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let is_atmosmesh = matches!(config.mesh_type.trim(), "atmos" | "atmosmesh");
    let native_mdomain = read_olam_native_mdomain(&contents)?;
    let native_deltax = read_olam_native_deltax(&contents)?;
    let native_global_like_domain =
        native_mdomain.map_or(config.mask_domain_global, |mdomain| mdomain < 2);
    let native_surface_global_domain =
        native_mdomain.map_or(config.mask_domain_global, |mdomain| mdomain == 0);
    let native_sfcgrid_res_factor = read_olam_native_sfcgrid_res_factor(&contents)?;
    let native_surface_global_expansion = !is_atmosmesh && native_sfcgrid_res_factor > 1;
    let native_olam_regions_requested =
        olam_native_refinement_requested(&contents, config.mesh_type.trim())?;
    let hfield_options = crate::hfield_refine::read_hfield_refine_options(&contents)?;
    if !config.refine && !native_surface_global_expansion && !native_olam_regions_requested {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OLAM specified refine requires NL%refine=.true.",
        ));
    }
    if !matches!(
        config.mesh_type.trim(),
        "atmos" | "atmosmesh" | "landmesh" | "oceanmesh" | "LOCmesh" | "earthmesh"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OLAM global-source specified refine currently supports atmos, atmosmesh, landmesh, oceanmesh, LOCmesh, and earthmesh",
        ));
    }
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for OLAM specified refine",
        ));
    }
    let uses_existing_mode_file = PathBuf::from(config.mode_file.trim()).exists();
    let native_global_grid_requested = native_mdomain.is_some()
        || native_olam_regions_requested
        || native_surface_global_expansion;
    if native_global_grid_requested
        && native_global_like_domain
        && !uses_existing_mode_file
        && config.nxp % 3 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be divisible by 3 for an OLAM global run",
        ));
    }
    if config.niter < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "niter must be non-negative for OLAM specified refine",
        ));
    }
    let native_atmosphere_regions =
        read_olam_native_refinement_regions_for_grid(&contents, true, native_global_like_domain)?;
    let native_surface_regions = if is_atmosmesh {
        Vec::new()
    } else {
        read_olam_native_refinement_regions_for_grid(&contents, false, native_global_like_domain)?
    };
    if !is_atmosmesh
        && !native_surface_global_domain
        && (native_surface_global_expansion
            || !native_atmosphere_regions.is_empty()
            || !native_surface_regions.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native OLAM surface Method-C grids require a global domain",
        ));
    }
    let native_regions =
        read_olam_native_refinement_regions(&contents, is_atmosmesh, native_global_like_domain)?;
    if !native_regions.is_empty() {
        validate_olam_native_method_c_spawn_mdomain(native_mdomain)?;
    }
    let refine = match RefineConfig::from_mkrefine_namelist(
        &contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
    ) {
        Ok(refine) => refine,
        Err(_err) if !native_regions.is_empty() || native_surface_global_expansion => {
            read_olam_native_refine_controls(&contents)?
        }
        Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidInput, err)),
    };
    if !refine.refine_spc
        && !refine.refine_cal
        && native_regions.is_empty()
        && !native_surface_global_expansion
    {
        return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM direct path requires refine_spc, refine_cal, or native OLAM ngrids/nsfcgrids to be active.",
            ));
    }
    let max_spc_level = if refine.refine_spc {
        final_quality_non_negative_usize(
            refine.max_iter_spc,
            "OLAM specified refine max_iter_spc must be non-negative",
        )?
    } else {
        0
    };
    let max_cal_level = if refine.refine_cal {
        final_quality_non_negative_usize(
            refine.max_iter_cal,
            "OLAM calculated refine max_iter_cal must be non-negative",
        )?
    } else {
        0
    };
    let max_native_level = olam_native_refinement_depth(&contents, is_atmosmesh)?;
    let max_surface_expansion_level = usize::from(native_surface_global_expansion);
    let max_level = max_spc_level
        .max(max_cal_level)
        .max(max_native_level)
        .max(max_surface_expansion_level);
    if refine.refine_spc && !(1..=5).contains(&max_spc_level) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OLAM direct refine max_iter_spc/max_iter_cal must select a level in 1..=5",
        ));
    }
    if refine.refine_cal && !(1..=5).contains(&max_cal_level) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OLAM direct refine max_iter_spc/max_iter_cal must select a level in 1..=5",
        ));
    }

    let native_only_spawn = !native_regions.is_empty() && !refine.refine_spc && !refine.refine_cal;
    let native_cartesian_xy = olam_native_method_c_uses_cartesian_xy(
        native_mdomain,
        config.mask_domain_global,
        native_only_spawn,
    );
    let olam_nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;

    let gridinit = run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)?;
    let mut regions = native_regions;
    if refine.refine_spc {
        regions.extend(read_olam_specified_refinement_regions(
            &refine,
            max_spc_level,
            olam_nxp,
        )?);
    }
    if refine.refine_cal {
        regions.extend(read_olam_calculated_refinement_regions(
            &refine,
            max_cal_level,
        )?);
    }
    if regions.is_empty() && !native_surface_global_expansion {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "OLAM direct refine found no region sources",
        ));
    }

    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let mesh = if let Some(mesh) =
        olam_native_initial_delaunay_mesh(nxp, native_mdomain, native_deltax)?
    {
        mesh
    } else {
        let source_gridfile = read_unstructured_mesh_netcdf(&gridinit.gridfile.output)?;
        olam_delaunay_mesh_from_unstructured_gridfile(
            &source_gridfile,
            nxp,
            usize::try_from(config.niter).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "NL%niter must fit usize")
            })?,
            f64::from(config.beta),
            f64::from(config.relax),
            max_tris,
        )?
    };
    let spring_nest_iterations = if native_only_spawn {
        if !is_atmosmesh {
            let atmosphere_iterations = if native_atmosphere_regions.is_empty() {
                0
            } else {
                olam_native_method_c_spring_iterations(&refine, true, &config.runtype)?
            };
            let surface_iterations = if native_surface_regions.is_empty() {
                0
            } else {
                olam_native_method_c_spring_iterations(&refine, false, &config.runtype)?
            };
            atmosphere_iterations.max(surface_iterations)
        } else {
            olam_native_method_c_spring_iterations(&refine, is_atmosmesh, &config.runtype)?
        }
    } else if native_surface_global_expansion
        && native_surface_regions.is_empty()
        && !refine.refine_spc
        && !refine.refine_cal
    {
        0
    } else {
        olam_method_c_spring_iterations(&refine, is_atmosmesh)?
    };
    let (mesh, spring_nest_passes) = if !is_atmosmesh
        && (native_only_spawn || native_surface_global_expansion)
        && !refine.refine_spc
        && !refine.refine_cal
    {
        let atmosphere_max_level = native_atmosphere_regions
            .iter()
            .map(olam_refinement_region_level)
            .max()
            .unwrap_or(0);
        let surface_max_level = native_surface_regions
            .iter()
            .map(olam_refinement_region_level)
            .max()
            .unwrap_or(0);
        let atmosphere_spring_iterations =
            olam_native_method_c_spring_iterations(&refine, true, &config.runtype)?;
        let surface_spring_iterations =
            olam_native_method_c_spring_iterations(&refine, false, &config.runtype)?;
        let (mesh, atmosphere_spring_passes) = if atmosphere_max_level > 0 {
            if atmosphere_spring_iterations > 0 {
                if native_cartesian_xy {
                    mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                        &native_atmosphere_regions,
                        atmosphere_max_level,
                        OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
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
                            OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
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
                "OLAM native nxp_sfc overflows usize",
            )
        })?;
        let (mesh, surface_spring_passes) = if native_surface_regions.is_empty() {
            (mesh, 0)
        } else if surface_spring_iterations > 0 {
            if native_cartesian_xy {
                mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                    &native_surface_regions,
                    surface_max_level,
                    OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
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
                        OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                    )?
                } else {
                    mesh.spawn_nest_as_surface(&native_surface_regions, surface_max_level)?
                },
                0,
            )
        };
        (mesh, atmosphere_spring_passes + surface_spring_passes)
    } else if let (Some(hfield), false) = (hfield_options.as_ref(), native_cartesian_xy) {
        // H-field mode: compose the same specified regions into a
        // gradient-limited cell-width field and let quantized target levels
        // drive Method-C ("split between levels" with legality by
        // construction). Spherical runs only; Cartesian-XY keeps the
        // geometric region path.
        let base_m = hfield.base_m.unwrap_or_else(|| {
            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * nxp as f64)
        });
        let field_max_level = hfield.max_level.unwrap_or(max_level).clamp(1, 5);
        let field = crate::hfield_refine::build_hfield_from_regions(
            &regions,
            base_m,
            hfield.g,
            hfield.nlon,
            hfield.nlat,
        )?;
        mesh.spawn_nest_from_target_levels_with_spring(
            |lon, lat| field.level_at(lon, lat, base_m, field_max_level as u8),
            field_max_level,
            if is_atmosmesh {
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS
            } else {
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
            },
            nxp,
            spring_nest_iterations,
        )?
    } else if spring_nest_iterations > 0 {
        if native_cartesian_xy {
            mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                &regions,
                max_level,
                if is_atmosmesh {
                    OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS
                } else {
                    OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
                },
                nxp,
                spring_nest_iterations,
                native_deltax,
            )?
        } else if is_atmosmesh {
            mesh.spawn_nest_with_spring_and_max_mrows(
                &regions,
                max_level,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                nxp,
                spring_nest_iterations,
            )?
        } else {
            mesh.spawn_nest_with_spring_and_max_mrows(
                &regions,
                max_level,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
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
                    OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS
                } else {
                    OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE
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
        let mut state = voronoi_grid_from_olam_delaunay_mesh_cartesian(
            &mesh,
            earthmesh_core::EARTH_RADIUS_METERS,
        )?;
        grid_cartesian_xy_to_lonlat_placeholders_fortran_indexed_state(&mut state.grid)?;
        state
    } else {
        let mut state =
            voronoi_grid_from_olam_delaunay_mesh(&mesh, earthmesh_core::EARTH_RADIUS_METERS)?;
        pcvt_adjust_voronoi_grid_state(&mut state)?;
        grid_xyz2lonlat_fortran_indexed_state(&mut state.grid)?;
        state
    };

    let file_dir = PathBuf::from(config.file_dir());
    let domain_region = read_olam_domain_region(&config)?;
    let output_mesh = gridfile_mesh_from_fortran_indexed_state(&state.grid, &state.tabs)?;
    let outputs = write_olam_refined_outputs(
        &contents,
        &config,
        source_gridnum_perdegree,
        &file_dir,
        nxp,
        max_level,
        &output_mesh,
        domain_region.as_ref(),
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

    Ok(MkgrdOlamSpecifiedRefineRunReport {
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
