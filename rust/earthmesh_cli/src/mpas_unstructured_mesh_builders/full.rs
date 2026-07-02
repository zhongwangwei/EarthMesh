use std::io;

use super::legacy::{normalize_mpas_legacy_placeholder_inputs, trim_mpas_leading_placeholders};
use crate::*;

/// Build the in-memory payload produced by `MOD_mask_postproc.F90:MPAS_Mesh_Cal`
/// before `MPAS_Mesh_Save` and `MPAS_info_Save` write side effects.
///
/// The input mesh preserves EarthMesh/Fortran indexing. The returned payload
/// keeps a placeholder row at index 0. Connectivity ids have that internal
/// placeholder removed before `write_mpas_mesh_netcdf` writes rows, so `0` stays
/// a missing/boundary marker and valid file ids start at `1`.
pub fn build_mpas_mesh_from_unstructured_fortran_indexed(
    mesh: &UnstructuredMesh,
    cellwidth: &[f64],
    nxp: usize,
    step: usize,
) -> io::Result<MpasMesh> {
    let original_cell_rows = mesh.w_points.len();
    let original_vertex_rows = mesh.m_points.len();
    let (mesh, cellwidth) = normalize_mpas_legacy_placeholder_inputs(mesh, cellwidth)?;
    let mesh = &mesh;
    let extra_cell_rows = mesh.w_points.len().saturating_sub(original_cell_rows);
    let extra_vertex_rows = mesh.m_points.len().saturating_sub(original_vertex_rows);
    let cellwidth = cellwidth.as_slice();
    validate_unstructured_mesh(mesh)?;
    if cellwidth.len() != mesh.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match w_points length {}",
                cellwidth.len(),
                mesh.w_points.len()
            ),
        ));
    }
    if nxp == 0 || step == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "nxp and step must be positive for MPAS nominalMinDc",
        ));
    }
    if cellwidth
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth values must be finite and positive",
        ));
    }

    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let vertices_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    let edge_output = get_edge_from_unstructured_mesh(mesh)?;

    let vertices = lonlat_points_to_unit_xyz(&triangle_lonlat);
    let cells = lonlat_points_to_unit_xyz(&cell_lonlat);
    let edge_points = lonlat_points_to_unit_xyz(&edge_output.edge_points);

    let ordered_vertices_on_cell = order_vertices_on_cell_fortran_indexed(
        &cells,
        &vertices,
        &vertices_on_cell,
        &n_edges_on_cell,
    )
    .and_then(|ordered| {
        standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, &n_edges_on_cell)
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to order MPAS verticesOnCell from unstructured mesh",
        )
    })?;
    let mut ordered_vertices_on_cell = ordered_vertices_on_cell;
    let mut cell_connectivity = connect_on_cell_fortran_indexed(
        &n_edges_on_cell,
        &edge_output.cells_on_edge,
        &edge_output.edges_on_vertex,
        &ordered_vertices_on_cell,
    );
    if cell_connectivity.is_none() {
        if let Some(topological_order) = order_vertices_on_cell_by_shared_edges_fortran_indexed(
            &vertices_on_cell,
            &n_edges_on_cell,
            &edge_output.edges_on_vertex,
            &vertices,
            &cells,
        )
        .and_then(|ordered| {
            standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, &n_edges_on_cell)
        }) {
            if let Some(topological_connectivity) = connect_on_cell_fortran_indexed(
                &n_edges_on_cell,
                &edge_output.cells_on_edge,
                &edge_output.edges_on_vertex,
                &topological_order,
            ) {
                ordered_vertices_on_cell = topological_order;
                cell_connectivity = Some(topological_connectivity);
            }
        }
    }
    let cell_connectivity = cell_connectivity.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to build MPAS cell connectivity from unstructured mesh",
        )
    })?;

    let area = get_area_production_fortran_indexed(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cells,
        cells_on_vertex: &cells_on_triangle,
        edges_on_vertex: &edge_output.edges_on_vertex,
        cells_on_edge: &edge_output.cells_on_edge,
        vertices_on_cell: &ordered_vertices_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to compute MPAS area payload from unstructured mesh",
        )
    })?;

    let lon_edge_degrees = edge_output
        .edge_points
        .iter()
        .map(|point| point.lon_degrees)
        .collect::<Vec<_>>();
    let lat_edge_degrees = edge_output
        .edge_points
        .iter()
        .map(|point| point.lat_degrees)
        .collect::<Vec<_>>();
    let lat_vertex_degrees = triangle_lonlat
        .iter()
        .map(|point| point.lat_degrees)
        .collect::<Vec<_>>();
    let edge_metrics = edge_distance_angle_fortran_indexed(
        &vertices,
        &cells,
        &edge_points,
        &edge_output.vertices_on_edge,
        &edge_output.cells_on_edge,
        &lat_vertex_degrees,
        &lon_edge_degrees,
        &lat_edge_degrees,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to compute MPAS edge distance/angle payload",
        )
    })?;

    let weights = set_weights_on_edge_fortran_indexed(
        &area.unit.area_cell,
        &edge_metrics.angle_edge,
        &edge_metrics.dc_edge,
        &edge_metrics.dv_edge,
        &area.unit.kite_areas_on_vertex,
        &cell_connectivity.edges_on_cell,
        &cells_on_triangle,
        &edge_output.cells_on_edge,
        &ordered_vertices_on_cell,
        &edge_output.vertices_on_edge,
        &n_edges_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to compute MPAS weightsOnEdge payload",
        )
    })?;

    let (x_vertex, y_vertex, z_vertex) = split_cartesian_components(&vertices);
    let (x_cell, y_cell, z_cell) = split_cartesian_components(&cells);
    let (x_edge, y_edge, z_edge) = split_cartesian_components(&edge_points);
    let (lat_cell, lon_cell) = mpas_lat_lon_radians(&cell_lonlat);
    let (lat_vertex, lon_vertex) = mpas_lat_lon_radians(&triangle_lonlat);
    let (lat_edge, lon_edge) = mpas_lat_lon_radians(&edge_output.edge_points);

    let min_cellwidth = cellwidth.iter().copied().fold(f64::INFINITY, f64::min);
    let mesh_density = cellwidth
        .iter()
        .map(|width| (min_cellwidth / width).powi(4))
        .collect::<Vec<_>>();
    let nominal_min_dc = (7680 / nxp / 2_usize.pow((step - 1) as u32)) as f64
        / earthmesh_core::EARTH_RADIUS_METERS
        * 1000.0;

    let mut mpas = MpasMesh {
        lat_cell,
        lon_cell,
        x_cell,
        y_cell,
        z_cell,
        lat_vertex,
        lon_vertex,
        x_vertex,
        y_vertex,
        z_vertex,
        lat_edge,
        lon_edge,
        x_edge,
        y_edge,
        z_edge,
        n_edges_on_cell: usize_values_to_i32("n_edges_on_cell", &n_edges_on_cell)?,
        cells_on_cell: zero_based_padded_rows(
            "cells_on_cell",
            &cell_connectivity.cells_on_cell,
            10,
        )?,
        vertices_on_cell: zero_based_padded_rows(
            "vertices_on_cell",
            &ordered_vertices_on_cell,
            10,
        )?,
        edges_on_cell: zero_based_padded_rows(
            "edges_on_cell",
            &cell_connectivity.edges_on_cell,
            10,
        )?,
        cells_on_vertex: zero_based_triplet_rows("cells_on_vertex", &edge_output.cells_on_vertex)?,
        edges_on_vertex: zero_based_triplet_rows("edges_on_vertex", &edge_output.edges_on_vertex)?,
        cells_on_edge: zero_based_pair_rows("cells_on_edge", &edge_output.cells_on_edge)?,
        vertices_on_edge: zero_based_pair_rows("vertices_on_edge", &edge_output.vertices_on_edge)?,
        n_edges_on_edge: usize_values_to_i32("n_edges_on_edge", &weights.n_edges_on_edge)?,
        edges_on_edge: zero_based_padded_rows("edges_on_edge", &weights.edges_on_edge, 20)?,
        area_cell: area.unit.area_cell,
        area_triangle: area.unit.area_triangle,
        kite_areas_on_vertex: area
            .unit
            .kite_areas_on_vertex
            .into_iter()
            .map(|row| row.to_vec())
            .collect(),
        dv_edge: edge_metrics.dv_edge,
        dc_edge: edge_metrics.dc_edge,
        angle_edge: edge_metrics.angle_edge,
        weights_on_edge: pad_f64_rows(&weights.weights_on_edge, 20),
        mesh_density,
        nominal_min_dc,
        error_segment: weights.error_segment,
    };
    trim_mpas_leading_placeholders(&mut mpas, extra_cell_rows, extra_vertex_rows);
    validate_mpas_mesh(&mpas)?;
    Ok(mpas)
}
