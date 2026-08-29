use std::{fs, path::PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cmrc_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn namelist(root: &std::path::Path, case: &str, n: usize, maximum_cells: usize) -> String {
    format!(
        "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{}/'\n  NL%NXP={n}\n  \
         NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  \
         NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  \
         NL%refine_backend='certified'\n  NL%mask_domain_global=.true.\n  \
         NL%landtype_file='none'\n/\n\
         &certified\n  NL%mode='safe_mother_only'\n  NL%delivery='coupled'\n  \
         NL%maximum_level=2\n  NL%maximum_cells={maximum_cells}\n  \
         NL%gradation_rings_per_level=3\n  NL%search_budget=100\n/\n",
        root.display()
    )
}

fn write_landtype(path: &std::path::Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("longitude", 360).unwrap();
    file.add_dimension("latitude", 180).unwrap();
    let mut values = (0..360 * 180)
        .map(|index| if index % 2 == 0 { 1_i8 } else { 2_i8 })
        .collect::<Vec<_>>();
    values[0] = 3;
    file.add_variable::<i8>("landtype", &["longitude", "latitude"])
        .unwrap()
        .put_values(&values, (.., ..))
        .unwrap();
}

fn landtype_namelist(root: &std::path::Path, case: &str, landtype: &std::path::Path) -> String {
    format!(
        "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{}/'\n  NL%NXP=3\n  \
         NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  \
         NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  \
         NL%refine_backend='certified'\n  NL%mask_domain_global=.true.\n  \
         NL%landtype_file='{}'\n/\n\
         &mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  \
         RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  \
         RL%max_iter_cal=1\n  RL%mask_refine_cal_type='bbox'\n  \
         RL%mask_refine_cal_fprefix='none'\n  RL%refine_num_landtypes=.true.\n  \
         RL%th_num_landtypes=1\n/\n\
         &certified\n  NL%mode='safe_mother_only'\n  NL%delivery='coupled'\n  \
         NL%maximum_level=1\n  NL%maximum_cells=1000\n  \
         NL%gradation_rings_per_level=3\n  NL%search_budget=100\n/\n",
        root.display(),
        landtype.display()
    )
}

fn specified_circle_namelist(
    root: &std::path::Path,
    case: &str,
    prefix: &std::path::Path,
) -> String {
    format!(
        "&mkgrd\n  NL%EXPNME='{case}'\n  NL%base_dir='{}/'\n  NL%NXP=3\n  \
         NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  \
         NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  \
         NL%refine_backend='certified'\n  NL%mask_domain_global=.true.\n  \
         NL%landtype_file='none'\n/\n\
         &mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  \
         RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  \
         RL%max_iter_spc=1\n  RL%mask_refine_spc_type='circle'\n  \
         RL%mask_refine_spc_fprefix='{}'\n/\n\
         &certified\n  NL%mode='safe_mother_only'\n  NL%delivery='coupled'\n  \
         NL%maximum_level=1\n  NL%maximum_cells=1000\n  \
         NL%gradation_rings_per_level=3\n  NL%search_budget=100\n/\n",
        root.display(),
        prefix.display()
    )
}

#[test]
fn safe_mother_publishes_only_after_all_hard_gates_pass() {
    let root = temp_root("success");
    let case = "certified_success";
    let path = root.join("cmrc.nml");
    fs::write(&path, namelist(&root, case, 3, 1_000)).unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.as_ref().expect("CMRC record");
    assert!(run.gridinit.is_none());
    assert_eq!(certified.mother_subdivision, 3);
    assert_eq!(certified.mother_cells, 180);
    assert_eq!(certified.physical_residuals, 0);
    assert_eq!(certified.balance_residuals, 0);
    assert_eq!(certified.topology_errors, 0);
    assert_eq!(certified.dual_errors, 0);
    assert_eq!(certified.remap_closure_errors, 0);
    assert!(run.output.output.exists());
    assert!(certified.remap.exists());
    assert!(certified.certificate.exists());
    assert!(certified.manifest.exists());
    assert!(certified.resources.exists());
    assert!(certified.ready_marker.exists());
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.resources).unwrap()).unwrap();
    assert_eq!(resources["remap_rows"], 92);
    assert_eq!(resources["remap_entries"], 92);
    assert!(resources["artifact_bytes"]["gridfile"].as_u64().unwrap() > 0);
    assert!(resources["peak_memory_bytes"].is_null());
    assert_eq!(
        fs::read_to_string(&certified.ready_marker).unwrap(),
        "certified\n"
    );
}

