use crate::coordinates::{magnitude, vector_between};
use crate::{CartesianPoint, SpringDiagnosticMaxDisplacement, SpringDynamicsRegionalOutput};
use rayon::prelude::*;

/// Rust port of `MOD_grid_preprocess:spring_dynamics_regionalv2`.
///
/// The Canonical routine builds a compact calculation set from every movable
/// cell plus its neighbor cells, but only cells flagged by `IsdbxMove` are
/// updated. Each moved cell is replaced by the average of its neighboring cell
/// coordinates from the previous iteration and then projected back to `radius`.
pub fn spring_dynamics_regional_one_based(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    cells_on_cell: &[Vec<usize>],
    move_mask: &[bool],
    niter_refine: usize,
    radius: f64,
    diagnostic_every: usize,
) -> Option<SpringDynamicsRegionalOutput> {
    if diagnostic_every == 0
        || n_edges_on_cell.len() != cell_points.len()
        || cells_on_cell.len() != cell_points.len()
        || move_mask.len() != cell_points.len()
    {
        return None;
    }

    let mut calculated_mask = move_mask.to_vec();
    for cell_id in 2..cell_points.len() {
        if !move_mask[cell_id] {
            continue;
        }
        let edge_count = n_edges_on_cell[cell_id];
        let neighbors = cells_on_cell.get(cell_id)?;
        if edge_count == 0 || edge_count > neighbors.len() {
            return None;
        }
        for &neighbor_id in neighbors.iter().take(edge_count) {
            *calculated_mask.get_mut(neighbor_id)? = true;
        }
    }

    let calculated_cells = (2..cell_points.len())
        .filter(|&cell_id| calculated_mask[cell_id])
        .collect::<Vec<_>>();
    let moved_cells = (2..cell_points.len())
        .filter(|&cell_id| move_mask[cell_id])
        .collect::<Vec<_>>();

    let mut current_cell_points = cell_points.to_vec();
    let mut next_cell_points = cell_points.to_vec();
    let mut diagnostic_max_displacements = Vec::new();

    for iteration in 1..=niter_refine {
        next_cell_points
            .par_iter_mut()
            .enumerate()
            .skip(2)
            .try_for_each(|(cell_id, next)| {
                if !move_mask[cell_id] {
                    *next = current_cell_points[cell_id];
                    return Some(());
                }
                let edge_count = n_edges_on_cell[cell_id];
                let neighbors = cells_on_cell.get(cell_id)?;
                if edge_count == 0 || edge_count > neighbors.len() {
                    return None;
                }
                let mut averaged = CartesianPoint::new(0.0, 0.0, 0.0);
                for &neighbor_id in neighbors.iter().take(edge_count) {
                    let neighbor = *current_cell_points.get(neighbor_id)?;
                    averaged.x += neighbor.x / edge_count as f64;
                    averaged.y += neighbor.y / edge_count as f64;
                    averaged.z += neighbor.z / edge_count as f64;
                }
                let norm = magnitude(averaged);
                if norm == 0.0 {
                    return None;
                }
                let expansion = radius / norm;
                *next = CartesianPoint::new(
                    averaged.x * expansion,
                    averaged.y * expansion,
                    averaged.z * expansion,
                );
                Some(())
            })?;

        if iteration == 1 || iteration % diagnostic_every == 0 {
            let max_displacement = moved_cells
                .par_iter()
                .map(|&cell_id| {
                    magnitude(vector_between(
                        current_cell_points[cell_id],
                        next_cell_points[cell_id],
                    ))
                })
                .reduce(|| 0.0_f64, f64::max);
            diagnostic_max_displacements.push(SpringDiagnosticMaxDisplacement {
                iteration,
                max_displacement,
            });
        }
        std::mem::swap(&mut current_cell_points, &mut next_cell_points);
    }

    Some(SpringDynamicsRegionalOutput {
        updated_cell_points: current_cell_points,
        calculated_cells,
        moved_cells,
        diagnostic_max_displacements,
    })
}
