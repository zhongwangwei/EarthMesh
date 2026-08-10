use crate::final_quality_non_negative_usize;
use crate::gridfile_mesh_from_one_based_state;
use crate::harp_dv_options::{read_harp_dv_options, HarpDvRunOptions};
use crate::method_c_algorithm::{
    read_method_c_algorithm_options, MethodCAlgorithm, MethodCAlgorithmOptions,
};
use crate::method_c_delaunay_mesh_from_unstructured_gridfile;
use crate::method_c_refinement_region_level;
use crate::mkgrd_run_types::{LeppAdaptiveHybridRunRecord, LeppPostQualityRunRecord};
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
use crate::refinement_spring_iterations;
use crate::run_mkgrd_gridinit_global_namelist;
use crate::validate_native_spawn_mdomain;
use crate::GridRegion;
use crate::MethodCGridfileMetadataSlices;
use crate::RefinePipelineRunReport;
use earthmesh_refine_method_c::{
    improve_lepp_post_quality, refine_adaptive_hybrid, refine_adaptive_hybrid_constrained,
    AdaptiveHybridConfig, AdaptiveHybridDemand, AdaptiveHybridUnresolvedDemand,
    AdaptiveHybridUnresolvedReason, LeppInsertionGates, LeppPostQualityConfig,
    LeppPostQualityReport, LeppSearchConfig, MethodCMesh,
};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, QualityNamelist, RefineConfig};
use earthmesh_mesh::{
    grid_cartesian_xy_to_lonlat_placeholders_one_based_state, grid_xyz2lonlat_one_based_state,
    pcvt_adjust_voronoi_grid_state, voronoi_grid_from_triangular_mesh,
    voronoi_grid_from_triangular_mesh_cartesian, MeshState, TriangularMesh,
};
use rayon::prelude::*;

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
    let quality = QualityNamelist::from_quality_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let method_c_algorithm = read_method_c_algorithm_options(&contents)?;
    let harp_dv_options = read_harp_dv_options(&contents)?;
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
    if method_c_algorithm.algorithm == MethodCAlgorithm::LeppDelaunay
        && (native_cartesian_xy || native_surface_global_expansion)
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "LEPP AdaptiveHybrid requires the spherical Method-C base mesh; Cartesian-XY and native surface expansion are unsupported",
        ));
    }
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
    // Two demand sources with no defined composition. Method-C's branch chain
    // takes `&adaptive` first, so `&hfield` would be skipped -- except that
    // configuring it also changes how regions are gathered, so the pair
    // produces a third mesh that is neither. Measured at NXP 21: adaptive alone
    // 7023 cells, h-field alone 9510, both 4875 -- less refinement than either,
    // in silence.
    if adaptive_options.is_some() && hfield_options.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "&adaptive and &hfield are both enabled and nothing composes them: the h-field is a \
             target-level field and &adaptive produces circles, and Method-C serves whichever \
             branch it reaches first. Enable one",
        ));
    }

    // Same shape again, one branch earlier. The native `&ngrids`/`&nsfcgrids`
    // spawn sits at the head of Method-C's chain and never consults either
    // route, so a namelist carrying both gets the native mesh and no word about
    // the other. Measured at NXP 6: `&nsfcgrids` alone and `&nsfcgrids` with
    // `&adaptive` produced bit-identical 435-cell meshes, exit 0, and not one
    // line of adaptive output.
    //
    // The condition is the branch's own, character for character, because
    // "native regions are configured" is not the same thing. With `refine_spc`
    // on, the native spawn stands down and the h-field branch runs -- which is
    // how Cartesian-XY serves `&ngrids` *and* an h-field together, a
    // combination this guard refused outright on its first attempt and 64 tests
    // said so.
    let native_spawn_takes_precedence = !is_atmosmesh
        && (native_only_spawn || native_surface_global_expansion)
        && !refine.refine_spc
        && !refine.refine_cal;
    if native_spawn_takes_precedence && (adaptive_options.is_some() || hfield_options.is_some()) {
        let other = if adaptive_options.is_some() {
            "&adaptive"
        } else {
            "&hfield"
        };
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "native Method-C grids (NL%ngrids/NL%nsfcgrids) and {other} are both configured \
                 and nothing composes them: the native spawn refines the grids it was given and \
                 never reads the other route. Enable one"
            ),
        ));
    }

    // Named before anything dispatches on it, because the dispatch used to end
    // in a `_ =>` arm that ran Method-C. Measured: `harpdv`, `harp-dv`,
    // `redgreen`, `method-c` and `HARP_DV` all produced a Method-C mesh and
    // said nothing -- a user asking for one backend and silently getting
    // another, which is the failure class guide 11.1 records.
    let backend = refine_backend_name(&config.refine_backend)?;
    if quality.lepp_post_quality && backend != RefineBackend::MethodC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%lepp_post_quality requires NL%refine_backend='method_c'",
        ));
    }
    if method_c_algorithm.algorithm == MethodCAlgorithm::LeppDelaunay
        && backend != RefineBackend::MethodC
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "&method_c algorithm='lepp_delaunay' requires NL%refine_backend='method_c'",
        ));
    }
    if method_c_algorithm.algorithm == MethodCAlgorithm::LeppDelaunay && quality.lepp_post_quality {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "LEPP AdaptiveHybrid and LEPP post-quality cannot both own the same Method-C run",
        ));
    }
    if method_c_algorithm.algorithm == MethodCAlgorithm::LeppDelaunay && hfield_options.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "LEPP AdaptiveHybrid does not consume &hfield; use &adaptive or named regions",
        ));
    }

    // named region by another name, but a criterion with neither a file nor
    // `&adaptive` behind it has nowhere to go.
    //
    // Said now rather than at the backend branch, because the reader below runs
    // first -- and on the unconfigured prefix it fails with a message about
    // Method-C and a `/tmp` path nobody typed.
    if backend == RefineBackend::RedGreen
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
        refinement_spring_iterations(&refine, is_atmosmesh)?
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
        harp_dv_run,
        lepp_hard_regions,
        lepp_adaptive_hybrid,
        lepp_post_quality,
    } = match backend {
        RefineBackend::RedGreen => {
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
                        inputs: adaptive_demand_inputs(
                            domain_region.as_ref(),
                            &config,
                            adaptive_landtype_file(&config),
                            mesh_type,
                            adaptive.coastline,
                        )?,
                        base_cell_meters: adaptive.base_m.unwrap_or_else(|| {
                            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS
                                / (5.0 * method_c_nxp as f64)
                        }),
                        coastline: adaptive.coastline,
                    })
                })
                .transpose()?;
            refine_with_redgreen(
                &mesh,
                &regions,
                &refine,
                max_level,
                adaptive,
                config.mode_grid.trim() == "tri",
                spring_nest_iterations,
            )?
        }
        RefineBackend::HarpDv => {
            // The same list red-green refuses, and for the same reason: each of
            // these would otherwise be dropped and the run would still write a
            // valid mesh that is not the mesh that was asked for. Measured
            // before this guard: `harp_dv` with `&hfield` configured produced
            // 6450 cells and never read the field.
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
                        "NL%refine_backend = harp_dv does not serve {unsupported}; it re-reads a \
                         target scale against the cells that exist and serves circular regions. \
                         Use method_c for this run"
                    ),
                ));
            }
            refine_with_harp_dv(
                &mesh,
                &regions,
                adaptive_options.as_ref(),
                &config,
                domain_region.as_ref(),
                mesh_type,
                method_c_nxp,
                max_level,
                spring_nest_iterations,
                &refine,
                harp_dv_options,
            )?
        }
        RefineBackend::MethodC => {
            if method_c_algorithm.algorithm == MethodCAlgorithm::LeppDelaunay {
                refine_with_method_c_lepp(
                    mesh,
                    &regions,
                    adaptive_options.as_ref(),
                    &refine,
                    &config,
                    domain_region.as_ref(),
                    mesh_type,
                    method_c_nxp,
                    max_level,
                    method_c_algorithm,
                    spring_nest_iterations,
                )?
            } else {
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
                let lepp_post_quality = if quality.lepp_post_quality {
                    if native_cartesian_xy || domain_region.is_some() {
                        return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "NL%lepp_post_quality currently requires a global spherical closed mesh",
                    ));
                    }
                    let mut post_quality_mesh = MeshState::from_triangular_mesh(mesh.mesh())?;
                    let post_quality_config = LeppPostQualityConfig {
                        maximum_edge_length: (quality.lepp_post_quality_max_edge_km > 0.0)
                            .then_some(quality.lepp_post_quality_max_edge_km * 1000.0),
                        minimum_spherical_triangle_angle_degrees: Some(quality.min_angle_warn_deg),
                        maximum_insertions: usize::try_from(
                            quality.lepp_post_quality_max_insertions,
                        )
                        .map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "NL%lepp_post_quality_max_insertions must fit usize",
                            )
                        })?,
                        gates: LeppInsertionGates::for_method_c(mesh.impent),
                        ..LeppPostQualityConfig::default()
                    };
                    let report =
                        improve_lepp_post_quality(&mut post_quality_mesh, &post_quality_config)
                            .map_err(|error| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!("LEPP post-quality failed: {error}"),
                                )
                            })?;
                    let optimized = post_quality_mesh.to_triangular_mesh(mesh.impent, None)?;
                    let optimized_state = spherical_voronoi_state(&optimized)?;
                    Some(LeppPostQualityGrid {
                        output_mesh: gridfile_mesh_from_one_based_state(
                            &optimized_state.grid,
                            &optimized_state.tabs,
                        )?,
                        report,
                    })
                } else {
                    None
                };
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
                    harp_dv_run: None,
                    lepp_hard_regions: Vec::new(),
                    lepp_adaptive_hybrid: None,
                    lepp_post_quality,
                }
            }
        }
    };

    // Measured from backend output, not from the request: Method-C records face
    // generations; criteria-driven Red-Green records the deepest pass it
    // actually completed. A backend with neither still reports zero.
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
        .or_else(|| {
            adaptive_run
                .as_ref()
                .map(|(report, _, _, _)| report.deepest_level)
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
        // Two corrections the per-region metric already had and this did not.
        //
        // `n_w_to_m` gives how many of a row's seven slots are corners; the
        // rest are placeholder id 1, which resolves to a real but unrelated
        // point. Reading the whole row builds a polygon out of a cell plus a
        // stranger, which is the defect guide 11.x records for the per-region
        // count -- fixed there, missed here, in the same file.
        //
        // And the area is signed: about half the cells wind the other way, so
        // discarding `steradians <= 0.0` threw away half the mesh and reported
        // the extremes of what was left.
        for (row_index, corners) in output_mesh.w_to_m.iter().enumerate() {
            let valid = output_mesh
                .n_w_to_m
                .get(row_index)
                .and_then(|&n| usize::try_from(n).ok())
                .unwrap_or(corners.len())
                .min(corners.len());
            let polygon: Vec<earthmesh_mesh::LonLatDegrees> = corners
                .iter()
                .take(valid)
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
            // A cell is a small patch, so its area is the *minor* one. A
            // polygon containing a pole comes back as the complement instead:
            // measured, a triangle whose three corners sit at 89 north returns
            // 12.5654 sr against a true 0.00096 -- four pi minus almost
            // nothing, and 13000 times too big. Taking `abs()` does not help;
            // the sign only says which way the ring was walked.
            //
            // Every cell here is far smaller than a hemisphere, so the minor
            // area is the one below 2*pi and the complement is the one above.
            let Some(steradians) = minor_cell_steradians(steradians) else {
                continue;
            };
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

    // What a refinement level actually delivered: the median cell width inside
    // the regions that asked, against the median outside them.
    //
    // The global percentiles above are not a level -- they carry the
    // icosahedron's own variation and the coastline carve, and both backends
    // read near four halvings there whatever was requested. A level is a claim
    // about the refined region relative to the rest.
    let realized_region_halvings = {
        let adaptive_regions = adaptive_run
            .as_ref()
            .map(|(report, _, _, _)| {
                report
                    .passes
                    .iter()
                    .flat_map(|pass| pass.regions.iter().cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let measure_regions = if !lepp_hard_regions.is_empty() {
            lepp_hard_regions.as_slice()
        } else if !adaptive_regions.is_empty() {
            adaptive_regions.as_slice()
        } else {
            regions.as_slice()
        };
        let radius_km = earthmesh_core::EARTH_RADIUS_METERS / 1000.0;
        let measure_index = earthmesh_mesh::RefinementRegionIndex::new(measure_regions);
        let mut inside: Vec<f64> = Vec::new();
        let mut outside: Vec<f64> = Vec::new();
        for (row, corners) in output_mesh.w_to_m.iter().enumerate() {
            // `w_to_m` rows are the full seven-wide `itab_w.im`, and only the
            // first `n_w_to_m` entries are corners of this cell. The rest are
            // placeholders, and placeholder id 1 resolves to a real point
            // somewhere else entirely -- which is what made four earlier
            // attempts at this measure compute cell areas twenty-four times
            // too large (guide 11.35).
            let valid = output_mesh
                .n_w_to_m
                .get(row)
                .and_then(|count| usize::try_from(*count).ok())
                .unwrap_or(0)
                .min(corners.len());
            if valid < 3 {
                continue;
            }
            let polygon: Vec<earthmesh_mesh::LonLatDegrees> = corners[..valid]
                .iter()
                .filter_map(|&im| {
                    let index = usize::try_from(im).ok()?.checked_sub(1)?;
                    let point = output_mesh.m_points.get(index)?;
                    Some(earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat))
                })
                .collect();
            if polygon.len() < 3 {
                continue;
            }
            let Some(steradians) = earthmesh_mesh::robust_spherical_area_unit(&polygon) else {
                continue;
            };
            let Some(steradians) = minor_cell_steradians(steradians) else {
                continue;
            };
            let Some(centre) = output_mesh.w_points.get(row) else {
                continue;
            };
            let across_km = (steradians / std::f64::consts::PI).sqrt() * radius_km;
            let centre = earthmesh_mesh::LonLatDegrees::new(centre.lon, centre.lat);
            let in_region = measure_index.contains_lonlat_canonical(centre, 0);
            if in_region {
                inside.push(across_km);
            } else {
                outside.push(across_km);
            }
        }
        let median = |values: &mut Vec<f64>| -> Option<f64> {
            if values.is_empty() {
                return None;
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            Some(values[values.len() / 2])
        };
        let (fine, coarse) = (median(&mut inside), median(&mut outside));
        if std::env::var("EM_DEBUG_LEVEL").is_ok() {
            let total: f64 = inside
                .iter()
                .chain(outside.iter())
                .map(|across| across * across * std::f64::consts::PI / (radius_km * radius_km))
                .sum();
            eprintln!(
                "level-debug: cells={} in={} out={} in-median={fine:?} out-median={coarse:?} \
                 area-sum={total:.4} (4pi={:.4})",
                output_mesh.w_to_m.len(),
                inside.len(),
                outside.len(),
                4.0 * std::f64::consts::PI
            );
        }
        match (fine, coarse) {
            (Some(fine), Some(coarse)) if fine > 0.0 && coarse > 0.0 => (coarse / fine).log2(),
            _ => 0.0,
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
    let hard_center_demand = if lepp_hard_regions.is_empty() {
        adaptive_hard_center_demand(adaptive_run.as_ref(), config.mode_grid.trim(), &output_mesh)
    } else {
        Some(region_center_demand(
            &lepp_hard_regions,
            config.mode_grid.trim(),
            &output_mesh,
        ))
    };
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
        "",
    )?;

    let lepp_adaptive_hybrid = if let Some(report) = lepp_adaptive_hybrid {
        let result_dir = file_dir.join("result");
        let report_path = result_dir.join("method_c_lepp_report.json");
        let unresolved_path = result_dir.join("unresolved_demand.json");
        let unresolved = report
            .unresolved_demands
            .iter()
            .map(|demand| {
                serde_json::json!({
                    "criterion_id": demand.criterion_id,
                    "face": demand.face.map(|face| serde_json::json!({
                        "slot": face.slot,
                        "generation": face.generation,
                    })),
                    "hard": demand.hard,
                    "reason": format!("{:?}", demand.reason),
                    "message": demand.message,
                })
            })
            .collect::<Vec<_>>();
        fs::write(
            &unresolved_path,
            serde_json::to_vec_pretty(&unresolved).map_err(io::Error::other)?,
        )?;
        let report_json = serde_json::json!({
            "algorithm": "lepp_delaunay",
            "mode": "adaptive_hybrid",
            "canonical_method_c_compatible": false,
            "transition_model": "lepp_natural_gradation",
            "stop_reason": format!("{:?}", report.stop_reason),
            "cycles": report.cycles,
            "counts": {
                "initial_vertices": report.initial_vertices,
                "final_vertices": report.final_vertices,
                "initial_faces": report.initial_faces,
                "final_faces": report.final_faces,
            },
            "insertions": {
                "physical": report.insertion_counts.physical,
                "balance": report.insertion_counts.balance,
                "quality": report.insertion_counts.quality,
                "boundary": report.insertion_counts.boundary,
            },
            "lepp_paths": {
                "attempted": report.path_stats.attempted,
                "committed": report.path_stats.committed,
                "rejected": report.path_stats.rejected,
                "total_faces": report.path_stats.total_path_faces,
                "maximum": report.path_stats.max_path_faces,
                "mean": report.path_stats.mean_path_faces,
                "p95": report.path_stats.p95_path_faces,
            },
            "target_satisfaction": {
                "target_faces": report.target_satisfaction.target_faces,
                "satisfied_faces": report.target_satisfaction.satisfied_faces,
                "unsatisfied_faces": report.target_satisfaction.unsatisfied_faces,
            },
            "unresolved_demands": report.unresolved_demand_count,
            "sampled_unresolved_demand_details": unresolved.len(),
            "unresolved_demand_file": unresolved_path.display().to_string(),
            "rejections": report.rejections.iter().map(|rejection| serde_json::json!({
                "criterion_id": rejection.criterion_id,
                "face": {
                    "slot": rejection.face.slot,
                    "generation": rejection.face.generation,
                },
                "hard": rejection.hard,
                "error": rejection.error.to_string(),
            })).collect::<Vec<_>>(),
            "sampled_rejection_details": report.rejections.len(),
            "output": outputs.output.output.display().to_string(),
            "config": {
                "max_cycles": method_c_algorithm.max_cycles,
                "target_size_tolerance": method_c_algorithm.target_size_tolerance,
                "maximum_neighbor_size_ratio": method_c_algorithm.maximum_neighbor_size_ratio,
                "maximum_vertices": method_c_algorithm.maximum_vertices,
                "maximum_insertions_per_cycle": method_c_algorithm.maximum_insertions_per_cycle,
                "maximum_path_length": method_c_algorithm.maximum_path_length,
                "stop_at_source_resolution": method_c_algorithm.stop_at_source_resolution,
                "minimum_triangle_angle_deg": method_c_algorithm.minimum_triangle_angle_deg,
            },
        });
        fs::write(
            &report_path,
            serde_json::to_vec_pretty(&report_json).map_err(io::Error::other)?,
        )?;
        Some(LeppAdaptiveHybridRunRecord {
            stop_reason: format!("{:?}", report.stop_reason),
            cycles: report.cycles,
            physical_insertions: report.insertion_counts.physical,
            balance_insertions: report.insertion_counts.balance,
            quality_insertions: report.insertion_counts.quality,
            boundary_insertions: report.insertion_counts.boundary,
            unresolved_demands: report.unresolved_demand_count,
            report: report_path,
            unresolved_report: unresolved_path,
        })
    } else {
        None
    };

    let lepp_post_quality = if let Some(lepp) = lepp_post_quality {
        let hard_center_demand = adaptive_hard_center_demand(
            adaptive_run.as_ref(),
            config.mode_grid.trim(),
            &lepp.output_mesh,
        );
        let lepp_outputs = write_refined_outputs(
            &contents,
            &config,
            source_gridnum_perdegree,
            &file_dir,
            nxp,
            max_level,
            &lepp.output_mesh,
            None,
            None,
            hard_center_demand.as_deref(),
            "_lepp",
        )?;
        let report_path = file_dir
            .join("result")
            .join("method_c_lepp_post_quality.json");
        let report_json = serde_json::json!({
            "algorithm": "lepp_delaunay_post_quality",
            "canonical_output": outputs.output.output.display().to_string(),
            "optimized_output": lepp_outputs.output.output.display().to_string(),
            "config": {
                "maximum_insertions": quality.lepp_post_quality_max_insertions,
                "maximum_edge_km": quality.lepp_post_quality_max_edge_km,
                "minimum_spherical_triangle_angle_degrees": quality.min_angle_warn_deg,
            },
            "before": {
                "violating_faces": lepp.report.before.violating_faces,
                "worst_violation": lepp.report.before.worst_violation,
                "total_violation": lepp.report.before.total_violation,
            },
            "after": {
                "violating_faces": lepp.report.after.violating_faces,
                "worst_violation": lepp.report.after.worst_violation,
                "total_violation": lepp.report.after.total_violation,
            },
            "attempted": lepp.report.attempted,
            "committed": lepp.report.committed,
            "rejected": lepp.report.rejected,
            "sampled_insertion_details": lepp.report.insertions.len(),
            "sampled_rejection_details": lepp.report.rejections.len(),
            "stop_reason": format!("{:?}", lepp.report.stop_reason),
            "insertions": lepp.report.insertions.iter().map(|insertion| serde_json::json!({
                "start_face": insertion.path.faces.first(),
                "terminal": format!("{:?}", insertion.path.terminal),
                "path_faces": insertion.path.faces,
                "site": {
                    "slot": insertion.insertion.site_id.slot,
                    "generation": insertion.insertion.site_id.generation,
                },
                "created_faces": insertion.created_faces.iter().map(|face| serde_json::json!({
                    "slot": face.slot,
                    "generation": face.generation,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "rejections": lepp.report.rejections.iter().map(|rejection| serde_json::json!({
                "face": {
                    "slot": rejection.face.slot,
                    "generation": rejection.face.generation,
                },
                "error": rejection.error.to_string(),
            })).collect::<Vec<_>>(),
        });
        let encoded = serde_json::to_vec_pretty(&report_json).map_err(io::Error::other)?;
        fs::write(&report_path, encoded).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("write {}: {error}", report_path.display()),
            )
        })?;
        Some(LeppPostQualityRunRecord {
            stop_reason: format!("{:?}", lepp.report.stop_reason),
            attempted: lepp.report.attempted,
            committed: lepp.report.committed,
            rejected: lepp.report.rejected,
            violations_before: lepp.report.before.violating_faces,
            violations_after: lepp.report.after.violating_faces,
            worst_violation_before: lepp.report.before.worst_violation,
            worst_violation_after: lepp.report.after.worst_violation,
            report: report_path,
            raw_output: lepp_outputs.raw_output,
            landtype_masked_cells: lepp_outputs.landtype_masked_cells,
            coupled_outputs: lepp_outputs.coupled_outputs,
            output: lepp_outputs.output,
        })
    } else {
        None
    };

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
        realized_region_halvings,
        coarsest_cell_km,
        hfield_diagnostics,
        transition_faces,
        spring_nest_passes,
        harp_dv_run,
        lepp_adaptive_hybrid,
        lepp_post_quality,
        spring_nest_iterations,
        raw_output: outputs.raw_output,
        landtype_masked_cells: outputs.landtype_masked_cells,
        coupled_outputs: outputs.coupled_outputs,
        output: outputs.output,
        runtime_state,
    })
}

fn minor_cell_steradians(area: f64) -> Option<f64> {
    let area = area.abs();
    let area = if area > 2.0 * std::f64::consts::PI {
        4.0 * std::f64::consts::PI - area
    } else {
        area
    };
    (area.is_finite() && area > 0.0).then_some(area)
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
    /// What HARP-DV's own run reported, or `None` from the other two backends.
    ///
    /// It used to reach only stderr, so a caller reading the run record could
    /// not tell a mesh that met its demands from one that stopped at a budget
    /// or a scale floor -- both exited zero with a mesh written. `adaptive_run`
    /// could not carry it: that is Method-C's per-level circle record and says
    /// nothing about cycles, refusals or a stop reason.
    harp_dv_run: Option<HarpDvRunRecord>,
    /// Hard regions the LEPP driver consumed, used by output carving and
    /// backend-neutral achieved-resolution measurements.
    lepp_hard_regions: Vec<earthmesh_mesh::RefinementRegion>,
    /// LEPP-Delaunay AdaptiveHybrid's own run report.
    lepp_adaptive_hybrid: Option<earthmesh_refine_method_c::AdaptiveHybridReport>,
    /// Optional repair derived from, but never replacing, the canonical mesh.
    lepp_post_quality: Option<LeppPostQualityGrid>,
}

struct LeppPostQualityGrid {
    output_mesh: crate::UnstructuredMesh,
    report: LeppPostQualityReport,
}

fn adaptive_hard_center_demand(
    adaptive_run: Option<&AdaptiveRunRecord>,
    mode_grid: &str,
    output_mesh: &crate::UnstructuredMesh,
) -> Option<Vec<bool>> {
    adaptive_run.map(|(report, _, _, _)| {
        let regions = report
            .passes
            .iter()
            .flat_map(|pass| pass.regions.iter().cloned())
            .collect::<Vec<_>>();
        let index = earthmesh_mesh::RefinementRegionIndex::new(&regions);
        let centers = if mode_grid == "tri" {
            &output_mesh.m_points
        } else {
            &output_mesh.w_points
        };
        centers
            .par_iter()
            .map(|point| {
                index.contains_lonlat_great_circle(
                    earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat),
                    1,
                )
            })
            .collect()
    })
}

fn region_center_demand(
    regions: &[earthmesh_mesh::RefinementRegion],
    mode_grid: &str,
    output_mesh: &crate::UnstructuredMesh,
) -> Vec<bool> {
    let index = earthmesh_mesh::RefinementRegionIndex::new(regions);
    let centers = if mode_grid == "tri" {
        &output_mesh.m_points
    } else {
        &output_mesh.w_points
    };
    centers
        .par_iter()
        .map(|point| {
            index.contains_lonlat_canonical(
                earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat),
                0,
            )
        })
        .collect()
}

/// The part of HARP-DV's report a run record can carry.
#[derive(Clone, Debug, PartialEq)]
pub struct HarpDvRunRecord {
    pub stop_reason: String,
    pub cycles_completed: u32,
    pub transactions_committed: usize,
    pub unresolved_cells: usize,
    pub unbalanced_pairs_remaining: usize,
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

/// Move only cells safely inside a requested refinement region.
///
/// A cell touching the refined/coarse interface is pinned automatically: one
/// outside corner on any incident triangle clears the move bit for all three.
/// That keeps a backend-neutral spring from drifting the transition boundary.
fn spring_region_interior_mask(
    mesh: &crate::UnstructuredMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
) -> io::Result<Vec<bool>> {
    let cells_on_triangle = crate::cells_on_triangle_one_based_from_mesh(mesh)?;
    let index = earthmesh_mesh::RefinementRegionIndex::new(regions);
    let inside = mesh
        .w_points
        .iter()
        .map(|point| {
            index.contains_lonlat_canonical(
                earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat),
                1,
            )
        })
        .collect::<Vec<_>>();
    let mut movable = inside.clone();
    movable.iter_mut().take(2).for_each(|value| *value = false);
    for corners in cells_on_triangle.iter().skip(2) {
        if corners.iter().any(|&cell| !inside[cell]) {
            for &cell in corners {
                movable[cell] = false;
            }
        }
    }
    Ok(movable)
}

/// Apply the existing spherical regional spring without requiring Method-C
/// boundary-row metadata. Red-Green and LEPP both preserve connectivity here;
/// only cell coordinates and their derived triangle centres are replaced.
fn spring_unstructured_region_interiors(
    mesh: &crate::UnstructuredMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    iterations: usize,
) -> io::Result<(crate::UnstructuredMesh, usize)> {
    if iterations == 0 || regions.is_empty() {
        return Ok((mesh.clone(), 0));
    }
    let spring_mesh = unstructured_mesh_with_one_based_rows(mesh);
    let move_mask = spring_region_interior_mask(&spring_mesh, regions)?;
    if move_mask.iter().skip(2).all(|&movable| !movable) {
        eprintln!(
            "earthmesh_cli: refinement spring skipped: no cell lies safely inside the requested regions"
        );
        return Ok((mesh.clone(), 0));
    }
    let movable_cells = move_mask.iter().skip(2).filter(|&&movable| movable).count();
    let baseline_angles = match unstructured_triangle_angle_range(&spring_mesh) {
        Ok(angles) => angles,
        Err(error) => {
            eprintln!(
                "earthmesh_cli: warning: refinement spring skipped because the input mesh cannot be quality-checked ({error})"
            );
            return Ok((mesh.clone(), 0));
        }
    };
    let started = std::time::Instant::now();
    eprintln!(
        "earthmesh_cli: refinement spring started: {movable_cells} movable cells, {iterations} iterations"
    );
    let report = match crate::springjustment_gridfile_adapters::run_springjustment_regional_from_unstructured_mesh(
        &spring_mesh,
        crate::SpringjustmentRegionalRunOptions {
            move_mask: &move_mask,
            niter_refine: iterations,
            radius: earthmesh_core::EARTH_RADIUS_METERS,
            diagnostic_every: 100,
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!(
                "earthmesh_cli: warning: refinement spring declined ({error}); keeping the unsmoothed mesh"
            );
            return Ok((mesh.clone(), 0));
        }
    };
    let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&report.mesh);
    if !topology.is_consistent() {
        eprintln!(
            "earthmesh_cli: warning: refinement spring produced inconsistent connectivity ({}); keeping the unsmoothed mesh",
            topology
                .violations
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        );
        return Ok((mesh.clone(), 0));
    }
    let candidate_angles = match unstructured_triangle_angle_range(&report.mesh) {
        Ok(angles) => angles,
        Err(error) => {
            eprintln!(
                "earthmesh_cli: warning: refinement spring produced invalid triangle geometry ({error}); keeping the unsmoothed mesh"
            );
            return Ok((mesh.clone(), 0));
        }
    };
    let tolerance = 1.0e-4;
    if candidate_angles.0 < baseline_angles.0 - tolerance
        || candidate_angles.1 > baseline_angles.1 + tolerance
    {
        eprintln!(
            "earthmesh_cli: warning: refinement spring worsened triangle angles ({:.3}..{:.3} -> {:.3}..{:.3} degrees); keeping the unsmoothed mesh",
            baseline_angles.0, baseline_angles.1, candidate_angles.0, candidate_angles.1
        );
        return Ok((mesh.clone(), 0));
    }
    eprintln!(
        "earthmesh_cli: refinement spring complete in {:.1}s",
        started.elapsed().as_secs_f64()
    );
    Ok((report.mesh, 1))
}

fn unstructured_mesh_with_one_based_rows(
    mesh: &crate::UnstructuredMesh,
) -> crate::UnstructuredMesh {
    let mut normalized = mesh.clone();
    if !crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows(
        &normalized.m_points,
    ) {
        normalized
            .m_points
            .insert(0, crate::LonLatPoint { lon: 0.0, lat: 0.0 });
        normalized.m_to_w.insert(0, [0; 3]);
    }
    if !crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows(
        &normalized.w_points,
    ) {
        normalized
            .w_points
            .insert(0, crate::LonLatPoint { lon: 0.0, lat: 0.0 });
        normalized.w_to_m.insert(0, Vec::new());
        normalized.n_w_to_m.insert(0, 0);
    }
    normalized
}

fn unstructured_triangle_angle_range(mesh: &crate::UnstructuredMesh) -> io::Result<(f64, f64)> {
    let triangles = crate::cells_on_triangle_one_based_from_mesh(mesh)?;
    let points = mesh
        .w_points
        .iter()
        .map(|point| earthmesh_mesh::LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for corners in triangles.iter().skip(2) {
        let triangle = [points[corners[0]], points[corners[1]], points[corners[2]]];
        let metrics = earthmesh_mesh::polygon_length_angle_metrics(&triangle).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "refinement spring encountered a degenerate triangle",
            )
        })?;
        for angle in metrics.angles_degrees {
            if !angle.is_finite() || angle <= 0.0 || angle >= 180.0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "refinement spring encountered a non-finite or degenerate triangle angle",
                ));
            }
            minimum = minimum.min(angle);
            maximum = maximum.max(angle);
        }
    }
    if !minimum.is_finite() || !maximum.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "refinement spring mesh contains no physical triangles",
        ));
    }
    Ok((minimum, maximum))
}

