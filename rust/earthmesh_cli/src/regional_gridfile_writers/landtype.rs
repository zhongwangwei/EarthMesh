use crate::classify_area_judge_landtype_one_based;
use crate::ensure_leading_mask_postproc_placeholder;
use crate::finalize_mask_postproc_layout_with_reindex_report;
use crate::mask_postproc_layout_from_unstructured_mesh;
use crate::masked_topology_cleanup::{
    cleanup_masked_topology_one_based, ComponentRetentionPolicy, MaskedTopologyCleanupInput,
};
use crate::read_unstructured_mesh_netcdf;
use crate::sample_landtype_values_for_points_one_based;
use crate::write_unstructured_mesh_netcdf_with_method_c_metadata;
use crate::AreaJudgeLandtypeClass;
use crate::UnstructuredMesh;
use std::io;
use std::path::Path;

use super::levels::{final_method_c_metadata_for_mask_postproc, refine_levels_from_gridfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LandtypeMaskedGridfileReport {
    pub(crate) kept: usize,
    pub(crate) active_source_centers: Vec<bool>,
    pub(crate) delivered_source_cells: Vec<(usize, Vec<usize>)>,
}

/// Carve a gridfile to land-only or ocean-only cells from a land-type NetCDF.
///
/// This is the direct, post-run counterpart of the data_preprocess/Area_judge
/// sea-land classification: sample each grid cell centre against `landtype_file`,
/// keep land cells for `landmesh`, keep ocean cells for `oceanmesh`, then use the
/// existing mask-postproc finalization path so connectivity is compacted and
/// renumbered consistently. Returns the number of cells kept.
pub fn write_landtype_masked_gridfile(
    input_gridfile: impl AsRef<Path>,
    output_gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mode_grid: &str,
    mesh_type: &str,
) -> io::Result<usize> {
    write_landtype_masked_gridfile_with_refine_levels(
        input_gridfile,
        output_gridfile,
        landtype_file,
        gridnum_perdegree,
        mode_grid,
        mesh_type,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn write_landtype_masked_gridfile_with_refine_levels(
    input_gridfile: impl AsRef<Path>,
    output_gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mode_grid: &str,
    mesh_type: &str,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
) -> io::Result<usize> {
    Ok(write_landtype_masked_gridfile_with_refine_levels_impl(
        input_gridfile,
        output_gridfile,
        landtype_file,
        gridnum_perdegree,
        mode_grid,
        mesh_type,
        m_refine_level,
        w_refine_level,
        None,
        false,
    )?
    .kept)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_landtype_masked_gridfile_with_hard_demand_report(
    input_gridfile: impl AsRef<Path>,
    output_gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mode_grid: &str,
    mesh_type: &str,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
    hard_center_demand: Option<&[bool]>,
    coupled_provenance: bool,
) -> io::Result<LandtypeMaskedGridfileReport> {
    write_landtype_masked_gridfile_with_refine_levels_impl(
        input_gridfile,
        output_gridfile,
        landtype_file,
        gridnum_perdegree,
        mode_grid,
        mesh_type,
        m_refine_level,
        w_refine_level,
        hard_center_demand,
        coupled_provenance,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_landtype_masked_gridfile_with_refine_levels_impl(
    input_gridfile: impl AsRef<Path>,
    output_gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mode_grid: &str,
    mesh_type: &str,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
    hard_center_demand: Option<&[bool]>,
    coupled_provenance: bool,
) -> io::Result<LandtypeMaskedGridfileReport> {
    let keep_land = match mesh_type.trim() {
        "landmesh" => true,
        "oceanmesh" => false,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("landtype masked gridfile supports landmesh or oceanmesh, got {other}"),
            ));
        }
    };
    let input_gridfile = input_gridfile.as_ref();
    let mesh = read_unstructured_mesh_netcdf(input_gridfile)?;
    let raw_layout = mask_postproc_layout_from_unstructured_mesh(&mesh, mode_grid)?;
    let layout = ensure_leading_mask_postproc_placeholder(raw_layout);
    let landtype_values = sample_landtype_values_for_points_one_based(
        landtype_file,
        gridnum_perdegree,
        &layout.center_points[2..],
    )?;

    let mut is_in_domain = vec![-1i32; layout.ustr_points];
    if !is_in_domain.is_empty() {
        is_in_domain[0] = 0;
    }
    if is_in_domain.len() > 1 {
        is_in_domain[1] = 0;
    }
    for (i, landtype_value) in (2..layout.ustr_points).zip(landtype_values) {
        let is_land = matches!(
            classify_area_judge_landtype_one_based(landtype_value),
            AreaJudgeLandtypeClass::Land
        );
        if is_land == keep_land {
            is_in_domain[i] = 1;
        }
    }
    let mut source_metadata = refine_levels_from_gridfile(input_gridfile)?;
    if let Some(levels) = m_refine_level {
        source_metadata.m = levels.to_vec();
    }
    if let Some(levels) = w_refine_level {
        source_metadata.w = levels.to_vec();
    }
    // A coupled land/ocean pair is a partition of one parent mesh. A cell that
    // is isolated within one side is still coupled across the internal
    // interface, so per-side orphan deletion would break the partition.
    if !coupled_provenance {
        let allowed_before_cleanup = is_in_domain.clone();
        let seeds = vec![false; layout.ustr_points];
        let no_hard_demand = vec![false; layout.ustr_points];
        let simulation_ready_support = cleanup_masked_topology_one_based(
            MaskedTopologyCleanupInput {
                layout: &layout,
                allowed_before_cleanup: &allowed_before_cleanup,
                provisional_active: &allowed_before_cleanup,
                hard_demand: &no_hard_demand,
                seeds: &seeds,
                cell_areas: &[],
                minimum_component_area: 0.0,
                retention: ComponentRetentionPolicy::KeepAllNonSingletons,
            },
        )
        .map_err(|error| {
            if crate::masked_topology_cleanup::domain_topology_failure(&error).is_some_and(
                |failure| {
                    failure.kind()
                        == crate::masked_topology_cleanup::DomainTopologyFailureKind::NoRetainedCells
                },
            ) {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{mesh_type} landtype mask kept no cells"),
                )
            } else {
                error
            }
        })?
        .active;
        let hard_demand = hard_center_demand
            .map(|demand| {
                normalize_product_hard_center_demand(
                    demand,
                    &simulation_ready_support,
                    layout.ustr_points,
                )
            })
            .transpose()?
            .unwrap_or_else(|| vec![false; layout.ustr_points]);
        is_in_domain = cleanup_masked_topology_one_based(MaskedTopologyCleanupInput {
            layout: &layout,
            allowed_before_cleanup: &allowed_before_cleanup,
            provisional_active: &allowed_before_cleanup,
            hard_demand: &hard_demand,
            seeds: &seeds,
            cell_areas: &[],
            minimum_component_area: 0.0,
            retention: ComponentRetentionPolicy::KeepAllNonSingletons,
        })?
        .active;
    }
    let kept = is_in_domain
        .iter()
        .skip(2)
        .filter(|&&value| value == 1)
        .count();
    if kept == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{mesh_type} landtype mask kept no cells"),
        ));
    }
    let mut report =
        finalize_mask_postproc_layout_with_reindex_report(&layout, &is_in_domain, mode_grid)?;
    let active_source_centers = is_in_domain
        .iter()
        .map(|&value| value == 1)
        .collect::<Vec<_>>();
    let mut final_metadata = final_method_c_metadata_for_mask_postproc(
        mode_grid,
        &report,
        &is_in_domain,
        layout.ustr_points,
        &source_metadata,
    )?;
    let mut source_vertex_by_delivered_id = Vec::new();
    if coupled_provenance {
        source_vertex_by_delivered_id = vec![0; report.vertex_reindex.sorted_vertices.len() + 1];
        for (offset, &source_vertex_id) in report.vertex_reindex.sorted_vertices.iter().enumerate()
        {
            source_vertex_by_delivered_id[offset + 1] = source_vertex_id;
        }
    }
    if mode_grid.trim() == "tri" {
        let duplicate_sources =
            crate::unstructured_mesh_support::split_non_manifold_triangle_vertex_fans(
                &mut report.mesh,
            )?;
        if coupled_provenance {
            for &source_row in &duplicate_sources {
                let source_vertex_id = source_vertex_by_delivered_id
                    .get(source_row)
                    .copied()
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "split triangle vertex source row {source_row} is outside coupled source-edge provenance"
                            ),
                        )
                    })?;
                source_vertex_by_delivered_id.push(source_vertex_id);
            }
        }
        final_metadata.duplicate_w_vertices(&duplicate_sources)?;
    }
    let delivered_source_cells = if coupled_provenance {
        delivered_source_cells(
            &report.mesh,
            mode_grid,
            &active_source_centers,
            &source_vertex_by_delivered_id,
        )?
    } else {
        Vec::new()
    };
    write_unstructured_mesh_netcdf_with_method_c_metadata(
        output_gridfile,
        &report.mesh,
        final_metadata.slices(),
    )?;
    Ok(LandtypeMaskedGridfileReport {
        kept,
        active_source_centers,
        delivered_source_cells,
    })
}

