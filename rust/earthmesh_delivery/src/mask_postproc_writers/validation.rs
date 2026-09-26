use std::io;

use crate::matrix_width;

use super::types::{EarthmeshInfo, PatchIdMesh};

pub(super) fn validate_patchid_mesh(patch: &PatchIdMesh) -> io::Result<()> {
    let nlon = patch.elmindex.len();
    let nlat = matrix_width("elmindex", &patch.elmindex)?;
    for (name, actual, required) in [
        ("lon_w", patch.lon_w.len(), nlon),
        ("lon_e", patch.lon_e.len(), nlon),
        ("longitude", patch.longitude.len(), nlon),
        ("lat_n", patch.lat_n.len(), nlat),
        ("lat_s", patch.lat_s.len(), nlat),
        ("latitude", patch.latitude.len(), nlat),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_earthmesh_info(info: &EarthmeshInfo) -> io::Result<()> {
    if info.refine_degree_f.len() != info.seaorland_ustr_f.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refine_degree_f and seaorland_ustr_f must have matching length: {} != {}",
                info.refine_degree_f.len(),
                info.seaorland_ustr_f.len()
            ),
        ));
    }
    Ok(())
}