fn triangular_mesh_from_unstructured(
    mesh: &crate::UnstructuredMesh,
    pentagons: [usize; 12],
) -> io::Result<TriangularMesh> {
    let vertices = mesh
        .w_points
        .iter()
        .map(|point| {
            earthmesh_mesh::lonlat_degrees_to_unit_xyz(earthmesh_mesh::LonLatDegrees::new(
                point.lon, point.lat,
            ))
        })
        .collect();
    let triangles = crate::cells_on_triangle_one_based_from_mesh(mesh)?;
    let state = MeshState::from_parts(vertices, triangles).map_err(|errors| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "smoothed mesh does not convert back to a triangulation: {}",
                errors
                    .iter()
                    .take(4)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )
    })?;
    state.to_triangular_mesh(pentagons, None)
}

/// The criteria half of the point+radius route, as red-green consumes it.
struct RedGreenAdaptive<'a> {
    inputs: Vec<crate::refinement_demand::plan::DemandPlanInputs<'a>>,
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
    preserve_locality: bool,
    spring_iterations: usize,
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
    let mut spring_regions = named_regions.to_vec();
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
            let cell_meters = adaptive.base_cell_meters / 2f64.powi((level - 1) as i32);
            let demand = crate::refinement_demand::nest::adaptive_demand_circles_for_level_windows_at_radius(
                refine,
                &adaptive.inputs,
                level,
                cell_meters,
                cell_meters,
                cell_meters,
            )?;
            demanded_cells = demand.demanded_cells;
            eprintln!(
                "red-green refine level {level} judging {:.0} m cells: {} circles over {} \
                 demanded source cells",
                adaptive.base_cell_meters / 2f64.powi((level - 1) as i32),
                demand.circles.len(),
                demand.demanded_cells,
            );
            spring_regions.extend(demand.circles.iter().cloned());
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
            preserve_locality,
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
        if !preserve_locality && widest_cell > REDGREEN_MAX_CELL_DEGREE {
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
    let (output_mesh, spring_nest_passes) =
        spring_unstructured_region_interiors(&output_mesh, &spring_regions, spring_iterations)?;
    Ok(RefinedGrid {
        state: None,
        output_mesh,
        method_c_metadata: None,
        // Red-green renumbers each round, but `vertex_mapping` is the identity
        // over the cells that went in, so a base-mesh cell keeps its id through
        // every level. The pentagons are base-mesh cells.
        pentagon_indices: mesh.impent,
        transition_faces: 0,
        spring_nest_passes,
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
                    spring_passes: spring_nest_passes,
                },
                max_level,
                adaptive.base_cell_meters,
                adaptive.coastline,
            )
        }),
        lepp_hard_regions: Vec::new(),
        lepp_adaptive_hybrid: None,
        lepp_post_quality: None,
        harp_dv_run: None,
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

