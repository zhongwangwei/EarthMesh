use crate::netcdf_to_io_error;
use crate::one_to_n_i32;
use crate::validate_mpas_mesh;
use crate::write_f64_1d;
use crate::write_f64_matrix_rows;
use crate::write_i32_1d;
use crate::write_i32_matrix_rows;
use crate::write_i32_pair_rows;
use crate::MpasMesh;
use crate::MpasMeshWriteReport;
use std::io;
use std::path::Path;

pub const MPAS_OCEAN_SPHERE_RADIUS_METERS: f64 = 6_371_220.0;

/// Write the full MPAS mesh schema produced by
/// `MOD_file_preprocess.F90:MPAS_Mesh_Save`.
///
/// The Rust data shape preserves the compatibility placeholder row at index `0`; all
/// MPAS-facing variables are written from index `1..`, matching Canonical slices
/// such as `2:num_dbx`, `2:num_sjx`, and `2:num_edge`. Connectivity ids have
/// the internal placeholder row removed before writing, so valid file ids stay
/// 1-based while `0` remains the no-neighbour/missing marker.
pub fn write_mpas_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasMesh,
) -> io::Result<MpasMeshWriteReport> {
    write_mpas_mesh_netcdf_with_radius(output, mesh, 1.0, false)
}

/// Write an MPAS-Ocean mesh using the physical sphere and metric units used by
/// the official MPAS-Ocean mesh database.
pub fn write_mpas_ocean_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasMesh,
) -> io::Result<MpasMeshWriteReport> {
    write_mpas_mesh_netcdf_with_radius(output, mesh, MPAS_OCEAN_SPHERE_RADIUS_METERS, true)
}

