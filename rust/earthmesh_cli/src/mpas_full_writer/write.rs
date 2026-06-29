use std::io;
use std::path::Path;

use crate::*;

/// Write the full MPAS mesh schema produced by
/// `MOD_file_preprocess.F90:MPAS_Mesh_Save`.
///
/// The Rust data shape preserves the legacy placeholder row at index `0`; all
/// MPAS-facing variables are written from index `1..`, matching Fortran slices
/// such as `2:num_dbx`, `2:num_sjx`, and `2:num_edge`. Connectivity ids have
/// the internal placeholder row removed before writing, so valid file ids stay
/// 1-based while `0` remains the no-neighbour/missing marker.
pub fn write_mpas_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasMesh,
) -> io::Result<MpasMeshWriteReport> {
    validate_mpas_mesh(mesh)?;
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;

    let n_cells = mesh.x_cell.len() - 1;
    let n_vertices = mesh.x_vertex.len() - 1;
    let n_edges = mesh.x_edge.len() - 1;

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nCells", n_cells)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nVertices", n_vertices)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nEdges", n_edges)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("maxEdges", 10)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("maxEdges2", 20)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("TWO", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("vertexDegree", 3)
        .map_err(netcdf_to_io_error)?;

    write_i32_1d(
        &mut file,
        "indexToCellID",
        "nCells",
        &one_to_n_i32(n_cells, "indexToCellID")?,
    )?;
    write_f64_1d(&mut file, "latCell", "nCells", &mesh.lat_cell[1..])?;
    write_f64_1d(&mut file, "lonCell", "nCells", &mesh.lon_cell[1..])?;
    write_f64_1d(&mut file, "xCell", "nCells", &mesh.x_cell[1..])?;
    write_f64_1d(&mut file, "yCell", "nCells", &mesh.y_cell[1..])?;
    write_f64_1d(&mut file, "zCell", "nCells", &mesh.z_cell[1..])?;
    write_i32_1d(
        &mut file,
        "indexToVertexID",
        "nVertices",
        &one_to_n_i32(n_vertices, "indexToVertexID")?,
    )?;
    write_f64_1d(&mut file, "latVertex", "nVertices", &mesh.lat_vertex[1..])?;
    write_f64_1d(&mut file, "lonVertex", "nVertices", &mesh.lon_vertex[1..])?;
    write_f64_1d(&mut file, "xVertex", "nVertices", &mesh.x_vertex[1..])?;
    write_f64_1d(&mut file, "yVertex", "nVertices", &mesh.y_vertex[1..])?;
    write_f64_1d(&mut file, "zVertex", "nVertices", &mesh.z_vertex[1..])?;
    write_i32_1d(
        &mut file,
        "indexToEdgeID",
        "nEdges",
        &one_to_n_i32(n_edges, "indexToEdgeID")?,
    )?;
    write_f64_1d(&mut file, "latEdge", "nEdges", &mesh.lat_edge[1..])?;
    write_f64_1d(&mut file, "lonEdge", "nEdges", &mesh.lon_edge[1..])?;
    write_f64_1d(&mut file, "xEdge", "nEdges", &mesh.x_edge[1..])?;
    write_f64_1d(&mut file, "yEdge", "nEdges", &mesh.y_edge[1..])?;
    write_f64_1d(&mut file, "zEdge", "nEdges", &mesh.z_edge[1..])?;
    write_i32_1d(
        &mut file,
        "nEdgesOnCell",
        "nCells",
        &mesh.n_edges_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "cellsOnCell",
        &["nCells", "maxEdges"],
        &mesh.cells_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "verticesOnCell",
        &["nCells", "maxEdges"],
        &mesh.vertices_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "edgesOnCell",
        &["nCells", "maxEdges"],
        &mesh.edges_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "cellsOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.cells_on_vertex[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "edgesOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.edges_on_vertex[1..],
    )?;
    write_i32_pair_rows(
        &mut file,
        "cellsOnEdge",
        &["nEdges", "TWO"],
        &mesh.cells_on_edge[1..],
    )?;
    write_i32_pair_rows(
        &mut file,
        "verticesOnEdge",
        &["nEdges", "TWO"],
        &mesh.vertices_on_edge[1..],
    )?;
    write_i32_1d(
        &mut file,
        "nEdgesOnEdge",
        "nEdges",
        &mesh.n_edges_on_edge[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "edgesOnEdge",
        &["nEdges", "maxEdges2"],
        &mesh.edges_on_edge[1..],
    )?;
    write_f64_1d(&mut file, "areaCell", "nCells", &mesh.area_cell[1..])?;
    write_f64_1d(
        &mut file,
        "areaTriangle",
        "nVertices",
        &mesh.area_triangle[1..],
    )?;
    write_f64_matrix_rows(
        &mut file,
        "kiteAreasOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.kite_areas_on_vertex[1..],
    )?;
    write_f64_1d(&mut file, "dvEdge", "nEdges", &mesh.dv_edge[1..])?;
    write_f64_1d(&mut file, "dcEdge", "nEdges", &mesh.dc_edge[1..])?;
    write_f64_1d(&mut file, "angleEdge", "nEdges", &mesh.angle_edge[1..])?;
    write_f64_matrix_rows(
        &mut file,
        "weightsOnEdge",
        &["nEdges", "maxEdges2"],
        &mesh.weights_on_edge[1..],
    )?;
    write_f64_1d(&mut file, "meshDensity", "nCells", &mesh.mesh_density[1..])?;
    {
        let mut var = file
            .add_variable::<f64>("nominalMinDc", &[])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&[mesh.nominal_min_dc], ..)
            .map_err(netcdf_to_io_error)?;
    }
    write_f64_1d(
        &mut file,
        "error_segment",
        "nEdges",
        &mesh.error_segment[1..],
    )?;

    file.add_attribute("mesh_spec", "1.0")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("on_a_sphere", "YES")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("sphere_radius", 1.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("is_periodic", "NO")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("x_period", 0.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("y_period", 0.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("file_id", "bbdd9043")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("source", "Generated by EarthMesh")
        .map_err(netcdf_to_io_error)?;

    Ok(MpasMeshWriteReport {
        output: output.to_path_buf(),
        n_cells,
        n_vertices,
        n_edges,
    })
}
