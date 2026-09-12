use crate::atomic_output::publish_artifacts;
use crate::certified_options::{
    read_certified_options, CertifiedDelivery, CertifiedMode, CertifiedRunOptions,
};
use crate::final_quality_non_negative_usize;
use crate::fvcom_mesh_2dm_output_path;
use crate::gridfile_mesh_from_one_based_state;
use crate::method_c_algorithm::{
    read_method_c_algorithm_options, MethodCAlgorithm, MethodCAlgorithmOptions,
};
use crate::method_c_delaunay_mesh_from_unstructured_gridfile;
use crate::method_c_refinement_region_level;
use crate::mkgrd_run_types::{
    CertifiedRunRecord, LeppAdaptiveHybridRunRecord, LeppPostQualityRunRecord,
};
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
use crate::read_obc_order_netcdf;
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
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, QualityNamelist, RefineConfig};
use earthmesh_mesh::{
    grid_cartesian_xy_to_lonlat_placeholders_one_based_state, grid_xyz2lonlat_one_based_state,
    pcvt_adjust_voronoi_grid_state, voronoi_grid_from_triangular_mesh,
    voronoi_grid_from_triangular_mesh_cartesian, MeshState, RefinementRegion, TriangularMesh,
};
use rayon::prelude::*;

use super::outputs::{write_refined_outputs, MethodCMetadataSlices};
use crate::{write_clean_regional_ocean_gridfile, write_fvcom_2dm_from_carved};

const REMAP_CSV_CHUNK_ROWS: usize = 4096;

fn cmrc_timing_enabled() -> bool {
    std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1")
}

fn log_cmrc_phase(enabled: bool, phase: &str, started: &mut Instant) {
    if enabled {
        eprintln!(
            "earthmesh_cli: cmrc_timing phase={phase} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        *started = Instant::now();
    }
}

fn format_remap_csv_row(row: &earthmesh_refine_certified::remap::RemapRow) -> String {
    let mut output = String::with_capacity(row.sources.len().saturating_mul(32));
    for &(source, weight) in &row.sources {
        std::fmt::Write::write_fmt(
            &mut output,
            format_args!("{},{},{weight:.17}\n", row.target, source),
        )
        .expect("writing remap CSV to String cannot fail");
    }
    output
}

fn write_remap_csv<W: Write>(
    writer: &mut W,
    rows: &[earthmesh_refine_certified::remap::RemapRow],
) -> io::Result<()> {
    writer.write_all(b"target,source,weight\n")?;
    for chunk in rows.chunks(REMAP_CSV_CHUNK_ROWS) {
        let formatted = chunk
            .par_iter()
            .map(format_remap_csv_row)
            .collect::<Vec<_>>();
        for row in formatted {
            writer.write_all(row.as_bytes())?;
        }
    }
    Ok(())
}

fn certified_gridfile_refine_levels(
    mesh: &crate::UnstructuredMesh,
    delivered_levels: &[usize],
) -> io::Result<(Vec<i32>, Vec<i32>)> {
    let w_has_placeholders =
        crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows(&mesh.w_points);
    let mut source_levels = delivered_levels.iter().copied();
    let mut w_levels = vec![0; mesh.w_points.len()];
    for (row, level) in w_levels.iter_mut().enumerate() {
        let Some(id) =
            crate::unstructured_mesh_support::mesh_canonical_id_for_row(row, w_has_placeholders)
        else {
            continue;
        };
        if id <= 1 {
            continue;
        }
        *level = i32::try_from(source_levels.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "CMRC delivered levels do not cover every published W cell",
            )
        })?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "CMRC level exceeds i32"))?;
    }
    if source_levels.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "CMRC delivered levels exceed the published W-cell count",
        ));
    }

    let m_has_placeholders =
        crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows(&mesh.m_points);
    let mut m_levels = vec![0; mesh.m_points.len()];
    for (row, triangle) in mesh.m_to_w.iter().enumerate() {
        let Some(id) =
            crate::unstructured_mesh_support::mesh_canonical_id_for_row(row, m_has_placeholders)
        else {
            continue;
        };
        if id <= 1 {
            continue;
        }
        m_levels[row] = triangle
            .iter()
            .map(|&w_id| {
                crate::unstructured_mesh_support::mesh_row_for_canonical_id(
                    w_id,
                    mesh.w_points.len(),
                    w_has_placeholders,
                )
                .and_then(|w_row| w_levels.get(w_row).copied())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("CMRC triangle row {row} has invalid W id {w_id}"),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
    }
    Ok((m_levels, w_levels))
}

// These IDs identify the final closed-sphere export rows, not coarsening ancestry.
fn certified_gridfile_pre_export_lineages(mesh: &crate::UnstructuredMesh) -> (Vec<i64>, Vec<i64>) {
    let m_has_placeholders =
        crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows(&mesh.m_points);
    let w_has_placeholders =
        crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows(&mesh.w_points);
    let lineage_for_row = |row: usize, has_placeholders: bool| {
        crate::unstructured_mesh_support::mesh_canonical_id_for_row(row, has_placeholders)
            .map(i64::from)
            .unwrap_or(0)
    };
    (
        (0..mesh.m_points.len())
            .map(|row| lineage_for_row(row, m_has_placeholders))
            .collect(),
        (0..mesh.w_points.len())
            .map(|row| lineage_for_row(row, w_has_placeholders))
            .collect(),
    )
}

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
    let backend = refine_backend_name(&config.refine_backend)?;
    if earthmesh_core::namelist_has_section(&contents, "harp_dv") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "retired &harp_dv namelist section is no longer supported; use method_c, red_green, or certified",
        ));
    }
    if std::env::var_os("EARTHMESH_CMRC_LOCAL_UPDATE").is_some()
        && backend != RefineBackend::Certified
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local updates require the certified backend",
        ));
    }
    if backend == RefineBackend::Certified {
        return run_certified_pipeline(
            &contents,
            &config,
            read_certified_options(&contents)?,
            workdir.as_ref(),
            max_tris,
        );
    }
    let quality = QualityNamelist::from_quality_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let method_c_algorithm = read_method_c_algorithm_options(&contents)?;
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
    let has_threshold_sources =
        refine.refine_cal && crate::hfield_refine::has_threshold_hfield_sources(&refine, mesh_type);
    let has_threshold_hfield_sources = use_hfield_regions && has_threshold_sources;
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
        !matches!(calculated_region_prefix, "" | "/tmp" | "none");
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
    // in a `_ =>` arm that ran Method-C. Measured: misspellings
    // `redgreen` and `method-c` produced a Method-C mesh and
    // said nothing -- a user asking for one backend and silently getting
    // another, which is the failure class guide 11.1 records.
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
            // The shared statistical planner owns degree-zero evaluation
            // windows; do not also inject them as hard refinement regions.
            has_threshold_sources && (use_hfield_regions || adaptive_options.is_some()),
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
        let source_lineages =
            crate::grid_quality_pipeline::read_gridfile_cell_lineages(&gridinit.gridfile.output)?;
        method_c_delaunay_mesh_from_unstructured_gridfile(
            &source_gridfile,
            MethodCGridfileMetadataSlices {
                mpas: None,
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
                m_lineage: (!source_lineages.m.is_empty()).then_some(source_lineages.m.as_slice()),
                w_lineage: (!source_lineages.w.is_empty()).then_some(source_lineages.w.as_slice()),
            },
            nxp,
            usize::try_from(config.niter).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "NL%niter must fit usize")
            })?,
            config.beta,
            config.relax,
        )?
    };
    let requested_spring_nest_iterations = if native_only_spawn {
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
    let spring_nest_iterations =
        effective_refinement_spring_iterations(backend, requested_spring_nest_iterations);
    if requested_spring_nest_iterations > 0 && spring_nest_iterations == 0 {
        // Name the settings that were dropped, not the mechanism. The message
        // used to say "regional Laplacian spring", which reads as "not you" to
        // the far more common configuration that reaches here: a namelist with
        // RL%SpringRegional_type = 0 and RL%SpringGlobal_type = 1, whose
        // RL%niter_refine is the number actually being discarded.
        let requested_by = match (refine.spring_global_type, refine.spring_regional_type) {
            (1, r) if r > 0 => "RL%SpringGlobal_type = 1 and RL%SpringRegional_type",
            (1, _) => "RL%SpringGlobal_type = 1",
            _ => "RL%SpringRegional_type",
        };
        eprintln!(
            "earthmesh_cli: ignoring {requested_by} and the {requested_spring_nest_iterations} \
             refinement spring iteration(s) they ask for. certified refinement owns its geometry certificate, and a Laplacian spring on top of that would invalidate it. Use NL%refine_backend = method_c to run the spring instead.              This does not affect NL%niter, the initial quasi-uniform relaxation, which still runs."
        );
    }
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
                    lepp_hard_regions: Vec::new(),
                    lepp_adaptive_hybrid: None,
                    lepp_post_quality,
                }
            }
        }
        RefineBackend::Certified => unreachable!("CMRC is dispatched before source-grid setup"),
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
        gridinit: Some(gridinit),
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
        certified_run: None,
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

struct CertifiedConstruction {
    geometry: Box<earthmesh_refine_certified::GeometryCertifiedMotherGrid>,
    pentagons: [usize; 12],
    remap: earthmesh_refine_certified::remap::ConservativeRemap,
    remap_certificate: earthmesh_refine_certified::remap::RemapCertificate,
    final_cell_requirements: Option<earthmesh_refine_certified::FinalCellRequirementCertificate>,
    delivered_level: usize,
    delivered_levels: Vec<usize>,
    coarsening_strategy: &'static str,
    initial_subdivision: usize,
    final_subdivision: usize,
    initial_cells: usize,
    attempted_patches: usize,
    accepted_patches: usize,
    removed_vertices: usize,
    removed_faces: usize,
    search_budget_exhausted: bool,
    components_total: usize,
    components_committed: usize,
    components_promoted: usize,
    components_exhausted: usize,
    search_complete: bool,
    elastic_report: Option<earthmesh_refine_certified::coarsen::ElasticCmrcReport>,
    local_update: Option<serde_json::Value>,
}

fn certified_subdivision(base_nxp: usize, level: usize) -> io::Result<usize> {
    let scale = 1usize.checked_shl(level as u32).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC level overflows the platform subdivision range",
        )
    })?;
    base_nxp.checked_mul(scale).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC mother subdivision overflows usize",
        )
    })
}

// Before scanning threshold rasters, reject only families with no possible
// supported mother. The actual chosen level still passes full certification.
fn validate_certified_mother_family(base_nxp: usize, maximum_level: usize) -> io::Result<()> {
    let possible = (0..=maximum_level.min(usize::BITS as usize - 1))
        .filter_map(|level| certified_subdivision(base_nxp, level).ok())
        .any(earthmesh_refine_certified::certificate::is_supported_mother_subdivision);
    if !possible {
        return Err(io::Error::new(io::ErrorKind::Unsupported, format!(
            "CMRC NXP={base_nxp} has no certified mother subdivision at levels 0..={maximum_level}; rejected before threshold preparation"
        )));
    }
    Ok(())
}

