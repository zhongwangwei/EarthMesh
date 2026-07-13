/// Output of `MOD_grid_preprocess:Get_ConnectOnCell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellConnectivityOnCell {
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
}

/// Port of `MOD_grid_preprocess:Get_ConnectOnCell`.
///
/// The input `vertices_on_cell` must already be ordered around each cell. For
/// each consecutive vertex pair, this finds the shared edge from the two
/// `edgesOnVertex` triplets, then maps that edge to the neighboring cell via
/// `cellsOnEdge`.
pub fn connect_on_cell_one_based(
    n_edges_on_cell: &[usize],
    cells_on_edge: &[[usize; 2]],
    edges_on_vertex: &[[usize; 3]],
    vertices_on_cell: &[Vec<usize>],
) -> Option<CellConnectivityOnCell> {
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

    let mut edges_on_cell = vec![Vec::new(); vertices_on_cell.len()];
    let mut cells_on_cell = vec![Vec::new(); vertices_on_cell.len()];

    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if vertices_on_cell[cell_id].len() < ne {
            return None;
        }

        let mut cell_edges = Vec::with_capacity(ne);
        let mut neighbor_cells = Vec::with_capacity(ne);
        for vertex_slot in 0..ne {
            let vertex1 = vertices_on_cell[cell_id][vertex_slot];
            let vertex2 = vertices_on_cell[cell_id][(vertex_slot + 1) % ne];
            let edges_vertex1 = *edges_on_vertex.get(vertex1)?;
            let edges_vertex2 = *edges_on_vertex.get(vertex2)?;
            let edge_id = match edges_vertex1
                .iter()
                .copied()
                .find(|edge| *edge > 0 && edges_vertex2.contains(edge))
            {
                Some(edge_id) => edge_id,
                None => {
                    if debug {
                        eprintln!(
                            "EARTHMESH_MPAS_DEBUG: no shared edge cell={cell_id} slot={vertex_slot} vertex1={vertex1} vertex2={vertex2} edges1={edges_vertex1:?} edges2={edges_vertex2:?} vertices={:?}",
                            vertices_on_cell[cell_id]
                        );
                    }
                    return None;
                }
            };
            let cells = *cells_on_edge.get(edge_id)?;
            let neighbor = if cells[0] == cell_id {
                cells[1]
            } else if cells[1] == cell_id {
                cells[0]
            } else {
                if debug {
                    eprintln!(
                        "EARTHMESH_MPAS_DEBUG: edge cell mismatch cell={cell_id} slot={vertex_slot} edge={edge_id} cells_on_edge={cells:?} vertices={:?}",
                        vertices_on_cell[cell_id]
                    );
                }
                return None;
            };
            cell_edges.push(edge_id);
            neighbor_cells.push(neighbor);
        }
        edges_on_cell[cell_id] = cell_edges;
        cells_on_cell[cell_id] = neighbor_cells;
    }

    Some(CellConnectivityOnCell {
        edges_on_cell,
        cells_on_cell,
    })
}
