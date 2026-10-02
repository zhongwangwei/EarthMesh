//! CMRC's own pipeline: build the certified mother grid, coarsen it against
//! the requirement plan, publish the domain or MPAS delivery.
//!
//! Moved out of `global_source.rs` unchanged (layering step 6a). It still runs
//! as its own pipeline -- dispatched before the shared source-mesh setup --
//! and delivering through the shared result is step 6b.

use crate::atomic_output::publish_artifacts;
use crate::certified_options::{CertifiedDelivery, CertifiedMode, CertifiedRunOptions};
use crate::fvcom_mesh_2dm_output_path;
use crate::gridfile_mesh_from_one_based_state;
use crate::mkgrd_run_types::CertifiedRunRecord;
use crate::native_grid_refinement_requested;
use crate::read_method_c_calculated_refinement_regions;
use crate::read_method_c_domain_region;
use crate::read_method_c_specified_refinement_regions;
use crate::GridRegion;
use crate::GridfileMetadataSlices;
use crate::RefinePipelineRunReport;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_mesh::{MeshState, RefinementRegion};

use crate::write_clean_regional_ocean_gridfile;

use super::global_source::*;

pub(super) fn cmrc_timing_enabled() -> bool {
    std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1")
}

pub(super) fn log_cmrc_phase(enabled: bool, phase: &str, started: &mut Instant) {
    if enabled {
        eprintln!(
            "earthmesh_cli: cmrc_timing phase={phase} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        *started = Instant::now();
    }
}

pub(super) fn certified_gridfile_refine_levels(
    mesh: &crate::UnstructuredMesh,
    delivered_levels: &[usize],
) -> io::Result<(Vec<i32>, Vec<i32>)> {
    let w_has_placeholders =
        crate::unstructured_mesh_support::mesh_w_has_two_placeholder_rows(mesh);
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
        crate::unstructured_mesh_support::mesh_m_has_two_placeholder_rows(mesh);
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
pub(super) fn certified_gridfile_pre_export_lineages(
    mesh: &crate::UnstructuredMesh,
) -> (Vec<i64>, Vec<i64>) {
    let m_has_placeholders =
        crate::unstructured_mesh_support::mesh_m_has_two_placeholder_rows(mesh);
    let w_has_placeholders =
        crate::unstructured_mesh_support::mesh_w_has_two_placeholder_rows(mesh);
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

pub(super) struct CertifiedConstruction {
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

pub(super) fn certified_subdivision(base_nxp: usize, level: usize) -> io::Result<usize> {
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

pub(super) fn build_certified_construction(
    base_nxp: usize,
    chosen_level: usize,
    options: &CertifiedRunOptions,
    raster_requirements: &earthmesh_refine_certified::RasterLevelField,
    max_tris: usize,
    local_update_path: Option<&Path>,
    fixed_topology: bool,
) -> io::Result<CertifiedConstruction> {
    let budget = options.maximum_cells.min(max_tris);
    let mixed_requirement = chosen_level > 0
        && raster_requirements
            .levels()
            .iter()
            .any(|&level| level < chosen_level);
    // Moved mothers: the vertices of a certified mother move toward the
    // demand, the connectivity stays, so no heptagon appears.
    let moved = match options.mode {
        CertifiedMode::StretchedMother => Some(
            earthmesh_refine_certified::stretched_certified_mother(
                base_nxp,
                chosen_level,
                raster_requirements,
                options.angle_contract,
                budget,
            )
            .map(|stretched| earthmesh_refine_certified::AdaptedMother {
                summary: format!(
                    "stretched mother n={} (factor {:.2} toward {:.3}E {:.3}N)",
                    stretched.subdivision,
                    stretched.factor,
                    stretched.focus_lonlat.0,
                    stretched.focus_lonlat.1
                ),
                geometry: stretched.geometry,
                subdivision: stretched.subdivision,
                strategy: "schmidt_stretch",
                delivered_levels: stretched.delivered_levels,
                final_requirements: stretched.final_requirements,
                rejected: stretched.rejected,
            }),
        ),
        CertifiedMode::EquidistributedMother => {
            Some(earthmesh_refine_certified::adapted_certified_mother(
                base_nxp,
                chosen_level,
                raster_requirements,
                options.angle_contract,
                budget,
            ))
        }
        _ => None,
    };
    match moved {
        Some(Ok(adapted)) => {
            for reason in &adapted.rejected {
                eprintln!("earthmesh_cli: CMRC moved mother: set aside {reason}");
            }
            let safe_subdivision = certified_subdivision(base_nxp, chosen_level)?;
            eprintln!(
                "earthmesh_cli: CMRC moved mother: {} serves level {chosen_level}; the safe \
                 mother would be n={safe_subdivision} ({:.2}x the cells)",
                adapted.summary,
                (safe_subdivision as f64 / adapted.subdivision as f64).powi(2),
            );
            let geometry = Box::new(adapted.geometry);
            let cell_count = geometry.primal().vertex_count();
            let face_count = geometry.primal().triangle_count();
            let pentagons = certified_mother_pentagons(geometry.primal())?;
            // Only vertices moved: every cell is its mother cell, so the
            // lineage is the identity.
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
                final_cell_requirements: Some(adapted.final_requirements),
                delivered_level: adapted.delivered_levels.iter().copied().max().unwrap_or(0),
                delivered_levels: adapted.delivered_levels,
                coarsening_strategy: adapted.strategy,
                initial_subdivision: adapted.subdivision,
                final_subdivision: adapted.subdivision,
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
        // No moved mother serves a scattered demand: c09's global DEM roughness
        // has none below the safe mother even searched one subdivision at a
        // time (guide 11.101). Fixed topology leaves nothing between them but
        // a grid the demand never asked for -- 64,002 cells at level 1 where
        // coarsening the same mother keeps 33,622. So where a 5/7 pair may
        // stand, which is every target but ICON, the safe mother is coarsened
        // where the demand allows, as reverse coarsening does.
        Some(Err(reasons)) if mixed_requirement && !fixed_topology => {
            eprintln!(
                "earthmesh_cli: warning: CMRC moved mother: none passes the final certificates; \
                 coarsening the safe mother where the demand allows instead, so the grid has \
                 vertices of degree 5 and 7 (an ICON delivery keeps the safe mother). {}",
                reasons.join("; ")
            );
            return build_mixed_certified_construction(
                base_nxp,
                chosen_level,
                options,
                raster_requirements,
                budget,
                local_update_path,
            );
        }
        Some(Err(reasons)) => {
            eprintln!(
                "earthmesh_cli: CMRC moved mother: none passes the final certificates; \
                 delivering the safe mother. {}",
                reasons.join("; ")
            );
        }
        None => {}
    }
    if matches!(
        options.mode,
        CertifiedMode::SafeMotherOnly
            | CertifiedMode::StretchedMother
            | CertifiedMode::EquidistributedMother
    ) {
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

    if mixed_requirement {
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
            let geometry = match earthmesh_refine_certified::certify_mother_grid_with_contract(
                mesh,
                options.angle_contract,
            ) {
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

pub(super) fn build_mixed_certified_construction(
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
    let mut result = match epoch {
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
    let handed_on_remap = result.final_remap.take();
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
    } else if let Some(remap) = handed_on_remap.filter(|remap| remap.joins(&initial_mesh, &mesh)) {
        // The last committed component certified this very remap; computing it
        // again cost as much as all the components' own (guide 11.103).
        remap
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

pub(super) fn elastic_outcome_name(
    outcome: earthmesh_refine_certified::coarsen::ComponentOutcomeKind,
) -> &'static str {
    outcome.as_str()
}

pub(super) fn elastic_report_json(
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

pub(super) struct CertifiedMpasPublication {
    report: crate::MpasFullMeshPipelineReport,
    mesh_density_min: f64,
    mesh_density_max: f64,
    step: usize,
}

pub(super) fn certified_mpas_cellwidth(
    base_nxp: usize,
    w_refine_levels: &[i32],
) -> io::Result<Vec<f64>> {
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

pub(super) fn publish_certified_atmos_mpas(
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
pub(super) fn publish_certified_domain_gridfile(
    source_gridfile: &Path,
    output_gridfile: &Path,
    config: &EarthmeshConfig,
    base_nxp: usize,
    workdir: &Path,
    domain_region: Option<&GridRegion>,
    angle_contract: earthmesh_refine_certified::AngleContractId,
    fvcom_output: Option<&Path>,
) -> io::Result<CertifiedDomainPublication> {
    let mode_grid = config.mode_grid.trim();
    let mesh_type = config.mesh_type.trim();
    let (kept_cells, fvcom_2dm) = if let (Some(region), "earthmesh" | "atmos" | "atmosmesh") =
        (domain_region, mesh_type)
    {
        if mode_grid == "hex" {
            return super::cmrc_region::publish_regional_hex(
                source_gridfile,
                output_gridfile,
                None,
                region,
                workdir,
            );
        }
        let kept = crate::regional_gridfile_writers::write_regional_gridfile(
            source_gridfile,
            output_gridfile,
            region,
            mode_grid,
        )?;
        (Some(kept), None)
    } else {
        let gridnum_perdegree = crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(
            Path::new(config.landtype_file.trim()),
        )?;
        if let (Some(region), "landmesh", "hex") = (domain_region, mesh_type, mode_grid) {
            return super::cmrc_region::publish_regional_hex(
                source_gridfile,
                output_gridfile,
                Some((Path::new(config.landtype_file.trim()), gridnum_perdegree)),
                region,
                workdir,
            );
        }
        let clean_close = match (domain_region, mesh_type, mode_grid) {
            (Some(GridRegion::Close { points }), "oceanmesh", "tri") => Some(points.as_slice()),
            _ => None,
        };

        if let Some(close_points) = clean_close {
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
            // The boundary context travels in the gridfile; FVCOM reads it there.
            let fvcom = fvcom_output
                .map(|output| {
                    crate::regional_gridfile_writers::write_fvcom_from_final_gridfile(
                        output_gridfile,
                        output,
                    )
                })
                .transpose()?;
            (None, fvcom)
        } else if let (Some(region), "landmesh", "tri") = (domain_region, mesh_type, mode_grid) {
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
            // Not an empty boundary list assumed here: the carve records
            // whether its boundary is coastline only, and a bounded mesh
            // without that record is refused rather than exported as all wall.
            let fvcom = fvcom_output
                .map(|output| {
                    crate::regional_gridfile_writers::write_fvcom_from_final_gridfile(
                        output_gridfile,
                        output,
                    )
                })
                .transpose()?;
            (Some(kept), fvcom)
        }
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
    if domain_region.is_some() && mesh_type != "oceanmesh" {
        crate::regional_gridfile_writers::lineage::verify_whole_triangle_lineage(
            source_gridfile,
            output_gridfile,
            &quality_mesh,
        )?;
    }
    let quality_input = match mode_grid {
        "hex" => crate::grid_quality_pipeline::quality_input_from_gridfile_hex(&quality_mesh)?,
        _ => crate::grid_quality_pipeline::quality_input_from_gridfile(&quality_mesh)?,
    };
    let quality_report = earthmesh_quality::compute(
        &quality_input,
        &earthmesh_quality::QualityThresholds::default(),
    );
    let published_minimum = quality_report.geometry.min_angle_deg;
    let published_maximum = quality_report.geometry.max_angle_deg;
    let delivery_window =
        earthmesh_refine_certified::AngleContract::for_id(angle_contract).final_delivery;
    if !published_minimum.is_finite() || !published_maximum.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "CMRC published domain has non-finite cell angles",
        ));
    }
    if mode_grid == "tri" && !delivery_window.contains_range(published_minimum, published_maximum) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CMRC published domain angle contract failed: [{published_minimum}, {published_maximum}] is outside [{}, {}]",
                delivery_window.minimum_degrees, delivery_window.maximum_degrees
            ),
        ));
    }
    if mode_grid == "hex"
        && (quality_report.geometry.non_finite_cell_count > 0
            || quality_report.geometry.invalid_polygon_count > 0
            || quality_report.geometry.zero_area_cell_count > 0
            || quality_report.geometry.negative_area_cell_count > 0
            || quality_report.geometry.self_intersection_count > 0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "CMRC published Hex domain contains invalid geometry",
        ));
    }
    let component_count = earthmesh_quality::topology::connected_component_count(&quality_input);
    let mut quality_issues =
        earthmesh_quality::topology::MeshTopologyValidator::new(&quality_input).validate_all();
    if mesh_type == "landmesh" || (domain_region.is_some() && mesh_type != "oceanmesh") {
        // As for regional dual cells, retain islands (including one-cell islands)
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
            "cell_view": mode_grid,
            "cells": quality_report.geometry.cell_count,
            "minimum_angle_deg": published_minimum,
            "maximum_angle_deg": published_maximum,
            "contract_minimum_deg": (mode_grid == "tri").then_some(delivery_window.minimum_degrees),
            "contract_maximum_deg": (mode_grid == "tri").then_some(delivery_window.maximum_degrees),
            "contract_pass": (mode_grid == "tri").then_some(true),
        }),
        fvcom_2dm,
    })
}

pub(super) fn validate_local_update_mode(
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

/// Everything CMRC's construction hands to its delivery: the certified mesh and
/// its dual, the delivered levels, and the record of how they were reached.
pub(super) struct CertifiedRefinement {
    options: CertifiedRunOptions,
    regional_domain: Option<GridRegion>,
    is_surface_masked: bool,
    is_domain_export: bool,
    regional_whole_cells: bool,
    refine: RefineConfig,
    base_nxp: usize,
    requirements: CertifiedRequirementPlan,
    chosen_level: usize,
    delivered_level: usize,
    delivered_levels: earthmesh_refine_certified::TargetLevelField,
    coarsening_strategy: &'static str,
    initial_subdivision: usize,
    subdivision: usize,
    initial_cells: usize,
    attempted_patches: usize,
    accepted_patches: usize,
    removed_vertices: usize,
    removed_faces: usize,
    search_budget_exhausted: bool,
    elastic_report: Option<earthmesh_refine_certified::coarsen::ElasticCmrcReport>,
    local_update: Option<serde_json::Value>,
    final_mesh: Box<earthmesh_refine_certified::CertifiedPrimalDualMesh>,
    fulfillment: Box<earthmesh_refine_certified::AdaptivityFulfillmentReport>,
    product_outcome: &'static str,
    fallback_reason: Option<String>,
    safe_fallback: bool,
    remap: earthmesh_refine_certified::remap::ConservativeRemap,
    pentagons: [usize; 12],
    state: earthmesh_mesh::VoronoiGridState,
    started: Instant,
    timing_enabled: bool,
    phase_started: Instant,
}

/// CMRC's construction in the shape the shared dispatch hands the tail: the
/// published mesh and its per-cell levels as the `RefinedGrid`, the rest of the
/// construction record in `diagnostics.certified` for certified delivery.
pub(super) fn refine_with_certified_as_grid(
    contents: &str,
    config: &EarthmeshConfig,
    options: CertifiedRunOptions,
    workdir: &Path,
    max_tris: usize,
    output_dir: Option<&Path>,
) -> io::Result<(RefinedGrid, TailInputs)> {
    let (refinement, output_mesh) = refine_with_certified(contents, config, options, max_tris)?;
    let configured_dir = PathBuf::from(config.file_dir());
    let file_dir = if let Some(directory) = output_dir {
        directory.to_path_buf()
    } else if configured_dir.is_absolute() {
        configured_dir
    } else {
        workdir.join(configured_dir)
    };
    let (m, w) =
        certified_gridfile_refine_levels(&output_mesh, refinement.delivered_levels.levels())?;
    let inputs = TailInputs {
        refine: refinement.refine.clone(),
        max_level: refinement.chosen_level,
        native_cartesian_xy: false,
        domain_region: refinement.regional_domain.clone(),
        gridinit: None,
        regions: refinement.requirements.regions.clone(),
        nxp: refinement.base_nxp,
        spring_nest_iterations: 0,
        file_dir,
    };
    let refined = RefinedGrid {
        output_mesh,
        cell_levels: Some(CellRefineLevels { m, w }),
        pentagon_indices: refinement.pentagons,
        demand: RefinedDemandRecord::default(),
        diagnostics: BackendDiagnostics {
            certified: Some(Box::new(refinement)),
            ..Default::default()
        },
    };
    Ok((refined, inputs))
}

/// CMRC's construction: requirement plan, certified mother grid, reverse
/// coarsening, final certification and the published dual.
pub(super) fn refine_with_certified(
    contents: &str,
    config: &EarthmeshConfig,
    options: CertifiedRunOptions,
    max_tris: usize,
) -> io::Result<(CertifiedRefinement, crate::UnstructuredMesh)> {
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
    let is_surface_masked = matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh");
    let is_domain_export = is_surface_masked || regional_domain.is_some();
    let regional_whole_cells = matches!(
        config.mesh_type.trim(),
        "earthmesh" | "atmos" | "atmosmesh" | "landmesh"
    ) && matches!(config.mode_grid.trim(), "hex" | "tri")
        && matches!(
            regional_domain,
            Some(
                GridRegion::Bbox { .. }
                    | GridRegion::Circle { .. }
                    | GridRegion::Close { .. }
                    | GridRegion::Any(_)
            )
        );
    if regional_domain.is_some()
        && !regional_whole_cells
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
            "CMRC regional publication supports {earthmesh,atmos,atmosmesh,landmesh}/{hex,tri} with bbox, circle or close regions, or oceanmesh/tri with a single close polygon only"));
    }
    if is_surface_masked
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
    let requirements = if config.refine {
        certified_requirement_plan(
            contents,
            config,
            &refine,
            base_nxp,
            specified_level,
            calculated_level,
            regional_domain.as_ref(),
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
    // Without any source, a uniform requirement is reverse coarsening's own
    // complete-hierarchy epoch (level 1 back to 0, with its lineage); with
    // sources that asked for nothing, it is an answer.
    let options = if config.refine && chosen_level == 0 && requirements.sourced {
        requirements.nothing_requested();
        CertifiedRunOptions {
            mode: CertifiedMode::SafeMotherOnly,
            ..options
        }
    } else {
        options
    };
    if options.mode != CertifiedMode::SafeMotherOnly {
        for (index, region) in requirements.regions.iter().enumerate() {
            let requested = region.level();
            if requested == 0 {
                continue;
            }
            let sampled = (0..requirement_nlat).any(|j| {
                let lat = -90.0 + (j as f64 + 0.5) * 180.0 / requirement_nlat as f64;
                (0..requirement_nlon).any(|i| {
                    let lon = -180.0 + (i as f64 + 0.5) * 360.0 / requirement_nlon as f64;
                    region.contains_lonlat_canonical(earthmesh_mesh::LonLatDegrees::new(lon, lat))
                        && required_levels[j * requirement_nlon + i] >= requested
                })
            });
            if !sampled {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "CMRC CriterionNotCertifiable: refinement region {index} at level {requested} has no qualifying HField sample centers on the {requirement_nlon}x{requirement_nlat} requirement raster"
                    ),
                ));
            }
        }
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
        // ICON takes no 5/7 pair: a closed refined sphere needs them.
        config.output_format.trim().eq_ignore_ascii_case("ICON"),
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
            // A moved-mother run that fell back delivered the safe mother.
            options.mode == CertifiedMode::SafeMotherOnly
                || (matches!(
                    options.mode,
                    CertifiedMode::StretchedMother | CertifiedMode::EquidistributedMother
                ) && !matches!(coarsening_strategy, "schmidt_stretch" | "equidistribution")),
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
    let refinement = CertifiedRefinement {
        options,
        regional_domain,
        is_surface_masked,
        is_domain_export,
        regional_whole_cells,
        refine,
        base_nxp,
        requirements,
        chosen_level,
        delivered_level,
        delivered_levels,
        coarsening_strategy,
        initial_subdivision,
        subdivision,
        initial_cells,
        attempted_patches,
        accepted_patches,
        removed_vertices,
        removed_faces,
        search_budget_exhausted,
        elastic_report,
        local_update,
        final_mesh,
        fulfillment,
        product_outcome,
        fallback_reason,
        safe_fallback,
        remap,
        pentagons,
        state,
        started,
        timing_enabled,
        phase_started,
    };
    Ok((refinement, output_mesh))
}

/// CMRC's delivery: gridfile(s), remap table, certificate, manifest and model
/// exports, staged and published atomically.
pub(super) fn deliver_certified(
    refinement: CertifiedRefinement,
    output_mesh: crate::UnstructuredMesh,
    cell_levels: CellRefineLevels,
    realized: RealizedResolution,
    config: &EarthmeshConfig,
    file_dir: &Path,
) -> io::Result<RefinePipelineRunReport> {
    let CertifiedRefinement {
        options,
        regional_domain,
        is_surface_masked,
        is_domain_export,
        regional_whole_cells,
        refine,
        base_nxp,
        requirements,
        chosen_level,
        delivered_level,
        delivered_levels,
        coarsening_strategy,
        initial_subdivision,
        subdivision,
        initial_cells,
        attempted_patches,
        accepted_patches,
        removed_vertices,
        removed_faces,
        search_budget_exhausted,
        elastic_report,
        local_update,
        final_mesh,
        fulfillment,
        product_outcome,
        fallback_reason,
        safe_fallback,
        remap,
        pentagons,
        state,
        started,
        timing_enabled,
        mut phase_started,
    } = refinement;
    let requested_view = config.mode_grid.trim();
    let required_levels = &requirements.effective_levels;
    let requirement_nlon = requirements.nlon;
    let requirement_nlat = requirements.nlat;
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
        CertifiedMode::StretchedMother => "stretched_mother",
        CertifiedMode::EquidistributedMother => "equidistributed_mother",
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
    // Only whole regional HEX publication proves dual-cell lineage. Neither
    // TRI output nor legacy global surface masks carry that dual-view proof.
    certificate_document["published_grid_is_certified_dual_cell_subset"] =
        if is_domain_export && requested_view == "hex" && !regional_whole_cells {
            serde_json::Value::Null
        } else {
            (regional_whole_cells && requested_view == "hex").into()
        };
    certificate_document["requirement_layers"] = requirement_layers.clone();
    certificate_document["physical_balance_domain_scope"] =
        serde_json::Value::from(if is_domain_export {
            "pre_export_closed_sphere"
        } else {
            "published_grid"
        });
    certificate_document["published_grid_lineage_scope"] =
        serde_json::Value::from(if is_domain_export {
            "pre_export_closed_sphere_canonical_ids"
        } else {
            "not_emitted"
        });
    let fvcom_output_path = (!config.defer_model_exports
        && is_domain_export
        && config.mesh_type.trim() == "oceanmesh"
        && config.mode_grid.trim() == "tri"
        && config.output_format.trim().eq_ignore_ascii_case("FVCOM"))
    .then(|| fvcom_mesh_2dm_output_path(file_dir));
    let temporary_fvcom_path =
        result_dir.join(format!(".fvcom.2dm.cmrc-tmp-{}", std::process::id()));
    let mpas_suffix = if safe_fallback {
        "_certified_safe_fallback"
    } else {
        ""
    };
    let mpas_output_paths = (!config.defer_model_exports
        && !is_domain_export
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
        "published_grid_remap_status": if is_surface_masked { "not_available_after_landtype_subset" } else if is_domain_export { "not_available_after_regional_subset" } else { "certified" },
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
    let CellRefineLevels {
        m: m_refine_levels,
        w: w_refine_levels,
    } = cell_levels;
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
            region_center_retention,
            fvcom_2dm,
        ) = if is_domain_export {
            let (m_pre_export_lineage, w_pre_export_lineage) =
                certified_gridfile_pre_export_lineages(&output_mesh);
            let requested_center_lineages = (!requirements.regions.is_empty()).then(|| {
                let center_lineages = if requested_view == "tri" {
                    &m_pre_export_lineage
                } else {
                    &w_pre_export_lineage
                };
                region_center_demand(&requirements.regions, requested_view, &output_mesh)
                    .into_iter()
                    .zip(center_lineages.iter().copied())
                    .filter_map(|(requested, lineage)| {
                        (requested && lineage > 1).then_some(lineage)
                    })
                    .collect::<BTreeSet<_>>()
            });
            let mut parent_report = crate::write_unstructured_mesh_netcdf_with_metadata(
                &temporary_source_path,
                &output_mesh,
                GridfileMetadataSlices {
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
            let region_center_retention = if let Some(requested) =
                requested_center_lineages.as_ref()
            {
                let lineages = crate::read_gridfile_cell_lineages(&temporary_path)?;
                let published_centers = if requested_view == "tri" {
                    &lineages.m
                } else {
                    &lineages.w
                };
                if !requested.is_empty() && published_centers.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "CMRC published domain lacks center lineage needed to audit requested regions",
                    ));
                }
                let retained_lineages = published_centers.iter().copied().collect::<BTreeSet<_>>();
                let retained = requested.intersection(&retained_lineages).count();
                let dropped = requested.len() - retained;
                if dropped > 0 {
                    eprintln!(
                        "earthmesh_cli: CMRC final domain mask removed {dropped} of {} pre-export cell centers in declared refinement regions",
                        requested.len()
                    );
                }
                Some(serde_json::json!({
                    "scope": "pre_export_cell_centers_in_declared_regions",
                    "requested": requested.len(),
                    "retained": retained,
                    "dropped": dropped,
                }))
            } else {
                None
            };
            (
                published.report,
                is_surface_masked.then_some(published.kept_cells),
                Some(published.topology),
                Some(published.quality_topology),
                Some(published.geometry),
                region_center_retention,
                published.fvcom_2dm,
            )
        } else {
            (
                crate::write_unstructured_mesh_netcdf_with_metadata(
                    &temporary_path,
                    &output_mesh,
                    GridfileMetadataSlices {
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
        if let Some(retention) = &region_center_retention {
            certificate_document
                .as_object_mut()
                .expect("CMRC certificate document is an object")
                .insert(
                    "published_refinement_region_centers".to_string(),
                    retention.clone(),
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
            "physical_balance_domain_scope": if is_domain_export { "pre_export_closed_sphere" } else { "published_grid" },
            "published_grid_remap_available": !is_domain_export,
            "published_domain_topology": topology,
            "published_domain_quality_topology": domain_quality.as_ref().map(|(component_count, issues)| serde_json::json!({
                "connected_components": component_count,
                "issues": issues,
            })),
            "published_domain_geometry": published_geometry,
            "published_refinement_region_centers": region_center_retention,
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

    let runtime_state = refined_runtime_state(
        config,
        &refine,
        Some(state),
        &output_mesh,
        pentagons,
        delivered_level + 1,
    )?;

    Ok(RefinePipelineRunReport {
        gridinit: None,
        refine,
        regions: requirements.regions,
        max_level: chosen_level,
        realized_max_level: realized.max_level,
        finest_cell_km: realized.finest_cell_km,
        coarsest_cell_km: realized.coarsest_cell_km,
        realized_region_halvings: realized.region_halvings,
        hfield_diagnostics: Default::default(),
        transition_faces: 0,
        spring_nest_passes: 0,
        icon_nest_run: None,
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

pub(super) fn build_certified_cmrc_gridfile(
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
pub(super) struct CertifiedRequirementPlan {
    regions: Vec<RefinementRegion>,
    nlon: usize,
    nlat: usize,
    effective_levels: Vec<usize>,
    // None for threshold/hydro sources: partial raw provenance would be misleading.
    raw_region_levels: Option<Vec<usize>>,
    threshold_provenance: Option<serde_json::Value>,
    conservative_global_bound: bool,
    /// Composed from at least one refinement source: criteria, named regions
    /// or hydro targets.
    sourced: bool,
    /// Composed inside a regional domain rather than over the globe.
    domain_scoped: bool,
    /// Named regions dropped because they never reach the domain.
    regions_outside_domain: usize,
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
            sourced: false,
            domain_scoped: false,
            regions_outside_domain: 0,
        }
    }

    /// Valid source samples the threshold criteria read, when they report any;
    /// summed over criteria, as the demand plan counts them.
    fn threshold_valid_samples(&self) -> Option<u64> {
        let criteria = self.threshold_provenance.as_ref()?["criteria"].as_array()?;
        let counts = criteria
            .iter()
            .filter_map(|criterion| criterion["raw_support"]["valid_source_samples"].as_u64())
            .collect::<Vec<_>>();
        (!counts.is_empty()).then(|| counts.iter().sum())
    }

    /// A requirement of level 0 everywhere: refinement was enabled and nothing
    /// asks for it -- the criteria found nothing over their thresholds, or on
    /// a regional run all the demand lies outside the domain. Reverse
    /// coarsening would build the level-1 mother only to coarsen all of it
    /// back, and gave up on Yunnan's 1.47M cells as CompressionIncomplete,
    /// when the certified level-0 mother is the answer -- as it is on every
    /// other backend (guide 11.98). Criteria that read no valid sample are
    /// said so rather than refused: an ocean domain judged by a land-type
    /// criterion reads none, and CMRC delivered there before.
    fn nothing_requested(&self) {
        let place = if self.domain_scoped {
            "in the regional domain"
        } else {
            "anywhere"
        };
        let why = match self.threshold_valid_samples() {
            Some(0) => format!(
                "the enabled criteria read no valid source sample {place} (check that their \
                 sources cover it)"
            ),
            Some(samples) => format!(
                "the enabled criteria read {samples} valid source samples {place} and none of \
                 them meets a threshold at this resolution"
            ),
            None => format!("the requirement asks for no refinement {place}"),
        };
        eprintln!(
            "earthmesh_cli: warning: refinement was requested, but {why}; the certified mother \
             is delivered unrefined"
        );
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
            "requirement_scope": if self.domain_scoped { "regional_domain" } else { "global" },
            "regions_outside_domain": self.regions_outside_domain,
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

/// The requirement raster's sample centres that lie in `domain`, as
/// `(i, j, lon, lat)`.
fn domain_sample_centers(
    domain: &GridRegion,
    nlon: usize,
    nlat: usize,
) -> Vec<(usize, usize, f64, f64)> {
    let prepared = domain.prepared();
    let mut inside = Vec::new();
    for j in 0..nlat {
        let lat = -90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64;
        for i in 0..nlon {
            let lon = -180.0 + (i as f64 + 0.5) * 360.0 / nlon as f64;
            if prepared.contains(lon, lat) {
                inside.push((i, j, lon, lat));
            }
        }
    }
    inside
}

/// Whether `region` asks anything of a mesh that keeps only `domain`: one of
/// its own points lies in the domain (a region smaller than a raster cell
/// holds no sample centre), or a sample centre of the domain lies in it.
fn region_reaches_domain(
    region: &RefinementRegion,
    domain: &GridRegion,
    domain_samples: &[(usize, usize, f64, f64)],
) -> bool {
    let own_points = match region {
        RefinementRegion::Circle { center, .. } => vec![*center],
        RefinementRegion::Bbox {
            west_degrees,
            east_degrees,
            south_degrees,
            north_degrees,
            ..
        } => {
            let span = (east_degrees - west_degrees).rem_euclid(360.0);
            vec![earthmesh_mesh::LonLatDegrees::new(
                west_degrees + span / 2.0,
                (south_degrees + north_degrees) / 2.0,
            )]
        }
        RefinementRegion::Corridor { points, .. } | RefinementRegion::Polygon { points, .. } => {
            points.clone()
        }
    };
    own_points
        .iter()
        .any(|point| domain.contains(point.lon_degrees, point.lat_degrees))
        || domain_samples.iter().any(|&(_, _, lon, lat)| {
            region.contains_lonlat_canonical(earthmesh_mesh::LonLatDegrees::new(lon, lat))
        })
}

pub(super) fn certified_requirement_plan(
    contents: &str,
    config: &EarthmeshConfig,
    refine: &RefineConfig,
    base_nxp: usize,
    specified_level: usize,
    calculated_level: usize,
    domain: Option<&GridRegion>,
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

    // A regional run publishes its domain alone, yet the requirement set the
    // mother's depth and its coarsening over the whole globe: Yunnan at 40 km
    // (soil k_s) built a level-1 mother of 1.47M cells for demand in northern
    // Vietnam, then published 595. Composed inside the domain, demand
    // elsewhere no longer reaches the mother, and a named region that never
    // touches the domain asks nothing of this mesh.
    let domain_samples =
        domain.map(|domain| domain_sample_centers(domain, hfield.nlon, hfield.nlat));
    let mut regions_outside_domain = 0;
    let (specified_regions, calculated_regions) = match (domain, &domain_samples) {
        (Some(domain), Some(samples)) => {
            let named = specified_regions.len() + calculated_regions.len();
            let reaching = |regions: Vec<RefinementRegion>| {
                regions
                    .into_iter()
                    .filter(|region| region_reaches_domain(region, domain, samples))
                    .collect::<Vec<_>>()
            };
            let (specified, calculated) =
                (reaching(specified_regions), reaching(calculated_regions));
            regions_outside_domain = named - specified.len() - calculated.len();
            if regions_outside_domain > 0 {
                if specified.is_empty()
                    && calculated.is_empty()
                    && !has_threshold_sources
                    && hydro_level == 0
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "CMRC: all {named} refinement region(s) lie outside the regional domain, \
                             so none of them asks anything of the mesh this run publishes"
                        ),
                    ));
                }
                eprintln!(
                    "earthmesh_cli: warning: CMRC ignores {regions_outside_domain} of {named} \
                     refinement region(s): they lie outside the regional domain"
                );
            }
            (specified, calculated)
        }
        _ => (specified_regions, calculated_regions),
    };
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
            domain,
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
                domain,
            )?,
            None,
        )
    };
    crate::hydro_refinement_adapter::apply_hydro_target_to_field(
        &mut field, &hfield, base_m, domain,
    )?;
    let mut levels = field
        .level_map(base_m, quantized_max_level)?
        .into_iter()
        .map(usize::from)
        .collect::<Vec<_>>();
    // Do not mistake a region-only subset for the raw demand of a mixed-source run.
    let raw_region_levels = if !has_threshold_sources && hfield.hydro_target_paths().is_none() {
        let mask = domain.map(|domain| {
            crate::hfield_refine::HfieldDomainMask::new(field.nlon(), field.nlat(), domain)
        });
        Some(
            crate::hfield_refine::build_raw_region_hfield(
                &regions,
                base_m,
                field.nlon(),
                field.nlat(),
                mask.as_ref(),
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
    // conservative bound and costs no additional geometric machinery. Over a
    // regional domain the bound is the domain's: the rest is carved away.
    let raise = |levels: &mut Vec<usize>, level: usize| match domain {
        Some(domain) => {
            for (i, j, ..) in domain_sample_centers(domain, field.nlon(), field.nlat()) {
                let at = &mut levels[j * field.nlon() + i];
                *at = (*at).max(level);
            }
        }
        None => levels.fill(level),
    };
    if specified_present && levels.iter().copied().max().unwrap_or(0) < specified_level {
        raise(&mut levels, specified_level);
        conservative_global_bound = true;
    }
    if calculated_present && levels.iter().copied().max().unwrap_or(0) < calculated_level {
        raise(&mut levels, calculated_level);
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
        sourced: true,
        domain_scoped: domain.is_some(),
        regions_outside_domain,
    })
}

pub(super) fn certified_outcome_error(
    outcome: earthmesh_refine_certified::CertifiedMeshOutcome,
) -> io::Error {
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

pub(super) fn certified_mother_pentagons(mesh: &MeshState) -> io::Result<[usize; 12]> {
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

pub(super) fn certified_icosahedron_vertices(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::certified_options::{CertifiedDelivery, CertifiedMode, CertifiedRunOptions};

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
        let plan = certified_requirement_plan(contents, &config, &refine, 144, 0, 2, None).unwrap();
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
        let repeat =
            certified_requirement_plan(contents, &config, &refine, 144, 0, 2, None).unwrap();
        assert_eq!(layers, repeat.layer_report(None, 3));
        // Preserve the named-region-only contract when there is no threshold.
        refine.refine_num_landtypes = false;
        let plan = certified_requirement_plan(contents, &config, &refine, 144, 0, 2, None).unwrap();
        assert_eq!(plan.regions.len(), 1);
        assert_eq!(plan.effective_levels.iter().max(), Some(&2));
        assert!(plan.threshold_provenance.is_none());
        assert!(plan.layer_report(None, 3)["threshold_sources"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_regional_requirement_is_composed_inside_its_domain() {
        // A 5-degree raster, a domain of 20 degrees, level-2 circles in and far
        // outside it.
        let contents = "&hfield\n NL%hfield_nlon=72\n NL%hfield_nlat=36\n/\n";
        let config = EarthmeshConfig::default();
        let circles = |chain: &str| RefineConfig {
            refine_spc: true,
            max_iter_spc: 2,
            mask_refine_spc_type: "circle".into(),
            mask_refine_spc_fprefix: format!("inline:circles:{chain}"),
            ..RefineConfig::default()
        };
        let domain = GridRegion::Bbox {
            west: 0.0,
            east: 20.0,
            north: 20.0,
            south: 0.0,
        };
        let level_at = |plan: &CertifiedRequirementPlan, lon: f64, lat: f64| {
            let i = ((lon + 180.0) / 360.0 * plan.nlon as f64) as usize;
            let j = ((lat + 90.0) / 180.0 * plan.nlat as f64) as usize;
            plan.effective_levels[j * plan.nlon + i]
        };
        let plan = |refine: &RefineConfig, domain: Option<&GridRegion>| {
            certified_requirement_plan(contents, &config, refine, 144, 2, 0, domain)
        };

        let both = circles("lon=10,lat=10,radius_km=600;lon=100,lat=10,radius_km=600");
        let global = plan(&both, None).unwrap();
        assert!(!global.domain_scoped);
        assert_eq!(global.regions.len(), 2);
        assert_eq!(level_at(&global, 100.0, 10.0), 2);
        let regional = plan(&both, Some(&domain)).unwrap();
        assert!(regional.domain_scoped);
        assert_eq!(
            (regional.regions.len(), regional.regions_outside_domain),
            (1, 1)
        );
        assert_eq!(level_at(&regional, 10.0, 10.0), 2);
        assert_eq!(
            level_at(&regional, 100.0, 10.0),
            0,
            "demand outside the domain no longer reaches the mother"
        );
        let report = regional.layer_report(None, 3);
        assert_eq!(report["requirement_scope"], "regional_domain");
        assert_eq!(report["regions_outside_domain"], 1);

        // The far circle alone asks nothing of the mesh the run publishes.
        let error = plan(&circles("lon=100,lat=10,radius_km=600"), Some(&domain))
            .err()
            .expect("a run whose regions all miss the domain is refused");
        assert!(
            error
                .to_string()
                .contains("lie outside the regional domain"),
            "{error}"
        );

        // A circle between sample centres keeps its level: over the domain,
        // no longer over the globe.
        let tiny = circles("lon=10,lat=10,radius_km=20");
        let bounded = plan(&tiny, Some(&domain)).unwrap();
        assert!(bounded.conservative_global_bound);
        assert_eq!(level_at(&bounded, 12.5, 12.5), 2);
        assert_eq!(level_at(&bounded, 100.0, 10.0), 0);
        let unbounded = plan(&tiny, None).unwrap();
        assert!(unbounded.effective_levels.iter().all(|&level| level == 2));
    }
}
