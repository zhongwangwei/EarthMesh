use super::*;

/// Borrowed inputs for the Canonical-indexed subset of `MOD_grid_preprocess:GetArea`.
///
/// Index `0` is unused and index `1` is skipped to mirror the Canonical loops
/// that run from `2` through the allocated counts. Positive connectivity ids
/// are therefore used directly as Rust vector indices.
#[derive(Debug, Clone, Copy)]
pub struct GetAreaUnitInput<'a> {
    pub vertices: &'a [CartesianPoint],
    pub edge_points: &'a [CartesianPoint],
    pub cell_points: &'a [CartesianPoint],
    pub cells_on_vertex: &'a [[usize; 3]],
    pub edges_on_vertex: &'a [[usize; 3]],
    pub cells_on_edge: &'a [[usize; 2]],
    pub vertices_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
}

/// Unit-sphere area outputs from the Canonical-indexed `GetArea` subset.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAreaUnitOutput {
    pub kite_areas_on_vertex: Vec<[f64; 3]>,
    pub area_triangle: Vec<f64>,
    pub area_cell: Vec<f64>,
}

/// Relative reconstruction error summary printed by `MOD_grid_preprocess:GetArea`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaTriangleReconstructionError {
    pub max_relative: f64,
    pub avg_relative: f64,
}

/// Port of the core array workflow in `MOD_grid_preprocess:GetArea`.
///
/// This keeps the Canonical indexing convention and computes:
///
/// - `kiteAreasOnVertex(icv, i)` from consecutive edge pairs around a vertex.
/// - `areaTriangle(i)` as the sum of the three kite slots for each vertex.
/// - `areaCell(i)` by fan-triangulating `verticesOnCell(:, i)`.
pub fn get_area_unit_one_based(input: GetAreaUnitInput<'_>) -> Option<GetAreaUnitOutput> {
    if input.cells_on_vertex.len() < input.vertices.len()
        || input.edges_on_vertex.len() < input.vertices.len()
    {
        return None;
    }

    let mut kite_areas_on_vertex = vec![[0.0; 3]; input.vertices.len()];
    let mut area_triangle = vec![0.0; input.vertices.len()];
    if input.n_edges_on_cell.len() < input.cell_points.len() {
        return None;
    }

    let mut area_cell = vec![0.0; input.cell_points.len()];

    for vertex_id in 2..input.vertices.len() {
        let vertex = input.vertices[vertex_id];
        let cells_on_vertex = input.cells_on_vertex[vertex_id];
        let edges_on_vertex = input.edges_on_vertex[vertex_id];

        for edge_slot in 0..3 {
            let next_edge_slot = (edge_slot + 1) % 3;
            let edge1 = edges_on_vertex[edge_slot];
            let edge2 = edges_on_vertex[next_edge_slot];
            if edge1 == 0 || edge2 == 0 {
                continue;
            }

            let edge1_cells = *input.cells_on_edge.get(edge1)?;
            let edge2_cells = *input.cells_on_edge.get(edge2)?;
            let Some(cell_id) = shared_cell_for_edge_pair(edge1_cells, edge2_cells) else {
                continue;
            };
            let Some(vertex_cell_slot) = vertex_cell_position(cells_on_vertex, cell_id) else {
                continue;
            };

            let edge1_point = *input.edge_points.get(edge1)?;
            let edge2_point = *input.edge_points.get(edge2)?;
            let cell_point = *input.cell_points.get(cell_id)?;
            kite_areas_on_vertex[vertex_id][vertex_cell_slot] =
                spherical_kite_area_unit(vertex, edge1_point, edge2_point, cell_point);
        }
    }

    for vertex_id in 2..input.vertices.len() {
        area_triangle[vertex_id] = kite_areas_on_vertex[vertex_id].iter().sum();
    }

    for cell_id in 2..input.cell_points.len() {
        let Some(vertex_ids) = input.vertices_on_cell.get(cell_id) else {
            continue;
        };
        if vertex_ids.len() < 3 {
            continue;
        }
        let num_edges = *input.n_edges_on_cell.get(cell_id)?;
        if num_edges < 3 || num_edges > vertex_ids.len() {
            continue;
        }
        let vertices = vertex_ids
            .iter()
            .map(|vertex_id| input.vertices.get(*vertex_id).copied())
            .collect::<Option<Vec<_>>>()?;
        area_cell[cell_id] = spherical_cell_area_from_vertices_unit(&vertices, num_edges)?;
    }

    Some(GetAreaUnitOutput {
        kite_areas_on_vertex,
        area_triangle,
        area_cell,
    })
}
