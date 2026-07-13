use super::*;

/// Topological fallback vertex-ring orderer (used when the geometric
/// `order_vertices_on_cell_one_based` fails). Walks shared edges to
/// rebuild each cell's vertex cycle, then orients it counterclockwise as seen
/// from outside the sphere (`cross(v_i - c, v_{i+1} - c) . c > 0`, the same
/// convention as the geometric orderer). Without the orientation pass the walk
/// direction was decided by vertex-ID magnitude, so a clockwise ring could
/// silently reach the CCW-assuming area/quality consumers downstream.
pub fn order_vertices_on_cell_by_shared_edges_one_based(
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
    edges_on_vertex: &[[usize; 3]],
    vertex_points: &[CartesianPoint],
    cell_points: &[CartesianPoint],
) -> Option<Vec<Vec<usize>>> {
    let debug = std::env::var_os("EARTHMESH_MPAS_DEBUG").is_some();
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        if debug {
            eprintln!(
                "EARTHMESH_MPAS_DEBUG: n_edges_on_cell len {} < vertices_on_cell len {}",
                n_edges_on_cell.len(),
                vertices_on_cell.len()
            );
        }
        return None;
    }
    let mut ordered = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne <= 2 {
            continue;
        }
        if vertices_on_cell[cell_id].len() < ne {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} vertices len {} < n_edges {ne}",
                    vertices_on_cell[cell_id].len()
                );
            }
            return None;
        }
        let active = vertices_on_cell[cell_id][0..ne]
            .iter()
            .copied()
            .filter(|vertex| *vertex > 0)
            .collect::<Vec<_>>();
        if active.len() != ne {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} has inactive vertices active={active:?} ne={ne} row={:?}",
                    vertices_on_cell[cell_id]
                );
            }
            return None;
        }

        let start = *active.iter().min()?;
        let mut start_neighbors = active
            .iter()
            .copied()
            .filter(|candidate| {
                *candidate != start
                    && vertices_share_edge(start, *candidate, edges_on_vertex).unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if start_neighbors.len() != 2 {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} start {start} neighbor count {} active={active:?} row={:?}",
                    start_neighbors.len(),
                    vertices_on_cell[cell_id]
                );
            }
            return None;
        }
        start_neighbors.sort_unstable();

        let mut cycle = vec![start, start_neighbors[0]];
        while cycle.len() < ne {
            let prev = cycle[cycle.len() - 2];
            let current = cycle[cycle.len() - 1];
            let mut next_candidates = active
                .iter()
                .copied()
                .filter(|candidate| {
                    *candidate != prev
                        && !cycle.contains(candidate)
                        && vertices_share_edge(current, *candidate, edges_on_vertex)
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            if next_candidates.len() != 1 {
                if debug {
                    eprintln!(
                        "EARTHMESH_MPAS_DEBUG: cell {cell_id} current {current} next candidate count {} active={active:?} cycle={cycle:?} row={:?}",
                        next_candidates.len(),
                        vertices_on_cell[cell_id]
                    );
                }
                return None;
            }
            cycle.push(next_candidates.remove(0));
        }
        if !vertices_share_edge(*cycle.last()?, start, edges_on_vertex)? {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} cycle does not close start={start} cycle={cycle:?} row={:?}",
                    vertices_on_cell[cell_id]
                );
            }
            return None;
        }

        // Orient the cycle CCW (outward normal convention of the geometric
        // orderer). The walk above picked its direction by vertex-ID order,
        // which is geometrically arbitrary.
        let Some(&cell_center) = cell_points.get(cell_id) else {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} missing cell point for orientation"
                );
            }
            return None;
        };
        let center_mag = magnitude(cell_center);
        if center_mag == 0.0 {
            if debug {
                eprintln!("EARTHMESH_MPAS_DEBUG: cell {cell_id} has zero-magnitude center");
            }
            return None;
        }
        let normal = CartesianPoint::new(
            cell_center.x / center_mag,
            cell_center.y / center_mag,
            cell_center.z / center_mag,
        );
        let mut orientation = 0.0;
        for k in 0..cycle.len() {
            let Some(&pa) = vertex_points.get(cycle[k]) else {
                if debug {
                    eprintln!(
                        "EARTHMESH_MPAS_DEBUG: cell {cell_id} vertex {} missing point for orientation",
                        cycle[k]
                    );
                }
                return None;
            };
            let Some(&pb) = vertex_points.get(cycle[(k + 1) % cycle.len()]) else {
                if debug {
                    eprintln!(
                        "EARTHMESH_MPAS_DEBUG: cell {cell_id} vertex {} missing point for orientation",
                        cycle[(k + 1) % cycle.len()]
                    );
                }
                return None;
            };
            let va = vector_between(cell_center, pa);
            let vb = vector_between(cell_center, pb);
            orientation += dot(cross(va, vb), normal);
        }
        if orientation < 0.0 {
            // Reverse the walk direction while keeping the deterministic
            // min-id start vertex in slot 0.
            cycle[1..].reverse();
        }
        ordered[cell_id][0..ne].copy_from_slice(&cycle);
    }
    Some(ordered)
}

fn vertices_share_edge(
    vertex1: usize,
    vertex2: usize,
    edges_on_vertex: &[[usize; 3]],
) -> Option<bool> {
    let edges_vertex1 = edges_on_vertex.get(vertex1)?;
    let edges_vertex2 = edges_on_vertex.get(vertex2)?;
    Some(
        edges_vertex1
            .iter()
            .any(|edge| *edge > 0 && edges_vertex2.contains(edge)),
    )
}