fn normalize_hard_center_demand(demand: &[bool], layout_len: usize) -> io::Result<Vec<bool>> {
    if demand.len() != layout_len
        && demand.len() + 1 != layout_len
        && demand.len() + 2 != layout_len
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "hard center demand length {} must equal mask-postproc layout length {layout_len}, include one placeholder, or contain physical cells only",
                demand.len()
            ),
        ));
    }
    let mut normalized = vec![false; layout_len];
    if demand.len() == layout_len {
        normalized.copy_from_slice(demand);
    } else if demand.len() + 1 == layout_len {
        normalized[1..].copy_from_slice(demand);
    } else {
        normalized[2..].copy_from_slice(demand);
    }
    Ok(normalized)
}

fn normalize_product_hard_center_demand(
    demand: &[bool],
    product_support: &[i32],
    layout_len: usize,
) -> io::Result<Vec<bool>> {
    let mut normalized = normalize_hard_center_demand(demand, layout_len)?;
    // Raster support is conservative at mixed coastline bins. Demand becomes
    // immutable only after intersecting it with simulation-ready product
    // support; an isolated product cell cannot be retained as a model domain.
    for (hard, &supported) in normalized.iter_mut().zip(product_support) {
        *hard &= supported == 1;
    }
    Ok(normalized)
}

