#[test]
fn land_patchtypes_map_active_cells_and_fill_ignored_land_pixels_from_previous_latitude() {
    let contain = earthmesh_cli::ContainMesh {
        // Fortran row 1 placeholder.  Land mask_postproc treats any non-zero
        // IsInDmArea_ustr value as active, unlike the Earth ratio classifier.
        ustr_id: vec![vec![0, 0], vec![2, 1], vec![1, 3]],
        ustr_ii: vec![vec![10, 20], vec![11, 20], vec![10, 22]],
        is_in_area_ustr: vec![0, 1, -1],
    };
    let seaorland = vec![vec![1, 0, 1], vec![1, 1, 0]];

    let result =
        earthmesh_cli::build_land_patchtypes_fortran_indexed(&contain, &seaorland, 10, 20, 2, 3)
            .expect("land patchtypes");

    assert_eq!(result.patchtypes_select, vec![vec![2, 0, 3], vec![2, 2, 0]]);
    assert_eq!(result.seaorland, vec![vec![0, 0, 0], vec![0, 0, 0]]);
    assert_eq!(result.filled_ignored_land_pixels, 1);
}

#[test]
fn land_patchtypes_reject_bad_schema_and_fill_or_reject_unmapped_latitude_land_pixel() {
    let short_pixels = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![1, 1]],
        ustr_ii: vec![vec![10]],
        is_in_area_ustr: vec![0, 1],
    };
    let seaorland = vec![vec![0]];
    let err = earthmesh_cli::build_land_patchtypes_fortran_indexed(
        &short_pixels,
        &seaorland,
        10,
        20,
        1,
        1,
    )
    .expect_err("missing latitude column rejected");
    assert!(err.to_string().contains("at least two columns"));

    let first_row_contain = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![1, 1]],
        ustr_ii: vec![vec![10, 21]],
        is_in_area_ustr: vec![0, 1],
    };
    let first_row_land = vec![vec![1, 0]];
    let result = earthmesh_cli::build_land_patchtypes_fortran_indexed(
        &first_row_contain,
        &first_row_land,
        10,
        20,
        1,
        2,
    )
    .expect("first latitude row inherits next available patch id");
    assert_eq!(result.patchtypes_select, vec![vec![2, 2]]);
    assert_eq!(result.filled_ignored_land_pixels, 1);

    let no_active_cells = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0]],
        ustr_ii: vec![vec![10, 20]],
        is_in_area_ustr: vec![0],
    };
    let first_row_land = vec![vec![1]];
    let err = earthmesh_cli::build_land_patchtypes_fortran_indexed(
        &no_active_cells,
        &first_row_land,
        10,
        20,
        1,
        1,
    )
    .expect_err("unmapped land with no neighboring patch id is rejected");
    assert!(err.to_string().contains("neighboring patch"));
}
