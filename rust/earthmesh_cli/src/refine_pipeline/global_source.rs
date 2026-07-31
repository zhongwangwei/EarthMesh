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
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_core::{EarthmeshConfig, EarthmeshRuntimeState, RefineConfig};
use earthmesh_mesh::{
    grid_cartesian_xy_to_lonlat_placeholders_one_based_state, grid_xyz2lonlat_one_based_state,
    method_c_hfield_spawn_failure, pcvt_adjust_voronoi_grid_state,
    voronoi_grid_from_method_c_delaunay_mesh, voronoi_grid_from_method_c_delaunay_mesh_cartesian,
    MethodCDelaunayMesh, MethodCHfieldLegalizationPreflight, MethodCHfieldPassDiagnostics,
    MethodCHfieldSelectionCheckpoint, MethodCHfieldSpawnFailure,
};

use super::outputs::{write_method_c_refined_outputs, MethodCMetadataSlices};

fn method_c_topology_gradation_g(
    refine: &RefineConfig,
    max_level: usize,
    requested_g: f64,
    cap_enabled: bool,
) -> f64 {
    if !cap_enabled || max_level < 2 {
        return requested_g;
    }
    let transition_rows = (1..max_level)
        .map(|level| {
            refine.halo[level]
                .max(refine.max_transition_row[level])
                .max(0) as usize
        })
        .max()
        .unwrap_or(0)
        .max(4);
    requested_g.min(1.0 / (4.0 * transition_rows as f64))
}

fn add_method_c_face_lineage_demands(
    mesh: &MethodCDelaunayMesh,
    face_demand: &mut [bool],
    requested: &BTreeSet<i64>,
) -> io::Result<usize> {
    if requested.is_empty() {
        return Ok(0);
    }
    if face_demand.len() != mesh.nwd + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C support face-demand length does not match the current mesh",
        ));
    }
    let lineages = mesh.gridfile_m_cell_lineages()?;
    let mut matched = 0usize;
    for (row, lineage) in lineages.into_iter().enumerate() {
        if requested.contains(&lineage) {
            face_demand[row + 1] = true;
            matched += 1;
        }
    }
    if matched != requested.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Method-C cross-level support mapped {matched}/{} stable parent-face lineages",
                requested.len()
            ),
        ));
    }
    Ok(matched)
}

fn hfield_raster_resolution_warning(field: &earthmesh_hfield::HField) -> Option<String> {
    let dy = earthmesh_hfield::great_circle_distance_m(
        0.0,
        field.lat_center(0),
        0.0,
        field.lat_center(1),
    );
    let mut underresolved = 0usize;
    let mut max_ratio = 0.0_f64;
    for j in 0..field.nlat() {
        let lat = field.lat_center(j);
        let dx = earthmesh_hfield::great_circle_distance_m(
            field.lon_center(0),
            lat,
            field.lon_center(1),
            lat,
        );
        let spacing = dx.max(dy);
        for i in 0..field.nlon() {
            let ratio = spacing / field.get(i, j);
            if ratio > 1.0 {
                underresolved += 1;
                max_ratio = max_ratio.max(ratio);
            }
        }
    }
    (underresolved > 0).then(|| {
        format!(
            "HField raster {}x{} under-resolves {underresolved}/{} bins for gradient limiting \
             (max axis spacing/local h={max_ratio:.3}; required <=1); increase \
             hfield_nlon/hfield_nlat when sub-raster detail must be preserved",
            field.nlon(),
            field.nlat(),
            field.nlon() * field.nlat(),
        )
    })
}

#[derive(Clone, Copy)]
struct M0TopologyGradation {
    cap_enabled: bool,
    requested_g: f64,
    effective_g: f64,
}

const M0_LEGALIZATION_CHECKPOINT_SCHEMA: &str = "earthmesh-method-c-legalization-checkpoint-v2";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct M0LegalizationCheckpointProvenance {
    build_profile: String,
    executable_sha256: String,
    namelist_sha256: String,
    landcover_file_name: String,
    landcover_sha256: String,
    source_nlon: usize,
    source_nlat: usize,
    source_samples_per_degree: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct M0MethodCLegalizationCheckpoint {
    schema: String,
    pass: usize,
    child_grid_number: usize,
    field_max_level: usize,
    max_mrows: usize,
    support_lineages: Vec<Vec<i64>>,
    selection: MethodCHfieldSelectionCheckpoint,
    preflight: MethodCHfieldLegalizationPreflight,
    mesh: MethodCDelaunayMesh,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct M0MethodCLegalizationCheckpointReceipt {
    checkpoint_sha256: String,
    provenance: M0LegalizationCheckpointProvenance,
}

fn m0_legalization_checkpoint_bytes(
    checkpoint: &M0MethodCLegalizationCheckpoint,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(checkpoint).map_err(io::Error::other)
}

fn m0_sidecar_path(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let mut file_name = path.file_name().map(OsString::from).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "checkpoint has no file name")
    })?;
    file_name.push(suffix);
    Ok(path.with_file_name(file_name))
}

fn write_m0_method_c_legalization_checkpoint(
    path: &Path,
    checkpoint: &M0MethodCLegalizationCheckpoint,
    provenance: &M0LegalizationCheckpointProvenance,
) -> io::Result<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = m0_legalization_checkpoint_bytes(checkpoint)?;
    let mut temp_name = path.file_name().map(OsString::from).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "checkpoint has no file name")
    })?;
    temp_name.push(format!(".{}.tmp", std::process::id()));
    let temp = path.with_file_name(temp_name);
    fs::write(&temp, bytes)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    let sha256 = earthmesh_project::file_content_hash(path)?;
    fs::write(m0_sidecar_path(path, ".sha256")?, format!("{sha256}\n"))?;
    fs::write(
        m0_sidecar_path(path, ".provenance.json")?,
        serde_json::to_vec(&M0MethodCLegalizationCheckpointReceipt {
            checkpoint_sha256: sha256.clone(),
            provenance: provenance.clone(),
        })
        .map_err(io::Error::other)?,
    )?;
    Ok(sha256)
}

fn m0_method_c_checkpoint_provenance(
    executable: &Path,
    namelist: &Path,
    landcover: &Path,
    source_nlon: usize,
    source_nlat: usize,
    source_samples_per_degree: usize,
) -> io::Result<M0LegalizationCheckpointProvenance> {
    Ok(M0LegalizationCheckpointProvenance {
        build_profile: if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        },
        executable_sha256: earthmesh_project::file_content_hash(executable)?,
        namelist_sha256: earthmesh_project::file_content_hash(namelist)?,
        landcover_file_name: landcover
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<non-utf8>")
            .to_string(),
        landcover_sha256: earthmesh_project::file_content_hash(landcover)?,
        source_nlon,
        source_nlat,
        source_samples_per_degree,
    })
}

