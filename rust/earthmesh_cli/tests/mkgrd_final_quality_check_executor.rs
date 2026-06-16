use std::fs;

use earthmesh_cli::{
    refine_loop_post_counts_fortran_indexed, write_unstructured_mesh_netcdf, LonLatPoint,
    MkgrdFinalQualityCheckIoPlan, MkgrdFinalQualityGlobalDistanceStepIoPlan,
    MkgrdFinalQualityGlobalSpringIoPlan, MkgrdFinalQualityRegionalSourceMaskIoPlan,
    MkgrdFinalQualityRegionalSpringIoPlan, MkgrdFinalQualitySpringMode, MkgrdRefineLoopExecutor,
    MkgrdRefineLoopWorkingStateExecutor, MkgrdRefineSourceBranchExecutor,
    MkgrdRefineSourceBranchExecutorOptions, UnstructuredMesh,
};

#[test]
fn post_refine_counts_follow_fortran_num_vertex_and_num_center_formula() {
    let mesh = UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
        ],
        w_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 4],
        m_to_w: vec![
            [1, 2, 3],
            [1, 2, 3],
            [2, 3, 4],
            [3, 4, 4],
            [2, 4, 4],
            [3, 4, 4],
        ],
        w_to_m: vec![vec![1], vec![1], vec![3], vec![4], vec![5]],
        n_w_to_m: vec![1, 1, 1, 1, 1],
    };

    let counts = refine_loop_post_counts_fortran_indexed(4, 3, 8, &mesh, 0)
        .expect("compute post-refine counts");

    assert_eq!(counts, (2, 2));
}

