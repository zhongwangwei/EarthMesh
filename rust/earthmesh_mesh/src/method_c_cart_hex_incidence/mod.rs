use super::*;
use crate::method_c_incidence_ring::{
    method_c_face_incident_edges_for_m, order_method_c_m_ring_from_incidence,
};

pub(crate) fn derive_cart_hex_m_neighbors_from_active_faces(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
    w_prognostic: &[usize],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    let mut incident_w = vec![Vec::<usize>::new(); nmd + 1];
    for (iw, face) in w_faces.iter().enumerate().skip(2) {
        if w_prognostic.get(iw).copied().unwrap_or(iw) != iw {
            continue;
        }
        if face.npoly != 3 || face.im.iter().any(|&im| im <= 1) {
            continue;
        }
        for &im in &face.im {
            require_method_c_id("Method-C cart_hex W face M vertex", im, nmd)?;
            incident_w[im].push(iw);
        }
    }

    let mut m_neighbors = vec![IcosahedronMPointNeighbors::default(); nmd + 1];
    for im in 2..=nmd {
        let mut w_list = incident_w[im].clone();
        w_list.sort_unstable();
        w_list.dedup();
        if !(3..=7).contains(&w_list.len()) {
            continue;
        }

        let mut edge_hits = BTreeMap::<usize, usize>::new();
        for &iw in &w_list {
            for iu in method_c_face_incident_edges_for_m(im, iw, u_edges, w_faces)? {
                *edge_hits.entry(iu).or_insert(0usize) += 1;
            }
        }
        let mut u_list = edge_hits
            .into_iter()
            .filter(|(_, hits)| *hits >= 2)
            .map(|(iu, _)| iu)
            .collect::<Vec<_>>();
        u_list.sort_unstable();
        u_list.dedup();
        if !(3..=7).contains(&u_list.len()) {
            continue;
        }

        let (ordered_u, ordered_w) =
            order_method_c_m_ring_from_incidence(im, &u_list, &w_list, u_edges, w_faces)?;
        let mut neighbor = IcosahedronMPointNeighbors {
            npoly: ordered_u.len(),
            ..IcosahedronMPointNeighbors::default()
        };
        for (slot, (&iu, &iw)) in ordered_u.iter().zip(ordered_w.iter()).enumerate() {
            neighbor.iu[slot] = iu;
            neighbor.iw[slot] = iw;
        }
        m_neighbors[im] = neighbor;
    }

    Ok(m_neighbors)
}