fn refine_with_method_c_lepp(
    mesh: TriangularMesh,
    named_regions: &[earthmesh_mesh::RefinementRegion],
    adaptive: Option<&crate::adaptive_refine::AdaptiveRefineOptions>,
    refine: &RefineConfig,
    config: &EarthmeshConfig,
    domain_region: Option<&GridRegion>,
    mesh_type: &str,
    method_c_nxp: usize,
    max_level: usize,
    options: MethodCAlgorithmOptions,
    spring_iterations: usize,
) -> io::Result<RefinedGrid> {
    let pentagons = mesh.impent;
    let mut state = MeshState::from_triangular_mesh(&mesh)?;
    for (index, region) in named_regions.iter().enumerate() {
        region.validate().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LEPP refinement region {index} is invalid: {error}"),
            )
        })?;
    }
    let mut demands = named_regions
        .iter()
        .enumerate()
        .map(|(index, region)| {
            AdaptiveHybridDemand::user_region(format!("named-region-{index}"), region.clone())
        })
        .collect::<Vec<_>>();
    let mut hard_regions = named_regions.to_vec();
    let mut pre_unresolved = Vec::new();

    if let Some(adaptive) = adaptive {
        let base_m = adaptive.base_m.unwrap_or_else(|| {
            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS
                / (5.0 * method_c_nxp as f64)
        });
        let depth = adaptive.max_level.unwrap_or(max_level).clamp(1, 5);
        let inputs = adaptive_demand_inputs(
            domain_region,
            config,
            adaptive_landtype_file(config),
            mesh_type,
            adaptive.coastline,
        )?;
        let gridnum_perdegree = usize::try_from(config.gridnum_perdegree).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "NL%gridnum_perdegree must fit usize",
            )
        })?;
        if gridnum_perdegree == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NL%gridnum_perdegree must be positive for LEPP source-resolution stopping",
            ));
        }
        // `gridnum_perdegree` applies to both source axes. The meridional cell
        // dimension is latitude-independent and is the conservative resolvable
        // scale; shrinking only the zonal dimension by cos(latitude) would
        // claim resolution the north-south sampling does not have.
        let source_resolution_m =
            earthmesh_core::KM_PER_DEGREE_EQUATOR * 1000.0 / gridnum_perdegree as f64;
        for level in 1..=depth {
            let target_edge_m = base_m / 2f64.powi(level as i32);
            let level_started = std::time::Instant::now();
            eprintln!(
                "earthmesh_cli: LEPP AdaptiveHybrid planning demand level {level}/{depth} (target edge {:.1} km)",
                target_edge_m / 1000.0
            );
            let level_demand =
                crate::refinement_demand::nest::adaptive_demand_circles_for_level_windows(
                    refine, &inputs, level, base_m, depth,
                )?;
            eprintln!(
                "earthmesh_cli: LEPP AdaptiveHybrid demand level {level}/{depth} complete: {} source cells -> {} circles in {:.1}s",
                level_demand.demanded_cells,
                level_demand.circles.len(),
                level_started.elapsed().as_secs_f64()
            );
            let criterion_id = if level_demand.criterion_ids.is_empty() {
                format!("adaptive-level-{level}")
            } else {
                format!("{}-level-{level}", level_demand.criterion_ids.join("+"))
            };
            if level_demand.demanded && level_demand.circles.is_empty() {
                pre_unresolved.push(AdaptiveHybridUnresolvedDemand {
                    criterion_id,
                    face: None,
                    hard: true,
                    reason: AdaptiveHybridUnresolvedReason::Rejection,
                    message: format!(
                        "{} demanded source cells at level {level}, but circle reduction produced no region",
                        level_demand.demanded_cells
                    ),
                });
                continue;
            }
            for circle in level_demand.circles {
                let mut demand =
                    AdaptiveHybridDemand::physical_region(criterion_id.clone(), circle.clone());
                demand.source_resolution_m = Some(source_resolution_m);
                demand.target_edge_m = Some(target_edge_m);
                demands.push(demand);
                hard_regions.push(circle);
            }
        }
    }

    let adaptive_config = AdaptiveHybridConfig {
        max_cycles: options.max_cycles,
        target_size_tolerance: options.target_size_tolerance,
        stop_at_source_resolution: options.stop_at_source_resolution,
        maximum_neighbor_size_ratio: options.maximum_neighbor_size_ratio,
        maximum_vertices: options.maximum_vertices,
        maximum_insertions_per_cycle: options.maximum_insertions_per_cycle,
        minimum_triangle_angle: options.minimum_triangle_angle_deg,
        search: LeppSearchConfig {
            maximum_path_length: options.maximum_path_length,
            ..LeppSearchConfig::default()
        },
        gates: LeppInsertionGates::for_method_c(pentagons),
    };
    let mut boundary_segments = lepp_region_boundary_segments(&state, named_regions, domain_region);
    let refinement_started = std::time::Instant::now();
    eprintln!(
        "earthmesh_cli: LEPP AdaptiveHybrid mesh refinement started: {} demands, {} protected boundary segments, at most {} cycles",
        demands.len(),
        boundary_segments.len(),
        adaptive_config.max_cycles
    );
    let mut report = if boundary_segments.is_empty() {
        refine_adaptive_hybrid(&mut state, &demands, &adaptive_config)
    } else {
        refine_adaptive_hybrid_constrained(
            &mut state,
            &mut boundary_segments,
            &demands,
            &adaptive_config,
        )
    }
    .map_err(|error| io::Error::other(error.to_string()))?;
    eprintln!(
        "earthmesh_cli: LEPP AdaptiveHybrid mesh refinement complete: {} cycles, {} committed insertions, {} -> {} faces, stop={:?}, {:.1}s",
        report.cycles,
        report.path_stats.committed,
        report.initial_faces,
        report.final_faces,
        report.stop_reason,
        refinement_started.elapsed().as_secs_f64()
    );
    if !pre_unresolved.is_empty() {
        for unresolved in pre_unresolved {
            report.add_unresolved_demand(unresolved);
        }
        if matches!(
            report.stop_reason,
            earthmesh_refine_method_c::AdaptiveHybridStopReason::Satisfied
        ) {
            report.stop_reason =
                earthmesh_refine_method_c::AdaptiveHybridStopReason::NoCommittableInsertion;
        }
    }
    let refined = state.to_triangular_mesh(pentagons, None)?;
    let initial_voronoi = spherical_voronoi_state(&refined)?;
    let initial_output =
        gridfile_mesh_from_one_based_state(&initial_voronoi.grid, &initial_voronoi.tabs)?;
    let (output_mesh, spring_nest_passes) =
        spring_unstructured_region_interiors(&initial_output, &hard_regions, spring_iterations)?;
    let (voronoi, output_mesh) = if spring_nest_passes > 0 {
        let refined = triangular_mesh_from_unstructured(&output_mesh, pentagons)?;
        let voronoi = spherical_voronoi_state(&refined)?;
        let output = gridfile_mesh_from_one_based_state(&voronoi.grid, &voronoi.tabs)?;
        (voronoi, output)
    } else {
        (initial_voronoi, output_mesh)
    };
    Ok(RefinedGrid {
        state: Some(voronoi),
        output_mesh,
        method_c_metadata: None,
        pentagon_indices: pentagons,
        transition_faces: 0,
        spring_nest_passes,
        hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics::default(),
        adaptive_run: None,
        harp_dv_run: None,
        lepp_hard_regions: hard_regions,
        lepp_adaptive_hybrid: Some(report),
        lepp_post_quality: None,
    })
}

