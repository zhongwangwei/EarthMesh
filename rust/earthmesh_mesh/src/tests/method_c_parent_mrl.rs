use super::*;

#[test]
fn method_c_selected_faces_use_current_parent_mrl_inside_existing_nest() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let first_region = RefinementRegion::Circle {
        center: LonLatDegrees::new(-120.0, 0.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let first = mesh
        .spawn_nest(&[first_region], 1)
        .expect("first Method-C nest");
    let nested_point = (2..=first.nmd)
        .find(|&im| {
            let neighbors = first.m_neighbors[im];
            first.m_metadata[im].mrlm == 2
                && neighbors.npoly == 6
                && neighbors
                    .iu
                    .iter()
                    .take(neighbors.npoly)
                    .all(|&iu| first.u_edges[iu].mrlu == 2)
        })
        .expect("first nest should create an interior level-2 M point");
    let region = RefinementRegion::Circle {
        center: xyz_to_lonlat_degrees(first.m_points[nested_point]),
        radius_meters: 1.0,
        level: 1,
    };

    let selected = first
        .selected_region_faces(&region, 1, false)
        .expect("selected inner Method-C faces");
    let selected_levels = selected
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(iw, selected)| selected.then_some(first.w_faces[iw].mrlw))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        selected_levels,
        BTreeSet::from([2]),
        "Canonical derives mrlo from the current starting M point, not from the pass counter"
    );
}

#[test]
fn method_c_selected_faces_parent_halo_keeps_current_parent_mrl() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let first_region = RefinementRegion::Circle {
        center: LonLatDegrees::new(-120.0, 0.0),
        radius_meters: 500_000.0,
        level: 1,
    };
    let first = mesh
        .spawn_nest(&[first_region], 1)
        .expect("first Method-C nest");
    let nested_point = (2..=first.nmd)
        .find(|&im| {
            let neighbors = first.m_neighbors[im];
            first.m_metadata[im].mrlm == 2
                && neighbors.npoly == 6
                && neighbors
                    .iu
                    .iter()
                    .take(neighbors.npoly)
                    .all(|&iu| first.u_edges[iu].mrlu == 2)
        })
        .expect("first nest should create an interior level-2 M point");
    let region = RefinementRegion::Circle {
        center: xyz_to_lonlat_degrees(first.m_points[nested_point]),
        radius_meters: 1.0,
        level: 2,
    };

    let selected = first
        .selected_region_faces(&region, 1, false)
        .expect("selected inner Method-C faces with parent halo");
    let selected_levels = selected
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(iw, selected)| selected.then_some(first.w_faces[iw].mrlw))
        .collect::<BTreeSet<_>>();

    assert_eq!(
            selected_levels,
            BTreeSet::from([2]),
            "Canonical Method-C expands one current parent MRL at a time; selected levels were {selected_levels:?}"
        );
}
