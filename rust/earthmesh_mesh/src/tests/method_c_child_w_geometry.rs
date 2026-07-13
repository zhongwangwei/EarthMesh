use super::*;

#[test]
fn method_c_full_subdivision_child_w_vertices_match_canonical_geometry() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let radius = active_mesh_radius(&mesh).expect("active mesh radius");
    let region = MethodCRefinementRegion::Circle {
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
    let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
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
                expected_midpoint_m[iu] = imnext;
            }
        }
        imnext += 1;
    }

    let mut expected_parent_w = vec![1usize; mesh.nwd + 1];
    let mut expected_child_w = vec![[1usize; 3]; mesh.nwd + 1];
    let mut iwnext = 2usize;
    for iw in 2..=mesh.nwd {
        expected_parent_w[iw] = iwnext;
        if nest_wd[iw].is_subdivided() {
            iwnext += 1;
            expected_child_w[iw][0] = iwnext;
            iwnext += 1;
            expected_child_w[iw][1] = iwnext;
            iwnext += 1;
            expected_child_w[iw][2] = iwnext;
        }
        iwnext += 1;
    }

    let refined = mesh
        .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
        .expect("Method-C pass");
    let mut checked = 0usize;
    for iw in 2..=mesh.nwd {
        if !nest_wd[iw].is_subdivided() {
            continue;
        }
        if !mesh.w_faces[iw]
            .iw
            .iter()
            .take(3)
            .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
        {
            continue;
        }

        let original_vertices = mesh.w_faces[iw].im.map(|im| imnew[im]);
        let midpoint_vertices = mesh.w_faces[iw].iu.map(|iu| expected_midpoint_m[iu]);
        let parent_w = expected_parent_w[iw];
        let mut actual_parent_vertices = refined.w_faces[parent_w].im;
        actual_parent_vertices.sort_unstable();
        let mut expected_parent_vertices = midpoint_vertices;
        expected_parent_vertices.sort_unstable();
        assert_eq!(
                actual_parent_vertices,
                expected_parent_vertices,
                "Canonical remapped parent W {parent_w} for old W {iw} should be the central split-midpoint triangle"
            );
        for &vertex in &refined.w_faces[parent_w].im {
            let old_iu = mesh.w_faces[iw]
                .iu
                .into_iter()
                .find(|&iu| expected_midpoint_m[iu] == vertex)
                .expect("central parent midpoint vertex should map to old U edge");
            let edge = mesh.u_edges[old_iu];
            let linear_midpoint = weighted_point(
                mesh.m_points[edge.im[0]],
                1.0,
                mesh.m_points[edge.im[1]],
                1.0,
            )
            .expect("Canonical midpoint average");
            let expected = normalize_cartesian_to_radius(linear_midpoint, radius)
                .expect("Canonical final radius projection");
            let actual = refined.m_points[vertex];
            let delta = magnitude(CartesianPoint::new(
                actual.x - expected.x,
                actual.y - expected.y,
                actual.z - expected.z,
            ));
            assert!(
                    delta < 1.0e-6,
                    "Canonical remapped parent W {parent_w} midpoint vertex {vertex} should be edge-average projected to radius; delta={delta}"
                );
        }
        for child_w in expected_child_w[iw] {
            let child_vertices = refined.w_faces[child_w].im;
            let original_count = child_vertices
                .iter()
                .filter(|vertex| original_vertices.contains(vertex))
                .count();
            let midpoint_count = child_vertices
                .iter()
                .filter(|vertex| midpoint_vertices.contains(vertex))
                .count();
            assert_eq!(
                original_count, 1,
                "Canonical child W {child_w} for old W {iw} should keep exactly one old M vertex"
            );
            assert_eq!(
                    midpoint_count, 2,
                    "Canonical child W {child_w} for old W {iw} should use exactly two split-U midpoint M vertices"
                );
            for &vertex in &child_vertices {
                if original_vertices.contains(&vertex) {
                    continue;
                }
                let old_iu = mesh.w_faces[iw]
                    .iu
                    .into_iter()
                    .find(|&iu| expected_midpoint_m[iu] == vertex)
                    .expect("child midpoint vertex should map to old U edge");
                let edge = mesh.u_edges[old_iu];
                let linear_midpoint = weighted_point(
                    mesh.m_points[edge.im[0]],
                    1.0,
                    mesh.m_points[edge.im[1]],
                    1.0,
                )
                .expect("Canonical midpoint average");
                let expected = normalize_cartesian_to_radius(linear_midpoint, radius)
                    .expect("Canonical final radius projection");
                let actual = refined.m_points[vertex];
                let delta = magnitude(CartesianPoint::new(
                    actual.x - expected.x,
                    actual.y - expected.y,
                    actual.z - expected.z,
                ));
                assert!(
                        delta < 1.0e-6,
                        "Canonical child W {child_w} midpoint vertex {vertex} should be edge-average projected to radius; delta={delta}"
                    );
            }
            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "test should exercise full-interior Method-C child W face geometry"
    );
}
