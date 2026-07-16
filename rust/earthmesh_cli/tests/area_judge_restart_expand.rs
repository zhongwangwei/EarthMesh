use earthmesh_cli::{
    area_judge_grid_io::expand_area_judge_grid_payload_one_based,
    area_judge_grid_io::AreaJudgeGridPayload,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

#[test]
fn restart_payload_expands_selected_domain_and_seaorland_into_full_source_grids() {
    let payload = AreaJudgeGridPayload {
        bounds: AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 3,
            maxlat_source: 1,
            minlat_source: 2,
        },
        longitude: vec![20.0, 30.0],
        latitude: vec![50.0, 40.0],
        is_in_area_select: vec![vec![21, 22], vec![31, 32]],
        seaorland_select: Some(vec![vec![1, 0], vec![0, 1]]),
    };

    let report =
        expand_area_judge_grid_payload_one_based(&payload, 4, 4).expect("expand restart payload");

    assert_eq!(report.bounds, payload.bounds);
    assert_eq!(report.nlons_select, 2);
    assert_eq!(report.nlats_select, 2);
    assert!(report.is_in_domain[2][1]);
    assert!(report.is_in_domain[2][2]);
    assert!(report.is_in_domain[3][1]);
    assert!(report.is_in_domain[3][2]);
    assert!(!report.is_in_domain[1][1]);
    let _: &[Vec<bool>] = &report.seaorland;
    assert!(report.seaorland[2][1]);
    assert!(!report.seaorland[2][2]);
    assert!(!report.seaorland[3][1]);
    assert!(report.seaorland[3][2]);
    assert!(!report.seaorland[4][4]);
}

#[test]
fn restart_payload_expansion_requires_seaorland_like_canonical_restart_read() {
    let payload = AreaJudgeGridPayload {
        bounds: AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 1,
            maxlat_source: 1,
            minlat_source: 1,
        },
        longitude: vec![10.0],
        latitude: vec![20.0],
        is_in_area_select: vec![vec![1]],
        seaorland_select: None,
    };

    let err = expand_area_judge_grid_payload_one_based(&payload, 2, 2)
        .expect_err("restart expansion needs seaorland_select");

    assert!(
        err.to_string()
            .contains("Area_judge restart payload requires seaorland_select"),
        "unexpected error: {err}"
    );
}
