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
use crate::GridRegion;
use crate::MethodCGridfileMetadataSlices;
use crate::RefinePipelineRunReport;
use earthmesh_refine_method_c::MethodCMesh;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, RefineConfig};
use earthmesh_mesh::{
    grid_cartesian_xy_to_lonlat_placeholders_one_based_state, grid_xyz2lonlat_one_based_state,
    pcvt_adjust_voronoi_grid_state, voronoi_grid_from_triangular_mesh,
    voronoi_grid_from_triangular_mesh_cartesian, TriangularMesh,
};

use super::outputs::{write_refined_outputs, MethodCMetadataSlices};

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
    let adaptive_options = crate::adaptive_refine::read_adaptive_refine_options(&contents)?;
    if hfield_options.is_some() && config.nxp % 3 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Method-C HField refinement requires NXP divisible by 3; got {} (use {} or another higher multiple of 3)",
                config.nxp,
                config.nxp
                    .checked_add((3 - config.nxp.rem_euclid(3)) % 3)
                    .unwrap_or(config.nxp)
            ),
        ));
    }
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
    let domain_region = read_method_c_domain_region(&config)?;
    let use_hfield_regions = active_hfield_options.is_some();
    let mesh_type = config.mesh_type.trim();
    let has_threshold_hfield_sources = use_hfield_regions
        && refine.refine_cal
        && crate::hfield_refine::has_threshold_hfield_sources(&refine, mesh_type);
    // Whether *some* backend is going to consume the criteria itself. The
    // legacy calculated-region reader must stand down for either of them, not
    // just for the h-field: with the point+radius route the criteria are the
    // demand planner's business, and letting the reader also run sends it to
    // look for mask files that a criteria-driven run never has.
    let backend_consumes_criteria =
        has_threshold_hfield_sources || (adaptive_options.is_some() && refine.refine_cal);

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
    // `refine_cal` says a criterion decides where to refine. On red-green the
    // point+radius route is what reads one, so this only bites when that route
    // is off too: mask *files* are served either way, since a mask file is a
    // named region by another name, but a criterion with neither a file nor
    // `&adaptive` behind it has nowhere to go.
    //
    // Said now rather than at the backend branch, because the reader below runs
    // first -- and on the unconfigured prefix it fails with a message about
    // Method-C and a `/tmp` path nobody typed.
    if config.refine_backend.trim() == "red_green"
        && refine.refine_cal
        && adaptive_options.is_none()
        && !has_configured_calculated_regions
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "NL%refine_backend = red_green has no reader for calculated criteria with the \
             point+radius route off: it refines named regions, and &hfield is Method-C's. Enable \
             &adaptive, point RL%mask_refine_cal_fprefix at mask files, or use method_c",
        ));
    }
    if refine.refine_cal && (!backend_consumes_criteria || has_configured_calculated_regions) {
        regions.extend(read_method_c_calculated_refinement_regions(
            &refine,
            max_cal_level,
        )?);
    }
    if regions.is_empty()
        && !backend_consumes_criteria
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
                m_lineage: None,
                w_lineage: None,
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
    let file_dir = PathBuf::from(config.file_dir());
    // The choice of backend, at the one place where it is a choice. Method-C
    // continues as a `TriangularMesh` through the Voronoi/PCVT step; red-green's
    // mesh is already in lon/lat and skips it entirely. What the tail below
    // needs is the same from either -- a gridfile mesh, and whatever each one
    // can honestly say about how it was built.
    let RefinedGrid {
        state,
        output_mesh,
        method_c_metadata,
        pentagon_indices,
        transition_faces,
        spring_nest_passes,
        hfield_diagnostics,
        adaptive_run,
    } = match config.refine_backend.trim() {
        "red_green" => {
            // What this route does not read, said outright rather than served
            // quietly with less: any of these would simply be dropped, and the
            // run would still write a valid mesh that passes every quality check
            // it has and is not the mesh that was asked for.
            //
            // `&adaptive` is not on this list. Its criteria half is a shared
            // upstream -- raster work that produces an ordinary circle list --
            // and red-green consumes it below. Only turning circles into mesh is
            // per-backend, which is exactly the half suspended on Method-C.
            let unsupported = if active_hfield_options.is_some() {
                Some("an h-field (&hfield)")
            } else if native_cartesian_xy {
                Some("a Cartesian-XY mesh")
            } else if native_surface_global_expansion {
                Some("the native surface expansion (NL%sfcgrid_res_factor)")
            } else {
                None
            };
            if let Some(unsupported) = unsupported {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "NL%refine_backend = red_green does not serve {unsupported}; it refines \
                         named regions and the point+radius criteria. Use method_c for this run"
                    ),
                ));
            }
            let adaptive = adaptive_options
                .as_ref()
                .map(|adaptive| -> io::Result<RedGreenAdaptive<'_>> {
                    Ok(RedGreenAdaptive {
                        inputs: crate::refinement_demand::plan::DemandPlanInputs {
                            bounds: adaptive_demand_bounds(domain_region.as_ref(), &config)?,
                            gridnum_perdegree: usize::try_from(config.gridnum_perdegree).map_err(
                                |_| {
                                    io::Error::new(
                                        io::ErrorKind::InvalidInput,
                                        "NL%gridnum_perdegree must fit usize",
                                    )
                                },
                            )?,
                            landtype_file: adaptive_landtype_file(&config),
                            mesh_type,
                            refine_coastline: adaptive.coastline,
                        },
                        base_cell_meters: adaptive.base_m.unwrap_or_else(|| {
                            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS
                                / (5.0 * method_c_nxp as f64)
                        }),
                        coastline: adaptive.coastline,
                    })
                })
                .transpose()?;
            refine_with_redgreen(&mesh, &regions, &refine, max_level, adaptive)?
        }
        "harp_dv" => refine_with_harp_dv(
            &mesh,
            &regions,
            method_c_nxp,
            usize::try_from(refine.niter_refine).unwrap_or(0),
        )?,
        _ => {
            let MethodCRefineOutcome {
                mesh,
                spring_nest_passes,
                hfield_diagnostics,
                adaptive_run,
            } = refine_with_method_c(
                mesh,
                MethodCRefineRequest {
                    config: &config,
                    refine: &refine,
                    mesh_type,
                    regions: &regions,
                    native_atmosphere_regions: &native_atmosphere_regions,
                    native_surface_regions: &native_surface_regions,
                    domain_region: domain_region.as_ref(),
                    hfield_options: active_hfield_options,
                    adaptive_options: adaptive_options.as_ref(),
                    is_atmosmesh,
                    native_only_spawn,
                    native_surface_global_expansion,
                    native_cartesian_xy,
                    native_deltax,
                    native_sfcgrid_res_factor,
                    nxp,
                    method_c_nxp,
                    max_level,
                    max_cal_level,
                    has_hydro_hfield_source,
                    has_threshold_hfield_sources,
                    spring_nest_iterations,
                },
            )?;
            let state = if native_cartesian_xy {
                let mut state = voronoi_grid_from_triangular_mesh_cartesian(
                    &mesh,
                    earthmesh_core::EARTH_RADIUS_METERS,
                )?;
                grid_cartesian_xy_to_lonlat_placeholders_one_based_state(&mut state.grid)?;
                state
            } else {
                spherical_voronoi_state(&mesh)?
            };
            let output_mesh = gridfile_mesh_from_one_based_state(&state.grid, &state.tabs)?;
            let method_c_metadata = Some(gridfile_metadata(&state, &mesh)?);
            RefinedGrid {
                transition_faces: mesh.boundary_rows().len(),
                // The twelve pentagons are the icosahedron's, taken here off the
                // refined mesh -- which is the numbering the run record wants --
                // rather than off the Voronoi `state`, which not every backend
                // has.
                pentagon_indices: mesh.impent,
                state: Some(state),
                output_mesh,
                method_c_metadata,
                spring_nest_passes,
                hfield_diagnostics,
                adaptive_run,
            }
        }
    };

    // Measured from the produced mesh, not from the request: a pass whose demand
    // is clipped away stops descending without failing, and `max_level` alone
    // cannot show that. A backend that reports no levels has nothing measured
    // here, which is honest -- zero says "not known from this mesh", and the
    // requested `max_level` travels separately.
    let realized_max_level = method_c_metadata
        .as_ref()
        .map(|meta| {
            meta.w_refine_levels
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
                .max(0) as usize
        })
        .unwrap_or(0);
    // Measured off the produced mesh, so it means the same thing whichever
    // backend made it -- see the field docs for why `realized_max_level` does
    // not.
    // Percentiles, not extremes. The mask carve leaves partial cells at a
    // coastline, and on this very run the smallest was 2.4 km against a
    // nominal 300 km -- so a min/max pair reports the carve rather than the
    // refinement, and `log2(max/min)` came out at 12 halvings for a two-level
    // request.
    let (finest_cell_km, coarsest_cell_km) = {
        let mut across: Vec<f64> = Vec::with_capacity(output_mesh.w_to_m.len());
        let radius_km = earthmesh_core::EARTH_RADIUS_METERS / 1000.0;
        for corners in &output_mesh.w_to_m {
            let polygon: Vec<earthmesh_mesh::LonLatDegrees> = corners
                .iter()
                .filter_map(|&im| {
                    let row = usize::try_from(im).ok()?.checked_sub(1)?;
                    let point = output_mesh.m_points.get(row)?;
                    Some(earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat))
                })
                .collect();
            if polygon.len() < 3 {
                continue;
            }
            let Some(steradians) = earthmesh_mesh::robust_spherical_area_unit(&polygon) else {
                continue;
            };
            if !steradians.is_finite() || steradians <= 0.0 {
                continue;
            }
            across.push((steradians / std::f64::consts::PI).sqrt() * radius_km);
        }
        if across.is_empty() {
            (0.0, 0.0)
        } else {
            across.sort_by(|left, right| {
                left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
            });
            let at = |fraction: f64| {
                let index = ((across.len() - 1) as f64 * fraction).round() as usize;
                across[index]
            };
            (at(0.02), at(0.98))
        }
    };

    // Which cells the run named outright. The carve's largest-component rule
    // would otherwise delete a refinement circle sitting on a small bay, and
    // nothing would report that the region asked for is gone.
    //
    // The carve indexes this by centre id, and which points are centres depends
    // on the view: `hex` cells are centred on W points, `tri` cells on M points.
    // Sampling the wrong array does not fail loudly -- the lookup is bounds
    // checked -- it protects unrelated cells and leaves the demanded ones to be
    // carved away, which is the failure this array exists to prevent.
    let hard_center_demand = adaptive_run.as_ref().map(|(report, _, _, _)| {
        let centers = match config.mode_grid.trim() {
            "tri" => &output_mesh.m_points,
            _ => &output_mesh.w_points,
        };
        centers
            .iter()
            .map(|point| report.target_level_at(point.lon, point.lat) > 0)
            .collect::<Vec<bool>>()
    });
    let outputs = write_refined_outputs(
        &contents,
        &config,
        source_gridnum_perdegree,
        &file_dir,
        nxp,
        max_level,
        &output_mesh,
        domain_region.as_ref(),
        method_c_metadata
            .as_ref()
            .map(|meta| MethodCMetadataSlices {
                m_lineage: &meta.m_lineages,
                w_lineage: &meta.w_lineages,
                m_refine_level: &meta.m_refine_levels,
                m_refine_level_orig: &meta.m_refine_levels_orig,
                m_ngr: &meta.m_ngr,
                w_refine_level: &meta.w_refine_levels,
                w_refine_level_orig: &meta.w_refine_levels_orig,
                w_ngr: &meta.w_ngr,
            }),
        hard_center_demand.as_deref(),
    )?;

    // Beside the final gridfile, where the quality step can find it: both it and
    // the saved namelist live in `<case>/result/`.
    if let Some((report, depth, base_m, coastline)) = &adaptive_run {
        if let Some(directory) = outputs.output.output.parent() {
            let path = directory.join(crate::refinement_demand::nest::ADAPTIVE_REFINEMENT_FILE);
            std::fs::write(&path, report.to_json(*depth, *base_m, *coastline)).map_err(
                |error| io::Error::new(error.kind(), format!("write {}: {error}", path.display())),
            )?;
        }
    }

    let mut runtime_state =
        EarthmeshRuntimeState::new(config.clone()).with_refine_config(refine.clone());
    match state {
        Some(state) => {
            runtime_state.grid = state.grid;
            runtime_state.ijtabs = state.tabs;
        }
        // Red-green has no Voronoi state to hand over -- its mesh arrives in
        // lon/lat and never passes through one. The counts the step record
        // wants are the gridfile's own rows, which is what Method-C's
        // `nma`/`nwa` are as well; the tables stay empty because there are none
        // to fill, not because they were dropped.
        None => {
            runtime_state.grid.nma = output_mesh.m_points.len();
            runtime_state.grid.nwa = output_mesh.w_points.len();
        }
    }
    runtime_state
        .record_pentagon_indices_from_icosahedron(pentagon_indices)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    runtime_state
        .record_mesh_counts_for_step(max_level, runtime_state.grid.nma, runtime_state.grid.nwa)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    Ok(RefinePipelineRunReport {
        gridinit,
        refine,
        regions,
        max_level,
        realized_max_level,
        finest_cell_km,
        coarsest_cell_km,
        hfield_diagnostics,
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

/// Everything the gridfile carries that only Method-C can say.
///
/// Owned rather than borrowed because `MethodCMetadataSlices` borrows all of it
/// and has to outlive the call that consumes it. A backend with no generations
/// or ancestry to report leaves this `None` and the writer serves it the same.
struct MethodCMetadataOwned {
    m_refine_levels: Vec<i32>,
    m_refine_levels_orig: Vec<i32>,
    m_ngr: Vec<i32>,
    w_refine_levels: Vec<i32>,
    w_refine_levels_orig: Vec<i32>,
    w_ngr: Vec<i32>,
    m_lineages: Vec<i64>,
    w_lineages: Vec<i64>,
}

/// A refined mesh in the shape the rest of the pipeline reads, whichever
/// backend built it.
///
/// The fields a backend cannot fill are `Option` or zero rather than invented:
/// a fabricated level count or a fabricated ngr table would read as measured
/// and be wrong, which is the failure this whole path is built to avoid.
struct RefinedGrid {
    /// Method-C's Voronoi state. Red-green has none -- its mesh is already in
    /// lon/lat -- and the run record fills its counts from `output_mesh`.
    state: Option<earthmesh_mesh::VoronoiGridState>,
    output_mesh: crate::UnstructuredMesh,
    method_c_metadata: Option<MethodCMetadataOwned>,
    /// The twelve pentagons, in the numbering of the mesh that was produced.
    pentagon_indices: [usize; 12],
    /// Method-C's refinement-boundary rows. Zero from red-green: it builds its
    /// transition band a different way and does not count it in these terms, so
    /// zero here reads "not measured from this mesh", the same answer
    /// `realized_max_level` gives.
    transition_faces: usize,
    spring_nest_passes: usize,
    hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics,
    adaptive_run: Option<AdaptiveRunRecord>,
}

/// Most incident triangles a cell may have.
///
/// Method-C holds vertex degree to {5, 6, 7} by construction, and everything
/// downstream is sized for it: `mask_postproc_neighbor_widths` gives 7 for the
/// polygon side either way round, and `IcosahedronMPointNeighbors` carries
/// seven slots.
const REDGREEN_MAX_CELL_DEGREE: usize = 7;

/// Triangles left holding an edge no other triangle owns.
///
/// A mesh of the whole sphere has none: every edge is shared by exactly two
/// triangles. The subdivision steps used to leave them wherever their per
/// triangle antimeridian rotation fired for one of two triangles sharing an
/// edge but not the other, which is fixed -- see
/// `refine_onedivide_four_renew`.
///
/// Kept because of how that failed rather than because one is expected: only
/// the level *after* the one that opened the edges would say so, as "ngrmm row
/// N has invalid neighbor 0", and a single-level run has no next level. It
/// writes the gridfile, and the gridfile opens.
fn redgreen_open_edges(mesh: &earthmesh_refine_redgreen::RedGreenMesh) -> usize {
    let Some(rows) = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
        &mesh.cells_on_triangle,
        &mesh.triangles_on_cell,
        &mesh.n_triangles_on_cell,
    ) else {
        // Membership that does not resolve at all is worse than an open edge,
        // not better; report it as every triangle being suspect.
        return mesh.triangle_count();
    };
    (mesh.num_vertex + 1..=mesh.triangle_count())
        .filter(|&triangle| rows[triangle].contains(&0))
        .count()
}

/// The criteria half of the point+radius route, as red-green consumes it.
struct RedGreenAdaptive<'a> {
    inputs: crate::refinement_demand::plan::DemandPlanInputs<'a>,
    base_cell_meters: f64,
    coastline: bool,
}