fn lepp_region_boundary_segments(
    state: &MeshState,
    named_regions: &[earthmesh_mesh::RefinementRegion],
    domain_region: Option<&GridRegion>,
) -> earthmesh_boundary::SegmentList {
    if named_regions.is_empty() && domain_region.is_none() {
        return earthmesh_boundary::SegmentList::default();
    }
    let edges = (earthmesh_mesh::MESH_STATE_FIRST_ID..state.triangles().len())
        .flat_map(|face| {
            let corners = state.triangles()[face];
            (0..3).map(move |corner| {
                let a = corners[(corner + 1) % 3];
                let b = corners[(corner + 2) % 3];
                (a.min(b), a.max(b))
            })
        })
        .collect::<BTreeSet<_>>();
    let radius = state.sphere_radius();
    let mut protected = Vec::new();
    for region in named_regions {
        protected.extend(
            earthmesh_boundary::SegmentList::from_straddling_edges(
                edges.iter().copied(),
                |vertex| region.contains_cartesian(state.vertices()[vertex], radius),
            )
            .iter(),
        );
    }
    if let Some(domain) = domain_region {
        protected.extend(
            earthmesh_boundary::SegmentList::from_straddling_edges(
                edges.iter().copied(),
                |vertex| {
                    let point = earthmesh_mesh::xyz_to_lonlat_degrees(state.vertices()[vertex]);
                    domain.contains(point.lon_degrees, point.lat_degrees)
                },
            )
            .iter(),
        );
    }
    earthmesh_boundary::SegmentList::from_pairs(protected)
}

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
    adaptive_options: Option<&crate::adaptive_refine::AdaptiveRefineOptions>,
    config: &EarthmeshConfig,
    domain_region: Option<&GridRegion>,
    mesh_type: &str,
    nxp: usize,
    max_level: usize,
    spring_iterations: usize,
    refine: &RefineConfig,
    options: HarpDvRunOptions,
) -> io::Result<RefinedGrid> {
    use earthmesh_refine_harp_dv as harp;

    // Said outright rather than served quietly with less. Each of these would
    // otherwise be dropped and the run would still write a valid mesh that is
    // not the mesh that was asked for.
    // Circles and closed curves are served; a box or a corridor is not, and is
    // still said outright rather than dropped. A closed curve became servable
    // when `SphericalBoundaryModel` arrived: the question a target scale asks
    // is "is this cell inside the region", and for a curve with holes that is
    // the model's subject rather than something to reimplement per backend.
    let unsupported = regions
        .iter()
        .find(|region| {
            !matches!(
                region,
                earthmesh_mesh::RefinementRegion::Circle { .. }
                    | earthmesh_mesh::RefinementRegion::Polygon { .. }
            )
        })
        .map(|_| "a region that is not a circle or a closed curve");
    if let Some(unsupported) = unsupported {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "NL%refine_backend = harp_dv does not serve {unsupported}; it reads a target \
                 scale per cell, and a box or corridor carries no discretised boundary to read \
                 one against. Use method_c for this run"
            ),
        ));
    }

    // A named region asks for a level; HARP-DV asks for a length. One level is
    // one halving, the same relation Method-C's nesting produces, so a level-L
    // request becomes the base cell width divided by two to the L.
    // Two different lengths, measured rather than derived, because
    // `2*pi*R/(5*nxp)` is neither of them cleanly. At NXP 21 that formula gives
    // 381 km; the mesh's median cell `sqrt(A/pi)` is 190 km and its median
    // triangle edge is 364 km. The formula is an edge length, near enough --
    // which is why the spring, which wants edge lengths, was accidentally
    // right, and why `TargetScale`, which compares cell scales, was asking for
    // half of what a level meant. Guide 11.31.

    // Only the cell scale is used here -- `TargetScale` compares cell scales,
    // and the spring converts to edge lengths from this same number so the two
    // cannot disagree. `harp_base_lengths` still measures both because the
    // pair is what makes the distinction checkable: 190 km against 364 km at
    // NXP 21 is why taking the nominal `2*pi*R/(5*nxp)` for either was wrong.
    let (base_cell_m, _base_edge_m) = harp_base_lengths(mesh).unwrap_or_else(|| {
        let nominal =
            2.0 * std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / (5.0 * nxp as f64);
        (nominal / 2.0, nominal)
    });
    let boundaries = harp_region_boundaries(regions)?;
    let mut spring_boundaries = boundaries.clone();
    let mut spring_target_scales = regions
        .iter()
        .map(|region| base_cell_m / 2.0_f64.powi(region.level() as i32))
        .collect::<Vec<_>>();
    let criteria: Vec<Box<dyn harp::CellCriterion>> = regions
        .iter()
        .zip(&boundaries)
        .enumerate()
        .map(|(index, (region, boundary))| {
            let target_scale_m = base_cell_m / 2.0_f64.powi(region.level() as i32);
            match boundary {
                HarpRegionBoundary::Circle {
                    center,
                    radius_meters,
                } => Box::new(harp::TargetScale {
                    id: format!("region-{index}"),
                    target_scale_m,
                    region: harp::TargetRegion::Circle {
                        centre: *center,
                        radius_m: *radius_meters,
                    },
                    source_resolution_m: None,
                }) as Box<dyn harp::CellCriterion>,
                // One closed curve at a time: each mask is its own demand, and a
                // model holding all of them at once would answer "inside" for a
                // cell in any of them, which is a different question.
                HarpRegionBoundary::Polygon(boundary) => Box::new(harp::TargetScale {
                    id: format!("region-{index}"),
                    target_scale_m,
                    region: harp::TargetRegion::Polygon {
                        boundary: boundary.clone(),
                    },
                    source_resolution_m: None,
                })
                    as Box<dyn harp::CellCriterion>,
            }
        })
        .collect();

    let mut criteria = criteria;
    let mut adaptive_run: Option<AdaptiveRunRecord> = None;
    if let Some(adaptive_options) = adaptive_options {
        let adaptive_base_m = adaptive_options.base_m.unwrap_or(base_cell_m);
        let adaptive_inputs = adaptive_demand_inputs(
            domain_region,
            config,
            adaptive_landtype_file(config),
            mesh_type,
            adaptive_options.coastline,
        )?;
        let adaptive_depth = adaptive_options.max_level.unwrap_or(max_level).clamp(1, 5);
        let gridnum_perdegree = usize::try_from(config.gridnum_perdegree).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "NL%gridnum_perdegree must fit usize",
            )
        })?;
        if gridnum_perdegree == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NL%gridnum_perdegree must be positive for HARP-DV source-resolution stopping",
            ));
        }
        let source_resolution_m =
            earthmesh_core::KM_PER_DEGREE_EQUATOR * 1000.0 / gridnum_perdegree as f64;
        let mut passes = Vec::new();
        let mut deepest_level = 0usize;
        let mut stopped_on_empty_demand = false;
        for level in 1..=adaptive_depth {
            let demand = crate::refinement_demand::nest::adaptive_demand_circles_for_level_windows(
                refine,
                &adaptive_inputs,
                level,
                adaptive_base_m,
                adaptive_depth,
            )?;
            eprintln!(
                "harp_dv adaptive level {level} judging {:.0} m cells: {} circles over {} demanded source cells",
                adaptive_base_m / 2f64.powi((level - 1) as i32),
                demand.circles.len(),
                demand.demanded_cells,
            );
            if demand.circles.is_empty() {
                if demand.demanded {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "{} demanded source cells at HARP-DV adaptive level {level}, but circle reduction produced no region",
                            demand.demanded_cells
                        ),
                    ));
                }
                stopped_on_empty_demand = true;
                break;
            }
            let criterion_id = if demand.criterion_ids.is_empty() {
                format!("adaptive-level-{level}")
            } else {
                format!("{}-level-{level}", demand.criterion_ids.join("+"))
            };
            spring_boundaries.extend(harp_region_boundaries(&demand.circles)?);
            let target_scale_m = adaptive_base_m / 2.0_f64.powi(level as i32);
            spring_target_scales.extend(std::iter::repeat_n(target_scale_m, demand.circles.len()));
            for (circle_index, circle) in demand.circles.iter().enumerate() {
                let earthmesh_mesh::RefinementRegion::Circle {
                    center,
                    radius_meters,
                    ..
                } = circle
                else {
                    continue;
                };
                criteria.push(Box::new(harp::TargetScale {
                    id: format!("{criterion_id}-{circle_index}"),
                    target_scale_m,
                    region: harp::TargetRegion::Circle {
                        centre: *center,
                        radius_m: *radius_meters,
                    },
                    source_resolution_m: Some(source_resolution_m),
                }));
            }
            deepest_level = level;
            passes.push(crate::refinement_demand::nest::NestPassReport {
                level,
                cell_meters: adaptive_base_m / 2f64.powi((level - 1) as i32),
                circle_count: demand.circles.len(),
                regions: demand.circles,
                demanded_cells: demand.demanded_cells,
                faces_before: mesh.nwd,
                faces_after: mesh.nwd,
            });
        }
        adaptive_run = Some((
            crate::refinement_demand::nest::AdaptiveNestReport {
                passes,
                deepest_level,
                stopped_on_empty_demand,
                spring_passes: 0,
            },
            adaptive_depth,
            adaptive_base_m,
            adaptive_options.coastline,
        ));
    }
    let mut adaptive = harp::AdaptiveMesh::from_triangular_mesh(mesh)
        .map_err(|error| io::Error::other(error.to_string()))?;

    // Quality as a criterion, with Ruppert's precondition satisfied: the sites
    // ringing each refinement region are protected segments, so a circumcentre
    // that encroaches on one splits it instead of being inserted. Without that
    // the refinement does not terminate (guide 11.25); with it, it reaches
    // Ruppert's angle bound (11.26).
    if let Some(min_angle_deg) = harp_min_angle_target(refine) {
        let segments = harp_region_boundary_segments(&adaptive, &spring_boundaries);
        adaptive.protect_segments(segments);
        criteria.push(Box::new(harp::MinAngle {
            id: "min-angle".to_string(),
            min_angle_deg,
        }));
    }
    let outcome = harp::refine_harp_dv(
        adaptive,
        &harp::HarpDvRequest {
            config: options.config,
            criteria: &criteria,
            candidate_policy: options.candidate_policy,
            gates: options.gates,
        },
    )
    .map_err(|error| io::Error::other(error.to_string()))?;

    // What it could not do, on the run's own output rather than in a log line
    // nobody reads afterwards.
    //
    // Gated on the stop reason and not only on `unresolved_cells`. A run can
    // stop with that list empty and still not have delivered what was asked:
    // the budget can run out mid-traversal, leaving the demands it never
    // reached out of the list; the cycle limit can arrive with committed but
    // still-unmet demands; and neighbour-scale imbalance is counted separately
    // from unresolved cells altogether. Each of those used to exit silently,
    // and a silent exit reads as "the mesh you asked for".
    let finished_clean = matches!(
        outcome.report.stop_reason,
        harp::StopReason::AllSatisfied | harp::StopReason::NoAcceptedTransactions
    ) && outcome.unresolved_cells.is_empty()
        && outcome.report.unbalanced_pairs_remaining == 0;
    // Only when the block below will not fire: it says the same things in more
    // detail whenever there are unresolved cells, and two lines saying one
    // thing trains people to read neither.
    if !finished_clean && outcome.unresolved_cells.is_empty() {
        eprintln!(
            "harp_dv: stopped because {:?}; {} cells unresolved, {} adjacent pairs past the \
             neighbour scale bound. The mesh below is what the run reached, not what was asked",
            outcome.report.stop_reason,
            outcome.unresolved_cells.len(),
            outcome.report.unbalanced_pairs_remaining
        );
    }
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
    if let Some((report, _, _, _)) = &mut adaptive_run {
        // HARP-DV evaluates every requested level together rather than running
        // one topology pass per level. Each row is therefore a demand level,
        // and the only honest topology counts are the shared run boundaries.
        for pass in &mut report.passes {
            pass.faces_before = mesh.nwd;
            pass.faces_after = refined.nwd;
        }
    }

    // Smooth with the nest spring, targeting each edge at what the *criteria*
    // asked for there. Guide 11.21 records the version that took targets from
    // the mesh's own current scale: that tells the spring to keep things as
    // they are, and 5000 iterations under it made the angles worse.
    let unsmoothed = (spring_iterations > 0).then(|| refined.clone());
    let mut spring_nest_passes = 0usize;
    let refined = if spring_iterations > 0 {
        match harp_spring_smoothed(
            &refined,
            &spring_boundaries,
            &spring_target_scales,
            base_cell_m,
            spring_iterations,
        ) {
            Ok(mesh) => {
                spring_nest_passes = 1;
                mesh
            }
            // A smoothing pass that declines is not a reason to lose the mesh.
            Err(error) => {
                eprintln!("harp_dv: nest spring declined ({error}); writing the unsmoothed mesh");
                refined
            }
        }
    } else {
        refined
    };
    // The writer's admissibility check runs over every triangle, and the
    // transaction gates only ever saw the ones a change touched. So the
    // decision is made here, where both meshes are still in hand: try the one
    // that was smoothed, and fall back to the one that was not rather than
    // failing the run.
    let (refined, state) = match spherical_voronoi_state(&refined) {
        Ok(state) => (refined, state),
        Err(error) if unsmoothed.is_some() => {
            eprintln!(
                "harp_dv: the smoothed mesh is not writable ({error}); falling back to the \
                 unsmoothed one"
            );
            spring_nest_passes = 0;
            let plain = unsmoothed.expect("checked");
            let state = spherical_voronoi_state(&plain)?;
            (plain, state)
        }
        Err(error) => return Err(error),
    };
    if let Some((report, _, _, _)) = &mut adaptive_run {
        report.spring_passes = spring_nest_passes;
    }
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
        spring_nest_passes,
        hfield_diagnostics: earthmesh_refine_method_c::MethodCHfieldSpawnDiagnostics::default(),
        adaptive_run,
        // HARP-DV's own ending, on the record rather than only on stderr.
        harp_dv_run: Some(HarpDvRunRecord {
            stop_reason: format!("{:?}", outcome.report.stop_reason),
            cycles_completed: outcome.report.cycles_completed,
            transactions_committed: outcome.report.transactions_committed,
            unresolved_cells: outcome.unresolved_cells.len(),
            unbalanced_pairs_remaining: outcome.report.unbalanced_pairs_remaining,
        }),
        lepp_hard_regions: Vec::new(),
        lepp_adaptive_hybrid: None,
        lepp_post_quality: None,
    })
}

