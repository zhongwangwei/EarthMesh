use std::io;

use super::*;

pub(crate) fn fill_method_c_w_face_neighbors_from_edges(
    u_edges: &mut [IcosahedronUEdge],
    w_faces: &mut [IcosahedronWFace],
) -> io::Result<()> {
    let nwd = w_faces.len().saturating_sub(1);
    for iw in 2..=nwd {
        for slot in 0..3 {
            let iu = w_faces[iw].iu[slot];
            let edge = u_edges[iu];
            let other_iw = if edge.iw[0] == iw {
                edge.iw[1]
            } else if edge.iw[1] == iw {
                edge.iw[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("W face {iw} edge slot {slot} points at U edge {iu}, but edge does not point back"),
                ));
            };
            require_method_c_id("Method-C W neighbor face", other_iw, nwd)?;
            w_faces[iw].iw[slot] = other_iw;
        }
    }

    for iw in 2..=nwd {
        let [iw1, iw2, iw3] = [w_faces[iw].iw[0], w_faces[iw].iw[1], w_faces[iw].iw[2]];
        require_method_c_id("Method-C W inner neighbor", iw1, nwd)?;
        require_method_c_id("Method-C W inner neighbor", iw2, nwd)?;
        require_method_c_id("Method-C W inner neighbor", iw3, nwd)?;

        let pair1 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw1].iw[0], w_faces[iw1].iw[1], w_faces[iw1].iw[2]],
        );
        let pair2 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw2].iw[0], w_faces[iw2].iw[1], w_faces[iw2].iw[2]],
        );
        let pair3 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw3].iw[0], w_faces[iw3].iw[1], w_faces[iw3].iw[2]],
        );

        w_faces[iw].iw[3] = pair1[0];
        w_faces[iw].iw[4] = pair1[1];
        w_faces[iw].iw[5] = pair2[0];
        w_faces[iw].iw[6] = pair2[1];
        w_faces[iw].iw[7] = pair3[0];
        w_faces[iw].iw[8] = pair3[1];
    }

    Ok(())
}
