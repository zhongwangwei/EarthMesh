#[test]
fn earth_patchtypes_follow_land_ratio_and_fortran_pixel_mapping() {
    let contain = earthmesh_cli::ContainMesh {
        // Fortran row 1 placeholder; rows 2 and 3 are active domain cells.
        ustr_id: vec![vec![0, 0], vec![2, 1], vec![2, 3], vec![1, 5]],
        ustr_ii: vec![
            vec![10, 20, 1],
            vec![11, 20, 0],
            vec![10, 21, 0],
            vec![11, 21, 0],
            vec![10, 20, 1],
        ],
        is_in_area_ustr: vec![0, 1, 1, 0],
    };

    let result = earthmesh_cli::build_earth_patchtypes_fortran_indexed(&contain, 0.4, 10, 20, 2, 2)
        .expect("earth patchtypes");

    assert_eq!(result.seaorland_ustr, vec![0, 1, -1, 0]);
    assert_eq!(result.sum_land_ustr, 1);
    assert_eq!(result.sum_sea_ustr, 1);
    assert_eq!(result.patchtypes_select, vec![vec![2, 0], vec![0, 0]]);
}

#[test]
fn earth_patchtypes_allows_empty_pixels_when_no_domain_cells_reference_pixels() {
    let contain = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0, 0], vec![0, 0, 0], vec![0, 1, 0]],
        ustr_ii: Vec::new(),
        is_in_area_ustr: vec![0, 0, 0],
    };

    let result = earthmesh_cli::build_earth_patchtypes_fortran_indexed(&contain, 0.4, 10, 20, 2, 2)
        .expect("empty inactive earth patchtypes");

    assert_eq!(result.seaorland_ustr, vec![0, 0, 0]);
    assert_eq!(result.sum_land_ustr, 0);
    assert_eq!(result.sum_sea_ustr, 0);
    assert_eq!(result.patchtypes_select, vec![vec![0, 0], vec![0, 0]]);
}

#[test]
fn earth_patchtypes_reject_bad_schema_and_out_of_range_pixels() {
    let short_pixels = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![1, 1]],
        ustr_ii: vec![vec![10, 20]],
        is_in_area_ustr: vec![0, 1],
    };
    let err =
        earthmesh_cli::build_earth_patchtypes_fortran_indexed(&short_pixels, 0.5, 10, 20, 1, 1)
            .expect_err("missing land flag column rejected");
    assert!(err.to_string().contains("at least three columns"));

    let bad_coordinate = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![1, 1]],
        ustr_ii: vec![vec![99, 20, 1]],
        is_in_area_ustr: vec![0, 1],
    };
    let err =
        earthmesh_cli::build_earth_patchtypes_fortran_indexed(&bad_coordinate, 0.5, 10, 20, 1, 1)
            .expect_err("out of range coordinate rejected");
    assert!(err.to_string().contains("outside patchtype grid"));
}
