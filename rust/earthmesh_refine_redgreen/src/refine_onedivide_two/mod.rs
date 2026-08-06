use std::io;

use crate::{
    average_lonlat3, crossline_check_canonical, midpoint_lonlat, validate_triangle_neighbor_rows,
    LonLatDegrees,
};

/// Port of `MOD_refine.F90:OnedivideTwo`.
///
/// Splits each marked transition triangle into two child triangles.  Forward
/// mode chooses the neighboring already-refined triangle (`mrl_new == 4`) to
/// identify the shared edge; reverse mode chooses the neighboring unrefined
/// triangle (`mrl_new == 1`).  The parent triangle connectivity is cleared to
/// Canonical placeholder `1`, child connectivity and `sjx_child` are filled, and
/// dateline-crossing coordinates follow the Canonical `CheckCrossing` and
/// `crossline_check` rules.
pub fn refine_onedivide_two_one_based(
    iter: usize,
    is_reverse: bool,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    ref_sjx: &[i32],
    mrl_new: &[i32],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle_new: &mut [[usize; 3]],
    sjx_child: &mut [[usize; 2]],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let previous_sjx_points = num_mp[iter - 1];
    let sjx_points = *num_mp
        .get(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
    if sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx.len()
        || sjx_points >= mrl_new.len()
        || sjx_points >= sjx_child.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("base sjx_points {sjx_points} must be addressable in triangle arrays"),
        ));
    }
    if previous_sjx_points >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "previous sjx_points {previous_sjx_points} must be addressable in renewed triangle connectivity"
            ),
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_mp[iter] >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[{iter}] {} exceeds triangle output storage",
                num_mp[iter]
            ),
        ));
    }
    if num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_wp[{iter}] {} exceeds cell output storage",
                num_wp[iter]
            ),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds previous sjx_points {sjx_points}"),
        ));
    }
    validate_triangle_neighbor_rows(num_vertex, sjx_points, triangle_neighbors)?;

    let mut refed_iter = 0_usize;
    for triangle in (num_vertex + 1)..=sjx_points {
        if ref_sjx[triangle] == 0 {
            continue;
        }
        let required_state = if is_reverse { 1 } else { 4 };
        let split_neighbor = triangle_neighbors[triangle]
            .iter()
            .copied().rfind(|&neighbor| mrl_new[neighbor] == required_state)
            .ok_or_else(|| {
                let neighbor_states: Vec<(usize, i32)> = triangle_neighbors[triangle]
                    .iter()
                    .copied()
                    .map(|neighbor| (neighbor, mrl_new.get(neighbor).copied().unwrap_or_default()))
                    .collect();
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "triangle {triangle} has no {} neighbor for one-into-two split \
                         (iter={iter}, num_mp[1]={sjx_points}, previous_num_mp={previous_sjx_points}, neighbors={neighbor_states:?})",
                        if is_reverse { "unrefined" } else { "refined" }
                    ),
                )
            })?;
        let neighbor_cells = cells_on_triangle[split_neighbor];
        let parent_cells = cells_on_triangle_new[triangle];
        let unique_pos = parent_cells
            .iter()
            .position(|cell| !neighbor_cells.contains(cell))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "triangle {triangle} has no vertex opposite split neighbor {split_neighbor}"
                    ),
                )
            })?;
        let w1 = parent_cells[unique_pos];
        let w2 = parent_cells[(unique_pos + 1) % 3];
        let w3 = parent_cells[(unique_pos + 2) % 3];
        for &cell in &[w1, w2, w3] {
            if cell == 0 || cell >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid cell {cell}"),
                ));
            }
        }
        // Unshifted, and it matters most here. The edge this halves is shared
        // with a triangle the red step may have split into four, and that step
        // computes the same midpoint. The two have to agree to the last bit or
        // the vertex fails to merge and the edge between them opens -- see the
        // note in `refine_onedivide_four_renew`.
        let split_points = [cell_points[w1], cell_points[w2], cell_points[w3]];

        let new_cell_point =
            midpoint_lonlat(split_points[1], split_points[2]).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "split edge has no unique spherical midpoint",
                )
            })?;
        let child_point_a = average_lonlat3(split_points[0], new_cell_point, split_points[1])
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "first child centroid is degenerate",
                )
            })?;
        let child_point_b = average_lonlat3(split_points[0], new_cell_point, split_points[2])
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "second child centroid is degenerate",
                )
            })?;
        let m1 = num_mp[iter - 1] + refed_iter * 2 + 1;
        let m2 = num_mp[iter - 1] + refed_iter * 2 + 2;
        let w4 = num_wp[iter - 1] + refed_iter + 1;
        if m2 >= triangle_points.len()
            || m2 >= cells_on_triangle_new.len()
            || w4 >= cell_points.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle {triangle} exceeds allocated one-into-two child storage"),
            ));
        }

        cells_on_triangle_new[m1] = [w1, w2, w4];
        cells_on_triangle_new[m2] = [w1, w3, w4];
        triangle_points[m1] = child_point_a;
        triangle_points[m2] = child_point_b;
        cell_points[w4] = new_cell_point;
        cells_on_triangle_new[triangle] = [1, 1, 1];
        sjx_child[triangle] = [m1, m2];
        refed_iter += 1;
    }

    crossline_check_canonical(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}
