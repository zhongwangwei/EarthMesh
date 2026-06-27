use std::io;
use std::path::Path;

use super::quality::grid_quality_global_from_unstructured_mesh;
use crate::*;

/// Execute `mkgrd.F90:Inital_Grid_Quality_Check` for an existing gridfile.
///
/// The Fortran entry reads the current unstructured grid and writes
/// `quality_NXP####_##_global_orial.nc4`; this Rust wrapper keeps that side
/// effect explicit without touching spring-adjustment state.
pub fn run_mkgrd_initial_grid_quality_check(
    input_gridfile: impl AsRef<Path>,
    quality_output: impl AsRef<Path>,
) -> io::Result<()> {
    let mesh = normalize_unstructured_mesh_legacy_placeholders(&read_unstructured_mesh_netcdf(
        input_gridfile.as_ref(),
    )?)?;
    let quality = grid_quality_global_from_unstructured_mesh(&mesh)?;
    write_grid_quality_global_netcdf(quality_output, &quality).map(|_| ())
}
