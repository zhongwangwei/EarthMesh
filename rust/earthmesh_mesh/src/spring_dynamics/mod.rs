use crate::coordinates::{magnitude, vector_between};
use crate::spring_edge_dynamics::{
    spring_apply_cell_displacements_one_based, spring_edge_adjustment_canonical,
    spring_edge_directions_one_based,
};
use crate::{
    CartesianPoint, SpringDiagnosticMaxDisplacement, SpringDynamicsGlobalOutput,
    SpringGlobalIterationOutput,
};

/// One-iteration Rust wrapper for `MOD_grid_preprocess:spring_dynamics_global`.
///
/// This ports the calculation order inside one Canonical iteration: compute all
/// current edge distances, update per-edge correction vectors from
/// `EdgesOnedge_tri`, build/apply per-cell direction signs, then renormalize
/// cell coordinates back to `radius`.
pub fn spring_global_iteration_one_based(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    cells_on_edge: &[[usize; 2]],
    edges_on_edge_tri: &[[usize; 4]],
    dists_on_edge: &[f64],
    relax: f64,
    radius: f64,
) -> Option<SpringGlobalIterationOutput> {
    if cells_on_edge.len() != edges_on_edge_tri.len()
        || cells_on_edge.len() != dists_on_edge.len()
        || n_edges_on_cell.len() != cell_points.len()
        || edges_on_cell.len() != cell_points.len()
    {
        return None;
    }

    let mut edge_distances = vec![0.0; cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let cell1 = *cell_points.get(cells[0])?;
        let cell2 = *cell_points.get(cells[1])?;
        // The Canonical canonical (Method-C spring_dynamics_globe, mirrored by
        // MOD_grid_preprocess) computes `dx(iu) = real(xem8(im2) - xem8(im1))`
        // -- default-real truncated components -- and derives the single
        // `dist(iu)` array from them, which then serves BOTH the "self" and
        // "neighbor" roles of every edge in the twocosphi formula. Truncate
        // identically here so this array matches the self-distance computed in
        // `spring_edge_adjustment_canonical` for the same edge; a full-f64
        // distance made the two roles of one edge disagree at sub-meter scale
        // on Earth-radius coordinates.
        let edge_vector = CartesianPoint::new(
            (cell2.x - cell1.x) as f32 as f64,
            (cell2.y - cell1.y) as f32 as f64,
            (cell2.z - cell1.z) as f32 as f64,
        );
        edge_distances[edge_id] = magnitude(edge_vector);
    }

    let mut edge_displacements = vec![CartesianPoint::new(0.0, 0.0, 0.0); cells_on_edge.len()];
    let mut frac_change_squared = vec![0.0; cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let neighbor_edges = edges_on_edge_tri[edge_id];
        let adjustment = spring_edge_adjustment_canonical(
            *cell_points.get(cells[0])?,
            *cell_points.get(cells[1])?,
            dists_on_edge[edge_id],
            *edge_distances.get(neighbor_edges[0])?,
            *edge_distances.get(neighbor_edges[1])?,
            *edge_distances.get(neighbor_edges[2])?,
            *edge_distances.get(neighbor_edges[3])?,
        )?;
        edge_displacements[edge_id] = adjustment.displacement;
        frac_change_squared[edge_id] = adjustment.frac_change_squared;
    }

    let directions =
        spring_edge_directions_one_based(n_edges_on_cell, edges_on_cell, cells_on_edge, relax)?;
    let updated_cell_points = spring_apply_cell_displacements_one_based(
        cell_points,
        n_edges_on_cell,
        edges_on_cell,
        &directions,
        &edge_displacements,
        radius,
    )?;

    Some(SpringGlobalIterationOutput {
        updated_cell_points,
        edge_displacements,
        frac_change_squared,
    })
}

/// Multi-iteration Rust wrapper for `MOD_grid_preprocess:spring_dynamics_global`.
///
/// This keeps only the current coordinate arrays, matching the Canonical memory
/// model, and records the periodic `Max DS` diagnostics for `iter == 1` or
/// `iter % diagnostic_every == 0`.
pub fn spring_dynamics_global_one_based(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    cells_on_edge: &[[usize; 2]],
    edges_on_edge_tri: &[[usize; 4]],
    dists_on_edge: &[f64],
    niter_refine: usize,
    relax: f64,
    radius: f64,
    diagnostic_every: usize,
) -> Option<SpringDynamicsGlobalOutput> {
    if diagnostic_every == 0 {
        return None;
    }

    let mut current_cell_points = cell_points.to_vec();
    let mut diagnostic_canonical = cell_points.to_vec();
    let mut last_edge_displacements = vec![CartesianPoint::new(0.0, 0.0, 0.0); cells_on_edge.len()];
    let mut last_frac_change_squared = vec![0.0; cells_on_edge.len()];
    let mut diagnostic_max_displacements = Vec::new();

    for iteration in 1..=niter_refine {
        let record_diagnostic = iteration == 1 || iteration % diagnostic_every == 0;
        if record_diagnostic {
            diagnostic_canonical = current_cell_points.clone();
        }

        let iteration_output = spring_global_iteration_one_based(
            &current_cell_points,
            n_edges_on_cell,
            edges_on_cell,
            cells_on_edge,
            edges_on_edge_tri,
            dists_on_edge,
            relax,
            radius,
        )?;

        current_cell_points = iteration_output.updated_cell_points;
        last_edge_displacements = iteration_output.edge_displacements;
        last_frac_change_squared = iteration_output.frac_change_squared;

        if record_diagnostic {
            let mut max_displacement = 0.0_f64;
            for cell_id in 2..current_cell_points.len() {
                let before = *diagnostic_canonical.get(cell_id)?;
                let after = current_cell_points[cell_id];
                let displacement = magnitude(vector_between(before, after));
                max_displacement = max_displacement.max(displacement);
            }
            diagnostic_max_displacements.push(SpringDiagnosticMaxDisplacement {
                iteration,
                max_displacement,
            });
        }
    }

    Some(SpringDynamicsGlobalOutput {
        updated_cell_points: current_cell_points,
        last_edge_displacements,
        last_frac_change_squared,
        diagnostic_max_displacements,
    })
}
