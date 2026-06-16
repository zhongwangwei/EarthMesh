use earthmesh_cli::{
    apply_onedivide_two_fortran_indexed, write_getref_specified_threshold_netcdf,
    AreaJudgeGridWriteReport, AreaJudgeRefineGridRunReport, AreaJudgeRefineStepReport,
    ContainWriteReport, GetContainRefineFileRunReport, GetContainRuntimeCounts,
    GetRefSpecifiedThresholdWriteReport, LonLatPoint, MkgrdRefineLoopExecutor,
    MkgrdRefineLoopStepIoPlan, MkgrdRefineLoopWorkingStateExecutor, MkgrdRefineSource,
    MkgrdRefineSourceBranchReport, MkgrdRefineSourceIoPlan, MkgrdSpecifiedRefineSourceBranchReport,
    RefineLoopWorkingState, UnstructuredMesh,
};
use earthmesh_mesh::AreaJudgeSourceBounds;

fn point(lon: f64, lat: f64) -> LonLatPoint {
    LonLatPoint { lon, lat }
}

#[test]
fn working_state_derives_triangle_neighbors_from_gridfile_membership() {
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0), point(1.0, 1.0)],
        w_points: vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 1.0),
        ],
        m_to_w: vec![[1, 2, 3], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![1, 2], vec![1, 2], vec![2]],
        n_w_to_m: vec![1, 2, 2, 1],
    };

    let state = RefineLoopWorkingState::from_unstructured_mesh(&mesh);

    assert_eq!(state.triangle_neighbors[1], vec![2, 0, 0]);
    assert_eq!(state.triangle_neighbors[2], vec![0, 0, 1]);
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
        max_transition_row: 1,
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

fn lop_child_rows() -> Vec<Vec<usize>> {
    let mut rows = vec![vec![0; 90]; 4];
    let mut set = |triangle: usize, vertices: [usize; 3]| {
        rows[1][triangle] = vertices[0];
        rows[2][triangle] = vertices[1];
        rows[3][triangle] = vertices[2];
    };
    set(20, [1, 2, 3]);
    set(21, [90, 91, 92]);
    set(30, [10, 11, 13]);
    set(31, [30, 31, 32]);
    set(40, [60, 61, 62]);
    set(50, [40, 41, 42]);
    set(51, [2, 3, 4]);
    set(60, [10, 11, 12]);
    set(61, [2, 3, 4]);
    rows
}

#[test]
fn working_state_applies_sharp_concav_lop_judge_into_ref_segments() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 7],
        w_points: vec![point(0.0, 0.0); 7],
        m_to_w: vec![[1, 2, 3]; 7],
        w_to_m: vec![vec![1]; 7],
        n_w_to_m: vec![1; 7],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_mp = vec![0, 7, 89];
    state.ngrmw_new = lop_child_rows();
    state.mrl_new = vec![1; 8];
    state.mrl_new[6] = 4;
    state.triangle_neighbors = vec![vec![1, 1, 1]; 8];
    state.triangle_neighbors[4] = vec![5, 6, 7];
    state.sjx_child = vec![[0, 0]; 8];
    state.sjx_child[2] = [20, 21];
    state.sjx_child[3] = [30, 31];
    state.sjx_child[6] = [60, 61];
    state.bdy_refine_segment = vec![vec![], vec![0, 4]];
    state.bdy_refine_segment_old = vec![vec![], vec![0, 2, 3]];
    state.n_bdy_refine_segment = vec![0, 2];
    state.ref_sjx_segment_temp = vec![vec![0; 9]; 2];
    state.n_ref_sjx_segment_temp = vec![0, 1];
    state.num_ref = 0;

    let report = state
        .apply_sharp_concav_lop_judge(1)
        .expect("apply sharp-concavity LOP through state");

    assert_eq!(state.ref_sjx_segment_temp[1][1..=4], [20, 61, 60, 30]);
    assert_eq!(state.n_ref_sjx_segment_temp[1], 4);
    assert_eq!(state.num_ref, 4);
    assert_eq!(report.num_ref_added, 4);
    assert_eq!(report.written_segments, vec![(1, vec![20, 61, 60, 30])]);
}

