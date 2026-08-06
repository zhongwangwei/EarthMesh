use std::io;

use crate::{
    average_lonlat3, check_crossing_canonical_lonlat, crossline_check_canonical, midpoint_lonlat,
    LonLatDegrees,
};

/// Port of `MOD_refine.F90:OnedivideFour_renew`.
///
/// The red step's geometry: every triangle the segment marking holds is
/// replaced by four, built on the midpoints of its three edges.
///
/// `refine_onedivide_four_connection_one_based` decided *which* triangles and
/// updated the refinement state; this one produces the points and the vertex
/// table. The two are separate because the connection pass runs many times --
/// once per judge round, as the marking grows -- and the geometry only once,
/// when the marking has stopped changing.
///
/// New ids follow the canonical block layout: the `k`-th triangle in ascending
/// order takes triangle centres `num_mp[1] + 4k + 1 ..= num_mp[1] + 4k + 4` and
/// cell points `num_wp[1] + 3k + 1 ..= num_wp[1] + 3k + 3`. The parent's own row
/// is blanked to `[1, 1, 1]`, the canonical deleted marker, rather than removed:
/// every later stage still addresses rows by the id they had.
///
/// Midpoints are geodesic (unit vectors summed and renormalised), not the
/// Fortran's lon/lat average, matching the rest of this crate. The dateline
/// dance around it is kept because the *centroids* are still built from lon/lat
/// triples, so a triangle straddling ±180 has to be unwrapped first and folded
/// back after.
#[allow(clippy::too_many_arguments)]
pub fn refine_onedivide_four_renew_one_based(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    cells_on_triangle: &[[usize; 3]],
    ref_sjx_segment: &[i32],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle_new: &mut [[usize; 3]],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let sjx_points = num_mp[1];
    let lbx_points = num_wp[1];
    if sjx_points >= cells_on_triangle.len() || sjx_points >= ref_sjx_segment.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sjx_points {sjx_points} must be addressable in all triangle arrays"),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }

    let mut refined = 0usize;
    for triangle in num_vertex + 1..=sjx_points {
        if ref_sjx_segment[triangle] == 0 {
            continue;
        }
        let corners = cells_on_triangle[triangle];
        for corner in corners {
            if corner >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("triangle {triangle} names cell point {corner} outside the mesh"),
                ));
            }
        }
        let mut split = [
            cell_points[corners[0]],
            cell_points[corners[1]],
            cell_points[corners[2]],
        ];
        let crosses_dateline = split
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::NEG_INFINITY, f64::max)
            - split
                .iter()
                .map(|point| point.lon_degrees)
                .fold(f64::INFINITY, f64::min)
            > 180.0;
        if crosses_dateline {
            check_crossing_canonical_lonlat(&mut split);
        }

        // Each new cell point is the midpoint of the edge *opposite* the corner
        // of the same index, which is what makes the four children come out in
        // counter-clockwise order.
        let opposite_midpoint = |a: LonLatDegrees, b: LonLatDegrees| {
            midpoint_lonlat(a, b).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("triangle {triangle} has an antipodal edge with no unique midpoint"),
                )
            })
        };
        let mut new_cells = [
            opposite_midpoint(split[1], split[2])?,
            opposite_midpoint(split[0], split[2])?,
            opposite_midpoint(split[0], split[1])?,
        ];

        let centroid = |a: LonLatDegrees, b: LonLatDegrees, c: LonLatDegrees| {
            average_lonlat3(a, b, c).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("triangle {triangle} has a child with no representable centre"),
                )
            })
        };
        let mut new_triangles = [
            centroid(split[0], new_cells[1], new_cells[2])?,
            centroid(split[1], new_cells[0], new_cells[2])?,
            centroid(split[2], new_cells[0], new_cells[1])?,
            centroid(new_cells[2], new_cells[0], new_cells[1])?,
        ];

        if crosses_dateline {
            check_crossing_canonical_lonlat(&mut new_triangles);
            check_crossing_canonical_lonlat(&mut new_cells);
        }

        let m0 = sjx_points + refined * 4;
        let w0 = lbx_points + refined * 3;
        if m0 + 4 >= triangle_points.len()
            || m0 + 4 >= cells_on_triangle_new.len()
            || w0 + 3 >= cell_points.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refinement storage is short: child block m{}..={} w{}..={} exceeds it",
                    m0 + 1,
                    m0 + 4,
                    w0 + 1,
                    w0 + 3
                ),
            ));
        }
        triangle_points[m0 + 1..=m0 + 4].copy_from_slice(&new_triangles);
        cell_points[w0 + 1..=w0 + 3].copy_from_slice(&new_cells);

        // Corner children keep the parent's corner of the same index and take
        // the two new cell points that are not opposite it; the middle child is
        // the three new points alone.
        cells_on_triangle_new[m0 + 1] = [corners[0], w0 + 3, w0 + 2];
        cells_on_triangle_new[m0 + 2] = [corners[1], w0 + 1, w0 + 3];
        cells_on_triangle_new[m0 + 3] = [corners[2], w0 + 2, w0 + 1];
        cells_on_triangle_new[m0 + 4] = [w0 + 1, w0 + 2, w0 + 3];
        cells_on_triangle_new[triangle] = [1, 1, 1];

        refined += 1;
    }

    crossline_check_canonical(iter, num_mp, num_wp, triangle_points, cell_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One triangle with corners at 3, 4, 5 and room for its four children.
    fn one_triangle() -> (
        Vec<[usize; 3]>,
        Vec<i32>,
        Vec<LonLatDegrees>,
        Vec<LonLatDegrees>,
        Vec<[usize; 3]>,
    ) {
        let cells_on_triangle = vec![[1, 1, 1], [1, 1, 1], [3, 4, 5]];
        let ref_sjx_segment = vec![0, 0, 1];
        let triangle_points = vec![LonLatDegrees::new(0.0, 0.0); 2 + 1 + 4];
        let mut cell_points = vec![LonLatDegrees::new(0.0, 0.0); 5 + 1 + 3];
        // Symmetric about the prime meridian, so the assertions can be exact:
        // a geodesic midpoint is not the lon/lat average except where symmetry
        // forces it to be, and pinning an inexact value would be testing the
        // arithmetic rather than the subdivision.
        cell_points[3] = LonLatDegrees::new(-1.0, 0.0);
        cell_points[4] = LonLatDegrees::new(1.0, 0.0);
        cell_points[5] = LonLatDegrees::new(0.0, 2.0);
        let cells_on_triangle_new = vec![[1usize, 1, 1]; 2 + 1 + 4];
        (
            cells_on_triangle,
            ref_sjx_segment,
            triangle_points,
            cell_points,
            cells_on_triangle_new,
        )
    }

    #[test]
    fn a_marked_triangle_becomes_four_children_on_its_edge_midpoints() {
        let (cells_on_triangle, segment, mut triangle_points, mut cell_points, mut new_table) =
            one_triangle();
        let num_mp = [0, 2, 6];
        let num_wp = [0, 5, 8];

        refine_onedivide_four_renew_one_based(
            2,
            1,
            &num_mp,
            &num_wp,
            &cells_on_triangle,
            &segment,
            &mut triangle_points,
            &mut cell_points,
            &mut new_table,
        )
        .expect("subdivide");

        // Edge midpoints, opposite the corner of the same index. A geodesic
        // midpoint is not the lon/lat average except where symmetry forces it
        // to be, so the assertions use the symmetry rather than a number: the
        // base midpoint lands on the meridian, and the two flank midpoints are
        // each other's mirror image.
        assert!(
            cell_points[8].lon_degrees.abs() < 1e-12 && cell_points[8].lat_degrees.abs() < 1e-12,
            "w3 = mid(3,4) is the base midpoint, got {:?}",
            cell_points[8]
        );
        assert!(
            (cell_points[6].lon_degrees + cell_points[7].lon_degrees).abs() < 1e-12,
            "w1 = mid(4,5) and w2 = mid(3,5) must mirror: {:?} vs {:?}",
            cell_points[6],
            cell_points[7]
        );
        assert!(
            cell_points[6].lon_degrees > 0.0 && cell_points[6].lat_degrees > 0.0,
            "w1 sits on the edge from (1,0) to (0,2), got {:?}",
            cell_points[6]
        );

        assert_eq!(new_table[3], [3, 8, 7], "corner child keeps corner 3");
        assert_eq!(new_table[4], [4, 6, 8]);
        assert_eq!(new_table[5], [5, 7, 6]);
        assert_eq!(
            new_table[6],
            [6, 7, 8],
            "the middle child is the new points"
        );
        assert_eq!(
            new_table[2],
            [1, 1, 1],
            "the parent row is blanked, not removed -- later stages still address it"
        );
    }

    #[test]
    fn storage_too_short_for_the_children_is_an_error_rather_than_a_partial_mesh() {
        // Half a subdivision is a mesh that passes no check and explains
        // nothing; the size comes from Array_length_calculation and a mismatch
        // there has to surface here.
        let (cells_on_triangle, segment, mut triangle_points, mut cell_points, mut new_table) =
            one_triangle();
        triangle_points.truncate(4);
        let num_mp = [0, 2, 6];
        let num_wp = [0, 5, 8];

        let error = refine_onedivide_four_renew_one_based(
            2,
            1,
            &num_mp,
            &num_wp,
            &cells_on_triangle,
            &segment,
            &mut triangle_points,
            &mut cell_points,
            &mut new_table,
        )
        .expect_err("short storage must not produce half a subdivision");
        assert!(error.to_string().contains("storage is short"), "{error}");
    }

    #[test]
    fn an_unmarked_mesh_is_left_exactly_as_it_was() {
        let (cells_on_triangle, _, mut triangle_points, mut cell_points, mut new_table) =
            one_triangle();
        let before = (
            triangle_points.clone(),
            cell_points.clone(),
            new_table.clone(),
        );
        let num_mp = [0, 2, 2];
        let num_wp = [0, 5, 5];

        refine_onedivide_four_renew_one_based(
            2,
            1,
            &num_mp,
            &num_wp,
            &cells_on_triangle,
            &vec![0; 3],
            &mut triangle_points,
            &mut cell_points,
            &mut new_table,
        )
        .expect("nothing marked");

        assert_eq!((triangle_points, cell_points, new_table), before);
    }
}
