use earthmesh_cli::validate_area_judge_refine_within_domain_fortran_indexed;
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn refine_domain_validator_accepts_refine_cells_inside_domain() {
    let mut domain = vec![vec![0; 5]; 5];
    let mut refine = vec![vec![0; 5]; 5];
    for i in 1..=4 {
        for j in 1..=4 {
            domain[i][j] = 1;
        }
    }
    refine[2][2] = 1;
    refine[3][3] = 1;

    validate_area_judge_refine_within_domain_fortran_indexed(
        &refine,
        &domain,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        },
    )
    .expect("refine is inside domain");
}

#[test]
fn refine_domain_validator_rejects_refine_cells_outside_domain() {
    let mut domain = vec![vec![0; 5]; 5];
    let mut refine = vec![vec![0; 5]; 5];
    domain[2][2] = 1;
    refine[2][2] = 1;
    refine[3][3] = 1;

    let err = validate_area_judge_refine_within_domain_fortran_indexed(
        &refine,
        &domain,
        AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 2,
            minlat_source: 3,
        },
    )
    .expect_err("refine outside domain should fail");

    assert!(err
        .to_string()
        .contains("refine area exceeds domain area at lon 3 lat 3"));
}
