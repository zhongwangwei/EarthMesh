use std::io;

use crate::{f64_matrix_width, matrix_width, MpasMesh, MpasSimpleMesh};

pub(crate) fn validate_mpas_mesh(mesh: &MpasMesh) -> io::Result<()> {
    if mesh.x_cell.is_empty() || mesh.x_vertex.is_empty() || mesh.x_edge.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS mesh arrays must include the compatibility placeholder row",
        ));
    }
    let n_cells = mesh.x_cell.len();
    let n_vertices = mesh.x_vertex.len();
    let n_edges = mesh.x_edge.len();
    for (name, actual, required) in [
        ("lat_cell", mesh.lat_cell.len(), n_cells),
        ("lon_cell", mesh.lon_cell.len(), n_cells),
        ("y_cell", mesh.y_cell.len(), n_cells),
        ("z_cell", mesh.z_cell.len(), n_cells),
        ("n_edges_on_cell", mesh.n_edges_on_cell.len(), n_cells),
        ("cells_on_cell", mesh.cells_on_cell.len(), n_cells),
        ("vertices_on_cell", mesh.vertices_on_cell.len(), n_cells),
        ("edges_on_cell", mesh.edges_on_cell.len(), n_cells),
        ("area_cell", mesh.area_cell.len(), n_cells),
        ("mesh_density", mesh.mesh_density.len(), n_cells),
        ("lat_vertex", mesh.lat_vertex.len(), n_vertices),
        ("lon_vertex", mesh.lon_vertex.len(), n_vertices),
        ("y_vertex", mesh.y_vertex.len(), n_vertices),
        ("z_vertex", mesh.z_vertex.len(), n_vertices),
        ("cells_on_vertex", mesh.cells_on_vertex.len(), n_vertices),
        ("edges_on_vertex", mesh.edges_on_vertex.len(), n_vertices),
        ("area_triangle", mesh.area_triangle.len(), n_vertices),
        (
            "kite_areas_on_vertex",
            mesh.kite_areas_on_vertex.len(),
            n_vertices,
        ),
        ("lat_edge", mesh.lat_edge.len(), n_edges),
        ("lon_edge", mesh.lon_edge.len(), n_edges),
        ("y_edge", mesh.y_edge.len(), n_edges),
        ("z_edge", mesh.z_edge.len(), n_edges),
        ("cells_on_edge", mesh.cells_on_edge.len(), n_edges),
        ("vertices_on_edge", mesh.vertices_on_edge.len(), n_edges),
        ("n_edges_on_edge", mesh.n_edges_on_edge.len(), n_edges),
        ("edges_on_edge", mesh.edges_on_edge.len(), n_edges),
        ("dv_edge", mesh.dv_edge.len(), n_edges),
        ("dc_edge", mesh.dc_edge.len(), n_edges),
        ("angle_edge", mesh.angle_edge.len(), n_edges),
        ("weights_on_edge", mesh.weights_on_edge.len(), n_edges),
        ("error_segment", mesh.error_segment.len(), n_edges),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    for (name, rows, width) in [
        ("cells_on_cell", &mesh.cells_on_cell, 10_usize),
        ("vertices_on_cell", &mesh.vertices_on_cell, 10_usize),
        ("edges_on_cell", &mesh.edges_on_cell, 10_usize),
        ("cells_on_vertex", &mesh.cells_on_vertex, 3_usize),
        ("edges_on_vertex", &mesh.edges_on_vertex, 3_usize),
        ("edges_on_edge", &mesh.edges_on_edge, 20_usize),
    ] {
        let actual = matrix_width(name, rows)?;
        if actual != width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} width {actual} must match required {width}"),
            ));
        }
    }
    let kite_width = f64_matrix_width("kite_areas_on_vertex", &mesh.kite_areas_on_vertex)?;
    if kite_width != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("kite_areas_on_vertex width {kite_width} must match required 3"),
        ));
    }
    let weights_width = f64_matrix_width("weights_on_edge", &mesh.weights_on_edge)?;
    if weights_width != 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("weights_on_edge width {weights_width} must match required 20"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_mpas_simple_mesh(mesh: &MpasSimpleMesh) -> io::Result<()> {
    if mesh.x_cell.is_empty() || mesh.x_vertex.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS simple mesh arrays must include the compatibility placeholder row",
        ));
    }
    for (name, actual, required) in [
        ("y_cell", mesh.y_cell.len(), mesh.x_cell.len()),
        ("z_cell", mesh.z_cell.len(), mesh.x_cell.len()),
        ("mesh_density", mesh.mesh_density.len(), mesh.x_cell.len()),
        ("y_vertex", mesh.y_vertex.len(), mesh.x_vertex.len()),
        ("z_vertex", mesh.z_vertex.len(), mesh.x_vertex.len()),
        (
            "cells_on_vertex",
            mesh.cells_on_vertex.len(),
            mesh.x_vertex.len(),
        ),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    let width = matrix_width("cells_on_vertex", &mesh.cells_on_vertex)?;
    if width != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cells_on_vertex width {width} must match vertexDegree 3"),
        ));
    }
    Ok(())
}
