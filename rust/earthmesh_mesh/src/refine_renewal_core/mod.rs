use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct RefineNgrRenewCore {
    pub num_sjx: usize,
    pub num_dbx: usize,
    pub triangle_points: Vec<LonLatDegrees>,
    pub cell_points: Vec<LonLatDegrees>,
    pub cells_on_triangle: Vec<[usize; 3]>,
    pub triangles_on_cell: Vec<Vec<usize>>,
    pub n_triangles_on_cell: Vec<usize>,
    pub boundary_refine: Vec<usize>,
    pub boundary_refine_transition: Vec<usize>,
    pub vertex_mapping: Vec<usize>,
}

/// Pure Rust core for `MOD_refine.F90:NGR_RENEW` before `GetSortNew` and file IO.
///
/// This preserves the Fortran-indexed data model: slot `0` is a placeholder,
/// original vertices `1..=num_wp[1]` are copied directly, new vertices are
/// deduplicated only against previously accepted new vertices, deleted
/// triangles have connectivity `[1, 1, 1]`, and triangle-to-vertex adjacency is
/// rebuilt from final compacted triangle ids starting at triangle `2`.
pub fn refine_ngr_renew_core_fortran_indexed(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points_new: &[LonLatDegrees],
    cell_points_new: &[LonLatDegrees],
    cells_on_triangle_new: &[[usize; 3]],
    boundary_refine: &[usize],
    boundary_refine_transition: &[usize],
) -> io::Result<RefineNgrRenewCore> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let original_wp = num_wp[1];
    let final_wp = num_wp[iter];
    let final_mp = num_mp[iter];
    if original_wp > final_wp || final_wp >= cell_points_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_wp bounds must address cell_points_new",
        ));
    }
    if final_mp >= triangle_points_new.len() || final_mp >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_mp bounds must address triangle inputs",
        ));
    }
    if num_vertex > final_mp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_vertex must not exceed final triangle count",
        ));
    }

    let mut vertex_mapping = vec![0_usize; final_wp + 1];
    let mut cell_points = vec![LonLatDegrees::new(9999.0, 9999.0); final_wp + 1];
    let mut num_dbx = original_wp;
    cell_points[1..=original_wp].copy_from_slice(&cell_points_new[1..=original_wp]);
    for (idx, mapping) in vertex_mapping
        .iter_mut()
        .enumerate()
        .take(original_wp + 1)
        .skip(1)
    {
        *mapping = idx;
    }

    for source_vertex in (original_wp + 1)..=final_wp {
        let duplicate = ((original_wp + 1)..=num_dbx).find(|&candidate| {
            cell_points[candidate].lon_degrees == cell_points_new[source_vertex].lon_degrees
                && cell_points[candidate].lat_degrees == cell_points_new[source_vertex].lat_degrees
        });
        if let Some(mapped) = duplicate {
            vertex_mapping[source_vertex] = mapped;
        } else {
            num_dbx += 1;
            if num_dbx >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "deduplicated vertex count exceeds allocated final cell storage",
                ));
            }
            cell_points[num_dbx] = cell_points_new[source_vertex];
            vertex_mapping[source_vertex] = num_dbx;
        }
    }
    cell_points.truncate(num_dbx + 1);
    let max_mapping = vertex_mapping.iter().copied().max().unwrap_or(0);
    if max_mapping != num_dbx {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max vertex_mapping does not match deduplicated vertex count",
        ));
    }

    let deleted_triangles = ((num_vertex + 1)..=final_mp)
        .filter(|&triangle| cells_on_triangle_new[triangle][0] == 1)
        .count();
    let num_sjx = final_mp - deleted_triangles;
    let mut triangle_points = vec![LonLatDegrees::new(0.0, 0.0); num_sjx + 1];
    let mut cells_on_triangle = vec![[1_usize, 1, 1]; num_sjx + 1];
    triangle_points[1..=num_vertex].copy_from_slice(&triangle_points_new[1..=num_vertex]);
    cells_on_triangle[1..=num_vertex].copy_from_slice(&cells_on_triangle_new[1..=num_vertex]);

    let mut out_triangle = num_vertex;
    for source_triangle in (num_vertex + 1)..=final_mp {
        if cells_on_triangle_new[source_triangle][0] == 1 {
            continue;
        }
        out_triangle += 1;
        triangle_points[out_triangle] = triangle_points_new[source_triangle];
        cells_on_triangle[out_triangle] = cells_on_triangle_new[source_triangle];
    }
    if out_triangle != num_sjx {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "compacted triangle count does not match expected num_sjx",
        ));
    }

    let mut n_triangles_on_cell = vec![0_usize; num_dbx + 1];
    for tri_cells in cells_on_triangle.iter_mut().take(num_sjx + 1).skip(2) {
        for cell in tri_cells.iter_mut() {
            if *cell == 0 || *cell >= vertex_mapping.len() || vertex_mapping[*cell] == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle references cell {cell} without final vertex mapping"),
                ));
            }
            *cell = vertex_mapping[*cell];
            n_triangles_on_cell[*cell] += 1;
        }
    }

    let mut triangles_on_cell = vec![Vec::<usize>::new(); num_dbx + 1];
    for (triangle, tri_cells) in cells_on_triangle
        .iter()
        .enumerate()
        .take(num_sjx + 1)
        .skip(2)
    {
        for &cell in tri_cells {
            triangles_on_cell[cell].push(triangle);
        }
    }

    let remap_boundary = |values: &[usize], vertex_mapping: &[usize]| -> io::Result<Vec<usize>> {
        values
            .iter()
            .map(|&value| {
                if value == 0 || value >= vertex_mapping.len() || vertex_mapping[value] == 0 {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("boundary vertex {value} has no final mapping"),
                    ))
                } else {
                    Ok(vertex_mapping[value])
                }
            })
            .collect()
    };

    Ok(RefineNgrRenewCore {
        num_sjx,
        num_dbx,
        triangle_points,
        cell_points,
        cells_on_triangle,
        triangles_on_cell,
        n_triangles_on_cell,
        boundary_refine: remap_boundary(boundary_refine, &vertex_mapping)?,
        boundary_refine_transition: remap_boundary(boundary_refine_transition, &vertex_mapping)?,
        vertex_mapping,
    })
}
