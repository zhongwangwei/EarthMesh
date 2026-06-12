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

#[test]
fn working_state_prologue_reads_gridfile_copies_snapshot_and_returns_state() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_prologue_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        sources: Vec::new(),
        refine_loop_input_gridfile: gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile.clone(),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: root.join("gridfile/gridfile_NXP0004_02_tri.nc4"),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![point(-1.0, -2.0), point(3.0, 4.0)],
        w_points: vec![point(10.0, 11.0), point(12.0, 13.0), point(14.0, 15.0)],
        m_to_w: vec![[1, 2, 3], [3, 1, 2]],
        w_to_m: vec![vec![1, 2], vec![1, 2], vec![2, 1]],
        n_w_to_m: vec![2, 2, 2],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&gridfile, &mesh).expect("write input gridfile");

    let report = earthmesh_cli::run_mkgrd_refine_loop_working_state_prologue(&step)
        .expect("read prologue working state");

    assert_eq!(report.snapshot.sjx_points, 2);
    assert_eq!(report.snapshot.lbx_points, 3);
    assert_eq!(report.state.num_mp, vec![0, 2]);
    assert_eq!(report.state.num_wp, vec![0, 3]);
    assert_eq!(report.state.ngrmw[1][2], 3);
    assert_eq!(
        report
            .state
            .to_unstructured_mesh()
            .expect("round trip state"),
        mesh
    );
    assert_eq!(
        std::fs::read(&original_tmpfile).expect("read copied original"),
        std::fs::read(&gridfile).expect("read source gridfile")
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_applies_ngr_renew_into_final_arrays() {
    let initial = UnstructuredMesh {
        m_points: vec![point(-1.0, -1.0), point(1.0, 1.0), point(2.0, 2.0)],
        w_points: vec![
            point(1.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
            point(4.0, 0.0),
        ],
        m_to_w: vec![[1, 2, 3], [2, 3, 4], [3, 4, 2]],
        w_to_m: vec![vec![1, 1], vec![1, 2], vec![2, 3], vec![2, 3]],
        n_w_to_m: vec![1, 2, 2, 2],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_vertex = 1;
    state.num_mp = vec![0, 3, 6];
    state.num_wp = vec![0, 4, 7];
    state.mp_new.resize(7, point(0.0, 0.0));
    state.mp_new[4] = point(4.0, 4.0);
    state.mp_new[5] = point(5.0, 5.0);
    state.mp_new[6] = point(6.0, 6.0);
    state.wp_new.resize(8, point(0.0, 0.0));
    state.wp_new[5] = point(10.0, 1.0);
    state.wp_new[6] = point(10.0, 1.0);
    state.wp_new[7] = point(11.0, 2.0);
    for row in &mut state.ngrmw_new {
        row.resize(7, 0);
    }
    state.ngrmw_new[1][4] = 2;
    state.ngrmw_new[2][4] = 5;
    state.ngrmw_new[3][4] = 6;
    state.ngrmw_new[1][5] = 1;
    state.ngrmw_new[2][5] = 1;
    state.ngrmw_new[3][5] = 1;
    state.ngrmw_new[1][6] = 4;
    state.ngrmw_new[2][6] = 6;
    state.ngrmw_new[3][6] = 7;
    state.bdy_refine = vec![5, 6, 7];
    state.bdy_refine_tran = vec![6, 7];

    let report = state
        .apply_ngr_renew()
        .expect("apply NGR_RENEW through state");

    assert_eq!(report.num_sjx, 5);
    assert_eq!(report.num_dbx, 6);
    assert_eq!(state.num_sjx, 5);
    assert_eq!(state.num_dbx, 6);
    assert_eq!(state.bdy_refine, vec![5, 5, 6]);
    assert_eq!(state.bdy_refine_tran, vec![5, 6]);
    assert_eq!(
        [
            state.ngrmw_f[1][4],
            state.ngrmw_f[2][4],
            state.ngrmw_f[3][4]
        ],
        [2, 5, 5]
    );
    assert_eq!(
        [
            state.ngrmw_f[1][5],
            state.ngrmw_f[2][5],
            state.ngrmw_f[3][5]
        ],
        [4, 5, 6]
    );

    let final_mesh = state.to_unstructured_mesh().expect("export renewed state");
    assert_eq!(final_mesh.m_points.len(), 5);
    assert_eq!(final_mesh.w_points.len(), 6);
    assert_eq!(final_mesh.m_to_w[3], [2, 5, 5]);
    assert_eq!(final_mesh.m_to_w[4], [4, 5, 6]);
}

#[test]
fn working_state_applies_onedivide_four_connection_then_renew() {
    let initial = UnstructuredMesh {
        m_points: vec![point(2.0, 2.0), point(2.0, 2.0)],
        w_points: vec![point(0.0, 0.0), point(6.0, 0.0), point(0.0, 6.0)],
        m_to_w: vec![[1, 2, 3], [1, 2, 3]],
        w_to_m: vec![vec![1, 2], vec![1, 2], vec![1, 2]],
        n_w_to_m: vec![2, 2, 2],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_vertex = 1;
    state.num_mp = vec![0, 2, 6];
    state.num_wp = vec![0, 3, 6];
    state.mp_new.resize(7, point(0.0, 0.0));
    state.wp_new.resize(7, point(0.0, 0.0));
    for row in &mut state.ngrmw_new {
        row.resize(7, 0);
    }
    state.ref_sjx = vec![0, 0, 1];
    state.ref_lbx = vec![0, 0, 0, 0, 0, 0, 0];
    state.mrl_new = vec![0, 1, 1];

    let connection = state
        .apply_onedivide_four_connection()
        .expect("mark one-into-four candidates");
    assert_eq!(connection.marked_triangles, vec![2]);
    assert_eq!(connection.marked_vertices, vec![1, 2, 3]);
    assert_eq!(state.mrl_new[2], 4);
    assert_eq!(&state.ref_lbx[1..=3], &[1, 1, 1]);

    let renew = state
        .apply_onedivide_four_renew()
        .expect("renew one-into-four children through state");
    assert_eq!(renew.refined_triangles, vec![2]);
    assert_eq!(renew.new_triangle_ids, vec![3, 4, 5, 6]);
    assert_eq!(renew.new_vertex_ids, vec![4, 5, 6]);
    assert_eq!(state.wp_new[4], point(3.0, 3.0));
    assert_eq!(state.mp_new[3], point(1.0, 1.0));
    assert_eq!(
        [
            state.ngrmw_new[1][6],
            state.ngrmw_new[2][6],
            state.ngrmw_new[3][6]
        ],
        [4, 5, 6]
    );
}
