use std::io;

use earthmesh_geometry::{
    is_point_in_convex_polygon, shift_longitudes_for_dateline_crossing, Point as AreaJudgePoint,
};

use crate::{ContainMesh, FlatContainMesh, GetContainMeshKind, LonLatPoint};

use super::helpers::{
    getcontain_axis_candidate_range, getcontain_cell_polygon,
    getcontain_restore_dateline_source_index, getcontain_south_pole_scan_polygons,
    getcontain_validate_source_matrix,
};

pub fn getcontain_containment_matrix_one_based(
    mesh_kind: GetContainMeshKind,
    vertices: &[LonLatPoint],
    cell_to_vertices: &[Vec<i32>],
    n_edges: &[i32],
    is_in_area_ustr: &[i32],
    is_in_area_grid: &[Vec<i32>],
    seaorland: &[Vec<i32>],
    lon_i: &[f64],
    lat_i: &[f64],
    num_vertex: usize,
) -> io::Result<ContainMesh> {
    getcontain_containment_matrix_flat_one_based(
        mesh_kind,
        vertices,
        cell_to_vertices,
        n_edges,
        is_in_area_ustr,
        is_in_area_grid,
        seaorland,
        lon_i,
        lat_i,
        num_vertex,
    )?
    .to_contain_mesh()
}

pub fn getcontain_containment_matrix_flat_one_based(
    mesh_kind: GetContainMeshKind,
    vertices: &[LonLatPoint],
    cell_to_vertices: &[Vec<i32>],
    n_edges: &[i32],
    is_in_area_ustr: &[i32],
    is_in_area_grid: &[Vec<i32>],
    seaorland: &[Vec<i32>],
    lon_i: &[f64],
    lat_i: &[f64],
    num_vertex: usize,
) -> io::Result<FlatContainMesh> {
    if is_in_area_ustr.len() != cell_to_vertices.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInArea_ustr length {} must match cell_to_vertices rows {}",
                is_in_area_ustr.len(),
                cell_to_vertices.len()
            ),
        ));
    }
    if lon_i.len() < 2 || lat_i.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lon_i and lat_i must include a dummy slot plus at least one source point",
        ));
    }
    getcontain_validate_source_matrix("IsInArea_grid", is_in_area_grid, lon_i.len(), lat_i.len())?;
    getcontain_validate_source_matrix("seaorland", seaorland, lon_i.len(), lat_i.len())?;

    let global_min_lat = vertices
        .iter()
        .filter_map(|point| point.lat.is_finite().then_some(point.lat))
        .fold(f64::INFINITY, f64::min);
    if !global_min_lat.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertices must include at least one finite latitude",
        ));
    }

    let id_width = mesh_kind.ustr_id_width();
    let ii_width = mesh_kind.ustr_ii_width();
    let mut ustr_id_values = vec![0_i32; cell_to_vertices.len() * id_width];
    let mut ustr_ii_values = Vec::<i32>::new();
    let mut selected_mask = is_in_area_ustr.to_vec();
    let start_row = num_vertex.saturating_add(1);

    for cell_index in start_row..cell_to_vertices.len() {
        if is_in_area_ustr[cell_index] != 1 {
            continue;
        }
        let polygon = getcontain_cell_polygon(cell_index, vertices, cell_to_vertices, n_edges)?;
        let scan_polygons = getcontain_south_pole_scan_polygons(&polygon, global_min_lat);

        let mut total_inside = 0;
        let entry_start = ustr_ii_values.len();
        for mut scan_polygon in scan_polygons {
            let (min_lon, max_lon) = scan_polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min_lon, max_lon), point| (min_lon.min(point.x), max_lon.max(point.x)),
            );
            let crosses_dateline = max_lon - min_lon > 180.0;
            if crosses_dateline {
                scan_polygon = shift_longitudes_for_dateline_crossing(&scan_polygon);
            }

            let (min_lon, max_lon) = scan_polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min_lon, max_lon), point| (min_lon.min(point.x), max_lon.max(point.x)),
            );
            let (min_lat, max_lat) = scan_polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min_lat, max_lat), point| (min_lat.min(point.y), max_lat.max(point.y)),
            );
            let Some(lon_range) = getcontain_axis_candidate_range(lon_i, min_lon, max_lon) else {
                continue;
            };
            let Some(lat_range) = getcontain_axis_candidate_range(lat_i, min_lat, max_lat) else {
                continue;
            };

            for i in lon_range {
                let restored_i = if crosses_dateline {
                    getcontain_restore_dateline_source_index(i, lon_i.len() - 1)?
                } else {
                    i
                };
                for j in lat_range.clone() {
                    if is_in_area_grid[restored_i][j] == 0 {
                        continue;
                    }
                    let point = AreaJudgePoint::new(lon_i[i], lat_i[j]);
                    if !is_point_in_convex_polygon(&scan_polygon, point) {
                        continue;
                    }
                    total_inside += 1;
                    match mesh_kind {
                        GetContainMeshKind::Land => {
                            if seaorland[restored_i][j] == 1 {
                                ustr_ii_values.extend_from_slice(&[restored_i as i32, j as i32]);
                            }
                        }
                        GetContainMeshKind::Ocean => {
                            if seaorland[restored_i][j] == 0 {
                                ustr_ii_values.extend_from_slice(&[restored_i as i32, j as i32]);
                            }
                        }
                        GetContainMeshKind::Atmos | GetContainMeshKind::Loc => {
                            ustr_ii_values.extend_from_slice(&[
                                restored_i as i32,
                                j as i32,
                                if seaorland[restored_i][j] == 1 { 1 } else { 0 },
                            ]);
                        }
                    }
                }
            }
        }

        let contained =
            i32::try_from((ustr_ii_values.len() - entry_start) / ii_width).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "contained source-pixel count exceeds i32",
                )
            })?;
        let id_offset = cell_index * id_width;
        ustr_id_values[id_offset] = contained;
        if mesh_kind == GetContainMeshKind::Ocean {
            ustr_id_values[id_offset + 2] = total_inside;
        }
        if contained == 0 {
            selected_mask[cell_index] = 0;
        }
    }

    let mut next_start = 1;
    if num_vertex > 0 && num_vertex < cell_to_vertices.len() {
        let id_offset = num_vertex * id_width;
        ustr_id_values[id_offset + 1] = next_start;
        next_start += ustr_id_values[id_offset];
    }
    for row in start_row..cell_to_vertices.len() {
        let id_offset = row * id_width;
        ustr_id_values[id_offset + 1] = next_start;
        next_start += ustr_id_values[id_offset];
    }

    let numpatch = usize::try_from(next_start - 1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "contained source-pixel count exceeds usize",
        )
    })?;
    if numpatch != ustr_ii_values.len() / ii_width {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "internal containment entry count mismatch",
        ));
    }

    Ok(FlatContainMesh {
        ustr_id_values,
        ustr_id_width: id_width,
        ustr_ii_values,
        ustr_ii_width: ii_width,
        is_in_area_ustr: selected_mask,
    })
}
