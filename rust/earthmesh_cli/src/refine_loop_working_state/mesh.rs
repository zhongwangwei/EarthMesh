use std::io;

use crate::{LonLatPoint, UnstructuredMesh};

pub(crate) fn derive_triangle_neighbors_from_one_based_membership(
    num_triangles: usize,
    num_vertices: usize,
    ngrmw: &[Vec<usize>],
    ngrwm: &[Vec<usize>],
    n_ngrwm: &[usize],
) -> Vec<Vec<usize>> {
    let mut neighbors = vec![vec![0_usize; 3]; num_triangles + 1];
    if ngrmw.len() <= 3 || n_ngrwm.len() <= num_vertices {
        return neighbors;
    }
    for triangle in 1..=num_triangles {
        if ngrmw[1..=3].iter().any(|row| row.len() <= triangle) {
            continue;
        }
        let cells = [ngrmw[1][triangle], ngrmw[2][triangle], ngrmw[3][triangle]];
        for &cell in &cells {
            if cell == 0 || cell > num_vertices || cell >= n_ngrwm.len() {
                continue;
            }
            let count = n_ngrwm[cell];
            if ngrwm.len() <= count || ngrwm[1..=count].iter().any(|row| row.len() <= cell) {
                continue;
            }
            for row in 1..=count {
                let candidate = ngrwm[row][cell];
                if candidate == 0 || candidate == triangle || candidate > num_triangles {
                    continue;
                }
                if ngrmw[1..=3].iter().any(|row| row.len() <= candidate) {
                    continue;
                }
                let candidate_cells = [
                    ngrmw[1][candidate],
                    ngrmw[2][candidate],
                    ngrmw[3][candidate],
                ];
                let shared = cells
                    .iter()
                    .filter(|&&triangle_cell| {
                        triangle_cell != 0 && candidate_cells.contains(&triangle_cell)
                    })
                    .count();
                if shared != 2 {
                    continue;
                }
                if let Some(slot) = cells
                    .iter()
                    .position(|triangle_cell| !candidate_cells.contains(triangle_cell))
                {
                    neighbors[triangle][slot] = candidate;
                }
            }
        }
    }
    neighbors
}

pub(crate) fn state_arrays_to_unstructured_mesh(
    num_triangles: usize,
    num_vertices: usize,
    triangle_points: &[LonLatPoint],
    vertex_points: &[LonLatPoint],
    cells_on_triangle_rows: &[Vec<usize>],
    triangles_on_cell_rows: &[Vec<usize>],
    n_triangles_on_cell: &[usize],
) -> io::Result<UnstructuredMesh> {
    if triangle_points.len() <= num_triangles || vertex_points.len() <= num_vertices {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "point arrays must include one-based final slots",
        ));
    }
    if cells_on_triangle_rows.len() <= 3
        || cells_on_triangle_rows[1..=3]
            .iter()
            .any(|row| row.len() <= num_triangles)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ngrmw rows must include one-based final triangle slots",
        ));
    }
    if n_triangles_on_cell.len() <= num_vertices {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_ngrwm must include one-based final vertex slots",
        ));
    }
    if triangles_on_cell_rows
        .iter()
        .any(|row| row.len() <= num_vertices)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ngrwm rows must include one-based final vertex slots",
        ));
    }

    let m_points = triangle_points[1..=num_triangles].to_vec();
    let w_points = vertex_points[1..=num_vertices].to_vec();
    let m_to_w = (1..=num_triangles)
        .map(|triangle| {
            [
                cells_on_triangle_rows[1][triangle] as i32,
                cells_on_triangle_rows[2][triangle] as i32,
                cells_on_triangle_rows[3][triangle] as i32,
            ]
        })
        .collect();
    let n_w_to_m = n_triangles_on_cell[1..=num_vertices]
        .iter()
        .map(|&count| count as i32)
        .collect();
    let w_to_m = (1..=num_vertices)
        .map(|vertex| {
            triangles_on_cell_rows
                .iter()
                .skip(1)
                .map(|row| row[vertex] as i32)
                .collect()
        })
        .collect();

    Ok(UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}
