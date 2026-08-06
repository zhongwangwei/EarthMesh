use std::io;

pub fn refine_boundary_closed_curves_one_based(
    num_vertex: usize,
    sjx_points: usize,
    num_center: usize,
    lbx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    mrl_new: &[i32],
) -> io::Result<Vec<Vec<usize>>> {
    let mut boundary_neighbors = vec![Vec::<usize>::new(); lbx_points + 1];
    let mut boundary_triangle_count = 1_usize; // Canonical keeps slot 1 empty.

    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_new[triangle] != 1 {
            continue;
        }
        let neighbor_state_sum: i32 = triangle_neighbors[triangle]
            .iter()
            .map(|&neighbor| mrl_new[neighbor])
            .sum();
        if neighbor_state_sum != 6 {
            continue;
        }
        let refined_neighbor = triangle_neighbors[triangle]
            .iter()
            .copied()
            .find(|&neighbor| mrl_new[neighbor] == 4)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary triangle {triangle} has no refined neighbor"),
                )
            })?;
        let triangle_cells = cells_on_triangle[triangle];
        let refined_cells = cells_on_triangle[refined_neighbor];
        let unique_pos = triangle_cells
            .iter()
            .position(|cell| !refined_cells.contains(cell))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} does not expose a unique boundary-opposite cell"),
                )
            })?;
        let mut shared_cells = Vec::with_capacity(2);
        for offset in 1..=2 {
            let cell = triangle_cells[(unique_pos + offset) % 3];
            if cell == 0 || cell > lbx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary triangle {triangle} canonicals invalid boundary cell {cell}"),
                ));
            }
            shared_cells.push(cell);
        }
        let w1 = shared_cells[0];
        let w2 = shared_cells[1];
        boundary_neighbors[w1].push(w2);
        boundary_neighbors[w2].push(w1);
        boundary_triangle_count += 1;
    }

    let mut boundary_order = vec![1_usize];
    for (cell, neighbors) in boundary_neighbors
        .iter()
        .enumerate()
        .take(lbx_points + 1)
        .skip(num_center + 1)
    {
        match neighbors.len() {
            0 => {}
            2 => boundary_order.push(cell),
            n => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary cell {cell} has {n} connections; expected 0 or 2"),
                ));
            }
        }
    }
    if boundary_triangle_count != boundary_order.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "boundary triangle count with empty slot {boundary_triangle_count} differs from boundary vertex count {}",
                boundary_order.len()
            ),
        ));
    }

    let mut boundary_position = vec![usize::MAX; lbx_points + 1];
    for (position, &cell) in boundary_order.iter().enumerate().skip(1) {
        boundary_position[cell] = position;
    }

    let mut available = vec![false; boundary_order.len()];
    for item in available.iter_mut().skip(1) {
        *item = true;
    }
    let mut closed_curves = Vec::new();
    while available.iter().skip(1).any(|&value| value) {
        let start_pos = available
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(idx, &value)| value.then_some(idx))
            .expect("at least one available boundary vertex");
        let start = boundary_order[start_pos];
        available[start_pos] = false;
        let mut curve = vec![start];
        let end = boundary_neighbors[start][1];
        let mut selected = boundary_neighbors[start][0];
        let mut previous = start;
        while selected != end {
            curve.push(selected);
            let selected_pos = boundary_position
                .get(selected)
                .copied()
                .filter(|&position| position != usize::MAX)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("boundary cell {selected} is not present in boundary order"),
                    )
                })?;
            if !available[selected_pos] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary curve revisits cell {selected} before closure"),
                ));
            }
            available[selected_pos] = false;
            let neighbors = &boundary_neighbors[selected];
            let next = if neighbors[0] == previous {
                neighbors[1]
            } else if neighbors[1] == previous {
                neighbors[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary cell {selected} is not connected back to {previous}"),
                ));
            };
            previous = selected;
            selected = next;
        }
        curve.push(end);
        let end_pos = boundary_position
            .get(end)
            .copied()
            .filter(|&position| position != usize::MAX)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary end cell {end} is not present in boundary order"),
                )
            })?;
        if !available[end_pos] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary curve end cell {end} was already consumed"),
            ));
        }
        available[end_pos] = false;
        if curve.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "boundary closed curve must contain at least three cells",
            ));
        }
        closed_curves.push(curve);
    }

    Ok(closed_curves)
}
