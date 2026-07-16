use earthmesh_mesh::{
    get_area_unit_one_based, get_edge_connectivity_one_based, CartesianPoint, GetAreaUnitInput,
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
            7.285_766_362_790_206e-1,
            3.942_021_890_840_816_7e-1,
            -5.601_613_331_797_062e-1,
        ),
        CartesianPoint::new(
            7.290_894_194_903_25e-1,
            4.007_193_663_973_309_7e-1,
            -5.548_446_699_585_231e-1,
        ),
        CartesianPoint::new(
            7.361_888_960_321_443e-1,
            4.007_746_958_894_907e-1,
            -5.453_490_189_719_413e-1,
        ),
        CartesianPoint::new(
            7.421_781_521_416_679e-1,
            3.913_119_465_559_448_7e-1,
            -5.441_015_998_562_92e-1,
        ),
        CartesianPoint::new(
            7.418_108_033_797_808e-1,
            3.846_204_975_307_679e-1,
            -5.493_485_276_836_817e-1,
        ),
        CartesianPoint::new(
            7.346_460_026_075_409e-1,
            3.845_328_633_889_552e-1,
            -5.589_541_374_984_642e-1,
        ),
        CartesianPoint::new(
            7.492_324_817_041_784e-1,
            3.913_864_456_638_736_5e-1,
            -5.342_914_359_317_371e-1,
        ),
        CartesianPoint::new(
            7.549_761_132_696_927e-1,
            3.822_144_124_065_588_5e-1,
            -5.328_444_532_327_397e-1,
        ),
        CartesianPoint::new(
            7.547_507_521_699_114e-1,
            3.752_854_197_518_125_7e-1,
            -5.380_633_381_124e-1,
        ),
        CartesianPoint::new(
            7.476_480_671_976_428e-1,
            3.751_917_028_059_747_5e-1,
            -5.479_539_704_767_017e-1,
        ),
        CartesianPoint::new(
            7.340_986_375_488_476e-1,
            3.778_467_947_256_447e-1,
            -5.642_082_860_650_687e-1,
        ),
        CartesianPoint::new(
            7.472_314_817_687_956e-1,
            3.683_224_486_304_216e-1,
            -5.531_579_218_347_149e-1,
        ),
        CartesianPoint::new(
            7.400_124_865_152_692e-1,
            3.682_115_049_938_831e-1,
            -5.628_514_967_481_459e-1,
        ),
    ];
    let edge_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(
            7.382_338_066_918_18e-1,
            3.845_834_895_240_147e-1,
            -5.541_718_021_000_702e-1,
        ),
        CartesianPoint::new(
            7.420_042_234_657_293e-1,
            3.880_249_011_123_293e-1,
            -5.466_867_553_506_186e-1,
        ),
        CartesianPoint::new(
            7.447_468_172_082_855e-1,
            3.799_027_412_170_040_7e-1,
            -5.486_584_415_407_59e-1,
        ),
    ];
    let cell_points = vec![
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(0.0, 0.0, 0.0),
        CartesianPoint::new(
            7.354_580_759_423_482e-1,
            3.926_879_853_633_090_7e-1,
            -5.521_753_024_923_197e-1,
        ),
        CartesianPoint::new(
            7.484_800_879_395_84e-1,
            3.833_250_629_285_821_5e-1,
            -5.411_464_257_377_583e-1,
        ),
        CartesianPoint::new(
            7.409_524_176_748_571e-1,
            3.764_492_370_969_083e-1,
            -5.561_254_234_711_267e-1,
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

    let output = get_area_unit_one_based(GetAreaUnitInput {
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

    approx_eq(output.area_triangle[6], 1.456_537_411_369_754e-4, 1.0e-15);
    approx_eq(output.area_cell[2], 2.893_344_967_073_004_3e-4, 1.0e-15);
    approx_eq(output.area_cell[3], 2.895_649_967_250_554_3e-4, 1.0e-15);
    approx_eq(output.area_cell[4], 2.917_605_338_803_966_6e-4, 1.0e-15);
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

    let output = get_edge_connectivity_one_based(&triangle_neighbors, &cells_on_vertex)
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
