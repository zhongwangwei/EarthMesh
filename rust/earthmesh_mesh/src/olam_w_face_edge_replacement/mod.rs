use std::io;

use super::IcosahedronWFace;

pub(crate) fn replace_w_face_edge_after(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    new_iu: usize,
    _label: &str,
) -> io::Result<()> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        face.iu[2] = new_iu;
    } else if face.iu[1] == old_iu {
        face.iu[0] = new_iu;
    } else {
        face.iu[1] = new_iu;
    }
    Ok(())
}

pub(crate) fn replace_w_face_edge_before(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    new_iu: usize,
    _label: &str,
) -> io::Result<()> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        face.iu[1] = new_iu;
    } else if face.iu[1] == old_iu {
        face.iu[2] = new_iu;
    } else {
        face.iu[0] = new_iu;
    }
    Ok(())
}

pub(crate) fn replace_w_face_edge_with_side_return(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    new_iu: usize,
    _label: &str,
) -> io::Result<usize> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        let side = face.iu[1];
        face.iu[2] = new_iu;
        Ok(side)
    } else if face.iu[1] == old_iu {
        let side = face.iu[2];
        face.iu[0] = new_iu;
        Ok(side)
    } else {
        let side = face.iu[0];
        face.iu[1] = new_iu;
        Ok(side)
    }
}

pub(crate) fn replace_w_face_edges_at(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    replacements: [usize; 2],
    _label: &str,
) -> io::Result<()> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        face.iu[1] = replacements[0];
        face.iu[2] = replacements[1];
    } else if face.iu[1] == old_iu {
        face.iu[2] = replacements[0];
        face.iu[0] = replacements[1];
    } else {
        face.iu[0] = replacements[0];
        face.iu[1] = replacements[1];
    }
    Ok(())
}