fn build_certified_construction(
    base_nxp: usize,
    chosen_level: usize,
    options: &CertifiedRunOptions,
    raster_requirements: &earthmesh_refine_certified::RasterLevelField,
    max_tris: usize,
    local_update_path: Option<&Path>,
) -> io::Result<CertifiedConstruction> {
    let budget = options.maximum_cells.min(max_tris);
    if options.mode == CertifiedMode::SafeMotherOnly {
        let subdivision = certified_subdivision(base_nxp, chosen_level)?;
        let mut config = earthmesh_refine_certified::CertifiedConfig::mother_only(subdivision);
        config.angle_contract = options.angle_contract;
        config.max_cells = Some(budget);
        config.grading_ring_width = options.gradation_rings_per_level;
        config.delivery = match options.delivery {
            CertifiedDelivery::Tri => earthmesh_refine_certified::DeliveryMode::Triangular,
            CertifiedDelivery::Hex => earthmesh_refine_certified::DeliveryMode::Voronoi,
            CertifiedDelivery::Coupled => earthmesh_refine_certified::DeliveryMode::Coupled,
        };
        let geometry = match earthmesh_refine_certified::generate_certified_mother_grid(&config) {
            earthmesh_refine_certified::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
            other => return Err(certified_outcome_error(other)),
        };
        let cell_count = geometry.primal().vertex_count();
        let face_count = geometry.primal().triangle_count();
        let pentagons = certified_mother_pentagons(geometry.primal())?;
        let remap = earthmesh_refine_certified::remap::ConservativeRemap::identity_for_mesh(
            geometry.primal(),
        );
        let remap_certificate = remap.certify_identity(cell_count);
        return Ok(CertifiedConstruction {
            initial_cells: face_count,
            geometry,
            pentagons,
            remap,
            remap_certificate,
            final_cell_requirements: None,
            delivered_level: chosen_level,
            delivered_levels: vec![chosen_level; cell_count],
            coarsening_strategy: "none",
            initial_subdivision: subdivision,
            final_subdivision: subdivision,
            attempted_patches: 0,
            accepted_patches: 0,
            removed_vertices: 0,
            removed_faces: 0,
            search_budget_exhausted: false,
            components_total: 0,
            components_committed: 0,
            components_promoted: 0,
            components_exhausted: 0,
            search_complete: true,
            elastic_report: None,
            local_update: None,
        });
    }

    if chosen_level > 0
        && raster_requirements
            .levels()
            .iter()
            .any(|&level| level < chosen_level)
    {
        return build_mixed_certified_construction(
            base_nxp,
            chosen_level,
            options,
            raster_requirements,
            budget,
            local_update_path,
        );
    }

    let initial_level = chosen_level.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC initial level overflows usize",
        )
    })?;
    if initial_level > options.maximum_level {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "CMRC MaximumLevelReached: reverse coarsening needs safe level {initial_level}, maximum {}",
                options.maximum_level
            ),
        ));
    }
    let initial_subdivision = certified_subdivision(base_nxp, initial_level)?;
    let required_cells =
        earthmesh_refine_certified::mother_grid::mother_cell_count(initial_subdivision)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "CMRC initial mother cell count overflows usize",
                )
            })?;
    if required_cells > budget {
        return Err(certified_outcome_error(
            earthmesh_refine_certified::CertifiedMeshOutcome::CellBudgetInsufficient {
                required_cells,
                budget,
            },
        ));
    }
    let fine = earthmesh_refine_certified::MotherGrid::generate(initial_subdivision)
        .map_err(io::Error::other)?;
    earthmesh_refine_certified::Certificate::final_delivery_for(options.angle_contract)
        .verify_mother_grid(&fine)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC initial mother certification failed: {error}"),
            )
        })?;
    let initial_cells = fine.mesh.triangle_count();
    let initial_mesh = fine.mesh.clone();
    let hierarchy_components_total =
        earthmesh_refine_certified::coarsen::complete_four_child_patch_candidates(&fine).len();
    match earthmesh_refine_certified::coarsen::rebuild_one_level_from_complete_mother_patches(
        fine,
        options.search_budget,
    ) {
        earthmesh_refine_certified::coarsen::HierarchyRebuildOutcome::Rebuilt {
            mesh,
            removed_vertices,
            removed_faces,
            candidates,
            remap: _,
            remap_certificate: _,
        } => {
            let remap =
                earthmesh_refine_certified::remap::ConservativeRemap::between_voronoi_meshes(
                    &initial_mesh,
                    mesh.primal(),
                )
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("CMRC Voronoi remap failed: {error}"),
                    )
                })?;
            let remap_certificate = remap.certify_spherical_overlap(
                initial_mesh.vertex_count(),
                mesh.primal().vertex_count(),
            );
            Ok(CertifiedConstruction {
                pentagons: certified_mother_pentagons(mesh.primal())?,
                delivered_levels: vec![chosen_level; mesh.primal().active_vertex_slots().count()],
                coarsening_strategy: "complete_global_hierarchy_2_to_1",
                geometry: mesh,
                remap,
                remap_certificate,
                final_cell_requirements: None,
                delivered_level: chosen_level,
                initial_subdivision,
                final_subdivision: certified_subdivision(base_nxp, chosen_level)?,
                initial_cells,
                attempted_patches: candidates.len(),
                accepted_patches: candidates.len(),
                removed_vertices,
                removed_faces,
                search_budget_exhausted: false,
                components_total: candidates.len(),
                components_committed: candidates.len(),
                components_promoted: 0,
                components_exhausted: 0,
                search_complete: true,
                elastic_report: None,
                local_update: None,
            })
        }
        earthmesh_refine_certified::coarsen::HierarchyRebuildOutcome::SearchBudgetExhausted {
            attempted_patches,
            snapshot_unchanged,
            mesh,
        } => {
            if !snapshot_unchanged {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "CMRC exhausted hierarchy search changed its rollback snapshot",
                ));
            }
            let geometry = match earthmesh_refine_certified::certify_mother_grid(mesh) {
                earthmesh_refine_certified::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
                other => return Err(certified_outcome_error(other)),
            };
            let cell_count = geometry.primal().vertex_count();
            let remap = earthmesh_refine_certified::remap::ConservativeRemap::identity_for_mesh(
                geometry.primal(),
            );
            let remap_certificate = remap.certify_identity(cell_count);
            let pentagons = certified_mother_pentagons(geometry.primal())?;
            Ok(CertifiedConstruction {
                geometry,
                pentagons,
                remap,
                remap_certificate,
                final_cell_requirements: None,
                delivered_level: initial_level,
                delivered_levels: vec![initial_level; cell_count],
                coarsening_strategy: "retained_fine_mother_after_budget_exhaustion",
                initial_subdivision,
                final_subdivision: initial_subdivision,
                initial_cells,
                attempted_patches,
                accepted_patches: 0,
                removed_vertices: 0,
                removed_faces: 0,
                search_budget_exhausted: true,
                components_total: hierarchy_components_total,
                components_committed: 0,
                components_promoted: hierarchy_components_total,
                components_exhausted: 1,
                search_complete: false,
                elastic_report: None,
                local_update: None,
            })
        }
        earthmesh_refine_certified::coarsen::HierarchyRebuildOutcome::UnsupportedCavity {
            reason,
            ..
        } => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("CMRC CriterionNotCertifiable: {reason}"),
        )),
    }
}

fn build_mixed_certified_construction(
    base_nxp: usize,
    chosen_level: usize,
    options: &CertifiedRunOptions,
    raster_requirements: &earthmesh_refine_certified::RasterLevelField,
    budget: usize,
    local_update_path: Option<&Path>,
) -> io::Result<CertifiedConstruction> {
    let timing_enabled = cmrc_timing_enabled();
    let mut phase_started = Instant::now();
    let initial_subdivision = certified_subdivision(base_nxp, chosen_level)?;
    let required_cells =
        earthmesh_refine_certified::mother_grid::mother_cell_count(initial_subdivision)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "CMRC mixed mother cell count overflows usize",
                )
            })?;
    if required_cells > budget {
        return Err(certified_outcome_error(
            earthmesh_refine_certified::CertifiedMeshOutcome::CellBudgetInsufficient {
                required_cells,
                budget,
            },
        ));
    }
    let fine = earthmesh_refine_certified::MotherGrid::generate(initial_subdivision)
        .map_err(io::Error::other)?;
    log_cmrc_phase(timing_enabled, "mother_grid", &mut phase_started);
    let source_pentagons = certified_icosahedron_vertices(&fine.addresses)?;
    earthmesh_refine_certified::Certificate::internal_for(options.angle_contract)
        .verify_mother_grid(&fine)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC initial mixed mother certification failed: {error}"),
            )
        })?;
    log_cmrc_phase(
        timing_enabled,
        "initial_geometry_certificate",
        &mut phase_started,
    );
    let initial_mesh = fine.mesh.clone();
    let initial_vertices = initial_mesh.vertex_count();
    let initial_faces = initial_mesh.triangle_count();
    let initial_levels = earthmesh_refine_certified::TargetLevelField::from_active_voronoi_cells(
        &initial_mesh,
        vec![chosen_level; initial_mesh.active_vertex_slots().count()],
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let projected = earthmesh_refine_certified::certify_final_cell_requirements_from_raster(
        raster_requirements,
        &initial_mesh,
        &initial_levels,
        1,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CMRC initial raster projection failed: {error}"),
        )
    })?;
    log_cmrc_phase(
        timing_enabled,
        "initial_requirement_projection",
        &mut phase_started,
    );
    let source_levels = earthmesh_refine_certified::SourceLevelField::from_active_voronoi_cells(
        &initial_mesh,
        projected.required_levels().to_vec(),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let active_sites = initial_mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut cell_by_site = vec![usize::MAX; initial_mesh.vertices().len()];
    for (cell, &site) in active_sites.iter().enumerate() {
        cell_by_site[site] = cell;
    }
    let mut adjacency = vec![Vec::new(); active_sites.len()];
    for (left, right) in earthmesh_refine_certified::requirement::target_site_edges(&initial_mesh) {
        let left = cell_by_site[left];
        let right = cell_by_site[right];
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    let graded = earthmesh_refine_certified::requirement::graded_envelope(
        &adjacency,
        projected.required_levels(),
        options.gradation_rings_per_level,
    );
    let mut graded_by_site = vec![usize::MAX; initial_mesh.vertices().len()];
    for (&site, level) in active_sites.iter().zip(graded) {
        graded_by_site[site] = level;
    }
    log_cmrc_phase(timing_enabled, "graded_envelope", &mut phase_started);
    let epoch = earthmesh_refine_certified::coarsen::run_elastic_component_epochs(
        fine,
        &initial_mesh,
        &source_levels,
        &graded_by_site,
        &earthmesh_refine_certified::coarsen::ElasticCmrcConfig {
            angle_contract: options.angle_contract,
            max_level: chosen_level,
            max_adjacent_level_delta: 1,
            initial_transition_rings: 1,
            maximum_transition_rings: 4,
            topology_states_per_component: options.search_budget.clamp(1, 10_000),
            elastic_iterations_per_topology: 256,
            interval_boxes_per_component: initial_faces.saturating_mul(3),
            total_transition_states: options.search_budget,
            allow_safe_fallback: false,
        },
    );
    log_cmrc_phase(
        timing_enabled,
        "elastic_component_epochs",
        &mut phase_started,
    );
    let result = match epoch {
        earthmesh_refine_certified::coarsen::ElasticCmrcOutcome::Completed(result) => result,
        earthmesh_refine_certified::coarsen::ElasticCmrcOutcome::NotCertifiable { reason } => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC mixed coarsening lost final-cell certification: {reason}"),
            ));
        }
        earthmesh_refine_certified::coarsen::ElasticCmrcOutcome::InvalidInput { reason } => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, reason));
        }
    };
    let leaf_mesh = result.state.mesh();
    let pentagons = source_pentagons
        .into_iter()
        .map(|source| {
            leaf_mesh
                .source_vertex_slots
                .iter()
                .position(|slot| *slot == Some(source))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("CMRC removed protected icosahedron vertex {source}"),
                    )
                })
        })
        .collect::<io::Result<Vec<_>>>()?
        .try_into()
        .expect("twelve source pentagons map to twelve compact sites");
    let mut mesh = leaf_mesh.mesh.clone();
    let delivered_levels = result
        .state
        .target_levels()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .levels()
        .to_vec();
    // Trial updates precede every final source/remap/geometry gate.
    // Invalid input aborts; a rejected trial cannot mutate the retained control.
    let local_update = local_update_path
        .map(|path| {
            let mut candidate = mesh.clone();
            match super::cmrc_local_updates::apply(
                &mut candidate,
                &delivered_levels,
                &pentagons,
                path,
                options.angle_contract,
            ) {
                Ok(report) => {
                    mesh = candidate;
                    Ok(serde_json::json!({"decision": "candidate", "proposal_validation": report}))
                }
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    Ok(serde_json::json!({"decision": "control", "reason": error.to_string()}))
                }
                Err(error) => Err(error),
            }
        })
        .transpose()?;
    if let Some(report) = &local_update {
        eprintln!("earthmesh_cli: experimental_local_update {report}");
    }
    let final_cell_requirements = if mesh == initial_mesh
        && delivered_levels.iter().all(|&level| level == chosen_level)
    {
        projected
    } else {
        let final_levels = earthmesh_refine_certified::TargetLevelField::from_active_voronoi_cells(
            &mesh,
            delivered_levels.clone(),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        earthmesh_refine_certified::certify_final_cell_requirements_from_raster(
            raster_requirements,
            &mesh,
            &final_levels,
            1,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC final-cell certification failed: {error}"),
            )
        })?
    };
    log_cmrc_phase(
        timing_enabled,
        "final_requirement_projection",
        &mut phase_started,
    );
    let remap = if initial_mesh == mesh {
        earthmesh_refine_certified::remap::ConservativeRemap::identity_for_mesh(&mesh)
    } else {
        earthmesh_refine_certified::remap::ConservativeRemap::between_voronoi_meshes(
            &initial_mesh,
            &mesh,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC mixed Voronoi remap failed: {error}"),
            )
        })?
    };
    log_cmrc_phase(timing_enabled, "voronoi_remap", &mut phase_started);
    let remap_certificate =
        remap.certify_spherical_overlap(initial_mesh.vertex_count(), mesh.vertex_count());
    let geometry = match earthmesh_refine_certified::certify_geometry_with_contract(
        mesh,
        options.angle_contract,
    ) {
        earthmesh_refine_certified::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
        other => return Err(certified_outcome_error(other)),
    };
    log_cmrc_phase(
        timing_enabled,
        "final_geometry_certificate",
        &mut phase_started,
    );
    Ok(CertifiedConstruction {
        delivered_level: delivered_levels.iter().copied().max().unwrap_or(0),
        delivered_levels,
        coarsening_strategy: "elastic_component_epochs",
        pentagons,
        initial_subdivision,
        final_subdivision: initial_subdivision,
        initial_cells: initial_faces,
        attempted_patches: result.report.components_total,
        accepted_patches: result.report.components_committed,
        removed_vertices: initial_vertices - geometry.primal().vertex_count(),
        removed_faces: initial_faces - geometry.primal().triangle_count(),
        search_budget_exhausted: !result.report.search_complete,
        components_total: result.report.components_total,
        components_committed: result.report.components_committed,
        components_promoted: result.report.components_promoted,
        components_exhausted: result.report.components_exhausted,
        search_complete: result.report.search_complete,
        geometry,
        remap,
        remap_certificate,
        final_cell_requirements: Some(final_cell_requirements),
        elastic_report: Some(result.report.clone()),
        local_update,
    })
}

