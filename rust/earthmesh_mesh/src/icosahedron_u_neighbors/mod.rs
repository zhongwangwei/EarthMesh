use super::*;

/// Port of the U-edge portion of `icosahedron.F90:tri_neighbors`.
///
/// This fills each active U edge's refinement level, four same-ring U
/// neighbors, four outer W neighbors, and eight second-ring U neighbors from
/// already-populated W-face inner-neighbor tables. W-face and M-point
/// derivation are intentionally kept as separate migration surfaces.
pub fn derive_icosahedron_u_neighbors_fortran(
    connectivity: &mut IcosahedronDiamondConnectivity,
) -> Option<()> {
    for iu in 2..connectivity.u_edges.len() {
        let mut edge = *connectivity.u_edges.get(iu)?;
        let iw1 = edge.iw[0];
        let iw2 = edge.iw[1];

        let w1 = *connectivity.w_faces.get(iw1)?;
        let w2 = *connectivity.w_faces.get(iw2)?;
        edge.mrlu = w1.mrlw.max(w2.mrlw);

        if w1.iu[0] == iu {
            edge.iu[0] = w1.iu[1];
            edge.iu[1] = w1.iu[2];
        } else if w1.iu[1] == iu {
            edge.iu[0] = w1.iu[2];
            edge.iu[1] = w1.iu[0];
        } else {
            edge.iu[0] = w1.iu[0];
            edge.iu[1] = w1.iu[1];
        }

        if w2.iu[0] == iu {
            edge.iu[2] = w2.iu[2];
            edge.iu[3] = w2.iu[1];
        } else if w2.iu[1] == iu {
            edge.iu[2] = w2.iu[0];
            edge.iu[3] = w2.iu[2];
        } else {
            edge.iu[2] = w2.iu[1];
            edge.iu[3] = w2.iu[0];
        }

        let iu1 = edge.iu[0];
        let iu2 = edge.iu[1];
        let iu3 = edge.iu[2];
        let iu4 = edge.iu[3];

        let neighbor1 = *connectivity.u_edges.get(iu1)?;
        edge.iw[2] = if neighbor1.iw[0] == iw1 {
            neighbor1.iw[1]
        } else {
            neighbor1.iw[0]
        };

        let neighbor2 = *connectivity.u_edges.get(iu2)?;
        edge.iw[3] = if neighbor2.iw[0] == iw1 {
            neighbor2.iw[1]
        } else {
            neighbor2.iw[0]
        };

        let neighbor3 = *connectivity.u_edges.get(iu3)?;
        edge.iw[4] = if neighbor3.iw[0] == iw2 {
            neighbor3.iw[1]
        } else {
            neighbor3.iw[0]
        };

        let neighbor4 = *connectivity.u_edges.get(iu4)?;
        edge.iw[5] = if neighbor4.iw[0] == iw2 {
            neighbor4.iw[1]
        } else {
            neighbor4.iw[0]
        };

        let iw3 = edge.iw[2];
        let iw4 = edge.iw[3];
        let iw5 = edge.iw[4];
        let iw6 = edge.iw[5];

        let w3 = *connectivity.w_faces.get(iw3)?;
        if iu1 == w3.iu[0] {
            edge.iu[4] = w3.iu[1];
            edge.iu[5] = w3.iu[2];
        } else if iu1 == w3.iu[1] {
            edge.iu[4] = w3.iu[2];
            edge.iu[5] = w3.iu[0];
        } else {
            edge.iu[4] = w3.iu[0];
            edge.iu[5] = w3.iu[1];
        }

        let w4 = *connectivity.w_faces.get(iw4)?;
        if iu2 == w4.iu[0] {
            edge.iu[6] = w4.iu[1];
            edge.iu[7] = w4.iu[2];
        } else if iu2 == w4.iu[1] {
            edge.iu[6] = w4.iu[2];
            edge.iu[7] = w4.iu[0];
        } else {
            edge.iu[6] = w4.iu[0];
            edge.iu[7] = w4.iu[1];
        }

        let w5 = *connectivity.w_faces.get(iw5)?;
        if iu3 == w5.iu[0] {
            edge.iu[8] = w5.iu[2];
            edge.iu[9] = w5.iu[1];
        } else if iu3 == w5.iu[1] {
            edge.iu[8] = w5.iu[0];
            edge.iu[9] = w5.iu[2];
        } else {
            edge.iu[8] = w5.iu[1];
            edge.iu[9] = w5.iu[0];
        }

        let w6 = *connectivity.w_faces.get(iw6)?;
        if iu4 == w6.iu[0] {
            edge.iu[10] = w6.iu[2];
            edge.iu[11] = w6.iu[1];
        } else if iu4 == w6.iu[1] {
            edge.iu[10] = w6.iu[0];
            edge.iu[11] = w6.iu[2];
        } else {
            edge.iu[10] = w6.iu[1];
            edge.iu[11] = w6.iu[0];
        }

        *connectivity.u_edges.get_mut(iu)? = edge;
    }

    Some(())
}