/// Refine by red-green: mark the triangles the regions ask for, split each into
/// four, close the seams by halving the neighbours left hanging, once per level.
///
/// Unlike Method-C this never refuses a region for its shape -- the judge chain
/// grows a marking until the triangulation closes -- which is the whole reason
/// the backend exists, and why the criteria route is served here and suspended
/// there. A criterion's demand has whatever shape the data has.
fn refine_with_redgreen(
    mesh: &TriangularMesh,
    named_regions: &[earthmesh_mesh::RefinementRegion],
    refine: &RefineConfig,
    max_level: usize,
    adaptive: Option<RedGreenAdaptive<'_>>,
) -> io::Result<RefinedGrid> {
    if !refine.is_transition {
        // Not only for a second level: the transition rows *are* red-green's
        // closure step, so without them even one level comes out with hanging
        // nodes -- 345 open edges on the shipped atmosphere example in tri mode.
        //
        // The engine allows the setting for `mode_grid = 'tri'` alone, and
        // Method-C closes without it, so this is red-green's limit rather than
        // the configuration's. Said here rather than met later as an open-edge
        // count or, at a second level, as "ngrmm row N has invalid neighbor 0".
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "red-green refinement requires RL%Istransition = .true.: the transition rows are what \
             close the seams a 1-into-4 split leaves, so without them the mesh has hanging nodes \
             at any depth. Method-C closes without them; use it for this run",
        ));
    }
    let mut redgreen =
        earthmesh_refine_redgreen::redgreen_mesh_from_triangular(mesh, &mesh.m_neighbors)?;
    let mut output_mesh = crate::redgreen_bridge::unstructured_mesh_from_redgreen(&redgreen)?;
    let mut previous_marks: Option<Vec<i32>> = None;
    let mut split_triangles = 0usize;
    let mut passes = Vec::new();
    let mut deepest_level = 0usize;
    let mut stopped_on_empty_demand = false;
    for level in 1..=max_level {
        let before = redgreen.triangle_count();
        // Named regions carry their own target level, so a deeper one is also
        // refined by every level above it -- that is the `>= level` the marking
        // applies. Criteria circles are planned *for* this level and nest by
        // radius instead, so they are added as they come.
        let mut level_regions: Vec<earthmesh_mesh::RefinementRegion> = named_regions.to_vec();
        let mut demanded_cells = 0usize;
        if let Some(adaptive) = &adaptive {
            let demand = crate::refinement_demand::nest::adaptive_demand_circles_for_level(
                refine,
                &adaptive.inputs,
                level,
                adaptive.base_cell_meters,
                max_level,
            )?;
            demanded_cells = demand.demanded_cells;
            eprintln!(
                "red-green refine level {level} judging {:.0} m cells: {} circles over {} \
                 demanded source cells",
                adaptive.base_cell_meters / 2f64.powi((level - 1) as i32),
                demand.circles.len(),
                demand.demanded_cells,
            );
            level_regions.extend(demand.circles);
        }
        // Nothing asks at this depth, and nothing deeper will either: the
        // criteria stopped and the named regions that reach here are gone.
        if !level_regions.iter().any(|region| region.level() >= level) {
            stopped_on_empty_demand = true;
            break;
        }
        let (written, outcome) = crate::redgreen_bridge::refine_redgreen_level(
            &redgreen,
            &level_regions,
            refine,
            level,
            previous_marks.as_deref(),
        )?;
        eprintln!(
            "red-green refine level {level}: {} triangles split, {} grown by the judges, \
             {} dropped as isolated, {} cancelled outside the halo, {} flipped, {before} -> {} triangles",
            outcome.refined_triangle_count,
            outcome.grown_triangle_count,
            outcome.isolated_dropped_count,
            outcome.halo_cancelled_count,
            outcome.flipped_triangle_count,
            outcome.mesh.triangle_count(),
        );
        // The degree the gridfile's dual and the mask post-process are built
        // for. Method-C guarantees {5, 6, 7} by construction; red-green only
        // reaches it by taking back, with Lawson flips, the degree each
        // transition split adds. Checked rather than trusted because a run
        // without a carve -- an atmosphere mesh -- would otherwise write a cell
        // the readers cannot address and say nothing.
        let widest_cell = (outcome.mesh.num_center + 1..=outcome.mesh.cell_count())
            .map(|cell| outcome.mesh.n_triangles_on_cell[cell])
            .max()
            .unwrap_or(0);
        if widest_cell > REDGREEN_MAX_CELL_DEGREE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "red-green level {level} produced a cell with {widest_cell} incident \
                     triangles; the gridfile's dual and the mask post-process address at most \
                     {REDGREEN_MAX_CELL_DEGREE}"
                ),
            ));
        }
        // Checked here rather than trusted, because the next level is the only
        // thing that would otherwise notice -- and a run that stops at this
        // level has no next level.
        let open_edges = redgreen_open_edges(&outcome.mesh);
        if open_edges > 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "red-green level {level} left {open_edges} triangle edge(s) with no \
                     neighbouring triangle, so the mesh does not close. Writing it would produce \
                     a gridfile that opens and carries a hole, and only a level after this one \
                     would otherwise notice"
                ),
            ));
        }
        split_triangles += outcome.refined_triangle_count;
        // What this level refined, in the numbering the *next* level will ask
        // about. Not the array just handed in: that one indexes the triangles
        // of the mesh this round consumed, and a round renumbers them. The
        // outcome's `cell_renumbering` cannot carry it either -- it maps cells,
        // and a marking is per triangle.
        //
        // Asking the regions again on the new mesh is the same question in the
        // right numbering, and where it differs it errs the safe way: the judge
        // chain grew this level's region past what was asked, so the recomputed
        // interior is the smaller of the two and the next level is held further
        // inside the transition band, never further out.
        previous_marks = Some(crate::redgreen_bridge::redgreen_marking_from_regions(
            &outcome.mesh,
            &level_regions,
            level,
        ));
        passes.push(crate::refinement_demand::nest::NestPassReport {
            level,
            cell_meters: adaptive
                .as_ref()
                .map(|adaptive| adaptive.base_cell_meters / 2f64.powi((level - 1) as i32))
                .unwrap_or(0.0),
            circle_count: level_regions.len(),
            regions: level_regions,
            demanded_cells,
            faces_before: before,
            faces_after: outcome.mesh.triangle_count(),
        });
        deepest_level = level;
        redgreen = outcome.mesh;
        output_mesh = written;
    }
    if split_triangles == 0 {
        // A run that asked to refine and refined nothing is the failure that
        // stays quiet: the gridfile opens, the quality checks pass, and the
        // mesh is uniform.
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "red-green refinement was requested over {} named region(s){} up to level \
                 {max_level} but no triangle was split; check that the regions carry a level in \
                 1..={max_level}, and that they or the criteria cover triangle centres of a mesh \
                 this coarse",
                named_regions.len(),
                if adaptive.is_some() {
                    " and the enabled criteria"
                } else {
                    ""
                }
            ),
        ));
    }
    Ok(RefinedGrid {
        state: None,
        output_mesh,
        method_c_metadata: None,
        // Red-green renumbers each round, but `vertex_mapping` is the identity
        // over the cells that went in, so a base-mesh cell keeps its id through
        // every level. The pentagons are base-mesh cells.
        pentagon_indices: mesh.impent,
        transition_faces: 0,
        spring_nest_passes: 0,
        hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics::default(),
        // Reported for the same two reasons Method-C reports it: the ocean
        // carve reads it to protect the cells a criterion demanded from its
        // largest-component rule, and the quality step reads the written file
        // to ask whether the mesh reached the level the circles asked for.
        // Without it a coastal circle sitting on a small bay is carved away and
        // nothing says the region asked for is gone.
        adaptive_run: adaptive.map(|adaptive| {
            (
                crate::refinement_demand::nest::AdaptiveNestReport {
                    passes,
                    deepest_level,
                    stopped_on_empty_demand,
                },
                max_level,
                adaptive.base_cell_meters,
                adaptive.coastline,
            )
        }),
    })
}