#[test]
fn working_state_executor_runs_global_final_quality_side_effects() {
    let root = temp_root("earthmesh_cli_final_quality_working_state");
    let plan = write_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::Global);

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("run final quality check");

    assert_final_quality_outputs(&plan, true);
    let round_trip = earthmesh_cli::read_unstructured_mesh_netcdf(
        plan.output_gridfile.as_ref().expect("output gridfile"),
    )
    .expect("read adjusted gridfile");
    assert_eq!(round_trip.m_to_w, fixture_mesh().m_to_w);
    assert_eq!(round_trip.w_to_m, fixture_mesh().w_to_m);
    assert_eq!(round_trip.n_w_to_m, fixture_mesh().n_w_to_m);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn working_state_executor_restores_compact_one_based_final_quality_gridfile_shape() {
    let root = temp_root("earthmesh_cli_final_quality_compact_gridfile");
    let plan = write_compact_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::Global);

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("run final quality check for compact generated gridfile");

    assert_final_quality_outputs(&plan, true);
    let round_trip = earthmesh_cli::read_unstructured_mesh_netcdf(
        plan.output_gridfile.as_ref().expect("output gridfile"),
    )
    .expect("read restored adjusted gridfile");
    assert_eq!(
        round_trip.m_to_w,
        compact_fixture_mesh_without_placeholders().m_to_w
    );
    assert_eq!(
        round_trip.w_to_m,
        compact_fixture_mesh_without_placeholders().w_to_m
    );
    assert_eq!(
        round_trip.n_w_to_m,
        compact_fixture_mesh_without_placeholders().n_w_to_m
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn global_final_quality_applies_configured_distance_steps() {
    let root = temp_root("earthmesh_cli_final_quality_global_distance_steps");
    let mut plan = write_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::Global);
    plan.global_spring = Some(MkgrdFinalQualityGlobalSpringIoPlan {
        distance_steps: vec![MkgrdFinalQualityGlobalDistanceStepIoPlan {
            active: true,
            halo: 1,
            refinement_flags: vec![false, false, true, false, false, false],
            num_vertex_in: 1,
            num_center_in: 1,
        }],
        ..default_global_spring_plan()
    });

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("run final quality with configured distance steps");

    let result_dir = plan
        .quality_after_spring
        .as_ref()
        .expect("after quality path")
        .parent()
        .expect("result dir");
    let dists_file = netcdf::open(result_dir.join("distsOnEdge_NXP0009_03_global.nc4"))
        .expect("open distsOnEdge output");
    let variable = dists_file
        .variable("distsOnEdge")
        .expect("distsOnEdge variable");
    let dists = variable.get_values::<f64, _>(..).expect("read distsOnEdge");

    assert!(
        dists.iter().any(|value| (*value - 1.0).abs() > 1.0e-12),
        "configured final global distance steps must alter at least one spring edge length"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn global_final_quality_ignores_padded_w_to_m_slots_for_distance_steps() {
    let root = temp_root("earthmesh_cli_final_quality_padded_w_to_m_distance_steps");
    let reference_root = temp_root("earthmesh_cli_final_quality_unpadded_w_to_m_distance_steps");
    let mut reference_plan =
        write_fixture_and_plan(&reference_root, MkgrdFinalQualitySpringMode::Global);
    reference_plan.global_spring = Some(MkgrdFinalQualityGlobalSpringIoPlan {
        distance_steps: vec![MkgrdFinalQualityGlobalDistanceStepIoPlan {
            active: true,
            halo: 1,
            refinement_flags: vec![false, false, true, false, false, false],
            num_vertex_in: 1,
            num_center_in: 1,
        }],
        ..default_global_spring_plan()
    });

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&reference_plan)
        .expect("run unpadded final quality");
    let reference_dists = read_final_quality_dists(&reference_plan);

    let mut mesh = fixture_mesh();
    for row in &mut mesh.w_to_m {
        while row.len() < 7 {
            row.push(1);
        }
    }
    let mut plan = write_mesh_and_plan(&root, mesh, MkgrdFinalQualitySpringMode::Global);
    plan.global_spring = Some(MkgrdFinalQualityGlobalSpringIoPlan {
        distance_steps: vec![MkgrdFinalQualityGlobalDistanceStepIoPlan {
            active: true,
            halo: 1,
            refinement_flags: vec![false, false, true, false, false, false],
            num_vertex_in: 1,
            num_center_in: 1,
        }],
        ..default_global_spring_plan()
    });

    executor
        .run_final_quality_check(&plan)
        .expect("run final quality with padded w_to_m rows");
    let padded_dists = read_final_quality_dists(&plan);

    assert_eq!(
        padded_dists, reference_dists,
        "padded zero slots in w_to_m must match the active n_w_to_m row semantics"
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&reference_root);
}

#[test]
fn compact_final_quality_restores_cellwidth_shape_after_global_spring() {
    let root = temp_root("earthmesh_cli_final_quality_compact_cellwidth");
    let mut plan = write_compact_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::Global);
    plan.global_spring = Some(MkgrdFinalQualityGlobalSpringIoPlan {
        base_cellwidth: Some(120.0),
        ..default_global_spring_plan()
    });

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("run compact final quality with cellwidth");

    let output = earthmesh_cli::read_unstructured_mesh_netcdf(
        plan.output_gridfile.as_ref().expect("output gridfile"),
    )
    .expect("read output gridfile");
    let cellwidth =
        earthmesh_cli::read_cellwidth_netcdf(root.join("result/cellwidth_NXP0009_global.nc4"))
            .expect("read restored cellwidth");
    assert_eq!(cellwidth.len(), output.w_points.len());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn global_final_quality_keeps_cellwidth_pre_spring_cell_coordinates() {
    let root = temp_root("earthmesh_cli_final_quality_cellwidth_prespring_coords");
    let mut plan = write_compact_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::Global);
    plan.global_spring = Some(MkgrdFinalQualityGlobalSpringIoPlan {
        base_cellwidth: Some(120.0),
        niter_refine: 1,
        ..default_global_spring_plan()
    });

    let input = compact_fixture_mesh_without_placeholders();
    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("run compact final quality with cellwidth");

    let file = netcdf::open(root.join("result/cellwidth_NXP0009_global.nc4"))
        .expect("open cellwidth output");
    let lonw = file
        .variable("lonw")
        .expect("lonw variable")
        .get_values::<f64, _>(..)
        .expect("read lonw");
    let latw = file
        .variable("latw")
        .expect("latw variable")
        .get_values::<f64, _>(..)
        .expect("read latw");

    let expected_lonw = input
        .w_points
        .iter()
        .map(|point| point.lon)
        .collect::<Vec<_>>();
    let expected_latw = input
        .w_points
        .iter()
        .map(|point| point.lat)
        .collect::<Vec<_>>();
    assert_eq!(lonw, expected_lonw);
    assert_eq!(latw, expected_latw);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fortran_placeholder_final_quality_preserves_connectivity_ids() {
    let root = temp_root("earthmesh_cli_final_quality_fortran_placeholder");
    let input_mesh = fortran_placeholder_fixture_mesh();
    let input_gridfile = root.join("gridfile/gridfile_NXP0009_03_hex.nc4");
    write_unstructured_mesh_netcdf(&input_gridfile, &input_mesh).expect("write input gridfile");
    let plan = MkgrdFinalQualityCheckIoPlan {
        step: 3,
        run_quality_check: true,
        spring_mode: MkgrdFinalQualitySpringMode::Global,
        input_gridfile: input_gridfile.clone(),
        original_gridfile: Some(root.join("gridfile/gridfile_NXP0009_03_hex_orial.nc4")),
        quality_before_spring: Some(root.join("result/quality_NXP0009_03_global_beforeSpring.nc4")),
        quality_after_spring: Some(root.join("result/quality_NXP0009_03_global.nc4")),
        output_gridfile: Some(input_gridfile),
        regional_set_dis: None,
        global_spring: Some(MkgrdFinalQualityGlobalSpringIoPlan {
            base_cellwidth: Some(120.0),
            ..default_global_spring_plan()
        }),
        regional_spring: None,
        regional_source_mask: None,
    };

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("run final quality for fortran placeholder gridfile");

    let output = earthmesh_cli::read_unstructured_mesh_netcdf(
        plan.output_gridfile.as_ref().expect("output gridfile"),
    )
    .expect("read output gridfile");
    assert_eq!(output.m_to_w, input_mesh.m_to_w);
    assert_eq!(output.w_to_m, input_mesh.w_to_m);
    assert_eq!(output.n_w_to_m, input_mesh.n_w_to_m);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn source_branch_executor_reuses_final_quality_side_effects_without_source_options() {
    let root = temp_root("earthmesh_cli_final_quality_branch_executor");
    let plan = write_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::Global);

    let mut executor =
        MkgrdRefineSourceBranchExecutor::new(MkgrdRefineSourceBranchExecutorOptions {
            calculated: None,
            specified: None,
        });
    executor
        .run_final_quality_check(&plan)
        .expect("run final quality check through branch executor");

    assert_final_quality_outputs(&plan, true);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_final_quality_requires_source_mask_classification_inputs() {
    let root = temp_root("earthmesh_cli_final_quality_regional_requires_mask");
    let mut plan = write_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::RegionalFinal);
    plan.regional_set_dis = Some(1);
    plan.regional_spring = Some(MkgrdFinalQualityRegionalSpringIoPlan {
        niter_refine: 0,
        radius: earthmesh_core::EARTH_RADIUS_METERS,
    });

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    let err = executor
        .run_final_quality_check(&plan)
        .expect_err("regional final quality must require source-mask classification inputs");

    assert!(err.to_string().contains("source mask"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_final_quality_runs_with_source_mask_classification_inputs() {
    let root = temp_root("earthmesh_cli_final_quality_regional_source_mask");
    let mut plan = write_fixture_and_plan(&root, MkgrdFinalQualitySpringMode::RegionalFinal);
    plan.regional_set_dis = Some(0);
    plan.regional_spring = Some(MkgrdFinalQualityRegionalSpringIoPlan {
        niter_refine: 0,
        radius: earthmesh_core::EARTH_RADIUS_METERS,
    });
    plan.regional_source_mask = Some(single_cell_source_mask());

    let mut executor = MkgrdRefineLoopWorkingStateExecutor::default();
    executor
        .run_final_quality_check(&plan)
        .expect("regional final quality source-mask path runs");

    assert_final_quality_outputs(&plan, false);
    let _ = fs::remove_dir_all(&root);
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{prefix}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gridfile")).expect("create gridfile dir");
    root
}

fn write_fixture_and_plan(
    root: &std::path::Path,
    spring_mode: MkgrdFinalQualitySpringMode,
) -> MkgrdFinalQualityCheckIoPlan {
    write_mesh_and_plan(root, fixture_mesh(), spring_mode)
}

fn write_mesh_and_plan(
    root: &std::path::Path,
    mesh: UnstructuredMesh,
    spring_mode: MkgrdFinalQualitySpringMode,
) -> MkgrdFinalQualityCheckIoPlan {
    let input_gridfile = root.join("gridfile/gridfile_NXP0009_03_hex.nc4");
    write_unstructured_mesh_netcdf(&input_gridfile, &mesh).expect("write input gridfile");
    MkgrdFinalQualityCheckIoPlan {
        step: 3,
        run_quality_check: true,
        spring_mode,
        input_gridfile: input_gridfile.clone(),
        original_gridfile: Some(root.join("gridfile/gridfile_NXP0009_03_hex_orial.nc4")),
        quality_before_spring: Some(root.join("result/quality_NXP0009_03_global_beforeSpring.nc4")),
        quality_after_spring: Some(root.join("result/quality_NXP0009_03_global.nc4")),
        output_gridfile: Some(input_gridfile),
        regional_set_dis: None,
        global_spring: (spring_mode == MkgrdFinalQualitySpringMode::Global)
            .then(default_global_spring_plan),
        regional_spring: None,
        regional_source_mask: None,
    }
}

fn write_compact_fixture_and_plan(
    root: &std::path::Path,
    spring_mode: MkgrdFinalQualitySpringMode,
) -> MkgrdFinalQualityCheckIoPlan {
    let input_gridfile = root.join("gridfile/gridfile_NXP0009_03_hex.nc4");
    write_unstructured_mesh_netcdf(
        &input_gridfile,
        &compact_fixture_mesh_without_placeholders(),
    )
    .expect("write compact input gridfile");
    MkgrdFinalQualityCheckIoPlan {
        step: 3,
        run_quality_check: true,
        spring_mode,
        input_gridfile: input_gridfile.clone(),
        original_gridfile: Some(root.join("gridfile/gridfile_NXP0009_03_hex_orial.nc4")),
        quality_before_spring: Some(root.join("result/quality_NXP0009_03_global_beforeSpring.nc4")),
        quality_after_spring: Some(root.join("result/quality_NXP0009_03_global.nc4")),
        output_gridfile: Some(input_gridfile),
        regional_set_dis: None,
        global_spring: (spring_mode == MkgrdFinalQualitySpringMode::Global)
            .then(default_global_spring_plan),
        regional_spring: None,
        regional_source_mask: None,
    }
}

fn default_global_spring_plan() -> MkgrdFinalQualityGlobalSpringIoPlan {
    MkgrdFinalQualityGlobalSpringIoPlan {
        base_dists_on_edge: 1.0,
        base_cellwidth: None,
        distance_num_rc: 0,
        distance_spacing: earthmesh_mesh::DistanceLayerSpacing::Linear,
        distance_steps: Vec::new(),
        niter_refine: 0,
        relax: 0.04,
        radius: earthmesh_core::EARTH_RADIUS_METERS,
    }
}

fn assert_final_quality_outputs(
    plan: &MkgrdFinalQualityCheckIoPlan,
    expect_global_spring_files: bool,
) {
    assert!(plan
        .original_gridfile
        .as_ref()
        .expect("original path")
        .exists());
    assert!(plan
        .quality_before_spring
        .as_ref()
        .expect("before quality path")
        .exists());
    assert!(plan
        .quality_after_spring
        .as_ref()
        .expect("after quality path")
        .exists());
    assert!(plan.output_gridfile.as_ref().expect("output path").exists());
    if expect_global_spring_files {
        let result_dir = plan
            .quality_after_spring
            .as_ref()
            .expect("after quality path")
            .parent()
            .expect("result dir");
        assert!(result_dir
            .join("distsOnEdge_NXP0009_03_global.nc4")
            .exists());
    }
}

fn read_final_quality_dists(plan: &MkgrdFinalQualityCheckIoPlan) -> Vec<f64> {
    let result_dir = plan
        .quality_after_spring
        .as_ref()
        .expect("after quality path")
        .parent()
        .expect("result dir");
    let dists_file = netcdf::open(result_dir.join("distsOnEdge_NXP0009_03_global.nc4"))
        .expect("open distsOnEdge output");
    let variable = dists_file
        .variable("distsOnEdge")
        .expect("distsOnEdge variable");
    variable.get_values::<f64, _>(..).expect("read distsOnEdge")
}

fn single_cell_source_mask() -> MkgrdFinalQualityRegionalSourceMaskIoPlan {
    MkgrdFinalQualityRegionalSourceMaskIoPlan {
        source_lon_vertices: vec![0.0, 0.0, 0.5, 1.0],
        source_lat_vertices: vec![0.0, 0.0, 0.5, 1.0],
        mask_patch: vec![
            vec![false, false, false, false],
            vec![false, true, false, false],
            vec![false, false, false, false],
            vec![false, false, false, false],
        ],
        first_triangle_id: 2,
    }
}

fn fixture_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.2, lat: 0.2 },
            LonLatPoint { lon: 0.8, lat: 0.2 },
            LonLatPoint { lon: 0.2, lat: 0.8 },
            LonLatPoint { lon: 0.8, lat: 0.8 },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
        ],
        m_to_w: vec![
            [1, 1, 1],
            [1, 1, 1],
            [2, 3, 4],
            [2, 3, 5],
            [2, 4, 5],
            [3, 4, 5],
        ],
        w_to_m: vec![
            vec![1],
            vec![1],
            vec![2, 3, 4],
            vec![2, 3, 5],
            vec![2, 4, 5],
            vec![3, 4, 5],
        ],
        n_w_to_m: vec![1, 1, 3, 3, 3, 3],
    }
}