#[test]
fn budget_failure_leaves_no_formal_gridfile_or_ready_marker() {
    let root = temp_root("budget_failure");
    let case = "certified_budget_failure";
    let path = root.join("cmrc.nml");
    fs::write(&path, namelist(&root, case, 3, 10)).unwrap();

    let error = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect_err("budget must fail closed");
    assert!(error.to_string().contains("CellBudgetInsufficient"));
    let result = root.join(case).join("result");
    assert!(!result.join("gridfile_NXP0003_hex.nc4").exists());
    assert!(!result.join("certified_ready").exists());
}

#[test]
fn unsupported_mother_fails_and_reverse_mode_is_bounded_and_certified() {
    let root = temp_root("unsupported");
    let path = root.join("unsupported.nml");
    fs::write(&path, namelist(&root, "unsupported_n5", 5, 1_000)).unwrap();
    let error = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect_err("unsupported support-table entry must fail");
    assert!(error.to_string().contains("CriterionNotCertifiable"));
    assert!(!root
        .join("unsupported_n5/result/gridfile_NXP0005_hex.nc4")
        .exists());

    let reverse = namelist(&root, "reverse_exhausted", 3, 1_000)
        .replace("safe_mother_only", "reverse_coarsening");
    fs::write(&path, reverse).unwrap();
    let exhausted = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect("exhaustion retains the certified finer mother");
    let exhausted = exhausted.certified_run.unwrap();
    assert!(exhausted.search_budget_exhausted);
    assert_eq!(exhausted.initial_mother_subdivision, 6);
    assert_eq!(exhausted.mother_subdivision, 6);
    assert_eq!(exhausted.accepted_patches, 0);

    let reverse = namelist(&root, "reverse_complete", 3, 1_000)
        .replace("safe_mother_only", "reverse_coarsening")
        .replace("NL%search_budget=100", "NL%search_budget=200");
    fs::write(&path, reverse).unwrap();
    let completed = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect("complete hierarchy epoch must deliver");
    let completed = completed.certified_run.unwrap();
    assert!(!completed.search_budget_exhausted);
    assert_eq!(completed.initial_mother_subdivision, 6);
    assert_eq!(completed.mother_subdivision, 3);
    assert_eq!(completed.initial_mother_cells, 720);
    assert_eq!(completed.mother_cells, 180);
    assert_eq!(completed.attempted_patches, 180);
    assert_eq!(completed.accepted_patches, 180);
    assert!(completed.removed_vertices > 0);
    let remap = fs::read_to_string(&completed.remap).unwrap();
    let targets = remap
        .lines()
        .skip(1)
        .map(|line| line.split(',').next().unwrap().parse::<usize>().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(targets, (0..92).collect());
}

#[test]
fn safe_mother_consumes_landtype_requirements_before_certifying() {
    let root = temp_root("landtype_requirement");
    let landtype = root.join("landtype.nc");
    write_landtype(&landtype);
    let path = root.join("cmrc.nml");
    fs::write(
        &path,
        landtype_namelist(&root, "landtype_requirement", &landtype),
    )
    .unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.as_ref().unwrap();
    assert_eq!(certified.chosen_level, 1);
    assert_eq!(certified.mother_subdivision, 6);
    assert_eq!(certified.physical_residuals, 0);

    let missing_case = "missing_landtype";
    fs::write(
        &path,
        landtype_namelist(&root, missing_case, &root.join("missing.nc")),
    )
    .unwrap();
    assert!(earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).is_err());
    assert!(!root
        .join(missing_case)
        .join("result/gridfile_NXP0003_hex.nc4")
        .exists());
}