/// Everything the Method-C refinement chain reads.
///
/// One struct rather than two dozen parameters: the chain used to be a single
/// expression in the middle of the pipeline, and what it closed over is what it
/// now takes. Gathering them is what let the choice of backend become a branch
/// at this call rather than a condition threaded through the chain.
struct MethodCRefineRequest<'a> {
    config: &'a EarthmeshConfig,
    refine: &'a RefineConfig,
    mesh_type: &'a str,
    regions: &'a [earthmesh_mesh::RefinementRegion],
    native_atmosphere_regions: &'a [earthmesh_mesh::RefinementRegion],
    native_surface_regions: &'a [earthmesh_mesh::RefinementRegion],
    domain_region: Option<&'a GridRegion>,
    hfield_options: Option<&'a crate::hfield_refine::HfieldRefineOptions>,
    adaptive_options: Option<&'a crate::adaptive_refine::AdaptiveRefineOptions>,
    is_atmosmesh: bool,
    native_only_spawn: bool,
    native_surface_global_expansion: bool,
    native_cartesian_xy: bool,
    native_deltax: f64,
    native_sfcgrid_res_factor: usize,
    nxp: usize,
    method_c_nxp: usize,
    max_level: usize,
    max_cal_level: usize,
    has_hydro_hfield_source: bool,
    has_threshold_hfield_sources: bool,
    spring_nest_iterations: usize,
}

