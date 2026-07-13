use crate::coordinates::magnitude;
use crate::{CartesianPoint, SpringEdgeAdjustment};

/// Port of the per-edge spring correction formula in
/// `MOD_grid_preprocess:spring_dynamics_global`.
///
/// `neighbor_distance_1..4` correspond to `dist(iu1..iu4)` from
/// `EdgesOnedge_tri(:, iu)`. The returned displacement is the Canonical-updated
/// `(dx, dy, dz)` after multiplying the edge vector by `frac_change`.
pub fn spring_edge_adjustment_canonical(
    cell1: CartesianPoint,
    cell2: CartesianPoint,
    target_edge_distance: f64,
    neighbor_distance_1: f64,
    neighbor_distance_2: f64,
    neighbor_distance_3: f64,
    neighbor_distance_4: f64,
) -> Option<SpringEdgeAdjustment> {
    // Canonical assigns the edge vector with `real(...)` and no kind argument
    // even though `dx/dy/dz` are real(r8), so each component is rounded through
    // default real before distance and displacement calculations.
    let edge_vector = CartesianPoint::new(
        (cell2.x - cell1.x) as f32 as f64,
        (cell2.y - cell1.y) as f32 as f64,
        (cell2.z - cell1.z) as f32 as f64,
    );
    let distance = magnitude(edge_vector);
    if distance == 0.0
        || neighbor_distance_1 == 0.0
        || neighbor_distance_2 == 0.0
        || neighbor_distance_3 == 0.0
        || neighbor_distance_4 == 0.0
    {
        return None;
    }

    let twocosphi3 = (neighbor_distance_1.powi(2) + neighbor_distance_2.powi(2) - distance.powi(2))
        / (neighbor_distance_1 * neighbor_distance_2);
    let twocosphi4 = (neighbor_distance_3.powi(2) + neighbor_distance_4.powi(2) - distance.powi(2))
        / (neighbor_distance_3 * neighbor_distance_4);
    let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
    let target_distance = target_edge_distance / 1.2 * ratio;
    let frac_change = (target_distance - distance) / distance;
    let displacement = CartesianPoint::new(
        edge_vector.x * frac_change,
        edge_vector.y * frac_change,
        edge_vector.z * frac_change,
    );

    Some(SpringEdgeAdjustment {
        displacement,
        distance,
        ratio,
        target_distance,
        frac_change,
        frac_change_squared: frac_change * frac_change,
    })
}

/// Port of the `dirs(j, iw)` sign setup in
/// `MOD_grid_preprocess:spring_dynamics_global`.
///
/// For each cell edge, Canonical assigns `+relax` when the current cell is
/// `CellsOnEdge(2, edge)` and `-relax` otherwise. Rows preserve the compact
/// `edgesOnCell` row length supplied for each Canonical-indexed cell id.
pub fn spring_edge_directions_one_based(
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    cells_on_edge: &[[usize; 2]],
    relax: f64,
) -> Option<Vec<Vec<f64>>> {
    if n_edges_on_cell.len() != edges_on_cell.len() {
        return None;
    }

    let mut directions = vec![Vec::<f64>::new(); n_edges_on_cell.len()];
    for cell_id in 2..n_edges_on_cell.len() {
        let edge_count = n_edges_on_cell[cell_id];
        let cell_edges = edges_on_cell.get(cell_id)?;
        if edge_count > cell_edges.len() {
            return None;
        }
        let mut row = Vec::with_capacity(edge_count);
        for &edge_id in cell_edges.iter().take(edge_count) {
            let cells = *cells_on_edge.get(edge_id)?;
            if cells[1] == cell_id {
                row.push(relax);
            } else {
                row.push(-relax);
            }
        }
        directions[cell_id] = row;
    }

    Some(directions)
}

/// Port of the cell accumulation and spherical renormalization steps inside
/// `MOD_grid_preprocess:spring_dynamics_global`.
///
/// The caller supplies the per-edge displacements already produced by
/// `spring_edge_adjustment_canonical` and the compact per-cell direction rows
/// produced by `spring_edge_directions_one_based`. This helper performs
/// the Canonical update:
/// `xew8(iw) += dirs(j, iw) * dx(edge)` for each cell edge, followed by
/// normalization back to `radius`.
pub fn spring_apply_cell_displacements_one_based(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    directions: &[Vec<f64>],
    edge_displacements: &[CartesianPoint],
    radius: f64,
) -> Option<Vec<CartesianPoint>> {
    if n_edges_on_cell.len() != cell_points.len()
        || edges_on_cell.len() != cell_points.len()
        || directions.len() != cell_points.len()
    {
        return None;
    }

    let mut updated = cell_points.to_vec();
    for cell_id in 2..cell_points.len() {
        let edge_count = n_edges_on_cell[cell_id];
        let cell_edges = edges_on_cell.get(cell_id)?;
        let cell_directions = directions.get(cell_id)?;
        if edge_count > cell_edges.len() || edge_count > cell_directions.len() {
            return None;
        }

        let mut point = updated[cell_id];
        for slot in 0..edge_count {
            let edge_id = cell_edges[slot];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = cell_directions[slot];
            point.x += direction * displacement.x;
            point.y += direction * displacement.y;
            point.z += direction * displacement.z;
        }

        let norm = magnitude(point);
        if norm == 0.0 {
            return None;
        }
        let expansion = radius / norm;
        updated[cell_id] = CartesianPoint::new(
            point.x * expansion,
            point.y * expansion,
            point.z * expansion,
        );
    }

    Some(updated)
}
