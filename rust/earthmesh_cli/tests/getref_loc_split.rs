use earthmesh_cli::split_getref_loc_containment_fortran_indexed;

fn one_based_i32(rows: &[[i32; 3]]) -> Vec<Vec<i32>> {
    std::iter::once(vec![0, 0, 0])
        .chain(rows.iter().map(|row| vec![row[0], row[1], row[2]]))
        .collect()
}

#[test]
fn loc_split_builds_land_ocean_and_atmos_lookups_like_getref_loc() {
    let loc_id = one_based_i32(&[[0, 0, 0], [3, 1, 0], [2, 4, 0], [0, 0, 0]]);
    let loc_ii = one_based_i32(&[
        [10, 20, 1],
        [11, 21, 0],
        [12, 22, 1],
        [13, 23, 0],
        [14, 24, 0],
    ]);

    let split = split_getref_loc_containment_fortran_indexed(&loc_id, &loc_ii, 1)
        .expect("split LOC containment");

    assert_eq!(split.sjx_points, 4);
    assert_eq!(
        split.land.mp_ii,
        vec![vec![0, 0], vec![10, 20], vec![12, 22]]
    );
    assert_eq!(
        split.land.mp_id,
        vec![vec![0, 0], vec![0, 1], vec![2, 1], vec![0, 3], vec![0, 3]]
    );

    assert_eq!(
        split.ocean.mp_ii,
        vec![vec![0, 0], vec![11, 21], vec![13, 23], vec![14, 24]]
    );
    assert_eq!(
        split.ocean.mp_id,
        vec![
            vec![0, 0, 0],
            vec![0, 1, 0],
            vec![1, 1, 3],
            vec![2, 2, 2],
            vec![0, 4, 0],
        ]
    );

    assert_eq!(
        split.atmos.mp_ii,
        vec![
            vec![0, 0],
            vec![10, 20],
            vec![11, 21],
            vec![12, 22],
            vec![13, 23],
            vec![14, 24],
        ]
    );
    assert_eq!(
        split.atmos.mp_id,
        vec![vec![0, 0], vec![0, 0], vec![3, 1], vec![2, 4], vec![0, 0]]
    );
}
