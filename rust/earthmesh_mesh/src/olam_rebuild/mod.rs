use super::*;
use crate::olam_rebuild_seeds::{assign_olam_triangle_seed_w_ids, insert_or_attach_olam_edge};

pub(crate) fn olam_mesh_from_triangle_seeds(
    nmd: usize,
    impent: [usize; 12],
    m_points: Vec<CartesianPoint>,
    face_seeds: &[OlamTriangleSeed],
) -> io::Result<OlamDelaunayMesh> {
    olam_mesh_from_triangle_seeds_with_boundary_rows(nmd, impent, m_points, face_seeds, Vec::new())
}

fn olam_mesh_from_triangle_seeds_with_boundary_rows(
    nmd: usize,
    impent: [usize; 12],
    m_points: Vec<CartesianPoint>,
    face_seeds: &[OlamTriangleSeed],
    boundary_rows: Vec<usize>,
) -> io::Result<OlamDelaunayMesh> {
    require_olam_len("m_points", m_points.len(), nmd + 1)?;

    let face_iw = assign_olam_triangle_seed_w_ids(face_seeds)?;
    let nwd = face_iw.iter().copied().max().unwrap_or(1);
    let mut u_edges = vec![IcosahedronUEdge::default(); 2];
    let mut w_faces = vec![IcosahedronWFace::default(); nwd + 1];
    let mut edge_by_key = BTreeMap::<(usize, usize), usize>::new();
    let reserved_u_ids = face_seeds
        .iter()
        .flat_map(|seed| seed.target_iu)
        .filter(|&iu| iu > 1)
        .collect::<BTreeSet<_>>();
    let mut occupied_u_ids = BTreeSet::<usize>::new();
    let mut next_auto_iu = 2usize;

    for (&iw, seed) in face_iw.iter().zip(face_seeds.iter()) {
        require_unique_active_triplet("OLAM W seed M vertices", iw, seed.im, nmd)?;

        let mut face = IcosahedronWFace {
            npoly: 3,
            im: seed.im,
            mrlw: seed.mrlw.max(1),
            mrlw_orig: seed.mrlw_orig.max(1),
            ngr: seed.ngr.max(1),
            mrow: seed.mrow,
            ..IcosahedronWFace::default()
        };

        let directed_sides = [
            (seed.im[2], seed.im[1]),
            (seed.im[0], seed.im[2]),
            (seed.im[1], seed.im[0]),
        ];
        for (slot, (from, to)) in directed_sides.into_iter().enumerate() {
            let iu = insert_or_attach_olam_edge(
                &mut u_edges,
                &mut edge_by_key,
                &reserved_u_ids,
                &mut occupied_u_ids,
                &mut next_auto_iu,
                iw,
                from,
                to,
                face.iu[slot],
                seed.target_iu[slot],
            )?;
            face.iu[slot] = iu;
        }

        w_faces[iw] = face;
    }

    let nud = u_edges.len() - 1;
    for iu in 2..=nud {
        let edge = u_edges[iu];
        if edge.iw[0] <= 1 || edge.iw[1] <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("U edge {iu} is not shared by two W faces"),
            ));
        }
    }

    let mut connectivity = IcosahedronDiamondConnectivity { u_edges, w_faces };
    fill_olam_w_face_neighbors_from_edges(&mut connectivity.u_edges, &mut connectivity.w_faces)?;
    derive_icosahedron_u_neighbors_fortran(&mut connectivity).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to derive OLAM U-edge neighbors from rebuilt triangle mesh",
        )
    })?;
    let m_neighbors =
        derive_olam_m_neighbors_from_incidence(nmd, &connectivity.u_edges, &connectivity.w_faces)?;
    let m_metadata = derive_olam_m_metadata_from_w_faces(nmd, &connectivity.w_faces)?;

    let mesh = OlamDelaunayMesh {
        nmd,
        nud,
        nwd,
        impent,
        m_points,
        m_metadata,
        u_edges: connectivity.u_edges,
        w_faces: connectivity.w_faces,
        m_neighbors,
        m_prognostic: olam_identity_prognostic_map(nmd),
        u_prognostic: olam_identity_prognostic_map(nud),
        w_prognostic: olam_identity_prognostic_map(nwd),
        boundary_rows,
    };
    mesh.validate_topology()?;
    Ok(mesh)
}