#[test]
fn working_state_applies_weak_concav_lop_judge_into_ref_segments() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 9],
        w_points: vec![point(0.0, 0.0); 9],
        m_to_w: vec![[1, 2, 3]; 9],
        w_to_m: vec![vec![1]; 9],
        n_w_to_m: vec![1; 9],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_mp = vec![0, 9, 89];
    state.ngrmw_new = lop_child_rows();
    state.mrl_new = vec![1; 10];
    state.triangle_neighbors = vec![vec![1, 1, 1]; 10];
    state.sjx_child = vec![[0, 0]; 10];
    state.sjx_child[2] = [20, 21];
    state.sjx_child[6] = [60, 61];
    state.weak_concav_segment = vec![vec![]];
    state.weak_concav_segment_old = vec![vec![]];
    state.n_weak_concav_segment = vec![0];
    state.weak_concav_pair = vec![[0, 0], [2, 6]];
    state.ref_sjx_segment_temp = vec![vec![0; 6]; 3];
    state.n_ref_sjx_segment_temp = vec![0; 3];
    state.num_ref = 0;

    let report = state
        .apply_weak_concav_lop_judge(1, 0, 0, 1)
        .expect("apply weak-concavity LOP through state");

    assert_eq!(state.ref_sjx_segment_temp[2][1..=2], [20, 61]);
    assert_eq!(state.n_ref_sjx_segment_temp[2], 2);
    assert_eq!(state.num_ref, 2);
    assert_eq!(report.num_ref_added, 2);
    assert_eq!(report.written_segments, vec![(2, vec![20, 61])]);
}

#[test]
fn working_state_applies_delaunay_lop_into_new_connectivity() {
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); 3],
        w_points: vec![point(0.0, 0.0); 13],
        m_to_w: vec![[1, 2, 3]; 3],
        w_to_m: vec![vec![1]; 13],
        n_w_to_m: vec![1; 13],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.iter = 2;
    state.num_mp = vec![0, 3, 5];
    state.num_wp = vec![0, 13, 14];
    state.mp_new.resize(6, point(0.0, 0.0));
    state.wp_new.resize(15, point(0.0, 0.0));
    state.wp_new[10] = point(0.0, 0.0);
    state.wp_new[11] = point(6.0, 0.0);
    state.wp_new[12] = point(0.0, 6.0);
    state.wp_new[13] = point(6.0, 6.0);
    state.wp_new[14] = point(-180.0, 9.0);
    state.ngrmw_new = vec![vec![0; 6]; 4];
    state.ngrmw_new[1][2] = 10;
    state.ngrmw_new[2][2] = 11;
    state.ngrmw_new[3][2] = 12;
    state.ngrmw_new[1][3] = 11;
    state.ngrmw_new[2][3] = 12;
    state.ngrmw_new[3][3] = 13;
    state.ref_sjx_segment = vec![0, 2, 3];
    state.num_ref = 2;

    let report = state
        .apply_delaunay_lop()
        .expect("apply Delaunay LOP through state");

    assert_eq!(report.flipped_pairs, vec![(2, 3)]);
    assert_eq!(report.new_triangle_ids, vec![4, 5]);
    assert_eq!(report.dateline_adjusted, false);
    assert_eq!(
        [
            state.ngrmw_new[1][4],
            state.ngrmw_new[2][4],
            state.ngrmw_new[3][4]
        ],
        [10, 11, 13]
    );
    assert_eq!(
        [
            state.ngrmw_new[1][5],
            state.ngrmw_new[2][5],
            state.ngrmw_new[3][5]
        ],
        [10, 12, 13]
    );
    assert_eq!(state.mp_new[4], point(4.0, 2.0));
    assert_eq!(state.mp_new[5], point(2.0, 4.0));
    assert_eq!(state.wp_new[14], point(180.0, 9.0));
}