fn elastic_outcome_name(
    outcome: earthmesh_refine_certified::coarsen::ComponentOutcomeKind,
) -> &'static str {
    outcome.as_str()
}

fn elastic_report_json(
    report: &earthmesh_refine_certified::coarsen::ElasticCmrcReport,
) -> serde_json::Value {
    serde_json::json!({
        "aggregate": {
            "initial_faces": report.initial_faces,
            "final_faces": report.final_faces,
            "initial_vertices": report.initial_vertices,
            "final_vertices": report.final_vertices,
            "components_total": report.components_total,
            "components_committed": report.components_committed,
            "components_promoted": report.components_promoted,
            "components_exhausted": report.components_exhausted,
            "total_topology_states": report.total_topology_states,
            "total_elastic_iterations": report.total_elastic_iterations,
            "total_interval_boxes": report.total_interval_boxes,
            "core_vertices_removed": report.core_vertices_removed,
            "search_complete": report.search_complete,
        },
        "requested_histogram": report.requested_histogram,
        "delivered_histogram": report.delivered_histogram,
        "per_level_histograms": report.levels.iter().map(|level| serde_json::json!({
            "source_level": level.source_level,
            "target_level": level.target_level,
            "components_total": level.components_total,
            "components_committed": level.components_committed,
            "components_promoted": level.components_promoted,
            "components_exhausted": level.components_exhausted,
            "delivered_histogram": level.delivered_histogram,
        })).collect::<Vec<_>>(),
        "per_component_records": report.components.iter().map(|component| serde_json::json!({
            "component_id": component.component_id,
            "source_level": component.source_level,
            "target_level": component.target_level,
            "parent_count": component.parent_count,
            "core_parent_count": component.core_parent_count,
            "transition_parent_count": component.transition_parent_count,
            "core_vertices_removed": component.core_vertices_removed,
            "topology_states": component.topology_states,
            "elastic_iterations": component.elastic_iterations,
            "interval_boxes": component.interval_boxes,
            "transition_ring_width": component.transition_ring_width,
            "outcome": elastic_outcome_name(component.outcome),
            "reason": component.reason,
        })).collect::<Vec<_>>(),
    })
}

pub(super) struct CertifiedDomainPublication {
    pub(super) report: crate::UnstructuredMeshWriteReport,
    pub(super) kept_cells: usize,
    pub(super) topology: serde_json::Value,
    pub(super) quality_topology: (usize, Vec<serde_json::Value>),
    pub(super) geometry: serde_json::Value,
    pub(super) fvcom_2dm: Option<crate::FvcomMesh2dmWriteReport>,
}

struct CertifiedMpasPublication {
    report: crate::MpasFullMeshPipelineReport,
    mesh_density_min: f64,
    mesh_density_max: f64,
    step: usize,
}

fn certified_mpas_cellwidth(base_nxp: usize, w_refine_levels: &[i32]) -> io::Result<Vec<f64>> {
    if base_nxp == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC MPAS export requires positive NXP",
        ));
    }
    let base_width = 7680.0 / base_nxp as f64;
    w_refine_levels
        .iter()
        .map(|&level| {
            if level < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "CMRC MPAS export requires non-negative W refinement levels",
                ));
            }
            Ok(base_width / 2_f64.powi(level))
        })
        .collect()
}

fn publish_certified_atmos_mpas(
    mesh: &crate::UnstructuredMesh,
    context: &crate::mpas_gridfile_context::MpasGridfileContext,
    mesh_output: &Path,
    graph_output: &Path,
) -> io::Result<CertifiedMpasPublication> {
    let step = context.step;
    let mpas = crate::build_mpas_mesh_from_unstructured_one_based(
        mesh,
        &context.cellwidth_km,
        context.base_nxp,
        step,
    )?;
    let mesh_density_min = mpas.mesh_density[1..]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let mesh_density_max = mpas.mesh_density[1..]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let mesh_report = crate::write_mpas_mesh_netcdf(mesh_output, &mpas)?;
    let graph_info = crate::write_mpas_graph_info(
        graph_output,
        10,
        &mpas.cells_on_cell,
        &mpas.cells_on_edge,
        &mpas.n_edges_on_cell,
    )?;
    Ok(CertifiedMpasPublication {
        report: crate::MpasFullMeshPipelineReport {
            mesh: mesh_report,
            graph_info,
        },
        mesh_density_min,
        mesh_density_max,
        step,
    })
}

#[allow(clippy::too_many_arguments)]
fn publish_certified_domain_gridfile(
    source_gridfile: &Path,
    output_gridfile: &Path,
    config: &EarthmeshConfig,
    base_nxp: usize,
    workdir: &Path,
    domain_region: Option<&GridRegion>,
    angle_contract: earthmesh_refine_certified::AngleContractId,
    fvcom_output: Option<&Path>,
) -> io::Result<CertifiedDomainPublication> {
    let gridnum_perdegree = crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(Path::new(
        config.landtype_file.trim(),
    ))?;
    let mode_grid = config.mode_grid.trim();
    let mesh_type = config.mesh_type.trim();
    if let (Some(region @ GridRegion::Close { .. }), "landmesh", "hex") =
        (domain_region, mesh_type, mode_grid)
    {
        return super::cmrc_land::publish_regional_land(
            source_gridfile,
            output_gridfile,
            Path::new(config.landtype_file.trim()),
            gridnum_perdegree,
            region,
            workdir,
        );
    }
    let clean_close = match (domain_region, mesh_type, mode_grid) {
        (Some(GridRegion::Close { points }), "oceanmesh", "tri") => Some(points.as_slice()),
        _ => None,
    };

    let (kept_cells, fvcom_2dm) = if let Some(close_points) = clean_close {
        let plan = write_clean_regional_ocean_gridfile(
            source_gridfile,
            close_points,
            Path::new(config.landtype_file.trim()),
            base_nxp,
            gridnum_perdegree,
            config.mask_sea_ratio,
            workdir,
        )?;
        fs::copy(&plan.result_gridfile, output_gridfile)?;
        let carved = crate::read_unstructured_mesh_netcdf(output_gridfile)?;
        let obc_order = match &plan.obc_output {
            Some(path) if path.exists() => read_obc_order_netcdf(path)?,
            _ => Vec::new(),
        };
        let fvcom = if let Some(output) = fvcom_output {
            Some(write_fvcom_2dm_from_carved(&carved, &obc_order, output)?)
        } else {
            None
        };
        (None, fvcom)
    } else if let (Some(region @ GridRegion::Close { .. }), "landmesh", "tri") =
        (domain_region, mesh_type, mode_grid)
    {
        fs::create_dir_all(workdir)?;
        let regional_gridfile = workdir.join("whole_regional_tri.nc4");
        crate::regional_gridfile_writers::write_regional_gridfile(
            source_gridfile,
            &regional_gridfile,
            region,
            "tri",
        )?;
        let kept = crate::write_landtype_masked_gridfile_with_refine_levels(
            &regional_gridfile,
            output_gridfile,
            &config.landtype_file,
            gridnum_perdegree,
            "tri",
            "landmesh",
            None,
            None,
            false,
            None,
        )?;
        (Some(kept), None)
    } else {
        let kept = crate::write_landtype_masked_gridfile_with_refine_levels(
            source_gridfile,
            output_gridfile,
            &config.landtype_file,
            gridnum_perdegree,
            mode_grid,
            mesh_type,
            None,
            None,
            config.isolated_ocean || mesh_type == "oceanmesh",
            None,
        )?;
        let fvcom = if let Some(output) = fvcom_output {
            let carved = crate::read_unstructured_mesh_netcdf(output_gridfile)?;
            Some(write_fvcom_2dm_from_carved(&carved, &[], output)?)
        } else {
            None
        };
        (Some(kept), fvcom)
    };

    let published = crate::read_unstructured_mesh_netcdf(output_gridfile)?;
    let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&published);
    if !topology.is_consistent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CMRC published domain topology failed: {}",
                topology.violations.join("; ")
            ),
        ));
    }
    let quality_mesh = crate::read_gridfile_mesh_points(output_gridfile)?;
    let quality_input = crate::grid_quality_pipeline::quality_input_from_gridfile(&quality_mesh)?;
    let quality_report = earthmesh_quality::compute(
        &quality_input,
        &earthmesh_quality::QualityThresholds::default(),
    );
    let published_minimum = quality_report.geometry.min_angle_deg;
    let published_maximum = quality_report.geometry.max_angle_deg;
    let delivery_window =
        earthmesh_refine_certified::AngleContract::for_id(angle_contract).final_delivery;
    if !published_minimum.is_finite()
        || !published_maximum.is_finite()
        || !delivery_window.contains_range(published_minimum, published_maximum)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CMRC published domain angle contract failed: [{published_minimum}, {published_maximum}] is outside [{}, {}]",
                delivery_window.minimum_degrees, delivery_window.maximum_degrees
            ),
        ));
    }
    let component_count = earthmesh_quality::topology::connected_component_count(&quality_input);
    let mut quality_issues =
        earthmesh_quality::topology::MeshTopologyValidator::new(&quality_input).validate_all();
    if mesh_type == "landmesh" {
        // As for land dual cells, retain islands (including one-cell islands)
        // and their diagnostics without relaxing winding or manifold checks.
        for issue in &mut quality_issues {
            if matches!(
                issue.issue_type,
                earthmesh_quality::topology::TopologyIssueType::DisconnectedMesh
                    | earthmesh_quality::topology::TopologyIssueType::OrphanCell
            ) {
                issue.severity = earthmesh_quality::topology::Severity::Warn;
            }
        }
    }
    let hard_issues = quality_issues
        .iter()
        .filter(|issue| issue.severity == earthmesh_quality::topology::Severity::Fail)
        .map(|issue| format!("{}: {}", issue.issue_type.as_str(), issue.message))
        .collect::<Vec<_>>();
    if !hard_issues.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CMRC published domain quality topology failed: {}",
                hard_issues.join("; ")
            ),
        ));
    }
    let quality_issue_json = quality_issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "type": issue.issue_type.as_str(),
                "severity": issue.severity.as_str(),
                "message": issue.message,
            })
        })
        .collect::<Vec<_>>();
    if fvcom_2dm
        .as_ref()
        .is_some_and(|report| report.boundary_segments == 0)
    {
        eprintln!("earthmesh_cli: FVCOM has no explicit open-boundary chains; model boundary classification and forcing must be supplied separately");
    }
    Ok(CertifiedDomainPublication {
        report: crate::unstructured_mesh_write_report_from_file(output_gridfile)?,
        kept_cells: kept_cells.unwrap_or(quality_report.geometry.cell_count),
        topology: serde_json::json!({
            "boundary_loops": topology.boundary_loop_count,
            "boundary_vertex_degree_violations": topology.boundary_vertex_degree_violation_count,
            "euler": topology.euler_characteristic,
            "expected_euler": topology.expected_euler_characteristic,
            "violations": topology.violations,
        }),
        quality_topology: (component_count, quality_issue_json),
        geometry: serde_json::json!({
            "cell_view": "tri",
            "cells": quality_report.geometry.cell_count,
            "minimum_angle_deg": published_minimum,
            "maximum_angle_deg": published_maximum,
            "contract_minimum_deg": delivery_window.minimum_degrees,
            "contract_maximum_deg": delivery_window.maximum_degrees,
            "contract_pass": true,
        }),
        fvcom_2dm,
    })
}

