use crate::classify_area_judge_landtype_one_based;
use crate::ensure_leading_mask_postproc_placeholder;
use crate::finalize_mask_postproc_layout_with_reindex_report;
use crate::mask_postproc_layout_from_unstructured_mesh;
use crate::read_unstructured_mesh_netcdf;
use crate::sample_landtype_values_for_points_one_based;
use crate::write_unstructured_mesh_netcdf_with_method_c_metadata;
use crate::AreaJudgeLandtypeClass;
use earthmesh_mesh::retain_edge_connected_components_with_hard_demand_one_based;
use std::io;
use std::path::Path;

use super::levels::{final_method_c_metadata_for_mask_postproc, refine_levels_from_gridfile};

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
        false,
        None,
    )
}

/// As [`write_landtype_masked_gridfile`], plus the carve-time topology cleanup.
///
/// `hard_center_demand` marks cell centres a run named outright, by one-based
/// centre id. A component holding one survives whatever its size: a refinement
/// circle over a small bay produces exactly the disjoint piece the
/// largest-component rule deletes, and nothing would report that the region the
/// run asked for is gone.
///
/// `retain_largest_ocean_component` drops every `oceanmesh` cell outside the
/// largest edge-connected piece of the carved domain — the narrow bays and river
/// mouths a centre-sample carve leaves behind as orphan cells or vertex-only
/// contacts. It is ignored for `landmesh`, where disjoint pieces are islands and
/// must survive.
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
    retain_largest_ocean_component: bool,
    hard_center_demand: Option<&[bool]>,
) -> io::Result<usize> {
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
    let mut kept = 0usize;
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
            kept += 1;
        }
    }
    if kept == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{mesh_type} landtype mask kept no cells"),
        ));
    }
    if retain_largest_ocean_component && !keep_land {
        let retention = retain_edge_connected_components_with_hard_demand_one_based(
            &mut is_in_domain,
            &layout.center_neighbors,
            &layout.center_neighbor_counts,
            &layout.vertex_neighbors,
            &layout.vertex_neighbor_counts,
            hard_center_demand.unwrap_or(&[]),
        )?;
        if !retention.removed_cell_ids.is_empty() {
            eprintln!(
                "earthmesh_cli: ocean carve dropped {} cell(s) ({} from pinched vertex fans) \
                 to keep the largest connected water body ({} components, {} cells retained)",
                retention.removed_cell_ids.len(),
                retention.non_manifold_removed_cell_count,
                retention.component_count,
                retention.retained_cell_count
            );
        }
        kept = retention.retained_cell_count;
    }
    let mut report =
        finalize_mask_postproc_layout_with_reindex_report(&layout, &is_in_domain, mode_grid)?;
    let mut source_metadata = refine_levels_from_gridfile(input_gridfile)?;
    if let Some(levels) = m_refine_level {
        source_metadata.m = levels.to_vec();
    }
    if let Some(levels) = w_refine_level {
        source_metadata.w = levels.to_vec();
    }
    let mut final_metadata = final_method_c_metadata_for_mask_postproc(
        mode_grid,
        &report,
        &is_in_domain,
        layout.ustr_points,
        &source_metadata,
    )?;
    if mode_grid.trim() == "tri" {
        // A carve can leave a vertex where the surviving cells fall into more
        // than one fan -- the mesh pinches to a point there, which the
        // `non_manifold_vertex_fan` gate rejects. Duplicating the vertex splits
        // the fans apart and keeps every cell; deleting the smaller fan also
        // clears the pinch but throws away cells the carve had decided to keep,
        // and on a demanded region that is the loss this whole path exists to
        // prevent.
        let duplicate_sources =
            crate::unstructured_mesh_support::split_non_manifold_triangle_vertex_fans(
                &mut report.mesh,
            )?;
        final_metadata.duplicate_w_vertices(&duplicate_sources)?;
    }
    write_unstructured_mesh_netcdf_with_method_c_metadata(
        output_gridfile,
        &report.mesh,
        final_metadata.slices(),
    )?;
    Ok(kept)
}
