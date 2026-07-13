use std::io;

use crate::{boundary_closed_curves_one_based, push_boundary_neighbor, BoundaryConnection};

/// Pure-data port of `MOD_refine.F90:bdy_connection_make`.
///
/// Builds boundary vertex-vertex connections from unrefined triangles that have
/// exactly one refined neighbor (`sum(mrl_bk(ngrmm(:, i))) == 6`), validates the
/// closed boundary degree invariant, then reuses the shared closed-curve walker.
pub fn refine_boundary_connection_make_one_based(
    num_vertex: usize,
    sjx_points: usize,
    lbx_points: usize,
    mrl_bk: &[i32],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
) -> io::Result<BoundaryConnection> {
    if sjx_points >= mrl_bk.len()
        || sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sjx_points must address refinement and triangle arrays",
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_vertex must not exceed sjx_points",
        ));
    }
    let mut boundary_neighbors = vec![Vec::<usize>::new(); lbx_points + 1];
    let mut bdy_num_in_save = 1_usize;

    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_bk[triangle] != 1 {
            continue;
        }
        if triangle_neighbors[triangle].len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle {triangle} must have three neighbors"),
            ));
        }
        let mut neighbor_state_sum = 0_i32;
        for &neighbor in triangle_neighbors[triangle].iter().take(3) {
            if neighbor == 0 {
                continue;
            }
            if neighbor >= mrl_bk.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} canonicals invalid neighbor {neighbor}"),
                ));
            }
            neighbor_state_sum += mrl_bk[neighbor];
        }
        if neighbor_state_sum != 6 {
            continue;
        }
        let refined_neighbor = triangle_neighbors[triangle]
            .iter()
            .take(3)
            .copied()
            .find(|&neighbor| mrl_bk[neighbor] == 4)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} has boundary sum 6 but no refined neighbor"),
                )
            })?;
        bdy_num_in_save += 1;

        let parent_cells = cells_on_triangle[triangle];
        let refined_cells = cells_on_triangle[refined_neighbor];
        let free_pos = parent_cells
            .iter()
            .position(|cell| !refined_cells.contains(cell))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} has no vertex opposite refined neighbor"),
                )
            })?;
        let w1 = parent_cells[(free_pos + 1) % 3];
        let w2 = parent_cells[(free_pos + 2) % 3];
        for &vertex in &[w1, w2] {
            if vertex == 0 || vertex > lbx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary vertex {vertex} must be in 1..={lbx_points}"),
                ));
            }
        }
        push_boundary_neighbor(&mut boundary_neighbors, w1, w2)?;
        push_boundary_neighbor(&mut boundary_neighbors, w2, w1)?;
    }

    for vertex in (num_vertex + 1)..=lbx_points {
        if boundary_neighbors[vertex].len() == 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex} has open degree 1"),
            ));
        }
        if boundary_neighbors[vertex].len() > 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex} has more than two refine boundary neighbors"),
            ));
        }
    }

    let mut boundary_order = vec![1_usize];
    for vertex in (num_vertex + 1)..=lbx_points {
        if boundary_neighbors[vertex].len() == 2 {
            boundary_order.push(vertex);
        }
    }
    let bdy_num_in = boundary_order.len();
    if bdy_num_in_save != bdy_num_in {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refine boundary triangle count {bdy_num_in_save} does not match boundary vertex count {bdy_num_in}"
            ),
        ));
    }

    let curves = boundary_closed_curves_one_based(&boundary_order, &boundary_neighbors)?;
    Ok(BoundaryConnection {
        bdy_num_in,
        boundary_order,
        boundary_neighbors,
        curves,
    })
}