fn validate_local_update_mode(
    config: &EarthmeshConfig,
    options: &CertifiedRunOptions,
    required_levels: &[usize],
) -> io::Result<()> {
    let maximum = required_levels.iter().copied().max().unwrap_or(0);
    if !config.refine
        || !config.mask_domain_global
        || !matches!(config.mesh_type.trim(), "atmos" | "atmosmesh")
        || config.mode_grid.trim() != "hex"
        || !config.output_format.trim().eq_ignore_ascii_case("MPAS")
        || options.mode != CertifiedMode::ReverseCoarsening
        || options.delivery != CertifiedDelivery::Coupled
        || options.angle_contract.as_str() != "domain_quality_38_to_82_v1"
        || maximum == 0
        || maximum > 2
        || !required_levels.iter().any(|&level| level < maximum)
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "local updates require global coupled atmosmesh/hex MPAS mixed reverse coarsening with the 38..82 angle contract"));
    }
    Ok(())
}

fn run_certified_pipeline(
    contents: &str,
    config: &EarthmeshConfig,
    options: CertifiedRunOptions,
    workdir: &Path,
    max_tris: usize,
) -> io::Result<RefinePipelineRunReport> {
    let started = Instant::now();
    let timing_enabled = cmrc_timing_enabled();
    let mut phase_started = Instant::now();
    let options = if config.refine {
        options
    } else {
        CertifiedRunOptions {
            mode: CertifiedMode::SafeMotherOnly,
            ..options
        }
    };
    let regional_domain = (!config.mask_domain_global)
        .then(|| read_method_c_domain_region(config))
        .transpose()?
        .flatten();
    let is_domain_export = matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh");
    let regional_land = config.mesh_type.trim() == "landmesh"
        && matches!(config.mode_grid.trim(), "hex" | "tri")
        && matches!(regional_domain, Some(GridRegion::Close { .. }));
    if regional_domain.is_some()
        && !regional_land
        && !matches!(
            (
                config.mesh_type.trim(),
                config.mode_grid.trim(),
                regional_domain.as_ref()
            ),
            ("oceanmesh", "tri", Some(GridRegion::Close { .. }))
        )
    {
        return Err(io::Error::new(io::ErrorKind::Unsupported,
            "CMRC regional publication supports oceanmesh/tri or landmesh/{hex,tri} with a single close polygon only"));
    }
    if is_domain_export
        && !(crate::namelist_sets_landtype_file(contents)
            && crate::landtype_file_is_real(&config.landtype_file))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC certified land/ocean publication requires a real NL%landtype_file",
        ));
    }
    let requested_view = config.mode_grid.trim();
    if !matches!(requested_view, "tri" | "hex") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC mode_grid must be tri or hex",
        ));
    }
    if matches!(
        (options.delivery, requested_view),
        (CertifiedDelivery::Tri, "hex") | (CertifiedDelivery::Hex, "tri")
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC delivery must match mode_grid unless delivery='coupled'",
        ));
    }
    let refine =
        if config.refine && crate::namelist_reader::namelist_has_section(contents, "mkrefine") {
            RefineConfig::from_mkrefine_namelist_with_external_field(
                contents,
                config.mesh_type.trim(),
                requested_view,
                crate::hfield_refine::read_hfield_refine_options(contents)?.is_some(),
            )
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?
        } else {
            RefineConfig::default()
        };
    let specified_level = if refine.refine_spc {
        usize::try_from(refine.max_iter_spc).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "CMRC max_iter_spc must be non-negative",
            )
        })?
    } else {
        0
    };
    let calculated_level = if refine.refine_cal {
        usize::try_from(refine.max_iter_cal).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "CMRC max_iter_cal must be non-negative",
            )
        })?
    } else {
        0
    };
    let base_nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "CMRC NXP must be positive"))?;
    if base_nxp == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC NXP must be positive",
        ));
    }
    validate_certified_mother_family(
        base_nxp,
        if config.refine {
            options.maximum_level
        } else {
            0
        },
    )?;
    let requirements = if config.refine {
        certified_requirement_plan(
            contents,
            config,
            &refine,
            base_nxp,
            specified_level,
            calculated_level,
        )?
    } else {
        CertifiedRequirementPlan::uniform()
    };
    let requirement_nlon = requirements.nlon;
    let requirement_nlat = requirements.nlat;
    let required_levels = &requirements.effective_levels;
    let chosen_level = required_levels.iter().copied().max().unwrap_or(0);
    if chosen_level > options.maximum_level {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "CMRC MaximumLevelReached: requested level {chosen_level}, maximum {}",
                options.maximum_level
            ),
        ));
    }
    let raster_requirements = earthmesh_refine_certified::RasterLevelField::new(
        requirement_nlon,
        requirement_nlat,
        required_levels.clone(),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let local_update_path = std::env::var_os("EARTHMESH_CMRC_LOCAL_UPDATE").map(PathBuf::from);
    if let Some(path) = &local_update_path {
        if !path.is_absolute() || !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "local update request must be an existing absolute file",
            ));
        }
        validate_local_update_mode(config, &options, required_levels)?;
        if [
            "EARTHMESH_CMRC_SELECT",
            "EARTHMESH_CMRC_TRIM",
            "EARTHMESH_CMRC_CHECKPOINT",
        ]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "local updates cannot be combined with another experimental CMRC mode",
            ));
        }
    }
    log_cmrc_phase(timing_enabled, "requirement_planning", &mut phase_started);
    let CertifiedConstruction {
        geometry,
        pentagons,
        remap,
        remap_certificate,
        final_cell_requirements,
        delivered_level,
        delivered_levels,
        coarsening_strategy,
        initial_subdivision,
        final_subdivision: subdivision,
        initial_cells,
        attempted_patches,
        accepted_patches,
        removed_vertices,
        removed_faces,
        search_budget_exhausted,
        components_total,
        components_committed,
        components_promoted,
        components_exhausted,
        search_complete,
        elastic_report,
        local_update,
    } = build_certified_construction(
        base_nxp,
        chosen_level,
        &options,
        &raster_requirements,
        max_tris,
        local_update_path.as_deref(),
    )?;
    log_cmrc_phase(timing_enabled, "certified_construction", &mut phase_started);
    let delivered_levels = earthmesh_refine_certified::TargetLevelField::from_active_voronoi_cells(
        geometry.primal(),
        delivered_levels,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let final_cell_requirements = match final_cell_requirements {
        Some(requirements) => requirements,
        None => {
            earthmesh_refine_certified::certify_final_cell_requirements_from_raster_global_bound(
                &raster_requirements,
                geometry.primal(),
                &delivered_levels,
                1,
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CMRC final-cell certification failed: {error}"),
                )
            })?
        }
    };
    let evidence = earthmesh_refine_certified::FinalCertificationEvidence::from_final_cells(
        &final_cell_requirements,
        remap_certificate,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CMRC remap certification failed: {error}"),
        )
    })?;
    let final_mesh =
        earthmesh_refine_certified::finalize_geometry_certified_mother(*geometry, evidence)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CMRC final certification failed: {error}"),
                )
            })?;
    let fulfillment = earthmesh_refine_certified::AdaptivityFulfillmentReport::from_levels(
        required_levels.iter().copied(),
        delivered_levels.levels().iter().copied(),
        initial_cells,
        final_mesh.primal().triangle_count(),
        components_total,
        components_committed,
        components_promoted,
        components_exhausted,
        search_complete,
    );
    let (final_mesh, fulfillment, product_outcome, fallback_reason, safe_fallback) =
        match earthmesh_refine_certified::classify_adaptivity_delivery(
            final_mesh,
            fulfillment,
            options.mode == CertifiedMode::SafeMotherOnly,
        ) {
            earthmesh_refine_certified::CertifiedMeshOutcome::CertifiedAdaptive {
                mesh,
                fulfillment,
            } => (mesh, fulfillment, "certified_adaptive", None, false),
            earthmesh_refine_certified::CertifiedMeshOutcome::CertifiedSafeFallback {
                mesh,
                fulfillment,
                reason,
            } => (
                mesh,
                fulfillment,
                "certified_safe_fallback",
                Some(reason.to_string()),
                true,
            ),
            incomplete @ earthmesh_refine_certified::CertifiedMeshOutcome::CompressionIncomplete {
                ..
            } => {
                let error = certified_outcome_error(incomplete);
                let component = elastic_report.as_ref().map_or_else(String::new, |report| {
                    let start = report.components.len().saturating_sub(8);
                    let records = report.components[start..]
                        .iter()
                        .map(|component| {
                            format!(
                                "id:{} {}->{} outcome:{} topology_states:{} elastic_iterations:{} halo_width:{} reason:{}",
                                component.component_id,
                                component.source_level,
                                component.target_level,
                                elastic_outcome_name(component.outcome),
                                component.topology_states,
                                component.elastic_iterations,
                                component.transition_ring_width,
                                component.reason.as_deref().unwrap_or("none")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | ");
                    format!("; recent_components=[{records}]")
                });
                return Err(io::Error::new(error.kind(), format!("{error}{component}")));
            }
            other => return Err(certified_outcome_error(other)),
        };
    let triangular = final_mesh.primal().to_triangular_mesh(pentagons, None)?;
    let state = spherical_voronoi_state(&triangular)?;
    let output_mesh = build_certified_cmrc_gridfile(final_mesh.primal(), &state)?;
    log_cmrc_phase(
        timing_enabled,
        "final_certification_and_dual",
        &mut phase_started,
    );

    let configured_dir = PathBuf::from(config.file_dir());
    let file_dir = if configured_dir.is_absolute() {
        configured_dir
    } else {
        workdir.join(configured_dir)
    };
    let result_dir = file_dir.join("result");
    fs::create_dir_all(&result_dir)?;
    let gridfile_suffix = if safe_fallback {
        "_certified_safe_fallback"
    } else {
        ""
    };
    let domain_suffix = if is_domain_export {
        format!("_{}", config.mesh_type.trim())
    } else {
        String::new()
    };
    let artifact_stem = if safe_fallback {
        "certified_safe_fallback"
    } else {
        "certified"
    };
    let output_path = result_dir.join(format!(
        "gridfile_NXP{base_nxp:04}_{}{domain_suffix}{gridfile_suffix}.nc4",
        config.mode_grid.trim(),
    ));
    let temporary_path = result_dir.join(format!(
        ".{}.cmrc-tmp-{}",
        output_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("gridfile.nc4"),
        std::process::id()
    ));
    let global_parent_path = is_domain_export.then(|| {
        output_path.with_file_name(format!(
            "{}_global_parent.nc4",
            output_path.file_stem().unwrap().to_string_lossy()
        ))
    });
    let temporary_source_path = result_dir.join(format!(
        ".certified_source_grid.cmrc-tmp-{}",
        std::process::id()
    ));
    let remap_role = if is_domain_export {
        "pre_export_remap"
    } else {
        "remap"
    };
    let remap_path = result_dir.join(format!("{artifact_stem}_{remap_role}.csv"));
    let obsolete_remap_path = result_dir.join(format!(
        "{artifact_stem}_{}.csv",
        if is_domain_export {
            "remap"
        } else {
            "pre_export_remap"
        }
    ));
    let temporary_remap_path = result_dir.join(format!(
        ".certified_remap.csv.cmrc-tmp-{}",
        std::process::id()
    ));
    let certificate_path = result_dir.join(format!("{artifact_stem}_certificate.json"));
    let temporary_certificate_path = result_dir.join(format!(
        ".certified_certificate.json.cmrc-tmp-{}",
        std::process::id()
    ));
    let manifest_path = result_dir.join(format!("{artifact_stem}_manifest.json"));
    let temporary_manifest_path = result_dir.join(format!(
        ".certified_manifest.json.cmrc-tmp-{}",
        std::process::id()
    ));
    let resources_path = result_dir.join(format!("{artifact_stem}_resources.json"));
    let temporary_resources_path = result_dir.join(format!(
        ".certified_resources.json.cmrc-tmp-{}",
        std::process::id()
    ));
    let ready_marker = result_dir.join(format!("{artifact_stem}_ready"));
    let temporary_ready_marker =
        result_dir.join(format!(".certified_ready.cmrc-tmp-{}", std::process::id()));
    let certificate = final_mesh.certificate();
    let geometry_report = &certificate.geometry;
    let mode_name = match options.mode {
        CertifiedMode::SafeMotherOnly => "safe_mother_only",
        CertifiedMode::ReverseCoarsening => "reverse_coarsening",
    };
    let physical_balance_scope = if coarsening_strategy == "elastic_component_epochs" {
        "final_voronoi_cells_exact_raster_overlap"
    } else {
        "final_voronoi_cells_global_raster_max_bound"
    };
    let requirement_layers =
        requirements.layer_report(elastic_report.as_ref(), options.gradation_rings_per_level);
    let elastic_report_json = elastic_report.as_ref().map(elastic_report_json);
    let mut certificate_document = serde_json::json!({
        "backend": "certified",
        "angle_contract": options.angle_contract.as_str(),
        "dqx_execution_status": if options.angle_contract.as_str() == "domain_quality_38_to_82_v1" { "geometry_contract_only" } else { "not_applicable" },
        "mode": mode_name,
        "product_outcome": product_outcome,
        "safe_fallback_reason": fallback_reason,
        "coarsening_strategy": coarsening_strategy,
        "geometry_scope": if is_domain_export { "pre_export_closed_sphere" } else { "published_grid" },
        "published_grid_is_certified_face_subset": is_domain_export && requested_view == "tri",
        "requirement_balance_scope": physical_balance_scope,
        "physical_balance_scope": physical_balance_scope,
        "remap_cells": "voronoi",
        "remap_scope": if is_domain_export { "pre_export_closed_sphere_voronoi" } else { "published_grid_voronoi" },
        "published_grid_remap_available": !is_domain_export,
        "published_grid_refinement_metadata": true,
        "chosen_level": chosen_level,
        "delivered_level": delivered_level,
        "delivered_level_min": delivered_levels.levels().iter().copied().min().unwrap_or(0),
        "delivered_level_max": delivered_levels.levels().iter().copied().max().unwrap_or(0),
        "requirement_samples": required_levels.len(),
        "requirement_grid": { "nlon": requirement_nlon, "nlat": requirement_nlat },
        "requirement_max_level": chosen_level,
        "initial_mother_subdivision": initial_subdivision,
        "mother_subdivision": subdivision,
        "initial_mother_cells": initial_cells,
        "coarsening": {
            "attempted_patches": attempted_patches,
            "accepted_patches": accepted_patches,
            "removed_vertices": removed_vertices,
            "removed_faces": removed_faces,
            "search_budget_exhausted": search_budget_exhausted,
            "components_total": fulfillment.components_total,
            "components_committed": fulfillment.components_committed,
            "components_promoted": fulfillment.components_promoted,
            "components_exhausted": fulfillment.components_exhausted,
            "search_complete": fulfillment.search_complete,
        },
        "adaptivity_fulfillment": {
            "requested_level_min": fulfillment.requested_level_min,
            "requested_level_max": fulfillment.requested_level_max,
            "requested_histogram": fulfillment.requested_histogram,
            "delivered_level_min": fulfillment.delivered_level_min,
            "delivered_level_max": fulfillment.delivered_level_max,
            "delivered_histogram": fulfillment.delivered_histogram,
            "mixed_levels_requested": fulfillment.mixed_levels_requested,
            "mixed_levels_delivered": fulfillment.mixed_levels_delivered,
            "initial_faces": fulfillment.initial_faces,
            "final_faces": fulfillment.final_faces,
            "compression_ratio": fulfillment.compression_ratio,
        },
        "geometry": {
            "vertices": geometry_report.vertices,
            "edges": geometry_report.edges,
            "faces": geometry_report.faces,
            "minimum_angle_deg": geometry_report.min_angle_degrees,
            "maximum_angle_deg": geometry_report.max_angle_degrees,
            "open_edges": geometry_report.open_edges,
            "topology_errors": geometry_report.topology_errors,
            "degree_outside_window": geometry_report.degree_outside_window,
            "euler": geometry_report.euler,
            "charge": geometry_report.charge,
            "delaunay_violations": geometry_report.delaunay_violations,
            "voronoi_invalid_cells": geometry_report.voronoi_invalid_cells,
            "primal_dual_errors": geometry_report.voronoi_reciprocal_errors,
        },
        "physical_residuals": certificate.physical_residuals,
        "balance_residuals": certificate.balance_residuals,
        "remap_closure_errors": certificate.remap_closure_errors,
        "elastic_component_epochs": elastic_report_json,
    });
    // Only the regional land adapter proves whole dual-cell lineage. Legacy
    // global hex domain exports have not passed that audit; do not certify them.
    certificate_document["published_grid_is_certified_dual_cell_subset"] =
        if is_domain_export && requested_view == "hex" && !regional_land {
            serde_json::Value::Null
        } else {
            regional_land.into()
        };
    certificate_document["requirement_layers"] = requirement_layers.clone();
    certificate_document["published_grid_lineage_scope"] =
        serde_json::Value::from(if is_domain_export {
            "pre_export_closed_sphere_canonical_ids"
        } else {
            "not_emitted"
        });
    let fvcom_output_path = (is_domain_export
        && config.mesh_type.trim() == "oceanmesh"
        && config.mode_grid.trim() == "tri"
        && config.output_format.trim().eq_ignore_ascii_case("FVCOM"))
    .then(|| fvcom_mesh_2dm_output_path(&file_dir));
    let temporary_fvcom_path =
        result_dir.join(format!(".fvcom.2dm.cmrc-tmp-{}", std::process::id()));
    let mpas_suffix = if safe_fallback {
        "_certified_safe_fallback"
    } else {
        ""
    };
    let mpas_output_paths = (!is_domain_export
        && matches!(config.mesh_type.trim(), "atmos" | "atmosmesh")
        && config.mode_grid.trim() == "hex"
        && config.output_format.trim().eq_ignore_ascii_case("MPAS"))
    .then(|| {
        (
            result_dir.join(format!("MPASOUT_NXP{base_nxp:04}_global{mpas_suffix}.nc4")),
            result_dir.join(format!(
                "MPASOUT_NXP{base_nxp:04}_global{mpas_suffix}.graph.info"
            )),
        )
    });
    let temporary_mpas_path = result_dir.join(format!(
        ".MPASOUT_NXP{base_nxp:04}_global.nc4.cmrc-tmp-{}",
        std::process::id()
    ));
    let temporary_mpas_graph_path = result_dir.join(format!(
        ".MPASOUT_NXP{base_nxp:04}_global.graph.info.cmrc-tmp-{}",
        std::process::id()
    ));
    let mut manifest = serde_json::json!({
        "backend": "certified",
        "angle_contract": options.angle_contract.as_str(),
        "dqx_execution_status": if options.angle_contract.as_str() == "domain_quality_38_to_82_v1" { "geometry_contract_only" } else { "not_applicable" },
        "mode": mode_name,
        "product_outcome": product_outcome,
        "safe_fallback_reason": fallback_reason,
        "geometry_scope": if is_domain_export { "pre_export_closed_sphere" } else { "published_grid" },
        "delivery": match options.delivery {
            CertifiedDelivery::Tri => "tri",
            CertifiedDelivery::Hex => "hex",
            CertifiedDelivery::Coupled => "coupled",
        },
        "gridfile": output_path.display().to_string(),
        "global_parent_gridfile": global_parent_path.as_ref().map(|path| path.display().to_string()),
        "remap": if is_domain_export { serde_json::Value::Null } else { serde_json::Value::String(remap_path.display().to_string()) },
        "pre_export_remap": if is_domain_export { serde_json::Value::String(remap_path.display().to_string()) } else { serde_json::Value::Null },
        "fvcom_2dm": fvcom_output_path.as_ref().map(|path| path.display().to_string()),
        "remap_scope": if is_domain_export { "pre_export_closed_sphere_voronoi" } else { "published_grid_voronoi" },
        "published_grid_remap_status": if is_domain_export { "not_available_after_landtype_subset" } else { "certified" },
        "certificate": certificate_path.display().to_string(),
        "resources": resources_path.display().to_string(),
        "ready": ready_marker.display().to_string(),
        "mpas": mpas_output_paths.as_ref().map(|(mesh, _)| mesh.display().to_string()),
        "mpas_graph_info": mpas_output_paths.as_ref().map(|(_, graph)| graph.display().to_string()),
        "mpas_sphere_radius": mpas_output_paths.as_ref().map(|_| 1.0),
    });
    if let Some(mut report) = local_update {
        report["full_delivery_recertified"] = serde_json::json!(true);
        report["angle_guard"] =
            serde_json::json!("global_extrema_and_preferred_distribution_not_per_element");
        manifest["experimental_local_update"] = report;
    }
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    let (m_refine_levels, w_refine_levels) =
        certified_gridfile_refine_levels(&output_mesh, delivered_levels.levels())?;
    let temporary_paths = [
        temporary_path.as_path(),
        temporary_source_path.as_path(),
        temporary_remap_path.as_path(),
        temporary_certificate_path.as_path(),
        temporary_manifest_path.as_path(),
        temporary_resources_path.as_path(),
        temporary_ready_marker.as_path(),
        temporary_fvcom_path.as_path(),
        temporary_mpas_path.as_path(),
        temporary_mpas_graph_path.as_path(),
    ];
    for path in &temporary_paths {
        let _ = fs::remove_file(path);
    }
    log_cmrc_phase(timing_enabled, "artifact_assembly", &mut phase_started);
    let mut raw_output = None;
    let staged = (|| -> io::Result<(crate::UnstructuredMeshWriteReport, Option<usize>)> {
        {
            let mut writer = BufWriter::new(fs::File::create(&temporary_remap_path)?);
            write_remap_csv(&mut writer, remap.rows())?;
            writer.flush()?;
        }
        log_cmrc_phase(timing_enabled, "remap_csv", &mut phase_started);
        fs::write(&temporary_manifest_path, manifest_json)?;
        fs::write(&temporary_ready_marker, format!("{product_outcome}\n"))?;
        let mpas_context = crate::mpas_gridfile_context::MpasGridfileContext::from_producer(
            &output_mesh,
            certified_mpas_cellwidth(base_nxp, &w_refine_levels)?,
            base_nxp,
            delivered_level.checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "CMRC MPAS step overflow")
            })?,
            "cmrc_delivered_w_levels",
        )?;
        let (
            report,
            landtype_masked_cells,
            topology,
            domain_quality,
            published_geometry,
            fvcom_2dm,
        ) = if is_domain_export {
            let (m_pre_export_lineage, w_pre_export_lineage) =
                certified_gridfile_pre_export_lineages(&output_mesh);
            let mut parent_report = crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
                &temporary_source_path,
                &output_mesh,
                MethodCGridfileMetadataSlices {
                    mpas: Some(&mpas_context),
                    m_lineage: Some(&m_pre_export_lineage),
                    w_lineage: Some(&w_pre_export_lineage),
                    m_refine_level: Some(&m_refine_levels),
                    w_refine_level: Some(&w_refine_levels),
                    ..Default::default()
                },
            )?;
            parent_report.output = global_parent_path.as_ref().unwrap().clone();
            raw_output = Some(parent_report);
            let domain_workdir =
                result_dir.join(format!(".certified_domain.cmrc-tmp-{}", std::process::id()));
            let published = publish_certified_domain_gridfile(
                &temporary_source_path,
                &temporary_path,
                config,
                base_nxp,
                &domain_workdir,
                regional_domain.as_ref(),
                options.angle_contract,
                fvcom_output_path
                    .as_ref()
                    .map(|_| temporary_fvcom_path.as_path()),
            );
            let _ = fs::remove_dir_all(&domain_workdir);
            let published = published?;
            (
                published.report,
                Some(published.kept_cells),
                Some(published.topology),
                Some(published.quality_topology),
                Some(published.geometry),
                published.fvcom_2dm,
            )
        } else {
            (
                crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
                    &temporary_path,
                    &output_mesh,
                    MethodCGridfileMetadataSlices {
                        mpas: Some(&mpas_context),
                        m_refine_level: Some(&m_refine_levels),
                        w_refine_level: Some(&w_refine_levels),
                        ..Default::default()
                    },
                )?,
                None,
                None,
                None,
                None,
                None,
            )
        };
        let mpas = if mpas_output_paths.is_some() {
            Some(publish_certified_atmos_mpas(
                &output_mesh,
                &mpas_context,
                &temporary_mpas_path,
                &temporary_mpas_graph_path,
            )?)
        } else {
            None
        };
        log_cmrc_phase(
            timing_enabled,
            "domain_export_and_audit",
            &mut phase_started,
        );
        if let Some(published_geometry) = &published_geometry {
            certificate_document
                .as_object_mut()
                .expect("CMRC certificate document is an object")
                .insert(
                    "published_domain_geometry".to_string(),
                    published_geometry.clone(),
                );
        }
        fs::write(
            &temporary_certificate_path,
            serde_json::to_vec_pretty(&certificate_document).map_err(io::Error::other)?,
        )?;
        let resource_json = serde_json::to_vec_pretty(&serde_json::json!({
            "certification_elapsed_ms": started.elapsed().as_millis(),
            "requirement_layers": requirement_layers,
            "requirement_raster_cells": required_levels.len(),
            "target_voronoi_cells": geometry_report.voronoi_cells,
            "remap_rows": remap.rows().len(),
            "remap_entries": remap.rows().iter().map(|row| row.sources.len()).sum::<usize>(),
            "artifact_bytes": {
                "gridfile": fs::metadata(&temporary_path)?.len(),
                "remap": fs::metadata(&temporary_remap_path)?.len(),
                "certificate": fs::metadata(&temporary_certificate_path)?.len(),
                "manifest": fs::metadata(&temporary_manifest_path)?.len(),
                "fvcom_2dm": if temporary_fvcom_path.exists() { serde_json::Value::from(fs::metadata(&temporary_fvcom_path)?.len()) } else { serde_json::Value::Null },
                "mpas": if temporary_mpas_path.exists() { serde_json::Value::from(fs::metadata(&temporary_mpas_path)?.len()) } else { serde_json::Value::Null },
                "mpas_graph_info": if temporary_mpas_graph_path.exists() { serde_json::Value::from(fs::metadata(&temporary_mpas_graph_path)?.len()) } else { serde_json::Value::Null },
            },
            "peak_memory_bytes": serde_json::Value::Null,
            "peak_memory_measurement": "external acceptance harness required",
            "landtype_masked_cells": landtype_masked_cells,
            "landtype_kept_cells": landtype_masked_cells,
            "remap_scope": if is_domain_export { "pre_export_closed_sphere_voronoi" } else { "published_grid_voronoi" },
            "published_grid_remap_available": !is_domain_export,
            "published_domain_topology": topology,
            "published_domain_quality_topology": domain_quality.as_ref().map(|(component_count, issues)| serde_json::json!({
                "connected_components": component_count,
                "issues": issues,
            })),
            "published_domain_geometry": published_geometry,
            "fvcom_2dm": fvcom_2dm.as_ref().map(|report| {
                let output = fvcom_output_path.as_ref().unwrap_or(&report.output);
                serde_json::json!({
                    "output": output.display().to_string(),
                    "triangles": report.triangles,
                    "nodes": report.nodes,
                    "boundary_segments": report.boundary_segments,
                })
            }),
            "mpas": mpas.as_ref().map(|report| {
                let (mesh, graph) = mpas_output_paths.as_ref().expect("MPAS paths");
                serde_json::json!({
                    "mesh": mesh.display().to_string(),
                    "graph_info": graph.display().to_string(),
                    "n_cells": report.report.mesh.n_cells,
                    "n_vertices": report.report.mesh.n_vertices,
                    "n_edges": report.report.mesh.n_edges,
                    "graph_interior_edges": report.report.graph_info.interior_edges,
                    "mesh_density_min": report.mesh_density_min,
                    "mesh_density_max": report.mesh_density_max,
                    "base_nxp": base_nxp,
                    "step": report.step,
                    "sphere_radius": 1.0,
                    "unit_convention": "unit_sphere",
                })
            }),
            "elastic_component_epochs": elastic_report_json,
        }))
        .map_err(io::Error::other)?;
        fs::write(&temporary_resources_path, resource_json)?;
        log_cmrc_phase(timing_enabled, "artifact_staging", &mut phase_started);
        Ok((report, landtype_masked_cells))
    })();
    let (temporary, landtype_masked_cells) = match staged {
        Ok(report) => report,
        Err(error) => {
            for path in &temporary_paths {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
    };
    let mut publications = vec![
        (
            temporary_certificate_path.as_path(),
            certificate_path.as_path(),
        ),
        (temporary_remap_path.as_path(), remap_path.as_path()),
        (temporary_path.as_path(), output_path.as_path()),
        (temporary_resources_path.as_path(), resources_path.as_path()),
        (temporary_manifest_path.as_path(), manifest_path.as_path()),
    ];
    if let Some(parent) = &global_parent_path {
        publications.push((temporary_source_path.as_path(), parent.as_path()));
    }
    if let Some(fvcom_path) = &fvcom_output_path {
        publications.push((temporary_fvcom_path.as_path(), fvcom_path.as_path()));
    }
    if let Some((mpas_path, graph_path)) = &mpas_output_paths {
        publications.push((temporary_mpas_path.as_path(), mpas_path.as_path()));
        publications.push((temporary_mpas_graph_path.as_path(), graph_path.as_path()));
    }
    publications.push((temporary_ready_marker.as_path(), ready_marker.as_path()));
    if let Err(error) = publish_artifacts(&publications, &[&obsolete_remap_path]) {
        for path in &temporary_paths {
            let _ = fs::remove_file(path);
        }
        return Err(io::Error::new(
            error.kind(),
            format!("CMRC atomic artifact publication failed: {error}"),
        ));
    }
    log_cmrc_phase(timing_enabled, "artifact_publish", &mut phase_started);
    let output = crate::UnstructuredMeshWriteReport {
        output: output_path.clone(),
        sjx_points: temporary.sjx_points,
        lbx_points: temporary.lbx_points,
        dimc: temporary.dimc,
    };

    let mut runtime_state =
        EarthmeshRuntimeState::new(config.clone()).with_refine_config(refine.clone());
    runtime_state.grid = state.grid;
    runtime_state.ijtabs = state.tabs;
    runtime_state
        .record_pentagon_indices_from_icosahedron(pentagons)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    runtime_state
        .record_mesh_counts_for_step(
            delivered_level + 1,
            runtime_state.grid.nma,
            runtime_state.grid.nwa,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(RefinePipelineRunReport {
        gridinit: None,
        refine,
        regions: requirements.regions,
        max_level: chosen_level,
        realized_max_level: delivered_level,
        finest_cell_km: 0.0,
        coarsest_cell_km: 0.0,
        realized_region_halvings: 0.0,
        hfield_diagnostics: Default::default(),
        transition_faces: 0,
        spring_nest_passes: 0,
        certified_run: Some(CertifiedRunRecord {
            mode: mode_name.to_string(),
            product_outcome: product_outcome.to_string(),
            safe_fallback_reason: fallback_reason,
            fulfillment: *fulfillment,
            chosen_level,
            delivered_level,
            initial_mother_subdivision: initial_subdivision,
            mother_subdivision: subdivision,
            initial_mother_cells: initial_cells,
            mother_cells: geometry_report.faces,
            attempted_patches,
            accepted_patches,
            removed_vertices,
            removed_faces,
            search_budget_exhausted,
            minimum_angle_deg: geometry_report.min_angle_degrees,
            maximum_angle_deg: geometry_report.max_angle_degrees,
            physical_residuals: certificate.physical_residuals,
            balance_residuals: certificate.balance_residuals,
            topology_errors: geometry_report.topology_errors,
            dual_errors: geometry_report.voronoi_invalid_cells
                + geometry_report.voronoi_reciprocal_errors,
            remap_closure_errors: certificate.remap_closure_errors,
            remap: (!is_domain_export).then(|| remap_path.clone()),
            pre_export_remap: is_domain_export.then_some(remap_path),
            certificate: certificate_path,
            manifest: manifest_path,
            resources: resources_path,
            ready_marker,
        }),
        lepp_adaptive_hybrid: None,
        lepp_post_quality: None,
        spring_nest_iterations: 0,
        raw_output,
        landtype_masked_cells,
        coupled_outputs: None,
        output,
        runtime_state,
    })
}

fn build_certified_cmrc_gridfile(
    primal: &MeshState,
    state: &earthmesh_mesh::VoronoiGridState,
) -> io::Result<crate::UnstructuredMesh> {
    let mut output = gridfile_mesh_from_one_based_state(&state.grid, &state.tabs)?;
    let mut site_remap = vec![0usize; primal.vertices().len()];
    for (compact, site) in primal.active_vertex_slots().enumerate() {
        site_remap[site] = compact + earthmesh_mesh::MESH_STATE_FIRST_ID;
    }
    let mut face_remap = vec![0usize; primal.triangles().len()];
    let mut seed_by_site = vec![None; primal.vertices().len()];
    for (compact, face) in primal.active_triangle_slots().enumerate() {
        face_remap[face] = compact + earthmesh_mesh::MESH_STATE_FIRST_ID;
        for site in primal.triangles()[face] {
            seed_by_site[site].get_or_insert(face);
        }
    }
    let same_direction = |left: earthmesh_mesh::CartesianPoint,
                          right: earthmesh_mesh::CartesianPoint| {
        let left_norm = earthmesh_mesh::magnitude(left);
        let right_norm = earthmesh_mesh::magnitude(right);
        left_norm > 0.0
            && right_norm > 0.0
            && ((left.x / left_norm - right.x / right_norm).powi(2)
                + (left.y / left_norm - right.y / right_norm).powi(2)
                + (left.z / left_norm - right.z / right_norm).powi(2))
            .sqrt()
                <= 1.0e-12
    };
    for site in primal.active_vertex_slots() {
        let published_site = site_remap[site];
        if published_site > state.grid.nwa || published_site >= state.tabs.w.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published dual is missing primal site {site}"),
            ));
        }
        let published = earthmesh_mesh::CartesianPoint::new(
            state.grid.xew[published_site],
            state.grid.yew[published_site],
            state.grid.zew[published_site],
        );
        if !same_direction(primal.vertices()[site], published) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published site {site} differs from the certified primal"),
            ));
        }
        let published_faces = state.tabs.w[published_site]
            .im
            .iter()
            .copied()
            .filter(|&face| face >= 2)
            .map(|face| face as usize)
            .collect::<std::collections::BTreeSet<_>>();
        if published_faces.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published Voronoi cell {site} has no incident face"),
            ));
        }
        let seed = seed_by_site[site].ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC certified primal site {site} has no incident face"),
            )
        })?;
        let cell = primal
            .voronoi_cell_from(site, seed)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let certified_faces = cell
            .triangles
            .iter()
            .map(|&face| face_remap[face])
            .collect::<std::collections::BTreeSet<_>>();
        if published_faces != certified_faces {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published Voronoi cell {site} has incident faces {published_faces:?}, certified {certified_faces:?}"),
            ));
        }
        // The generic state carries incident-face adjacency; native W cells
        // require the certified cyclic fan, not just the same unordered set.
        // This conversion emits one placeholder row, so canonical id maps to id-1.
        let row = published_site - 1;
        if output.n_w_to_m[row] as usize != cell.degree()
            || cell.degree() > output.w_to_m[row].len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published Voronoi cell {site} has inconsistent degree"),
            ));
        }
        output.w_to_m[row].fill(1);
        for (slot, face) in cell.triangles.iter().enumerate() {
            output.w_to_m[row][slot] = i32::try_from(face_remap[*face]).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "CMRC face id exceeds i32")
            })?;
        }
    }
    for face in primal.active_triangle_slots() {
        let published_face = face_remap[face];
        if published_face > state.grid.nma || published_face >= state.tabs.m.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published dual is missing primal face {face}"),
            ));
        }
        let expected = primal
            .circumcentre(face)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let published = earthmesh_mesh::CartesianPoint::new(
            state.grid.xem[published_face],
            state.grid.yem[published_face],
            state.grid.zem[published_face],
        );
        if !same_direction(expected, published) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CMRC published dual vertex {face} differs from the certified circumcentre"
                ),
            ));
        }
        let published_sites = state.tabs.m[published_face]
            .iw
            .iter()
            .copied()
            .filter(|&site| site >= 2)
            .map(|site| site as usize)
            .collect::<std::collections::BTreeSet<_>>();
        if published_sites
            != primal.triangles()[face]
                .map(|site| site_remap[site])
                .into_iter()
                .collect()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC published dual vertex {face} has different primal sites"),
            ));
        }
    }
    Ok(output)
}

