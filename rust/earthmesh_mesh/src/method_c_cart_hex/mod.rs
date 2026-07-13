use super::*;

impl MethodCDelaunayMesh {
    /// Build Method-C's local Cartesian hexagonal base grid used by
    /// `cart_hex.F90:cart_hex` for `MDOMAIN = 5`.
    pub fn from_cart_hex(nxp: usize, deltax_meters: f64) -> io::Result<Self> {
        if !deltax_meters.is_finite() || deltax_meters < METHOD_C_MIN_GRID_SPACING_METERS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Method-C cart_hex DELTAX must be at least {METHOD_C_MIN_GRID_SPACING_METERS} meters"
                ),
            ));
        }

        let mut nmd = 1;
        let mut nud = 1;
        let mut nwd = 1;
        let tab_width = nxp + 2;
        let tab_plane = tab_width * tab_width;
        let tab_len = 4 * tab_plane;
        let tab_idx = |i: usize, j: usize, ir: usize| ir * tab_plane + j * tab_width + i;
        let mut jm1 = vec![1usize; tab_len];
        let mut ju1 = vec![1usize; tab_len];
        let mut ju2 = vec![1usize; tab_len];
        let mut ju3 = vec![1usize; tab_len];
        let mut jw1 = vec![1usize; tab_len];
        let mut jw2 = vec![1usize; tab_len];

        for ir in 1..=3 {
            for j in 1..=nxp {
                for i in 1..=nxp + 1 {
                    let idx = tab_idx(i, j, ir);
                    jm1[idx] = nmd + 1;
                    ju1[idx] = nud + 1;
                    ju2[idx] = nud + 2;
                    ju3[idx] = nud + 3;
                    jw1[idx] = nwd + 1;
                    jw2[idx] = nwd + 2;
                    nmd += 1;
                    nud += 3;
                    nwd += 2;
                }
                jw1[tab_idx(0, j, ir)] = nwd + 1;
                nwd += 1;
            }

            for i in 1..=nxp + 1 {
                let idx = tab_idx(i, nxp + 1, ir);
                jm1[idx] = nmd + 1;
                ju1[idx] = nud + 1;
                jw2[tab_idx(i, 0, ir)] = nwd + 1;
                nmd += 1;
                nud += 1;
                nwd += 1;
            }
        }
        let jw0 = nwd + 1;
        nwd += 1;

        let zero = CartesianPoint::new(0.0, 0.0, 0.0);
        let mut m_points = vec![zero; nmd + 1];
        let mut u_edges = vec![IcosahedronUEdge::default(); nud + 1];
        let mut w_faces = vec![IcosahedronWFace::default(); nwd + 1];
        let mut m_prognostic = method_c_identity_prognostic_map(nmd);
        let mut u_prognostic = method_c_identity_prognostic_map(nud);
        let mut w_prognostic = method_c_identity_prognostic_map(nwd);
        for face in w_faces.iter_mut().take(nwd + 1).skip(2) {
            face.mrlw = 1;
            face.mrlw_orig = 1;
            face.ngr = 1;
        }

        let unit_dist = (4.0_f64 / 3.0).sqrt().sqrt() * deltax_meters;
        let xstart = -((nxp + 1) as f64) * 0.5 * unit_dist;
        let ystart = -((nxp as f64) + 1.0 / 3.0) * 0.5 * 3.0_f64.sqrt() * unit_dist;

        for ir in 1..=3 {
            let irm = if ir == 1 { 3 } else { ir - 1 };
            let irp = if ir == 3 { 1 } else { ir + 1 };
            let (rxx, rxy, ryx, ryy) = match ir {
                1 => (1.0, 0.0, 0.0, 1.0),
                2 => (-0.5, -0.5 * 3.0_f64.sqrt(), 0.5 * 3.0_f64.sqrt(), -0.5),
                _ => (-0.5, 0.5 * 3.0_f64.sqrt(), -0.5 * 3.0_f64.sqrt(), -0.5),
            };

            for j in 1..=nxp {
                for i in 1..=nxp + 1 {
                    let idx = tab_idx(i, j, ir);
                    let im1 = jm1[idx];
                    let xm = xstart + ((i - 1) as f64 - 0.5 * (j - 1) as f64) * unit_dist;
                    let ym = ystart + (j - 1) as f64 * 0.5 * 3.0_f64.sqrt() * unit_dist;
                    m_points[im1] =
                        CartesianPoint::new(rxx * xm + rxy * ym, ryx * xm + ryy * ym, 0.0);

                    let iu1 = ju1[idx];
                    let iu2 = ju2[idx];
                    let iu3 = ju3[idx];
                    let iw1 = jw1[idx];
                    let iw2 = jw2[idx];
                    let iw3 = jw2[tab_idx(i, j - 1, ir)];
                    let iw4 = jw1[tab_idx(i - 1, j, ir)];
                    let im3 = jm1[tab_idx(i, j + 1, ir)];
                    let iu5 = ju1[tab_idx(i, j + 1, ir)];

                    let (im2, im4, iu4) = if i <= nxp {
                        (
                            jm1[tab_idx(i + 1, j, ir)],
                            jm1[tab_idx(i + 1, j + 1, ir)],
                            ju3[tab_idx(i + 1, j, ir)],
                        )
                    } else {
                        (
                            jm1[tab_idx(j, nxp + 1, irp)],
                            jm1[tab_idx(j + 1, nxp + 1, irp)],
                            ju1[tab_idx(j, nxp + 1, irp)],
                        )
                    };

                    u_edges[iu1] = if ir == 1 {
                        IcosahedronUEdge {
                            im: [im1, im2],
                            iw: set_first_two([1; 6], iw3, iw1),
                            ..IcosahedronUEdge::default()
                        }
                    } else {
                        IcosahedronUEdge {
                            im: [im2, im1],
                            iw: set_first_two([1; 6], iw1, iw3),
                            ..IcosahedronUEdge::default()
                        }
                    };
                    u_edges[iu2] = if ir == 1 || ir == 3 {
                        IcosahedronUEdge {
                            im: [im1, im4],
                            iw: set_first_two([1; 6], iw1, iw2),
                            ..IcosahedronUEdge::default()
                        }
                    } else {
                        IcosahedronUEdge {
                            im: [im4, im1],
                            iw: set_first_two([1; 6], iw2, iw1),
                            ..IcosahedronUEdge::default()
                        }
                    };
                    u_edges[iu3] = if ir == 3 {
                        IcosahedronUEdge {
                            im: [im1, im3],
                            iw: set_first_two([1; 6], iw2, iw4),
                            ..IcosahedronUEdge::default()
                        }
                    } else {
                        IcosahedronUEdge {
                            im: [im3, im1],
                            iw: set_first_two([1; 6], iw4, iw2),
                            ..IcosahedronUEdge::default()
                        }
                    };

                    w_faces[iw1].npoly = 3;
                    w_faces[iw1].iu = [iu1, iu4, iu2];
                    w_faces[iw1].im = [im1, im2, im4];
                    w_faces[iw2].npoly = 3;
                    w_faces[iw2].iu = [iu2, iu5, iu3];
                    w_faces[iw2].im = [im1, im4, im3];

                    if i == 1 && j == 1 {
                        w_faces[iw3].iu[0] = iu1;
                        w_faces[iw4].iu[0] = iu3;
                        m_prognostic[im1] = jm1[tab_idx(2, 2, irp)];
                        u_prognostic[iu1] = if ir == 2 {
                            ju3[tab_idx(2, 1, irm)]
                        } else {
                            ju2[tab_idx(1, 1, irp)]
                        };
                        if ir == 3 {
                            u_prognostic[iu2] = ju3[tab_idx(2, 1, irp)];
                        }
                        u_prognostic[iu3] = ju1[tab_idx(2, 2, irp)];
                        w_prognostic[iw3] = jw2[tab_idx(2, 1, irm)];
                        w_prognostic[iw4] = jw1[tab_idx(2, 2, irp)];
                    } else if i == 1 {
                        w_faces[iw4].iu[0] = iu3;
                        m_prognostic[im1] = jm1[tab_idx(j + 1, 2, irp)];
                        if ir != 2 {
                            u_prognostic[iu1] = ju2[tab_idx(j, 1, irp)];
                        }
                        if ir == 3 {
                            u_prognostic[iu2] = ju3[tab_idx(j + 1, 1, irp)];
                        }
                        u_prognostic[iu3] = ju1[tab_idx(j + 1, 2, irp)];
                        w_prognostic[iw4] = jw1[tab_idx(j + 1, 2, irp)];
                    } else if j == 1 {
                        w_faces[iw3].iu[0] = iu1;
                        m_prognostic[im1] = jm1[tab_idx(2, i, irm)];
                        u_prognostic[iu1] = if i == nxp + 1 && ir == 2 {
                            ju1[tab_idx(1, nxp + 1, ir)]
                        } else if i == nxp + 1 {
                            ju2[tab_idx(nxp, 1, irp)]
                        } else {
                            ju3[tab_idx(2, i, irm)]
                        };
                        if ir == 3 {
                            u_prognostic[iu2] = ju1[tab_idx(1, i, irm)];
                        }
                        if ir != 1 {
                            u_prognostic[iu3] = ju2[tab_idx(1, i - 1, irm)];
                        }
                        w_prognostic[iw3] = if i == nxp + 1 {
                            jw2[tab_idx(nxp + 1, 1, irp)]
                        } else {
                            jw2[tab_idx(2, i, irm)]
                        };
                    }
                }
            }

            for i in 1..=nxp + 1 {
                let idx = tab_idx(i, nxp + 1, ir);
                let im1 = jm1[idx];
                let iu1 = ju1[idx];
                let iw3 = jw2[tab_idx(i, nxp, ir)];
                let xm = xstart + ((i - 1) as f64 - 0.5 * nxp as f64) * unit_dist;
                let ym = ystart + nxp as f64 * 0.5 * 3.0_f64.sqrt() * unit_dist;
                m_points[im1] = CartesianPoint::new(rxx * xm + rxy * ym, ryx * xm + ryy * ym, 0.0);

                let (im2, iw1) = if i <= nxp {
                    (
                        jm1[tab_idx(i + 1, nxp + 1, ir)],
                        jw1[tab_idx(nxp + 1, i, irm)],
                    )
                } else {
                    w_faces[jw0].iu[ir - 1] = iu1;
                    (jm1[tab_idx(i, nxp + 1, irp)], jw0)
                };
                u_edges[iu1] = if ir == 1 {
                    IcosahedronUEdge {
                        im: [im1, im2],
                        iw: set_first_two([1; 6], iw3, iw1),
                        ..IcosahedronUEdge::default()
                    }
                } else {
                    IcosahedronUEdge {
                        im: [im2, im1],
                        iw: set_first_two([1; 6], iw1, iw3),
                        ..IcosahedronUEdge::default()
                    }
                };
                if i == 1 {
                    m_prognostic[im1] = jm1[tab_idx(2, nxp + 1, irm)];
                    if ir != 2 {
                        u_prognostic[iu1] = ju2[tab_idx(nxp + 1, 1, irp)];
                    }
                }
            }
        }

        for edge in u_edges.iter_mut().take(nud + 1).skip(2) {
            edge.mrlu = 1;
        }

        let jw0_edges = w_faces[jw0].iu;
        let mut jw0_m = Vec::<usize>::new();
        for &iu in &jw0_edges {
            if iu <= 1 {
                continue;
            }
            for &im in &u_edges[iu].im {
                if im > 1 && !jw0_m.contains(&im) {
                    jw0_m.push(im);
                }
            }
        }
        if jw0_m.len() == 3 {
            w_faces[jw0].npoly = 3;
            w_faces[jw0].im = [jw0_m[0], jw0_m[1], jw0_m[2]];
        }
        fill_cart_hex_w_face_neighbors_from_edges(&u_edges, &mut w_faces, &w_prognostic)?;
        let mut connectivity = IcosahedronDiamondConnectivity { u_edges, w_faces };
        derive_icosahedron_u_neighbors_canonical(&mut connectivity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to derive Method-C cart_hex U-edge neighbors",
            )
        })?;
        let IcosahedronDiamondConnectivity { u_edges, w_faces } = connectivity;

        let m_neighbors =
            derive_cart_hex_m_neighbors_from_active_faces(nmd, &u_edges, &w_faces, &w_prognostic)?;

        Ok(Self {
            nmd,
            nud,
            nwd,
            impent: [1; 12],
            m_points,
            m_metadata: default_method_c_m_metadata(nmd),
            u_edges,
            w_faces,
            m_neighbors,
            m_prognostic,
            u_prognostic,
            w_prognostic,
            boundary_rows: Vec::new(),
        })
    }
}
