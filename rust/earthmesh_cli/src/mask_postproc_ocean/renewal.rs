use std::collections::BTreeSet;
use std::io;

use earthmesh_mesh::{
    boundary_connection_one_based, fill_vertex_only_ocean_contacts_one_based,
    remove_isolated_ocean_one_based, renew_mask_postproc_domain_triangles_one_based,
    renew_mask_postproc_opposite_domain_triangles_one_based, widen_narrow_waterway_one_based,
};

use crate::masked_topology_cleanup::{
    build_allowed_edge_adjacency, cleanup_masked_topology_one_based, domain_topology_error,
    ComponentRetentionPolicy, DomainTopologyFailureKind, MaskedTopologyCleanupInput,
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
    renew_mask_postproc_ocean_domain_one_based_with_hard_demand(
        layout,
        is_in_domain_ustr,
        mode_grid,
        &[],
    )
}

/// Ocean renewal with exact immutable per-center source demand.
///
/// `hard_center_demand` is a boolean coverage mask derived from the immutable
/// source-demand ledger. It is deliberately independent of actual refinement
/// level: transition/topology excess must never protect an otherwise invalid
/// product component.
///
/// Triangular products run compatibility renewal, isolated-water cleanup, and
/// exact edge-component repair to a finite fixed point. Hexagonal products use
/// the same exact edge-component rule without the tri-only compatibility pass.
/// A repeated state is a typed failure; an invalid intermediate mesh is never
/// published.
pub fn renew_mask_postproc_ocean_domain_one_based_with_hard_demand(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
    hard_center_demand: &[bool],
) -> io::Result<MaskPostprocOceanRenewalReport> {
    validate_mask_postproc_layout(layout)?;
    if is_in_domain_ustr.len() < layout.ustr_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ocean product mask length {} must cover {} one-based centers",
                is_in_domain_ustr.len(),
                layout.ustr_points
            ),
        ));
    }
    let source_product_mask = is_in_domain_ustr[..layout.ustr_points].to_vec();
    let mut hard_demand = normalize_hard_center_demand(hard_center_demand, layout.ustr_points)?;
    let mut excluded_unsupported_hard_demand_cells = Vec::new();
    // Raster projection is conservative at mixed coastline bins. Demand only
    // becomes immutable after intersecting the actual ocean product support.
    for (hard, &supported) in hard_demand.iter_mut().zip(&source_product_mask) {
        *hard &= supported == 1;
    }
    let allowed = source_product_mask
        .iter()
        .map(|value| *value == 1)
        .collect::<Vec<_>>();
    let adjacency = build_allowed_edge_adjacency(layout, &allowed)?;
    for center_id in 2..hard_demand.len() {
        if hard_demand[center_id] && adjacency[center_id].is_empty() {
            hard_demand[center_id] = false;
            excluded_unsupported_hard_demand_cells.push(center_id);
        }
    }

    let mut report = match mode_grid.trim() {
        "hex" => renew_hex_ocean_product(layout, &source_product_mask, &hard_demand),
        "tri" => renew_tri_ocean_product_to_fixed_point(
            layout,
            &source_product_mask,
            &mut hard_demand,
            &mut excluded_unsupported_hard_demand_cells,
        ),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ocean mask_postproc renewal supports tri or hex mode_grid only, got {other}"),
        )),
    }?;
    excluded_unsupported_hard_demand_cells.sort_unstable();
    excluded_unsupported_hard_demand_cells.dedup();
    report.excluded_unsupported_hard_demand_cells = excluded_unsupported_hard_demand_cells;
    Ok(report)
}

fn renew_hex_ocean_product(
    layout: &MaskPostprocLayout,
    source_product_mask: &[i32],
    hard_demand: &[bool],
) -> io::Result<MaskPostprocOceanRenewalReport> {
    let (provisional, _) =
        renew_mask_postproc_ocean_topology_one_based(layout, source_product_mask, "hex")?;
    let cleanup =
        cleanup_product_components(layout, source_product_mask, &provisional, hard_demand)?;
    let renewed = renew_mask_postproc_data_from_layout(layout, &cleanup, "hex")?;
    Ok(MaskPostprocOceanRenewalReport {
        is_in_domain_ustr: cleanup,
        renewed,
        boundary: None,
        isolated: None,
        excluded_unsupported_hard_demand_cells: Vec::new(),
    })
}

