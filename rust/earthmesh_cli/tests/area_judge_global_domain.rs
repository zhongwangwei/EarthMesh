use earthmesh_cli::area_judge_domain_builders::initialize_area_judge_global_domain_one_based;
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn global_domain_initializer_marks_every_one_based_source_cell() {
    let report =
        initialize_area_judge_global_domain_one_based(4, 3).expect("initialize global domain");

    assert_eq!(
        report.bounds,
        AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 4,
            maxlat_source: 1,
            minlat_source: 3,
        }
    );
    assert_eq!(report.numpatch, 12);
    assert_eq!(report.nlons_select, 4);
    assert_eq!(report.nlats_select, 3);
    assert_eq!(report.is_in_domain.len(), 5);
    assert_eq!(
        report.is_in_domain[0],
        vec![0; 4],
        "Canonical slot zero stays unused"
    );
    for lon_index in 1..=4 {
        assert_eq!(report.is_in_domain[lon_index][0], 0);
        for lat_index in 1..=3 {
            assert_eq!(report.is_in_domain[lon_index][lat_index], 1);
        }
    }
}

#[test]
fn global_domain_initializer_rejects_empty_source_dimensions() {
    let err = initialize_area_judge_global_domain_one_based(0, 3)
        .expect_err("empty longitude source should fail");

    assert!(err
        .to_string()
        .contains("global domain source dimensions must be positive"));
}
