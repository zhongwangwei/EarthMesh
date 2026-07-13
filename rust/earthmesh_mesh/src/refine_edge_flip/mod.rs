use std::io;

use crate::{average_lonlat3, check_crossing_canonical_lonlat, LonLatDegrees};

#[derive(Clone, Debug, PartialEq)]
pub struct CheckedEdgeFlip {
    pub triangles: [[usize; 3]; 2],
    pub centroids: [LonLatDegrees; 2],
}

pub fn checked_lop_edge_flip(
    left_id: usize,
    right_id: usize,
    left: [usize; 3],
    right: [usize; 3],
    cell_points: &[LonLatDegrees],
) -> io::Result<CheckedEdgeFlip> {
    let left_opposite = left
        .iter()
        .copied()
        .find(|vertex| !right.contains(vertex))
        .ok_or_else(|| {
            shared_edge_error(left_id, right_id, "left triangle has no opposite vertex")
        })?;
    let right_opposite = right
        .iter()
        .copied()
        .find(|vertex| !left.contains(vertex))
        .ok_or_else(|| {
            shared_edge_error(left_id, right_id, "right triangle has no opposite vertex")
        })?;
    let shared = left
        .iter()
        .copied()
        .filter(|vertex| right.contains(vertex))
        .collect::<Vec<_>>();
    if shared.len() != 2 {
        return Err(shared_edge_error(
            left_id,
            right_id,
            "triangles must share exactly one edge",
        ));
    }
    let triangles = [
        [left_opposite, shared[0], right_opposite],
        [left_opposite, shared[1], right_opposite],
    ];

    for &cell in triangles.iter().flatten() {
        if cell == 0 || cell >= cell_points.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP pair {left_id}/{right_id} references invalid cell {cell}"),
            ));
        }
    }

    let mut quad_points = [
        cell_points[left_opposite],
        cell_points[shared[0]],
        cell_points[right_opposite],
        cell_points[shared[1]],
    ];
    let crosses_dateline = quad_points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::NEG_INFINITY, f64::max)
        - quad_points
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::INFINITY, f64::min)
        > 180.0;
    if crosses_dateline {
        check_crossing_canonical_lonlat(&mut quad_points);
    }
    let mut centroids = [
        average_lonlat3(quad_points[0], quad_points[1], quad_points[2]).ok_or_else(|| {
            shared_edge_error(
                left_id,
                right_id,
                "first flipped triangle centroid is degenerate",
            )
        })?,
        average_lonlat3(quad_points[0], quad_points[3], quad_points[2]).ok_or_else(|| {
            shared_edge_error(
                left_id,
                right_id,
                "second flipped triangle centroid is degenerate",
            )
        })?,
    ];
    if crosses_dateline {
        check_crossing_canonical_lonlat(&mut centroids);
    }

    Ok(CheckedEdgeFlip {
        triangles,
        centroids,
    })
}

fn shared_edge_error(left_id: usize, right_id: usize, msg: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("LOP pair {left_id}/{right_id}: {msg}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_lop_flip_preserves_canonical_triangle_order() {
        let points = vec![
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(1.0, 0.0),
            LonLatDegrees::new(1.0, 1.0),
            LonLatDegrees::new(0.0, 1.0),
        ];
        let flip = checked_lop_edge_flip(10, 11, [1, 2, 4], [2, 3, 4], &points).unwrap();
        assert_eq!(flip.triangles, [[1, 2, 3], [1, 4, 3]]);
        assert!(flip.centroids[0].lon_degrees.is_finite());
    }
}