#[test]
fn working_state_executor_runs_file_backed_passthrough_refine_step() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_executor_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 1,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile.clone(),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
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
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");

    let executor = MkgrdRefineLoopWorkingStateExecutor::default();
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run passthrough refine-loop step");

    assert_eq!(report.output_gridfile, output_gridfile);
    assert_eq!(report.state.num_mp, vec![0, 2]);
    assert_eq!(
        earthmesh_cli::read_unstructured_mesh_netcdf(&report.output_gridfile)
            .expect("read output gridfile"),
        mesh
    );
    assert_eq!(
        std::fs::read(&original_tmpfile).expect("read copied original"),
        std::fs::read(&input_gridfile).expect("read input gridfile")
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_applies_array_length_calculation_and_close_mesh_outputs() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_state_array_length_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    let sjx_points = 9;
    let lbx_points = 13;
    let initial = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0); sjx_points],
        w_points: vec![point(0.0, 0.0); lbx_points],
        m_to_w: vec![[1, 2, 3]; sjx_points],
        w_to_m: vec![vec![1]; lbx_points],
        n_w_to_m: vec![1; lbx_points],
    };
    let mut state = RefineLoopWorkingState::from_unstructured_mesh(&initial);
    state.num_vertex = 1;
    state.num_mp = vec![0, sjx_points];
    state.num_wp = vec![0, lbx_points];
    state.num_tranrow_sjx = 0;
    state.mrl_new = vec![0; sjx_points + 1];
    state.triangle_neighbors = vec![vec![1, 1, 1]; sjx_points + 1];
    state.ngrmw = vec![vec![0; sjx_points + 1]; 4];
    state.ngrwm = vec![vec![0; lbx_points + 1]; 4];
    state.n_ngrwm = vec![0; lbx_points + 1];
    state.wp_new = vec![point(0.0, 0.0); lbx_points + 1];

    for triangle in 2..=9 {
        state.mrl_new[triangle] = 1;
    }
    for refined in [6, 7, 8, 9] {
        state.mrl_new[refined] = 4;
    }
    state.triangle_neighbors[2] = vec![6, 3, 5];
    state.triangle_neighbors[3] = vec![7, 4, 2];
    state.triangle_neighbors[4] = vec![8, 5, 3];
    state.triangle_neighbors[5] = vec![9, 2, 4];
    state.ngrmw[1][2] = 10;
    state.ngrmw[2][2] = 11;
    state.ngrmw[3][2] = 99;
    state.ngrmw[1][3] = 11;
    state.ngrmw[2][3] = 12;
    state.ngrmw[3][3] = 99;
    state.ngrmw[1][4] = 12;
    state.ngrmw[2][4] = 13;
    state.ngrmw[3][4] = 99;
    state.ngrmw[1][5] = 13;
    state.ngrmw[2][5] = 10;
    state.ngrmw[3][5] = 99;
    state.ngrmw[1][6] = 10;
    state.ngrmw[2][6] = 11;
    state.ngrmw[3][6] = 90;
    state.ngrmw[1][7] = 11;
    state.ngrmw[2][7] = 12;
    state.ngrmw[3][7] = 91;
    state.ngrmw[1][8] = 12;
    state.ngrmw[2][8] = 13;
    state.ngrmw[3][8] = 92;
    state.ngrmw[1][9] = 13;
    state.ngrmw[2][9] = 10;
    state.ngrmw[3][9] = 93;
    state.ngrwm[1][10] = 5;
    state.ngrwm[2][10] = 2;
    state.ngrwm[3][10] = 6;
    state.ngrwm[1][11] = 2;
    state.ngrwm[2][11] = 3;
    state.ngrwm[3][11] = 7;
    state.ngrwm[1][12] = 3;
    state.ngrwm[2][12] = 4;
    state.ngrwm[3][12] = 8;
    state.ngrwm[1][13] = 4;
    state.ngrwm[2][13] = 5;
    state.ngrwm[3][13] = 9;
    for cell in 10..=13 {
        state.n_ngrwm[cell] = 3;
        state.wp_new[cell] = point(100.0 + cell as f64, 20.0 + cell as f64);
    }

    let report = state
        .apply_array_length_calculation(&root, 4, 1)
        .expect("apply Array_length_calculation through state");

    assert_eq!(state.num_tranrow_sjx, 4);
    assert_eq!(state.bdy_refine, vec![10, 11, 12, 13]);
    assert_eq!(state.bdy_refine_tran, Vec::<usize>::new());
    assert_eq!(report.calculation.boundary.curves.num_closed_curve, 1);
    assert_eq!(report.close_meshes.mask_patch_ndm, 1);
    assert_eq!(
        earthmesh_cli::read_close_mesh_netcdf(root.join("tmpfile/mask_patch_close_4_001.nc4"))
            .expect("read close mesh"),
        vec![
            state.wp_new[10],
            state.wp_new[11],
            state.wp_new[12],
            state.wp_new[13]
        ]
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_executor_exits_one_into_four_pipeline_for_isolated_marker() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_onefour_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 0,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile.clone(),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0), point(2.0, 2.0)],
        w_points: vec![
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(6.0, 0.0),
            point(0.0, 6.0),
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");

    let executor =
        MkgrdRefineLoopWorkingStateExecutor::with_one_into_four_ref_sjx(vec![0, 0, 1], 1, 0);
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run configured one-into-four refine-loop step");

    assert_eq!(
        report
            .onedivide_four_renew
            .as_ref()
            .unwrap()
            .refined_triangles,
        Vec::<usize>::new()
    );
    assert_eq!(report.ngr_renew.as_ref().unwrap().num_sjx, 2);
    assert_eq!(report.ngr_renew.as_ref().unwrap().num_dbx, 4);
    let output = earthmesh_cli::read_unstructured_mesh_netcdf(&output_gridfile)
        .expect("read refined output gridfile");
    assert_eq!(output.m_points.len(), 2);
    assert_eq!(output.w_points.len(), 4);
    assert_eq!(output.m_points[1], point(2.0, 2.0));
    assert!(output.m_to_w.iter().all(|row| row.iter().all(|&id| id > 0)));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_executor_runs_configured_one_into_two_transition_pipeline() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_onetwo_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 1,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile.clone(),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
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
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write transition input gridfile");

    let executor = MkgrdRefineLoopWorkingStateExecutor::with_one_into_two_ref_sjx(
        vec![0, 0, 1, 0],
        false,
        1,
        vec![0, 1, 1, 4],
    )
    .with_one_into_two_triangle_neighbors(vec![
        vec![0, 0, 0],
        vec![0, 0, 0],
        vec![3, 3, 3],
        vec![2, 2, 2],
        vec![2, 3, 3],
        vec![2, 3, 3],
    ]);
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run configured one-into-two transition refine-loop step");

    assert!(report.onedivide_four_renew.is_none());
    assert_eq!(
        report.onedivide_two.as_ref().unwrap().split_triangles,
        vec![2]
    );
    assert_eq!(
        report.onedivide_two.as_ref().unwrap().new_triangle_ids,
        vec![4, 5]
    );
    assert_eq!(report.state.wp_new[7], point(3.0, 3.0));
    assert_eq!(report.state.sjx_child[2], [4, 5]);
    let output = earthmesh_cli::read_unstructured_mesh_netcdf(&output_gridfile)
        .expect("read transition output gridfile");
    assert_eq!(output.m_points.len(), 5);
    assert_eq!(output.w_points.len(), 7);
    assert_eq!(output.m_points[3], point(3.0, 1.0));
    assert_eq!(output.m_points[4], point(1.0, 3.0));
    assert_eq!(output.m_to_w[3], [2, 3, 7]);
    assert_eq!(output.m_to_w[4], [2, 4, 7]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_executor_reads_specified_threshold_file_for_one_into_four_markers() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_specified_threshold_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let threshold_file = root.join("threshold/threshold_specified_NXP0004_01.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 1,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile.clone(),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0), point(2.0, 2.0)],
        w_points: vec![
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(6.0, 0.0),
            point(0.0, 6.0),
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");
    write_getref_specified_threshold_netcdf(&threshold_file, &[0, 0, 1])
        .expect("write specified threshold marker file");

    let executor = MkgrdRefineLoopWorkingStateExecutor::with_specified_threshold_file(
        threshold_file.clone(),
        1,
        0,
    );
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run refine-loop from specified threshold marker file");

    assert_eq!(report.loaded_ref_sjx, Some(vec![0, 0, 1]));
    assert_eq!(
        report
            .onedivide_four_renew
            .as_ref()
            .unwrap()
            .refined_triangles,
        vec![2]
    );
    assert_eq!(report.ngr_renew.as_ref().unwrap().num_sjx, 5);
    assert_eq!(
        earthmesh_cli::read_unstructured_mesh_netcdf(&output_gridfile)
            .expect("read refined output")
            .m_points
            .len(),
        5
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_executor_accepts_specified_source_threshold_for_next_step() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_source_threshold_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let threshold_file = root.join("threshold/threshold_specified_NXP0004_01.nc4");
    let source = MkgrdRefineSourceIoPlan {
        source: MkgrdRefineSource::SpecifiedStep,
        area_judge_iter: 1,
        get_contain_iter: 1,
        getref_iter: 1,
        area_judge_output: root.join("tmpfile/refine_area.nc4"),
        contain_output: root.join("tmpfile/contain.nc4"),
        threshold_outputs: Vec::new(),
        specified_threshold_output: Some(threshold_file.clone()),
    };
    let step = MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 1,
        sources: vec![source.clone()],
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile,
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0), point(2.0, 2.0)],
        w_points: vec![
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(6.0, 0.0),
            point(0.0, 6.0),
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");
    write_getref_specified_threshold_netcdf(&threshold_file, &[0, 0, 1])
        .expect("write specified threshold marker file");

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    let bounds = AreaJudgeSourceBounds {
        minlon_source: 1,
        maxlon_source: 1,
        maxlat_source: 1,
        minlat_source: 1,
    };
    let report = MkgrdRefineSourceBranchReport::Specified(MkgrdSpecifiedRefineSourceBranchReport {
        area: AreaJudgeRefineGridRunReport {
            refine_step: AreaJudgeRefineStepReport {
                is_in_refine: vec![vec![0, 1]],
                bounds,
                nlons_select: 1,
                nlats_select: 1,
                selected_cells: 1,
                source_numpatch: None,
            },
            refine_write: AreaJudgeGridWriteReport {
                output: source.area_judge_output.clone(),
                bounds,
                nlons_select: 1,
                nlats_select: 1,
                selected_cells: 1,
                has_seaorland: false,
            },
        },
        contain: GetContainRefineFileRunReport {
            output: source.contain_output.clone(),
            active_unstructured_cells: 1,
            contained_source_pixels: 1,
            runtime_counts: GetContainRuntimeCounts {
                current_num_mp_step: 2,
                current_num_wp_step: 4,
                previous_num_vertex: 1,
            },
            write: ContainWriteReport {
                output: source.contain_output.clone(),
                num_ustr: 1,
                num_ii: 1,
                dim_a: 1,
                dim_b: 1,
            },
        },
        specified_threshold: GetRefSpecifiedThresholdWriteReport {
            output: threshold_file.clone(),
            sjx_points: 2,
        },
    });
    executor
        .accept_source_branch_report(&step, &source, &report)
        .expect("accept source threshold handoff");
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run refine-loop from accepted specified source threshold");

    assert_eq!(report.loaded_ref_sjx, Some(vec![0, 0, 1]));
    assert_eq!(report.ngr_renew.as_ref().unwrap().num_sjx, 5);
    assert_eq!(
        earthmesh_cli::read_unstructured_mesh_netcdf(&output_gridfile)
            .expect("read refined output")
            .m_points
            .len(),
        5
    );

    let _ = std::fs::remove_dir_all(&root);
}

fn write_ref_th_matrix(
    path: &std::path::Path,
    var_name: &str,
    rows: usize,
    cols: usize,
    values: &[i32],
) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create threshold parent");
    }
    let mut file = netcdf::create(path).expect("create calculated threshold file");
    file.add_dimension("sjx_points", rows).expect("sjx dim");
    file.add_dimension("ref_colnum", cols).expect("col dim");
    let mut var = file
        .add_variable::<i32>(var_name, &["sjx_points", "ref_colnum"])
        .expect("ref_th variable");
    var.put_values(values, (.., ..)).expect("write ref_th");
}

#[test]
fn working_state_executor_reads_calculated_threshold_files_for_one_into_four_markers() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_calculated_threshold_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let land_threshold = root.join("threshold/threshold_calculate_land_NXP0004_01.nc4");
    write_ref_th_matrix(&land_threshold, "ref_th_Lnd", 2, 2, &[0, 0, 1, 0]);
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 1,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile.clone(),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![point(0.0, 0.0), point(2.0, 2.0)],
        w_points: vec![
            point(0.0, 0.0),
            point(0.0, 0.0),
            point(6.0, 0.0),
            point(0.0, 6.0),
        ],
        m_to_w: vec![[1, 1, 1], [2, 3, 4]],
        w_to_m: vec![vec![1], vec![2], vec![2], vec![2]],
        n_w_to_m: vec![1, 1, 1, 1],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");

    let executor = MkgrdRefineLoopWorkingStateExecutor::with_calculated_threshold_files(
        vec![land_threshold],
        1,
        0,
    );
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run refine-loop from calculated threshold marker file");

    assert_eq!(report.loaded_ref_sjx, Some(vec![0, 0, 1]));
    assert_eq!(
        report
            .onedivide_four_renew
            .as_ref()
            .unwrap()
            .refined_triangles,
        vec![2]
    );
    assert_eq!(report.ngr_renew.as_ref().unwrap().num_sjx, 5);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn working_state_executor_keeps_non_isolated_one_into_four_transition_band() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_iterb_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let original_tmpfile = root.join("tmpfile/gridfile_NXP0004_01_ori.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 1,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: original_tmpfile,
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile,
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 1.0),
        ],
        w_points: vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 1.0),
        ],
        m_to_w: vec![[1, 2, 3], [1, 4, 2], [2, 4, 3], [3, 4, 1]],
        w_to_m: vec![vec![1, 2, 4], vec![1, 3, 2], vec![1, 4, 3], vec![2, 3, 4]],
        n_w_to_m: vec![3, 3, 3, 3],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");

    let executor =
        MkgrdRefineLoopWorkingStateExecutor::with_one_into_four_ref_sjx(vec![0, 0, 1, 1, 1], 1, 1);
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run one-into-four refine-loop step with iterB transition expansion");

    assert_eq!(
        report
            .onedivide_four_renew
            .as_ref()
            .unwrap()
            .refined_triangles,
        vec![2, 3, 4]
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn one_into_two_accepts_base_length_refinement_markers_after_one_into_four() {
    let num_mp = vec![0, 4, 8, 10];
    let num_wp = vec![0, 4, 6, 7];
    let triangle_neighbors = vec![
        vec![0, 0, 0],
        vec![2, 3, 4],
        vec![3, 4, 1],
        vec![4, 2, 1],
        vec![2, 3, 1],
    ];
    let ngrmw = vec![
        vec![0; 5],
        vec![0, 1, 1, 2, 3],
        vec![0, 2, 4, 4, 4],
        vec![0, 3, 2, 3, 1],
    ];
    let ref_sjx = vec![0, 0, 0, 0, 1];
    let mrl_new = vec![0, 1, 4, 4, 1];
    let mut mp_new = vec![point(0.0, 0.0); 11];
    let mut wp_new = vec![point(0.0, 0.0); 8];
    wp_new[1] = point(0.0, 0.0);
    wp_new[2] = point(1.0, 0.0);
    wp_new[3] = point(0.0, 1.0);
    wp_new[4] = point(1.0, 1.0);
    let mut ngrmw_new = vec![vec![0; 11]; 4];
    for row in 1..=3 {
        for triangle in 1..=4 {
            ngrmw_new[row][triangle] = ngrmw[row][triangle];
        }
    }
    let mut sjx_child = vec![[0, 0]; 5];

    let report = apply_onedivide_two_fortran_indexed(
        3,
        false,
        1,
        &num_mp,
        &num_wp,
        &triangle_neighbors,
        &ngrmw,
        &ref_sjx,
        &mrl_new,
        &mut mp_new,
        &mut wp_new,
        &mut ngrmw_new,
        &mut sjx_child,
    )
    .expect("one-into-two should use base-length mrl_new markers like Fortran");

    assert_eq!(report.split_triangles, vec![4]);
    assert_eq!(report.new_triangle_ids, vec![9, 10]);
    assert_eq!(report.new_vertex_ids, vec![7]);
}

#[test]
fn working_state_executor_removes_isolated_one_into_four_markers_like_fortran() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_refine_loop_working_state_isolated_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let input_gridfile = root.join("gridfile/gridfile_NXP0004_01_tri.nc4");
    let output_gridfile = root.join("gridfile/gridfile_NXP0004_02_tri.nc4");
    let step = earthmesh_cli::MkgrdRefineLoopStepIoPlan {
        step: 1,
        max_transition_row: 0,
        sources: Vec::new(),
        refine_loop_input_gridfile: input_gridfile.clone(),
        refine_loop_original_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_ori.nc4"),
        refine_loop_stage2_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_2.nc4"),
        refine_loop_stage5_tmpfile: root.join("tmpfile/gridfile_NXP0004_01_5.nc4"),
        refine_loop_output_gridfile: output_gridfile.clone(),
        run_refine_loop: true,
        stop_after_step: false,
    };
    let mesh = UnstructuredMesh {
        m_points: vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 1.0),
        ],
        w_points: vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
            point(1.0, 1.0),
        ],
        m_to_w: vec![[1, 2, 3], [1, 4, 2], [2, 4, 3], [3, 4, 1]],
        w_to_m: vec![vec![1, 2, 4], vec![1, 3, 2], vec![1, 4, 3], vec![2, 3, 4]],
        n_w_to_m: vec![3, 3, 3, 3],
    };
    earthmesh_cli::write_unstructured_mesh_netcdf(&input_gridfile, &mesh)
        .expect("write input gridfile");

    let executor =
        MkgrdRefineLoopWorkingStateExecutor::with_one_into_four_ref_sjx(vec![0, 0, 1, 0, 0], 1, 0);
    let report = executor
        .run_refine_loop_step_report(&step)
        .expect("run one-into-four refine-loop step with isolated marker removal");

    assert_eq!(
        report
            .onedivide_four_renew
            .as_ref()
            .unwrap()
            .refined_triangles,
        Vec::<usize>::new()
    );
    assert_eq!(report.ngr_renew.as_ref().unwrap().num_sjx, 4);

    let _ = std::fs::remove_dir_all(&root);
}
