pub fn order_vertices_on_cell_by_shared_edges_fortran_indexed(
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
    edges_on_vertex: &[[usize; 3]],
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
