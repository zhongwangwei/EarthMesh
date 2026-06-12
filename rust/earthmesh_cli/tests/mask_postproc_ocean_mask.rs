#[test]
fn ocean_mask_ratio_adjustment_matches_fortran_contain_loop() {
    let contain = earthmesh_cli::ContainMesh {
        // Rust row 0 mirrors Fortran row 1.  The helper must only scan
        // Fortran rows num_vertex+1..num_ustr and use columns 1 and 3.
        ustr_id: vec![
            vec![0, 0, 0],
            vec![99, 1, 100],
            vec![2, 1, 4],
            vec![3, 3, 4],
            vec![0, 0, 4],
        ],
        ustr_ii: vec![vec![1, 1], vec![2, 2], vec![3, 3], vec![4, 4]],
        is_in_area_ustr: vec![0, 1, 1, 1, 1],
    };

    let adjusted = earthmesh_cli::apply_ocean_mask_sea_ratio_fortran_indexed(&contain, 2, 0.6)
        .expect("adjust ocean mask");

    assert_eq!(adjusted, vec![0, 1, -1, 1, 1]);
}

#[test]
fn ocean_mask_ratio_adjustment_rejects_missing_ratio_columns_and_bad_denominator() {
    let short_rows = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0], vec![2, 1]],
        ustr_ii: vec![vec![1, 1]],
        is_in_area_ustr: vec![0, 1],
    };
    let err = earthmesh_cli::apply_ocean_mask_sea_ratio_fortran_indexed(&short_rows, 1, 0.5)
        .expect_err("missing third ustr_id column rejected");
    assert!(err.to_string().contains("at least three columns"));

    let zero_denominator = earthmesh_cli::ContainMesh {
        ustr_id: vec![vec![0, 0, 0], vec![1, 1, 0]],
        ustr_ii: vec![vec![1, 1]],
        is_in_area_ustr: vec![0, 1],
    };
    let err = earthmesh_cli::apply_ocean_mask_sea_ratio_fortran_indexed(&zero_denominator, 1, 0.5)
        .expect_err("zero denominator rejected");
    assert!(err.to_string().contains("non-positive denominator"));
}
