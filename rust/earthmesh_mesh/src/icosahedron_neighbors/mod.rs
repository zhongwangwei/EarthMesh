use super::*;

pub fn tri_neighbors_outer_w_pair(current_iw: usize, neighbor_inner: [usize; 3]) -> [usize; 2] {
    if current_iw == neighbor_inner[0] {
        [neighbor_inner[1], neighbor_inner[2]]
    } else if current_iw == neighbor_inner[1] {
        [neighbor_inner[2], neighbor_inner[0]]
    } else {
        [neighbor_inner[0], neighbor_inner[1]]
    }
}

/// Port of the W-face portions of `icosahedron.F90:tri_neighbors`.
///
/// This fills `itab_wd(iw)%npoly`, the three surrounding M points, the three
/// inner W neighbors, and the six outer W neighbors for every active W face
/// (`iw = 2..nwd`). U-edge and M-point reciprocal connectivity remain separate
/// architecture surfaces.
pub fn derive_icosahedron_w_neighbors_canonical(
    connectivity: &mut IcosahedronDiamondConnectivity,
) -> Option<()> {
    for iw in 2..connectivity.w_faces.len() {
        let [iu1, iu2, iu3] = connectivity.w_faces.get(iw)?.iu;

        let mut face = *connectivity.w_faces.get(iw)?;
        face.npoly = usize::from(iu1 > 1) + usize::from(iu2 > 1) + usize::from(iu3 > 1);

        if iu1 > 1 {
            let edge = connectivity.u_edges.get(iu1)?;
            if iw == edge.iw[0] {
                face.im[2] = edge.im[0];
                face.im[1] = edge.im[1];
                face.iw[0] = edge.iw[1];
            } else if iw == edge.iw[1] {
                face.im[2] = edge.im[1];
                face.im[1] = edge.im[0];
                face.iw[0] = edge.iw[0];
            }
        }

        if iu2 > 1 {
            let edge = connectivity.u_edges.get(iu2)?;
            if iw == edge.iw[0] {
                face.im[0] = edge.im[0];
                face.im[2] = edge.im[1];
                face.iw[1] = edge.iw[1];
            } else if iw == edge.iw[1] {
                face.im[0] = edge.im[1];
                face.im[2] = edge.im[0];
                face.iw[1] = edge.iw[0];
            }
        }

        if iu3 > 1 {
            let edge = connectivity.u_edges.get(iu3)?;
            if iw == edge.iw[0] {
                face.im[1] = edge.im[0];
                face.im[0] = edge.im[1];
                face.iw[2] = edge.iw[1];
            } else if iw == edge.iw[1] {
                face.im[1] = edge.im[1];
                face.im[0] = edge.im[0];
                face.iw[2] = edge.iw[0];
            }
        }

        *connectivity.w_faces.get_mut(iw)? = face;
    }

    for iw in 2..connectivity.w_faces.len() {
        let [iw1, iw2, iw3] = [
            connectivity.w_faces.get(iw)?.iw[0],
            connectivity.w_faces.get(iw)?.iw[1],
            connectivity.w_faces.get(iw)?.iw[2],
        ];
        let neighbor1 = connectivity.w_faces.get(iw1)?.iw;
        let neighbor2 = connectivity.w_faces.get(iw2)?.iw;
        let neighbor3 = connectivity.w_faces.get(iw3)?.iw;

        let pair1 = tri_neighbors_outer_w_pair(iw, [neighbor1[0], neighbor1[1], neighbor1[2]]);
        let pair2 = tri_neighbors_outer_w_pair(iw, [neighbor2[0], neighbor2[1], neighbor2[2]]);
        let pair3 = tri_neighbors_outer_w_pair(iw, [neighbor3[0], neighbor3[1], neighbor3[2]]);

        let face = connectivity.w_faces.get_mut(iw)?;
        face.iw[3] = pair1[0];
        face.iw[4] = pair1[1];
        face.iw[5] = pair2[0];
        face.iw[6] = pair2[1];
        face.iw[7] = pair3[0];
        face.iw[8] = pair3[1];
    }

    Some(())
}

/// Integrated Rust wrapper for `icosahedron.F90:tri_neighbors`.
///
/// The mutable U/W tables are updated in the same high-level sequence as the
/// Canonical subroutine: W-face neighbors, U-edge reciprocal neighbors, then
/// M-point polygon rings. The returned M table is Canonical-indexed.
pub fn derive_icosahedron_tri_neighbors_canonical(
    nmd: usize,
    connectivity: &mut IcosahedronDiamondConnectivity,
) -> Option<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_w_neighbors_canonical(connectivity)?;
    derive_icosahedron_u_neighbors_canonical(connectivity)?;
    derive_icosahedron_m_neighbors_canonical(nmd, &connectivity.u_edges, &connectivity.w_faces)
}