fn m0_seed_hash(ids: &[usize]) -> String {
    let hash = ids.iter().fold(0xcbf29ce484222325_u64, |hash, id| {
        (*id as u64).to_le_bytes().iter().fold(hash, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
    });
    format!("{hash:016x}")
}

fn m0_hfield_pass_json(pass: &MethodCHfieldPassDiagnostics) -> serde_json::Value {
    let reasons = pass.face_reason_mask_counts;
    let candidate_validation = pass.candidate_validation.as_ref().map(|validation| serde_json::json!({
        "selected_faces_after_concavity": validation.selected_faces_after_concavity,
        "coverage_valid": validation.coverage_valid,
        "parent_level_histogram": validation.parent_level_histogram.iter().map(|(level, faces)| {
            serde_json::json!({"level": level, "faces": faces})
        }).collect::<Vec<_>>(),
        "parent_level_valid": validation.parent_level_valid,
        "perimeter_lengths": validation.perimeter_lengths,
        "perimeters_triplets": validation.perimeters_triplets,
        "predicted_transition_self_loops": validation.predicted_transition_self_loops,
        "predicted_transition_first_parent_u_edge": validation.predicted_transition_first_parent_u_edge,
        "local_seed_candidate_pool": validation.local_seed_candidate_pool,
        "local_seed_edit_sets_tested": validation.local_seed_edit_sets_tested,
        "local_seed_edit_coverage_valid": validation.local_seed_edit_coverage_valid,
        "local_seed_edit_parent_level_valid": validation.local_seed_edit_parent_level_valid,
        "local_seed_edit_triplet_valid": validation.local_seed_edit_triplet_valid,
        "local_seed_edit_predictor_clear": validation.local_seed_edit_predictor_clear,
        "local_seed_edit_first_predictor_clear_seeds": validation.local_seed_edit_first_predictor_clear_seeds,
        "local_seed_edit_first_predictor_clear_removes_seed": validation.local_seed_edit_first_predictor_clear_removes_seed,
        "local_seed_edit_materializable": validation.local_seed_edit_materializable,
        "local_seed_edit_first_seeds": validation.local_seed_edit_first_seeds,
        "local_seed_edit_first_removes_seed": validation.local_seed_edit_first_removes_seed,
        "local_seed_edit_first_failure_kind": validation.local_seed_edit_first_failure_kind.map(|kind| kind.as_str()),
        "local_seed_edit_first_failure_parent_m_point": validation.local_seed_edit_first_failure_parent_m_point,
        "local_seed_edit_first_failure_parent_u_edge": validation.local_seed_edit_first_failure_parent_u_edge,
        "local_seed_edit_first_failure_parent_m_valence_witnesses": validation.local_seed_edit_first_failure_parent_m_valence_witnesses,
        "local_seed_edit_first_failure_message": validation.local_seed_edit_first_failure_message,
        "transition_materializable": validation.transition_materializable,
        "materialized_m_valence_census_available": validation.materialized_m_valence_census_available,
        "materialized_m_valence_violation_count": validation.materialized_m_valence_violation_count,
        "failure_kind": validation.failure_kind.map(|kind| kind.as_str()),
        "failure_message": validation.failure_message,
    }));
    serde_json::json!({
        "pass": pass.pass,
        "preserve_all_demands": pass.preserve_all_demands,
        "parent_interior_m_points": pass.parent_interior_m_points,
        "hard_demand_m_points": pass.hard_demand_m_points,
        "hard_demand_anchors": pass.hard_demand_anchors,
        "phase_support_m_points": pass.phase_support_m_points,
        "component_count": pass.component_count,
        "component_phases": pass.component_phases.iter().map(|component| serde_json::json!({
            "component_index": component.component_index,
            "component_m_points": component.component_m_points,
            "demand_start": component.demand_start,
            "phase_class_count": component.phase_class_count,
            "phase_starts": component.phase_starts,
            "selected_phase_ordinal": component.selected_phase_ordinal,
            "selected_start": component.selected_start,
            "legal_seed_ids": component.legal_seed_ids,
            "selected_seed_ids": component.selected_seed_ids,
        })).collect::<Vec<_>>(),
        "legal_rad3_seeds": pass.legal_rad3_seeds,
        "initial_selected_seeds": pass.initial_selected_seeds,
        "initial_seed_footprint_faces": pass.initial_seed_footprint_faces,
        "demand_tail_seeds": pass.demand_tail_seeds,
        "demand_tail_faces": pass.demand_tail_faces,
        "connectivity_bridge_seeds": pass.connectivity_bridge_seeds,
        "connectivity_bridge_faces": pass.connectivity_bridge_faces,
        "face_reason_mask_counts": {
            "unexplained": reasons[0],
            "initial_seed_footprint_only": reasons[1],
            "demand_tail_only": reasons[2],
            "initial_seed_footprint_and_demand_tail": reasons[3],
            "connectivity_bridge_only": reasons[4],
            "initial_seed_footprint_and_connectivity_bridge": reasons[5],
            "demand_tail_and_connectivity_bridge": reasons[6],
            "initial_seed_footprint_and_demand_tail_and_connectivity_bridge": reasons[7],
        },
        "face_reason_exclusive_counts": {
            "initial_seed_footprint": reasons[1],
            "demand_tail": reasons[2],
            "connectivity_bridge": reasons[4],
        },
        "face_reason_pairwise_overlap_counts": {
            "initial_seed_footprint_and_demand_tail": reasons[3] + reasons[7],
            "initial_seed_footprint_and_connectivity_bridge": reasons[5] + reasons[7],
            "demand_tail_and_connectivity_bridge": reasons[6] + reasons[7],
        },
        "alignable_faces": pass.alignable_faces,
        "final_selected_faces": pass.final_selected_faces,
        "unexplained_selected_faces": pass.unexplained_selected_faces,
        "selected_seed_count": pass.selected_seed_ids.len(),
        "selected_seed_hash": m0_seed_hash(&pass.selected_seed_ids),
        "seed_union_vertex_only_contacts": pass.seed_union_vertex_only_contacts,
        "seed_union_first_contact_m_point": pass.seed_union_first_contact_m_point,
        "seed_reconstruction_matches": pass.seed_reconstruction_matches,
        "seed_reconstruction_error": pass.seed_reconstruction_error,
        "candidate_validation": candidate_validation,
    })
}

fn write_m0_hfield_diagnostics(
    passes: &[MethodCHfieldPassDiagnostics],
    failure: Option<&MethodCHfieldSpawnFailure>,
    topology_gradation: M0TopologyGradation,
) -> io::Result<()> {
    let Some(path) = std::env::var_os("EARTHMESH_M0_DIAGNOSTICS_PATH") else {
        return Ok(());
    };
    let failure = failure.map(|failure| {
        serde_json::json!({
            "pass": failure.pass,
            "kind": failure.kind.as_str(),
            "perimeter_lengths": &failure.perimeter_lengths,
            "repair_attempts": failure.repair_attempts,
            "m_point": failure.m_point,
            "parent_m_point": failure.parent_m_point,
            "parent_u_edge": failure.parent_u_edge,
            "parent_m_valence_witnesses": &failure.parent_m_valence_witnesses,
            "w_face": failure.w_face,
            "actual_mrlw": failure.actual_mrlw,
            "expected_mrlw": failure.expected_mrlw,
            "message": failure.to_string(),
        })
    });
    let payload = serde_json::to_vec_pretty(&serde_json::json!({
        "topology_gradation": {
            "cap_enabled": topology_gradation.cap_enabled,
            "requested_g": topology_gradation.requested_g,
            "effective_g": topology_gradation.effective_g,
        },
        "passes": passes.iter().map(m0_hfield_pass_json).collect::<Vec<_>>(),
        "failure": failure,
    }))
    .map_err(io::Error::other)?;
    fs::write(path, payload)
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
        let source_lineages =
            crate::grid_quality_pipeline::read_gridfile_cell_lineages(&gridinit.gridfile.output)?;
        method_c_delaunay_mesh_from_unstructured_gridfile(
            &source_gridfile,
            MethodCGridfileMetadataSlices {
                m_lineage: (!source_lineages.m.is_empty()).then_some(source_lineages.m.as_slice()),
                m_refine_level: (!source_levels.m_refine_level.is_empty())
                    .then_some(source_levels.m_refine_level.as_slice()),
                m_refine_level_orig: (!source_levels.m_refine_level_orig.is_empty())
                    .then_some(source_levels.m_refine_level_orig.as_slice()),
                m_ngr: (!source_levels.m_ngr.is_empty()).then_some(source_levels.m_ngr.as_slice()),
                w_lineage: (!source_lineages.w.is_empty()).then_some(source_lineages.w.as_slice()),
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
    let mut hfield_demand = None;
    let mut single_hfield_product_support = None;
    let mut coupled_hfield_product_support = None;
    let mut spherical_hfield = None;
    let mut native_landcover = None;
    if let Some(hfield) = active_hfield_options.filter(|_| !native_cartesian_xy) {
        let base_m = hfield.base_m.unwrap_or_else(|| {
            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * nxp as f64)
        });
        let field_max_level = hfield.max_level.unwrap_or(max_level).clamp(1, 5);
        let mut hfield_refine = refine.clone();
        if refine.refine_cal
            && crate::landtype_file_is_real(&config.landtype_file)
            && (refine.refine_num_landtypes
                || refine.refine_area_mainland
                || refine.refine_sea_ratio)
        {
            let path = Path::new(config.landtype_file.trim());
            let (source_nlon, source_nlat, maxlc) =
                crate::hfield_refine::landtype_source_shape_and_maxlc(path)?;
            if source_nlon > hfield.nlon || source_nlat > hfield.nlat {
                let gridnum_perdegree = source_nlon.checked_div(360).filter(|value| {
                    *value > 0
                        && value.checked_mul(360) == Some(source_nlon)
                        && value.checked_mul(180) == Some(source_nlat)
                });
                let gridnum_perdegree = gridnum_perdegree.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "native landtype dimensions {source_nlon}x{source_nlat} must be a global 360x180 multiple"
                        ),
                    )
                })?;
                eprintln!(
                    "earthmesh_cli: native landcover refinement uses all {source_nlon}x{source_nlat} source pixels; coarse HField projection disabled"
                );
                native_landcover = Some((path.to_path_buf(), maxlc, gridnum_perdegree));
                hfield_refine.refine_num_landtypes = false;
                hfield_refine.refine_area_mainland = false;
                hfield_refine.refine_sea_ratio = false;
            }
        }
        let mut composed = crate::hfield_refine::build_composed_hfield_with_demand(
            &regions,
            &hfield_refine,
            mesh_type,
            Some(&config),
            base_m,
            hfield,
            max_cal_level.clamp(1, field_max_level),
            domain_region.as_ref(),
        )?;
        if let Some((_summary, hydro_hard)) =
            crate::hydro_refinement_adapter::apply_hydro_target_to_fields(
                &mut composed.regularized,
                &mut composed.hard,
                hfield,
                base_m,
                domain_region.as_ref(),
            )?
        {
            composed
                .hard_layers
                .push(crate::hfield_refine::HfieldHardDemandLayer {
                    kind: earthmesh_hfield::DemandSourceKind::Hydro,
                    descriptor: "hydro",
                    field: hydro_hard,
                });
        }
        crate::hfield_refine::constrain_hfield_to_domain(
            &mut composed.regularized,
            domain_region.as_ref(),
            base_m,
            hfield.g,
        )?;
        // Native landcover stays on source pixels. Product support is bound
        // later from the actual masked output grid, never from a coarse
        // landtype-to-HField projection.
        let native_product_support = native_landcover
            .as_ref()
            .map(|_| vec![true; composed.regularized.nlon() * composed.regularized.nlat()]);
        match config.mesh_type.trim() {
            "landmesh" => {
                single_hfield_product_support = Some(match &native_product_support {
                    Some(support) => support.clone(),
                    None => crate::hfield_refine::intended_output_landtype_support_mask(
                        &composed.regularized,
                        &config,
                        true,
                    )?,
                });
            }
            "oceanmesh" => {
                single_hfield_product_support = Some(match &native_product_support {
                    Some(support) => support.clone(),
                    None => crate::hfield_refine::intended_output_landtype_support_mask(
                        &composed.regularized,
                        &config,
                        false,
                    )?,
                });
            }
            "LOCmesh" => {
                coupled_hfield_product_support = Some(match &native_product_support {
                    Some(support) => (support.clone(), support.clone()),
                    None => (
                        crate::hfield_refine::intended_output_landtype_support_mask(
                            &composed.regularized,
                            &config,
                            true,
                        )?,
                        crate::hfield_refine::intended_output_landtype_support_mask(
                            &composed.regularized,
                            &config,
                            false,
                        )?,
                    ),
                });
            }
            _ => {}
        }
        if native_surface_global_expansion {
            // A requested global expansion is hard source demand, not topology excess.
            let mut native_hard =
                earthmesh_hfield::HField::uniform(hfield.nlon, hfield.nlat, base_m)?;
            native_hard.min_with_fn(|_, _| base_m / 2.0);
            composed.hard.min_with_field(&native_hard)?;
            composed.regularized.min_with_field(&native_hard)?;
            composed.regularized.limit_gradient(hfield.g)?;
            composed
                .hard_layers
                .push(crate::hfield_refine::HfieldHardDemandLayer {
                    kind: earthmesh_hfield::DemandSourceKind::Specified,
                    descriptor: "native-surface-global-expansion",
                    field: native_hard,
                });
        }
        if let Some(warning) = hfield_raster_resolution_warning(&composed.regularized) {
            eprintln!("earthmesh_cli: warning: {warning}");
        }
        hfield_demand = Some(
            crate::source_demand_artifact::PreparedHfieldDemand::
                capture_with_hard_sources_and_product_support(
                &composed.hard,
                &composed.regularized,
                &composed.hard_layers,
                base_m,
                field_max_level as u8,
                hfield.g,
                &contents,
                native_product_support.as_deref(),
            )?,
        );
        spherical_hfield = Some(composed.regularized);
    }
    let mut spring_diagnostics = Vec::new();
    let mut hfield_pass_diagnostics = Vec::new();
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
                    None,
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
        } else if let Some((landtype_path, maxlc, gridnum_perdegree)) = native_landcover.as_ref() {
            let sizing_field = spherical_hfield
                .as_ref()
                .expect("spherical HField is prepared before mesh generation");
            let topology_g_cap_enabled =
                std::env::var("EARTHMESH_M0_TOPOLOGY_G_CAP").map_or(true, |value| value != "off");
            let topology_g = method_c_topology_gradation_g(
                &refine,
                field_max_level,
                hfield.g,
                topology_g_cap_enabled,
            );
            let topology_field = if topology_g < hfield.g {
                let mut field = sizing_field.clone();
                field.limit_gradient(topology_g)?;
                eprintln!(
                    "earthmesh_cli: Method-C topology limited HField g {} -> {} to preserve transition clearance",
                    hfield.g, topology_g
                );
                Some(field)
            } else {
                None
            };
            let field = topology_field.as_ref().unwrap_or(sizing_field);
            let source_nlon = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "landtype longitude count overflows usize",
                )
            })?;
            let source_nlat = gridnum_perdegree.checked_mul(180).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "landtype latitude count overflows usize",
                )
            })?;
            let axes = crate::build_global_source_axes_one_based(
                *gridnum_perdegree,
                source_nlon,
                source_nlat,
            )?;
            let sampler = crate::mkgrd_data_preprocess_source::FrozenLandtypeSampler::open(
                landtype_path,
                *gridnum_perdegree,
            )?;
            let mut refined = mesh;
            let mut passes = 0usize;
            let mut grid_number = refined
                .w_faces
                .iter()
                .skip(2)
                .map(|face| face.ngr)
                .max()
                .unwrap_or(1)
                .max(1)
                + 1;
            let first_grid_number = grid_number;
            // M0-only: this closes cross-level parent support, but Case 9 still
            // has an independent same-level TransitionPatch.
            let cross_level_support =
                std::env::var_os("EARTHMESH_M0_CROSS_LEVEL_SUPPORT").is_some();
            let legalization_checkpoint_path =
                std::env::var_os("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_PATH").map(PathBuf::from);
            let mut legalization_checkpoint_provenance = None;
            let mut support_lineages = vec![BTreeSet::new(); field_max_level + 1];
            let mut checkpoints = Vec::with_capacity(field_max_level);
            let mut pass = 1usize;
            while pass <= field_max_level {
                if cross_level_support {
                    if checkpoints.len() < pass {
                        checkpoints.push(refined.clone());
                    } else {
                        checkpoints[pass - 1] = refined.clone();
                    }
                }
                let mut face_demand =
                    crate::native_landcover_refine::native_landcover_face_demands(
                        &refined,
                        &sampler,
                        &axes,
                        &refine,
                        mesh_type,
                        *maxlc,
                        pass,
                        domain_region.as_ref(),
                    )?;
                // The native landcover path carries demand per face rather than
                // through the HField raster, so the persisted HField snapshot is
                // all zeros for these runs. Dump the per-face demand instead,
                // before materialization, so a failing pass still leaves the
                // demand behind for analysis.
                if let Some(path) = std::env::var_os("EARTHMESH_M0_FACE_DEMAND_DUMP_DIR") {
                    let dir = PathBuf::from(path);
                    fs::create_dir_all(&dir)?;
                    // Face ids index the internal W table, which is rebuilt
                    // every pass and does not line up with the written
                    // gridfile. Carry the centre coordinate so a demand point
                    // stays identifiable across passes and meshes.
                    let demanded = face_demand
                        .iter()
                        .enumerate()
                        .filter_map(|(iw, &wanted)| wanted.then_some(iw))
                        .map(|iw| {
                            let corners = refined.w_faces[iw].im;
                            let points = corners
                                .iter()
                                .take(3)
                                .map(|&im| refined.m_points[im])
                                .collect::<Vec<_>>();
                            let centre = points.iter().fold([0.0f64; 3], |mut acc, p| {
                                acc[0] += p.x / 3.0;
                                acc[1] += p.y / 3.0;
                                acc[2] += p.z / 3.0;
                                acc
                            });
                            let norm =
                                (centre[0].powi(2) + centre[1].powi(2) + centre[2].powi(2)).sqrt();
                            let lat = (centre[2] / norm).asin().to_degrees();
                            let lon = centre[1].atan2(centre[0]).to_degrees();
                            serde_json::json!({
                                "face": iw,
                                "mrlw": refined.w_faces[iw].mrlw,
                                "lon": lon,
                                "lat": lat,
                            })
                        })
                        .collect::<Vec<_>>();
                    let report = serde_json::json!({
                        "kind": "earthmesh_native_landcover_face_demand",
                        "pass": pass,
                        "face_count": face_demand.len(),
                        "demanded_face_count": demanded.len(),
                        "demanded": demanded,
                    });
                    let out = dir.join(format!("face-demand-pass{pass}.json"));
                    fs::write(&out, serde_json::to_vec_pretty(&report)?)?;
                    eprintln!(
                        "earthmesh_cli: pass {pass} face demand {}/{} -> {}",
                        report["demanded_face_count"], face_demand.len(), out.display()
                    );
                }
                if cross_level_support {
                    add_method_c_face_lineage_demands(
                        &refined,
                        &mut face_demand,
                        &support_lineages[pass],
                    )?;
                }
                let preserve_all_demands = pass == field_max_level;
                if cross_level_support && pass > 1 {
                    let mut required = refined
                        .required_parent_support_lineages_from_target_levels_and_face_demands(
                            |lon, lat| {
                                field.topology_level_at(lon, lat, base_m, field_max_level as u8)
                            },
                            &face_demand,
                            pass,
                            preserve_all_demands,
                        )?;
                    // A concession in the previous attempt moved the boundary
                    // after the oracle had already answered, so the faces the
                    // new perimeter needs were never requested. Fold them in
                    // here; the `added == 0` guard below still stops the loop if
                    // this stops making progress.
                    let post_drop = earthmesh_mesh::take_post_drop_support_lineages();
                    if !post_drop.is_empty() {
                        eprintln!(
                            "earthmesh_cli: Method-C pass {pass} folding {} post-concession support lineages",
                            post_drop.len()
                        );
                        for lineage in post_drop {
                            if let Ok(value) = i64::try_from(lineage) {
                                required.push(value);
                            }
                        }
                        required.sort_unstable();
                        required.dedup();
                    }
                    if !required.is_empty() {
                        let parent_pass = pass - 1;
                        let mut added = 0usize;
                        for lineage in required {
                            added += usize::from(support_lineages[parent_pass].insert(lineage));
                        }
                        if added == 0 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "Method-C cross-level support made no progress before pass {pass}"
                                ),
                            ));
                        }
                        eprintln!(
                            "earthmesh_cli: Method-C pass {pass} requested {added} stable parent-face support refinements from pass {parent_pass}"
                        );
                        refined = checkpoints[parent_pass - 1].clone();
                        checkpoints.truncate(parent_pass - 1);
                        for pending in support_lineages.iter_mut().skip(parent_pass + 1) {
                            pending.clear();
                        }
                        pass = parent_pass;
                        passes = parent_pass - 1;
                        grid_number = first_grid_number + parent_pass - 1;
                        continue;
                    }
                }
                if cross_level_support
                    && pass > 1
                    && support_lineages.iter().any(|lineages| !lineages.is_empty())
                {
                    if let Some(path) = legalization_checkpoint_path.as_deref() {
                        if legalization_checkpoint_provenance.is_none() {
                            legalization_checkpoint_provenance =
                                Some(m0_method_c_checkpoint_provenance(
                                    &std::env::current_exe()?,
                                    namelist_source,
                                    landtype_path,
                                    source_nlon,
                                    source_nlat,
                                    *gridnum_perdegree,
                                )?);
                        }
                        let selection = refined
                            .selection_checkpoint_from_target_levels_and_face_demands(
                                |lon, lat| {
                                    field.topology_level_at(lon, lat, base_m, field_max_level as u8)
                                },
                                &face_demand,
                                pass,
                                preserve_all_demands,
                            )?;
                        let preflight = refined.legalization_preflight_from_selected_faces(
                            &selection.selected_faces,
                            &selection.legal_seed_ids,
                            &selection.selected_seed_ids,
                            grid_number,
                        )?;
                        for patch in &preflight.patches {
                            let boundary = refined.legalization_patch_boundary_check(
                                &selection,
                                &preflight,
                                patch,
                                &patch.selected_candidate_seed_ids,
                                grid_number,
                                max_mrows,
                            )?;
                            if !boundary.is_closed() {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "Method-C legalization patch {} baseline changes {} faces outside its boundary (perimeter_interface_changed={})",
                                        patch.cluster_index,
                                        boundary.outside_changed_faces.len(),
                                        boundary.outside_perimeter_interface_changed
                                    ),
                                ));
                            }
                        }
                        let checkpoint = M0MethodCLegalizationCheckpoint {
                            schema: M0_LEGALIZATION_CHECKPOINT_SCHEMA.to_string(),
                            pass,
                            child_grid_number: grid_number,
                            field_max_level,
                            max_mrows,
                            support_lineages: support_lineages
                                .iter()
                                .map(|lineages| lineages.iter().copied().collect())
                                .collect(),
                            selection,
                            preflight,
                            mesh: refined.clone(),
                        };
                        let sha256 = write_m0_method_c_legalization_checkpoint(
                            path,
                            &checkpoint,
                            legalization_checkpoint_provenance
                                .as_ref()
                                .expect("checkpoint provenance initialized above"),
                        )?;
                        eprintln!(
                            "earthmesh_cli: wrote Method-C legalization checkpoint {} sha256={sha256}",
                            path.display()
                        );
                    }
                }
                let Some(next) = refined.spawn_nest_pass_from_target_levels_and_face_demands(
                    |lon, lat| field.topology_level_at(lon, lat, base_m, field_max_level as u8),
                    &face_demand,
                    pass,
                    grid_number,
                    max_mrows,
                    preserve_all_demands,
                )?
                else {
                    break;
                };
                refined = if spring_nest_iterations > 0 {
                    next.spring_nest(nxp, spring_nest_iterations, grid_number, false)?
                } else {
                    next
                };
                passes += 1;
                grid_number += 1;
                pass += 1;
            }
            (refined, passes)
        } else {
            let sizing_field = spherical_hfield
                .as_ref()
                .expect("spherical HField is prepared before mesh generation");
            let topology_g_cap_enabled =
                std::env::var("EARTHMESH_M0_TOPOLOGY_G_CAP").map_or(true, |value| value != "off");
            let topology_g = method_c_topology_gradation_g(
                &refine,
                field_max_level,
                hfield.g,
                topology_g_cap_enabled,
            );
            let topology_gradation = M0TopologyGradation {
                cap_enabled: topology_g_cap_enabled,
                requested_g: hfield.g,
                effective_g: topology_g,
            };
            let topology_field = if topology_g < hfield.g {
                let mut field = sizing_field.clone();
                field.limit_gradient(topology_g)?;
                eprintln!(
                    "earthmesh_cli: Method-C topology limited HField g {} -> {} to preserve transition clearance",
                    hfield.g, topology_g
                );
                Some(field)
            } else {
                None
            };
            let field = topology_field.as_ref().unwrap_or(sizing_field);
            let collect_m0 = std::env::var_os("EARTHMESH_M0_DIAGNOSTICS").is_some();
            let measured = if collect_m0 {
                mesh.spawn_nest_from_target_levels_with_m0_diagnostics(
                    |lon, lat| field.topology_level_at(lon, lat, base_m, field_max_level as u8),
                    field_max_level,
                    max_mrows,
                    nxp,
                    spring_nest_iterations,
                    true,
                )
            } else {
                mesh.spawn_nest_from_target_levels_with_spring_diagnostics(
                    |lon, lat| field.topology_level_at(lon, lat, base_m, field_max_level as u8),
                    field_max_level,
                    max_mrows,
                    nxp,
                    spring_nest_iterations,
                    false,
                )
                .map(|(mesh, passes, spring)| (mesh, passes, spring, Vec::new()))
            };
            let (mesh, passes, diagnostics, pass_diagnostics) = match measured {
                Ok(measured) => measured,
                Err(error) => {
                    let failure = method_c_hfield_spawn_failure(&error);
                    write_m0_hfield_diagnostics(
                        failure
                            .map(|failure| failure.pass_diagnostics.as_slice())
                            .unwrap_or(&[]),
                        failure,
                        topology_gradation,
                    )?;
                    return Err(error);
                }
            };
            spring_diagnostics = diagnostics;
            hfield_pass_diagnostics = pass_diagnostics;
            write_m0_hfield_diagnostics(&hfield_pass_diagnostics, None, topology_gradation)?;
            if let Err(error) = crate::m1_topology_frozen::run_if_requested(
                &mesh,
                field,
                hfield_demand.as_ref(),
                &contents,
                nxp,
            ) {
                eprintln!("earthmesh_cli: warning: M1 diagnostics failed: {error}");
            }
            (mesh, passes)
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
    let m_lineages = mesh.gridfile_m_cell_lineages()?;
    let w_lineages = mesh.gridfile_w_cell_lineages()?;

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
            m_lineage: &m_lineages,
            m_refine_level: &m_refine_levels,
            m_refine_level_orig: &m_refine_levels_orig,
            m_ngr: &m_ngr,
            w_lineage: &w_lineages,
            w_refine_level: &w_refine_levels,
            w_refine_level_orig: &w_refine_levels_orig,
            w_ngr: &w_ngr,
        }),
        hfield_demand.as_ref(),
        single_hfield_product_support.as_deref(),
        coupled_hfield_product_support
            .as_ref()
            .map(|(land, ocean)| (land.as_slice(), ocean.as_slice())),
    )?;
    if let Some(demand) = &hfield_demand {
        if let Some(product_support) = single_hfield_product_support.as_deref() {
            let product = match config.mesh_type.trim() {
                "landmesh" => crate::source_demand_artifact::HfieldDemandProductKind::Land,
                "oceanmesh" => crate::source_demand_artifact::HfieldDemandProductKind::Ocean,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("single HField product support is invalid for mesh_type {other}"),
                    ));
                }
            };
            demand.persist_for_product_gridfile(
                &outputs.output.output,
                product,
                config.mode_grid.trim(),
                product_support,
            )?;
        } else {
            demand.persist_for_gridfile(&outputs.output.output)?;
        }
        match (
            outputs.coupled_outputs.as_ref(),
            coupled_hfield_product_support.as_ref(),
        ) {
            (Some(coupled), Some((land_support, ocean_support))) => {
                demand.persist_for_product_gridfile(
                    &coupled.land_output.output,
                    crate::source_demand_artifact::HfieldDemandProductKind::Land,
                    config.mode_grid.trim(),
                    land_support,
                )?;
                demand.persist_for_product_gridfile(
                    &coupled.ocean_output.output,
                    crate::source_demand_artifact::HfieldDemandProductKind::Ocean,
                    config.mode_grid.trim(),
                    ocean_support,
                )?;
            }
            (None, None) => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "LOCmesh HField product support and coupled outputs were not produced together",
                ));
            }
        }
    }
    let (actual_max_level, refined_cells) =
        final_output_refinement_stats(&outputs.output.output, &config.mode_grid)?;

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
        actual_max_level,
        refined_cells,
        transition_faces,
        spring_nest_passes,
        spring_nest_iterations,
        spring_diagnostics,
        hfield_pass_diagnostics,
        raw_output: outputs.raw_output,
        landtype_masked_cells: outputs.landtype_masked_cells,
        coupled_outputs: outputs.coupled_outputs,
        output: outputs.output,
        runtime_state,
    })
}

