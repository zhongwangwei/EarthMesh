use std::io;

use earthmesh_mesh::MaskPostprocRenewedData;

use crate::MaskPostprocLayout;

pub(super) fn renew_mask_postproc_data_from_layout(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<MaskPostprocRenewedData> {
    let active_centers = is_in_domain_ustr
        .iter()
        .map(|&value| value == 1)
        .collect::<Vec<_>>();
    earthmesh_mesh::renew_mask_postproc_data_one_based(
        mode_grid,
        &active_centers,
        &layout.center_neighbors,
        &layout.center_neighbor_counts,
        layout.ustr_bounds.saturating_sub(1),
    )
}

pub(super) fn restore_mask_postproc_placeholders(is_in_domain: &mut [i32], original: &[i32]) {
    for placeholder_id in 0..=1 {
        if placeholder_id < is_in_domain.len() && placeholder_id < original.len() {
            is_in_domain[placeholder_id] = original[placeholder_id];
        }
    }
}
