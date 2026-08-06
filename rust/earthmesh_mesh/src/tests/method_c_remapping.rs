use super::*;

#[test]
fn method_c_remaps_impent_through_canonical_imnew_table() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &method_c_m_neighbors)
        .expect("Method-C closure");

    let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
    for iw in 2..=mesh.nwd {
        if selected[iw] {
            nest_wd[iw].iw[2] = 1;
        }
    }
    let perimeter = mesh
        .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
        .expect("Method-C perimeter");
    for triple in perimeter.chunks_exact(3) {
        let center = triple[1];
        let edge = mesh.u_edges[center.iu];
        let suppressed_w = if center.im == edge.im[0] {
            edge.iw[1]
        } else {
            edge.iw[0]
        };
        nest_wd[suppressed_w].iw[2] = -1;
    }

    let mut imnew = vec![1usize; mesh.nmd + 1];
    let mut iudiv = vec![false; mesh.nud + 1];
    let mut imnext = 2usize;
    imnew[1] = 1;
    for im in 2..=mesh.nmd {
        imnew[im] = imnext;
        for &iu in method_c_m_neighbors[im]
            .iu
            .iter()
            .take(method_c_m_neighbors[im].npoly)
        {
            if iudiv[iu] {
                continue;
            }
            iudiv[iu] = true;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
            {
                imnext += 1;
            }
        }
        imnext += 1;
    }

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let expected_impent = mesh.impent.map(|im| imnew[im]);

    assert_eq!(
        refined.impent, expected_impent,
        "Canonical spawn_nest remaps impent through imnew after Method-C table allocation"
    );
}

#[test]
fn method_c_remaps_prognostic_partners_through_canonical_tables() {
    let mut mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let mut selected = mesh
        .selected_region_faces(&region, 1, false)
        .expect("selected Method-C faces");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &method_c_m_neighbors)
        .expect("Method-C closure");

    let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
    for iw in 2..=mesh.nwd {
        if selected[iw] {
            nest_wd[iw].iw[2] = 1;
        }
    }
    let perimeter = mesh
        .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
        .expect("Method-C perimeter");
    for triple in perimeter.chunks_exact(3) {
        let center = triple[1];
        let edge = mesh.u_edges[center.iu];
        let suppressed_w = if center.im == edge.im[0] {
            edge.iw[1]
        } else {
            edge.iw[0]
        };
        nest_wd[suppressed_w].iw[2] = -1;
    }

    let mut iwnew = vec![1usize; mesh.nwd + 1];
    let mut iwnext = 2usize;
    iwnew[1] = 1;
    for iw in 2..=mesh.nwd {
        iwnew[iw] = iwnext;
        if nest_wd[iw].is_subdivided() {
            iwnext += 3;
        }
        iwnext += 1;
    }

    let mut iunew = vec![1usize; mesh.nud + 1];
    let mut iwdiv = vec![false; mesh.nwd + 1];
    let mut iunext = 2usize;
    iunew[1] = 1;
    for iu in 2..=mesh.nud {
        iunew[iu] = iunext;
        let edge = mesh.u_edges[iu];
        let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
        if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
            && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
        {
            iunext += 1;
        }
        for &iw in &edge.iw[0..2] {
            if !iwdiv[iw] {
                iwdiv[iw] = true;
                if nest_wd[iw].is_subdivided() {
                    iunext += 3;
                }
            }
        }
        iunext += 1;
    }

    let mut imnew = vec![1usize; mesh.nmd + 1];
    let mut iudiv = vec![false; mesh.nud + 1];
    let mut imnext = 2usize;
    imnew[1] = 1;
    for im in 2..=mesh.nmd {
        imnew[im] = imnext;
        for &iu in method_c_m_neighbors[im]
            .iu
            .iter()
            .take(method_c_m_neighbors[im].npoly)
        {
            if iudiv[iu] {
                continue;
            }
            iudiv[iu] = true;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
            {
                imnext += 1;
            }
        }
        imnext += 1;
    }

    let m_pair = (mesh.impent[0], mesh.impent[1]);
    let u_pair = (2usize, 3usize);
    let w_pair = (2usize, 3usize);
    mesh.m_prognostic[m_pair.0] = m_pair.1;
    mesh.u_prognostic[u_pair.0] = u_pair.1;
    mesh.w_prognostic[w_pair.0] = w_pair.1;

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");

    assert_eq!(
        refined.m_prognostic[imnew[m_pair.0]], imnew[m_pair.1],
        "Canonical Method-C remaps M prognostic partner through imnew"
    );
    assert_eq!(
        refined.u_prognostic[iunew[u_pair.0]], iunew[u_pair.1],
        "Canonical Method-C remaps U prognostic partner through iunew"
    );
    assert_eq!(
        refined.w_prognostic[iwnew[w_pair.0]], iwnew[w_pair.1],
        "Canonical Method-C remaps W prognostic partner through iwnew"
    );
}
