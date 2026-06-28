use std::io;
use std::path::Path;

use crate::{
    netcdf_to_io_error, rad_to_deg, require_len, required_dimension_len, required_values_f64,
    required_values_i32_matrix, LonLatPoint,
};

/// Edge-reference payload read by `MOD_file_preprocess.F90:data_read`.
#[derive(Debug, Clone, PartialEq)]
pub struct MpasEdgeReference {
    pub cells_on_edge_reference: Vec<[i32; 2]>,
    pub edge_points: Vec<LonLatPoint>,
}

/// Read the MPAS edge-reference fields consumed by
/// `MOD_file_preprocess.F90:data_read`.
///
/// The returned payload preserves the Fortran placeholder row at index `0`,
/// shifts `cellsOnEdge` by `+1` after reading, converts edge coordinates from
/// radians to degrees, and applies the legacy single-step `lon > 180 => lon -=
/// 360` normalization.
pub fn read_mpas_edge_reference_netcdf(input: impl AsRef<Path>) -> io::Result<MpasEdgeReference> {
    let file = crate::open_netcdf(input.as_ref()).map_err(netcdf_to_io_error)?;
    let n_edges = required_dimension_len(&file, "nEdges")?;
    let two = required_dimension_len(&file, "TWO")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("TWO dimension must be 2 for MPAS cellsOnEdge, got {two}"),
        ));
    }

    let cells = required_values_i32_matrix(&file, "cellsOnEdge", "nEdges", "TWO", n_edges, 2)?;
    let lon_edge = required_values_f64(&file, "lonEdge")?;
    let lat_edge = required_values_f64(&file, "latEdge")?;
    require_len("lonEdge", lon_edge.len(), n_edges)?;
    require_len("latEdge", lat_edge.len(), n_edges)?;

    let mut cells_on_edge_reference = Vec::with_capacity(n_edges + 1);
    cells_on_edge_reference.push([1, 1]);
    for edge in 0..n_edges {
        let base = edge * 2;
        cells_on_edge_reference.push([cells[base] + 1, cells[base + 1] + 1]);
    }

    let mut edge_points = Vec::with_capacity(n_edges + 1);
    edge_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for edge in 0..n_edges {
        let mut lon = rad_to_deg(lon_edge[edge]);
        if lon > 180.0 {
            lon -= 360.0;
        }
        edge_points.push(LonLatPoint {
            lon,
            lat: rad_to_deg(lat_edge[edge]),
        });
    }

    Ok(MpasEdgeReference {
        cells_on_edge_reference,
        edge_points,
    })
}