fn fortran_placeholder_fixture_mesh() -> UnstructuredMesh {
    let legacy = fixture_mesh();
    UnstructuredMesh {
        m_points: std::iter::once(legacy.m_points[0])
            .chain(legacy.m_points[2..].iter().copied())
            .collect(),
        w_points: std::iter::once(legacy.w_points[0])
            .chain(legacy.w_points[2..].iter().copied())
            .collect(),
        m_to_w: std::iter::once([1, 1, 1])
            .chain(legacy.m_to_w[2..].iter().copied())
            .collect(),
        w_to_m: std::iter::once(vec![1])
            .chain(legacy.w_to_m[2..].iter().cloned())
            .collect(),
        n_w_to_m: std::iter::once(0)
            .chain(legacy.n_w_to_m[2..].iter().copied())
            .collect(),
    }
}

fn compact_fixture_mesh_without_placeholders() -> UnstructuredMesh {
    let legacy = fixture_mesh();
    UnstructuredMesh {
        m_points: legacy.m_points[2..].to_vec(),
        w_points: legacy.w_points[2..].to_vec(),
        m_to_w: legacy.m_to_w[2..]
            .iter()
            .map(|row| row.map(|value| value - 1))
            .collect(),
        w_to_m: legacy.w_to_m[2..]
            .iter()
            .map(|row| row.iter().map(|value| value - 1).collect())
            .collect(),
        n_w_to_m: legacy.n_w_to_m[2..].to_vec(),
    }
}
