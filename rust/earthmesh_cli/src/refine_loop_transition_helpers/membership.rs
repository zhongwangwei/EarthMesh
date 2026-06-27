use std::io;

use crate::refine_loop_adapters::fortran_rows_to_triangle_major;
use crate::*;

pub(crate) fn transition_cell_views(
    state: &RefineLoopWorkingState,
    old_mp: usize,
    old_wp: usize,
) -> io::Result<(Vec<[usize; 3]>, Vec<Vec<usize>>)> {
    let cells_on_triangle = fortran_rows_to_triangle_major(&state.ngrmw, old_mp)?;
    if state.n_ngrwm.len() <= old_wp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("old_wp {old_wp} requires one-based n_ngrwm storage"),
        ));
    }
    let mut triangles_on_cell = vec![Vec::<usize>::new(); old_wp + 1];
    for cell in 1..=old_wp {
        let count = state.n_ngrwm[cell];
        if state.ngrwm.len() <= count || state.ngrwm[1..=count].iter().any(|row| row.len() <= cell)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} requires ngrwm rows 1..={count}"),
            ));
        }
        triangles_on_cell[cell].extend((1..=count).map(|row| state.ngrwm[row][cell]));
    }
    Ok((cells_on_triangle, triangles_on_cell))
}

pub(crate) fn refresh_working_vertex_membership_from_ngrmw_new(
    state: &mut RefineLoopWorkingState,
) -> io::Result<()> {
    let num_triangles = *state
        .num_mp
        .get(state.iter)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "iter must address num_mp"))?;
    let num_vertices = *state
        .num_wp
        .get(state.iter)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "iter must address num_wp"))?;
    if state.ngrmw_new.len() <= 3
        || state.ngrmw_new[1..=3]
            .iter()
            .any(|row| row.len() <= num_triangles)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ngrmw_new rows must address current working triangles",
        ));
    }

    let mut triangles_by_vertex = vec![Vec::<usize>::new(); num_vertices + 1];
    for triangle in 1..=num_triangles {
        for row in 1..=3 {
            let vertex = state.ngrmw_new[row][triangle];
            if vertex == 0 || vertex > num_vertices {
                continue;
            }
            triangles_by_vertex[vertex].push(triangle);
        }
    }
    let capacity = triangles_by_vertex.iter().map(Vec::len).max().unwrap_or(0);
    state.ngrwm = vec![vec![0_usize; num_vertices + 1]; capacity + 1];
    state.n_ngrwm = vec![0_usize; num_vertices + 1];
    for (vertex, triangles) in triangles_by_vertex.iter().enumerate().skip(1) {
        state.n_ngrwm[vertex] = triangles.len();
        for (row, &triangle) in triangles.iter().enumerate() {
            state.ngrwm[row + 1][vertex] = triangle;
        }
    }
    Ok(())
}
