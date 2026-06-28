use std::{fs, io, path::Path};

use crate::{gridfile_output_path, netcdf_to_io_error, UnstructuredMeshWriteReport};

pub fn copy_existing_earthmesh_mode_file(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let source = crate::open_netcdf(mode_file).map_err(netcdf_to_io_error)?;
    let sjx_points = source
        .dimension("sjx_points")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode_file missing sjx_points"))?
        .len();
    let lbx_points = source
        .dimension("lbx_points")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode_file missing lbx_points"))?
        .len();
    let dimc = source
        .dimension("dimc")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode_file missing dimc"))?
        .len();
    drop(source);

    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(mode_file, &output)?;
    Ok(UnstructuredMeshWriteReport {
        output,
        sjx_points,
        lbx_points,
        dimc,
    })
}
