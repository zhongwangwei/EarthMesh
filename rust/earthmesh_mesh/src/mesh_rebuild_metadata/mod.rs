use std::io;

use super::*;

pub fn default_method_c_m_metadata(nmd: usize) -> Vec<IcosahedronMPointMetadata> {
    vec![IcosahedronMPointMetadata::default(); nmd + 1]
}

pub fn method_c_identity_prognostic_map(max_id: usize) -> Vec<usize> {
    (0..=max_id).collect()
}

pub fn derive_method_c_m_metadata_from_w_faces(
    nmd: usize,
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointMetadata>> {
    let mut metadata = default_method_c_m_metadata(nmd);
    let mut seen = vec![false; nmd + 1];
    for face in w_faces.iter().skip(2) {
        for &im in &face.im {
            require_method_c_id("Method-C M metadata face vertex", im, nmd)?;
            seen[im] = true;
            metadata[im].mrlm = metadata[im].mrlm.max(face.mrlw.max(1));
            metadata[im].mrlm_orig = metadata[im].mrlm_orig.max(face.mrlw_orig.max(1));
            metadata[im].ngr = metadata[im].ngr.max(face.ngr.max(1));
        }
    }
    for (im, &has_face) in seen.iter().enumerate().skip(2) {
        if !has_face {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Method-C M metadata point {im} is not incident on any W face"),
            ));
        }
    }
    Ok(metadata)
}
