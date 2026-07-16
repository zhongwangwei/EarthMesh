use earthmesh_cli::area_judge_refine_steps::activate_area_judge_calculated_refine_one_based;
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn refine_activation_iter_zero_copies_calculated_grid_and_bounds() {
    let mut calculated = vec![vec![0; 5]; 5];
    calculated[2][2] = 1;
    calculated[3][3] = 1;
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 2,
        maxlon_source: 3,
        maxlat_source: 2,
        minlat_source: 3,
    };

    let report = activate_area_judge_calculated_refine_one_based(&calculated, bounds)
        .expect("activate calculated refine");

    assert_eq!(report.bounds, bounds);
    assert_eq!(report.nlons_select, 2);
    assert_eq!(report.nlats_select, 2);
    assert_eq!(report.selected_cells, 2);
    assert!(report.is_in_refine[2][2]);
    assert!(report.is_in_refine[3][3]);
    assert_eq!(
        report.is_in_refine,
        calculated
            .iter()
            .map(|row| row.iter().map(|value| *value != 0).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
}

#[test]
fn refine_activation_rejects_invalid_bounds_like_area_judge_refine() {
    let calculated = vec![vec![0; 5]; 5];

    let err = activate_area_judge_calculated_refine_one_based(
        &calculated,
        AreaJudgeSourceBounds {
            minlon_source: 4,
            maxlon_source: 2,
            maxlat_source: 2,
            minlat_source: 3,
        },
    )
    .expect_err("invalid refine bounds should fail");

    assert!(err
        .to_string()
        .contains("invalid Area_judge refine bounds lon 4..2 lat 2..3"));
}
