use std::io;
use std::path::Path;

use crate::{
    require_len, required_dimension_len, required_values_f64, required_values_i32,
    required_values_i32_matrix, validate_unstructured_mesh, LonLatPoint, UnstructuredMesh,
};

use super::rows::trim_trailing_zero_connectivity;

/// Read the compact EarthMesh unstructured gridfile schema produced by
/// `MOD_file_preprocess.F90:Unstructured_Mesh_Save`.
pub fn read_unstructured_mesh_netcdf(input: impl AsRef<Path>) -> io::Result<UnstructuredMesh> {
    let input = input.as_ref();
    let file = netcdf::open(input).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "failed to open unstructured mesh {}: {err}",
                input.display()
            ),
        )
    })?;
    let sjx_points = required_dimension_len(&file, "sjx_points")?;
    let lbx_points = required_dimension_len(&file, "lbx_points")?;
    let dimb = required_dimension_len(&file, "dimb")?;
    let dimc = required_dimension_len(&file, "dimc")?;
    if dimb != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("dimb must be 3 for EarthMesh triangle connectivity, got {dimb}"),
        ));
    }

    let glonm = required_values_f64(&file, "GLONM")?;
    let glatm = required_values_f64(&file, "GLATM")?;
    let glonw = required_values_f64(&file, "GLONW")?;
    let glatw = required_values_f64(&file, "GLATW")?;
    require_len("GLONM", glonm.len(), sjx_points)?;
    require_len("GLATM", glatm.len(), sjx_points)?;
    require_len("GLONW", glonw.len(), lbx_points)?;
    require_len("GLATW", glatw.len(), lbx_points)?;

    let m_to_w_values =
        required_values_i32_matrix(&file, "itab_m%iw", "sjx_points", "dimb", sjx_points, dimb)?;
    let w_to_m_values =
        required_values_i32_matrix(&file, "itab_w%im", "lbx_points", "dimc", lbx_points, dimc)?;
    let n_w_to_m = required_values_i32(&file, "n_ngrwm")?;
    require_len("n_ngrwm", n_w_to_m.len(), lbx_points)?;

    let m_points = (0..sjx_points)
        .map(|idx| LonLatPoint {
            lon: glonm[idx],
            lat: glatm[idx],
        })
        .collect();
    let w_points = (0..lbx_points)
        .map(|idx| LonLatPoint {
            lon: glonw[idx],
            lat: glatw[idx],
        })
        .collect();
    let m_to_w = m_to_w_values
        .chunks_exact(3)
        .map(|row| [row[0], row[1], row[2]])
        .collect();
    let w_to_m = w_to_m_values
        .chunks_exact(dimc)
        .map(trim_trailing_zero_connectivity)
        .collect();

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    validate_unstructured_mesh(&mesh)?;
    Ok(mesh)
}
