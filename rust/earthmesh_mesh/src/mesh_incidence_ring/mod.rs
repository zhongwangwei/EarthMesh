use super::*;

pub(crate) fn order_method_c_m_ring_from_incidence(
    im: usize,
    u_list: &[usize],
    w_list: &[usize],
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<(Vec<usize>, Vec<usize>)> {
    if u_list.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let expected_u = u_list.iter().copied().collect::<BTreeSet<_>>();
    let expected_w = w_list.iter().copied().collect::<BTreeSet<_>>();
    let mut best_actual_u = BTreeSet::<usize>::new();
    let mut best_actual_w = BTreeSet::<usize>::new();
    let mut best_ordered_u = Vec::<usize>::new();
    let mut best_ordered_w = Vec::<usize>::new();
    let mut best_error = String::new();

    for &start_face in w_list {
        let incident = match method_c_face_incident_edges_for_m(im, start_face, u_edges, w_faces) {
            Ok(incident) => incident,
            Err(err) => {
                best_error = err.to_string();
                continue;
            }
        };
        for &start_edge in &incident {
            let mut ordered_u = Vec::with_capacity(u_list.len());
            let mut ordered_w = Vec::with_capacity(w_list.len());
            let mut current_face = start_face;
            let mut incoming_edge = start_edge;
            let mut candidate_error = None::<String>;

            for _ in 0..w_list.len() {
                if ordered_w.contains(&current_face) {
                    break;
                }
                if !expected_w.contains(&current_face) {
                    candidate_error = Some(format!(
                        "walk reached non-incident W face {current_face} from start W {start_face}, U {start_edge}"
                    ));
                    break;
                }
                if !expected_u.contains(&incoming_edge) {
                    candidate_error = Some(format!(
                        "walk reached non-incident U edge {incoming_edge} from start W {start_face}, U {start_edge}"
                    ));
                    break;
                }
                ordered_u.push(incoming_edge);
                ordered_w.push(current_face);

                let face_edges =
                    method_c_face_incident_edges_for_m(im, current_face, u_edges, w_faces)?;
                let outgoing_edge = if face_edges[0] == incoming_edge {
                    face_edges[1]
                } else if face_edges[1] == incoming_edge {
                    face_edges[0]
                } else {
                    candidate_error = Some(format!(
                        "face {current_face} does not contain incoming U edge {incoming_edge}"
                    ));
                    break;
                };
                let edge = u_edges[outgoing_edge];
                let next_face = if edge.iw[0] == current_face {
                    edge.iw[1]
                } else if edge.iw[1] == current_face {
                    edge.iw[0]
                } else {
                    candidate_error = Some(format!(
                        "outgoing U edge {outgoing_edge} does not contain W face {current_face}"
                    ));
                    break;
                };
                current_face = next_face;
                incoming_edge = outgoing_edge;
            }

            let actual_u = ordered_u.iter().copied().collect::<BTreeSet<_>>();
            let actual_w = ordered_w.iter().copied().collect::<BTreeSet<_>>();
            if actual_u == expected_u {
                return Ok((ordered_u, ordered_w));
            }
            if actual_u.len() + actual_w.len() > best_actual_u.len() + best_actual_w.len() {
                best_actual_u = actual_u;
                best_actual_w = actual_w;
                best_ordered_u = ordered_u;
                best_ordered_w = ordered_w;
                best_error = candidate_error.unwrap_or_else(|| "walk closed early".to_string());
            }
        }
    }

    let edge_rows = u_list
        .iter()
        .map(|&iu| (iu, u_edges[iu].im, u_edges[iu].iw))
        .collect::<Vec<_>>();
    let face_rows = w_list
        .iter()
        .map(|&iw| (iw, w_faces[iw].im, w_faces[iw].iu))
        .collect::<Vec<_>>();
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "M point {im} incidence ring did not close over the same U/W sets: best U {:?} vs {:?}, best W {:?} vs {:?}; ordered U {:?}, ordered W {:?}; last walk error {}; U rows {:?}; W rows {:?}",
            best_actual_u,
            expected_u,
            best_actual_w,
            expected_w,
            best_ordered_u,
            best_ordered_w,
            best_error,
            edge_rows,
            face_rows
        ),
    ))
}

pub(crate) fn method_c_face_incident_edges_for_m(
    im: usize,
    iw: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<[usize; 2]> {
    let face = w_faces[iw];
    if !face.im.contains(&im) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("W face {iw} does not contain M point {im}"),
        ));
    }
    let mut edges = Vec::with_capacity(2);
    for &iu in &face.iu {
        if u_edges[iu].im.contains(&im) {
            edges.push(iu);
        }
    }
    if edges.len() != 2 {
        let edge_rows = face
            .iu
            .iter()
            .map(|&iu| (iu, u_edges[iu].im))
            .collect::<Vec<_>>();
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "W face {iw} has {edges_len} incident U edges for M point {im}, expected 2; face.im={:?}, face.iu={:?}, edge.im={:?}",
                face.im,
                face.iu,
                edge_rows,
                edges_len = edges.len(),
            ),
        ));
    }
    Ok([edges[0], edges[1]])
}
