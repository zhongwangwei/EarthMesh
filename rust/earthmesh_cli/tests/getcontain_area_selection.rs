use earthmesh_cli::{
    coordinate_types::LonLatPoint, getcontain_geometry::getcontain_is_in_area_ustr_one_based,
    getcontain_types::GetContainAreaBounds,
};

#[test]
fn getcontain_area_selection_matches_canonical_vertex_corner_and_skip_rules() {
    let bounds = GetContainAreaBounds {
        west: 0.0,
        east: 10.0,
        south: 0.0,
        north: 10.0,
    };
    let vertices = vec![
        LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        },
        // row 1 is skipped by num_vertex even though its vertices are inside.
        LonLatPoint { lon: 2.0, lat: 2.0 },
        LonLatPoint { lon: 3.0, lat: 2.0 },
        LonLatPoint { lon: 2.0, lat: 3.0 },
        // row 2 has a vertex strictly inside the selected area.
        LonLatPoint { lon: 5.0, lat: 5.0 },
        LonLatPoint {
            lon: 14.0,
            lat: 5.0,
        },
        LonLatPoint {
            lon: 5.0,
            lat: 14.0,
        },
        // row 3 encloses the selected area; selected by the bbox-corner pass.
        LonLatPoint {
            lon: -1.0,
            lat: -1.0,
        },
        LonLatPoint {
            lon: 11.0,
            lat: -1.0,
        },
        LonLatPoint {
            lon: 11.0,
            lat: 11.0,
        },
        LonLatPoint {
            lon: -1.0,
            lat: 11.0,
        },
        // row 4 also encloses a bbox corner, but spans >180 degrees longitude.
        LonLatPoint {
            lon: -100.0,
            lat: -1.0,
        },
        LonLatPoint {
            lon: 100.0,
            lat: -1.0,
        },
        LonLatPoint {
            lon: 100.0,
            lat: 1.0,
        },
        LonLatPoint {
            lon: -100.0,
            lat: 1.0,
        },
        // row 5 touches the west bound only; strict vertex pass must not select it.
        LonLatPoint { lon: 0.0, lat: 4.0 },
        LonLatPoint {
            lon: -2.0,
            lat: 4.0,
        },
        LonLatPoint { lon: 0.0, lat: 6.0 },
    ];
    let cell_to_vertices = vec![
        vec![0],
        vec![1, 2, 3],
        vec![4, 5, 6],
        vec![7, 8, 9, 10],
        vec![11, 12, 13, 14],
        vec![15, 16, 17],
    ];
    let n_edges = vec![0, 3, 3, 4, 4, 3];

    let selected =
        getcontain_is_in_area_ustr_one_based(bounds, &vertices, &cell_to_vertices, &n_edges, 1)
            .expect("calculate getcontain area mask");

    assert_eq!(selected, vec![0, 0, 1, 1, 0, 0]);
}

#[test]
fn getcontain_area_selection_rejects_missing_vertex_canonicals() {
    let err = getcontain_is_in_area_ustr_one_based(
        GetContainAreaBounds {
            west: 0.0,
            east: 1.0,
            south: 0.0,
            north: 1.0,
        },
        &[LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        }],
        &[vec![0], vec![2, 2, 2]],
        &[0, 3],
        0,
    )
    .expect_err("invalid vertex id must be rejected");

    assert!(err.to_string().contains("canonicals missing vertex 2"));
}

#[test]
fn getcontain_area_selection_rejects_zero_vertex_canonicals_in_active_cells() {
    let err = getcontain_is_in_area_ustr_one_based(
        GetContainAreaBounds {
            west: 0.0,
            east: 1.0,
            south: 0.0,
            north: 1.0,
        },
        &[LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        }],
        &[vec![0], vec![0, 0, 0]],
        &[0, 3],
        0,
    )
    .expect_err("active one-based vertex id zero must be rejected");

    assert!(err.to_string().contains("canonicals missing vertex 0"));
}
