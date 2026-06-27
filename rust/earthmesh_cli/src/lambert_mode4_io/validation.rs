use std::io;

use super::Mode4Mesh;

pub(crate) fn validate_mode4_mesh_for_area_judge(mesh: &Mode4Mesh) -> io::Result<()> {
    if mesh.bound_points() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mode4 mesh must include a placeholder plus at least one boundary point",
        ));
    }
    if mesh.mode_points() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mode4 mesh must include a placeholder plus at least one cell",
        ));
    }
    if mesh.ngr_bound.len() != mesh.n_ngr.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mode4 ngr_bound and n_ngr lengths must match",
        ));
    }
    for (index, point) in mesh.lonlat_bound.iter().enumerate() {
        if index == 0 {
            continue;
        }
        if !point.lon.is_finite() || !point.lat.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mode4 boundary point {} coordinates must be finite",
                    index + 1
                ),
            ));
        }
    }
    for cell_index in 1..mesh.mode_points() {
        for &bound_index in &mesh.ngr_bound[cell_index] {
            if bound_index < 1 || bound_index as usize > mesh.bound_points() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("mode4 cell {cell_index} references out-of-range vertex {bound_index}"),
                ));
            }
        }
    }
    Ok(())
}