/// The mesh's own median cell scale and median triangle edge, in metres.
///
/// Two quantities that a single nominal-spacing formula conflates. A criterion
/// comparing `sqrt(A/pi)` wants the first; a spring pulling on edges wants the
/// second.
fn harp_base_lengths(mesh: &earthmesh_mesh::TriangularMesh) -> Option<(f64, f64)> {
    let state = earthmesh_mesh::MeshState::from_triangular_mesh(mesh).ok()?;
    let radius = state.sphere_radius();
    let mut scales: Vec<f64> = (earthmesh_mesh::MESH_STATE_FIRST_ID..state.vertices().len())
        .filter_map(|site| {
            let cell = state.voronoi_cell(site).ok()?;
            let area = cell.area_on_unit_sphere()? * radius * radius;
            Some((area / std::f64::consts::PI).sqrt())
        })
        .collect();
    let mut edges: Vec<f64> = Vec::new();
    for triangle in earthmesh_mesh::MESH_STATE_FIRST_ID..state.triangles().len() {
        let corners = state.triangles()[triangle];
        for corner in 0..3 {
            edges.push(earthmesh_mesh::arc_length_unit_sphere(
                state.vertices()[corners[corner]],
                state.vertices()[corners[(corner + 1) % 3]],
            ));
        }
    }
    if scales.is_empty() || edges.is_empty() {
        return None;
    }
    let median = |values: &mut Vec<f64>| {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values[values.len() / 2]
    };
    Some((median(&mut scales), median(&mut edges)))
}

/// Which backend a run asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefineBackend {
    MethodC,
    RedGreen,
    HarpDv,
}

/// Resolve `NL%refine_backend`, refusing anything that is not a backend.
///
/// Case-insensitive, because `HARP_DV` asks for HARP-DV by any reading. What it
/// will not do is guess: the dispatch this replaced fell through to Method-C
/// for every unrecognised value, so `redgreen`, `harp-dv` and `method-c` each
/// produced a Method-C mesh in silence.
fn refine_backend_name(requested: &str) -> io::Result<RefineBackend> {
    let name = requested.trim().to_ascii_lowercase();
    match name.as_str() {
        "method_c" => Ok(RefineBackend::MethodC),
        "red_green" => Ok(RefineBackend::RedGreen),
        "harp_dv" => Ok(RefineBackend::HarpDv),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "NL%refine_backend = '{other}' is not a refinement backend; the choices are \
                 method_c, red_green and harp_dv"
            ),
        )),
    }
}

/// The angle floor a run asks for, if any.
///
/// Off unless asked: a quality criterion adds cells nobody requested, and
/// Ruppert's bound is about 20.7 degrees -- above it the refinement is not
/// guaranteed to terminate and a run can spend its whole budget.
///
/// From `RL%harp_min_angle_deg`. It used to come from an
/// `EARTHMESH_HARP_MIN_ANGLE` environment variable that no document and no
/// interface mentioned, so the only way to find the feature was to read this
/// function. The namelist is where a run says what it wants, and the parser
/// refuses anything above the bound rather than letting the run discover it.
fn harp_min_angle_target(refine: &RefineConfig) -> Option<f64> {
    (refine.harp_min_angle_deg > 0.0).then_some(refine.harp_min_angle_deg)
}

#[derive(Clone)]
enum HarpRegionBoundary {
    Circle {
        center: earthmesh_mesh::LonLatDegrees,
        radius_meters: f64,
    },
    Polygon(earthmesh_boundary::SphericalBoundaryModel),
}

/// Validate and compile region geometry once per HARP-DV run.
///
/// Both the demand criterion and protected-segment scan consume this result;
/// rebuilding a spherical boundary once per mesh vertex made polygon runs
/// quadratic in input-ring size and let invalid rings silently mean no demand.
fn harp_region_boundaries(
    regions: &[earthmesh_mesh::RefinementRegion],
) -> io::Result<Vec<HarpRegionBoundary>> {
    regions
        .iter()
        .enumerate()
        .map(|(index, region)| {
            region.validate().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("HARP-DV region {index} is invalid: {error}"),
                )
            })?;
            match region {
                earthmesh_mesh::RefinementRegion::Circle {
                    center,
                    radius_meters,
                    ..
                } => Ok(HarpRegionBoundary::Circle {
                    center: *center,
                    radius_meters: *radius_meters,
                }),
                earthmesh_mesh::RefinementRegion::Polygon { .. } => {
                    let boundary = crate::boundary_model::boundary_model_from_regions(
                        std::slice::from_ref(region),
                    );
                    if boundary.loops.is_empty() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("HARP-DV polygon region {index} does not enclose a loop"),
                        ));
                    }
                    if let Err(errors) = boundary.validate() {
                        let details = errors
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("; ");
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("HARP-DV polygon region {index} is invalid: {details}"),
                        ));
                    }
                    Ok(HarpRegionBoundary::Polygon(boundary))
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("HARP-DV region {index} is not a circle or closed curve"),
                )),
            }
        })
        .collect()
}

