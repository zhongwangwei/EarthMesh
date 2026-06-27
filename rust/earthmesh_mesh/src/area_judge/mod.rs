fn regional_boundary_mask_fortran_indexed(
    triangle_flags: &[bool],
    triangles_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<bool>> {
    if triangles_on_cell.len() != n_edges_on_cell.len() {
        return None;
    }
    let mut boundary = vec![false; triangles_on_cell.len()];
    for cell_id in 2..triangles_on_cell.len() {
        let edge_count = n_edges_on_cell[cell_id];
        let cell_triangles = triangles_on_cell.get(cell_id)?;
        if edge_count == 0 {
            continue;
        }
        if edge_count > cell_triangles.len() {
            return None;
        }
        let mut flagged = 0usize;
        for &triangle_id in cell_triangles.iter().take(edge_count) {
            if *triangle_flags.get(triangle_id)? {
                flagged += 1;
            }
        }
        boundary[cell_id] = flagged != 0 && flagged != edge_count;
    }
    Some(boundary)
}

pub(crate) fn expand_triangles_from_boundary_fortran_indexed(
    mut triangle_flags: Vec<bool>,
    triangles_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
    expansion_layers: usize,
) -> Option<(Vec<bool>, Vec<bool>)> {
    let mut boundary = regional_boundary_mask_fortran_indexed(
        &triangle_flags,
        triangles_on_cell,
        n_edges_on_cell,
    )?;
    for _ in 0..expansion_layers {
        for cell_id in 2..boundary.len() {
            if !boundary[cell_id] {
                continue;
            }
            let edge_count = n_edges_on_cell[cell_id];
            let cell_triangles = triangles_on_cell.get(cell_id)?;
            if edge_count > cell_triangles.len() {
                return None;
            }
            for &triangle_id in cell_triangles.iter().take(edge_count) {
                *triangle_flags.get_mut(triangle_id)? = true;
            }
        }
        boundary = regional_boundary_mask_fortran_indexed(
            &triangle_flags,
            triangles_on_cell,
            n_edges_on_cell,
        )?;
    }
    Some((triangle_flags, boundary))
}

pub(crate) fn source_find_lon_fortran_indexed(
    source_lon_vertices: &[f64],
    lon: f64,
) -> Option<usize> {
    (1..source_lon_vertices.len()).find(|&index| lon <= source_lon_vertices[index])
}

pub(crate) fn source_find_lat_fortran_indexed(
    source_lat_vertices: &[f64],
    lat: f64,
) -> Option<usize> {
    (1..source_lat_vertices.len()).find(|&index| lat >= source_lat_vertices[index])
}
