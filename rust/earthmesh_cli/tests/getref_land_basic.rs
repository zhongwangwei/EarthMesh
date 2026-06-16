use earthmesh_cli::{calculate_getref_land_basic_fortran_indexed, GetRefLandBasicConfig};

fn one_based_i32(rows: &[[i32; 2]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1]]))
        .collect()
}

#[test]
fn getref_land_basic_counts_landtypes_and_mainland_fraction_like_fortran() {
    let is_in_refine_sjx = vec![0, 0, 0, 1, 1, 0];
    let lnd_id = one_based_i32(&[[0, 0], [0, 0], [3, 1], [2, 4], [2, 6]]);
    let lnd_ii = one_based_i32(&[[1, 1], [2, 1], [2, 2], [3, 1], [3, 2], [4, 1], [4, 2]]);
    let mut landtypes = vec![vec![0; 4]; 5];
    landtypes[1][1] = 1;
    landtypes[2][1] = 2;
    landtypes[2][2] = 2;
    landtypes[3][1] = 5;
    landtypes[3][2] = 5;
    landtypes[4][1] = 9;
    landtypes[4][2] = 1;

    let report = calculate_getref_land_basic_fortran_indexed(
        &is_in_refine_sjx,
        &lnd_id,
        &lnd_ii,
        &landtypes,
        GetRefLandBasicConfig {
            num_vertex: 2,
            maxlc: 9,
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            refine_area_mainland: true,
            th_area_mainland: 0.70,
        },
    )
    .expect("calculate land thresholds");

    assert_eq!(report.ref_colnum, 2);
    assert_eq!(report.n_landtypes.as_ref().unwrap()[3], 2);
    assert_eq!(report.n_landtypes.as_ref().unwrap()[4], 1);
    assert_eq!(report.ref_th_land[3][1], 1);
    assert_eq!(report.ref_th_land[4][1], 0);

    assert!((report.f_mainarea.as_ref().unwrap()[3] - (2.0 / 3.0)).abs() < 1.0e-12);
    assert_eq!(report.f_mainarea.as_ref().unwrap()[4], 1.0);
    assert_eq!(report.ref_th_land[3][2], 1);
    assert_eq!(report.ref_th_land[4][2], 0);

    assert_eq!(report.ref_sjx[3], 1);
    assert_eq!(report.ref_sjx[4], 0);
    assert_eq!(report.ref_sjx[5], 0);
}

#[test]
fn getref_land_basic_ignores_maxlc_and_inactive_refine_cells() {
    let is_in_refine_sjx = vec![0, 0, 0, 1, 0];
    let lnd_id = one_based_i32(&[[0, 0], [0, 0], [2, 1], [2, 3]]);
    let lnd_ii = one_based_i32(&[[1, 1], [1, 2], [2, 1], [2, 2]]);
    let mut landtypes = vec![vec![0; 3]; 3];
    landtypes[1][1] = 9;
    landtypes[1][2] = 1;
    landtypes[2][1] = 1;
    landtypes[2][2] = 2;

    let report = calculate_getref_land_basic_fortran_indexed(
        &is_in_refine_sjx,
        &lnd_id,
        &lnd_ii,
        &landtypes,
        GetRefLandBasicConfig {
            num_vertex: 2,
            maxlc: 9,
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            refine_area_mainland: false,
            th_area_mainland: 0.0,
        },
    )
    .expect("calculate landtype threshold");

    assert_eq!(report.ref_colnum, 1);
    assert_eq!(report.n_landtypes.as_ref().unwrap()[3], 1);
    assert_eq!(report.ref_sjx[3], 0);
    assert_eq!(report.n_landtypes.as_ref().unwrap()[4], 0);
    assert_eq!(report.ref_sjx[4], 0);
    assert!(report.f_mainarea.is_none());
}

#[test]
fn getref_land_basic_rejects_landtype_codes_outside_fortran_maxlc_class_range() {
    let is_in_refine_sjx = vec![0, 0, 1];
    let lnd_id = one_based_i32(&[[0, 0], [1, 1]]);
    let lnd_ii = one_based_i32(&[[1, 1]]);
    let mut landtypes = vec![vec![0; 2]; 2];
    landtypes[1][1] = 10;

    let err = calculate_getref_land_basic_fortran_indexed(
        &is_in_refine_sjx,
        &lnd_id,
        &lnd_ii,
        &landtypes,
        GetRefLandBasicConfig {
            num_vertex: 1,
            maxlc: 9,
            refine_num_landtypes: true,
            th_num_landtypes: 0,
            refine_area_mainland: false,
            th_area_mainland: 0.0,
        },
    )
    .expect_err("landtype above maxlc should be rejected like Fortran nlaa(0:maxlc) bounds");

    assert!(
        err.to_string()
            .contains("landtypes value 10 at (1,1) outside 0..=maxlc 9"),
        "unexpected error: {err}"
    );
}

#[test]
fn getref_land_basic_skips_lookup_width_when_basic_thresholds_are_disabled() {
    let report = calculate_getref_land_basic_fortran_indexed(
        &[0, 1, 1],
        &[vec![0], vec![0], vec![0]],
        &[vec![0], vec![1], vec![2]],
        &[vec![0]],
        GetRefLandBasicConfig {
            num_vertex: 1,
            maxlc: 99,
            refine_num_landtypes: false,
            th_num_landtypes: 0,
            refine_area_mainland: false,
            th_area_mainland: 0.0,
        },
    )
    .expect("inactive land-basic thresholds should not require lookup tables");

    assert_eq!(report.ref_colnum, 0);
    assert_eq!(report.ref_sjx, vec![0, 0, 0]);
    assert!(report.n_landtypes.is_none());
    assert!(report.f_mainarea.is_none());
}
