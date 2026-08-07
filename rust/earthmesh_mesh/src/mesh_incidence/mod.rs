use super::*;
use crate::mesh_incidence_ring::{
    method_c_face_incident_edges_for_m, order_method_c_m_ring_from_incidence,
};

pub(crate) fn derive_method_c_m_neighbors_from_incidence(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    let mut incident_w = vec![Vec::<usize>::new(); nmd + 1];

    for (iw, face) in w_faces.iter().enumerate().skip(2) {
        let mut unique_im = face.im.to_vec();
        unique_im.sort_unstable();
        unique_im.dedup();
        for &im in &unique_im {
            require_method_c_id("W face M vertex", im, nmd)?;
        }
        if unique_im.len() < 2 {
            continue;
        }
        for &im in &unique_im {
            incident_w[im].push(iw);
        }
    }

    let mut m_neighbors = vec![IcosahedronMPointNeighbors::default(); nmd + 1];
    for im in 2..=nmd {
        let mut w_list = incident_w[im].clone();
        w_list.sort_unstable();
        w_list.dedup();

        if w_list.is_empty() {
            continue;
        }
        if !(3..=7).contains(&w_list.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "M point {im} has unsupported Method-C incident face count {}",
                    w_list.len()
                ),
            ));
        }

        let mut edge_hits = BTreeMap::<usize, usize>::new();
        let mut valid_w_list = Vec::<usize>::new();
        for &iw in &w_list {
            let Ok(incident) = method_c_face_incident_edges_for_m(im, iw, u_edges, w_faces) else {
                continue;
            };
            valid_w_list.push(iw);
            for iu in incident {
                *edge_hits.entry(iu).or_insert(0usize) += 1;
            }
        }
        if valid_w_list.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "M point {im} has too few incident W faces after filtering malformed ring entries: {}",
                    valid_w_list.len()
                ),
            ));
        }
        let mut u_list = edge_hits
            .into_iter()
            .filter(|(_, hits)| *hits >= 2)
            .map(|(iu, _)| iu)
            .collect::<Vec<_>>();
        u_list.sort_unstable();
        u_list.dedup();

        if !(3..=7).contains(&u_list.len()) {
            let edge_vertices = u_list
                .iter()
                .map(|&iu| (iu, u_edges[iu].im, u_edges[iu].iw))
                .collect::<Vec<_>>();
            let face_vertices = w_list
                .iter()
                .map(|&iw| (iw, w_faces[iw].im))
                .collect::<Vec<_>>();
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "M point {im} has unsupported Method-C incidence: {} U edges {:?}, {} W faces {:?}",
                    u_list.len(),
                    edge_vertices,
                    w_list.len(),
                    face_vertices
                ),
            ));
        }

        let (ordered_u, ordered_w) =
            order_method_c_m_ring_from_incidence(im, &u_list, &valid_w_list, u_edges, w_faces)?;
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
