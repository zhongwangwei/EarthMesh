use super::*;

pub(crate) fn fill_cart_hex_w_face_neighbors_from_edges(
    u_edges: &[IcosahedronUEdge],
    w_faces: &mut [IcosahedronWFace],
    w_prognostic: &[usize],
) -> io::Result<()> {
    let nwd = w_faces.len().saturating_sub(1);
    for iw in 2..=nwd {
        if w_prognostic[iw] != iw {
            continue;
        }
        for slot in 0..3 {
            let iu = w_faces[iw].iu[slot];
            require_method_c_id(
                "Method-C cart_hex W face U edge",
                iu,
                u_edges.len().saturating_sub(1),
            )?;
            let edge = u_edges[iu];
            let other_iw = if edge.iw[0] == iw {
                edge.iw[1]
            } else if edge.iw[1] == iw {
                edge.iw[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cart_hex W face {iw} edge slot {slot} points at U edge {iu}, but edge does not point back"),
                ));
            };
            require_method_c_id("Method-C cart_hex W neighbor face", other_iw, nwd)?;
            w_faces[iw].iw[slot] = other_iw;
        }
    }

    for iw in 2..=nwd {
        let partner = w_prognostic[iw];
        if partner > 1 && partner != iw {
            require_method_c_id("Method-C cart_hex periodic W face partner", partner, nwd)?;
            let boundary_iu = w_faces[iw].iu[0];
            w_faces[iw] = w_faces[partner];
            if boundary_iu > 1 {
                w_faces[iw].iu[0] = boundary_iu;
            }
        }
    }

    for iw in 2..=nwd {
        if w_prognostic[iw] != iw {
            continue;
        }
        let [iw1, iw2, iw3] = [w_faces[iw].iw[0], w_faces[iw].iw[1], w_faces[iw].iw[2]];
        require_method_c_id("Method-C cart_hex W inner neighbor", iw1, nwd)?;
        require_method_c_id("Method-C cart_hex W inner neighbor", iw2, nwd)?;
        require_method_c_id("Method-C cart_hex W inner neighbor", iw3, nwd)?;

        let raw_pair1 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw1].iw[0], w_faces[iw1].iw[1], w_faces[iw1].iw[2]],
        );
        let raw_pair2 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw2].iw[0], w_faces[iw2].iw[1], w_faces[iw2].iw[2]],
        );
        let raw_pair3 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw3].iw[0], w_faces[iw3].iw[1], w_faces[iw3].iw[2]],
        );
        let outer_candidates = [
            raw_pair1[0],
            raw_pair1[1],
            raw_pair2[0],
            raw_pair2[1],
            raw_pair3[0],
            raw_pair3[1],
        ];
        let pair1 = order_method_c_outer_w_pair_for_fill_rad3(
            w_faces,
            raw_pair1,
            outer_candidates,
            w_faces[iw].im[1],
        )?;
        let pair2 = order_method_c_outer_w_pair_for_fill_rad3(
            w_faces,
            raw_pair2,
            outer_candidates,
            w_faces[iw].im[2],
        )?;
        let pair3 = order_method_c_outer_w_pair_for_fill_rad3(
            w_faces,
            raw_pair3,
            outer_candidates,
            w_faces[iw].im[0],
        )?;

        w_faces[iw].iw[3] = pair1[0];
        w_faces[iw].iw[4] = pair1[1];
        w_faces[iw].iw[5] = pair2[0];
        w_faces[iw].iw[6] = pair2[1];
        w_faces[iw].iw[7] = pair3[0];
        w_faces[iw].iw[8] = pair3[1];
    }

    Ok(())
}
