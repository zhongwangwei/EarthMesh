use std::io;

pub(crate) fn validate_refine_cell_neighbors(
    cell: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    sjx_points: usize,
    max_triangle_connectivity: Option<usize>,
) -> io::Result<()> {
    let num_edges = edge_counts[cell];
    let neighbors = &triangles_on_cell[cell];
    if num_edges > neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cell {cell} edge_count {num_edges} exceeds neighbor row length {}",
                neighbors.len()
            ),
        ));
    }
    for &triangle in &neighbors[..num_edges] {
        if triangle == 0 || triangle > sjx_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} has invalid triangle neighbor {triangle}"),
            ));
        }
        if let Some(max_triangle_connectivity) = max_triangle_connectivity {
            if triangle > max_triangle_connectivity {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "cell {cell} triangle {triangle} missing cells_on_triangle connectivity"
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn unique_triangle_cell(
    triangle: usize,
    other_triangle: usize,
    cells_on_triangle: &[[usize; 3]],
) -> io::Result<usize> {
    let cells = cells_on_triangle.get(triangle).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangle {triangle} missing cells_on_triangle connectivity"),
        )
    })?;
    let other_cells = cells_on_triangle.get(other_triangle).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangle {other_triangle} missing cells_on_triangle connectivity"),
        )
    })?;
    let mut unique = None;
    for &cell in cells {
        if cell != 0 && !other_cells.contains(&cell) {
            unique = Some(cell);
        }
    }
    unique.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangles {triangle} and {other_triangle} do not expose an opposite cell"),
        )
    })
}

pub(crate) fn validate_triangle_neighbor_rows(
    num_vertex: usize,
    sjx_points: usize,
    triangle_neighbors: &[Vec<usize>],
) -> io::Result<()> {
    for (triangle, neighbors) in triangle_neighbors
        .iter()
        .enumerate()
        .take(sjx_points + 1)
        .skip(num_vertex + 1)
    {
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle neighbor row {triangle} must contain exactly three neighbors"),
            ));
        }
        for &neighbor in neighbors {
            if neighbor > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle neighbor row {triangle} has invalid neighbor {neighbor}"),
                ));
            }
        }
    }
    Ok(())
}
