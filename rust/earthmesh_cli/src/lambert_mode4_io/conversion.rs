use std::io;
use std::path::{Path, PathBuf};

use crate::{LonLatPoint, MaskCountState};

use super::{read_lambert_vertices_netcdf, write_mode4_mesh_netcdf, LambertVertices, Mode4Mesh};

/// Convert Lambert vertex arrays into the Fortran-indexed mode4 mesh payload.
pub fn lambert_vertices_to_mode4_mesh(vertices: &LambertVertices) -> io::Result<Mode4Mesh> {
    if vertices.xi_vert < 2 || vertices.eta_vert < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lambert xi_vert and eta_vert must both be at least 2",
        ));
    }
    let expected = vertices.xi_vert * vertices.eta_vert;
    if vertices.lon_vert.len() != expected || vertices.lat_vert.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lambert lon_vert/lat_vert lengths must match xi_vert * eta_vert",
        ));
    }

    let lon_points = vertices.xi_vert - 1;
    let lat_points = vertices.eta_vert - 1;
    let bound_points = (lon_points + 1) * (lat_points + 1) + 1;
    let mode_points = lon_points * lat_points + 1;

    let mut lonlat_bound = vec![
        LonLatPoint {
            lon: -999.0,
            lat: -999.0
        };
        bound_points
    ];
    let mut out_idx = 1;
    for j in 0..vertices.eta_vert {
        for i in 0..vertices.xi_vert {
            let source_idx = i + j * vertices.xi_vert;
            let mut lon = vertices.lon_vert[source_idx];
            if lon > 180.0 {
                lon -= 360.0;
            }
            lonlat_bound[out_idx] = LonLatPoint {
                lon,
                lat: vertices.lat_vert[source_idx],
            };
            out_idx += 1;
        }
    }

    let mut ngr_bound = vec![[1_i32; 4]; mode_points];
    let mut cell_idx = 1;
    for j in 0..lat_points {
        for i in 0..lon_points {
            let lower_left = i + j * vertices.xi_vert + 2;
            ngr_bound[cell_idx] = [
                lower_left as i32,
                (lower_left + 1) as i32,
                (lower_left + vertices.xi_vert + 1) as i32,
                (lower_left + vertices.xi_vert) as i32,
            ];
            cell_idx += 1;
        }
    }

    Ok(Mode4Mesh {
        lonlat_bound,
        ngr_bound,
        n_ngr: vec![4; mode_points],
    })
}

pub fn convert_lambert_mask_netcdf(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<PathBuf> {
    let vertices = read_lambert_vertices_netcdf(inputfile)?;
    let mesh = lambert_vertices_to_mode4_mesh(&vertices)?;
    let output = counts.next_lambert_output(mask_select, 0, file_dir)?;
    write_mode4_mesh_netcdf(&output, &mesh)?;
    Ok(output)
}
