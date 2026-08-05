use super::*;

/// Port of `MOD_grid_preprocess:CheckLon`.
///
/// The Canonical routine performs a single +/-360 adjustment rather than a full
/// modulo normalization. Preserve that behavior for parity.
pub fn normalize_lon_m180_180(lon_degrees: f64) -> f64 {
    if lon_degrees > 180.0 {
        lon_degrees - 360.0
    } else if lon_degrees < -180.0 {
        lon_degrees + 360.0
    } else {
        lon_degrees
    }
}

/// Port of the swap predicate in `MOD_grid_preprocess:GetSort_verticesOnEdge`.
///
/// Canonical compares the 2-D cross product between the cell-center edge vector
/// and the current vertex-edge vector. If `res > 0`, ordering is kept; otherwise
/// `verticesOnEdge(1:2, i)` is swapped.
pub fn should_swap_vertices_on_edge(
    cell1: LonLatDegrees,
    cell2: LonLatDegrees,
    vertex1: LonLatDegrees,
    vertex2: LonLatDegrees,
) -> bool {
    let cell_delta_lon = normalize_lon_m180_180(cell2.lon_degrees - cell1.lon_degrees);
    let cell_delta_lat = cell2.lat_degrees - cell1.lat_degrees;
    let vertex_delta_lon = normalize_lon_m180_180(vertex2.lon_degrees - vertex1.lon_degrees);
    let vertex_delta_lat = vertex2.lat_degrees - vertex1.lat_degrees;

    let cross = cell_delta_lon * vertex_delta_lat - cell_delta_lat * vertex_delta_lon;
    cross <= 0.0
}

/// Port of `MOD_grid_preprocess:GetSort_verticesOnEdge`.
///
/// Returns a sorted copy of `verticesOnEdge`, preserving the Canonical convention
/// that edge ids start at `2`. Each edge is swapped when the current
/// cross-product predicate indicates Canonical would exchange
/// `verticesOnEdge(1:2, i)`.
pub fn order_vertices_on_edge_one_based(
    point_lonlat: &[LonLatDegrees],
    cell_lonlat: &[LonLatDegrees],
    cells_on_edge: &[[usize; 2]],
    vertices_on_edge: &[[usize; 2]],
) -> Option<Vec<[usize; 2]>> {
    if cells_on_edge.len() != vertices_on_edge.len() {
        return None;
    }

    let mut ordered = vertices_on_edge.to_vec();
    for edge_id in 2..vertices_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let vertices = ordered[edge_id];
        let cell1 = *cell_lonlat.get(cells[0])?;
        let vertex1 = *point_lonlat.get(vertices[0])?;
        let vertex2 = *point_lonlat.get(vertices[1])?;
        let swap = if cells[1] == 0 {
            // MPAS uses 0 as the exterior cell on a limited-area boundary.
            // Match MpasMeshConverter.x: orient against a midpoint ghost in
            // three dimensions so polar and date-line boundaries are safe.
            let ghost =
                lonlat_degrees_to_unit_xyz(spherical_centroid_degrees(&[vertex1, vertex2])?);
            let cell = lonlat_degrees_to_unit_xyz(cell1);
            let v1 = lonlat_degrees_to_unit_xyz(vertex1);
            let v2 = lonlat_degrees_to_unit_xyz(vertex2);
            dot(
                cross(vector_between(cell, ghost), vector_between(v1, v2)),
                cell,
            ) < 0.0
        } else {
            should_swap_vertices_on_edge(cell1, *cell_lonlat.get(cells[1])?, vertex1, vertex2)
        };

        if swap {
            ordered[edge_id].swap(0, 1);
        }
    }

    Some(ordered)
}

pub fn next_ccw_edge_candidate_slot(
    vertex: CartesianPoint,
    canonical_edge: CartesianPoint,
    candidate_edges: &[CartesianPoint],
) -> Option<usize> {
    let normal = vertex;
    let normal_mag = magnitude(normal);
    let canonical_vec = vector_between(vertex, canonical_edge);
    let canonical_mag = magnitude(canonical_vec);
    let mut min_angle = std::f64::consts::PI * 2.0;
    let mut best_slot = None;

    for (slot, candidate_edge) in candidate_edges.iter().copied().enumerate() {
        let candidate_vec = vector_between(vertex, candidate_edge);
        let candidate_mag = magnitude(candidate_vec);
        let cross_prod = cross(canonical_vec, candidate_vec);
        let cross_mag = magnitude(cross_prod);

        if cross_mag > 1.0e-15 && normal_mag > 1.0e-15 {
            let dot_val = dot(cross_prod, normal) / (cross_mag * normal_mag);
            if dot_val > 0.0 {
                let denom = canonical_mag * candidate_mag;
                if denom == 0.0 {
                    continue;
                }
                let cos_angle = (dot(canonical_vec, candidate_vec) / denom).clamp(-1.0, 1.0);
                let angle = cos_angle.acos();
                if angle < min_angle {
                    min_angle = angle;
                    best_slot = Some(slot);
                }
            }
        }
    }

    best_slot
}
