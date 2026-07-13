use std::io;
use std::path::{Path, PathBuf};

use crate::{
    lambert_vertices_to_mode4_mesh, read_lambert_vertices_netcdf, write_mode4_mesh_netcdf,
};

/// Report for the current NetCDF branch of `mkgrd.F90:mode4mesh_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode4MeshMakeReport {
    pub input: PathBuf,
    pub grid_select: String,
    pub output: PathBuf,
    pub bound_points: usize,
    pub mode_points: usize,
}

/// Execute the NetCDF-supported branch of `mode4mesh_make`.
///
/// This currently ports the active Lambert `.nc/.nc4` path. The compatibility Canonical
/// lonlat `.nc` path and Lambert `.nml` path stop immediately, so they are
/// represented as `InvalidInput` until deliberately enabled with tests.
pub fn mode4mesh_make_netcdf(
    inputfile: impl AsRef<Path>,
    grid_select: &str,
    output: impl AsRef<Path>,
) -> io::Result<Mode4MeshMakeReport> {
    let inputfile = inputfile.as_ref();
    let output = output.as_ref();
    let extension = inputfile
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let grid_select_trimmed = grid_select.trim();

    match grid_select_trimmed {
        "lambert" => {
            if !matches!(extension.as_deref(), Some("nc") | Some("nc4")) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "lambert mode4mesh_make currently requires .nc or .nc4 input",
                ));
            }
            let vertices = read_lambert_vertices_netcdf(inputfile)?;
            let mesh = lambert_vertices_to_mode4_mesh(&vertices)?;
            write_mode4_mesh_netcdf(output, &mesh)?;
            Ok(Mode4MeshMakeReport {
                input: inputfile.to_path_buf(),
                grid_select: grid_select_trimmed.to_string(),
                output: output.to_path_buf(),
                bound_points: mesh.bound_points(),
                mode_points: mesh.mode_points(),
            })
        }
        "lonlat" => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lonlat mode4mesh_make is not enabled by this NetCDF adapter",
        )),
        "cubical" => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cubical mode4mesh_make is not supported",
        )),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported grid_select {other}"),
        )),
    }
}