struct TriOceanPass {
    active: Vec<i32>,
    provisional_after_isolated: Vec<i32>,
    renewed: earthmesh_mesh::MaskPostprocRenewedData,
    boundary: earthmesh_mesh::BoundaryConnection,
    isolated: earthmesh_mesh::IsolatedOceanRenewal,
}

fn renew_tri_ocean_product_to_fixed_point(
    layout: &MaskPostprocLayout,
    source_product_mask: &[i32],
    hard_demand: &mut [bool],
    excluded_unsupported_hard_demand_cells: &mut Vec<usize>,
) -> io::Result<MaskPostprocOceanRenewalReport> {
    let mut current = source_product_mask.to_vec();
    let mut legal_product_closure = source_product_mask.to_vec();
    let mut seen = BTreeSet::new();
    let mut last_transition = String::new();

    for pass_index in 0..128 {
        if !seen.insert(active_mask_signature(&current)) {
            return Err(domain_topology_error(
                DomainTopologyFailureKind::CompatibilityCleanupDidNotConverge,
                None,
                format!(
                    "tri ocean compatibility and exact product-topology cleanup entered a repeated state at pass {}; {last_transition}",
                    pass_index + 1
                ),
            ));
        }

        let pass = renew_tri_ocean_product_once(
            layout,
            source_product_mask,
            &current,
            &mut legal_product_closure,
            hard_demand,
            excluded_unsupported_hard_demand_cells,
        )?;
        let current_active = current.iter().filter(|value| **value == 1).count();
        let provisional_active = pass
            .provisional_after_isolated
            .iter()
            .filter(|value| **value == 1)
            .count();
        let final_active = pass.active.iter().filter(|value| **value == 1).count();
        let cleanup_added = pass
            .provisional_after_isolated
            .iter()
            .zip(&pass.active)
            .filter(|(before, after)| **before != 1 && **after == 1)
            .count();
        let cleanup_removed = pass
            .provisional_after_isolated
            .iter()
            .zip(&pass.active)
            .filter(|(before, after)| **before == 1 && **after != 1)
            .count();
        last_transition = format!(
            "previous pass current/provisional/final active={current_active}/{provisional_active}/{final_active}, cleanup added/removed={cleanup_added}/{cleanup_removed}"
        );
        if pass.active == current && pass.provisional_after_isolated == pass.active {
            return Ok(MaskPostprocOceanRenewalReport {
                is_in_domain_ustr: pass.active,
                renewed: pass.renewed,
                boundary: Some(pass.boundary),
                isolated: Some(pass.isolated),
                excluded_unsupported_hard_demand_cells: Vec::new(),
            });
        }
        current = pass.active;
    }

    Err(domain_topology_error(
        DomainTopologyFailureKind::CompatibilityCleanupDidNotConverge,
        None,
        "tri ocean compatibility and exact product-topology cleanup did not converge within 128 passes",
    ))
}

