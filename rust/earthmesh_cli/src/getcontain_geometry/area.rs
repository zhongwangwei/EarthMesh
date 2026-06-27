use std::io;

use earthmesh_geometry::{is_point_in_convex_polygon, Point as AreaJudgePoint};

use crate::{GetContainAreaBounds, LonLatPoint};

use super::helpers::getcontain_cell_polygon;

pub fn getcontain_is_in_area_ustr_fortran_indexed(
    bounds: GetContainAreaBounds,
    vertices: &[LonLatPoint],
    cell_to_vertices: &[Vec<i32>],
    n_edges: &[i32],
    num_vertex: usize,
) -> io::Result<Vec<i32>> {
    if n_edges.len() != cell_to_vertices.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "n_edges length {} must match cell_to_vertices rows {}",
                n_edges.len(),
                cell_to_vertices.len()
            ),
        ));
    }
    if bounds.west >= bounds.east || bounds.south >= bounds.north {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "area bounds must satisfy west < east and south < north",
        ));
    }

    let mut is_in_area_ustr = vec![0; cell_to_vertices.len()];
    let start_row = num_vertex.saturating_add(1);

    for cell_index in start_row..cell_to_vertices.len() {
        let polygon = getcontain_cell_polygon(cell_index, vertices, cell_to_vertices, n_edges)?;
        if polygon.iter().any(|point| {
            point.x > bounds.west
                && point.x < bounds.east
                && point.y > bounds.south
                && point.y < bounds.north
        }) {
            is_in_area_ustr[cell_index] = 1;
        }
    }

    let domain_corners = [
        AreaJudgePoint::new(bounds.east, bounds.north),
        AreaJudgePoint::new(bounds.west, bounds.north),
        AreaJudgePoint::new(bounds.west, bounds.south),
        AreaJudgePoint::new(bounds.east, bounds.south),
    ];
    for cell_index in start_row..cell_to_vertices.len() {
        if is_in_area_ustr[cell_index] != 0 {
            continue;
        }
        let polygon = getcontain_cell_polygon(cell_index, vertices, cell_to_vertices, n_edges)?;
        if polygon.is_empty() {
            continue;
        }
        let (min_lon, max_lon) = polygon.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(min_lon, max_lon), point| (min_lon.min(point.x), max_lon.max(point.x)),
        );
        for corner in domain_corners {
            if is_point_in_convex_polygon(&polygon, corner) {
                if max_lon - min_lon > 180.0 {
                    break;
                }
                is_in_area_ustr[cell_index] = 1;
                break;
            }
        }
    }

    Ok(is_in_area_ustr)
}
