use std::io;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn ensure_method_c_selected_faces_share_parent_mrlw(
        &self,
        selected_faces: &[bool],
        child_level: usize,
    ) -> io::Result<()> {
        require_method_c_len(
            "Method-C selected faces",
            selected_faces.len(),
            self.nwd + 1,
        )?;

        let radius = active_mesh_radius(self)?;
        let mut parent_mrlw = None;
        for iw in 2..=self.nwd {
            if !selected_faces[iw] {
                continue;
            }

            let face = self.w_faces[iw];
            if let Some(expected_mrlw) = parent_mrlw {
                if face.mrlw != expected_mrlw {
                    let center = normalized_face_center(
                        self.m_points[face.im[0]],
                        self.m_points[face.im[1]],
                        self.m_points[face.im[2]],
                        radius,
                    )?;
                    let ll = xyz_to_lonlat_degrees(center);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Current nested grid {child_level} crosses (or is too close to) the next coarser grid boundary at W face {iw} (mrlw={}, expected_mrlw={}, lon={:.3}, lat={:.3})",
                            face.mrlw, expected_mrlw, ll.lon_degrees, ll.lat_degrees
                        ),
                    ));
                }
            } else {
                parent_mrlw = Some(face.mrlw);
            }
        }

        Ok(())
    }
}