fn final_output_refinement_stats(output: &Path, mode_grid: &str) -> io::Result<(usize, usize)> {
    let variable_name = if mode_grid.trim().eq_ignore_ascii_case("tri") {
        "earthmesh_m_refine_level"
    } else if mode_grid.trim().eq_ignore_ascii_case("hex") {
        "earthmesh_w_refine_level"
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "final refinement statistics support tri or hex mode_grid only, got {}",
                mode_grid.trim()
            ),
        ));
    };
    let file = crate::open_netcdf(output).map_err(crate::netcdf_to_io_error)?;
    let variable = file.variable(variable_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "final output {} is missing {variable_name}",
                output.display()
            ),
        )
    })?;
    let levels = variable
        .get_values::<i32, _>(..)
        .map_err(crate::netcdf_to_io_error)?;
    levels.iter().enumerate().try_fold(
        (0usize, 0usize),
        |(actual_max_level, refined_cells), (row, &level)| {
            let level = usize::try_from(level).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "final output {} has negative {variable_name} at row {}",
                        output.display(),
                        row + 1
                    ),
                )
            })?;
            Ok((
                actual_max_level.max(level),
                refined_cells + usize::from(level > 0),
            ))
        },
    )
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
    use crate::{GridRegion, LonLatPoint, MethodCGridfileMetadataSlices, UnstructuredMesh};
    use earthmesh_core::{GridMemory, IjTabs, ItabM, ItabW};
    use earthmesh_mesh::{
        method_c_hfield_failure_kind, xyz_to_lonlat_degrees, CartesianPoint,
        MethodCHfieldExactPatchTableStatus, MethodCHfieldLegalizationPatch,
    };
    use std::collections::{BTreeMap, HashMap, VecDeque};

    #[derive(Debug, Serialize)]
    struct M0LegalizationEnumerationReport {
        status: &'static str,
        cluster_index: usize,
        search_order: &'static str,
        candidate_seed_count: usize,
        candidate_seed_ids: Vec<usize>,
        current_perimeter_scope_candidate_seed_ids: Option<Vec<usize>>,
        covers_current_perimeter_scope: bool,
        total_assignments: Option<usize>,
        skipped_assignments: usize,
        assignment_limit: usize,
        evaluated_assignments: usize,
        boundary_incomplete_assignments: usize,
        hard_rejected_assignments: BTreeMap<&'static str, usize>,
        unclassified_error_assignments: usize,
        first_unclassified_error: Option<String>,
        exact_failure_counts: BTreeMap<&'static str, usize>,
        assignment: Option<Vec<usize>>,
    }

    enum M0LegalizationAssignmentOutcome {
        BoundaryIncomplete,
        Sat,
        HardRejected(&'static str),
        ExactFailure(&'static str),
        Unclassified(String),
    }

    fn m0_frozen_target_levels(
        checkpoint: &M0MethodCLegalizationCheckpoint,
    ) -> HashMap<(u64, u64), u8> {
        let mut levels = HashMap::new();
        let mut record = |point: CartesianPoint, level: usize| {
            let point = xyz_to_lonlat_degrees(point);
            let key = (point.lon_degrees.to_bits(), point.lat_degrees.to_bits());
            let level = u8::try_from(level).expect("frozen target level fits u8");
            if let Some(previous) = levels.insert(key, level) {
                assert_eq!(
                    previous, level,
                    "one frozen sample coordinate has conflicting target levels"
                );
            }
        };
        for im in 2..=checkpoint.mesh.nmd {
            record(
                checkpoint.mesh.m_points[im],
                checkpoint.selection.m_target_levels[im],
            );
        }
        for iu in 2..=checkpoint.mesh.nud {
            let [im1, im2] = checkpoint.mesh.u_edges[iu].im;
            let p1 = checkpoint.mesh.m_points[im1];
            let p2 = checkpoint.mesh.m_points[im2];
            record(
                CartesianPoint::new(
                    0.5 * (p1.x + p2.x),
                    0.5 * (p1.y + p2.y),
                    0.5 * (p1.z + p2.z),
                ),
                checkpoint.selection.u_target_levels[iu],
            );
        }
        levels
    }

    fn m0_dilate_face_demands_same_level(mesh: &MethodCDelaunayMesh, demand: &[bool]) -> Vec<bool> {
        assert_eq!(demand.len(), mesh.nwd + 1, "face-demand length");
        let mut dilated = demand.to_vec();
        for iw in 2..=mesh.nwd {
            if !demand[iw] {
                continue;
            }
            let level = mesh.w_faces[iw].mrlw;
            for &iu in &mesh.w_faces[iw].iu[..3] {
                for &neighbor in &mesh.u_edges[iu].iw[..2] {
                    if neighbor >= 2 && mesh.w_faces[neighbor].mrlw == level {
                        dilated[neighbor] = true;
                    }
                }
            }
        }
        dilated
    }

    fn m0_face_demand_components_same_level(
        mesh: &MethodCDelaunayMesh,
        demand: &[bool],
    ) -> Vec<Vec<usize>> {
        assert_eq!(demand.len(), mesh.nwd + 1, "face-demand length");
        let mut seen = vec![false; demand.len()];
        let mut components = Vec::new();
        for start in 2..=mesh.nwd {
            if !demand[start] || seen[start] {
                continue;
            }
            let level = mesh.w_faces[start].mrlw;
            let mut queue = VecDeque::from([start]);
            let mut component = Vec::new();
            seen[start] = true;
            while let Some(iw) = queue.pop_front() {
                component.push(iw);
                for &iu in &mesh.w_faces[iw].iu[..3] {
                    for &neighbor in &mesh.u_edges[iu].iw[..2] {
                        if neighbor >= 2
                            && !seen[neighbor]
                            && demand[neighbor]
                            && mesh.w_faces[neighbor].mrlw == level
                        {
                            seen[neighbor] = true;
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
            component.sort_unstable();
            components.push(component);
        }
        components
    }

    fn m0_dilate_one_face_demand_component(
        mesh: &MethodCDelaunayMesh,
        baseline: &[bool],
        component: &[usize],
    ) -> Vec<bool> {
        let mut component_mask = vec![false; baseline.len()];
        for &iw in component {
            component_mask[iw] = true;
        }
        let expanded = m0_dilate_face_demands_same_level(mesh, &component_mask);
        baseline
            .iter()
            .zip(expanded)
            .map(|(&baseline, expanded)| baseline || expanded)
            .collect()
    }

    fn m0_usize_list_env(name: &str) -> Vec<usize> {
        std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| {
                value
                    .split(',')
                    .map(|item| item.parse::<usize>().expect("unsigned integer list"))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn m0_evaluate_face_demand(
        checkpoint: &M0MethodCLegalizationCheckpoint,
        levels: &HashMap<(u64, u64), u8>,
        demand: &[bool],
    ) -> serde_json::Value {
        let target_level = |lon: f64, lat: f64| {
            *levels
                .get(&(lon.to_bits(), lat.to_bits()))
                .expect("selection sampled outside the frozen target-level coordinates")
        };
        let demand_face_count = demand.iter().skip(2).filter(|&&face| face).count();
        match checkpoint
            .mesh
            .selection_checkpoint_from_target_levels_and_face_demands(
                &target_level,
                demand,
                checkpoint.pass,
                checkpoint.selection.preserve_all_demands,
            ) {
            Ok(selection) => {
                match checkpoint.mesh.legalization_preflight_from_selected_faces(
                    &selection.selected_faces,
                    &selection.legal_seed_ids,
                    &selection.selected_seed_ids,
                    checkpoint.child_grid_number,
                ) {
                    Ok(preflight) => {
                        let offsets = vec![0usize; preflight.perimeter_lengths.len()];
                        match checkpoint
                            .mesh
                            .spawn_nest_pass_method_c_with_perimeter_component_offsets_for_diagnostics(
                                &preflight.prepared_selected_faces,
                                &offsets,
                                checkpoint.child_grid_number,
                                checkpoint.max_mrows,
                                true,
                            )
                        {
                            Ok(mesh) => {
                                mesh.validate_topology().expect("face-demand SAT topology");
                                serde_json::json!({
                                    "status": "SAT",
                                    "demand_face_count": demand_face_count,
                                    "legal_seed_count": selection.legal_seed_ids.len(),
                                    "selected_seed_count": selection.selected_seed_ids.len(),
                                    "selected_face_count": preflight.prepared_selected_faces
                                        .iter().skip(2).filter(|&&face| face).count(),
                                    "perimeter_lengths": preflight.perimeter_lengths,
                                    "self_loop_witness_count": preflight.self_loop_witnesses.len(),
                                    "child_counts": {"m": mesh.nmd, "u": mesh.nud, "w": mesh.nwd},
                                })
                            }
                            Err(error) => serde_json::json!({
                                "status": "exact_failure",
                                "demand_face_count": demand_face_count,
                                "legal_seed_count": selection.legal_seed_ids.len(),
                                "selected_seed_count": selection.selected_seed_ids.len(),
                                "selected_face_count": preflight.prepared_selected_faces
                                    .iter().skip(2).filter(|&&face| face).count(),
                                "perimeter_lengths": preflight.perimeter_lengths,
                                "self_loop_witness_count": preflight.self_loop_witnesses.len(),
                                "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                                "error": error.to_string(),
                            }),
                        }
                    }
                    Err(error) => serde_json::json!({
                        "status": "preflight_failure",
                        "demand_face_count": demand_face_count,
                        "legal_seed_count": selection.legal_seed_ids.len(),
                        "selected_seed_count": selection.selected_seed_ids.len(),
                        "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                        "error": error.to_string(),
                    }),
                }
            }
            Err(error) => serde_json::json!({
                "status": "selection_failure",
                "demand_face_count": demand_face_count,
                "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                "error": error.to_string(),
            }),
        }
    }

    fn m0_evaluate_legalization_assignment(
        checkpoint: &M0MethodCLegalizationCheckpoint,
        patch: &MethodCHfieldLegalizationPatch,
        assignment: &[usize],
    ) -> M0LegalizationAssignmentOutcome {
        match checkpoint.mesh.legalization_patch_boundary_check(
            &checkpoint.selection,
            &checkpoint.preflight,
            patch,
            assignment,
            checkpoint.child_grid_number,
            checkpoint.max_mrows,
        ) {
            Ok(check) if !check.is_closed() => M0LegalizationAssignmentOutcome::BoundaryIncomplete,
            Ok(check) if check.exact_materializable => M0LegalizationAssignmentOutcome::Sat,
            Ok(check) => M0LegalizationAssignmentOutcome::ExactFailure(
                check
                    .exact_failure_kind
                    .map_or("unknown", |kind| kind.as_str()),
            ),
            Err(error) => {
                let kind = method_c_hfield_failure_kind(&error);
                if kind == earthmesh_mesh::MethodCHfieldFailureKind::Other {
                    M0LegalizationAssignmentOutcome::Unclassified(error.to_string())
                } else {
                    M0LegalizationAssignmentOutcome::HardRejected(kind.as_str())
                }
            }
        }
    }

    fn m0_enumerate_legalization_patch(
        checkpoint: &M0MethodCLegalizationCheckpoint,
        patch: &MethodCHfieldLegalizationPatch,
        skip_assignments: usize,
        max_assignments: usize,
    ) -> M0LegalizationEnumerationReport {
        let candidate_seed_count = patch.candidate_seed_ids.len();
        let current_perimeter_scope_candidate_seed_ids = checkpoint
            .preflight
            .current_perimeter_candidate_scope(&patch.perimeter_components)
            .expect("valid current perimeter candidate scope");
        let covers_current_perimeter_scope = current_perimeter_scope_candidate_seed_ids
            .as_ref()
            .is_some_and(|scope| {
                scope
                    .iter()
                    .all(|seed| patch.candidate_seed_ids.binary_search(seed).is_ok())
            });
        let total_assignments =
            (candidate_seed_count < usize::BITS as usize).then(|| 1usize << candidate_seed_count);
        let assignment_limit = total_assignments.map_or(max_assignments, |total| {
            total.saturating_sub(skip_assignments).min(max_assignments)
        });
        let mut boundary_incomplete_assignments = 0usize;
        let mut hard_rejected_assignments = BTreeMap::new();
        let mut unclassified_error_assignments = 0usize;
        let mut first_unclassified_error = None;
        let mut exact_failure_counts = BTreeMap::new();
        let baseline = patch
            .selected_candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut visited_assignments = 0usize;
        let mut evaluated_assignments = 0usize;

        'search: for hamming_weight in 0..=candidate_seed_count {
            let mut toggles = (0..hamming_weight).collect::<Vec<_>>();
            loop {
                if visited_assignments < skip_assignments {
                    visited_assignments += 1;
                } else if evaluated_assignments == assignment_limit {
                    break 'search;
                } else {
                    if evaluated_assignments > 0 && evaluated_assignments % 65_536 == 0 {
                        eprintln!(
                            "earthmesh_cli: Method-C legalization patch {} evaluated {evaluated_assignments}/{assignment_limit} assignments after skipping {skip_assignments}",
                            patch.cluster_index
                        );
                    }
                    let assignment = patch
                        .candidate_seed_ids
                        .iter()
                        .enumerate()
                        .filter_map(|(bit, &seed)| {
                            (baseline.contains(&seed) ^ toggles.binary_search(&bit).is_ok())
                                .then_some(seed)
                        })
                        .collect::<Vec<_>>();
                    evaluated_assignments += 1;
                    match m0_evaluate_legalization_assignment(checkpoint, patch, &assignment) {
                        M0LegalizationAssignmentOutcome::BoundaryIncomplete => {
                            boundary_incomplete_assignments += 1;
                        }
                        M0LegalizationAssignmentOutcome::Sat => {
                            return M0LegalizationEnumerationReport {
                                status: "SAT",
                                cluster_index: patch.cluster_index,
                                search_order: "baseline_hamming_weight_then_seed_order",
                                candidate_seed_count,
                                candidate_seed_ids: patch.candidate_seed_ids.clone(),
                                current_perimeter_scope_candidate_seed_ids,
                                covers_current_perimeter_scope,
                                total_assignments,
                                skipped_assignments: skip_assignments,
                                assignment_limit,
                                evaluated_assignments,
                                boundary_incomplete_assignments,
                                hard_rejected_assignments,
                                unclassified_error_assignments,
                                first_unclassified_error,
                                exact_failure_counts,
                                assignment: Some(assignment),
                            };
                        }
                        M0LegalizationAssignmentOutcome::HardRejected(kind) => {
                            *hard_rejected_assignments.entry(kind).or_default() += 1;
                        }
                        M0LegalizationAssignmentOutcome::ExactFailure(kind) => {
                            *exact_failure_counts.entry(kind).or_default() += 1;
                        }
                        M0LegalizationAssignmentOutcome::Unclassified(error) => {
                            unclassified_error_assignments += 1;
                            first_unclassified_error.get_or_insert(error);
                        }
                    }
                    visited_assignments += 1;
                }

                if hamming_weight == 0 {
                    break;
                }
                let Some(index) = (0..hamming_weight)
                    .rev()
                    .find(|&index| toggles[index] < candidate_seed_count - hamming_weight + index)
                else {
                    break;
                };
                toggles[index] += 1;
                for next in (index + 1)..hamming_weight {
                    toggles[next] = toggles[next - 1] + 1;
                }
            }
        }

        let exhaustive = skip_assignments == 0 && total_assignments == Some(evaluated_assignments);
        M0LegalizationEnumerationReport {
            status: if exhaustive
                && boundary_incomplete_assignments == 0
                && unclassified_error_assignments == 0
                && covers_current_perimeter_scope
            {
                "PATCH_UNSAT"
            } else {
                "INCOMPLETE"
            },
            cluster_index: patch.cluster_index,
            search_order: "baseline_hamming_weight_then_seed_order",
            candidate_seed_count,
            candidate_seed_ids: patch.candidate_seed_ids.clone(),
            current_perimeter_scope_candidate_seed_ids,
            covers_current_perimeter_scope,
            total_assignments,
            skipped_assignments: skip_assignments,
            assignment_limit,
            evaluated_assignments,
            boundary_incomplete_assignments,
            hard_rejected_assignments,
            unclassified_error_assignments,
            first_unclassified_error,
            exact_failure_counts,
            assignment: None,
        }
    }

    #[derive(Debug, Serialize)]
    struct M0LegalizationNogoodReport {
        cluster_index: usize,
        candidate_seed_count: usize,
        fixed_seed_states: Vec<(usize, bool)>,
        free_seed_count: usize,
        blocked_assignments: usize,
        generalization_evaluations: usize,
        verification_evaluations: usize,
        hard_rejected_assignments: BTreeMap<&'static str, usize>,
        exact_failure_counts: BTreeMap<&'static str, usize>,
    }

    struct M0LegalizationSubcubeProof {
        safe: bool,
        evaluated_assignments: usize,
        hard_rejected_assignments: BTreeMap<&'static str, usize>,
        exact_failure_counts: BTreeMap<&'static str, usize>,
    }

    fn m0_assignment_from_value_mask(
        patch: &MethodCHfieldLegalizationPatch,
        value_mask: usize,
    ) -> Vec<usize> {
        patch
            .candidate_seed_ids
            .iter()
            .enumerate()
            .filter_map(|(bit, &seed)| ((value_mask >> bit) & 1 == 1).then_some(seed))
            .collect()
    }

    fn m0_prove_legalization_nogood_subcube(
        checkpoint: &M0MethodCLegalizationCheckpoint,
        patch: &MethodCHfieldLegalizationPatch,
        fixed_mask: usize,
        fixed_values: usize,
    ) -> M0LegalizationSubcubeProof {
        let free_bits = (0..patch.candidate_seed_ids.len())
            .filter(|bit| fixed_mask & (1usize << bit) == 0)
            .collect::<Vec<_>>();
        let total = 1usize << free_bits.len();
        let mut hard_rejected_assignments = BTreeMap::new();
        let mut exact_failure_counts = BTreeMap::new();
        for completion in 0..total {
            let mut value_mask = fixed_values & fixed_mask;
            for (completion_bit, &candidate_bit) in free_bits.iter().enumerate() {
                if completion & (1usize << completion_bit) != 0 {
                    value_mask |= 1usize << candidate_bit;
                }
            }
            let assignment = m0_assignment_from_value_mask(patch, value_mask);
            match m0_evaluate_legalization_assignment(checkpoint, patch, &assignment) {
                M0LegalizationAssignmentOutcome::HardRejected(kind) => {
                    *hard_rejected_assignments.entry(kind).or_default() += 1;
                }
                M0LegalizationAssignmentOutcome::ExactFailure(kind) => {
                    *exact_failure_counts.entry(kind).or_default() += 1;
                }
                M0LegalizationAssignmentOutcome::BoundaryIncomplete
                | M0LegalizationAssignmentOutcome::Sat
                | M0LegalizationAssignmentOutcome::Unclassified(_) => {
                    return M0LegalizationSubcubeProof {
                        safe: false,
                        evaluated_assignments: completion + 1,
                        hard_rejected_assignments,
                        exact_failure_counts,
                    };
                }
            }
        }
        M0LegalizationSubcubeProof {
            safe: true,
            evaluated_assignments: total,
            hard_rejected_assignments,
            exact_failure_counts,
        }
    }

    fn m0_generalize_legalization_nogood(
        checkpoint: &M0MethodCLegalizationCheckpoint,
        patch: &MethodCHfieldLegalizationPatch,
        assignment: &[usize],
    ) -> Option<M0LegalizationNogoodReport> {
        let candidate_seed_count = patch.candidate_seed_ids.len();
        if candidate_seed_count >= usize::BITS as usize {
            return None;
        }
        let selected = assignment.iter().copied().collect::<BTreeSet<_>>();
        let candidates = patch
            .candidate_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if selected.len() != assignment.len() || !selected.is_subset(&candidates) {
            return None;
        }
        let fixed_values = patch
            .candidate_seed_ids
            .iter()
            .enumerate()
            .fold(0usize, |mask, (bit, seed)| {
                mask | (usize::from(selected.contains(seed)) << bit)
            });
        let mut fixed_mask = (1usize << candidate_seed_count) - 1;
        let initial =
            m0_prove_legalization_nogood_subcube(checkpoint, patch, fixed_mask, fixed_values);
        if !initial.safe {
            return None;
        }
        let mut generalization_evaluations = initial.evaluated_assignments;
        for bit in 0..candidate_seed_count {
            let trial = fixed_mask & !(1usize << bit);
            let proof =
                m0_prove_legalization_nogood_subcube(checkpoint, patch, trial, fixed_values);
            generalization_evaluations += proof.evaluated_assignments;
            if proof.safe {
                fixed_mask = trial;
            }
        }
        let verification =
            m0_prove_legalization_nogood_subcube(checkpoint, patch, fixed_mask, fixed_values);
        if !verification.safe {
            return None;
        }
        let fixed_seed_states = patch
            .candidate_seed_ids
            .iter()
            .enumerate()
            .filter_map(|(bit, &seed)| {
                (fixed_mask & (1usize << bit) != 0)
                    .then_some((seed, fixed_values & (1usize << bit) != 0))
            })
            .collect::<Vec<_>>();
        let free_seed_count = candidate_seed_count - fixed_seed_states.len();
        Some(M0LegalizationNogoodReport {
            cluster_index: patch.cluster_index,
            candidate_seed_count,
            fixed_seed_states,
            free_seed_count,
            blocked_assignments: 1usize << free_seed_count,
            generalization_evaluations,
            verification_evaluations: verification.evaluated_assignments,
            hard_rejected_assignments: verification.hard_rejected_assignments,
            exact_failure_counts: verification.exact_failure_counts,
        })
    }

    #[test]
    fn cross_level_support_maps_stable_w_lineages_to_face_demands() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100)
            .expect("build canonical mesh");
        let lineages = mesh
            .gridfile_m_cell_lineages()
            .expect("read W-face lineages");
        let requested = BTreeSet::from([lineages[1], lineages[lineages.len() - 1]]);
        let mut face_demand = vec![false; mesh.nwd + 1];

        assert_eq!(
            add_method_c_face_lineage_demands(&mesh, &mut face_demand, &requested)
                .expect("map support"),
            2
        );
        assert!(face_demand[2]);
        assert!(face_demand[mesh.nwd]);
    }

    #[test]
    fn m0_legalization_checkpoint_is_byte_stable_and_round_trips() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100)
            .expect("build canonical mesh");
        let face_demand = vec![false; mesh.nwd + 1];
        let selection = mesh
            .selection_checkpoint_from_target_levels_and_face_demands(
                |_, _| 0,
                &face_demand,
                2,
                false,
            )
            .expect("selection checkpoint");
        let checkpoint = M0MethodCLegalizationCheckpoint {
            schema: M0_LEGALIZATION_CHECKPOINT_SCHEMA.to_string(),
            pass: 2,
            child_grid_number: 3,
            field_max_level: 3,
            max_mrows: MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            support_lineages: vec![Vec::new(), vec![17, 23], Vec::new(), Vec::new()],
            selection,
            preflight: MethodCHfieldLegalizationPreflight {
                prepared_selected_faces: face_demand,
                perimeter_lengths: Vec::new(),
                perimeter_remainders: Vec::new(),
                perimeter_candidate_seed_ids: Vec::new(),
                self_loop_witnesses: Vec::new(),
                witness_dependency_clusters: Vec::new(),
                patches: Vec::new(),
            },
            mesh,
        };
        let first = m0_legalization_checkpoint_bytes(&checkpoint).expect("serialize checkpoint");
        let second = m0_legalization_checkpoint_bytes(&checkpoint).expect("serialize checkpoint");
        assert_eq!(first, second);
        assert_eq!(
            serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(&first)
                .expect("parse checkpoint"),
            checkpoint
        );

        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_method_c_legalization_checkpoint_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let path = root.join("checkpoint.json");
        let provenance = M0LegalizationCheckpointProvenance {
            build_profile: "test".to_string(),
            executable_sha256: "11".repeat(32),
            namelist_sha256: "22".repeat(32),
            landcover_file_name: "landtype.nc".to_string(),
            landcover_sha256: "33".repeat(32),
            source_nlon: 86_400,
            source_nlat: 43_200,
            source_samples_per_degree: 240,
        };
        let first_hash = write_m0_method_c_legalization_checkpoint(&path, &checkpoint, &provenance)
            .expect("write checkpoint");
        let first_file = fs::read(&path).expect("read checkpoint");
        let second_hash =
            write_m0_method_c_legalization_checkpoint(&path, &checkpoint, &provenance)
                .expect("rewrite checkpoint");
        assert_eq!(first_hash, second_hash);
        assert_eq!(
            fs::read(&path).expect("read rewritten checkpoint"),
            first_file
        );
        assert_eq!(
            fs::read_to_string(m0_sidecar_path(&path, ".sha256").expect("sidecar path"))
                .expect("read sidecar"),
            format!("{first_hash}\n")
        );
        assert_eq!(
            serde_json::from_slice::<M0MethodCLegalizationCheckpointReceipt>(
                &fs::read(
                    m0_sidecar_path(&path, ".provenance.json").expect("provenance sidecar path"),
                )
                .expect("read provenance sidecar"),
            )
            .expect("parse provenance sidecar"),
            M0MethodCLegalizationCheckpointReceipt {
                checkpoint_sha256: first_hash,
                provenance,
            }
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn m0_legalization_enumerator_finds_known_sat() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100)
            .expect("build canonical mesh");
        let mut face_demand = vec![false; mesh.nwd + 1];
        face_demand[2] = true;
        let selection = mesh
            .selection_checkpoint_from_target_levels_and_face_demands(
                |_, _| 0,
                &face_demand,
                1,
                true,
            )
            .expect("selection checkpoint");
        let preflight = mesh
            .legalization_preflight_from_selected_faces(
                &selection.selected_faces,
                &selection.legal_seed_ids,
                &selection.selected_seed_ids,
                2,
            )
            .expect("legalization preflight");
        let mut candidate_seeds = selection
            .selected_seed_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for &seed in &selection.legal_seed_ids {
            candidate_seeds.insert(seed);
            if candidate_seeds.len() == 8 {
                break;
            }
        }
        let candidate_seed_ids = candidate_seeds.into_iter().collect::<Vec<_>>();
        assert_eq!(candidate_seed_ids.len(), 8);
        let mutable_faces = mesh
            .selected_faces_from_method_c_seed_ids(&candidate_seed_ids)
            .expect("candidate footprints")
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(iw, &selected)| selected.then_some(iw))
            .collect();
        let patch = MethodCHfieldLegalizationPatch {
            cluster_index: 0,
            witness_indices: Vec::new(),
            witness_perimeter_components: Vec::new(),
            perimeter_components: Vec::new(),
            perimeter_interfaces: Vec::new(),
            dependency_faces: Vec::new(),
            dependency_face_lineages: Vec::new(),
            candidate_seed_lineages: Vec::new(),
            selected_candidate_seed_ids: Vec::new(),
            candidate_seed_ids,
            mutable_faces,
            mutable_face_lineages: Vec::new(),
        };
        assert!(patch.candidate_seed_ids.len() < usize::BITS as usize);
        let checkpoint = M0MethodCLegalizationCheckpoint {
            schema: M0_LEGALIZATION_CHECKPOINT_SCHEMA.to_string(),
            pass: 1,
            child_grid_number: 2,
            field_max_level: 1,
            max_mrows: MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            support_lineages: Vec::new(),
            selection,
            preflight,
            mesh,
        };
        let report = m0_enumerate_legalization_patch(
            &checkpoint,
            &patch,
            0,
            1usize << patch.candidate_seed_ids.len(),
        );
        let sharded = m0_enumerate_legalization_patch(&checkpoint, &patch, 1, 1);

        assert_eq!(report.status, "SAT");
        assert!(report.assignment.is_some());
        assert!(report.evaluated_assignments > 1);
        assert_eq!(sharded.status, "SAT");
        assert_eq!(sharded.skipped_assignments, 1);
        assert_eq!(sharded.evaluated_assignments, 1);
        assert_eq!(sharded.assignment, report.assignment);
        assert!(!report.covers_current_perimeter_scope);
        assert_eq!(report.current_perimeter_scope_candidate_seed_ids, None);
        assert!(
            m0_generalize_legalization_nogood(
                &checkpoint,
                &patch,
                report.assignment.as_deref().expect("SAT assignment"),
            )
            .is_none(),
            "a SAT assignment must never produce a blocking nogood"
        );

        let mut empty_scope_patch = patch;
        empty_scope_patch.candidate_seed_ids.clear();
        empty_scope_patch.selected_candidate_seed_ids.clear();
        empty_scope_patch.mutable_faces.clear();
        let empty_scope = m0_enumerate_legalization_patch(&checkpoint, &empty_scope_patch, 0, 1);
        assert_ne!(empty_scope.status, "PATCH_UNSAT");
        assert!(!empty_scope.covers_current_perimeter_scope);
        assert_eq!(empty_scope.current_perimeter_scope_candidate_seed_ids, None);
    }

    #[test]
    #[ignore = "requires one frozen successful M0 gridfile"]
    fn m0_legalization_enumerator_finds_sat_on_frozen_m0_grid() {
        let gridfile = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_SAT_GRIDFILE")
                .expect("frozen successful M0 gridfile"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_SAT_OUTPUT")
                .expect("positive-control output path"),
        );
        let nxp = std::env::var("EARTHMESH_M0_LEGALIZATION_SAT_NXP")
            .unwrap_or_else(|_| "81".to_string())
            .parse::<usize>()
            .expect("numeric positive-control NXP");
        let source = read_unstructured_mesh_netcdf(&gridfile).expect("read frozen M0 gridfile");
        let levels = crate::grid_quality_pipeline::read_gridfile_mesh_points(&gridfile)
            .expect("read frozen M0 levels");
        let lineages = crate::grid_quality_pipeline::read_gridfile_cell_lineages(&gridfile)
            .expect("read frozen M0 lineages");
        let mesh = method_c_delaunay_mesh_from_unstructured_gridfile(
            &source,
            MethodCGridfileMetadataSlices {
                m_lineage: (!lineages.m.is_empty()).then_some(lineages.m.as_slice()),
                m_refine_level: (!levels.m_refine_level.is_empty())
                    .then_some(levels.m_refine_level.as_slice()),
                m_refine_level_orig: (!levels.m_refine_level_orig.is_empty())
                    .then_some(levels.m_refine_level_orig.as_slice()),
                m_ngr: (!levels.m_ngr.is_empty()).then_some(levels.m_ngr.as_slice()),
                w_lineage: (!lineages.w.is_empty()).then_some(lineages.w.as_slice()),
                w_refine_level: (!levels.w_refine_level.is_empty())
                    .then_some(levels.w_refine_level.as_slice()),
                w_refine_level_orig: (!levels.w_refine_level_orig.is_empty())
                    .then_some(levels.w_refine_level_orig.as_slice()),
                w_ngr: (!levels.w_ngr.is_empty()).then_some(levels.w_ngr.as_slice()),
            },
            nxp,
            0,
            1.0,
            0.25,
            2_000_000,
        )
        .expect("rebuild frozen M0 Method-C mesh");
        let child_grid_number = mesh
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.ngr)
            .max()
            .unwrap_or(1)
            + 1;
        let pass = child_grid_number - 1;
        let parent_level = mesh
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.mrlw)
            .max()
            .expect("frozen M0 physical W faces");

        let mut positive = None;
        for demand_face in (2..=mesh.nwd)
            .filter(|&iw| mesh.w_faces[iw].mrlw == parent_level)
            .take(256)
        {
            let mut face_demand = vec![false; mesh.nwd + 1];
            face_demand[demand_face] = true;
            let Ok(selection) = mesh.selection_checkpoint_from_target_levels_and_face_demands(
                |_, _| 0,
                &face_demand,
                pass,
                true,
            ) else {
                continue;
            };
            if selection.selected_seed_ids.is_empty() || selection.selected_seed_ids.len() > 8 {
                continue;
            }
            let Ok(preflight) = mesh.legalization_preflight_from_selected_faces(
                &selection.selected_faces,
                &selection.legal_seed_ids,
                &selection.selected_seed_ids,
                child_grid_number,
            ) else {
                continue;
            };
            let mut candidate_seeds = selection
                .selected_seed_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            for &seed in &selection.legal_seed_ids {
                candidate_seeds.insert(seed);
                if candidate_seeds.len() == 8 {
                    break;
                }
            }
            if candidate_seeds.len() != 8 {
                continue;
            }
            let candidate_seed_ids = candidate_seeds.into_iter().collect::<Vec<_>>();
            let mutable_faces = mesh
                .selected_faces_from_method_c_seed_ids(&candidate_seed_ids)
                .expect("positive-control candidate footprints")
                .iter()
                .enumerate()
                .skip(2)
                .filter_map(|(iw, &selected)| selected.then_some(iw))
                .collect();
            let mut patch = MethodCHfieldLegalizationPatch {
                cluster_index: 0,
                witness_indices: Vec::new(),
                witness_perimeter_components: Vec::new(),
                perimeter_components: Vec::new(),
                perimeter_interfaces: Vec::new(),
                dependency_faces: Vec::new(),
                dependency_face_lineages: Vec::new(),
                candidate_seed_lineages: Vec::new(),
                selected_candidate_seed_ids: selection.selected_seed_ids.clone(),
                candidate_seed_ids,
                mutable_faces,
                mutable_face_lineages: Vec::new(),
            };
            let Ok(known) = mesh.legalization_patch_boundary_check(
                &selection,
                &preflight,
                &patch,
                &patch.selected_candidate_seed_ids,
                child_grid_number,
                MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            ) else {
                continue;
            };
            if !known.is_closed() || !known.exact_materializable {
                continue;
            }
            patch.selected_candidate_seed_ids.clear();
            let checkpoint = M0MethodCLegalizationCheckpoint {
                schema: M0_LEGALIZATION_CHECKPOINT_SCHEMA.to_string(),
                pass,
                child_grid_number,
                field_max_level: pass,
                max_mrows: MethodCDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                support_lineages: Vec::new(),
                selection,
                preflight,
                mesh: mesh.clone(),
            };
            let report = m0_enumerate_legalization_patch(&checkpoint, &patch, 0, 256);
            if report.status == "SAT" && report.evaluated_assignments > 1 {
                positive = Some((demand_face, checkpoint, report));
                break;
            }
        }

        let (demand_face, checkpoint, report) =
            positive.expect("find an uninjected 8-variable SAT positive control");
        let artifact = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_frozen_m0_sat_control",
            "gridfile_sha256": earthmesh_project::file_content_hash(&gridfile)
                .expect("frozen gridfile hash"),
            "enumerator_executable_sha256": earthmesh_project::file_content_hash(
                &std::env::current_exe().expect("current test executable")
            ).expect("enumerator executable hash"),
            "child_grid_number": checkpoint.child_grid_number,
            "max_mrows": checkpoint.max_mrows,
            "demand_face": demand_face,
            "result": report,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create positive-control output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&artifact).expect("serialize positive-control artifact"),
        )
        .expect("write positive-control artifact");
        let hash = earthmesh_project::file_content_hash(&output).expect("artifact hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("positive-control sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write positive-control sidecar");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_enumeration_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_ENUMERATION_OUTPUT")
                .expect("enumeration output path"),
        );
        let cluster_index = std::env::var("EARTHMESH_M0_LEGALIZATION_PATCH_CLUSTER")
            .expect("patch cluster")
            .parse::<usize>()
            .expect("numeric patch cluster");
        let max_assignments = std::env::var("EARTHMESH_M0_LEGALIZATION_MAX_ASSIGNMENTS")
            .unwrap_or_else(|_| "1024".to_string())
            .parse::<usize>()
            .expect("numeric assignment limit");
        let skip_assignments = std::env::var("EARTHMESH_M0_LEGALIZATION_SKIP_ASSIGNMENTS")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<usize>()
            .expect("numeric assignment skip");
        let expansion_rings = std::env::var("EARTHMESH_M0_LEGALIZATION_PATCH_RINGS")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<usize>()
            .expect("numeric patch expansion rings");
        let include_local_phases =
            std::env::var_os("EARTHMESH_M0_LEGALIZATION_LOCAL_PHASES").is_some();
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let enumerator_executable_sha256 = earthmesh_project::file_content_hash(
            &std::env::current_exe().expect("current test executable"),
        )
        .expect("enumerator executable hash");
        let mut patch = checkpoint
            .preflight
            .patches
            .iter()
            .find(|patch| patch.cluster_index == cluster_index)
            .expect("checkpoint patch cluster")
            .clone();
        for _ in 0..expansion_rings {
            patch = checkpoint
                .mesh
                .expand_legalization_patch_one_ring(
                    &checkpoint.selection,
                    &checkpoint.preflight,
                    &patch,
                )
                .expect("expand checkpoint patch");
        }
        if include_local_phases {
            patch = checkpoint
                .mesh
                .expand_legalization_patch_local_phases(
                    &checkpoint.selection,
                    &checkpoint.preflight,
                    &patch,
                )
                .expect("expand checkpoint local phase candidates");
        }
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_enumeration_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "enumerator_executable_sha256": enumerator_executable_sha256,
            "child_grid_number": checkpoint.child_grid_number,
            "max_mrows": checkpoint.max_mrows,
            "expansion_rings": expansion_rings,
            "include_local_phases": include_local_phases,
            "result": m0_enumerate_legalization_patch(
                &checkpoint,
                &patch,
                skip_assignments,
                max_assignments,
            ),
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_compiled_table_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_TABLE_OUTPUT")
                .expect("compiled-table output path"),
        );
        let cluster_index = std::env::var("EARTHMESH_M0_LEGALIZATION_PATCH_CLUSTER")
            .expect("patch cluster")
            .parse::<usize>()
            .expect("numeric patch cluster");
        let max_variables = std::env::var("EARTHMESH_M0_LEGALIZATION_TABLE_MAX_VARIABLES")
            .unwrap_or_else(|_| "12".to_string())
            .parse::<usize>()
            .expect("numeric compiled-table variable limit");
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let patch = checkpoint
            .preflight
            .patches
            .iter()
            .find(|patch| patch.cluster_index == cluster_index)
            .expect("checkpoint patch cluster");
        let compiled = checkpoint
            .mesh
            .compile_bounded_exact_legalization_patch_table_for_diagnostics(
                &checkpoint.selection,
                &checkpoint.preflight,
                patch,
                checkpoint.child_grid_number,
                checkpoint.max_mrows,
                max_variables,
            )
            .expect("compile exact patch table");
        let status = match compiled.status {
            MethodCHfieldExactPatchTableStatus::Sat => "SAT",
            MethodCHfieldExactPatchTableStatus::PatchUnsat => "PATCH_UNSAT",
            MethodCHfieldExactPatchTableStatus::Incomplete => "INCOMPLETE",
        };
        let propagation = compiled.propagation.as_ref().map(|propagation| {
            serde_json::json!({
                "consistent": propagation.consistent,
                "rounds": propagation.rounds,
                "pruned_values": propagation.pruned_values,
                "residual_variable_count": propagation.residual_variable_count,
                "active_row_counts": propagation.active_row_counts,
            })
        });
        let system_analysis = compiled.system_analysis.as_ref().map(|analysis| {
            serde_json::json!({
                "residual_components": analysis.residual_components,
                "max_residual_component_width": analysis.max_residual_component_width,
            })
        });
        let demand_coverage = serde_json::json!({
            "anchor_count": compiled.demand_anchor_count,
            "fixed_direct_covered": compiled.fixed_direct_covered_demand_anchors,
            "fixed_closed_covered": compiled.fixed_closed_covered_demand_anchors,
            "maximal_direct_covered": compiled.maximal_direct_covered_demand_anchors,
            "maximal_closed_covered": compiled.maximal_closed_covered_demand_anchors,
            "fixed_uncovered": compiled.fixed_uncovered_demand_anchors,
            "direct_unsupported": compiled.direct_unsupported_demand_anchors,
            "distinct_direct_candidate_support_scope_count":
                compiled.distinct_direct_candidate_support_scope_count,
            "min_direct_candidate_support_count":
                compiled.min_direct_candidate_support_count,
            "max_direct_candidate_support_count":
                compiled.max_direct_candidate_support_count,
            "direct_coverage_clause_satisfying_assignments":
                compiled.direct_coverage_clause_satisfying_assignments,
        });
        let ordered_perimeter_scope_analyses = compiled
            .ordered_perimeter_scope_analyses
            .iter()
            .map(|analysis| {
                serde_json::json!({
                    "component_index": analysis.component_index,
                    "perimeter_point_count": analysis.perimeter_point_count,
                    "candidate_seed_count": analysis.candidate_seed_count,
                    "point_seed_incidences": analysis.point_seed_incidences,
                    "max_point_candidate_seed_count": analysis.max_point_candidate_seed_count,
                    "distinct_incidence_signature_count":
                        analysis.distinct_incidence_signature_count,
                    "max_incidence_signature_multiplicity":
                        analysis.max_incidence_signature_multiplicity,
                    "max_local_ring_face_count": analysis.max_local_ring_face_count,
                    "max_distinct_local_footprint_mask_count":
                        analysis.max_distinct_local_footprint_mask_count,
                    "max_local_union_state_count": analysis.max_local_union_state_count,
                    "projected_interface_face_count": analysis.projected_interface_face_count,
                    "projected_direct_union_state_cap":
                        analysis.projected_direct_union_state_cap,
                    "projected_direct_union_state_count":
                        analysis.projected_direct_union_state_count,
                    "projected_direct_union_state_cap_exceeded_after_variables":
                        analysis.projected_direct_union_state_cap_exceeded_after_variables,
                    "candidate_footprint_face_count":
                        analysis.candidate_footprint_face_count,
                    "candidate_footprint_union_state_count":
                        analysis.candidate_footprint_union_state_count,
                    "candidate_footprint_union_state_cap_exceeded_after_variables":
                        analysis.candidate_footprint_union_state_cap_exceeded_after_variables,
                    "closure_prefix_variable_count":
                        analysis.closure_prefix_variable_count,
                    "closure_prefix_assignment_count":
                        analysis.closure_prefix_assignment_count,
                    "closure_prefix_distinct_direct_mask_count":
                        analysis.closure_prefix_distinct_direct_mask_count,
                    "closure_prefix_distinct_closed_mask_count":
                        analysis.closure_prefix_distinct_closed_mask_count,
                    "closure_prefix_max_closed_mask_multiplicity":
                        analysis.closure_prefix_max_closed_mask_multiplicity,
                    "closure_incremental_prefix_parity":
                        analysis.closure_incremental_prefix_parity,
                    "best_cut_point": analysis.best_cut_point,
                    "min_linearized_frontier_width": analysis.min_linearized_frontier_width,
                })
            })
            .collect::<Vec<_>>();
        // Only populated when EARTHMESH_M0_LEGALIZATION_ASSIGNMENT_DUMP is set;
        // omitted from the report otherwise so existing evidence hashes stay
        // byte-identical.
        let assignment_outcome_records = (!compiled.assignment_outcome_records.is_empty())
            .then(|| {
                compiled
                    .assignment_outcome_records
                    .iter()
                    .map(|record| {
                        serde_json::json!({
                            "value_mask": record.value_mask,
                            "outcome": record.outcome,
                            "exact_state_ordinal": record.exact_state_ordinal,
                        })
                    })
                    .collect::<Vec<_>>()
            });
        let mut report = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_compiled_table_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "compiler_executable_sha256": earthmesh_project::file_content_hash(
                &std::env::current_exe().expect("current test executable")
            ).expect("compiler executable hash"),
            "cluster_index": cluster_index,
            "max_variables": max_variables,
            "candidate_seed_ids": compiled.candidate_seed_ids,
            "demand_coverage": demand_coverage,
            "status": status,
            "total_assignments": compiled.total_assignments,
            "evaluated_assignments": compiled.evaluated_assignments,
            "sat_assignments": compiled.sat_assignments,
            "boundary_incomplete_assignments": compiled.boundary_incomplete_assignments,
            "hard_rejected_assignments": compiled.hard_rejected_assignments,
            "exact_failure_assignments": compiled.exact_failure_assignments,
            "unclassified_error_assignments": compiled.unclassified_error_assignments,
            "first_unclassified_error": compiled.first_unclassified_error,
            "triplet_assignment_count": compiled.triplet_assignment_count,
            "distinct_exact_state_count": compiled.distinct_exact_state_count,
            "max_exact_state_multiplicity": compiled.max_exact_state_multiplicity,
            "mixed_exact_outcome_state_count": compiled.mixed_exact_outcome_state_count,
            "current_perimeter_scope_candidate_seed_ids":
                compiled.current_perimeter_scope_candidate_seed_ids,
            "covers_current_perimeter_scope": compiled.covers_current_perimeter_scope,
            "ordered_perimeter_scope_analyses": ordered_perimeter_scope_analyses,
            "propagation": propagation,
            "system_analysis": system_analysis,
        });
        if let Some(records) = assignment_outcome_records {
            report
                .as_object_mut()
                .expect("compiled-table report object")
                .insert(
                    "assignment_outcome_records".to_string(),
                    serde_json::Value::Array(records),
                );
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize compiled-table report"),
        )
        .expect("write compiled-table report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_nogood_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_NOGOOD_OUTPUT").expect("nogood output path"),
        );
        let cluster_index = std::env::var("EARTHMESH_M0_LEGALIZATION_PATCH_CLUSTER")
            .expect("patch cluster")
            .parse::<usize>()
            .expect("numeric patch cluster");
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let patch = checkpoint
            .preflight
            .patches
            .iter()
            .find(|patch| patch.cluster_index == cluster_index)
            .expect("checkpoint patch cluster");
        assert!(
            patch.candidate_seed_ids.len() <= 20,
            "nogood probe is intentionally bounded to at most 20 variables"
        );
        let nogood = m0_generalize_legalization_nogood(
            &checkpoint,
            patch,
            &patch.selected_candidate_seed_ids,
        )
        .expect("starting assignment must be an exactly classified failure");
        let mut full_counts = BTreeMap::<String, usize>::new();
        let mut pruned_counts = BTreeMap::<String, usize>::new();
        let mut skipped_assignments = 0usize;
        for value_mask in 0..(1usize << patch.candidate_seed_ids.len()) {
            let assignment = m0_assignment_from_value_mask(patch, value_mask);
            let label = |outcome| match outcome {
                M0LegalizationAssignmentOutcome::BoundaryIncomplete => {
                    "boundary_incomplete".to_string()
                }
                M0LegalizationAssignmentOutcome::Sat => "sat".to_string(),
                M0LegalizationAssignmentOutcome::HardRejected(kind) => {
                    format!("hard:{kind}")
                }
                M0LegalizationAssignmentOutcome::ExactFailure(kind) => {
                    format!("exact:{kind}")
                }
                M0LegalizationAssignmentOutcome::Unclassified(_) => "unclassified".to_string(),
            };
            *full_counts
                .entry(label(m0_evaluate_legalization_assignment(
                    &checkpoint,
                    patch,
                    &assignment,
                )))
                .or_default() += 1;
            let selected = assignment.iter().copied().collect::<BTreeSet<_>>();
            if nogood
                .fixed_seed_states
                .iter()
                .all(|(seed, value)| selected.contains(seed) == *value)
            {
                skipped_assignments += 1;
            } else {
                *pruned_counts
                    .entry(label(m0_evaluate_legalization_assignment(
                        &checkpoint,
                        patch,
                        &assignment,
                    )))
                    .or_default() += 1;
            }
        }
        for (kind, count) in &nogood.hard_rejected_assignments {
            *pruned_counts.entry(format!("hard:{kind}")).or_default() += count;
        }
        for (kind, count) in &nogood.exact_failure_counts {
            *pruned_counts.entry(format!("exact:{kind}")).or_default() += count;
        }
        assert_eq!(skipped_assignments, nogood.blocked_assignments);
        assert_eq!(pruned_counts, full_counts);
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_exact_nogood_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "enumerator_executable_sha256": earthmesh_project::file_content_hash(
                &std::env::current_exe().expect("current test executable")
            ).expect("enumerator executable hash"),
            "child_grid_number": checkpoint.child_grid_number,
            "max_mrows": checkpoint.max_mrows,
            "starting_assignment": &patch.selected_candidate_seed_ids,
            "nogood": nogood,
            "parity": {
                "full_assignment_counts": full_counts,
                "pruned_plus_proof_counts": pruned_counts,
                "skipped_assignments": skipped_assignments,
                "matches": true,
            },
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_phase_inventory_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_PHASE_INVENTORY_OUTPUT")
                .expect("phase inventory output path"),
        );
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        assert!(
            !checkpoint.selection.component_phases.is_empty(),
            "checkpoint predates component seed ownership diagnostics"
        );
        let mut seed_component = BTreeMap::new();
        for component in &checkpoint.selection.component_phases {
            for &seed in &component.legal_seed_ids {
                assert!(
                    seed_component
                        .insert(seed, component.component_index)
                        .is_none(),
                    "legal seed {seed} belongs to more than one demand component"
                );
            }
        }
        let mut affected_components = BTreeSet::new();
        let patches = checkpoint
            .preflight
            .patches
            .iter()
            .map(|patch| {
                let mut candidate_counts = BTreeMap::<usize, usize>::new();
                let mut selected_counts = BTreeMap::<usize, usize>::new();
                let mut unmapped_candidate_seed_ids = Vec::new();
                for &seed in &patch.candidate_seed_ids {
                    if let Some(&component) = seed_component.get(&seed) {
                        *candidate_counts.entry(component).or_default() += 1;
                        affected_components.insert(component);
                    } else {
                        unmapped_candidate_seed_ids.push(seed);
                    }
                }
                for &seed in &patch.selected_candidate_seed_ids {
                    if let Some(&component) = seed_component.get(&seed) {
                        *selected_counts.entry(component).or_default() += 1;
                    }
                }
                serde_json::json!({
                    "cluster_index": patch.cluster_index,
                    "candidate_seed_count": patch.candidate_seed_ids.len(),
                    "component_candidate_seed_counts": candidate_counts,
                    "component_selected_seed_counts": selected_counts,
                    "unmapped_candidate_seed_ids": unmapped_candidate_seed_ids,
                })
            })
            .collect::<Vec<_>>();
        let baseline_combination_count = checkpoint
            .selection
            .component_phases
            .iter()
            .filter(|component| affected_components.contains(&component.component_index))
            .try_fold(1usize, |count, component| {
                count.checked_mul(component.phase_class_count)
            });
        let affected = checkpoint
            .selection
            .component_phases
            .iter()
            .filter(|component| affected_components.contains(&component.component_index))
            .map(|component| {
                let point_json = |im: usize| {
                    let point = checkpoint.mesh.m_points[im];
                    serde_json::json!([point.x, point.y, point.z])
                };
                serde_json::json!({
                    "component_index": component.component_index,
                    "component_m_point_count": component.component_m_points.len(),
                    "component_anchor": point_json(component.demand_start),
                    "phase_class_count": component.phase_class_count,
                    "phase_anchors": component.phase_starts.iter()
                        .map(|&im| point_json(im))
                        .collect::<Vec<_>>(),
                    "selected_phase_ordinal": component.selected_phase_ordinal,
                    "legal_seed_count": component.legal_seed_ids.len(),
                    "selected_seed_count": component.selected_seed_ids.len(),
                })
            })
            .collect::<Vec<_>>();
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_phase_inventory",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "affected_component_count": affected.len(),
            "baseline_phase_combination_count": baseline_combination_count,
            "affected_components": affected,
            "patches": patches,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_perimeter_component_offset_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_PERIMETER_OFFSET_OUTPUT")
                .expect("perimeter offset output path"),
        );
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let affected_components = checkpoint
            .preflight
            .self_loop_witnesses
            .iter()
            .map(|witness| witness.perimeter_component)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let total_assignments = 3usize
            .checked_pow(
                u32::try_from(affected_components.len()).expect("component count fits u32"),
            )
            .expect("perimeter offset assignment count fits usize");
        let assignment_limit = std::env::var("EARTHMESH_M0_PERIMETER_OFFSET_MAX_ASSIGNMENTS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(total_assignments)
            .min(total_assignments);
        let mut failure_counts = BTreeMap::<&'static str, usize>::new();
        let mut first_failure_messages = BTreeMap::<&'static str, String>::new();
        let mut evaluated_assignments = 0usize;
        let mut sat_assignment = None;

        'search: for hamming_weight in 0..=affected_components.len() {
            for encoded in 0..total_assignments {
                let mut value = encoded;
                let mut offsets = vec![0usize; checkpoint.preflight.perimeter_lengths.len()];
                let mut assignment = Vec::new();
                for &component in &affected_components {
                    let offset = value % 3;
                    value /= 3;
                    offsets[component] = offset;
                    if offset != 0 {
                        assignment.push((component, offset));
                    }
                }
                if assignment.len() != hamming_weight {
                    continue;
                }
                if evaluated_assignments == assignment_limit {
                    break 'search;
                }
                evaluated_assignments += 1;
                match checkpoint
                    .mesh
                    .spawn_nest_pass_method_c_with_perimeter_component_offsets_for_diagnostics(
                        &checkpoint.preflight.prepared_selected_faces,
                        &offsets,
                        checkpoint.child_grid_number,
                        checkpoint.max_mrows,
                        true,
                    ) {
                    Ok(mesh) => {
                        mesh.validate_topology().expect("SAT topology");
                        sat_assignment = Some(assignment);
                        break 'search;
                    }
                    Err(error) => {
                        let kind = method_c_hfield_failure_kind(&error).as_str();
                        *failure_counts.entry(kind).or_default() += 1;
                        first_failure_messages
                            .entry(kind)
                            .or_insert_with(|| error.to_string());
                    }
                }
            }
        }
        let status = if sat_assignment.is_some() {
            "SAT"
        } else if evaluated_assignments == total_assignments {
            "PATCH_UNSAT"
        } else {
            "INCOMPLETE"
        };
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_perimeter_component_offset_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "status": status,
            "affected_perimeter_components": affected_components,
            "perimeter_component_count": checkpoint.preflight.perimeter_lengths.len(),
            "total_assignments": total_assignments,
            "assignment_limit": assignment_limit,
            "evaluated_assignments": evaluated_assignments,
            "failure_counts": failure_counts,
            "first_failure_messages": first_failure_messages,
            "sat_assignment": sat_assignment,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_all_legal_seeds_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_ALL_SEEDS_OUTPUT")
                .expect("all-seeds output path"),
        );
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let selected = checkpoint
            .mesh
            .selected_faces_from_method_c_seed_ids(&checkpoint.selection.legal_seed_ids)
            .expect("all legal seed footprints");
        checkpoint
            .selection
            .validate_demand_coverage(&selected)
            .expect("all legal seeds preserve hard coverage");
        let selected_face_count = selected.iter().skip(2).filter(|&&face| face).count();
        let report = match checkpoint.mesh.legalization_preflight_from_selected_faces(
            &selected,
            &checkpoint.selection.legal_seed_ids,
            &checkpoint.selection.legal_seed_ids,
            checkpoint.child_grid_number,
        ) {
            Ok(preflight) => {
                let offsets = vec![0usize; preflight.perimeter_lengths.len()];
                match checkpoint
                    .mesh
                    .spawn_nest_pass_method_c_with_perimeter_component_offsets_for_diagnostics(
                        &preflight.prepared_selected_faces,
                        &offsets,
                        checkpoint.child_grid_number,
                        checkpoint.max_mrows,
                        true,
                    ) {
                    Ok(mesh) => {
                        mesh.validate_topology().expect("all-seeds SAT topology");
                        serde_json::json!({
                            "kind": "earthmesh_method_c_all_legal_seeds_probe",
                            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                                .expect("checkpoint hash"),
                            "status": "SAT",
                            "legal_seed_count": checkpoint.selection.legal_seed_ids.len(),
                            "baseline_selected_seed_count": checkpoint.selection.selected_seed_ids.len(),
                            "selected_face_count": selected_face_count,
                            "prepared_selected_face_count": preflight.prepared_selected_faces
                                .iter().skip(2).filter(|&&face| face).count(),
                            "perimeter_lengths": preflight.perimeter_lengths,
                            "self_loop_witness_count": preflight.self_loop_witnesses.len(),
                            "child_counts": {"m": mesh.nmd, "u": mesh.nud, "w": mesh.nwd},
                        })
                    }
                    Err(error) => serde_json::json!({
                        "kind": "earthmesh_method_c_all_legal_seeds_probe",
                        "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                            .expect("checkpoint hash"),
                        "status": "exact_failure",
                        "legal_seed_count": checkpoint.selection.legal_seed_ids.len(),
                        "baseline_selected_seed_count": checkpoint.selection.selected_seed_ids.len(),
                        "selected_face_count": selected_face_count,
                        "prepared_selected_face_count": preflight.prepared_selected_faces
                            .iter().skip(2).filter(|&&face| face).count(),
                        "perimeter_lengths": preflight.perimeter_lengths,
                        "self_loop_witness_count": preflight.self_loop_witnesses.len(),
                        "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                        "error": error.to_string(),
                    }),
                }
            }
            Err(error) => serde_json::json!({
                "kind": "earthmesh_method_c_all_legal_seeds_probe",
                "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                    .expect("checkpoint hash"),
                "status": "preflight_failure",
                "legal_seed_count": checkpoint.selection.legal_seed_ids.len(),
                "baseline_selected_seed_count": checkpoint.selection.selected_seed_ids.len(),
                "selected_face_count": selected_face_count,
                "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                "error": error.to_string(),
            }),
        };
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_face_demand_dilation_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_DEMAND_DILATION_OUTPUT")
                .expect("demand dilation output path"),
        );
        let max_rings = std::env::var("EARTHMESH_M0_DEMAND_DILATION_MAX_RINGS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(6);
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let levels = m0_frozen_target_levels(&checkpoint);
        let mut demand = checkpoint.selection.face_demand.clone();
        let mut rings = Vec::new();
        for ring in 0..=max_rings {
            let mut ring_report = m0_evaluate_face_demand(&checkpoint, &levels, &demand);
            ring_report
                .as_object_mut()
                .expect("face-demand report object")
                .insert("ring".to_string(), serde_json::json!(ring));
            let sat = ring_report["status"] == "SAT";
            rings.push(ring_report);
            if sat || ring == max_rings {
                break;
            }
            let dilated = m0_dilate_face_demands_same_level(&checkpoint.mesh, &demand);
            if dilated == demand {
                break;
            }
            demand = dilated;
        }
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_face_demand_dilation_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "max_rings": max_rings,
            "rings": rings,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_component_dilation_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_COMPONENT_DILATION_OUTPUT")
                .expect("component dilation output path"),
        );
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let levels = m0_frozen_target_levels(&checkpoint);
        let baseline_demand = &checkpoint.selection.face_demand;
        let components = m0_face_demand_components_same_level(&checkpoint.mesh, baseline_demand);
        let baseline = m0_evaluate_face_demand(&checkpoint, &levels, baseline_demand);
        let mut variants = Vec::with_capacity(components.len());
        for (component_index, component) in components.iter().enumerate() {
            let trial =
                m0_dilate_one_face_demand_component(&checkpoint.mesh, baseline_demand, component);
            let added_face_count = trial
                .iter()
                .zip(baseline_demand)
                .skip(2)
                .filter(|(trial, baseline)| **trial && !**baseline)
                .count();
            let mut variant = m0_evaluate_face_demand(&checkpoint, &levels, &trial);
            let fields = variant
                .as_object_mut()
                .expect("component-dilation report object");
            fields.insert(
                "component_index".to_string(),
                serde_json::json!(component_index),
            );
            fields.insert(
                "component_first_face".to_string(),
                serde_json::json!(component[0]),
            );
            fields.insert(
                "component_face_count".to_string(),
                serde_json::json!(component.len()),
            );
            fields.insert(
                "added_face_count".to_string(),
                serde_json::json!(added_face_count),
            );
            variants.push(variant);
        }
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_component_dilation_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "component_count": components.len(),
            "baseline": baseline,
            "variants": variants,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_greedy_component_dilation_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_GREEDY_DILATION_OUTPUT")
                .expect("greedy dilation output path"),
        );
        let max_steps = std::env::var("EARTHMESH_M0_GREEDY_DILATION_MAX_STEPS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(4);
        let initial_component_faces =
            m0_usize_list_env("EARTHMESH_M0_GREEDY_DILATION_INITIAL_COMPONENT_FACES");
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let levels = m0_frozen_target_levels(&checkpoint);
        let mut demand = checkpoint.selection.face_demand.clone();
        for &face in &initial_component_faces {
            let components = m0_face_demand_components_same_level(&checkpoint.mesh, &demand);
            let component = components
                .iter()
                .find(|component| component.binary_search(&face).is_ok())
                .unwrap_or_else(|| panic!("initial face {face} is not in a demand component"));
            demand = m0_dilate_one_face_demand_component(&checkpoint.mesh, &demand, component);
        }
        let initial = m0_evaluate_face_demand(&checkpoint, &levels, &demand);
        let mut current = initial.clone();
        let mut steps = Vec::new();
        let mut terminal_status = "STEP_LIMIT";

        for step in 1..=max_steps {
            if current["status"] == "SAT" {
                terminal_status = "SAT";
                break;
            }
            let current_witnesses = current["self_loop_witness_count"]
                .as_u64()
                .map(|value| value as usize)
                .expect("greedy baseline must expose self-loop witnesses");
            let components = m0_face_demand_components_same_level(&checkpoint.mesh, &demand);
            let mut candidates = Vec::with_capacity(components.len());
            let mut best: Option<((usize, usize, usize), Vec<bool>, serde_json::Value)> = None;
            for (component_index, component) in components.iter().enumerate() {
                let trial =
                    m0_dilate_one_face_demand_component(&checkpoint.mesh, &demand, component);
                let added_face_count = trial
                    .iter()
                    .zip(&demand)
                    .skip(2)
                    .filter(|(trial, baseline)| **trial && !**baseline)
                    .count();
                let outcome = m0_evaluate_face_demand(&checkpoint, &levels, &trial);
                let witness_count = outcome["self_loop_witness_count"]
                    .as_u64()
                    .map(|value| value as usize);
                candidates.push(serde_json::json!({
                    "component_index": component_index,
                    "component_first_face": component[0],
                    "component_face_count": component.len(),
                    "added_face_count": added_face_count,
                    "status": outcome["status"],
                    "self_loop_witness_count": witness_count,
                    "failure_kind": outcome.get("failure_kind"),
                }));
                let admissible = outcome["status"] == "SAT"
                    || (outcome["status"] == "exact_failure"
                        && outcome["failure_kind"] == "transition_patch"
                        && witness_count.is_some_and(|count| count < current_witnesses));
                if !admissible {
                    continue;
                }
                let key = (witness_count.unwrap_or(0), added_face_count, component[0]);
                if best.as_ref().is_none_or(|(best_key, _, _)| key < *best_key) {
                    best = Some((key, trial, outcome));
                }
            }
            let Some((chosen_key, chosen_demand, chosen)) = best else {
                terminal_status = "LOCAL_MINIMUM";
                steps.push(serde_json::json!({
                    "step": step,
                    "component_count": components.len(),
                    "current_self_loop_witness_count": current_witnesses,
                    "chosen": null,
                    "candidates": candidates,
                }));
                break;
            };
            let added_face_count = chosen_demand
                .iter()
                .zip(&demand)
                .skip(2)
                .filter(|(trial, baseline)| **trial && !**baseline)
                .count();
            steps.push(serde_json::json!({
                "step": step,
                "component_count": components.len(),
                "current_self_loop_witness_count": current_witnesses,
                "chosen_added_face_count": added_face_count,
                "chosen_component_first_face": chosen_key.2,
                "chosen": chosen,
                "candidates": candidates,
            }));
            demand = chosen_demand;
            current = steps
                .last()
                .and_then(|step| step.get("chosen"))
                .cloned()
                .expect("chosen outcome");
            if current["status"] == "SAT" {
                terminal_status = "SAT";
                break;
            }
        }
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_greedy_component_dilation_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "max_steps": max_steps,
            "initial_component_faces": initial_component_faces,
            "terminal_status": terminal_status,
            "initial": initial,
            "final": current,
            "steps": steps,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_single_component_phase_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_PHASE_PROBE_OUTPUT")
                .expect("phase probe output path"),
        );
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let levels = m0_frozen_target_levels(&checkpoint);
        let target_level = |lon: f64, lat: f64| {
            *levels
                .get(&(lon.to_bits(), lat.to_bits()))
                .expect("selection sampled outside the frozen target-level coordinates")
        };
        let mut seed_component = BTreeMap::new();
        for component in &checkpoint.selection.component_phases {
            for &seed in &component.legal_seed_ids {
                assert!(
                    seed_component
                        .insert(seed, component.component_index)
                        .is_none(),
                    "legal seed {seed} belongs to more than one demand component"
                );
            }
        }
        let affected_components = checkpoint
            .preflight
            .patches
            .iter()
            .flat_map(|patch| &patch.candidate_seed_ids)
            .map(|seed| {
                *seed_component
                    .get(seed)
                    .expect("patch candidate seed has no demand component")
            })
            .collect::<BTreeSet<_>>();
        let mut variants = Vec::new();
        for component in checkpoint
            .selection
            .component_phases
            .iter()
            .filter(|component| affected_components.contains(&component.component_index))
        {
            let component_anchor = checkpoint.mesh.m_points[component.demand_start];
            for phase_ordinal in 1..component.phase_class_count {
                let phase_anchor = checkpoint.mesh.m_points[component.phase_starts[phase_ordinal]];
                let variant = format!(
                    "{}:{}:{}:{}:{}:{}:{}",
                    checkpoint.pass,
                    component_anchor.x,
                    component_anchor.y,
                    component_anchor.z,
                    phase_anchor.x,
                    phase_anchor.y,
                    phase_anchor.z,
                );
                std::env::set_var("EARTHMESH_M0_HFIELD_PHASE_VARIANT", variant);
                let selection = checkpoint
                    .mesh
                    .selection_checkpoint_from_target_levels_and_face_demands(
                        &target_level,
                        &checkpoint.selection.face_demand,
                        checkpoint.pass,
                        checkpoint.selection.preserve_all_demands,
                    );
                std::env::remove_var("EARTHMESH_M0_HFIELD_PHASE_VARIANT");
                let Ok(selection) = selection else {
                    let error = selection.expect_err("selection error");
                    variants.push(serde_json::json!({
                        "component_index": component.component_index,
                        "phase_ordinal": phase_ordinal,
                        "status": "selection_error",
                        "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                        "error": error.to_string(),
                    }));
                    continue;
                };
                let preflight = checkpoint.mesh.legalization_preflight_from_selected_faces(
                    &selection.selected_faces,
                    &selection.legal_seed_ids,
                    &selection.selected_seed_ids,
                    checkpoint.child_grid_number,
                );
                let Ok(preflight) = preflight else {
                    let error = preflight.expect_err("preflight error");
                    variants.push(serde_json::json!({
                        "component_index": component.component_index,
                        "phase_ordinal": phase_ordinal,
                        "status": "preflight_error",
                        "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                        "error": error.to_string(),
                    }));
                    continue;
                };
                let target_seeds = selection
                    .component_phases
                    .iter()
                    .find(|item| item.component_index == component.component_index)
                    .expect("variant target component")
                    .legal_seed_ids
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                let target_witnesses = preflight
                    .patches
                    .iter()
                    .filter(|patch| {
                        patch
                            .candidate_seed_ids
                            .iter()
                            .any(|seed| target_seeds.contains(seed))
                    })
                    .map(|patch| patch.witness_indices.len())
                    .sum::<usize>();
                let mutable_faces = checkpoint
                    .mesh
                    .selected_faces_from_method_c_seed_ids(&selection.legal_seed_ids)
                    .expect("variant legal seed footprints")
                    .iter()
                    .enumerate()
                    .skip(2)
                    .filter_map(|(iw, &selected)| selected.then_some(iw))
                    .collect::<Vec<_>>();
                let patch = MethodCHfieldLegalizationPatch {
                    cluster_index: usize::MAX,
                    witness_indices: Vec::new(),
                    witness_perimeter_components: Vec::new(),
                    perimeter_components: Vec::new(),
                    perimeter_interfaces: Vec::new(),
                    dependency_faces: Vec::new(),
                    dependency_face_lineages: Vec::new(),
                    candidate_seed_lineages: Vec::new(),
                    selected_candidate_seed_ids: selection.selected_seed_ids.clone(),
                    candidate_seed_ids: selection.legal_seed_ids.clone(),
                    mutable_faces,
                    mutable_face_lineages: Vec::new(),
                };
                match checkpoint.mesh.legalization_patch_boundary_check(
                    &selection,
                    &preflight,
                    &patch,
                    &selection.selected_seed_ids,
                    checkpoint.child_grid_number,
                    checkpoint.max_mrows,
                ) {
                    Ok(check) => variants.push(serde_json::json!({
                        "component_index": component.component_index,
                        "phase_ordinal": phase_ordinal,
                        "status": if check.exact_materializable { "SAT" } else { "exact_failure" },
                        "selected_seed_count": selection.selected_seed_ids.len(),
                        "self_loop_witness_count": preflight.self_loop_witnesses.len(),
                        "target_component_witness_count": target_witnesses,
                        "exact_failure_kind": check.exact_failure_kind.map(|kind| kind.as_str()),
                        "exact_failure_message": check.exact_failure_message,
                    })),
                    Err(error) => variants.push(serde_json::json!({
                        "component_index": component.component_index,
                        "phase_ordinal": phase_ordinal,
                        "status": "hard_rejected",
                        "failure_kind": method_c_hfield_failure_kind(&error).as_str(),
                        "error": error.to_string(),
                    })),
                }
            }
        }
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_single_component_phase_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "variants": variants,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

    #[test]
    #[ignore = "requires a saved Method-C legalization checkpoint"]
    fn m0_legalization_checkpoint_first_toggle_boundary_probe() {
        let input = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_CHECKPOINT_INPUT")
                .expect("checkpoint input path"),
        );
        let output = PathBuf::from(
            std::env::var("EARTHMESH_M0_LEGALIZATION_BOUNDARY_PROBE_OUTPUT")
                .expect("boundary probe output path"),
        );
        let checkpoint = serde_json::from_slice::<M0MethodCLegalizationCheckpoint>(
            &fs::read(&input).expect("read checkpoint"),
        )
        .expect("parse checkpoint");
        let mut patches = Vec::new();
        for patch in &checkpoint.preflight.patches {
            let seed = *patch
                .candidate_seed_ids
                .first()
                .expect("patch candidate seed");
            let mut assignment = patch.selected_candidate_seed_ids.clone();
            let action = if let Ok(index) = assignment.binary_search(&seed) {
                assignment.remove(index);
                "remove"
            } else {
                assignment.push(seed);
                assignment.sort_unstable();
                "add"
            };
            let result = checkpoint.mesh.legalization_patch_boundary_check(
                &checkpoint.selection,
                &checkpoint.preflight,
                patch,
                &assignment,
                checkpoint.child_grid_number,
                checkpoint.max_mrows,
            );
            patches.push(match result {
                Ok(boundary) => serde_json::json!({
                    "cluster_index": patch.cluster_index,
                    "seed": seed,
                    "action": action,
                    "status": if boundary.is_closed() { "closed" } else { "incomplete" },
                    "outside_changed_face_count": boundary.outside_changed_faces.len(),
                    "outside_changed_faces": boundary.outside_changed_faces,
                    "outside_perimeter_interface_changed": boundary.outside_perimeter_interface_changed,
                    "perimeter_lengths": boundary.perimeter_lengths,
                    "vertex_only_contact_count": boundary.vertex_only_contact_count,
                    "predicted_transition_self_loop_count": boundary.predicted_transition_self_loop_count,
                    "exact_materializable": boundary.exact_materializable,
                    "exact_failure_kind": boundary.exact_failure_kind.map(|kind| kind.as_str()),
                    "exact_failure_message": boundary.exact_failure_message,
                }),
                Err(error) => serde_json::json!({
                    "cluster_index": patch.cluster_index,
                    "seed": seed,
                    "action": action,
                    "status": "candidate_invalid",
                    "error": error.to_string(),
                }),
            });
        }
        let report = serde_json::json!({
            "kind": "earthmesh_method_c_legalization_first_toggle_boundary_probe",
            "checkpoint_sha256": earthmesh_project::file_content_hash(&input)
                .expect("checkpoint hash"),
            "patches": patches,
        });
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).expect("create output directory");
        }
        fs::write(
            &output,
            serde_json::to_vec_pretty(&report).expect("serialize report"),
        )
        .expect("write report");
        let hash = earthmesh_project::file_content_hash(&output).expect("report hash");
        fs::write(
            m0_sidecar_path(&output, ".sha256").expect("report sidecar path"),
            format!("{hash}\n"),
        )
        .expect("write report hash");
    }

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

    #[test]
    fn method_c_topology_gradation_uses_active_transition_clearance() {
        let mut refine = RefineConfig::default();
        assert_eq!(method_c_topology_gradation_g(&refine, 2, 0.2, false), 0.2);
        assert_eq!(method_c_topology_gradation_g(&refine, 1, 0.2, true), 0.2);
        assert_eq!(method_c_topology_gradation_g(&refine, 2, 0.2, true), 0.0625);
        assert_eq!(method_c_topology_gradation_g(&refine, 2, 0.05, true), 0.05);

        refine.max_transition_row[1] = 5;
        refine.max_transition_row[2] = 8;
        assert_eq!(method_c_topology_gradation_g(&refine, 2, 0.2, true), 0.05);
        assert_eq!(
            method_c_topology_gradation_g(&refine, 3, 0.2, true),
            0.03125
        );
    }

    #[test]
    fn hfield_raster_resolution_warning_reports_only_underresolved_bins() {
        let mut field = earthmesh_hfield::HField::uniform(360, 180, 200_000.0).unwrap();
        assert_eq!(hfield_raster_resolution_warning(&field), None);

        field.set(180, 90, 25_000.0);
        let warning = hfield_raster_resolution_warning(&field).unwrap();
        assert!(warning.contains("under-resolves 1/64800 bins"));
        assert!(warning.contains("required <=1"));
    }

    fn refinement_stats_fixture() -> (UnstructuredMesh, Vec<i32>, Vec<i32>) {
        let mesh = UnstructuredMesh {
            m_points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 0.2, lat: 0.2 },
                LonLatPoint {
                    lon: 20.2,
                    lat: 20.2,
                },
            ],
            w_points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 0.1, lat: 0.1 },
                LonLatPoint { lon: 0.3, lat: 0.1 },
                LonLatPoint { lon: 0.2, lat: 0.3 },
                LonLatPoint {
                    lon: 20.1,
                    lat: 20.1,
                },
                LonLatPoint {
                    lon: 20.3,
                    lat: 20.1,
                },
                LonLatPoint {
                    lon: 20.2,
                    lat: 20.3,
                },
            ],
            m_to_w: vec![[1, 1, 1], [2, 3, 4], [5, 6, 7]],
            w_to_m: vec![
                vec![1],
                vec![2],
                vec![2],
                vec![2],
                vec![3],
                vec![3],
                vec![3],
            ],
            n_w_to_m: vec![1; 7],
        };
        (mesh, vec![0, 2, 7], vec![0, 1, 3, 0, 0, 4, 0])
    }

    #[test]
    fn final_output_refinement_stats_selects_cells_for_tri_and_hex() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_final_refine_stats_modes_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        let output = root.join("gridfile.nc4");
        let (mesh, m_levels, w_levels) = refinement_stats_fixture();
        crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &output,
            &mesh,
            MethodCGridfileMetadataSlices {
                m_refine_level: Some(&m_levels),
                w_refine_level: Some(&w_levels),
                ..Default::default()
            },
        )
        .expect("write final output");

        assert_eq!(
            final_output_refinement_stats(&output, "tri").expect("tri statistics"),
            (7, 2)
        );
        assert_eq!(
            final_output_refinement_stats(&output, "hex").expect("hex statistics"),
            (4, 3)
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn final_output_refinement_stats_exclude_cells_removed_by_regional_crop() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cli_final_refine_stats_crop_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        let input = root.join("global.nc4");
        let output = root.join("regional.nc4");
        let (mesh, m_levels, w_levels) = refinement_stats_fixture();
        crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &input,
            &mesh,
            MethodCGridfileMetadataSlices {
                m_refine_level: Some(&m_levels),
                w_refine_level: Some(&w_levels),
                ..Default::default()
            },
        )
        .expect("write global input");
        let kept = crate::write_regional_gridfile_with_refine_levels(
            &input,
            &output,
            &GridRegion::Bbox {
                west: -1.0,
                east: 1.0,
                north: 1.0,
                south: -1.0,
            },
            "tri",
            None,
            None,
        )
        .expect("crop final output");

        assert_eq!(kept, 1);
        assert_eq!(
            final_output_refinement_stats(&output, "tri").expect("cropped statistics"),
            (2, 1),
            "the removed level-7 cell must not remain in final statistics"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