/// What a Method-C refinement produced, and the three things the pipeline tail
/// reports about how it got there.
struct MethodCRefineOutcome {
    mesh: MethodCMesh,
    spring_nest_passes: usize,
    hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics,
    adaptive_run: Option<AdaptiveRunRecord>,
}

/// The adaptive route's report, and the three settings needed to write it out.
type AdaptiveRunRecord = (
    crate::refinement_demand::nest::AdaptiveNestReport,
    usize,
    f64,
    bool,
);

/// Refine `mesh` the Method-C way: native spawn, adaptive point+radius, h-field
/// target levels, or plain specified regions, whichever the request selects.
/// Refine by re-reading the criteria against the cells that exist.
///
/// The one thing this route does differently at the pipeline boundary: it
/// produces its mesh from `MeshState`, so it goes out through
/// `to_triangular_mesh` rather than arriving as a `TriangularMesh` already.
fn refine_with_harp_dv(
    mesh: &earthmesh_mesh::TriangularMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    nxp: usize,
    spring_iterations: usize,
) -> io::Result<RefinedGrid> {
    use earthmesh_refine_harp_dv as harp;

    // Said outright rather than served quietly with less. Each of these would
    // otherwise be dropped and the run would still write a valid mesh that is
    // not the mesh that was asked for.
    let unsupported = regions
        .iter()
        .find(|region| !matches!(region, earthmesh_mesh::RefinementRegion::Circle { .. }))
        .map(|_| "a region that is not a circle");
    if let Some(unsupported) = unsupported {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "NL%refine_backend = harp_dv does not serve {unsupported}; it reads a target \
                 scale per cell, and only circles carry one today. Use method_c for this run"
            ),
        ));
    }

    // A named region asks for a level; HARP-DV asks for a length. One level is
    // one halving, the same relation Method-C's nesting produces, so a level-L
    // request becomes the base cell width divided by two to the L.
    let base_cell_m =
        2.0 * std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / (5.0 * nxp as f64);
    let criteria: Vec<Box<dyn harp::CellCriterion>> = regions
        .iter()
        .enumerate()
        .filter_map(|(index, region)| match region {
            earthmesh_mesh::RefinementRegion::Circle {
                center,
                radius_meters,
                level,
            } => Some(Box::new(harp::TargetScale {
                id: format!("region-{index}"),
                target_scale_m: base_cell_m / 2.0_f64.powi(*level as i32),
                region: harp::TargetRegion::Circle {
                    centre: *center,
                    radius_m: *radius_meters,
                },
                source_resolution_m: None,
            }) as Box<dyn harp::CellCriterion>),
            _ => None,
        })
        .collect();

    let adaptive = harp::AdaptiveMesh::from_triangular_mesh(mesh)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let outcome = harp::refine_harp_dv(
        adaptive,
        &harp::HarpDvRequest {
            config: harp::HarpDvConfig::default(),
            criteria: &criteria,
            candidate_policy: harp::CandidatePolicy::default(),
            gates: harp::HardGates::default(),
        },
    )
    .map_err(|error| io::Error::other(error.to_string()))?;

    // What it could not do, on the run's own output rather than in a log line
    // nobody reads afterwards.
    if !outcome.unresolved_cells.is_empty() {
        let refusals = outcome.report.refusals;
        eprintln!(
            "harp_dv: {} cells could not be refined further, and {} adjacent pairs are past the \
             neighbour scale bound; stopped because {:?}",
            outcome.unresolved_cells.len(),
            outcome.report.unbalanced_pairs_remaining,
            outcome.report.stop_reason
        );
        // By kind, because the three want different answers: a degree wall
        // wants site motion, a pentagon wall wants the demand moved off it, and
        // a ladder that ran out wants another rung.
        eprintln!(
            "harp_dv: refusals -- degree {}, pentagon {}, not insertable {}, topology {}, no \
             improvement {}, unmeasurable {}",
            refusals.degree,
            refusals.pentagon,
            refusals.not_insertable,
            refusals.topology,
            refusals.no_improvement,
            refusals.unmeasurable
        );
        if outcome.report.degree_relieving_moves > 0 {
            eprintln!(
                "harp_dv: {} moves relieved a degree wall",
                outcome.report.degree_relieving_moves
            );
        }
    }

    let refined = outcome
        .mesh
        .to_triangular_mesh()
        .map_err(|error| io::Error::other(error.to_string()))?;

    // Smooth with the nest spring, targeting each edge at what the *criteria*
    // asked for there. Guide 11.21 records the version that took targets from
    // the mesh's own current scale: that tells the spring to keep things as
    // they are, and 5000 iterations under it made the angles worse.
    let refined = if spring_iterations > 0 {
        match harp_spring_smoothed(&refined, regions, base_cell_m, spring_iterations) {
            Ok(mesh) => mesh,
            // A smoothing pass that declines is not a reason to lose the mesh.
            Err(error) => {
                eprintln!("harp_dv: nest spring declined ({error}); writing the unsmoothed mesh");
                refined
            }
        }
    } else {
        refined
    };
    let state = spherical_voronoi_state(&refined)?;
    let output_mesh = gridfile_mesh_from_one_based_state(&state.grid, &state.tabs)?;
    let method_c_metadata = Some(gridfile_metadata(&state, &refined)?);
    Ok(RefinedGrid {
        // HARP-DV builds no transition band, so there is nothing to count in
        // these terms -- the same answer red-green gives, and for the same
        // reason.
        transition_faces: 0,
        pentagon_indices: refined.impent,
        state: Some(state),
        output_mesh,
        method_c_metadata,
        spring_nest_passes: 0,
        hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics::default(),
        adaptive_run: None,
    })
}

