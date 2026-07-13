use earthmesh_cli::{
    coordinate_types::LonLatPoint,
    getcontain_geometry::getcontain_containment_matrix_flat_one_based,
    getcontain_geometry::getcontain_containment_matrix_one_based,
    getcontain_types::GetContainMeshKind,
};

fn square_vertices() -> Vec<LonLatPoint> {
    vec![
        LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        },
        LonLatPoint { lon: 0.0, lat: 0.0 },
        LonLatPoint { lon: 3.0, lat: 0.0 },
        LonLatPoint { lon: 3.0, lat: 3.0 },
        LonLatPoint { lon: 0.0, lat: 3.0 },
    ]
}

fn one_cell_connectivity() -> (Vec<Vec<i32>>, Vec<i32>, Vec<i32>) {
    (vec![vec![0], vec![1, 2, 3, 4]], vec![0, 4], vec![0, 1])
}

fn source_grid() -> (Vec<f64>, Vec<f64>, Vec<Vec<i32>>, Vec<Vec<i32>>) {
    let lon_i = vec![f64::NAN, 0.5, 1.5, 2.5, 3.5];
    let lat_i = vec![f64::NAN, 0.5, 1.5, 2.5, 3.5];
    let mut is_in_area_grid = vec![vec![0; 5]; 5];
    let mut seaorland = vec![vec![0; 5]; 5];
    for i in 1..=3 {
        for j in 1..=3 {
            is_in_area_grid[i][j] = 1;
        }
    }
    is_in_area_grid[3][3] = 0;
    seaorland[1][1] = 1;
    seaorland[2][1] = 1;
    seaorland[1][2] = 1;
    (lon_i, lat_i, is_in_area_grid, seaorland)
}

#[test]
fn flat_containment_matches_compatibility_contain_mesh_without_distortion() {
    let (cell_to_vertices, n_edges, is_in_area_ustr) = one_cell_connectivity();
    let (lon_i, lat_i, is_in_area_grid, seaorland) = source_grid();

    let compatibility = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Ocean,
        &square_vertices(),
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate compatibility ocean containment");

    let flat = getcontain_containment_matrix_flat_one_based(
        GetContainMeshKind::Ocean,
        &square_vertices(),
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate flat ocean containment");

    assert_eq!(flat.ustr_id_width, 3);
    assert_eq!(flat.ustr_ii_width, 2);
    assert_eq!(
        flat.to_contain_mesh().expect("flat to compatibility"),
        compatibility
    );
}

#[test]
fn land_containment_keeps_only_land_pixels_and_canonical_offsets() {
    let (cell_to_vertices, n_edges, is_in_area_ustr) = one_cell_connectivity();
    let (lon_i, lat_i, is_in_area_grid, seaorland) = source_grid();

    let contain = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Land,
        &square_vertices(),
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate land containment");

    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![3, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![1, 1], vec![1, 2], vec![2, 1]]);
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
}

#[test]
fn ocean_containment_keeps_ocean_pixels_and_records_total_selected_pixels() {
    let (cell_to_vertices, n_edges, is_in_area_ustr) = one_cell_connectivity();
    let (lon_i, lat_i, is_in_area_grid, seaorland) = source_grid();

    let contain = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Ocean,
        &square_vertices(),
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate ocean containment");

    assert_eq!(contain.ustr_id, vec![vec![0, 0, 0], vec![5, 1, 8]]);
    assert_eq!(
        contain.ustr_ii,
        vec![vec![1, 3], vec![2, 2], vec![2, 3], vec![3, 1], vec![3, 2]]
    );
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
}

#[test]
fn atmos_containment_keeps_all_pixels_and_flags_land_pixels() {
    let (cell_to_vertices, n_edges, is_in_area_ustr) = one_cell_connectivity();
    let (lon_i, lat_i, is_in_area_grid, seaorland) = source_grid();

    let contain = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Atmos,
        &square_vertices(),
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate atmos containment");

    assert_eq!(contain.ustr_id, vec![vec![0, 0], vec![8, 1]]);
    assert_eq!(
        contain.ustr_ii,
        vec![
            vec![1, 1, 1],
            vec![1, 2, 1],
            vec![1, 3, 0],
            vec![2, 1, 1],
            vec![2, 2, 0],
            vec![2, 3, 0],
            vec![3, 1, 0],
            vec![3, 2, 0],
        ]
    );
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
}

