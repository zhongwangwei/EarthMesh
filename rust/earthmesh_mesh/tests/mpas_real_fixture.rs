use earthmesh_mesh::{
    get_area_unit_fortran_indexed, get_edge_connectivity_fortran_indexed, CartesianPoint,
    GetAreaUnitInput,
};

fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual} expected {expected} tolerance {tolerance}"
    );
}

fn assert_pair_unordered(actual: [usize; 2], expected: [usize; 2]) {
    assert!(
        actual == expected || actual == [expected[1], expected[0]],
        "actual {:?} expected unordered {:?}",
        actual,
        expected
    );
}

#[test]
fn get_area_matches_real_mpas_fixture_for_cell_and_vertex_areas() {
    let vertices = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(
            7.28576636279020584e-01,
            3.94202189084081667e-01,
            -5.60161333179706178e-01,
        ),
        CartesianPoint::new(
            7.29089419490325019e-01,
            4.00719366397330967e-01,
            -5.54844669958523107e-01,
        ),
        CartesianPoint::new(
            7.36188896032144324e-01,
            4.00774695889490717e-01,
            -5.45349018971941324e-01,
        ),
        CartesianPoint::new(
            7.42178152141667935e-01,
            3.91311946555944867e-01,
            -5.44101599856292029e-01,
        ),
        CartesianPoint::new(
            7.41810803379780781e-01,
            3.84620497530767880e-01,
            -5.49348527683681698e-01,
        ),
        CartesianPoint::new(
            7.34646002607540871e-01,
            3.84532863388955204e-01,
            -5.58954137498464232e-01,
        ),
        CartesianPoint::new(
            7.49232481704178443e-01,
            3.91386445663873650e-01,
            -5.34291435931737113e-01,
        ),
        CartesianPoint::new(
            7.54976113269692717e-01,
            3.82214412406558846e-01,
            -5.32844453232739657e-01,
        ),
        CartesianPoint::new(
            7.54750752169911387e-01,
            3.75285419751812566e-01,
            -5.38063338112400036e-01,
        ),
        CartesianPoint::new(
            7.47648067197642763e-01,
            3.75191702805974747e-01,
            -5.47953970476701691e-01,
        ),
        CartesianPoint::new(
            7.34098637548847632e-01,
            3.77846794725644697e-01,
            -5.64208286065068676e-01,
        ),
        CartesianPoint::new(
            7.47231481768795613e-01,
            3.68322448630421573e-01,
            -5.53157921834714905e-01,
        ),
        CartesianPoint::new(
            7.40012486515269186e-01,
            3.68211504993883110e-01,
            -5.62851496748145941e-01,
        ),
    ];
    let edge_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(
            7.38233806691818040e-01,
            3.84583489524014721e-01,
            -5.54171802100070221e-01,
        ),
        CartesianPoint::new(
            7.42004223465729251e-01,
            3.88024901112329290e-01,
            -5.46686755350618614e-01,
        ),
        CartesianPoint::new(
            7.44746817208285505e-01,
            3.79902741217004070e-01,
            -5.48658441540759045e-01,
        ),
    ];
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(
            7.35458075942348244e-01,
            3.92687985363309067e-01,
            -5.52175302492319720e-01,
        ),
        CartesianPoint::new(
            7.48480087939583982e-01,
            3.83325062928582150e-01,
            -5.41146425737758285e-01,
        ),
        CartesianPoint::new(
            7.40952417674857089e-01,
            3.76449237096908307e-01,
            -5.56125423471126701e-01,
        ),
    ];
    let cells_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
    ];
    let edges_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
    ];
    let cells_on_edge = vec![[0, 0], [0, 0], [2, 4], [2, 3], [4, 3]];
    let vertices_on_cell = vec![
        vec![],
        vec![],
        vec![2, 3, 4, 5, 6, 7],
        vec![6, 5, 8, 9, 10, 11],
        vec![12, 7, 6, 11, 13, 14],
    ];
    let n_edges_on_cell = vec![0, 0, 6, 6, 6];

    let output = get_area_unit_fortran_indexed(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cell_points,
        cells_on_vertex: &cells_on_vertex,
        edges_on_vertex: &edges_on_vertex,
        cells_on_edge: &cells_on_edge,
        vertices_on_cell: &vertices_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
    })
    .expect("valid compact MPAS GetArea fixture");

    approx_eq(output.area_triangle[6], 1.45653740047964190e-04, 1.0e-12);
    approx_eq(output.area_cell[2], 2.89334492920077725e-04, 1.0e-12);
    approx_eq(output.area_cell[3], 2.89565002952682569e-04, 1.0e-12);
    approx_eq(output.area_cell[4], 2.91760534643355750e-04, 1.0e-12);
}

#[test]
fn get_edge_connectivity_matches_real_mpas_fixture_for_vertex_ring() {
    // Compact reindexing of MPAS vertices 1000, 999, 1001, and 1127 plus
    // cells 438, 502, and 501 from MPASOUT_NXP0064_global.nc4.
    let triangle_neighbors = vec![
        [0, 0, 0],
        [0, 0, 0],
        [3, 4, 5],
        [2, 0, 0],
        [2, 0, 0],
        [2, 0, 0],
    ];
    let cells_on_vertex = vec![
        [0, 0, 0],
        [0, 0, 0],
        [2, 3, 4],
        [2, 4, 0],
        [2, 3, 0],
        [4, 3, 0],
    ];

    let output = get_edge_connectivity_fortran_indexed(&triangle_neighbors, &cells_on_vertex)
        .expect("valid compact MPAS GetEdge fixture");

    assert_eq!(output.vertices_on_edge[2], [2, 3]);
    assert_eq!(output.vertices_on_edge[3], [2, 4]);
    assert_eq!(output.vertices_on_edge[4], [2, 5]);
    assert_pair_unordered(output.cells_on_edge[2], [2, 4]);
    assert_pair_unordered(output.cells_on_edge[3], [2, 3]);
    assert_pair_unordered(output.cells_on_edge[4], [4, 3]);
    assert_eq!(output.edges_on_vertex[2], [2, 3, 4]);
    assert_eq!(output.edges_on_vertex[3][0], 2);
    assert_eq!(output.edges_on_vertex[4][0], 3);
    assert_eq!(output.edges_on_vertex[5][0], 4);
}