/// Smooth a HARP-DV mesh against the sizes the criteria asked for.
///
/// The targets come from the regions, not from the mesh -- which is what
/// makes this different from the attempt in guide 11.21. A region asking for
/// level L is asking for the base cell halved L times, and that is a length
/// the spring can pull toward.
///
/// The conversion from a cell width to a triangle edge length is measured off
/// this mesh rather than derived: the two differ by a shape factor that
/// depends on the dual, and measuring it is both shorter and harder to get
/// wrong than deriving it.
fn harp_spring_smoothed(
    mesh: &earthmesh_mesh::TriangularMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    base_cell_m: f64,
    iterations: usize,
) -> io::Result<earthmesh_mesh::TriangularMesh> {
    // Median triangle edge over median cell width, on this mesh.
    let mut edges: Vec<f64> = Vec::new();
    for iu in 2..=mesh.nud {
        let [im1, im2] = mesh.u_edges[iu].im;
        let (Some(a), Some(b)) = (mesh.m_points.get(im1), mesh.m_points.get(im2)) else {
            continue;
        };
        let length = earthmesh_mesh::arc_length_unit_sphere(*a, *b);
        if length.is_finite() && length > 0.0 {
            edges.push(length);
        }
    }
    if edges.is_empty() {
        return Err(io::Error::other(
            "no edges to measure a spring target against",
        ));
    }
    edges.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let median_edge = edges[edges.len() / 2];
    // The unrefined mesh's cell width, which the median edge belongs to.
    let shape_factor = median_edge / base_cell_m;

    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let targets =
        earthmesh_refine_method_c::method_c_edge_target_lengths_from_field(mesh, |lon, lat| {
            let here = earthmesh_mesh::lonlat_degrees_to_unit_xyz(
                earthmesh_mesh::LonLatDegrees::new(lon, lat),
            );
            // The finest thing any region asks for here; the base cell where
            // none does.
            let mut width = base_cell_m;
            for region in regions {
                if let earthmesh_mesh::RefinementRegion::Circle {
                    center,
                    radius_meters,
                    level,
                } = region
                {
                    let centre = earthmesh_mesh::lonlat_degrees_to_unit_xyz(*center);
                    let dot = (here.x * centre.x + here.y * centre.y + here.z * centre.z)
                        .clamp(-1.0, 1.0);
                    if dot.acos() * radius <= *radius_meters {
                        width = width.min(base_cell_m / 2.0_f64.powi(*level as i32));
                    }
                }
            }
            width * shape_factor
        })?;
    Ok(earthmesh_refine_method_c::MethodCMesh::new(mesh.clone())
        .spring_nest_with_edge_targets(iterations, 2, true, true, &targets)?
        .into_inner())
}