#[test]
#[ignore = "full raster-to-mixed-Voronoi publication acceptance"]
fn reverse_mode_publishes_a_strict_mixed_level_mesh() {
    let root = temp_root("mixed_reverse");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let prefix = sources.join("hotspot");
    earthmesh_cli::circle_close_mask_io::write_circle_mask_netcdf(
        sources.join("hotspot_001.nc4"),
        &earthmesh_cli::circle_close_mask_io::CircleMask {
            refine_degree: 1,
            points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }],
            radius_km: vec![800.0],
        },
    )
    .unwrap();
    let path = root.join("cmrc.nml");
    let namelist = specified_circle_namelist(&root, "mixed_reverse", &prefix)
        .replace("safe_mother_only", "reverse_coarsening")
        .replace("NL%search_budget=100", "NL%search_budget=4000");
    fs::write(&path, namelist).unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    assert!(certified.search_budget_exhausted);
    assert_eq!(certified.initial_mother_subdivision, 6);
    assert_eq!(certified.mother_subdivision, 6);
    assert_eq!(certified.initial_mother_cells, 720);
    assert_eq!(certified.mother_cells, 718);
    assert_eq!(certified.accepted_patches, 1);
    assert_eq!(certified.removed_vertices, 1);
    assert_eq!(certified.physical_residuals, 0);
    assert_eq!(certified.balance_residuals, 0);
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.certificate).unwrap()).unwrap();
    assert_eq!(
        certificate["coarsening_strategy"],
        "mixed_finite_cavity_with_certified_block_relocation"
    );
    assert_eq!(certificate["delivered_level_min"], 0);
    assert_eq!(certificate["delivered_level_max"], 1);
    assert_eq!(
        certificate["physical_balance_scope"],
        "final_voronoi_cells_exact_raster_overlap"
    );
}

#[test]
fn close_requirement_sources_are_order_invariant_and_artifact_deterministic() {
    let root = temp_root("close_sources");
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    let points = [
        earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 },
        earthmesh_cli::coordinate_types::LonLatPoint { lon: 5.0, lat: 0.0 },
    ];
    for (name, points) in [("forward", points), ("reverse", [points[1], points[0]])] {
        let prefix = sources.join(name);
        earthmesh_cli::circle_close_mask_io::write_circle_mask_netcdf(
            sources.join(format!("{name}_001.nc4")),
            &earthmesh_cli::circle_close_mask_io::CircleMask {
                refine_degree: 1,
                points: points.to_vec(),
                radius_km: vec![800.0; points.len()],
            },
        )
        .unwrap();
        let path = root.join(format!("{name}.nml"));
        fs::write(&path, specified_circle_namelist(&root, name, &prefix)).unwrap();
        let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
        let certified = run.certified_run.unwrap();
        assert_eq!(certified.chosen_level, 1);
        assert_eq!(certified.physical_residuals, 0);
        assert_eq!(certified.balance_residuals, 0);
    }
    let forward = root.join("forward/result");
    let reverse = root.join("reverse/result");
    assert_eq!(
        fs::read(forward.join("certified_certificate.json")).unwrap(),
        fs::read(reverse.join("certified_certificate.json")).unwrap()
    );
    assert_eq!(
        fs::read(forward.join("certified_remap.csv")).unwrap(),
        fs::read(reverse.join("certified_remap.csv")).unwrap()
    );
}

#[test]
fn tri_hex_and_coupled_delivery_share_one_certified_primal_dual_mesh() {
    let root = temp_root("delivery_modes");
    let mut artifacts = Vec::new();
    let mut mesh_counts = Vec::new();
    for (case, mode_grid, delivery) in [
        ("delivery_tri", "tri", "tri"),
        ("delivery_hex", "hex", "hex"),
        ("delivery_coupled", "hex", "coupled"),
    ] {
        let contents = namelist(&root, case, 3, 1_000)
            .replace("NL%mode_grid='hex'", &format!("NL%mode_grid='{mode_grid}'"))
            .replace(
                "NL%delivery='coupled'",
                &format!("NL%delivery='{delivery}'"),
            );
        let path = root.join(format!("{case}.nml"));
        fs::write(&path, contents).unwrap();
        let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
        let certified = run.certified_run.unwrap();
        let mesh =
            earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
                .unwrap();
        mesh_counts.push((mesh.m_points.len(), mesh.w_points.len()));
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&certified.manifest).unwrap()).unwrap();
        assert_eq!(manifest["delivery"], delivery);
        artifacts.push((
            fs::read(certified.certificate).unwrap(),
            fs::read(certified.remap).unwrap(),
        ));
    }
    assert!(artifacts
        .windows(2)
        .all(|pair| pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1));
    assert!(mesh_counts.windows(2).all(|pair| pair[0] == pair[1]));
}