/// Separate source provenance from the effective raster that is still hard-certified.
struct CertifiedRequirementPlan {
    regions: Vec<RefinementRegion>,
    nlon: usize,
    nlat: usize,
    effective_levels: Vec<usize>,
    // None for threshold/hydro sources: partial raw provenance would be misleading.
    raw_region_levels: Option<Vec<usize>>,
    threshold_provenance: Option<serde_json::Value>,
    conservative_global_bound: bool,
}

impl CertifiedRequirementPlan {
    fn uniform() -> Self {
        Self {
            regions: Vec::new(),
            nlon: 4,
            nlat: 2,
            effective_levels: vec![0; 8],
            raw_region_levels: Some(vec![0; 8]),
            threshold_provenance: None,
            conservative_global_bound: false,
        }
    }

    fn layer_report(
        &self,
        elastic: Option<&earthmesh_refine_certified::coarsen::ElasticCmrcReport>,
        rings: usize,
    ) -> serde_json::Value {
        let histogram = |levels: &[usize]| {
            let mut counts = std::collections::BTreeMap::<usize, usize>::new();
            for &level in levels {
                *counts.entry(level).or_default() += 1;
            }
            counts
        };
        serde_json::json!({
            "policy": "effective_raster_remains_hard",
            "raw_source_raster": {
                "status": if self.raw_region_levels.is_some() { "available" } else { "unavailable_threshold_or_hydro" },
                "scope": "canonical_region_sample_centers_quantized_not_analytic_coverage",
                "histogram": self.raw_region_levels.as_deref().map(histogram),
            },
            "threshold_sources": self.threshold_provenance.as_ref(),
            "effective_source_raster": {
                "scope": "gradient_limited_composed_sources_with_conservative_bounds",
                "histogram": histogram(&self.effective_levels),
                "conservative_global_bound": self.conservative_global_bound,
                "raised_samples_over_raw": self.raw_region_levels.as_ref().map(|raw| {
                    raw.iter().zip(&self.effective_levels).filter(|(r, e)| e > r).count()
                }),
            },
            "raster_grid": { "nlon": self.nlon, "nlat": self.nlat },
            "graph_scheduling_target": {
                "status": if elastic.is_some() { "applied" } else { "not_applied" },
                "scope": "initial_mother_voronoi_cells_after_raster_overlap_and_graph_gradation",
                "histogram": elastic.map(|report| &report.requested_histogram),
                "gradation_rings_per_level": elastic.map(|_| rings),
            },
        })
    }
}