/// A refinement region's boundary, discretised as mesh edges.
///
/// The edges that straddle it -- one endpoint inside, one outside -- which is
/// what the curve looks like on this mesh. Ruppert's segments are a list like
/// this; a set of nearby *sites* is a different predicate and an unsound one
/// (guide 11.28).
fn harp_region_boundary_segments(
    mesh: &earthmesh_refine_harp_dv::AdaptiveMesh,
    regions: &[HarpRegionBoundary],
) -> Vec<(usize, usize)> {
    let state = mesh.state();
    let radius = state.sphere_radius();
    // What "inside" means is the run's business; what a segment list *is*, and
    // that a split replaces one with two, is `earthmesh_boundary`'s. Guide
    // 11.28 asked for the list to live there rather than be rebuilt at each
    // call site, because the version that lived here was a predicate wearing a
    // list's name and 11.29 measured what that cost.
    let inside = |site: usize| {
        let point = state.vertices()[site];
        regions
            .iter()
            .any(|region| harp_region_contains(region, point, radius))
    };
    let edges =
        (earthmesh_mesh::MESH_STATE_FIRST_ID..state.triangles().len()).flat_map(|triangle| {
            let corners = state.triangles()[triangle];
            (0..3).map(move |corner| (corners[(corner + 1) % 3], corners[(corner + 2) % 3]))
        });
    earthmesh_boundary::SegmentList::from_straddling_edges(edges, inside)
        .iter()
        .collect()
}

fn harp_region_contains(
    region: &HarpRegionBoundary,
    point: earthmesh_mesh::CartesianPoint,
    radius: f64,
) -> bool {
    let length = earthmesh_mesh::magnitude(point);
    if length <= 0.0 {
        return false;
    }
    match region {
        HarpRegionBoundary::Circle {
            center,
            radius_meters,
        } => {
            let centre = earthmesh_mesh::lonlat_degrees_to_unit_xyz(*center);
            let dot = (point.x * centre.x + point.y * centre.y + point.z * centre.z) / length;
            dot.clamp(-1.0, 1.0).acos() * radius <= *radius_meters
        }
        HarpRegionBoundary::Polygon(boundary) => {
            let here = earthmesh_mesh::xyz_to_lonlat_degrees(point);
            boundary.contains(here.lon_degrees, here.lat_degrees)
        }
    }
}

/// Smooth a HARP-DV mesh against the sizes the criteria asked for.
///
/// The targets come from the criteria, not from the mesh -- which is what
/// makes this different from the attempt in guide 11.21. They are passed
/// beside the compiled boundaries so an explicit `adaptive_base_m` reaches
/// the spring instead of being silently replaced by the measured base scale.
///
/// The conversion from a cell width to a triangle edge length is measured off
/// this mesh rather than derived: the two differ by a shape factor that
/// depends on the dual, and measuring it is both shorter and harder to get
/// wrong than deriving it.
fn harp_spring_smoothed(
    mesh: &earthmesh_mesh::TriangularMesh,
    boundaries: &[HarpRegionBoundary],
    target_scales_m: &[f64],
    base_cell_m: f64,
    iterations: usize,
) -> io::Result<earthmesh_mesh::TriangularMesh> {
    if boundaries.len() != target_scales_m.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HARP-DV spring regions and target scales must have the same length",
        ));
    }
    let radius = earthmesh_core::EARTH_RADIUS_METERS;

    // The factor from a cell width to a triangle edge length, measured on the
    // parts of the mesh that were *not* refined -- those are the ones whose
    // width is `base_cell_m` by definition.
    //
    // Taking the median over every edge instead was wrong and worked by
    // accident: it is right only while refined edges are a minority, and a run
    // that refines most of its domain would get a factor pulled down by the
    // short edges, shrink every target, and have the spring compress the whole
    // mesh.
    let inside = |a: &earthmesh_mesh::CartesianPoint, b: &earthmesh_mesh::CartesianPoint| {
        let middle = earthmesh_mesh::CartesianPoint::new(
            (a.x + b.x) / 2.0,
            (a.y + b.y) / 2.0,
            (a.z + b.z) / 2.0,
        );
        boundaries
            .iter()
            .any(|region| harp_region_contains(region, middle, radius))
    };
    let mut edges: Vec<f64> = Vec::new();
    for iu in 2..=mesh.nud {
        let [im1, im2] = mesh.u_edges[iu].im;
        let (Some(a), Some(b)) = (mesh.m_points.get(im1), mesh.m_points.get(im2)) else {
            continue;
        };
        if inside(a, b) {
            continue;
        }
        let length = earthmesh_mesh::arc_length_unit_sphere(*a, *b);
        if length.is_finite() && length > 0.0 {
            edges.push(length);
        }
    }
    if edges.is_empty() {
        return Err(io::Error::other(
            "every edge is inside a refinement region, so there is no unrefined \
             scale to calibrate the spring targets against",
        ));
    }
    edges.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    // A conversion, not a guard. `width` below is built from `base_cell_m` --
    // a cell scale, `sqrt(A/pi)` -- and `spring_nest_with_edge_targets` wants
    // edge lengths, so every target has to cross between the two. Measured at
    // NXP 21: the median unrefined edge is 363 km against a 190 km cell scale,
    // so the factor is 1.91. An earlier comment here called it "near one",
    // which described a version whose divisor was the median *edge*; dividing
    // by that would leave cell scales fed to an edge-target spring, which is
    // the halved-target defect guide 11.31 records.
    let shape_factor = edges[edges.len() / 2] / base_cell_m;
    let targets =
        earthmesh_refine_method_c::method_c_edge_target_lengths_from_field(mesh, |lon, lat| {
            let here = earthmesh_mesh::lonlat_degrees_to_unit_xyz(
                earthmesh_mesh::LonLatDegrees::new(lon, lat),
            );
            // Gradient-limited, not a step at the region boundary. A target that
            // jumps from base to base/4 across one edge asks the spring for a
            // discontinuity it can only answer with a sliver; letting it grow
            // back at a bounded rate is what an h-field does and what a circle
            // list on its own does not.
            //
            // 0.3 metres of growth per metre of distance: shallow enough that
            // the transition spans a few cells, steep enough that a target
            // does not reach across the globe.
            //
            // Swept 0.05 to 0.50 both before and after the scale correction;
            // it changes nothing either way (guide 11.24, 11.32). What matters
            // is that the field is continuous, not its slope. The sweep left an
            // `EM_G` environment override here that was read into a variable
            // nothing used -- so the knob a reader would reach for did nothing,
            // and the constant below is what the run has always used.
            const GRADIENT: f64 = 0.3;
            let mut width = base_cell_m;
            for (boundary, asked) in boundaries.iter().zip(target_scales_m) {
                let outside = match boundary {
                    HarpRegionBoundary::Circle {
                        center,
                        radius_meters,
                    } => {
                        let centre = earthmesh_mesh::lonlat_degrees_to_unit_xyz(*center);
                        let dot = (here.x * centre.x + here.y * centre.y + here.z * centre.z)
                            .clamp(-1.0, 1.0);
                        (dot.acos() * radius - *radius_meters).max(0.0)
                    }
                    HarpRegionBoundary::Polygon(boundary) => {
                        let here = earthmesh_mesh::xyz_to_lonlat_degrees(here);
                        if boundary.contains(here.lon_degrees, here.lat_degrees) {
                            0.0
                        } else {
                            boundary
                                .distance_to_boundary_radians(here.lon_degrees, here.lat_degrees)
                                .unwrap_or(std::f64::consts::PI)
                                * radius
                        }
                    }
                };
                width = width.min(*asked + GRADIENT * outside);
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
        let inputs = adaptive_demand_inputs(
            domain_region,
            config,
            adaptive_landtype_file(config),
            mesh_type,
            adaptive.coastline,
        )?;
        // The spring the run configured, on the route that is the default.
        // Without this the two `spawn_nest` calls inside were the bare overload
        // and every point stayed where the nest put it, while the report went on
        // printing the iteration count it had been asked for. Measured on the
        // same namelist with and without `&adaptive`: the direct route moved
        // 5182 of 7023 points in two passes, this one moved none. Guide 11.39.
        let spring = (spring_nest_iterations > 0).then_some(
            crate::refinement_demand::nest::AdaptiveNestSpring {
                nxp: method_c_nxp,
                iterations: spring_nest_iterations,
                max_mrows: if is_atmosmesh {
                    MethodCMesh::MAX_MROWS_ATMOS
                } else {
                    MethodCMesh::MAX_MROWS_SURFACE
                },
            },
        );
        let (refined, report) =
            crate::refinement_demand::nest::spawn_nest_adaptive_with_named_region_windows(
                &mesh, refine, &inputs, regions, base_m, depth, spring,
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
        let sprang = report.spring_passes;
        (refined, sprang)
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
            let hfield_spring_iterations = refinement_spring_iterations(refine, is_atmosmesh)?;
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

/// Source windows the adaptive route evaluates criteria over.
///
/// A wrapped regional domain is two longitude intervals. Keeping those as two
/// source windows avoids the old full-band scan, and the plan inputs still carry
/// `domain_region` so cells inside the windows but outside the true shape cannot
/// consume demand budget.
fn adaptive_demand_inputs<'a>(
    domain_region: Option<&'a GridRegion>,
    config: &'a EarthmeshConfig,
    landtype_file: Option<&'a std::path::Path>,
    mesh_type: &'a str,
    refine_coastline: bool,
) -> io::Result<Vec<crate::refinement_demand::plan::DemandPlanInputs<'a>>> {
    let gridnum_perdegree = usize::try_from(config.gridnum_perdegree).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%gridnum_perdegree must fit usize",
        )
    })?;
    adaptive_demand_windows(domain_region, config)?
        .into_iter()
        .map(|bounds| {
            Ok(crate::refinement_demand::plan::DemandPlanInputs {
                bounds,
                gridnum_perdegree,
                landtype_file,
                mesh_type,
                refine_coastline,
                domain_region,
            })
        })
        .collect()
}