fn renew_tri_ocean_product_once(
    layout: &MaskPostprocLayout,
    source_product_mask: &[i32],
    current: &[i32],
    legal_product_closure: &mut [i32],
    hard_demand: &mut [bool],
    excluded_unsupported_hard_demand_cells: &mut Vec<usize>,
) -> io::Result<TriOceanPass> {
    let (mut provisional, renewed) =
        renew_mask_postproc_ocean_topology_one_based(layout, current, "tri")?;
    extend_legal_product_closure(
        legal_product_closure,
        source_product_mask,
        current,
        &provisional,
    );
    let boundary = boundary_connection_one_based(
        &renewed.center_neighbors_next,
        &renewed.center_neighbor_counts_next,
        &layout.vertex_neighbor_counts,
        &renewed.vertex_neighbor_counts_next,
    )?;
    let mut vertex_neighbor_counts_after = renewed.vertex_neighbor_counts_next.clone();
    let isolated = remove_isolated_ocean_one_based(
        &mut provisional,
        &layout.center_neighbors,
        &layout.center_neighbor_counts,
        &renewed.vertex_neighbors_next,
        &layout.vertex_neighbor_counts,
        &mut vertex_neighbor_counts_after,
        &boundary,
    )?;
    restore_mask_postproc_placeholders(&mut provisional, source_product_mask);
    exclude_hard_demand_outside_active_support(
        hard_demand,
        &provisional,
        excluded_unsupported_hard_demand_cells,
    );
    let active =
        cleanup_product_components(layout, legal_product_closure, &provisional, hard_demand)?;

    Ok(TriOceanPass {
        active,
        provisional_after_isolated: provisional,
        renewed,
        boundary,
        isolated,
    })
}

fn cleanup_product_components(
    layout: &MaskPostprocLayout,
    allowed_before_cleanup: &[i32],
    provisional_active: &[i32],
    hard_demand: &[bool],
) -> io::Result<Vec<i32>> {
    let seeds = vec![false; layout.ustr_points];
    Ok(
        cleanup_masked_topology_one_based(MaskedTopologyCleanupInput {
            layout,
            allowed_before_cleanup,
            provisional_active,
            hard_demand,
            seeds: &seeds,
            cell_areas: &[],
            minimum_component_area: 0.0,
            retention: ComponentRetentionPolicy::KeepAllNonSingletons,
        })?
        .active,
    )
}

fn normalize_hard_center_demand(demand: &[bool], layout_len: usize) -> io::Result<Vec<bool>> {
    if demand.is_empty() {
        return Ok(vec![false; layout_len]);
    }
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

fn exclude_hard_demand_outside_active_support(
    hard_demand: &mut [bool],
    active: &[i32],
    excluded: &mut Vec<usize>,
) {
    for center_id in 2..hard_demand.len() {
        if hard_demand[center_id] && active[center_id] != 1 {
            hard_demand[center_id] = false;
            excluded.push(center_id);
        }
    }
}

fn extend_legal_product_closure(
    legal: &mut [i32],
    source: &[i32],
    current: &[i32],
    topology: &[i32],
) {
    for center_id in 2..legal.len() {
        if source[center_id] == 1 || current[center_id] == 1 || topology[center_id] == 1 {
            legal[center_id] = 1;
        }
    }
}

fn active_mask_signature(mask: &[i32]) -> Vec<u64> {
    let physical_cells = mask.len().saturating_sub(2);
    let mut signature = vec![0_u64; physical_cells.div_ceil(64)];
    for center_id in 2..mask.len() {
        if mask[center_id] == 1 {
            let offset = center_id - 2;
            signature[offset / 64] |= 1_u64 << (offset % 64);
        }
    }
    signature
}

fn renew_mask_postproc_ocean_topology_one_based(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<(Vec<i32>, earthmesh_mesh::MaskPostprocRenewedData)> {
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
        "hex" => Ok((is_in_domain, renewed)),
        "tri" => {
            for _ in 0..128 {
                let before = is_in_domain.clone();
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

                if is_in_domain == before {
                    return Ok((is_in_domain, renewed));
                }
            }
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ocean mask_postproc triangle renewal did not converge within 128 passes",
            ))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ocean mask_postproc renewal supports tri or hex mode_grid only, got {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::exclude_hard_demand_outside_active_support;

    #[test]
    fn product_compatibility_can_exclude_unsupported_hard_demand() {
        let mut hard_demand = vec![false, false, true, true, false];
        let mut excluded = Vec::new();

        exclude_hard_demand_outside_active_support(
            &mut hard_demand,
            &[0, -1, -1, 1, -1],
            &mut excluded,
        );

        assert_eq!(hard_demand, vec![false, false, false, true, false]);
        assert_eq!(excluded, vec![2]);
    }
}
