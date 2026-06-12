use earthmesh_cli::build_area_judge_seaorland_fortran_indexed;
use earthmesh_mesh::AreaJudgeSourceBounds;

fn one_based_landtypes(nx: usize, ny: usize) -> Vec<Vec<i32>> {
    vec![vec![0; ny + 1]; nx + 1]
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

    let report = build_area_judge_seaorland_fortran_indexed(
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
fn seaorland_builder_skips_atmosmesh_without_refine_like_fortran() {
    let domain = vec![vec![1; 3]; 3];
    let landtypes = vec![vec![8; 3]; 3];

    let report = build_area_judge_seaorland_fortran_indexed(
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
