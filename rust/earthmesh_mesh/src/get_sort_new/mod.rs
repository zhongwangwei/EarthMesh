use super::*;

/// Port of `MOD_grid_preprocess.F90:GetSortNew` for final cell adjacency order.
///
/// For each cell `2..=num_dbx`, walks adjacent triangles using `IsNgrmm`, falls
/// back to the next unused input triangle when the walk is disconnected, then
/// reverses clockwise orders according to `robust_spherical_area_unit`.
pub fn get_sort_new_one_based(
    num_dbx: usize,
    n_triangles_on_cell: &[usize],
    cells_on_triangle: &[[usize; 3]],
    triangle_points: &[LonLatDegrees],
    triangles_on_cell: &mut [Vec<usize>],
) -> io::Result<()> {
    if num_dbx >= n_triangles_on_cell.len() || num_dbx >= triangles_on_cell.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_dbx must address cell adjacency arrays",
        ));
    }

    for cell in 2..=num_dbx {
        let num_inter = n_triangles_on_cell[cell];
        if triangles_on_cell[cell].len() < num_inter {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} adjacency row shorter than n_triangles_on_cell"),
            ));
        }
        if num_inter <= 1 {
            triangles_on_cell[cell].truncate(num_inter);
            continue;
        }
        let input = triangles_on_cell[cell][..num_inter].to_vec();
        for &triangle in &input {
            if triangle == 0
                || triangle >= cells_on_triangle.len()
                || triangle >= triangle_points.len()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} canonicals invalid triangle {triangle}"),
                ));
            }
        }

        let mut neighbor_degree = vec![0_usize; num_inter];
        for j in 0..num_inter {
            for next_pos in 0..num_inter {
                if next_pos == j {
                    continue;
                }
                if is_ngrmm(
                    cells_on_triangle[input[j]],
                    cells_on_triangle[input[next_pos]],
                )
                .is_some()
                {
                    neighbor_degree[j] += 1;
                }
            }
        }

        let start_pos = neighbor_degree
            .iter()
            .position(|&degree| degree == 1)
            .unwrap_or(0);
        let mut sorted = Vec::with_capacity(num_inter);
        let mut used = vec![false; num_inter];
        let mut ref_triangle = input[start_pos];
        sorted.push(ref_triangle);
        used[start_pos] = true;

        while sorted.len() < num_inter {
            let mut found = false;
            // Scan every slot (these are 0-based local arrays, matching the
            // degree loop above). Starting at 1 previously excluded slot 0
            // from the walk; the disconnected-fallback happened to compensate
            // on well-formed chains/cycles, but only by accident.
            for j in 0..num_inter {
                if used[j] {
                    continue;
                }
                let candidate = input[j];
                if is_ngrmm(
                    cells_on_triangle[ref_triangle],
                    cells_on_triangle[candidate],
                )
                .is_none()
                {
                    continue;
                }
                ref_triangle = candidate;
                sorted.push(ref_triangle);
                used[j] = true;
                found = true;
                break;
            }
            if !found {
                if let Some((j, &candidate)) = input.iter().enumerate().find(|(idx, _)| !used[*idx])
                {
                    ref_triangle = candidate;
                    sorted.push(ref_triangle);
                    used[j] = true;
                } else {
                    break;
                }
            }
        }

        let polygon = sorted
            .iter()
            .map(|&triangle| triangle_points[triangle])
            .collect::<Vec<_>>();
        if let Some(area) = robust_spherical_area_unit(&polygon) {
            if area < 0.0 {
                sorted.reverse();
            }
        }
        triangles_on_cell[cell] = sorted;
    }

    Ok(())
}