/// The Voronoi/PCVT step, in lon/lat, for a mesh on the sphere.
///
/// Shared by every backend that produces a spherical mesh. The Cartesian-XY
/// route is Method-C's alone and stays where it is.
fn spherical_voronoi_state(
    mesh: &earthmesh_mesh::TriangularMesh,
) -> io::Result<earthmesh_mesh::VoronoiGridState> {
    let mut state = voronoi_grid_from_triangular_mesh(mesh, earthmesh_core::EARTH_RADIUS_METERS)?;
    pcvt_adjust_voronoi_grid_state(&mut state)?;
    grid_xyz2lonlat_one_based_state(&mut state.grid)?;
    Ok(state)
}

/// The per-row generations and ancestry the gridfile carries.
///
/// Named for the file rather than for Method-C: every backend fills these,
/// because the format has the columns.
fn gridfile_metadata(
    state: &earthmesh_mesh::VoronoiGridState,
    mesh: &earthmesh_mesh::TriangularMesh,
) -> io::Result<MethodCMetadataOwned> {
    Ok(MethodCMetadataOwned {
        m_refine_levels: method_c_m_refine_levels_zero_based(state)?,
        m_refine_levels_orig: method_c_m_refine_levels_orig_zero_based(state)?,
        m_ngr: method_c_m_ngr(state)?,
        w_refine_levels: method_c_w_refine_levels_zero_based(state)?,
        w_refine_levels_orig: method_c_w_refine_levels_orig_zero_based(state)?,
        w_ngr: method_c_w_ngr(state)?,
        // Ancestry as the mesh tracked it through every pass and renumbering.
        m_lineages: mesh.gridfile_m_cell_lineages()?,
        w_lineages: mesh.gridfile_w_cell_lineages()?,
    })
}

