use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MethodCMValenceWitness {
    pub(crate) m_point: usize,
    pub(crate) start_u_edge: usize,
    pub(crate) walked_u_edges: Vec<usize>,
}

/// Diagnostic counterpart to the canonical fail-fast ring builder. It walks
/// the same U-edge transitions but keeps scanning after an overfull M ring.
pub(crate) fn collect_icosahedron_m_valence_witnesses_canonical(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
    m_prognostic: Option<&[usize]>,
) -> io::Result<Vec<MethodCMValenceWitness>> {
    let mut completed_npoly = vec![0usize; nmd + 1];
    let mut invalid = vec![false; nmd + 1];
    let mut witnesses = Vec::new();

    for iu in 2..u_edges.len() {
        for j in 0..2 {
            let edge = u_edges[iu];
            let im = edge.im[j];
            let iw = edge.iw[j];
            if im >= completed_npoly.len() || iw >= w_faces.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} endpoint/face out of range: im={im}, iw={iw}"),
                ));
            }
            if invalid[im]
                || m_prognostic
                    .and_then(|map| map.get(im))
                    .copied()
                    .is_some_and(|partner| partner > 1 && partner != im)
                || (completed_npoly[im] != 0 && w_faces[iw].npoly >= 3)
            {
                continue;
            }

            let start_u_edge = iu;
            let mut current_u_edge = iu;
            let mut walked_u_edges = Vec::with_capacity(8);
            while current_u_edge > 1 {
                if walked_u_edges.contains(&current_u_edge) {
                    invalid[im] = true;
                    break;
                }
                let edge_now = *u_edges.get(current_u_edge).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("U edge {current_u_edge} is out of range in M point {im} ring"),
                    )
                })?;
                walked_u_edges.push(current_u_edge);
                if walked_u_edges.len() > 7 {
                    invalid[im] = true;
                    witnesses.push(MethodCMValenceWitness {
                        m_point: im,
                        start_u_edge,
                        walked_u_edges,
                    });
                    break;
                }

                current_u_edge = if edge_now.im[0] == im {
                    if edge_now.iw[1] > 1 {
                        edge_now.iu[2]
                    } else {
                        start_u_edge
                    }
                } else if edge_now.iw[0] > 1 {
                    edge_now.iu[1]
                } else {
                    start_u_edge
                };
                completed_npoly[im] = walked_u_edges.len();
                if current_u_edge == start_u_edge {
                    break;
                }
            }
        }
    }

    Ok(witnesses)
}

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

pub(crate) fn derive_icosahedron_m_neighbors_canonical_checked(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(nmd, u_edges, w_faces, None)
}

pub(crate) fn derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(
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
                if walk_trace.iter().any(|(walked_iu, ..)| *walked_iu == iunow) {
                    return Err(method_c_repairable_error(
                        MethodCRepairableKind::TransitionPatch,
                        Some(im),
                        format!(
                            "Method-C transition patch invalid: M point {im} revisits U edge {iunow} before returning to start U edge {start_iu}; trace {:?}",
                            walk_trace
                        ),
                    ));
                }
                npoly += 1;
                let edge_now = *u_edges.get(iunow).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("U edge {iunow} is out of range in M point {im} ring"),
                    )
                })?;
                walk_trace.push((iunow, edge_now.im, edge_now.iw, edge_now.iu));
                if npoly > 7 {
                    return Err(method_c_repairable_error(
                        MethodCRepairableKind::Valence,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valence_census_reports_every_overfull_ring() {
        let mut u_edges = vec![IcosahedronUEdge::default(); 18];
        let mut w_faces = vec![IcosahedronWFace::default(); 20];
        for face in w_faces.iter_mut().skip(2) {
            face.npoly = 3;
        }

        for (ring, center_m) in [(2..10, 2usize), (10..18, 3usize)] {
            let edges = ring.collect::<Vec<_>>();
            for (slot, &iu) in edges.iter().enumerate() {
                let next = edges[(slot + 1) % edges.len()];
                u_edges[iu].im = [center_m, center_m + 10 + slot];
                u_edges[iu].iw[0..2].copy_from_slice(&[2 + slot, 3 + slot]);
                u_edges[iu].iu[2] = next;
            }
        }

        let witnesses =
            collect_icosahedron_m_valence_witnesses_canonical(20, &u_edges, &w_faces, None)
                .expect("valence census");
        let first_failure = derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(
            20, &u_edges, &w_faces, None,
        )
        .expect_err("overfull ring must fail canonical neighbor derivation");
        assert_eq!(
            method_c_repairable_payload(&first_failure).and_then(|failure| failure.m_point),
            witnesses.first().map(|witness| witness.m_point)
        );
        assert_eq!(
            witnesses
                .iter()
                .map(|witness| witness.m_point)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(witnesses
            .iter()
            .all(|witness| witness.walked_u_edges.len() == 8));
    }

    #[test]
    fn valence_census_accepts_a_valid_canonical_mesh() {
        let mesh =
            MethodCDelaunayMesh::from_icosahedron(9, 0, 1.0, 0.25, 0).expect("canonical mesh");
        assert!(collect_icosahedron_m_valence_witnesses_canonical(
            mesh.nmd,
            &mesh.u_edges,
            &mesh.w_faces,
            None,
        )
        .expect("valence census")
        .is_empty());
    }

    #[test]
    fn repeated_non_start_u_edge_is_a_transition_patch_not_valence() {
        let mut u_edges = vec![IcosahedronUEdge::default(); 6];
        let mut w_faces = vec![IcosahedronWFace::default(); 8];
        for face in w_faces.iter_mut().skip(2) {
            face.npoly = 3;
        }

        for (iu, next) in [(2, 3), (3, 4), (4, 3)] {
            u_edges[iu].im = [2, iu + 2];
            u_edges[iu].iw[0..2].copy_from_slice(&[iu, iu + 1]);
            u_edges[iu].iu[2] = next;
        }

        let error = derive_icosahedron_m_neighbors_canonical_checked_with_prognostic(
            8, &u_edges, &w_faces, None,
        )
        .expect_err("a ring that revisits a non-start U edge must fail");
        assert_eq!(
            method_c_repairable_payload(&error).map(|failure| failure.kind),
            Some(MethodCRepairableKind::TransitionPatch)
        );
        assert!(
            collect_icosahedron_m_valence_witnesses_canonical(8, &u_edges, &w_faces, None)
                .expect("valence census")
                .is_empty(),
            "a cyclic transition patch is not a genuine overfull M ring"
        );
    }
}
