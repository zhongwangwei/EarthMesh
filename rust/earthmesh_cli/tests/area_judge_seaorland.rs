use earthmesh_cli::{
    area_judge_domain_builders::build_area_judge_seaorland_one_based,
    area_judge_domain_builders::classify_area_judge_landtype_one_based,
    area_judge_types::AreaJudgeLandtypeClass,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn one_based_landtypes(nx: usize, ny: usize) -> Vec<Vec<i32>> {
    vec![vec![0; ny + 1]; nx + 1]
}

#[test]
fn seaorland_builder_treats_ocean_land_river_and_coast_codes_like_canonical_binary_mask() {
    // MOD_Area_judge.F90 uses `landtypes_global(i, j) /= 0` for seaorland.
    // That means river/coast-coded cells remain land in this binary Area_judge mask.
    assert_eq!(
        classify_area_judge_landtype_one_based(0),
        AreaJudgeLandtypeClass::Ocean
    );
    assert_eq!(
        classify_area_judge_landtype_one_based(1),
        AreaJudgeLandtypeClass::Land
    );
    assert_eq!(
        classify_area_judge_landtype_one_based(2),
        AreaJudgeLandtypeClass::Land
    );
    assert_eq!(
        classify_area_judge_landtype_one_based(7),
        AreaJudgeLandtypeClass::Land
    );

    let mut domain = vec![vec![0; 5]; 5];
    let mut landtypes = one_based_landtypes(4, 4);
    for i in 1..=4 {
        for j in 1..=4 {
            domain[i][j] = 1;
        }
    }

    landtypes[1][1] = 0; // ocean code
    landtypes[2][1] = 1; // ordinary land code
    landtypes[3][1] = 2; // river-like nonzero code
    landtypes[4][1] = 7; // coast-like nonzero code

    let report = build_area_judge_seaorland_one_based(
        &domain,
        &landtypes,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 4,
            maxlat_source: 1,
            minlat_source: 1,
        },
        "landmesh",
        true,
    )
    .expect("build seaorland classification parity report");

    assert_eq!(report.sum_land_grid, 3);
    assert_eq!(report.seaorland[1][1], 0);
    assert_eq!(report.seaorland[2][1], 1);
    assert_eq!(report.seaorland[3][1], 1);
    assert_eq!(report.seaorland[4][1], 1);
}

#[test]
fn seaorland_builder_marks_only_domain_land_cells_and_counts_them() {
    let mut domain = vec![vec![0; 5]; 5];
    let mut landtypes = one_based_landtypes(4, 4);
    for i in 1..=4 {
        for j in 1..=4 {
            domain[i][j] = 1;
        }
    }
    domain[1][1] = 0;
    landtypes[2][2] = 3;
    landtypes[3][2] = 0;
    landtypes[3][3] = 7;
    landtypes[4][4] = 9;

    let report = build_area_judge_seaorland_one_based(
        &domain,
        &landtypes,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        },
        "landmesh",
        true,
    )
    .expect("build seaorland");

    assert_eq!(report.sum_land_grid, 2);
    assert_eq!(report.seaorland[2][2], 1);
    assert_eq!(report.seaorland[3][2], 0);
    assert_eq!(report.seaorland[3][3], 1);
    assert_eq!(report.seaorland[4][4], 0);
}

#[test]
fn seaorland_builder_skips_atmosmesh_without_refine_like_canonical() {
    let domain = vec![vec![1; 3]; 3];
    let landtypes = vec![vec![8; 3]; 3];

    let report = build_area_judge_seaorland_one_based(
        &domain,
        &landtypes,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 2,
            maxlat_source: 1,
            minlat_source: 2,
        },
        "atmosmesh",
        false,
    )
    .expect("build atmos seaorland");

    assert_eq!(report.sum_land_grid, 0);
    assert_eq!(report.seaorland, vec![vec![0; 3]; 3]);
}
