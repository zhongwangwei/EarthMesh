use std::{io, path::Path};

use crate::{
    gridfile_output_path, netcdf_to_io_error, normalize_degrees, rad_to_deg, require_len,
    required_dimension_len, required_values_f64, required_values_i32, required_values_i32_2d,
    write_unstructured_mesh_netcdf, LonLatPoint, UnstructuredMesh, UnstructuredMeshWriteReport,
};

pub fn convert_mpas_mode_file_to_earthmesh(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let file = netcdf::open(mode_file).map_err(netcdf_to_io_error)?;
    let n_vertices = required_dimension_len(&file, "nVertices")?;
    let n_cells = required_dimension_len(&file, "nCells")?;
    let max_edges = required_dimension_len(&file, "maxEdges")?;

    let lon_vertex = required_values_f64(&file, "lonVertex")?;
    let lat_vertex = required_values_f64(&file, "latVertex")?;
    let lon_cell = required_values_f64(&file, "lonCell")?;
    let lat_cell = required_values_f64(&file, "latCell")?;
    let cells_on_vertex = required_values_i32_2d(&file, "cellsOnVertex")?;
    let vertices_on_cell = required_values_i32_2d(&file, "verticesOnCell")?;
    let n_edges_on_cell = required_values_i32(&file, "nEdgesOnCell")?;

    require_len("lonVertex", lon_vertex.len(), n_vertices)?;
    require_len("latVertex", lat_vertex.len(), n_vertices)?;
    require_len("lonCell", lon_cell.len(), n_cells)?;
    require_len("latCell", lat_cell.len(), n_cells)?;
    require_len("cellsOnVertex", cells_on_vertex.len(), n_vertices * 3)?;
    require_len(
        "verticesOnCell",
        vertices_on_cell.len(),
        n_cells * max_edges,
    )?;
    require_len("nEdgesOnCell", n_edges_on_cell.len(), n_cells)?;

    let mut m_points = Vec::with_capacity(n_vertices + 1);
    m_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_vertices {
        m_points.push(LonLatPoint {
            lon: normalize_degrees(rad_to_deg(lon_vertex[idx])),
            lat: rad_to_deg(lat_vertex[idx]),
        });
    }

    let mut w_points = Vec::with_capacity(n_cells + 1);
    w_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_cells {
        w_points.push(LonLatPoint {
            lon: normalize_degrees(rad_to_deg(lon_cell[idx])),
            lat: rad_to_deg(lat_cell[idx]),
        });
    }

    let mut m_to_w = Vec::with_capacity(n_vertices + 1);
    m_to_w.push([1, 1, 1]);
    for vertex in 0..n_vertices {
        let base = vertex * 3;
        m_to_w.push([
            cells_on_vertex[base] + 1,
            cells_on_vertex[base + 1] + 1,
            cells_on_vertex[base + 2] + 1,
        ]);
    }

    let mut w_to_m = Vec::with_capacity(n_cells + 1);
    w_to_m.push(vec![1]);
    for cell in 0..n_cells {
        let base = cell * max_edges;
        w_to_m.push(
            vertices_on_cell[base..base + max_edges]
                .iter()
                .map(|value| value + 1)
                .collect(),
        );
    }

    let mut n_w_to_m = Vec::with_capacity(n_cells + 1);
    n_w_to_m.push(1);
    n_w_to_m.extend(n_edges_on_cell);

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}
