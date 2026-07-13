use std::io;

use earthmesh_mesh::{
    boundary_connection_one_based, fill_vertex_only_ocean_contacts_one_based,
    remove_isolated_ocean_one_based, renew_mask_postproc_domain_triangles_one_based,
    renew_mask_postproc_opposite_domain_triangles_one_based, widen_narrow_waterway_one_based,
};

use crate::{validate_mask_postproc_layout, MaskPostprocLayout, MaskPostprocOceanRenewalReport};

use super::helpers::{renew_mask_postproc_data_from_layout, restore_mask_postproc_placeholders};

/// Pure-data composition of the ocean branch renewal sequence in
/// `MOD_mask_postproc.F90:mask_postproc_Ocn`.
///
/// This starts after the sea-ratio mask has been applied and before the final
/// `Data_Finial`/gridfile/OBC writers.  Hex grids only need the generic
/// `Data_Renew` compaction.  Tri grids also run the compatibility triangle cleanups,
/// narrow-waterway widening, boundary-curve discovery, and isolated-ocean
/// peeling metadata.
pub fn renew_mask_postproc_ocean_domain_one_based(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<MaskPostprocOceanRenewalReport> {
    validate_mask_postproc_layout(layout)?;
    if is_in_domain_ustr.len() < layout.ustr_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInDmArea_ustr length {} must cover ustr_points {}",
                is_in_domain_ustr.len(),
                layout.ustr_points
            ),
        ));
    }

    let mode_grid = mode_grid.trim();
    let mut is_in_domain = is_in_domain_ustr[..layout.ustr_points].to_vec();
    let mut renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

    match mode_grid {
        "hex" => Ok(MaskPostprocOceanRenewalReport {
            is_in_domain_ustr: is_in_domain,
            renewed,
            boundary: None,
            isolated: None,
        }),
        "tri" => {
            let mut points_new = isize::try_from(renewed.points_next).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "renewed point count does not fit isize",
                )
            })?;
            renew_mask_postproc_domain_triangles_one_based(
                &mut is_in_domain,
                &layout.vertex_neighbors,
                &renewed.vertex_neighbors_next,
                &layout.vertex_neighbor_counts,
                &renewed.vertex_neighbor_counts_next,
                &mut points_new,
            )?;
            restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
            renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

            let mut converged = false;
            for _ in 0..128 {
                let before_opposite = renewed.points_next;
                let mut points_new = isize::try_from(renewed.points_next).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "renewed point count does not fit isize",
                    )
                })?;
                renew_mask_postproc_opposite_domain_triangles_one_based(
                    &mut is_in_domain,
                    &layout.vertex_neighbors,
                    &layout.vertex_neighbor_counts,
                    &renewed.vertex_neighbor_counts_next,
                    &mut points_new,
                )?;
                restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
                renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

                let before_widen = renewed.points_next;
                widen_narrow_waterway_one_based(
                    &mut is_in_domain,
                    &layout.vertex_neighbors,
                    &renewed.center_neighbors_next,
                    &layout.vertex_neighbor_counts,
                    &renewed.vertex_neighbor_counts_next,
                    &renewed.center_neighbor_counts_next,
                )?;
                restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);

                fill_vertex_only_ocean_contacts_one_based(
                    &mut is_in_domain,
                    &layout.vertex_neighbors,
                    &layout.vertex_neighbor_counts,
                )?;
                restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
                renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

                if renewed.points_next == before_opposite || renewed.points_next == before_widen {
                    converged = true;
                    break;
                }
            }
            if !converged {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "ocean mask_postproc triangle renewal did not converge within 128 passes",
                ));
            }

            let boundary = boundary_connection_one_based(
                &renewed.center_neighbors_next,
                &renewed.center_neighbor_counts_next,
                &layout.vertex_neighbor_counts,
                &renewed.vertex_neighbor_counts_next,
            )?;
            let mut vertex_neighbor_counts_after = renewed.vertex_neighbor_counts_next.clone();
            let isolated = remove_isolated_ocean_one_based(
                &mut is_in_domain,
                &layout.center_neighbors,
                &layout.center_neighbor_counts,
                &renewed.vertex_neighbors_next,
                &layout.vertex_neighbor_counts,
                &mut vertex_neighbor_counts_after,
                &boundary,
            )?;
            restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
            renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

            Ok(MaskPostprocOceanRenewalReport {
                is_in_domain_ustr: is_in_domain,
                renewed,
                boundary: Some(boundary),
                isolated: Some(isolated),
            })
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ocean mask_postproc renewal supports tri or hex mode_grid only, got {other}"),
        )),
    }
}