fn write_mpas_mesh_netcdf_with_radius(
    output: impl AsRef<Path>,
    mesh: &MpasMesh,
    sphere_radius: f64,
    write_ocean_diagnostics: bool,
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
    write_scaled_f64_1d(
        &mut file,
        "xCell",
        "nCells",
        &mesh.x_cell[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "yCell",
        "nCells",
        &mesh.y_cell[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "zCell",
        "nCells",
        &mesh.z_cell[1..],
        sphere_radius,
    )?;
    write_i32_1d(
        &mut file,
        "indexToVertexID",
        "nVertices",
        &one_to_n_i32(n_vertices, "indexToVertexID")?,
    )?;
    write_f64_1d(&mut file, "latVertex", "nVertices", &mesh.lat_vertex[1..])?;
    write_f64_1d(&mut file, "lonVertex", "nVertices", &mesh.lon_vertex[1..])?;
    write_scaled_f64_1d(
        &mut file,
        "xVertex",
        "nVertices",
        &mesh.x_vertex[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "yVertex",
        "nVertices",
        &mesh.y_vertex[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "zVertex",
        "nVertices",
        &mesh.z_vertex[1..],
        sphere_radius,
    )?;
    write_i32_1d(
        &mut file,
        "indexToEdgeID",
        "nEdges",
        &one_to_n_i32(n_edges, "indexToEdgeID")?,
    )?;
    write_f64_1d(&mut file, "latEdge", "nEdges", &mesh.lat_edge[1..])?;
    write_f64_1d(&mut file, "lonEdge", "nEdges", &mesh.lon_edge[1..])?;
    write_scaled_f64_1d(
        &mut file,
        "xEdge",
        "nEdges",
        &mesh.x_edge[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "yEdge",
        "nEdges",
        &mesh.y_edge[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "zEdge",
        "nEdges",
        &mesh.z_edge[1..],
        sphere_radius,
    )?;
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
    // Match the checked MPAS-Ocean SOMA reference meshes: two surviving cells
    // keep boundaryVertex=0; only a single surviving cell marks the vertex 1.
    let boundary_vertex = mesh.cells_on_vertex[1..]
        .iter()
        .map(|row| i32::from(row.iter().filter(|cell| **cell > 0).count() == 1))
        .collect::<Vec<_>>();
    write_i32_1d(&mut file, "boundaryVertex", "nVertices", &boundary_vertex)?;
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
    let area_scale = sphere_radius * sphere_radius;
    write_scaled_f64_1d(
        &mut file,
        "areaCell",
        "nCells",
        &mesh.area_cell[1..],
        area_scale,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "areaTriangle",
        "nVertices",
        &mesh.area_triangle[1..],
        area_scale,
    )?;
    write_scaled_f64_matrix_rows(
        &mut file,
        "kiteAreasOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.kite_areas_on_vertex[1..],
        area_scale,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "dvEdge",
        "nEdges",
        &mesh.dv_edge[1..],
        sphere_radius,
    )?;
    write_scaled_f64_1d(
        &mut file,
        "dcEdge",
        "nEdges",
        &mesh.dc_edge[1..],
        sphere_radius,
    )?;
    write_f64_1d(&mut file, "angleEdge", "nEdges", &mesh.angle_edge[1..])?;
    write_f64_matrix_rows(
        &mut file,
        "weightsOnEdge",
        &["nEdges", "maxEdges2"],
        &mesh.weights_on_edge[1..],
    )?;
    write_f64_1d(&mut file, "meshDensity", "nCells", &mesh.mesh_density[1..])?;
    if write_ocean_diagnostics {
        write_mpas_ocean_quality_fields(&mut file, mesh)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("nominalMinDc", &[])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&[mesh.nominal_min_dc * sphere_radius], ..)
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
    file.add_attribute("sphere_radius", sphere_radius)
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
    file.add_attribute("Conventions", "MPAS")
        .map_err(netcdf_to_io_error)?;

    Ok(MpasMeshWriteReport {
        output: output.to_path_buf(),
        n_cells,
        n_vertices,
        n_edges,
    })
}

fn write_mpas_ocean_quality_fields(file: &mut netcdf::FileMut, mesh: &MpasMesh) -> io::Result<()> {
    let mut cell_quality = Vec::with_capacity(mesh.edges_on_cell.len() - 1);
    let mut grid_spacing = Vec::with_capacity(mesh.edges_on_cell.len() - 1);
    for cell in 1..mesh.edges_on_cell.len() {
        let count = usize::try_from(mesh.n_edges_on_cell[cell]).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("n_edges_on_cell[{cell}] must not be negative"),
            )
        })?;
        if count > mesh.edges_on_cell[cell].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("n_edges_on_cell[{cell}] exceeds edges_on_cell row width"),
            ));
        }
        let mut min_dv = f64::MAX;
        let mut max_dv = 0.0_f64;
        let mut dc_sum = 0.0;
        for &edge_id in &mesh.edges_on_cell[cell][..count] {
            let edge = mpas_edge_index(edge_id, mesh.dv_edge.len(), "edges_on_cell")?;
            min_dv = min_dv.min(mesh.dv_edge[edge]);
            max_dv = max_dv.max(mesh.dv_edge[edge]);
            dc_sum += mesh.dc_edge[edge];
        }
        cell_quality.push(if max_dv > 0.0 { min_dv / max_dv } else { 0.0 });
        grid_spacing.push(if count > 0 {
            dc_sum / count as f64
        } else {
            0.0
        });
    }

    let mut triangle_quality = Vec::with_capacity(mesh.edges_on_vertex.len() - 1);
    let mut triangle_angle_quality = Vec::with_capacity(mesh.edges_on_vertex.len() - 1);
    let mut obtuse_triangle = Vec::with_capacity(mesh.edges_on_vertex.len() - 1);
    for row in &mesh.edges_on_vertex[1..] {
        let mut lengths = [0.0; 3];
        let mut count = 0;
        for &edge_id in row.iter().filter(|edge_id| **edge_id > 0) {
            let edge = mpas_edge_index(edge_id, mesh.dc_edge.len(), "edges_on_vertex")?;
            lengths[count] = mesh.dc_edge[edge];
            count += 1;
        }
        let active = &lengths[..count];
        let min_dc = active.iter().copied().fold(f64::MAX, f64::min);
        let max_dc = active.iter().copied().fold(0.0_f64, f64::max);
        triangle_quality.push(if max_dc > 0.0 { min_dc / max_dc } else { 0.0 });

        if count == 3 {
            let [a, b, c] = lengths;
            if a <= 0.0 || b <= 0.0 || c <= 0.0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "MPAS-Ocean triangle edge lengths must be positive",
                ));
            }
            // Match MPAS-Tools mesh_converter exactly, including its historical
            // b*c numerator term for angle2, so diagnostics compare bit-for-bit.
            let angles = [
                ((b * b + c * c - a * a) / (2.0 * b * c))
                    .clamp(-1.0, 1.0)
                    .acos(),
                ((a * a + c * c - b * c) / (2.0 * a * c))
                    .clamp(-1.0, 1.0)
                    .acos(),
                ((a * a + b * b - c * c) / (2.0 * a * b))
                    .clamp(-1.0, 1.0)
                    .acos(),
            ];
            let min_angle = angles.iter().copied().fold(f64::MAX, f64::min);
            let max_angle = angles.iter().copied().fold(0.0_f64, f64::max);
            triangle_angle_quality.push(min_angle / max_angle);
            obtuse_triangle.push(i32::from(max_angle > std::f64::consts::FRAC_PI_2));
        } else {
            triangle_angle_quality.push(1.0);
            obtuse_triangle.push(0);
        }
    }

    write_f64_1d(file, "cellQuality", "nCells", &cell_quality)?;
    write_f64_1d(file, "gridSpacing", "nCells", &grid_spacing)?;
    write_f64_1d(file, "triangleQuality", "nVertices", &triangle_quality)?;
    write_f64_1d(
        file,
        "triangleAngleQuality",
        "nVertices",
        &triangle_angle_quality,
    )?;
    write_i32_1d(file, "obtuseTriangle", "nVertices", &obtuse_triangle)
}

fn mpas_edge_index(edge_id: i32, len: usize, field: &str) -> io::Result<usize> {
    let edge = usize::try_from(edge_id).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} contains negative edge id {edge_id}"),
        )
    })?;
    if edge == 0 || edge >= len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} contains invalid edge id {edge_id}"),
        ));
    }
    Ok(edge)
}

fn write_scaled_f64_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[f64],
    scale: f64,
) -> io::Result<()> {
    if scale == 1.0 {
        return write_f64_1d(file, name, dim, values);
    }
    let scaled = values.iter().map(|value| value * scale).collect::<Vec<_>>();
    write_f64_1d(file, name, dim, &scaled)
}

fn write_scaled_f64_matrix_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<f64>],
    scale: f64,
) -> io::Result<()> {
    if scale == 1.0 {
        return write_f64_matrix_rows(file, name, dims, rows);
    }
    let scaled = rows
        .iter()
        .flat_map(|row| row.iter().map(|value| value * scale))
        .collect::<Vec<_>>();
    let mut var = file
        .add_variable::<f64>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&scaled, (.., ..))
        .map_err(netcdf_to_io_error)
}
