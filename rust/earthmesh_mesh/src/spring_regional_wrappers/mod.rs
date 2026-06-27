use super::*;

/// Pure Rust adapter for the in-memory mask + calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_regional_step`.
///
/// This keeps NetCDF/file persistence and the original upstream
/// `refine_sjx_regional_make` source classification outside the kernel, but
/// wires the already-migrated `set_dbxMove_regional_step` mask derivation into
/// the regional spring core so callers do not have to manually compose them.
pub fn springjustment_regional_from_refinement_fortran_indexed(
    input: SpringjustmentRegionalFromRefinementInput<'_>,
) -> Option<SpringjustmentRegionalFromRefinementOutput> {
    let mask = set_dbx_move_regional_step_fortran_indexed(RegionalMoveMaskInput {
        set_dis: input.set_dis,
        refined_triangles: input.refined_triangles,
        cells_on_triangle: input.cells_on_triangle,
        triangles_on_cell: input.triangles_on_cell,
        n_edges_on_cell: input.n_edges_on_cell,
        protected_seed_cells: input.protected_seed_cells,
        vertex_protect_layers: input.vertex_protect_layers,
    })?;
    let core = springjustment_regional_core_fortran_indexed(SpringjustmentRegionalCoreInput {
        triangle_lonlat: input.triangle_lonlat,
        cell_lonlat: input.cell_lonlat,
        cells_on_triangle: input.cells_on_triangle,
        triangles_on_cell: input.triangles_on_cell,
        n_edges_on_cell: input.n_edges_on_cell,
        move_mask: &mask.move_mask,
        niter_refine: input.niter_refine,
        radius: input.radius,
        diagnostic_every: input.diagnostic_every,
    })?;

    Some(SpringjustmentRegionalFromRefinementOutput { mask, core })
}

/// Pure Rust adapter for the in-memory source-mask branch of
/// `MOD_grid_preprocess:Springjustment_regional_step`.
///
/// This composes `refine_sjx_regional_make`, `set_dbxMove_regional_step`, and
/// the migrated regional spring/circumcenter core while still leaving NetCDF
/// mask loading and final persistence outside this deterministic kernel.
pub fn springjustment_regional_from_source_mask_fortran_indexed(
    input: SpringjustmentRegionalFromSourceMaskInput<'_>,
) -> Option<SpringjustmentRegionalFromSourceMaskOutput> {
    let refined_triangles = refine_sjx_regional_make_fortran_indexed(RefineRegionalMaskInput {
        triangle_lonlat: input.triangle_lonlat,
        source_lon_vertices: input.source_lon_vertices,
        source_lat_vertices: input.source_lat_vertices,
        mask_patch: input.mask_patch,
        first_triangle_id: input.first_triangle_id,
    })?;
    let regional = springjustment_regional_from_refinement_fortran_indexed(
        SpringjustmentRegionalFromRefinementInput {
            triangle_lonlat: input.triangle_lonlat,
            cell_lonlat: input.cell_lonlat,
            cells_on_triangle: input.cells_on_triangle,
            triangles_on_cell: input.triangles_on_cell,
            n_edges_on_cell: input.n_edges_on_cell,
            refined_triangles: &refined_triangles,
            set_dis: input.set_dis,
            protected_seed_cells: input.protected_seed_cells,
            vertex_protect_layers: input.vertex_protect_layers,
            niter_refine: input.niter_refine,
            radius: input.radius,
            diagnostic_every: input.diagnostic_every,
        },
    )?;

    Some(SpringjustmentRegionalFromSourceMaskOutput {
        refined_triangles,
        regional,
    })
}