fn adaptive_demand_windows(
    domain_region: Option<&GridRegion>,
    config: &EarthmeshConfig,
) -> io::Result<Vec<earthmesh_mesh::AreaJudgeSourceBounds>> {
    let gridnum_perdegree = usize::try_from(config.gridnum_perdegree).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%gridnum_perdegree must fit usize",
        )
    })?;
    let windows = match domain_region {
        Some(region) => {
            let windows = region_lonlat_windows(region).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "regional adaptive demand domain has no valid lon/lat extent",
                )
            })?;
            if windows.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "regional adaptive demand domain has no non-empty source window",
                ));
            }
            merge_lonlat_windows(windows)
        }
        None => {
            let nlons = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "global source longitude overflows",
                )
            })?;
            let nlats = gridnum_perdegree.checked_mul(180).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "global source latitude overflows",
                )
            })?;
            // ponytail: 30-degree tiles bound peak raster memory; make this
            // adaptive only if profiling shows file-open overhead dominates.
            let tile = gridnum_perdegree.checked_mul(30).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "global source tile size overflows",
                )
            })?;
            let mut bounds = Vec::new();
            for lat_start in (1..=nlats).step_by(tile) {
                for lon_start in (1..=nlons).step_by(tile) {
                    bounds.push(earthmesh_mesh::AreaJudgeSourceBounds {
                        minlon_source: lon_start,
                        maxlon_source: (lon_start + tile - 1).min(nlons),
                        maxlat_source: lat_start,
                        minlat_source: (lat_start + tile - 1).min(nlats),
                    });
                }
            }
            return Ok(bounds);
        }
    };
    windows
        .into_iter()
        .map(|window| {
            crate::refinement_demand::source_bounds_for_bbox(
                window.west,
                window.east,
                window.south,
                window.north,
                gridnum_perdegree,
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LonLatWindow {
    west: f64,
    east: f64,
    south: f64,
    north: f64,
}

impl LonLatWindow {
    fn new(west: f64, east: f64, south: f64, north: f64) -> Option<Self> {
        (west.is_finite()
            && east.is_finite()
            && south.is_finite()
            && north.is_finite()
            && east > west
            && north > south)
            .then_some(Self {
                west,
                east,
                south,
                north,
            })
    }

    fn touches(self, other: Self) -> bool {
        self.west <= other.east
            && other.west <= self.east
            && self.south <= other.north
            && other.south <= self.north
    }

    fn union(self, other: Self) -> Self {
        Self {
            west: self.west.min(other.west),
            east: self.east.max(other.east),
            south: self.south.min(other.south),
            north: self.north.max(other.north),
        }
    }
}

/// Enclosing lon/lat windows of a regional shape, or `None` when it has no bound.
fn region_lonlat_windows(region: &GridRegion) -> Option<Vec<LonLatWindow>> {
    match region {
        GridRegion::Bbox {
            west,
            east,
            south,
            north,
        } if west.is_finite()
            && east.is_finite()
            && south.is_finite()
            && north.is_finite()
            && south < north
            && west != east =>
        {
            Some(split_lon_window(*west, *east, *south, *north))
        }
        GridRegion::Bbox { .. } => None,
        GridRegion::Circle {
            lon,
            lat,
            radius_km,
        } if lon.is_finite() && lat.is_finite() && radius_km.is_finite() && *radius_km > 0.0 => {
            let angular = radius_km / (earthmesh_core::EARTH_RADIUS_METERS / 1000.0);
            let lat_rad = lat.to_radians();
            let lat_pad = angular.to_degrees();
            let south = (lat - lat_pad).max(-90.0);
            let north = (lat + lat_pad).min(90.0);
            if angular >= std::f64::consts::PI
                || lat_rad + angular >= std::f64::consts::FRAC_PI_2
                || lat_rad - angular <= -std::f64::consts::FRAC_PI_2
            {
                return Some(vec![LonLatWindow::new(-180.0, 180.0, south, north)?]);
            }
            let lon_pad = (angular.sin() / lat_rad.cos().abs())
                .clamp(-1.0, 1.0)
                .asin()
                .abs()
                .to_degrees();
            let lon = normalize_lon_for_window(*lon);
            Some(split_lon_window(lon - lon_pad, lon + lon_pad, south, north))
        }
        GridRegion::Circle { .. } => None,
        GridRegion::Close { points } => {
            let mut windows = close_lonlat_windows(points)?;
            let contains_north_pole = region.contains(0.0, 90.0);
            let contains_south_pole = region.contains(0.0, -90.0);
            if contains_north_pole || contains_south_pole {
                let south = if contains_south_pole {
                    -90.0
                } else {
                    windows
                        .iter()
                        .map(|window| window.south)
                        .fold(90.0, f64::min)
                };
                let north = if contains_north_pole {
                    90.0
                } else {
                    windows
                        .iter()
                        .map(|window| window.north)
                        .fold(-90.0, f64::max)
                };
                windows = vec![LonLatWindow::new(-180.0, 180.0, south, north)?];
            }
            Some(windows)
        }
        GridRegion::Any(regions) => {
            let windows = regions
                .iter()
                .map(region_lonlat_windows)
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            (!windows.is_empty()).then_some(windows)
        }
    }
}

fn split_lon_window(west: f64, east: f64, south: f64, north: f64) -> Vec<LonLatWindow> {
    if !west.is_finite() || !east.is_finite() {
        return Vec::new();
    }
    if (east - west).abs() >= 360.0 {
        return LonLatWindow::new(-180.0, 180.0, south, north)
            .into_iter()
            .collect();
    }
    let west = normalize_lon_for_window(west);
    let east = normalize_lon_for_window(east);
    if west < east {
        LonLatWindow::new(west, east, south, north)
            .into_iter()
            .collect()
    } else {
        [
            LonLatWindow::new(west, 180.0, south, north),
            LonLatWindow::new(-180.0, east, south, north),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

fn merge_lonlat_windows(windows: Vec<LonLatWindow>) -> Vec<LonLatWindow> {
    let mut merged: Vec<LonLatWindow> = Vec::new();
    'next: for window in windows {
        let mut window = window;
        loop {
            let Some(index) = merged.iter().position(|existing| existing.touches(window)) else {
                merged.push(window);
                continue 'next;
            };
            let existing = merged.swap_remove(index);
            window = existing.union(window);
        }
    }
    merged.sort_by(|a, b| {
        a.west
            .partial_cmp(&b.west)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.south
                    .partial_cmp(&b.south)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    merged
}

fn close_lonlat_windows(points: &[crate::LonLatPoint]) -> Option<Vec<LonLatWindow>> {
    if points.len() < 3
        || points.iter().any(|point| {
            !point.lon.is_finite() || !point.lat.is_finite() || !(-90.0..=90.0).contains(&point.lat)
        })
    {
        return None;
    }
    let points = if points.len() > 1 && close_points_coincide(points[0], points[points.len() - 1]) {
        &points[..points.len() - 1]
    } else {
        points
    };
    if points.len() < 3 {
        return None;
    }
    let (mut south, mut north) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut lons = Vec::new();
    for point in points {
        south = south.min(point.lat);
        north = north.max(point.lat);
        lons.push(normalize_lon_for_window(point.lon));
    }
    expand_close_latitude_bounds(points, &mut south, &mut north)?;
    lons.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    lons.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12);
    if lons.len() == 1 {
        let lon = lons[0];
        let west = (lon - 5.0e-10).max(-180.0);
        let east = (lon + 5.0e-10).min(180.0);
        return Some(vec![LonLatWindow::new(west, east, south, north)?]);
    }
    let mut largest_gap = -1.0;
    let mut gap_after = 0usize;
    for i in 0..lons.len() {
        let next = if i + 1 == lons.len() {
            lons[0] + 360.0
        } else {
            lons[i + 1]
        };
        let gap = next - lons[i];
        if gap > largest_gap {
            largest_gap = gap;
            gap_after = i;
        }
    }
    let start = lons[(gap_after + 1) % lons.len()];
    let end = lons[gap_after];
    Some(split_lon_window(start, end, south, north))
}

fn close_points_coincide(a: crate::LonLatPoint, b: crate::LonLatPoint) -> bool {
    let (a_lon, a_lat, b_lon, b_lat) = (
        a.lon.to_radians(),
        a.lat.to_radians(),
        b.lon.to_radians(),
        b.lat.to_radians(),
    );
    let a = [
        a_lat.cos() * a_lon.cos(),
        a_lat.cos() * a_lon.sin(),
        a_lat.sin(),
    ];
    let b = [
        b_lat.cos() * b_lon.cos(),
        b_lat.cos() * b_lon.sin(),
        b_lat.sin(),
    ];
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let cross_norm = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    cross_norm.atan2(dot) <= 1.0e-12
}

/// Include latitude extrema reached between ring vertices by minor great-circle arcs.
fn expand_close_latitude_bounds(
    points: &[crate::LonLatPoint],
    south: &mut f64,
    north: &mut f64,
) -> Option<()> {
    let unit = |point: crate::LonLatPoint| {
        let (lon, lat) = (point.lon.to_radians(), point.lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let norm = |point: [f64; 3]| dot(point, point).sqrt();
    let angle = |a: [f64; 3], b: [f64; 3]| norm(cross(a, b)).atan2(dot(a, b));

    for edge in 0..points.len() {
        let a = unit(points[edge]);
        let b = unit(points[(edge + 1) % points.len()]);
        let edge_angle = angle(a, b);
        if edge_angle <= 1.0e-12 || (std::f64::consts::PI - edge_angle).abs() <= 1.0e-10 {
            return None;
        }
        let normal = cross(a, b);
        let normal_squared = dot(normal, normal);
        let projected = [
            -normal[0] * normal[2] / normal_squared,
            -normal[1] * normal[2] / normal_squared,
            1.0 - normal[2] * normal[2] / normal_squared,
        ];
        let projected_length = norm(projected);
        if projected_length <= 1.0e-12 {
            continue;
        }
        let maximum = [
            projected[0] / projected_length,
            projected[1] / projected_length,
            projected[2] / projected_length,
        ];
        for candidate in [maximum, [-maximum[0], -maximum[1], -maximum[2]]] {
            if angle(a, candidate) + angle(candidate, b) <= edge_angle + 1.0e-10 {
                let latitude = candidate[2].clamp(-1.0, 1.0).asin().to_degrees();
                *south = south.min(latitude);
                *north = north.max(latitude);
            }
        }
    }
    Some(())
}

fn normalize_lon_for_window(lon: f64) -> f64 {
    let normalized = ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    if (normalized + 180.0).abs() < 1.0e-12 && lon > 0.0 {
        180.0
    } else {
        normalized
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

    #[test]
    fn cell_area_metrics_always_use_the_minor_spherical_patch() {
        let tiny = 1.0e-3;
        assert_eq!(minor_cell_steradians(tiny), Some(tiny));
        assert!(
            (minor_cell_steradians(4.0 * std::f64::consts::PI - tiny).unwrap() - tiny).abs()
                < 1.0e-12
        );
        assert_eq!(minor_cell_steradians(f64::NAN), None);
    }

    #[test]
    fn lepp_region_constraints_are_real_mesh_edges() {
        let method_c =
            MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
        let state = MeshState::from_triangular_mesh(method_c.mesh()).expect("mesh state");
        let region = earthmesh_mesh::RefinementRegion::Bbox {
            west_degrees: -20.0,
            east_degrees: 20.0,
            south_degrees: -90.0,
            north_degrees: 90.0,
            level: 1,
        };
        let segments = lepp_region_boundary_segments(&state, &[region], None);
        assert!(!segments.is_empty());
        let mesh_edges = (earthmesh_mesh::MESH_STATE_FIRST_ID..state.triangles().len())
            .flat_map(|face| {
                let corners = state.triangles()[face];
                (0..3).map(move |corner| {
                    let a = corners[(corner + 1) % 3];
                    let b = corners[(corner + 2) % 3];
                    (a.min(b), a.max(b))
                })
            })
            .collect::<BTreeSet<_>>();
        assert!(segments.iter().all(|edge| mesh_edges.contains(&edge)));
    }

    #[test]
    fn harp_rejects_invalid_polygon_geometry_before_refinement() {
        let crossing = earthmesh_mesh::RefinementRegion::Polygon {
            points: vec![
                earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(10.0, 10.0),
                earthmesh_mesh::LonLatDegrees::new(0.0, 10.0),
                earthmesh_mesh::LonLatDegrees::new(10.0, 0.0),
            ],
            level: 1,
        };
        let error = match harp_region_boundaries(&[crossing]) {
            Ok(_) => panic!("self-intersecting HARP polygon must fail"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("crosses itself"));

        let valid = earthmesh_mesh::RefinementRegion::Polygon {
            points: vec![
                earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(10.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(10.0, 10.0),
                earthmesh_mesh::LonLatDegrees::new(0.0, 10.0),
            ],
            level: 1,
        };
        let compiled = harp_region_boundaries(&[valid]).expect("valid polygon");
        assert!(harp_region_contains(
            &compiled[0],
            earthmesh_mesh::lonlat_degrees_to_unit_xyz(earthmesh_mesh::LonLatDegrees::new(
                5.0, 5.0
            )),
            earthmesh_core::EARTH_RADIUS_METERS,
        ));
    }

    #[test]
    fn harp_polygon_targets_reach_the_spring_path() {
        let mesh = earthmesh_refine_method_c::MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh")
            .into_inner();
        let region = earthmesh_mesh::RefinementRegion::Polygon {
            points: vec![
                earthmesh_mesh::LonLatDegrees::new(-20.0, -20.0),
                earthmesh_mesh::LonLatDegrees::new(20.0, -20.0),
                earthmesh_mesh::LonLatDegrees::new(20.0, 20.0),
                earthmesh_mesh::LonLatDegrees::new(-20.0, 20.0),
            ],
            level: 1,
        };
        let boundaries = harp_region_boundaries(std::slice::from_ref(&region)).expect("boundary");
        let (base_cell_m, _) = harp_base_lengths(&mesh).expect("base scale");
        let smoothed =
            harp_spring_smoothed(&mesh, &boundaries, &[base_cell_m / 2.0], base_cell_m, 1)
                .expect("polygon target spring");
        assert_eq!(smoothed.nmd, mesh.nmd);
        assert_eq!(smoothed.nud, mesh.nud);
        assert_eq!(smoothed.nwd, mesh.nwd);
    }

    #[test]
    fn backend_neutral_regional_spring_moves_only_safe_region_interiors() {
        let base = earthmesh_refine_method_c::MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh")
            .into_inner();
        let neighbors = base.m_neighbors.clone();
        let redgreen = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("red-green bridge");
        let mut mesh = crate::redgreen_bridge::unstructured_mesh_from_redgreen(&redgreen)
            .expect("unstructured mesh");
        let region = earthmesh_mesh::RefinementRegion::Bbox {
            west_degrees: -120.0,
            east_degrees: 120.0,
            south_degrees: -70.0,
            north_degrees: 70.0,
            level: 1,
        };
        let mask =
            spring_region_interior_mask(&mesh, std::slice::from_ref(&region)).expect("spring mask");
        let moved = (2..mask.len())
            .find(|&cell| mask[cell])
            .expect("movable cell");
        let fixed = (2..mask.len())
            .find(|&cell| !mask[cell])
            .expect("fixed boundary cell");
        let original = mesh.clone();
        mesh.w_points[moved].lon += 5.0;
        let before_moved = mesh.w_points[moved];
        let before_fixed = mesh.w_points[fixed];

        let (smoothed, passes) =
            spring_unstructured_region_interiors(&mesh, std::slice::from_ref(&region), 1)
                .expect("regional spring");

        assert_eq!(passes, 1);
        assert_ne!(smoothed.w_points[moved], before_moved);
        assert_eq!(smoothed.w_points[fixed], before_fixed);
        assert_eq!(smoothed.m_to_w, mesh.m_to_w);
        assert_eq!(smoothed.w_to_m, mesh.w_to_m);
        assert!(
            crate::unstructured_mesh_support::check_unstructured_mesh_topology(&smoothed)
                .is_consistent()
        );

        let mut slightly_worse = original;
        slightly_worse.w_points[moved].lon += 0.25;
        let (kept, passes) =
            spring_unstructured_region_interiors(&slightly_worse, std::slice::from_ref(&region), 1)
                .expect("quality fallback");
        assert_eq!(passes, 0);
        assert_eq!(kept, slightly_worse);
    }

    #[test]
    fn redgreen_consumes_the_configured_refinement_spring() {
        let mesh = earthmesh_refine_method_c::MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh")
            .into_inner();
        let region = earthmesh_mesh::RefinementRegion::Bbox {
            west_degrees: -120.0,
            east_degrees: 120.0,
            south_degrees: -70.0,
            north_degrees: 70.0,
            level: 1,
        };
        let refine = RefineConfig {
            is_transition: true,
            weak_concav_eliminate: false,
            ..RefineConfig::default()
        };

        let refined = refine_with_redgreen(&mesh, &[region], &refine, 1, None, true, 1)
            .expect("red-green with spring");

        assert_eq!(refined.spring_nest_passes, 1);
        assert!(
            crate::unstructured_mesh_support::check_unstructured_mesh_topology(
                &refined.output_mesh
            )
            .is_consistent()
        );
    }

    /// A demand grid is `nlons * nlats`, so this is the cost of its windows.
    fn demand_cells(region: Option<&GridRegion>, per_degree: i32) -> usize {
        let config = EarthmeshConfig {
            gridnum_perdegree: per_degree,
            ..EarthmeshConfig::default()
        };
        adaptive_demand_windows(region, &config)
            .expect("bounds")
            .into_iter()
            .map(|bounds| {
                (bounds.maxlon_source - bounds.minlon_source + 1)
                    * (bounds.minlat_source - bounds.maxlat_source + 1)
            })
            .sum()
    }

    #[test]
    fn invalid_regional_domains_do_not_fall_back_to_global_demand() {
        let config = EarthmeshConfig {
            gridnum_perdegree: 1,
            ..EarthmeshConfig::default()
        };
        for region in [
            GridRegion::Circle {
                lon: 0.0,
                lat: 0.0,
                radius_km: 0.0,
            },
            GridRegion::Bbox {
                west: 10.0,
                east: 10.0,
                south: 0.0,
                north: 1.0,
            },
            GridRegion::Close {
                points: vec![crate::LonLatPoint {
                    lon: f64::NAN,
                    lat: 0.0,
                }],
            },
        ] {
            let err = adaptive_demand_windows(Some(&region), &config)
                .expect_err("invalid regional domain must fail");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        }

        assert!(
            adaptive_demand_windows(None, &config).is_ok(),
            "only domain=None falls back to global"
        );

        let partly_invalid = GridRegion::Any(vec![
            GridRegion::Bbox {
                west: 0.0,
                east: 1.0,
                south: 0.0,
                north: 1.0,
            },
            GridRegion::Close { points: vec![] },
        ]);
        assert_eq!(
            adaptive_demand_windows(Some(&partly_invalid), &config)
                .expect_err("an invalid union member must not be silently dropped")
                .kind(),
            io::ErrorKind::InvalidInput
        );
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
    fn global_adaptive_demand_is_complete_but_memory_bounded() {
        let config = EarthmeshConfig {
            gridnum_perdegree: 240,
            ..EarthmeshConfig::default()
        };
        let windows = adaptive_demand_windows(None, &config).expect("global windows");
        let cells = |bounds: &earthmesh_mesh::AreaJudgeSourceBounds| {
            (bounds.maxlon_source - bounds.minlon_source + 1)
                * (bounds.minlat_source - bounds.maxlat_source + 1)
        };
        assert_eq!(
            windows.iter().map(cells).sum::<usize>(),
            360 * 240 * 180 * 240,
            "tiling must neither drop nor duplicate a source cell"
        );
        assert!(
            windows.iter().map(cells).max().unwrap_or(0) <= (30usize * 240).pow(2),
            "no individual demand allocation may cover the whole globe"
        );
    }

    #[test]
    fn spherical_close_windows_cover_poles_and_great_circle_bulges() {
        let polar = GridRegion::Close {
            points: vec![
                crate::LonLatPoint {
                    lon: -120.0,
                    lat: 80.0,
                },
                crate::LonLatPoint {
                    lon: 0.0,
                    lat: 80.0,
                },
                crate::LonLatPoint {
                    lon: 120.0,
                    lat: 80.0,
                },
            ],
        };
        let windows = region_lonlat_windows(&polar).expect("polar close window");
        assert_eq!(windows.len(), 1);
        assert_eq!(
            (windows[0].west, windows[0].east, windows[0].north),
            (-180.0, 180.0, 90.0)
        );

        let bulging_edge = [
            crate::LonLatPoint {
                lon: -45.0,
                lat: 45.0,
            },
            crate::LonLatPoint {
                lon: 45.0,
                lat: 45.0,
            },
            crate::LonLatPoint { lon: 0.0, lat: 0.0 },
        ];
        let windows = close_lonlat_windows(&bulging_edge).expect("great-circle bounds");
        assert!(
            windows.iter().any(|window| window.north > 54.7),
            "the minor arc rises above its 45-degree endpoints: {windows:?}"
        );

        let explicitly_closed = [
            crate::LonLatPoint { lon: 0.0, lat: 0.0 },
            crate::LonLatPoint { lon: 4.0, lat: 0.0 },
            crate::LonLatPoint { lon: 0.0, lat: 4.0 },
            crate::LonLatPoint {
                lon: 360.0,
                lat: 0.0,
            },
        ];
        assert!(
            close_lonlat_windows(&explicitly_closed).is_some(),
            "a repeated physical first point is an accepted explicit closure"
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
        let windows = region_lonlat_windows(&union).expect("union bounds");
        assert_eq!(
            windows,
            vec![
                LonLatWindow {
                    west: 100.0,
                    east: 110.0,
                    south: 10.0,
                    north: 20.0,
                },
                LonLatWindow {
                    west: 130.0,
                    east: 140.0,
                    south: 30.0,
                    north: 40.0,
                },
            ]
        );
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

    /// A bbox that crosses the antimeridian is two source windows, not a
    /// full-longitude band.
    #[test]
    fn a_bbox_across_the_antimeridian_produces_two_windows() {
        let wrapped = GridRegion::Bbox {
            west: 170.0,
            east: -170.0,
            south: -10.0,
            north: 10.0,
        };
        let windows = region_lonlat_windows(&wrapped).expect("a wrapped bbox has bounds");
        assert_eq!(
            windows,
            vec![
                LonLatWindow {
                    west: 170.0,
                    east: 180.0,
                    south: -10.0,
                    north: 10.0,
                },
                LonLatWindow {
                    west: -180.0,
                    east: -170.0,
                    south: -10.0,
                    north: 10.0,
                },
            ]
        );

        let global_band = demand_cells(
            Some(&GridRegion::Bbox {
                west: -180.0,
                east: 180.0,
                south: -10.0,
                north: 10.0,
            }),
            120,
        );
        let wrapped_cells = demand_cells(Some(&wrapped), 120);
        assert!(
            wrapped_cells * 10 < global_band,
            "wrapped 20-degree bbox must not scan the full band: {wrapped_cells} vs {global_band}"
        );

        let plain = GridRegion::Bbox {
            west: 100.0,
            east: 120.0,
            south: -10.0,
            north: 10.0,
        };
        assert_eq!(
            region_lonlat_windows(&plain),
            Some(vec![LonLatWindow {
                west: 100.0,
                east: 120.0,
                south: -10.0,
                north: 10.0,
            }])
        );
    }

    #[test]
    fn overlapping_any_windows_merge_even_with_different_latitudes() {
        let union = GridRegion::Any(vec![
            GridRegion::Bbox {
                west: 10.0,
                east: 20.0,
                south: 0.0,
                north: 10.0,
            },
            GridRegion::Bbox {
                west: 15.0,
                east: 25.0,
                south: 5.0,
                north: 15.0,
            },
        ]);
        assert_eq!(
            region_lonlat_windows(&union).map(merge_lonlat_windows),
            Some(vec![LonLatWindow {
                west: 10.0,
                east: 25.0,
                south: 0.0,
                north: 15.0,
            }])
        );
    }

    #[test]
    fn a_midlatitude_circle_uses_spherical_longitude_padding() {
        let radius_km = (earthmesh_core::EARTH_RADIUS_METERS / 1000.0) * 30_f64.to_radians();
        let circle = GridRegion::Circle {
            lon: 0.0,
            lat: 45.0,
            radius_km,
        };
        let windows = region_lonlat_windows(&circle).expect("circle window");
        assert_eq!(windows.len(), 1);
        assert!(
            windows[0].west < -44.9 && windows[0].east > 44.9,
            "30-degree great-circle radius at 45N needs about ±45 degrees longitude, got {windows:?}"
        );
    }

    #[test]
    fn overlapping_any_windows_merge_before_planning() {
        let config = EarthmeshConfig {
            gridnum_perdegree: 10,
            ..EarthmeshConfig::default()
        };
        let union = GridRegion::Any(vec![
            GridRegion::Bbox {
                west: 170.0,
                east: -175.0,
                south: -5.0,
                north: 5.0,
            },
            GridRegion::Bbox {
                west: 175.0,
                east: -170.0,
                south: -5.0,
                north: 5.0,
            },
        ]);
        let windows = adaptive_demand_windows(Some(&union), &config).expect("windows");
        assert_eq!(windows.len(), 2, "overlapping seam halves are merged");
        let cells: usize = windows
            .iter()
            .map(|b| {
                (b.maxlon_source - b.minlon_source + 1) * (b.minlat_source - b.maxlat_source + 1)
            })
            .sum();
        let full_band = demand_cells(
            Some(&GridRegion::Bbox {
                west: -180.0,
                east: 180.0,
                south: -5.0,
                north: 5.0,
            }),
            10,
        );
        assert!(
            cells * 10 < full_band,
            "merged windows still avoid full-band scan"
        );
    }

    #[test]
    fn a_circle_touching_a_pole_uses_all_longitudes() {
        let cap = GridRegion::Circle {
            lon: 40.0,
            lat: 89.0,
            radius_km: 500.0,
        };
        assert_eq!(
            region_lonlat_windows(&cap),
            Some(vec![LonLatWindow {
                west: -180.0,
                east: 180.0,
                south: 89.0 - (500.0 / (earthmesh_core::EARTH_RADIUS_METERS / 1000.0)).to_degrees(),
                north: 90.0,
            }])
        );
    }

    /// A circle on the seam keeps the half that used to be clipped away.
    ///
    /// The box was clamped to 180, so for a circle centred near the dateline
    /// the far side was never scanned and whatever a criterion asked for there
    /// disappeared without a word.
    #[test]
    fn a_circle_on_the_antimeridian_keeps_both_sides() {
        let straddling = GridRegion::Circle {
            lon: 179.0,
            lat: 0.0,
            radius_km: 500.0,
        };
        let windows = region_lonlat_windows(&straddling).expect("bounds");
        assert_eq!(windows.len(), 2, "the far side is a second tight window");
        assert!(windows.iter().any(|w| w.west < -179.0 && w.east < -170.0));
        assert!(windows.iter().any(|w| w.west > 170.0 && w.east == 180.0));

        // A circle well clear of the seam still gets its own tight box.
        let inland = GridRegion::Circle {
            lon: 100.0,
            lat: 0.0,
            radius_km: 500.0,
        };
        let windows = region_lonlat_windows(&inland).expect("bounds");
        assert_eq!(windows.len(), 1);
        assert!(
            windows[0].west > 90.0 && windows[0].east < 110.0,
            "an inland circle must not widen to the whole band: {windows:?}"
        );
    }

    /// The window a wrapped bbox asks for is one a run can actually build.
    ///
    /// The bounds alone are not the claim -- what broke was the call they feed.
    #[test]
    fn a_wrapped_window_reaches_source_bounds_without_an_error() {
        let wrapped = GridRegion::Bbox {
            west: 170.0,
            east: -170.0,
            south: -10.0,
            north: 10.0,
        };
        for window in region_lonlat_windows(&wrapped).expect("bounds") {
            crate::refinement_demand::source_bounds_for_bbox(
                window.west,
                window.east,
                window.south,
                window.north,
                1,
            )
            .expect("a wrapped window must produce source bounds");
        }
    }
}
