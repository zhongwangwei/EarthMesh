use earthmesh_cli::{LonLatPoint, RefineLoopWorkingState, UnstructuredMesh};

fn point(lon: f64, lat: f64) -> LonLatPoint {
    LonLatPoint { lon, lat }
}

#[test]
fn working_state_round_trips_unstructured_mesh_into_fortran_indexed_arrays() {
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 10.0), point(20.0, 30.0)],
        w_points: vec![point(1.0, 2.0), point(3.0, 4.0), point(5.0, 6.0)],
        m_to_w: vec![[1, 2, 3], [3, 2, 1]],
        w_to_m: vec![vec![1, 2, 1], vec![2, 1, 1], vec![1, 2, 2]],
        n_w_to_m: vec![2, 2, 3],
    };

    let state = RefineLoopWorkingState::from_unstructured_mesh(&mesh);

    assert_eq!(state.iter, 1);
    assert_eq!(state.num_vertex, 0);
    assert_eq!(state.num_mp, vec![0, 2]);
    assert_eq!(state.num_wp, vec![0, 3]);
    assert_eq!(state.mp_new[0], point(0.0, 0.0));
    assert_eq!(state.mp_new[1], point(0.0, 10.0));
    assert_eq!(state.mp_new[2], point(20.0, 30.0));
    assert_eq!(state.wp_new[1], point(1.0, 2.0));
    assert_eq!(state.ngrmw[1][1], 1);
    assert_eq!(state.ngrmw[2][1], 2);
    assert_eq!(state.ngrmw[3][1], 3);
    assert_eq!(state.ngrmw_new, state.ngrmw);

    let round_trip = state.to_unstructured_mesh().expect("export final mesh");
    assert_eq!(round_trip, mesh);
}

#[test]
fn working_state_exports_current_renewed_final_arrays_when_available() {
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0)],
        w_points: vec![point(1.0, 1.0), point(2.0, 2.0), point(3.0, 3.0)],
        m_to_w: vec![[1, 2, 3]],
        w_to_m: vec![vec![1], vec![1], vec![1]],
        n_w_to_m: vec![1, 1, 1],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&mesh);
    state.mp_f = vec![point(0.0, 0.0), point(9.0, 9.0)];
    state.wp_f = vec![
        point(0.0, 0.0),
        point(7.0, 7.0),
        point(8.0, 8.0),
        point(9.0, 9.0),
    ];
    state.ngrmw_f = vec![vec![0, 0], vec![0, 3], vec![0, 2], vec![0, 1]];
    state.ngrwm_f = vec![vec![0, 0, 0, 0], vec![0, 1, 1, 1], vec![0, 1, 1, 1]];
    state.n_ngrwm_f = vec![0, 1, 1, 1];
    state.num_sjx = 1;
    state.num_dbx = 3;

    let exported = state.to_unstructured_mesh().expect("export renewed mesh");

    assert_eq!(exported.m_points, vec![point(9.0, 9.0)]);
    assert_eq!(
        exported.w_points,
        vec![point(7.0, 7.0), point(8.0, 8.0), point(9.0, 9.0)]
    );
    assert_eq!(exported.m_to_w, vec![[3, 2, 1]]);
    assert_eq!(exported.w_to_m, vec![vec![1, 1], vec![1, 1], vec![1, 1]]);
    assert_eq!(exported.n_w_to_m, vec![1, 1, 1]);
}
