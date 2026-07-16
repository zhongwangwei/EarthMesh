use std::{io, ops::Range};

use earthmesh_geometry::Point as AreaJudgePoint;

use crate::LonLatPoint;

pub(super) fn getcontain_axis_candidate_range(
    axis: &[f64],
    min_value: f64,
    max_value: f64,
) -> Option<Range<usize>> {
    if axis.len() < 2 || !min_value.is_finite() || !max_value.is_finite() {
        return None;
    }
    let lower = min_value.min(max_value) - 1.0e-12;
    let upper = min_value.max(max_value) + 1.0e-12;
    let values = &axis[1..];
    let first = values[0];
    let last = *values.last()?;
    let (start_offset, end_offset) = if first <= last {
        (
            values.partition_point(|value| *value < lower),
            values.partition_point(|value| *value <= upper),
        )
    } else {
        (
            values.partition_point(|value| *value > upper),
            values.partition_point(|value| *value >= lower),
        )
    };
    let start = start_offset + 1;
    let end = end_offset + 1;
    (start < end).then_some(start..end)
}

pub(super) fn getcontain_south_pole_scan_polygons(
    polygon: &[AreaJudgePoint],
    global_min_lat: f64,
) -> Vec<Vec<AreaJudgePoint>> {
    let cell_min_lat = polygon
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    if (cell_min_lat - global_min_lat).abs() > 1.0e-12 {
        return vec![polygon.to_vec()];
    }

    if polygon.len() == 3 {
        return vec![vec![
            AreaJudgePoint::new(polygon[2].x, polygon[0].y),
            AreaJudgePoint::new(polygon[1].x, polygon[0].y),
            polygon[1],
            polygon[2],
        ]];
    }

    if polygon.len() != 5 {
        return vec![polygon.to_vec()];
    }

    let mut sorted = polygon.to_vec();
    sorted.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut wedges = Vec::with_capacity(sorted.len());
    for i in 0..sorted.len() {
        let j = (i + 1) % sorted.len();
        wedges.push(vec![
            sorted[j],
            sorted[i],
            AreaJudgePoint::new(sorted[i].x, -90.0),
            AreaJudgePoint::new(sorted[j].x, -90.0),
        ]);
    }
    wedges
}

pub(super) fn getcontain_restore_dateline_source_index(
    index: usize,
    nlons_source: usize,
) -> io::Result<usize> {
    if nlons_source == 0 || !nlons_source.is_multiple_of(2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "dateline containment requires a positive even nlons_source, got {nlons_source}"
            ),
        ));
    }
    if index == 0 || index > nlons_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("dateline source index {index} is outside 1..={nlons_source}"),
        ));
    }
    let half = nlons_source / 2;
    if index < half + 1 {
        Ok(index + half)
    } else {
        Ok(index - half)
    }
}

pub(super) fn getcontain_cell_polygon(
    cell_index: usize,
    vertices: &[LonLatPoint],
    cell_to_vertices: &[Vec<i32>],
    n_edges: &[i32],
) -> io::Result<Vec<AreaJudgePoint>> {
    let edge_count = usize::try_from(n_edges[cell_index]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "cell {cell_index} has negative edge count {}",
                n_edges[cell_index]
            ),
        )
    })?;
    let row = &cell_to_vertices[cell_index];
    if edge_count > row.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "cell {cell_index} edge count {edge_count} exceeds vertex row width {}",
                row.len()
            ),
        ));
    }

    let mut polygon = Vec::with_capacity(edge_count);
    for vertex_id in row.iter().take(edge_count) {
        if *vertex_id <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell {cell_index} canonicals missing vertex {vertex_id}"),
            ));
        }
        let vertex_index = usize::try_from(*vertex_id).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell {cell_index} canonicals missing vertex {vertex_id}"),
            )
        })?;
        let vertex = vertices.get(vertex_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cell {cell_index} canonicals missing vertex {vertex_id}"),
            )
        })?;
        polygon.push(AreaJudgePoint::new(vertex.lon, vertex.lat));
    }
    Ok(polygon)
}

pub(crate) fn getcontain_validate_source_matrix<T>(
    name: &str,
    matrix: &[Vec<T>],
    lon_len: usize,
    lat_len: usize,
) -> io::Result<()> {
    if matrix.len() != lon_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} longitude rows {} must match lon_i length {lon_len}",
                matrix.len()
            ),
        ));
    }
    for (index, row) in matrix.iter().enumerate() {
        if row.len() != lat_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{name} row {index} latitude length {} must match lat_i length {lat_len}",
                    row.len()
                ),
            ));
        }
    }
    Ok(())
}
