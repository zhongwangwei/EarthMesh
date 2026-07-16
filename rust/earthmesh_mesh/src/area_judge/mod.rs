fn regional_boundary_mask_one_based(
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

pub(crate) fn expand_triangles_from_boundary_one_based(
    mut triangle_flags: Vec<bool>,
    triangles_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
    expansion_layers: usize,
) -> Option<(Vec<bool>, Vec<bool>)> {
    let mut boundary =
        regional_boundary_mask_one_based(&triangle_flags, triangles_on_cell, n_edges_on_cell)?;
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
        boundary =
            regional_boundary_mask_one_based(&triangle_flags, triangles_on_cell, n_edges_on_cell)?;
    }
    Some((triangle_flags, boundary))
}

pub(crate) fn source_find_lon_one_based(source_lon_vertices: &[f64], lon: f64) -> Option<usize> {
    if lon.is_nan() {
        return None;
    }
    let vertices = source_lon_vertices.get(1..)?;
    let offset = vertices.partition_point(|&vertex| vertex < lon);
    (offset < vertices.len()).then_some(offset + 1)
}

pub(crate) fn source_find_lat_one_based(source_lat_vertices: &[f64], lat: f64) -> Option<usize> {
    if lat.is_nan() {
        return None;
    }
    let vertices = source_lat_vertices.get(1..)?;
    let offset = vertices.partition_point(|&vertex| vertex > lat);
    (offset < vertices.len()).then_some(offset + 1)
}

#[cfg(test)]
mod tests {
    use super::{source_find_lat_one_based, source_find_lon_one_based};

    #[test]
    fn source_axis_binary_search_matches_first_canonical_vertex() {
        let lon = vec![0.0, -180.0, -179.0, -179.0, -178.0, 0.0, 180.0];
        let lat = vec![0.0, 90.0, 89.0, 89.0, 88.0, 0.0, -90.0];

        for query in [
            -181.0, -180.0, -179.5, -179.0, -178.5, 0.0, 179.9, 180.0, 181.0,
        ] {
            let expected = (1..lon.len()).find(|&index| query <= lon[index]);
            assert_eq!(source_find_lon_one_based(&lon, query), expected);
        }
        for query in [91.0, 90.0, 89.5, 89.0, 88.5, 0.0, -89.9, -90.0, -91.0] {
            let expected = (1..lat.len()).find(|&index| query >= lat[index]);
            assert_eq!(source_find_lat_one_based(&lat, query), expected);
        }
    }

    #[test]
    fn source_axis_binary_search_preserves_non_finite_behavior() {
        let lon = [0.0, -180.0, 0.0, 180.0];
        let lat = [0.0, 90.0, 0.0, -90.0];

        assert_eq!(source_find_lon_one_based(&lon, f64::NEG_INFINITY), Some(1));
        assert_eq!(source_find_lon_one_based(&lon, f64::INFINITY), None);
        assert_eq!(source_find_lon_one_based(&lon, f64::NAN), None);
        assert_eq!(source_find_lat_one_based(&lat, f64::INFINITY), Some(1));
        assert_eq!(source_find_lat_one_based(&lat, f64::NEG_INFINITY), None);
        assert_eq!(source_find_lat_one_based(&lat, f64::NAN), None);
    }
}