fn refine_with_method_c(
    mesh: TriangularMesh,
    request: MethodCRefineRequest<'_>,
) -> io::Result<MethodCRefineOutcome> {
    // Into the nesting here and back out at the boundary, so the transition
    // rows exist exactly where they mean something.
    let mesh = MethodCMesh::new(mesh);
    let MethodCRefineRequest {
        config,
        refine,
        mesh_type,
        regions,
        native_atmosphere_regions,
        native_surface_regions,
        domain_region,
        hfield_options,
        adaptive_options,
        is_atmosmesh,
        native_only_spawn,
        native_surface_global_expansion,
        native_cartesian_xy,
        native_deltax,
        native_sfcgrid_res_factor,
        nxp,
        method_c_nxp,
        max_level,
        max_cal_level,
        has_hydro_hfield_source,
        has_threshold_hfield_sources,
        spring_nest_iterations,
    } = request;
    // Captured out of the h-field branch so every arm of this chain keeps the
    // same tuple shape; stays at its default for the geometric region paths.
    let mut hfield_diagnostics =
        earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics::default();
    // Same shape as `hfield_diagnostics`: assigned inside the branch that owns
    // it, carried out to the layer that knows where the run's outputs land.
    let mut adaptive_run: Option<AdaptiveRunRecord> = None;
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
            native_spawn_spring_iterations(refine, true, &config.runtype)?;
        let surface_spring_iterations =
            native_spawn_spring_iterations(refine, false, &config.runtype)?;
        let (mesh, atmosphere_spring_passes) = if atmosphere_max_level > 0 {
            if atmosphere_spring_iterations > 0 {
                if native_cartesian_xy {
                    mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                        native_atmosphere_regions,
                        atmosphere_max_level,
                        MethodCMesh::MAX_MROWS_ATMOS,
                        nxp,
                        atmosphere_spring_iterations,
                        native_deltax,
                    )?
                } else {
                    mesh.spawn_nest_with_spring_as_atmosmesh(
                        native_atmosphere_regions,
                        atmosphere_max_level,
                        nxp,
                        atmosphere_spring_iterations,
                    )?
                }
            } else {
                (
                    if native_cartesian_xy {
                        mesh.spawn_nest_cartesian_xy_with_max_mrows(
                            native_atmosphere_regions,
                            atmosphere_max_level,
                            MethodCMesh::MAX_MROWS_ATMOS,
                        )?
                    } else {
                        mesh.spawn_nest_as_atmosmesh(
                            native_atmosphere_regions,
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
            // A shared operation, so it hands back the shared mesh and the
            // nesting has to be re-entered. Expansion emits no transition rows,
            // so there are none to carry across.
            MethodCMesh::new(mesh.expand_by_factor(native_sfcgrid_res_factor)?)
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
                    native_surface_regions,
                    surface_max_level,
                    MethodCMesh::MAX_MROWS_SURFACE,
                    surface_nxp,
                    surface_spring_iterations,
                    native_deltax,
                )?
            } else {
                mesh.spawn_nest_with_spring(
                    native_surface_regions,
                    surface_max_level,
                    surface_nxp,
                    surface_spring_iterations,
                )?
            }
        } else {
            (
                if native_cartesian_xy {
                    mesh.spawn_nest_cartesian_xy_with_max_mrows(
                        native_surface_regions,
                        surface_max_level,
                        MethodCMesh::MAX_MROWS_SURFACE,
                    )?
                } else {
                    mesh.spawn_nest_as_surface(native_surface_regions, surface_max_level)?
                },
                0,
            )
        };
        (mesh, atmosphere_spring_passes + surface_spring_passes)
    } else if let Some(adaptive) = adaptive_options {
        // Point+radius mode: ask every enabled criterion again before each pass,
        // cover what it demands with circles, and refine one level. The h-field
        // reads the same criteria but settles them all up front, so a criterion
        // whose answer depends on the cell size can only be honoured here.
        let base_m = adaptive.base_m.unwrap_or_else(|| {
            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS
                / (5.0 * method_c_nxp as f64)
        });
        let depth = adaptive.max_level.unwrap_or(max_level).clamp(1, 5);
        let inputs = crate::refinement_demand::plan::DemandPlanInputs {
            bounds: adaptive_demand_bounds(domain_region, config)?,
            gridnum_perdegree: usize::try_from(config.gridnum_perdegree).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "NL%gridnum_perdegree must fit usize",
                )
            })?,
            landtype_file: adaptive_landtype_file(config),
            mesh_type,
            refine_coastline: adaptive.coastline,
        };
        let (refined, report) =
            crate::refinement_demand::nest::spawn_nest_adaptive_with_named_regions(
                &mesh, refine, &inputs, regions, base_m, depth,
            )?;
        for pass in &report.passes {
            eprintln!(
                "adaptive refine level {} judging {:.0} m cells: {} circles over {} demanded source cells, {} -> {} faces",
                pass.level,
                pass.cell_meters,
                pass.circle_count,
                pass.demanded_cells,
                pass.faces_before,
                pass.faces_after
            );
        }
        adaptive_run = Some((report.clone(), depth, base_m, adaptive.coastline));
        if report.deepest_level == 0 {
            // A run that asked to refine and refined nothing is the failure that
            // stays quiet: the mesh is valid, passes its quality checks, and is
            // simply not the mesh that was requested. It is only acceptable when
            // nothing was named and no criterion is on -- then "uniform" is the
            // right answer.
            if refine.refine_spc || refine.refine_cal || !regions.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "adaptive refinement was requested ({} named regions, refine_spc={}, \
                         refine_cal={}) but no level refined; check that the criteria have data \
                         over the domain and that named regions carry a level in 1..={depth}",
                        regions.len(),
                        refine.refine_spc,
                        refine.refine_cal
                    ),
                ));
            }
            eprintln!("adaptive refine: nothing asked for refinement; mesh left uniform");
        }
        (refined, 0)
    } else if let Some(hfield) = hfield_options {
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
            MethodCMesh::MAX_MROWS_ATMOS
        } else {
            MethodCMesh::MAX_MROWS_SURFACE
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
                    refine,
                    mesh_type,
                    Some(config),
                    base_m,
                    hfield,
                    max_cal_level.clamp(1, field_max_level),
                    None,
                )?)
            } else {
                None
            };
            for region in regions {
                region.validate_cartesian_xy()?;
            }
            // An explicit h-field is a mkrefine request, not the implicit
            // native ngrids-only path; honor its niter_refine controls instead
            // of forcing Method-C's 5000-iteration native spawn default.
            let hfield_spring_iterations = method_c_spring_iterations(refine, is_atmosmesh)?;
            let (refined, passes, diagnostics) = mesh
                .spawn_nest_from_cartesian_xy_target_levels_with_spring_deltax(
                    |x, y| {
                        let region_level = crate::hfield_refine::cartesian_hfield_level_at(
                            regions,
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
                )?;
            hfield_diagnostics = diagnostics;
            (refined, passes)
        } else {
            let mut field = crate::hfield_refine::build_composed_hfield(
                regions,
                refine,
                mesh_type,
                Some(config),
                base_m,
                hfield,
                max_cal_level.clamp(1, field_max_level),
                domain_region,
            )?;
            crate::hydro_refinement_adapter::apply_hydro_target_to_field(
                &mut field,
                hfield,
                base_m,
                domain_region,
            )?;
            crate::hfield_refine::constrain_hfield_to_domain(
                &mut field,
                domain_region,
                base_m,
                hfield.g,
            )?;
            let (refined, passes, diagnostics) = mesh.spawn_nest_from_target_levels_with_spring(
                |lon, lat| field.level_at(lon, lat, base_m, field_max_level as u8),
                field_max_level,
                max_mrows,
                nxp,
                spring_nest_iterations,
            )?;
            hfield_diagnostics = diagnostics;
            (refined, passes)
        }
    } else if spring_nest_iterations > 0 {
        if native_cartesian_xy {
            mesh.spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                regions,
                max_level,
                if is_atmosmesh {
                    MethodCMesh::MAX_MROWS_ATMOS
                } else {
                    MethodCMesh::MAX_MROWS_SURFACE
                },
                nxp,
                spring_nest_iterations,
                native_deltax,
            )?
        } else if is_atmosmesh {
            mesh.spawn_nest_with_spring_and_max_mrows(
                regions,
                max_level,
                MethodCMesh::MAX_MROWS_ATMOS,
                nxp,
                spring_nest_iterations,
            )?
        } else {
            mesh.spawn_nest_with_spring_and_max_mrows(
                regions,
                max_level,
                MethodCMesh::MAX_MROWS_SURFACE,
                nxp,
                spring_nest_iterations,
            )?
        }
    } else if native_cartesian_xy {
        (
            mesh.spawn_nest_cartesian_xy_with_max_mrows(
                regions,
                max_level,
                if is_atmosmesh {
                    MethodCMesh::MAX_MROWS_ATMOS
                } else {
                    MethodCMesh::MAX_MROWS_SURFACE
                },
            )?,
            0,
        )
    } else if is_atmosmesh {
        (mesh.spawn_nest_as_atmosmesh(regions, max_level)?, 0)
    } else {
        (mesh.spawn_nest(regions, max_level)?, 0)
    };

    Ok(MethodCRefineOutcome {
        mesh,
        spring_nest_passes,
        hfield_diagnostics,
        adaptive_run,
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

/// The window the adaptive route evaluates criteria over.
///
/// A regional run judges its own domain; a global run judges the whole sphere,
/// which is what `source_bounds_for_bbox` returns for the full range.
///
/// Every regional shape has to be covered here. A shape that fell through to the
/// global range would raster the whole planet for a domain a few degrees across:
/// the demand grid is `nlons * nlats` at `gridnum_perdegree`, so the 120 per
/// degree the shipped examples use is 43200 x 21600 -- about 930 million cells
/// to allocate and scan before the first refinement pass.
fn adaptive_demand_bounds(
    domain_region: Option<&GridRegion>,
    config: &EarthmeshConfig,
) -> io::Result<earthmesh_mesh::AreaJudgeSourceBounds> {
    let gridnum_perdegree = usize::try_from(config.gridnum_perdegree).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%gridnum_perdegree must fit usize",
        )
    })?;
    let (west, east, south, north) = match domain_region.map(region_lonlat_bounds) {
        Some(Some(bounds)) => bounds,
        _ => (-180.0, 180.0, -90.0, 90.0),
    };
    crate::refinement_demand::source_bounds_for_bbox(west, east, south, north, gridnum_perdegree)
}

