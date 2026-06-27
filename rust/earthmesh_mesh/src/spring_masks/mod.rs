use super::*;

/// Pure Rust port of the source-mask classification core in
/// `MOD_grid_preprocess:refine_sjx_regional_make`.
///
/// The original routine reads the `mask_patch` NetCDF/file state before this
/// classification loop. This kernel accepts that mask and the source lon/lat
/// vertex arrays explicitly, then mirrors the Fortran `Source_Find` lookup and
/// subsequent `max(1, source - 1)` cell-index shift for each triangle center
/// from `num_mp_step(iter)` onward.
pub fn refine_sjx_regional_make_fortran_indexed(
    input: RefineRegionalMaskInput<'_>,
) -> Option<Vec<bool>> {
    if input.source_lon_vertices.len() < 2
        || input.source_lat_vertices.len() < 2
        || input.mask_patch.is_empty()
    {
        return None;
    }

    let mut refined_triangles = vec![false; input.triangle_lonlat.len()];
    for triangle_id in input.first_triangle_id..input.triangle_lonlat.len() {
        let center = input.triangle_lonlat[triangle_id];
        let lon_source =
            source_find_lon_fortran_indexed(input.source_lon_vertices, center.lon_degrees)?
                .saturating_sub(1)
                .max(1);
        let lat_source =
            source_find_lat_fortran_indexed(input.source_lat_vertices, center.lat_degrees)?
                .saturating_sub(1)
                .max(1);
        if *input.mask_patch.get(lon_source)?.get(lat_source)? {
            refined_triangles[triangle_id] = true;
        }
    }

    Some(refined_triangles)
}

/// Pure Rust port of `MOD_grid_preprocess:set_dbxMove_regional_step`.
///
/// The original routine derives initial refinement flags either from
/// `num_sjx_ref` or `refine_sjx_regional_make`. This core accepts those flags
/// explicitly, expands them through `set_dis` boundary layers, marks cells on
/// refined triangles as movable, freezes mixed boundary cells, then optionally
/// removes cells in protected seed-vertex neighborhoods for
/// `vertex_protect_layers`.
pub fn set_dbx_move_regional_step_fortran_indexed(
    input: RegionalMoveMaskInput<'_>,
) -> Option<RegionalMoveMaskOutput> {
    if input.refined_triangles.len() != input.cells_on_triangle.len()
        || input.triangles_on_cell.len() != input.n_edges_on_cell.len()
    {
        return None;
    }

    let (expanded_refined_triangles, boundary_mask) =
        expand_triangles_from_boundary_fortran_indexed(
            input.refined_triangles.to_vec(),
            input.triangles_on_cell,
            input.n_edges_on_cell,
            input.set_dis,
        )?;

    let mut move_mask = vec![false; input.triangles_on_cell.len()];
    for triangle_id in 2..expanded_refined_triangles.len() {
        if !expanded_refined_triangles[triangle_id] {
            continue;
        }
        for &cell_id in input.cells_on_triangle.get(triangle_id)? {
            if cell_id == 0 {
                continue;
            }
            *move_mask.get_mut(cell_id)? = true;
        }
    }
    for cell_id in 2..boundary_mask.len() {
        if boundary_mask[cell_id] {
            move_mask[cell_id] = false;
        }
    }

    let mut protected_triangles = vec![false; input.refined_triangles.len()];
    if input.vertex_protect_layers > 0 && !input.protected_seed_cells.is_empty() {
        let mut active_protected_seed_cells = Vec::new();
        for &cell_id in input.protected_seed_cells {
            let edge_count = *input.n_edges_on_cell.get(cell_id)?;
            let cell_triangles = input.triangles_on_cell.get(cell_id)?;
            if edge_count > cell_triangles.len() {
                return None;
            }
            let touches_refinement = cell_triangles.iter().take(edge_count).any(|&triangle_id| {
                *expanded_refined_triangles
                    .get(triangle_id)
                    .unwrap_or(&false)
            });
            if touches_refinement {
                active_protected_seed_cells.push(cell_id);
            }
        }

        if !active_protected_seed_cells.is_empty() {
            for cell_id in active_protected_seed_cells {
                let edge_count = input.n_edges_on_cell[cell_id];
                let cell_triangles = input.triangles_on_cell.get(cell_id)?;
                for &triangle_id in cell_triangles.iter().take(edge_count) {
                    *protected_triangles.get_mut(triangle_id)? = true;
                }
            }
            protected_triangles = expand_triangles_from_boundary_fortran_indexed(
                protected_triangles,
                input.triangles_on_cell,
                input.n_edges_on_cell,
                input.vertex_protect_layers,
            )?
            .0;

            for triangle_id in 2..protected_triangles.len() {
                if !protected_triangles[triangle_id] {
                    continue;
                }
                for &cell_id in input.cells_on_triangle.get(triangle_id)? {
                    if cell_id == 0 {
                        continue;
                    }
                    *move_mask.get_mut(cell_id)? = false;
                }
            }
        }
    }

    Some(RegionalMoveMaskOutput {
        move_mask,
        boundary_mask,
        expanded_refined_triangles,
        protected_triangles,
    })
}