#[test]
fn dateline_containment_shifts_test_points_and_restores_source_indices() {
    let vertices = vec![
        LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        },
        LonLatPoint {
            lon: 160.0,
            lat: 0.0,
        },
        LonLatPoint {
            lon: -160.0,
            lat: 0.0,
        },
        LonLatPoint {
            lon: -160.0,
            lat: 10.0,
        },
        LonLatPoint {
            lon: 160.0,
            lat: 10.0,
        },
    ];
    let cell_to_vertices = vec![vec![0], vec![1, 2, 3, 4]];
    let n_edges = vec![0, 4];
    let is_in_area_ustr = vec![0, 1];
    let lon_i = vec![
        f64::NAN,
        -165.0,
        -135.0,
        -105.0,
        -75.0,
        -45.0,
        -15.0,
        15.0,
        45.0,
        75.0,
        105.0,
        135.0,
        165.0,
    ];
    let lat_i = vec![f64::NAN, 15.0, 5.0, -5.0];
    let mut is_in_area_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    let seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    is_in_area_grid[12][2] = 1;
    is_in_area_grid[1][2] = 1;

    let contain = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Ocean,
        &vertices,
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate dateline containment");

    assert_eq!(contain.ustr_id, vec![vec![0, 0, 0], vec![2, 1, 2]]);
    assert_eq!(contain.ustr_ii, vec![vec![12, 2], vec![1, 2]]);
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
}

#[test]
fn south_pole_pentagon_splits_virtual_wedges_and_merges_back_to_original_cell() {
    let vertices = vec![
        LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        },
        LonLatPoint {
            lon: -144.0,
            lat: -80.0,
        },
        LonLatPoint {
            lon: -72.0,
            lat: -80.0,
        },
        LonLatPoint {
            lon: 0.0,
            lat: -80.0,
        },
        LonLatPoint {
            lon: 72.0,
            lat: -80.0,
        },
        LonLatPoint {
            lon: 144.0,
            lat: -80.0,
        },
    ];
    let cell_to_vertices = vec![vec![0], vec![1, 2, 3, 4, 5]];
    let n_edges = vec![0, 5];
    let is_in_area_ustr = vec![0, 1];
    let lon_i = vec![f64::NAN, -150.0, -90.0, -30.0, 30.0, 90.0, 150.0];
    let lat_i = vec![f64::NAN, -85.0, -75.0];
    let mut is_in_area_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    let seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    for row in is_in_area_grid.iter_mut().take(6).skip(2) {
        row[1] = 1;
    }

    let contain = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Ocean,
        &vertices,
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate south-pole containment");

    assert_eq!(contain.ustr_id, vec![vec![0, 0, 0], vec![4, 1, 4]]);
    assert_eq!(
        contain.ustr_ii,
        vec![vec![2, 1], vec![3, 1], vec![4, 1], vec![5, 1]]
    );
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
}

#[test]
fn south_pole_triangle_reshapes_to_canonical_rectangle_before_scanning() {
    let vertices = vec![
        LonLatPoint {
            lon: f64::NAN,
            lat: f64::NAN,
        },
        LonLatPoint {
            lon: 0.0,
            lat: -90.0,
        },
        LonLatPoint {
            lon: -10.0,
            lat: -80.0,
        },
        LonLatPoint {
            lon: 10.0,
            lat: -80.0,
        },
    ];
    let cell_to_vertices = vec![vec![0], vec![1, 2, 3]];
    let n_edges = vec![0, 3];
    let is_in_area_ustr = vec![0, 1];
    let lon_i = vec![f64::NAN, -8.0, 0.0, 8.0];
    let lat_i = vec![f64::NAN, -85.0, -75.0];
    let mut is_in_area_grid = vec![vec![0; lat_i.len()]; lon_i.len()];
    let seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    is_in_area_grid[3][1] = 1;

    let contain = getcontain_containment_matrix_one_based(
        GetContainMeshKind::Ocean,
        &vertices,
        &cell_to_vertices,
        &n_edges,
        &is_in_area_ustr,
        &is_in_area_grid,
        &seaorland,
        &lon_i,
        &lat_i,
        0,
    )
    .expect("calculate south-pole triangle containment");

    assert_eq!(contain.ustr_id, vec![vec![0, 0, 0], vec![1, 1, 1]]);
    assert_eq!(contain.ustr_ii, vec![vec![3, 1]]);
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
}
