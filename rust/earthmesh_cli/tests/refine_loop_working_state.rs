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

#[test]
fn working_state_applies_isreverse_judge_into_markers_and_segments() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 6],
        w_points: vec![point(0.0, 0.0); 6],
        m_to_w: vec![[1, 2, 3]; 6],
        w_to_m: vec![vec![1]; 6],
        n_w_to_m: vec![1; 6],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![4, 5, 7],
        vec![5, 6, 8],
        vec![2, 5, 9],
        vec![2, 3, 4],
        vec![3, 5, 10],
        vec![2, 11, 12],
        vec![3, 13, 14],
        vec![4, 15, 16],
        vec![6, 17, 18],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
        vec![2, 3, 4],
    ];
    state.mrl_new = vec![0, 1, 1, 1, 4, 1, 4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    state.segments = vec![vec![2, 3, 1], vec![0, 0, 0]];
    state.n_segments = vec![2, 0];

    let report = state
        .apply_isreverse_judge(3)
        .expect("apply reverse one-into-two judge through state");

    assert_eq!(report.marked_triangles, vec![5]);
    assert_eq!(report.active_segments, vec![0]);
    assert_eq!(report.rewritten_segments, vec![vec![3, 1, 1]]);
    assert_eq!(state.ref_sjx[5], 1);
    assert_eq!(state.ref_sjx.iter().sum::<i32>(), 1);
    assert_eq!(state.segments[0], vec![3, 1, 1]);
    assert_eq!(state.segments[1], vec![0, 0, 0]);
}

#[test]
fn working_state_applies_onedivide_two_into_child_geometry() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 3],
        w_points: vec![
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(6.0, 0.0),
            point(0.0, 6.0),
            point(0.0, 0.0),
            point(0.0, 0.0),
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4], [3, 4, 5]],
        w_to_m: vec![vec![1], vec![2], vec![2, 3], vec![2, 3], vec![3], vec![1]],
        n_w_to_m: vec![1, 1, 2, 2, 1, 1],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_vertex = 1;
    state.num_mp = vec![0, 3, 5];
    state.num_wp = vec![0, 6, 7];
    state.mp_new.resize(6, point(0.0, 0.0));
    state.wp_new.resize(8, point(0.0, 0.0));
    for row in &mut state.ngrmw_new {
        row.resize(6, 0);
    }
    state.triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ];
    state.ref_sjx = vec![0, 0, 1, 0, 0, 0];
    state.mrl_new = vec![0, 1, 1, 4, 1, 1];
    state.sjx_child = vec![[0, 0]; state.num_mp[state.iter] + 1];

    let report = state
        .apply_onedivide_two(false)
        .expect("apply one-into-two transition split through state");

    assert_eq!(report.split_triangles, vec![2]);
    assert_eq!(report.new_triangle_ids, vec![4, 5]);
    assert_eq!(report.new_vertex_ids, vec![7]);
    assert_eq!(state.wp_new[7], point(3.0, 3.0));
    assert_eq!(state.mp_new[4], point(3.0, 1.0));
    assert_eq!(state.mp_new[5], point(1.0, 3.0));
    assert_eq!(state.sjx_child[2], [4, 5]);
    assert_eq!(
        [
            state.ngrmw_new[1][4],
            state.ngrmw_new[2][4],
            state.ngrmw_new[3][4]
        ],
        [2, 3, 7]
    );
    assert_eq!(
        [
            state.ngrmw_new[1][5],
            state.ngrmw_new[2][5],
            state.ngrmw_new[3][5]
        ],
        [2, 4, 7]
    );
}

#[test]
fn working_state_looks_up_child_pair_from_current_sjx_child() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 2],
        w_points: vec![point(0.0, 0.0); 3],
        m_to_w: vec![[1, 2, 3], [1, 2, 3]],
        w_to_m: vec![vec![1], vec![1, 2], vec![2]],
        n_w_to_m: vec![1, 2, 1],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_mp = vec![0, 2, 7];
    state.sjx_child = vec![[0, 0], [4, 5], [6, 7]];
    state.ngrmw_new = vec![
        vec![0, 0, 0, 0, 10, 20, 30, 11],
        vec![0, 0, 0, 0, 11, 21, 31, 12],
        vec![0, 0, 0, 0, 12, 22, 32, 40],
        vec![0, 0, 0, 0, 0, 0, 0, 0],
    ];

    let report = state
        .lookup_m1w1_to_m11w11(1, 2)
        .expect("lookup child pair through state");

    assert_eq!(report.parent_pair, (1, 2));
    assert_eq!(report.child_pair, Some((4, 7)));
}

#[test]
fn working_state_applies_weak_concav_pair_special_into_lop_workspace() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 12],
        w_points: vec![point(0.0, 0.0); 12],
        m_to_w: vec![[1, 2, 3]; 12],
        w_to_m: vec![vec![1]; 12],
        n_w_to_m: vec![1; 12],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_mp = vec![0, 12, 12];
    state.triangle_neighbors = vec![vec![1, 1, 1]; 13];
    state.ngrmw = vec![vec![0; 13]; 4];
    state.mrl_new = vec![0; 13];
    state.ref_sjx = vec![0; 13];
    for triangle in 1..=12 {
        state.mrl_new[triangle] = 1;
        state.ngrmw[1][triangle] = triangle * 10;
        state.ngrmw[2][triangle] = triangle * 10 + 1;
        state.ngrmw[3][triangle] = triangle * 10 + 2;
    }
    state.triangle_neighbors[2] = vec![4, 5, 6];
    state.mrl_new[4] = 4;
    state.triangle_neighbors[5] = vec![7, 8, 4];
    state.ngrmw[1][3] = 100;
    state.ngrmw[2][3] = 101;
    state.ngrmw[3][3] = 102;
    state.ngrmw[1][7] = 100;
    state.ngrmw[2][7] = 200;
    state.ngrmw[3][7] = 201;
    state.ngrmw[1][8] = 300;
    state.ngrmw[2][8] = 301;
    state.ngrmw[3][8] = 302;
    state.triangle_neighbors[3] = vec![9, 10, 11];
    state.mrl_new[9] = 4;
    state.triangle_neighbors[10] = vec![11, 12, 9];
    state.ngrmw[1][2] = 500;
    state.ngrmw[2][2] = 501;
    state.ngrmw[3][2] = 502;
    state.ngrmw[1][11] = 500;
    state.ngrmw[2][11] = 600;
    state.ngrmw[3][11] = 601;
    state.ngrmw[1][12] = 700;
    state.ngrmw[2][12] = 701;
    state.ngrmw[3][12] = 702;
    state.weak_concav_pair = vec![[0, 0], [2, 0], [3, 0]];
    state.weak_concav_segment = vec![vec![0; 2]; 5];

    let report = state
        .apply_weak_concav_pair_special(2, 4)
        .expect("apply weak-concavity special state update");

    assert_eq!(state.weak_concav_pair[1], [2, 5]);
    assert_eq!(state.weak_concav_pair[2], [3, 10]);
    assert_eq!(state.ref_sjx[5], 1);
    assert_eq!(state.ref_sjx[10], 1);
    assert_eq!(state.weak_concav_segment[3][0], 7);
    assert_eq!(state.weak_concav_segment[4][0], 11);
    assert_eq!(state.mrl_new[8], 4);
    assert_eq!(state.mrl_new[12], 4);
    assert_eq!(report.updated_pairs, vec![[2, 5], [3, 10]]);
    assert_eq!(report.marked_ref_sjx_triangles, vec![5, 10]);
}
