use super::*;

/// Port of the M-point polygon assembly portion of
/// `icosahedron.F90:tri_neighbors`.
///
/// Returns a Canonical-indexed table (`0` and `1` are placeholders) with each M
/// point's surrounding U and W rings. The original routine stops when a ring
/// exceeds seven sides; this Rust boundary returns `None` instead.
pub fn derive_icosahedron_m_neighbors_canonical(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> Option<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_m_neighbors_canonical_checked(nmd, u_edges, w_faces).ok()
}

pub fn derive_icosahedron_m_neighbors_canonical_checked(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(nmd, u_edges, w_faces, None)
}

pub fn derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
    m_prognostic: Option<&[usize]>,
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    let mut m_points = vec![IcosahedronMPointNeighbors::default(); nmd + 1];

    for iu in 2..u_edges.len() {
        for j in 0..2 {
            let im = u_edges.get(iu).map(|edge| edge.im[j]).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} is out of range while deriving M neighbors"),
                )
            })?;
            let iw = u_edges.get(iu).map(|edge| edge.iw[j]).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} is out of range while deriving M neighbors"),
                )
            })?;
            if im >= m_points.len() || iw >= w_faces.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} endpoint/face out of range: im={im}, iw={iw}"),
                ));
            }
            if m_prognostic
                .and_then(|map| map.get(im))
                .copied()
                .is_some_and(|partner| partner > 1 && partner != im)
            {
                continue;
            }

            if m_points[im].npoly != 0 && w_faces[iw].npoly >= 3 {
                continue;
            }

            let mut m_point = m_points[im];
            let start_iu = iu;
            let mut iunow = iu;
            let mut npoly = 0usize;
            let mut walk_trace = Vec::<(usize, [usize; 2], [usize; 6], [usize; 12])>::new();

            while iunow > 1 {
                npoly += 1;
                let edge_now = *u_edges.get(iunow).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("U edge {iunow} is out of range in M point {im} ring"),
                    )
                })?;
                walk_trace.push((iunow, edge_now.im, edge_now.iw, edge_now.iu));
                if npoly > 7 {
                    return Err(repairable_error(
                        RepairableKind::Valence,
                        Some(im),
                        format!(
                            "Method-C perimeter length invalid: Current nested grid crosses (or is too close to) the next coarser grid boundary; M point {im} exceeds 7-edge Method-C ring while walking from U edge {iu}; trace {:?}",
                            walk_trace
                        ),
                    ));
                }

                let ring_slot = npoly - 1;
                m_point.iu[ring_slot] = iunow;

                if edge_now.im[0] == im {
                    if edge_now.iw[1] > 1 {
                        m_point.iw[ring_slot] = edge_now.iw[1];
                        iunow = edge_now.iu[2];
                    } else {
                        iunow = start_iu;
                    }
                } else if edge_now.iw[0] > 1 {
                    m_point.iw[ring_slot] = edge_now.iw[0];
                    iunow = edge_now.iu[1];
                } else {
                    iunow = start_iu;
                }

                m_point.npoly = npoly;
                if iunow == start_iu {
                    break;
                }
            }

            m_points[im] = m_point;
        }
    }

    Ok(m_points)
}
