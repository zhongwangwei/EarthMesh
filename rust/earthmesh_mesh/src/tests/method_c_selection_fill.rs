use super::super::*;

#[test]
fn method_c_selected_faces_close_sharp_concavity_around_m_point() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let point_id = (2..=mesh.nmd)
        .find(|id| !mesh.impent.contains(id) && mesh.m_neighbors[*id].npoly == 6)
        .expect("six-sided non-pentagon M point");
    let neighbors = mesh.m_neighbors[point_id];
    let missing_face = neighbors.iw[neighbors.npoly - 1];
    let mut selected = vec![false; mesh.nwd + 1];
    for &iw in neighbors.iw.iter().take(neighbors.npoly - 1) {
        selected[iw] = true;
    }

    mesh.close_method_c_selected_face_concavities(&mut selected)
        .expect("close sharp concavity");

    assert!(
        selected[missing_face],
        "Method-C sharp-concavity fill should add the only missing W face around M point {point_id}"
    );
}

#[test]
fn method_c_concavity_fill_keeps_canonical_npoly_minus_one_threshold_at_pentagons() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let point_id = mesh.impent[0];
    let neighbors = mesh.m_neighbors[point_id];
    assert_eq!(
        neighbors.npoly, 5,
        "test point should be an Method-C pentagon"
    );

    let mut too_sparse = vec![false; mesh.nwd + 1];
    for &iw in neighbors.iw.iter().take(neighbors.npoly - 2) {
        too_sparse[iw] = true;
    }
    mesh.close_method_c_selected_face_concavities(&mut too_sparse)
        .expect("close sparse pentagon case");
    assert_eq!(
        neighbors
            .iw
            .iter()
            .take(neighbors.npoly)
            .filter(|&&iw| too_sparse[iw])
            .count(),
        neighbors.npoly - 2,
        "Canonical skips concavity fill while nw < npoly - 1"
    );

    let mut one_missing = vec![false; mesh.nwd + 1];
    for &iw in neighbors.iw.iter().take(neighbors.npoly - 1) {
        one_missing[iw] = true;
    }
    mesh.close_method_c_selected_face_concavities(&mut one_missing)
        .expect("close one-missing pentagon case");
    assert!(
        neighbors
            .iw
            .iter()
            .take(neighbors.npoly)
            .all(|&iw| one_missing[iw]),
        "Canonical fills pentagon concavities only once nw reaches npoly - 1"
    );
}

#[test]
fn method_c_fill_rad3_marks_all_current_pentagon_faces_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
    let pentagon = mesh.impent[0];
    let neighbors = method_c_m_neighbors[pentagon];
    assert_eq!(neighbors.npoly, 5, "test requires an Method-C pentagon");

    let mut selected = vec![false; mesh.nwd + 1];
    mesh.mark_fill_rad3_faces_with_neighbors(pentagon, &mut selected, &method_c_m_neighbors)
        .expect("Canonical fill_rad3 around a pentagon");

    let missed = neighbors
        .iw
        .iter()
        .take(neighbors.npoly)
        .copied()
        .filter(|&iw| !selected[iw])
        .collect::<Vec<_>>();
    assert!(
            missed.is_empty(),
            "Canonical fill_rad3 loops over current M point npoly, not a hard-coded hexagon width; missed W faces: {missed:?}"
        );
}

#[test]
fn method_c_fill_rad3_marks_six_neighbors_of_three_distant_m_points_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let method_c_m_neighbors = mesh
        .derive_icosahedron_m_neighbors_canonical()
        .expect("Method-C M neighbors");
    let im = (2..=mesh.nmd)
        .find(|&candidate| method_c_m_neighbors[candidate].npoly == 6)
        .expect("ordinary hexagonal M point");
    let immediate = method_c_m_neighbors[im]
        .iw
        .iter()
        .take(method_c_m_neighbors[im].npoly)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut expected_far_w = std::collections::BTreeSet::new();

    for &iw in &immediate {
        let face = mesh.w_faces[iw];
        let (imx, iwx, iwy) = if im == face.im[0] {
            (face.im[1], face.iw[3], face.iw[4])
        } else if im == face.im[1] {
            (face.im[2], face.iw[5], face.iw[6])
        } else {
            (face.im[0], face.iw[7], face.iw[8])
        };
        let (im1, im2) =
            face_following_two_vertices(mesh.w_faces[iwx], imx, iwx).expect("Canonical im1/im2");
        let im3 = face_following_vertex(mesh.w_faces[iwy], im2, iwy).expect("Canonical im3");
        for far_im in [im1, im2, im3] {
            for &far_iw in method_c_m_neighbors[far_im].iw.iter().take(6) {
                expected_far_w.insert(far_iw);
            }
        }
    }

    let mut selected = vec![false; mesh.nwd + 1];
    mesh.mark_fill_rad3_faces_with_neighbors(im, &mut selected, &method_c_m_neighbors)
        .expect("Canonical fill_rad3 around a hexagon");
    let missed = expected_far_w
        .iter()
        .copied()
        .filter(|&iw| !selected[iw])
        .collect::<Vec<_>>();

    assert!(
            missed.is_empty(),
            "Canonical fill_rad3 marks all six W neighbors of each im1/im2/im3 distant M point; missed W faces: {missed:?}"
        );
    assert!(
        expected_far_w.iter().any(|iw| !immediate.contains(iw)),
        "test must cover fill_rad3's distant M-point expansion beyond the immediate ring"
    );
}