fn certified_requirement_plan(
    contents: &str,
    config: &EarthmeshConfig,
    refine: &RefineConfig,
    base_nxp: usize,
    specified_level: usize,
    calculated_level: usize,
) -> io::Result<CertifiedRequirementPlan> {
    if crate::adaptive_refine::read_adaptive_refine_options(contents)?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "CMRC CriterionNotCertifiable: &adaptive demand has no final-cell certificate",
        ));
    }
    if native_grid_refinement_requested(contents, config.mesh_type.trim())? {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "CMRC CriterionNotCertifiable: native ngrids/nsfcgrids are not certified requirement sources",
        ));
    }

    let configured_hfield = crate::hfield_refine::read_hfield_refine_options(contents)?;
    let hfield = configured_hfield.clone().unwrap_or_default();
    let hydro_level = configured_hfield
        .as_ref()
        .map(crate::hydro_refinement_adapter::hydro_target_max_level)
        .transpose()?
        .unwrap_or(0);
    let mesh_type = config.mesh_type.trim();
    let has_threshold_sources =
        refine.refine_cal && crate::hfield_refine::has_threshold_hfield_sources(refine, mesh_type);

    let specified_regions = if refine.refine_spc {
        read_method_c_specified_refinement_regions(refine, specified_level, base_nxp, false)?
    } else {
        Vec::new()
    };
    let calculated_region_prefix = refine.mask_refine_cal_fprefix.trim().trim_end_matches('/');
    let has_configured_calculated_regions =
        !matches!(calculated_region_prefix, "" | "/tmp" | "none");
    let calculated_regions =
        if refine.refine_cal && (!has_threshold_sources || has_configured_calculated_regions) {
            read_method_c_calculated_refinement_regions(
                refine,
                calculated_level,
                has_threshold_sources,
            )?
        } else {
            Vec::new()
        };
    if (refine.refine_spc || refine.refine_cal || hydro_level > 0)
        && specified_regions.is_empty()
        && calculated_regions.is_empty()
        && !has_threshold_sources
        && hydro_level == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "CMRC found no certifiable specified, calculated, threshold, or hydro requirement source",
        ));
    }

    let specified_present = !specified_regions.is_empty();
    let calculated_present = !calculated_regions.is_empty();
    let mut regions = specified_regions;
    regions.extend(calculated_regions);
    if regions.is_empty() && !has_threshold_sources && hydro_level == 0 {
        return Ok(CertifiedRequirementPlan::uniform());
    }

    let source_max_level = specified_level
        .max(calculated_level)
        .max(hydro_level)
        .max(1);
    let field_max_level = hfield.max_level.unwrap_or(source_max_level);
    let quantized_max_level = u8::try_from(field_max_level).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC h-field maximum level must fit u8",
        )
    })?;
    let base_m = hfield.base_m.unwrap_or_else(|| {
        2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * base_nxp as f64)
    });
    let (mut field, threshold_provenance) = if has_threshold_sources {
        let (field, report) = crate::hfield_refine::build_composed_hfield_with_report(
            &regions,
            refine,
            mesh_type,
            Some(config),
            base_m,
            &hfield,
            calculated_level.clamp(1, field_max_level),
            None,
        )?;
        (field, Some(report))
    } else {
        (
            crate::hfield_refine::build_composed_hfield(
                &regions,
                refine,
                mesh_type,
                Some(config),
                base_m,
                &hfield,
                calculated_level.clamp(1, field_max_level),
                None,
            )?,
            None,
        )
    };
    crate::hydro_refinement_adapter::apply_hydro_target_to_field(
        &mut field, &hfield, base_m, None,
    )?;
    let mut levels = field
        .level_map(base_m, quantized_max_level)?
        .into_iter()
        .map(usize::from)
        .collect::<Vec<_>>();
    // Do not mistake a region-only subset for the raw demand of a mixed-source run.
    let raw_region_levels = if !has_threshold_sources && hfield.hydro_target_paths().is_none() {
        Some(
            crate::hfield_refine::build_raw_region_hfield(
                &regions,
                base_m,
                field.nlon(),
                field.nlat(),
                None,
            )?
            .level_map(base_m, quantized_max_level)?
            .into_iter()
            .map(usize::from)
            .collect(),
        )
    } else {
        None
    };
    let mut conservative_global_bound = false;
    // A sub-raster specified region can fall between HField sample centers.
    // The safe-mother path is global, so retaining its declared level is the
    // conservative bound and costs no additional geometric machinery.
    if specified_present && levels.iter().copied().max().unwrap_or(0) < specified_level {
        levels.fill(specified_level);
        conservative_global_bound = true;
    }
    if calculated_present && levels.iter().copied().max().unwrap_or(0) < calculated_level {
        levels.fill(calculated_level);
        conservative_global_bound = true;
    }
    Ok(CertifiedRequirementPlan {
        regions,
        nlon: field.nlon(),
        nlat: field.nlat(),
        effective_levels: levels,
        raw_region_levels,
        threshold_provenance,
        conservative_global_bound,
    })
}