/// Enclosing lon/lat box of a regional shape, or `None` when it has no bound.
fn region_lonlat_bounds(region: &GridRegion) -> Option<(f64, f64, f64, f64)> {
    match region {
        GridRegion::Bbox {
            west,
            east,
            south,
            north,
        } => Some((*west, *east, *south, *north)),
        GridRegion::Circle {
            lon,
            lat,
            radius_km,
        } => {
            // A circle's enclosing box, clipped to the sphere. Demand outside
            // the circle is harmless: the reduction only emits circles where a
            // criterion actually asked for one.
            let degrees = radius_km / 111.195;
            let lat_pad = degrees;
            let lon_pad = degrees / lat.to_radians().cos().abs().max(0.05);
            Some((
                (lon - lon_pad).max(-180.0),
                (lon + lon_pad).min(180.0),
                (lat - lat_pad).max(-90.0),
                (lat + lat_pad).min(90.0),
            ))
        }
        // A closed curve -- a watershed, a coastline traced by hand -- is the
        // shape a project most often draws, and its box is just its extent.
        GridRegion::Close { points } => {
            let mut bounds: Option<(f64, f64, f64, f64)> = None;
            for point in points {
                if !point.lon.is_finite() || !point.lat.is_finite() {
                    continue;
                }
                bounds = Some(match bounds {
                    None => (point.lon, point.lon, point.lat, point.lat),
                    Some((west, east, south, north)) => (
                        west.min(point.lon),
                        east.max(point.lon),
                        south.min(point.lat),
                        north.max(point.lat),
                    ),
                });
            }
            bounds
        }
        // A union covers each member, so its box covers all of them.
        GridRegion::Any(regions) => {
            regions
                .iter()
                .filter_map(region_lonlat_bounds)
                .reduce(|left, right| {
                    (
                        left.0.min(right.0),
                        left.1.max(right.1),
                        left.2.min(right.2),
                        left.3.max(right.3),
                    )
                })
        }
    }
}

/// Land-type raster for the adaptive route, or `None` when the run has none.
fn adaptive_landtype_file(config: &EarthmeshConfig) -> Option<&std::path::Path> {
    let path = config.landtype_file.trim();
    (!path.is_empty() && path != "none").then(|| std::path::Path::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A demand grid is `nlons * nlats`, so this is the cost of the window.
    fn demand_cells(region: Option<&GridRegion>, per_degree: i32) -> usize {
        let config = EarthmeshConfig {
            gridnum_perdegree: per_degree,
            ..EarthmeshConfig::default()
        };
        let bounds = adaptive_demand_bounds(region, &config).expect("bounds");
        (bounds.maxlon_source - bounds.minlon_source + 1)
            * (bounds.minlat_source - bounds.maxlat_source + 1)
    }

    #[test]
    fn a_closed_curve_domain_is_judged_over_itself_not_the_planet() {
        // A shape with no arm of its own fell through to the whole sphere. At
        // the 120 per degree the examples ship, that is a ~930 million cell
        // grid allocated and scanned for a domain a few degrees across -- no
        // error, just a run that will not finish.
        let watershed = GridRegion::Close {
            points: vec![
                crate::LonLatPoint {
                    lon: 104.0,
                    lat: 16.0,
                },
                crate::LonLatPoint {
                    lon: 120.0,
                    lat: 16.0,
                },
                crate::LonLatPoint {
                    lon: 120.0,
                    lat: 32.0,
                },
                crate::LonLatPoint {
                    lon: 104.0,
                    lat: 32.0,
                },
            ],
        };
        let global = demand_cells(None, 120);
        let regional = demand_cells(Some(&watershed), 120);
        assert!(
            regional * 100 < global,
            "a 16 by 16 degree domain must not cost the globe: {regional} vs {global}"
        );
    }

    #[test]
    fn a_union_domain_is_judged_over_every_member() {
        let west = GridRegion::Bbox {
            west: 100.0,
            east: 110.0,
            south: 10.0,
            north: 20.0,
        };
        let east = GridRegion::Bbox {
            west: 130.0,
            east: 140.0,
            south: 30.0,
            north: 40.0,
        };
        let union = GridRegion::Any(vec![west, east]);
        let (w, e, s, n) = region_lonlat_bounds(&union).expect("union bounds");
        assert_eq!((w, e, s, n), (100.0, 140.0, 10.0, 40.0));
    }
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