#[test]
fn method_c_cart_hex_initializes_m_metadata_like_canonical() {
    let mesh = MethodCMesh::from_cart_hex(2, 1000.0).expect("cart_hex Method-C mesh");

    for im in 2..=mesh.nmd {
        assert_eq!(mesh.m_metadata[im].mrlm, 1);
        assert_eq!(mesh.m_metadata[im].mrlm_orig, 1);
        assert_eq!(mesh.m_metadata[im].ngr, 1);
    }
}

#[test]
fn method_c_skips_cart_hex_periodic_copy_faces_like_canonical() {
    let mesh = MethodCMesh::from_cart_hex(5, 1000.0).expect("cart_hex Method-C mesh");
    let ghost_iw = (2..=mesh.nwd)
        .find(|&iw| mesh.w_prognostic[iw] > 1 && mesh.w_prognostic[iw] != iw)
        .expect("Canonical cart_hex periodic W copy");
    let partner_iw = mesh.w_prognostic[ghost_iw];

    assert!(
        !mesh.method_c_w_face_is_active(ghost_iw),
        "Canonical Method-C must ignore cart_hex periodic W copies as active fill_rad3 faces"
    );
    assert!(
        mesh.method_c_w_face_is_active(partner_iw),
        "Canonical Method-C should keep the prognostic owner W face active"
    );

    let face_with_copy_m = (2..=mesh.nwd)
        .find(|&iw| {
            mesh.w_faces[iw]
                .im
                .iter()
                .any(|&im| mesh.m_prognostic[im] > 1 && mesh.m_prognostic[im] != im)
        })
        .expect("Canonical cart_hex W face containing a periodic M copy");
    assert!(
        !mesh.method_c_w_face_is_active(face_with_copy_m),
        "Canonical Method-C must ignore W faces that contain cart_hex periodic M copies"
    );
}

#[test]
fn method_c_fill_rad3_skips_cart_hex_periodic_copy_faces_like_canonical() {
    let mesh = MethodCMesh::from_cart_hex(5, 1000.0).expect("cart_hex Method-C mesh");
    let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
    let exposed_periodic_copies = (2..=mesh.nmd)
        .flat_map(|im| {
            method_c_m_neighbors[im]
                .iw
                .iter()
                .take(method_c_m_neighbors[im].npoly)
                .copied()
        })
        .filter(|&iw| mesh.w_prognostic[iw] > 1 && mesh.w_prognostic[iw] != iw)
        .collect::<Vec<_>>();
    assert!(
            exposed_periodic_copies.is_empty(),
            "Canonical Method-C M-neighbor rings must not expose cart_hex periodic-copy W faces to fill_rad3: {exposed_periodic_copies:?}"
        );
}

#[test]
fn method_c_selected_regions_skip_cart_hex_periodic_copy_faces_like_canonical() {
    let mesh = MethodCMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex Method-C mesh");
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(10_200_000.0, -310_000.0),
        radius_meters: 500_000.0,
        level: 1,
    };

    let selected = mesh
        .selected_regions_faces(&[region], 1, true)
        .expect("Canonical Method-C cart_hex region selection");
    let selected_periodic_copies = (2..=mesh.nwd)
        .filter(|&iw| selected[iw] && !mesh.method_c_w_face_is_active(iw))
        .collect::<Vec<_>>();

    assert!(
            selected_periodic_copies.is_empty(),
            "Canonical Method-C region selection must not include cart_hex periodic-copy W faces: {selected_periodic_copies:?}"
        );
}