fn certified_outcome_error(outcome: earthmesh_refine_certified::CertifiedMeshOutcome) -> io::Error {
    use earthmesh_refine_certified::CertifiedMeshOutcome;
    let (kind, message) = match outcome {
        CertifiedMeshOutcome::CellBudgetInsufficient {
            required_cells,
            budget,
        } => (
            io::ErrorKind::OutOfMemory,
            format!(
                "CMRC CellBudgetInsufficient: requires {required_cells} cells, budget is {budget}"
            ),
        ),
        CertifiedMeshOutcome::MaximumLevelReached {
            requested_level,
            max_level,
        } => (
            io::ErrorKind::InvalidInput,
            format!("CMRC MaximumLevelReached: requested {requested_level}, maximum {max_level}"),
        ),
        CertifiedMeshOutcome::CriterionNotCertifiable { reason } => (
            io::ErrorKind::Unsupported,
            format!("CMRC CriterionNotCertifiable: {reason}"),
        ),
        CertifiedMeshOutcome::PhysicalCriterionUnsatisfiable { reason } => (
            io::ErrorKind::InvalidData,
            format!("CMRC PhysicalCriterionUnsatisfiable: {reason}"),
        ),
        CertifiedMeshOutcome::UnsupportedBoundaryConstraint { reason } => (
            io::ErrorKind::Unsupported,
            format!("CMRC UnsupportedBoundaryConstraint: {reason}"),
        ),
        CertifiedMeshOutcome::SearchBudgetExhausted { attempted_patches } => (
            io::ErrorKind::TimedOut,
            format!("CMRC SearchBudgetExhausted after {attempted_patches} patches"),
        ),
        CertifiedMeshOutcome::InternalCertificationFailure { reason } => (
            io::ErrorKind::InvalidData,
            format!("CMRC InternalCertificationFailure: {reason}"),
        ),
        CertifiedMeshOutcome::CompressionIncomplete {
            fulfillment,
            reason,
            ..
        } => (
            io::ErrorKind::Other,
            format!(
                "CMRC CompressionIncomplete: {reason}; requested={:?}; delivered={:?}; components committed/promoted/exhausted={}/{}/{}; search_complete={}",
                fulfillment.requested_histogram,
                fulfillment.delivered_histogram,
                fulfillment.components_committed,
                fulfillment.components_promoted,
                fulfillment.components_exhausted,
                fulfillment.search_complete,
            ),
        ),
        CertifiedMeshOutcome::GeometryCertified(_)
        | CertifiedMeshOutcome::Certified(_)
        | CertifiedMeshOutcome::CertifiedAdaptive { .. }
        | CertifiedMeshOutcome::CertifiedSafeFallback { .. } => (
            io::ErrorKind::InvalidData,
            "CMRC returned an unexpected success outcome".to_string(),
        ),
    };
    io::Error::new(kind, message)
}