fn delivered_source_cells(
    mesh: &UnstructuredMesh,
    mode_grid: &str,
    active_source_centers: &[bool],
    source_vertex_by_delivered_id: &[usize],
) -> io::Result<Vec<(usize, Vec<usize>)>> {
    let source_center_ids = active_source_centers
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(source_id, &active)| active.then_some(source_id))
        .collect::<Vec<_>>();
    let delivered_cell_count = match mode_grid.trim() {
        "tri" => mesh.m_to_w.len().saturating_sub(2),
        "hex" => mesh.w_to_m.len().saturating_sub(2),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("coupled source-edge provenance supports tri or hex, got {other}"),
            ));
        }
    };
    if delivered_cell_count != source_center_ids.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "delivered {mode_grid} cell count {delivered_cell_count} does not match {} active source cells",
                source_center_ids.len(),
            ),
        ));
    }

    source_center_ids
        .into_iter()
        .enumerate()
        .map(|(offset, source_center_id)| {
            let delivered_center_id = offset + 2;
            let delivered_vertices = match mode_grid.trim() {
                "tri" => mesh.m_to_w[delivered_center_id].to_vec(),
                "hex" => {
                    let count =
                        usize::try_from(mesh.n_w_to_m[delivered_center_id]).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "delivered hex cell {delivered_center_id} has a negative vertex count"
                                ),
                            )
                        })?;
                    if count < 3 || count > mesh.w_to_m[delivered_center_id].len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "delivered hex cell {delivered_center_id} has invalid vertex count {count}"
                            ),
                        ));
                    }
                    mesh.w_to_m[delivered_center_id][..count].to_vec()
                }
                _ => unreachable!("mode_grid validated above"),
            };
            let source_vertices = delivered_vertices
                .into_iter()
                .map(|delivered_vertex_id| {
                    let delivered_vertex_id =
                        usize::try_from(delivered_vertex_id).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "delivered cell {delivered_center_id} has a negative vertex id"
                                ),
                            )
                        })?;
                    source_vertex_by_delivered_id
                        .get(delivered_vertex_id)
                        .copied()
                        .filter(|&source_id| source_id > 1)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "delivered cell {delivered_center_id} vertex {delivered_vertex_id} has no canonical source vertex"
                                ),
                            )
                        })
                })
                .collect::<io::Result<Vec<_>>>()?;
            Ok((source_center_id, source_vertices))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LonLatPoint;
    use crate::MaskPostprocLayout;

    fn layout_with_one_orphan() -> MaskPostprocLayout {
        MaskPostprocLayout {
            ustr_points: 5,
            ustr_bounds: 9,
            center_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 5],
            vertex_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 9],
            center_neighbors: vec![vec![], vec![], vec![2, 3, 4], vec![3, 2, 5], vec![6, 7, 8]],
            vertex_neighbors: vec![vec![]; 9],
            center_neighbor_counts: vec![0, 0, 3, 3, 3],
            vertex_neighbor_counts: vec![0; 9],
        }
    }

    #[test]
    fn actual_refinement_level_does_not_protect_topology_excess_orphan() {
        let layout = layout_with_one_orphan();
        let active = vec![0, 0, 1, 1, 1];
        let report = cleanup_masked_topology_one_based(MaskedTopologyCleanupInput {
            layout: &layout,
            allowed_before_cleanup: &active,
            provisional_active: &active,
            hard_demand: &[false; 5],
            seeds: &[false; 5],
            cell_areas: &[],
            minimum_component_area: 0.0,
            retention: ComponentRetentionPolicy::KeepAllNonSingletons,
        })
        .unwrap();
        assert_eq!(report.active, vec![0, 0, 1, 1, -1]);
    }

    #[test]
    fn exact_hard_demand_orphan_is_rejected_atomically() {
        let layout = layout_with_one_orphan();
        let active = vec![0, 0, 1, 1, 1];
        let error = cleanup_masked_topology_one_based(MaskedTopologyCleanupInput {
            layout: &layout,
            allowed_before_cleanup: &active,
            provisional_active: &active,
            hard_demand: &[false, false, false, false, true],
            seeds: &[false; 5],
            cell_areas: &[],
            minimum_component_area: 0.0,
            retention: ComponentRetentionPolicy::KeepAllNonSingletons,
        })
        .expect_err("disconnected hard-demand orphan has no legal product path");
        assert_eq!(
            crate::masked_topology_cleanup::domain_topology_failure(&error)
                .unwrap()
                .kind(),
            crate::masked_topology_cleanup::DomainTopologyFailureKind::
                RequiredComponentCannotBeConnected
        );
    }

    #[test]
    fn projected_hard_demand_is_clipped_to_exact_product_support() {
        let normalized =
            normalize_product_hard_center_demand(&[true, true, true], &[0, 0, 1, -1, 1], 5)
                .unwrap();

        assert_eq!(normalized, vec![false, false, true, false, true]);
    }

    #[test]
    fn projected_hard_demand_does_not_protect_an_orphan_product_cell() {
        let layout = layout_with_one_orphan();
        let allowed = vec![0, 0, 1, 1, 1];
        let seeds = vec![false; 5];
        let simulation_ready = cleanup_masked_topology_one_based(MaskedTopologyCleanupInput {
            layout: &layout,
            allowed_before_cleanup: &allowed,
            provisional_active: &allowed,
            hard_demand: &[false; 5],
            seeds: &seeds,
            cell_areas: &[],
            minimum_component_area: 0.0,
            retention: ComponentRetentionPolicy::KeepAllNonSingletons,
        })
        .unwrap()
        .active;
        let hard =
            normalize_product_hard_center_demand(&[false, false, true], &simulation_ready, 5)
                .unwrap();

        assert_eq!(simulation_ready, vec![0, 0, 1, 1, -1]);
        assert_eq!(hard, vec![false; 5]);
    }
}