fn certified_mother_pentagons(mesh: &MeshState) -> io::Result<[usize; 12]> {
    let mut degree = vec![0usize; mesh.vertices().len()];
    for triangle in mesh.active_triangle_slots() {
        for vertex in mesh.triangles()[triangle] {
            degree[vertex] += 1;
        }
    }
    let sites: Vec<_> = mesh
        .active_vertex_slots()
        .filter(|&site| degree[site] == 5)
        .collect();
    sites.try_into().map_err(|sites: Vec<usize>| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CMRC certified mother grid has {} degree-5 sites, expected 12",
                sites.len()
            ),
        )
    })
}

fn certified_icosahedron_vertices(
    addresses: &[Option<earthmesh_refine_certified::VertexAddress>],
) -> io::Result<[usize; 12]> {
    let mut sites = addresses
        .iter()
        .enumerate()
        .filter_map(|(site, address)| match address {
            Some(earthmesh_refine_certified::VertexAddress::IcosahedronVertex(vertex)) => {
                Some((*vertex, site))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    sites.sort_unstable();
    sites
        .into_iter()
        .map(|(_, site)| site)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|sites: Vec<usize>| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CMRC mother grid has {} icosahedron vertices, expected 12",
                    sites.len()
                ),
            )
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
        previous_marks = Some(outcome.interior_marks.clone());
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
    if preserve_locality {
        let flips = crate::redgreen_bridge::legalize_redgreen_mesh(&mut redgreen)?;
        if flips > 0 {
            eprintln!(
                "earthmesh_cli: Red-Green final spherical Delaunay legalization flipped {flips} edges"
            );
            output_mesh = crate::redgreen_bridge::unstructured_mesh_from_redgreen(&redgreen)?;
        }
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

/// Which retained backend a run asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefineBackend {
    MethodC,
    RedGreen,
    Certified,
}

fn effective_refinement_spring_iterations(backend: RefineBackend, requested: usize) -> usize {
    if matches!(backend, RefineBackend::Certified) {
        0
    } else {
        requested
    }
}

/// Resolve `NL%refine_backend`, refusing retired HARP-DV spellings explicitly.
fn refine_backend_name(requested: &str) -> io::Result<RefineBackend> {
    let name = requested.trim().to_ascii_lowercase();
    match name.as_str() {
        "method_c" => Ok(RefineBackend::MethodC),
        "red_green" => Ok(RefineBackend::RedGreen),
        "certified" => Ok(RefineBackend::Certified),
        "harp_dv" | "harp-dv" | "harpdv" => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "NL%refine_backend = harp_dv has been retired; use method_c, red_green, or certified",
        )),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "NL%refine_backend = '{other}' is not a refinement backend; the choices are method_c, red_green and certified"
            ),
        )),
    }
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
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn calculated_zero_mask_does_not_force_a_quiet_threshold_to_refine() {
        let root = std::env::temp_dir().join(format!("cmrc_quiet_cal_mask_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mask = root.join("mask.nml");
        std::fs::write(&mask, "bbox_num = 1\nbbox_refine = 0\n0 90 90 0\n").unwrap();
        let land = root.join("land.nc");
        let mut file = crate::create_netcdf_quiet(&land).unwrap();
        file.add_dimension("longitude", 8).unwrap();
        file.add_dimension("latitude", 4).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&[1_i8; 32], (.., ..))
            .unwrap();
        drop(file);
        let config = EarthmeshConfig {
            mesh_type: "landmesh".into(),
            landtype_file: land.display().to_string(),
            ..EarthmeshConfig::default()
        };
        let mut refine = RefineConfig {
            refine_cal: true,
            max_iter_cal: 2,
            refine_num_landtypes: true,
            th_num_landtypes: 10,
            mask_refine_cal_type: "bbox".into(),
            mask_refine_cal_fprefix: mask.display().to_string(),
            ..RefineConfig::default()
        };
        let contents = "&hfield\n NL%hfield_nlon=8\n NL%hfield_nlat=4\n/\n";
        let plan = certified_requirement_plan(contents, &config, &refine, 144, 0, 2).unwrap();
        assert!(
            plan.regions.is_empty(),
            "evaluation mask is not a hard demand"
        );
        assert!(plan.effective_levels.iter().all(|&level| level == 0));
        assert!(!plan.conservative_global_bound);
        let report = plan
            .threshold_provenance
            .as_ref()
            .expect("quiet threshold still reports provenance");
        assert_eq!(report["scope"], "threshold_only_before_gradient");
        assert!(report["criteria"].is_array());
        let layers = plan.layer_report(None, 3);
        assert_eq!(
            layers["threshold_sources"]["scope"],
            "threshold_only_before_gradient"
        );
        assert!(!layers["threshold_sources"]
            .to_string()
            .contains("composition_timing_ms"));
        let repeat = certified_requirement_plan(contents, &config, &refine, 144, 0, 2).unwrap();
        assert_eq!(layers, repeat.layer_report(None, 3));
        // Preserve the named-region-only contract when there is no threshold.
        refine.refine_num_landtypes = false;
        let plan = certified_requirement_plan(contents, &config, &refine, 144, 0, 2).unwrap();
        assert_eq!(plan.regions.len(), 1);
        assert_eq!(plan.effective_levels.iter().max(), Some(&2));
        assert!(plan.threshold_provenance.is_none());
        assert!(plan.layer_report(None, 3)["threshold_sources"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mother_family_preflight_checks_possible_levels_not_only_the_maximum() {
        assert!(super::validate_certified_mother_family(7, 1).is_err());
        assert!(super::validate_certified_mother_family(3, 4).is_ok());
        assert!(super::validate_certified_mother_family(5, 2).is_ok());
        assert!(super::validate_certified_mother_family(1, usize::MAX).is_ok());
    }

    use super::*;

    #[test]
    fn local_update_admission_is_explicit_and_mixed_coupled_only() {
        let config = EarthmeshConfig {
            refine: true,
            mask_domain_global: true,
            mesh_type: "atmosmesh".into(),
            mode_grid: "hex".into(),
            output_format: "MPAS".into(),
            ..EarthmeshConfig::default()
        };
        let options = CertifiedRunOptions {
            mode: CertifiedMode::ReverseCoarsening,
            delivery: CertifiedDelivery::Coupled,
            angle_contract: earthmesh_refine_certified::AngleContractId::DomainQuality38To82V1,
            ..CertifiedRunOptions::default()
        };
        for levels in [vec![0, 1], vec![0, 1, 2]] {
            validate_local_update_mode(&config, &options, &levels).expect("mixed scope");
        }
        for levels in [vec![], vec![0], vec![1, 1], vec![0, 3]] {
            assert!(validate_local_update_mode(&config, &options, &levels).is_err());
        }
        for rejected in [
            EarthmeshConfig {
                mask_domain_global: false,
                ..config.clone()
            },
            EarthmeshConfig {
                refine: false,
                ..config.clone()
            },
            EarthmeshConfig {
                mesh_type: "oceanmesh".into(),
                ..config.clone()
            },
            EarthmeshConfig {
                mode_grid: "tri".into(),
                ..config.clone()
            },
            EarthmeshConfig {
                output_format: "FVCOM".into(),
                ..config.clone()
            },
        ] {
            assert!(validate_local_update_mode(&rejected, &options, &[0, 1]).is_err());
        }
        for rejected in [
            CertifiedRunOptions {
                mode: CertifiedMode::SafeMotherOnly,
                ..options
            },
            CertifiedRunOptions {
                delivery: CertifiedDelivery::Hex,
                ..options
            },
            CertifiedRunOptions {
                angle_contract: earthmesh_refine_certified::AngleContractId::LegacyStrict40To80,
                ..options
            },
        ] {
            assert!(validate_local_update_mode(&config, &rejected, &[0, 1]).is_err());
        }
    }

    fn legacy_remap_csv(rows: &[earthmesh_refine_certified::remap::RemapRow]) -> Vec<u8> {
        let mut output = Vec::new();
        writeln!(output, "target,source,weight").expect("header");
        for row in rows {
            for &(source, weight) in &row.sources {
                writeln!(output, "{},{},{weight:.17}", row.target, source).expect("row");
            }
        }
        output
    }

    #[test]
    fn remap_csv_parallel_writer_matches_legacy_bytes() {
        let mut rows = vec![
            earthmesh_refine_certified::remap::RemapRow {
                target: 0,
                sources: vec![(2, 1.0), (5, 1.0 / 3.0)],
            },
            earthmesh_refine_certified::remap::RemapRow {
                target: 3,
                sources: vec![(4, 1.0e-14), (7, 0.99999999999999)],
            },
            earthmesh_refine_certified::remap::RemapRow {
                target: 8,
                sources: Vec::new(),
            },
        ];
        rows.extend((3..REMAP_CSV_CHUNK_ROWS + 5).map(|target| {
            earthmesh_refine_certified::remap::RemapRow {
                target,
                sources: vec![(target + 1, 0.5)],
            }
        }));
        let write = || {
            let mut output = Vec::new();
            write_remap_csv(&mut output, &rows).expect("parallel remap csv");
            output
        };
        let one_thread = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(write);
        let four_threads = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(write);

        assert_eq!(one_thread, four_threads);
        assert_eq!(four_threads, legacy_remap_csv(&rows));
    }

    #[test]
    fn failed_certified_publication_restores_the_previous_generation() {
        let names = [
            "certificate",
            "remap",
            "grid",
            "resources",
            "manifest",
            "mpas",
            "mpas-graph",
            "ready",
        ];
        for previous_generation in [false, true] {
            for missing in [4, 5, 6, 7] {
                let directory = std::env::temp_dir().join(format!(
                    "earthmesh-cmrc-publication-{}-{}-{missing}-{previous_generation}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ));
                fs::create_dir_all(&directory).expect("create publication test directory");
                let paths = names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        let temporary = directory.join(format!(".{name}.cmrc-tmp-test"));
                        let final_path = directory.join(name);
                        if index != missing {
                            fs::write(&temporary, format!("new-{name}"))
                                .expect("stage new artifact");
                        }
                        if previous_generation {
                            fs::write(&final_path, format!("old-{name}"))
                                .expect("write old artifact");
                        }
                        (temporary, final_path)
                    })
                    .collect::<Vec<_>>();
                let publications = paths
                    .iter()
                    .map(|(temporary, final_path)| (temporary.as_path(), final_path.as_path()))
                    .collect::<Vec<_>>();
                let obsolete = directory.join("obsolete-remap");
                fs::write(&obsolete, "old-obsolete-remap").expect("write obsolete remap");

                assert!(publish_artifacts(&publications, &[&obsolete]).is_err());
                for (name, (temporary, final_path)) in names.iter().zip(&paths) {
                    if previous_generation {
                        assert_eq!(
                            fs::read_to_string(final_path).expect("restored old artifact"),
                            format!("old-{name}")
                        );
                    } else {
                        assert!(!final_path.exists(), "partial new artifact {name}");
                    }
                    // The caller owns cleanup of staged files not consumed by publication.
                    let _ = fs::remove_file(temporary);
                }
                assert_eq!(
                    fs::read_to_string(&obsolete).expect("restored obsolete remap"),
                    "old-obsolete-remap"
                );
                assert!(fs::read_dir(&directory)
                    .expect("read test directory")
                    .all(|entry| {
                        let name = entry.expect("directory entry").file_name();
                        let name = name.to_string_lossy();
                        !name.contains("earthmesh-backup") && !name.contains("cmrc-tmp")
                    }));
                fs::remove_dir_all(directory).expect("clean publication test directory");
            }
        }
    }

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
    fn only_certified_disables_the_generic_spring() {
        assert_eq!(
            effective_refinement_spring_iterations(RefineBackend::RedGreen, 2_000),
            2_000
        );
        assert_eq!(
            effective_refinement_spring_iterations(RefineBackend::MethodC, 5_000),
            5_000
        );
        assert_eq!(
            effective_refinement_spring_iterations(RefineBackend::Certified, 5_000),
            0
        );
    }

    #[test]
    fn lepp_region_constraints_are_real_mesh_edges() {
        let method_c = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
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
    fn backend_neutral_regional_spring_moves_only_safe_region_interiors() {
        let base = earthmesh_refine_method_c::MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25)
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
        let mesh = earthmesh_refine_method_c::MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25)
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
