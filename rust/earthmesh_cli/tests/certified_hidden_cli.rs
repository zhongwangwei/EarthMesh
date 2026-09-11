use std::{collections::BTreeMap, fs, path::PathBuf};

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

fn write_land_and_ocean(path: &std::path::Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("longitude", 360).unwrap();
    file.add_dimension("latitude", 180).unwrap();
    let values = (0..360 * 180)
        .map(|index| if index % 2 == 0 { 0_i8 } else { 1_i8 })
        .collect::<Vec<_>>();
    file.add_variable::<i8>("landtype", &["longitude", "latitude"])
        .unwrap()
        .put_values(&values, (.., ..))
        .unwrap();
}

fn write_all_ocean(path: &std::path::Path) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("longitude", 360).unwrap();
    file.add_dimension("latitude", 180).unwrap();
    let values = vec![0_i8; 360 * 180];
    file.add_variable::<i8>("landtype", &["longitude", "latitude"])
        .unwrap()
        .put_values(&values, (.., ..))
        .unwrap();
}

fn write_close_domain(path: &std::path::Path) {
    fs::write(
        path,
        "close_num = 4\nclose_refine = 0\n100.0 0.0\n160.0 0.0\n160.0 50.0\n100.0 50.0\n",
    )
    .unwrap();
}

fn has_two_placeholders(points: &[earthmesh_cli::coordinate_types::LonLatPoint]) -> bool {
    points.len() > 1
        && points[0].lon == 0.0
        && points[0].lat == 0.0
        && points[1].lon == 0.0
        && points[1].lat == 0.0
}

fn row_for_canonical_id(canonical_id: i64, rows: usize, two_placeholders: bool) -> Option<usize> {
    if canonical_id <= 0 {
        return None;
    }
    let row = if two_placeholders {
        canonical_id as usize
    } else {
        canonical_id as usize - 1
    };
    (row < rows).then_some(row)
}

fn assert_regional_triangles_are_whole_global_subset(
    regional: &earthmesh_cli::unstructured_mesh_support::UnstructuredMesh,
    lineages: &earthmesh_cli::unstructured_mesh_support::MethodCGridfileLineages,
    global: &earthmesh_cli::unstructured_mesh_support::UnstructuredMesh,
) {
    let global_m_two = has_two_placeholders(&global.m_points);
    let global_w_two = has_two_placeholders(&global.w_points);
    let regional_w_two = has_two_placeholders(&regional.w_points);
    let first_m = if has_two_placeholders(&regional.m_points) {
        2
    } else {
        1
    };
    assert_eq!(lineages.m.len(), regional.m_points.len());
    assert_eq!(lineages.w.len(), regional.w_points.len());
    let mut checked = 0usize;
    for (m_row, triangle) in regional.m_to_w.iter().enumerate().skip(first_m) {
        let source_m_id = lineages.m[m_row];
        assert!(
            source_m_id >= 2,
            "physical triangle {m_row} lost its lineage"
        );
        let global_m_row = row_for_canonical_id(source_m_id, global.m_points.len(), global_m_two)
            .expect("every physical triangle has a valid global source");
        let source_triangle = global.m_to_w[global_m_row];
        let mapped_vertices = triangle.map(|w_id| {
            let row = row_for_canonical_id(w_id as i64, regional.w_points.len(), regional_w_two)
                .expect("regional vertex row");
            assert!(
                lineages.w[row] >= 2,
                "physical vertex {row} lost its lineage"
            );
            i32::try_from(lineages.w[row]).expect("global vertex ID fits canonical format")
        });
        assert_eq!(mapped_vertices, source_triangle);
        assert_eq!(regional.m_points[m_row], global.m_points[global_m_row]);
        for (&regional_w_id, &global_w_id) in triangle.iter().zip(source_triangle.iter()) {
            let regional_w_row = row_for_canonical_id(
                regional_w_id as i64,
                regional.w_points.len(),
                regional_w_two,
            )
            .expect("regional W row");
            let global_w_row =
                row_for_canonical_id(global_w_id as i64, global.w_points.len(), global_w_two)
                    .expect("global W row");
            assert_eq!(
                regional.w_points[regional_w_row],
                global.w_points[global_w_row]
            );
        }
        checked += 1;
    }
    assert!(checked > 0);
    assert_eq!(checked, regional.m_to_w.len() - first_m);
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

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}

fn assert_mpas_unit_sphere(path: &std::path::Path) {
    let file = netcdf::open(path).expect("open MPAS mesh");
    let radius: f64 = file
        .attribute("sphere_radius")
        .expect("sphere_radius")
        .value()
        .expect("read sphere_radius")
        .try_into()
        .expect("f64 attr");
    assert_eq!(radius, 1.0);
    assert!(read_f64(&file, "xCell")
        .iter()
        .all(|value| value.abs() <= 1.0 + 1.0e-12));
}

fn assert_graph_header_matches_mesh(graph: &std::path::Path, mesh: &std::path::Path) {
    let file = netcdf::open(mesh).expect("open MPAS mesh");
    let n_cells = file.dimension("nCells").expect("nCells").len();
    let cells_on_edge = file
        .variable("cellsOnEdge")
        .expect("cellsOnEdge")
        .get_values::<i32, _>(..)
        .expect("read cellsOnEdge");
    let interior_edges = cells_on_edge
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|edge| edge[0] > 0 && edge[1] > 0)
        .count();
    let graph_text = fs::read_to_string(graph).expect("read graph.info");
    let header = graph_text.lines().next().expect("graph header");
    let values = header
        .split_whitespace()
        .map(|value| value.parse::<usize>().expect("graph header usize"))
        .collect::<Vec<_>>();
    assert_eq!(values, vec![n_cells, interior_edges]);
}

fn artifact_bytes(paths: &[&std::path::Path]) -> BTreeMap<PathBuf, Vec<u8>> {
    paths
        .iter()
        .map(|path| {
            (
                (*path).to_path_buf(),
                fs::read(path).expect("read artifact"),
            )
        })
        .collect()
}

fn assert_artifacts_unchanged(before: &BTreeMap<PathBuf, Vec<u8>>) {
    for (path, bytes) in before {
        assert_eq!(
            &fs::read(path).expect("read restored artifact"),
            bytes,
            "{path:?}"
        );
    }
}

fn assert_no_cmrc_temporaries(result: &std::path::Path) {
    for entry in fs::read_dir(result).expect("read result dir") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .to_string();
        assert!(
            !name.contains("cmrc-tmp") && !name.contains("cmrc-backup"),
            "left temporary artifact {name}"
        );
    }
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
    assert!(certified.remap.as_ref().is_some_and(|path| path.exists()));
    assert!(certified.pre_export_remap.is_none());
    assert!(certified.certificate.exists());
    assert!(certified.manifest.exists());
    assert!(certified.resources.exists());
    assert!(certified.ready_marker.exists());
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.resources).unwrap()).unwrap();
    assert_eq!(
        resources["requirement_layers"]["policy"],
        "effective_raster_remains_hard"
    );
    assert_eq!(
        resources["requirement_layers"]["raw_source_raster"]["histogram"],
        serde_json::json!({"0": 8})
    );
    assert!(resources["requirement_layers"]["threshold_sources"].is_null());
    assert_eq!(
        resources["requirement_layers"]["graph_scheduling_target"]["status"],
        "not_applied"
    );
    assert_eq!(resources["remap_rows"], 92);
    assert_eq!(resources["remap_entries"], 92);
    assert!(resources["artifact_bytes"]["gridfile"].as_u64().unwrap() > 0);
    assert!(resources["peak_memory_bytes"].is_null());
    assert_eq!(
        fs::read_to_string(&certified.ready_marker).unwrap(),
        "certified_adaptive\n"
    );
}

#[test]
fn certified_atmos_mpas_safe_mother_publishes_mesh_and_graph_atomically() {
    let root = temp_root("atmos_mpas_safe");
    let case = "atmos_mpas_safe";
    let path = root.join("cmrc.nml");
    let contents = namelist(&root, case, 3, 1_000)
        .replace("NL%mesh_type='earthmesh'", "NL%mesh_type='atmosmesh'")
        .replace("NL%output_format='CoLM'", "NL%output_format='MPAS'")
        .replace("NL%delivery='coupled'", "NL%delivery='hex'");
    fs::write(&path, contents).unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    let result = root.join(case).join("result");
    let mpas = result.join("MPASOUT_NXP0003_global.nc4");
    let graph = result.join("MPASOUT_NXP0003_global.graph.info");
    assert!(mpas.exists());
    assert!(graph.exists());
    assert_mpas_unit_sphere(&mpas);
    assert_graph_header_matches_mesh(&graph, &mpas);
    let file = netcdf::open(&mpas).expect("open MPAS mesh");
    let density = read_f64(&file, "meshDensity");
    assert_eq!(density.len(), file.dimension("nCells").unwrap().len());
    assert!(density.iter().all(|value| *value == 1.0));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.manifest).unwrap()).unwrap();
    assert_eq!(manifest["mpas"], mpas.display().to_string());
    assert_eq!(manifest["mpas_graph_info"], graph.display().to_string());
    assert_eq!(manifest["mpas_sphere_radius"], 1.0);
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.resources).unwrap()).unwrap();
    assert_eq!(resources["mpas"]["mesh"], mpas.display().to_string());
    assert_eq!(resources["mpas"]["graph_info"], graph.display().to_string());
    assert!(resources["artifact_bytes"]["mpas"].as_u64().unwrap() > 0);
    assert!(
        resources["artifact_bytes"]["mpas_graph_info"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn certified_atmos_mpas_adaptive_density_uses_delivered_refinement_levels() {
    let root = temp_root("atmos_mpas_adaptive");
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
    let namelist = specified_circle_namelist(&root, "atmos_mpas_adaptive", &prefix)
        .replace("NL%mesh_type='earthmesh'", "NL%mesh_type='atmosmesh'")
        .replace("NL%output_format='CoLM'", "NL%output_format='MPAS'")
        .replace("safe_mother_only", "reverse_coarsening")
        .replace("NL%delivery='coupled'", "NL%delivery='hex'")
        .replace(
            "NL%delivery='hex'",
            "NL%delivery='hex'\n  NL%angle_contract='domain_quality_38_to_82_v1'",
        )
        .replace("NL%search_budget=100", "NL%search_budget=4000");
    fs::write(&path, namelist).unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    assert!(certified.fulfillment.mixed_levels_delivered);
    let mpas = root
        .join("atmos_mpas_adaptive")
        .join("result/MPASOUT_NXP0003_global.nc4");
    let file = netcdf::open(&mpas).expect("open MPAS mesh");
    let density = read_f64(&file, "meshDensity");
    assert!(density
        .iter()
        .all(|value| value.is_finite() && *value > 0.0));
    assert!(density.iter().any(|value| (*value - 1.0).abs() < 1.0e-12));
    assert!(density
        .iter()
        .any(|value| (*value - 0.0625).abs() < 1.0e-12));
    assert!(
        density
            .iter()
            .any(|value| (*value - density[0]).abs() > 1.0e-12),
        "adaptive CMRC MPAS must not synthesize all-one meshDensity"
    );
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.resources).unwrap()).unwrap();
    assert_eq!(resources["mpas"]["mesh_density_min"], 0.0625);
    assert_eq!(resources["mpas"]["mesh_density_max"], 1.0);
    assert_eq!(resources["mpas"]["step"], 2);
    let gridfile =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
            .unwrap();
    let placeholder_rows = gridfile.w_refine_level.len() - density.len();
    assert!(placeholder_rows <= 2);
    for (level, actual) in gridfile.w_refine_level[placeholder_rows..]
        .iter()
        .zip(density)
    {
        let expected = 0.0625_f64.powi(1 - level);
        assert!(
            (actual - expected).abs() < 1.0e-12,
            "level {level} density {actual} expected {expected}"
        );
    }
}

#[test]
fn certified_atmos_mpas_publication_failure_restores_prior_complete_bundle() {
    let root = temp_root("atmos_mpas_rollback");
    let case = "atmos_mpas_rollback";
    let path = root.join("cmrc.nml");
    let contents = namelist(&root, case, 3, 1_000)
        .replace("NL%mesh_type='earthmesh'", "NL%mesh_type='atmosmesh'")
        .replace("NL%output_format='CoLM'", "NL%output_format='MPAS'")
        .replace("NL%delivery='coupled'", "NL%delivery='hex'");
    fs::write(&path, contents).unwrap();

    let first = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect("initial complete MPAS bundle");
    let certified = first.certified_run.unwrap();
    let result = root.join(case).join("result");
    let grid = first.output.output;
    let mpas = result.join("MPASOUT_NXP0003_global.nc4");
    let graph = result.join("MPASOUT_NXP0003_global.graph.info");
    let ready = certified.ready_marker;
    let before = artifact_bytes(&[
        grid.as_path(),
        mpas.as_path(),
        certified.certificate.as_path(),
        certified.manifest.as_path(),
        certified.resources.as_path(),
        ready.as_path(),
    ]);

    fs::remove_file(&graph).unwrap();
    fs::create_dir(&graph).unwrap();
    let error = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect_err("directory at graph path must make publication fail");
    assert!(error
        .to_string()
        .contains("CMRC atomic artifact publication failed"));
    fs::remove_dir(&graph).unwrap();
    assert_artifacts_unchanged(&before);
    assert_no_cmrc_temporaries(&result);
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
        .expect_err("zero coarsening progress must not publish ordinary success");
    assert!(exhausted.to_string().contains("CompressionIncomplete"));
    let exhausted_result = root.join("reverse_exhausted/result");
    assert!(!exhausted_result.join("gridfile_NXP0003_hex.nc4").exists());
    assert!(!exhausted_result.join("certified_ready").exists());

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
    assert_eq!(completed.product_outcome, "certified_adaptive");
    assert_eq!(completed.fulfillment.components_committed, 180);
    assert_eq!(completed.attempted_patches, 180);
    assert_eq!(completed.accepted_patches, 180);
    assert!(completed.removed_vertices > 0);
    let remap = fs::read_to_string(completed.remap.as_ref().unwrap()).unwrap();
    let targets = remap
        .lines()
        .skip(1)
        .map(|line| line.split(',').next().unwrap().parse::<usize>().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(targets, (0..92).collect());
}

#[test]
fn mixed_uniform_delivery_fails_closed_or_uses_an_explicitly_named_safe_fallback() {
    let root = temp_root("mixed_fulfillment");
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
    let reverse = specified_circle_namelist(&root, "mixed_incomplete", &prefix)
        .replace("safe_mother_only", "reverse_coarsening")
        .replace("NL%search_budget=100", "NL%search_budget=1");
    fs::write(&path, reverse).unwrap();
    let error = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect_err("mixed request with uniform delivery must fail closed");
    assert!(error.to_string().contains("CompressionIncomplete"));
    let incomplete = root.join("mixed_incomplete/result");
    for artifact in [
        "gridfile_NXP0003_hex.nc4",
        "certified_remap.csv",
        "certified_certificate.json",
        "certified_manifest.json",
        "certified_resources.json",
        "certified_ready",
    ] {
        assert!(!incomplete.join(artifact).exists(), "unexpected {artifact}");
    }

    fs::write(
        &path,
        specified_circle_namelist(&root, "mixed_safe_fallback", &prefix)
            .replace("NL%mesh_type='earthmesh'", "NL%mesh_type='atmosmesh'")
            .replace("NL%output_format='CoLM'", "NL%output_format='MPAS'")
            .replace("NL%delivery='coupled'", "NL%delivery='hex'"),
    )
    .unwrap();
    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect("safe_mother_only explicitly permits a labeled safe fallback");
    let certified = run.certified_run.unwrap();
    assert_eq!(certified.product_outcome, "certified_safe_fallback");
    assert!(certified
        .safe_fallback_reason
        .as_deref()
        .unwrap()
        .contains("mixed levels"));
    assert!(certified.fulfillment.mixed_levels_requested);
    assert!(!certified.fulfillment.mixed_levels_delivered);
    assert_eq!(
        run.output.output.file_name().unwrap().to_str().unwrap(),
        "gridfile_NXP0003_hex_certified_safe_fallback.nc4"
    );
    assert!(root
        .join("mixed_safe_fallback/result/MPASOUT_NXP0003_global_certified_safe_fallback.nc4")
        .exists());
    assert!(root
        .join(
            "mixed_safe_fallback/result/MPASOUT_NXP0003_global_certified_safe_fallback.graph.info"
        )
        .exists());
    assert_eq!(
        fs::read_to_string(&certified.ready_marker).unwrap(),
        "certified_safe_fallback\n"
    );
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.certificate).unwrap()).unwrap();
    assert_eq!(certificate["product_outcome"], "certified_safe_fallback");
    assert_eq!(
        certificate["adaptivity_fulfillment"]["mixed_levels_delivered"],
        false
    );
    assert!(!root
        .join("mixed_safe_fallback/result/gridfile_NXP0003_hex.nc4")
        .exists());
}

#[test]
fn safe_mother_consumes_landtype_requirements_before_certifying() {
    let root = temp_root("landtype_requirement");
    let landtype = root.join("landtype.nc");
    write_landtype(&landtype);
    let path = root.join("cmrc.nml");
    fs::write(
        &path,
        landtype_namelist(&root, "landtype_requirement", &landtype)
            .replace("landmesh", "earthmesh"),
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
        landtype_namelist(&root, missing_case, &root.join("missing.nc"))
            .replace("landmesh", "earthmesh"),
    )
    .unwrap();
    assert!(earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).is_err());
    assert!(!root
        .join(missing_case)
        .join("result/gridfile_NXP0003_hex.nc4")
        .exists());
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.certificate).unwrap()).unwrap();
    let layers = &certificate["requirement_layers"];
    assert_eq!(
        layers["raw_source_raster"]["status"],
        "unavailable_threshold_or_hydro"
    );
    assert!(layers["raw_source_raster"]["histogram"].is_null());
    assert!(layers["effective_source_raster"]["raised_samples_over_raw"].is_null());
    assert_eq!(layers["policy"], "effective_raster_remains_hard");
    assert_eq!(
        layers["threshold_sources"]["scope"],
        "threshold_only_before_gradient"
    );
    assert!(layers["threshold_sources"]["criteria"].is_array());
    assert!(layers["threshold_sources"]["raw_threshold_histogram"].is_object());
    assert!(layers["threshold_sources"]["composition_timing_ms"].is_null());
    assert!(!layers["threshold_sources"]
        .to_string()
        .contains("composition_timing_ms"));
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.resources).unwrap()).unwrap();
    assert_eq!(layers, &resources["requirement_layers"]);
}

#[test]
fn certified_ocean_output_is_masked_and_boundary_checked() {
    let root = temp_root("ocean_mask_gate");
    let landtype = root.join("landtype.nc");
    write_land_and_ocean(&landtype);
    let path = root.join("cmrc.nml");
    let case = "ocean_mask_gate";
    let namelist = landtype_namelist(&root, case, &landtype)
        .replace("landmesh", "oceanmesh")
        .replace("mode_grid='hex'", "mode_grid='tri'")
        .replace("output_format='CoLM'", "output_format='FVCOM'");
    fs::write(&path, namelist).unwrap();
    let result_dir = root.join(case).join("result");
    fs::create_dir_all(&result_dir).unwrap();
    let stale_remap = result_dir.join("certified_remap.csv");
    fs::write(&stale_remap, "stale final-domain remap").unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let kept = run.landtype_masked_cells.expect("masked ocean cell count");
    assert!(kept > 0);
    assert!(kept < run.certified_run.as_ref().unwrap().mother_cells);
    assert_eq!(
        run.output.output.file_name().unwrap().to_str().unwrap(),
        "gridfile_NXP0003_tri_oceanmesh.nc4"
    );
    let certified = run.certified_run.unwrap();
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.certificate).unwrap()).unwrap();
    assert_eq!(certificate["geometry_scope"], "pre_export_closed_sphere");
    assert_eq!(certificate["published_grid_is_certified_face_subset"], true);
    assert_eq!(certificate["published_grid_remap_available"], false);
    assert_eq!(
        certificate["published_domain_geometry"]["contract_pass"],
        true
    );
    assert!(
        certificate["published_domain_geometry"]["minimum_angle_deg"]
            .as_f64()
            .is_some_and(|angle| angle >= 40.0)
    );
    assert!(
        certificate["published_domain_geometry"]["maximum_angle_deg"]
            .as_f64()
            .is_some_and(|angle| angle <= 80.0)
    );
    assert_eq!(
        certificate["remap_scope"],
        "pre_export_closed_sphere_voronoi"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.manifest).unwrap()).unwrap();
    assert!(manifest["remap"].is_null());
    assert!(certified.remap.is_none());
    let pre_export_remap = certified.pre_export_remap.as_ref().unwrap();
    assert_eq!(
        manifest["pre_export_remap"],
        pre_export_remap.display().to_string()
    );
    assert!(pre_export_remap.exists());
    assert!(!stale_remap.exists());
    let gridfile =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
            .unwrap();
    assert_eq!(gridfile.w_refine_level.len(), gridfile.w_lon.len());
    assert_eq!(gridfile.m_refine_level.len(), gridfile.m_lon.len());
    let quality_input =
        earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile(&gridfile).unwrap();
    let hard_issues = earthmesh_quality::topology::MeshTopologyValidator::new(&quality_input)
        .validate_all()
        .into_iter()
        .filter(|issue| issue.severity == earthmesh_quality::topology::Severity::Fail)
        .collect::<Vec<_>>();
    assert!(
        hard_issues.is_empty(),
        "published CMRC ocean topology issues: {hard_issues:?}"
    );
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.resources).unwrap()).unwrap();
    assert_eq!(resources["landtype_masked_cells"], kept);
    assert_eq!(
        resources["published_domain_quality_topology"]["connected_components"],
        1
    );
    assert_eq!(
        resources["published_domain_geometry"]["contract_pass"],
        true
    );
    assert_eq!(
        resources["published_domain_topology"]["violations"],
        serde_json::json!([])
    );
    assert!(
        resources["published_domain_topology"]["boundary_loops"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn certified_refine_false_uses_uniform_level_zero_safe_mother() {
    let root = temp_root("refine_false_uniform");
    let path = root.join("cmrc.nml");
    let contents = format!(
        "{}\n&mkrefine\n  RL%refine_cal=.true.\n  RL%mask_refine_cal_type='not_a_real_criterion'\n/\n&hfield\n  NL%hfield_gr=0.15\n/\n",
        namelist(&root, "refine_false_uniform", 3, 1_000)
            .replace("NL%refine=.true.", "NL%refine=.false.")
            .replace("NL%mode='safe_mother_only'", "NL%mode='reverse_coarsening'")
    );
    fs::write(&path, contents).unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    assert_eq!(certified.mode, "safe_mother_only");
    assert_eq!(certified.chosen_level, 0);
    assert_eq!(certified.delivered_level, 0);
    assert_eq!(certified.initial_mother_subdivision, 3);
    assert_eq!(certified.mother_subdivision, 3);
    assert_eq!(certified.attempted_patches, 0);
    assert_eq!(certified.removed_faces, 0);
}

#[test]
fn certified_close_ocean_publishes_regional_fvcom_after_global_certificate() {
    let root = temp_root("regional_ocean");
    let landtype = root.join("landtype.nc");
    write_all_ocean(&landtype);
    let close = root.join("domain.nml");
    write_close_domain(&close);
    let path = root.join("cmrc.nml");
    let regional_namelist = landtype_namelist(&root, "regional_ocean", &landtype)
        .replace("landmesh", "oceanmesh")
        .replace("mode_grid='hex'", "mode_grid='tri'")
        .replace("output_format='CoLM'", "output_format='FVCOM'")
        .replace(
            "NL%mask_domain_global=.true.",
            "NL%mask_domain_global=.false.",
        )
        .replace(
            "NL%landtype_file=",
            &format!(
                "NL%mask_domain_type='close'\n  NL%mask_domain_fprefix='{}'\n  NL%landtype_file=",
                close.display()
            ),
        );
    fs::write(&path, regional_namelist).unwrap();
    let global_path = root.join("global_tri.nml");
    let global_contents = namelist(&root, "regional_ocean_global_ref", 3, 1_000)
        .replace("NL%mode_grid='hex'", "NL%mode_grid='tri'")
        .replace("NL%delivery='coupled'", "NL%delivery='tri'");
    fs::write(&global_path, global_contents).unwrap();
    let global_run =
        earthmesh_cli::run_refine_pipeline_namelist(&global_path, &root, 1_000, None).unwrap();
    let global_mesh = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &global_run.output.output,
    )
    .unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let gridfile =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
            .unwrap();
    assert_eq!(gridfile.m_refine_level.len(), gridfile.m_lon.len());
    assert_eq!(gridfile.w_refine_level.len(), gridfile.w_lon.len());
    assert!(gridfile.m_refine_level.iter().all(|level| *level >= 0));
    assert!(gridfile.w_refine_level.iter().all(|level| *level >= 0));
    let lineages =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(&run.output.output)
            .unwrap();
    assert_eq!(lineages.m.len(), gridfile.m_lon.len());
    assert_eq!(lineages.w.len(), gridfile.w_lon.len());
    assert!(lineages.m.iter().any(|lineage| *lineage > 1));
    assert!(lineages.w.iter().any(|lineage| *lineage > 1));
    assert!(lineages.m.iter().all(|lineage| *lineage >= 0));
    assert!(lineages.w.iter().all(|lineage| *lineage >= 0));
    let regional_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .unwrap();
    assert_regional_triangles_are_whole_global_subset(&regional_mesh, &lineages, &global_mesh);
    let certified = run.certified_run.unwrap();
    assert_eq!(certified.topology_errors, 0);
    assert!(certified.remap.is_none());
    assert!(certified
        .pre_export_remap
        .as_ref()
        .is_some_and(|p| p.exists()));
    assert_eq!(
        run.output.output.file_name().unwrap().to_str().unwrap(),
        "gridfile_NXP0003_tri_oceanmesh.nc4"
    );
    let fvcom = root.join("regional_ocean/result/fvcom.2dm");
    assert!(fvcom.exists());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.manifest).unwrap()).unwrap();
    assert_eq!(manifest["fvcom_2dm"], fvcom.display().to_string());
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.certificate).unwrap()).unwrap();
    assert_eq!(certificate["geometry_scope"], "pre_export_closed_sphere");
    assert_eq!(certificate["published_grid_is_certified_face_subset"], true);
    assert_eq!(
        certificate["published_grid_lineage_scope"],
        "pre_export_closed_sphere_canonical_ids"
    );
    assert!(
        certificate["published_domain_geometry"]["cells"]
            .as_u64()
            .unwrap()
            < certified.mother_cells as u64
    );
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.resources).unwrap()).unwrap();
    assert_eq!(
        resources["published_domain_topology"]["violations"],
        serde_json::json!([])
    );
    assert_eq!(
        resources["fvcom_2dm"]["output"],
        fvcom.display().to_string()
    );
    assert!(resources["fvcom_2dm"]["triangles"].as_u64().unwrap() > 0);
    assert!(
        resources["published_domain_topology"]["boundary_loops"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        resources["landtype_masked_cells"],
        certificate["published_domain_geometry"]["cells"]
    );
}

#[test]
fn certified_close_land_triangles_preserve_global_faces_and_publication_guards() {
    let root = temp_root("regional_land_tri_colm");
    let landtype = root.join("landtype.nc");
    write_landtype(&landtype);
    let close = root.join("domain.nml");
    write_close_domain(&close);
    let global_path = root.join("global.nml");
    let global_contents =
        namelist(&root, "land_tri_global", 6, 1_000).replace("mode_grid='hex'", "mode_grid='tri'");
    fs::write(&global_path, &global_contents).unwrap();
    let global_run =
        earthmesh_cli::run_refine_pipeline_namelist(&global_path, &root, 1_000, None).unwrap();
    let global = earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(
        &global_run.output.output,
    )
    .unwrap();
    let path = root.join("regional.nml");
    let regional_contents = global_contents
        .replace("EXPNME='land_tri_global'", "EXPNME='land_tri_regional'")
        .replace("mesh_type='earthmesh'", "mesh_type='landmesh'")
        .replace("NL%landtype_file='none'", &format!("NL%landtype_file='{}'", landtype.display()))
        .replace("NL%mask_domain_global=.true.", &format!(
            "NL%mask_domain_global=.false.\n  NL%mask_domain_type='close'\n  NL%mask_domain_fprefix='{}'", close.display()));
    fs::write(&path, regional_contents).unwrap();
    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect("regional CoLM land triangles must use a real close-domain carve");
    assert_eq!(
        run.output.output.file_name().unwrap(),
        "gridfile_NXP0006_tri_landmesh.nc4"
    );
    let regional =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&run.output.output)
            .unwrap();
    let lineages =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(&run.output.output)
            .unwrap();
    assert_regional_triangles_are_whole_global_subset(&regional, &lineages, &global);
    assert!(regional.m_points.len() < global.m_points.len());
    for point in regional
        .m_points
        .iter()
        .skip(2)
        .chain(regional.w_points.iter().skip(2))
    {
        assert!((100.0..=160.0).contains(&point.lon) && (0.0..=55.0).contains(&point.lat));
    }
    let certified = run.certified_run.unwrap();
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.certificate).unwrap()).unwrap();
    assert_eq!(certificate["geometry_scope"], "pre_export_closed_sphere");
    assert_eq!(certificate["published_grid_is_certified_face_subset"], true);
    assert_eq!(certificate["published_domain_geometry"]["cell_view"], "tri");
    assert_eq!(
        certificate["published_domain_geometry"]["contract_pass"],
        true
    );
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.resources).unwrap()).unwrap();
    assert_eq!(
        resources["published_domain_topology"]["violations"],
        serde_json::json!([])
    );
    assert!(
        resources["published_domain_topology"]["boundary_loops"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(certified.ready_marker.exists());
    assert!(certified.remap.is_none());
    // A land island can be a single whole triangle; retain it and its warning.
    write_all_ocean(&landtype);
    let centre = regional.m_points[2];
    let lon = (centre.lon + 179.5).round().rem_euclid(360.0) as usize;
    let lat = (89.5 - centre.lat).round().clamp(0.0, 179.0) as usize;
    {
        let mut file = netcdf::append(&landtype).unwrap();
        file.variable_mut("landtype")
            .unwrap()
            .put_value(1_i8, [lon, lat])
            .unwrap();
        file.close().unwrap();
    }
    let island = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect("a one-cell land island must remain a warning, not a publication failure");
    let island_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(&island.output.output)
            .unwrap();
    let island_lineages =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_cell_lineages(&island.output.output)
            .unwrap();
    assert_regional_triangles_are_whole_global_subset(&island_mesh, &island_lineages, &global);
    let island_resources: serde_json::Value =
        serde_json::from_slice(&fs::read(&island.certified_run.unwrap().resources).unwrap())
            .unwrap();
    assert_eq!(island_resources["published_domain_geometry"]["cells"], 1);
    assert_eq!(
        island_resources["published_domain_geometry"]["contract_pass"],
        true
    );
    assert_eq!(
        island_resources["published_domain_topology"]["violations"],
        serde_json::json!([])
    );
    assert_eq!(
        island_resources["published_domain_topology"]["boundary_loops"],
        1
    );
    assert_eq!(island_resources["published_domain_topology"]["euler"], 1);
    assert!(
        island_resources["published_domain_quality_topology"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["type"] == "orphan_cell" && issue["severity"] == "warn")
    );
    let result_dir = run.output.output.parent().unwrap();
    let before = fs::read_dir(result_dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    write_all_ocean(&landtype);
    assert!(earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).is_err());
    for (path, bytes) in &before {
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    assert_eq!(fs::read_dir(result_dir).unwrap().count(), before.len());
}

#[test]
fn certified_close_land_publishes_whole_dual_cells_for_colm() {
    let root = temp_root("regional_land_colm");
    let landtype = root.join("landtype.nc");
    write_landtype(&landtype);
    let close = root.join("domain.nml");
    write_close_domain(&close);
    let path = root.join("cmrc.nml");
    let contents = namelist(&root, "regional_land_colm", 6, 1_000)
        .replace("mesh_type='earthmesh'", "mesh_type='landmesh'")
        .replace("NL%landtype_file='none'", &format!("NL%landtype_file='{}'", landtype.display()))
        .replace("NL%mask_domain_global=.true.", &format!(
            "NL%mask_domain_global=.false.\n  NL%mask_domain_type='close'\n  NL%mask_domain_fprefix='{}'", close.display()));
    fs::write(&path, contents).unwrap();
    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let grid = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
        .unwrap();
    let input = earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile_hex_native(&grid)
        .unwrap();
    assert!(!input.cells.is_empty());
    assert!(input.cells.len() < 362);
    for ((&lon, &lat), &count) in grid.w_lon.iter().zip(&grid.w_lat).zip(&grid.n_w) {
        if count >= 3 {
            // The northern close edge is a great-circle arc, not a latitude parallel.
            assert!((100.0..=160.0).contains(&lon) && (0.0..=55.0).contains(&lat));
        }
    }
    let geometry = earthmesh_quality::compute(&input, &Default::default()).geometry;
    assert_eq!(geometry.negative_area_cell_count, 0);
    assert_eq!(geometry.invalid_polygon_count, 0);
    assert!(geometry.min_angle_deg.is_finite() && geometry.max_angle_deg.is_finite());
    let run = run.certified_run.unwrap();
    let cert: serde_json::Value =
        serde_json::from_slice(&fs::read(run.certificate).unwrap()).unwrap();
    assert_eq!(cert["geometry_scope"], "pre_export_closed_sphere");
    assert_eq!(cert["published_grid_is_certified_face_subset"], false);
    assert_eq!(cert["published_grid_is_certified_dual_cell_subset"], true);
    assert_eq!(
        cert["published_domain_geometry"]["whole_cell_lineage_verified"],
        true
    );
    assert_eq!(cert["published_domain_geometry"]["cell_view"], "hex");
    assert_eq!(cert["published_grid_remap_available"], false);
    assert!(run.remap.is_none());
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(run.resources).unwrap()).unwrap();
    assert_eq!(
        resources["published_domain_topology"]["violations"],
        serde_json::json!([])
    );
    assert!(
        resources["published_domain_topology"]["boundary_loops"]
            .as_u64()
            .unwrap()
            > 0
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run.manifest).unwrap()).unwrap();
    assert!(std::path::Path::new(manifest["ready"].as_str().unwrap()).exists());
    // A subsequent empty land selection must not damage the previous ready bundle.
    let result_dir = root.join("regional_land_colm/result");
    let before = fs::read_dir(&result_dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    write_all_ocean(&landtype);
    assert!(earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).is_err());
    for (path, bytes) in &before {
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    assert_eq!(fs::read_dir(&result_dir).unwrap().count(), before.len());
}

#[test]
fn certified_regional_earthmesh_is_rejected_instead_of_published_global() {
    let root = temp_root("regional_earthmesh_reject");
    let path = root.join("cmrc.nml");
    let contents = namelist(&root, "regional_earthmesh_reject", 3, 1_000)
        .replace(
            "NL%mask_domain_global=.true.",
            "NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='inline:bbox:w=100,e=160,s=0,n=50'",
        );
    fs::write(&path, contents).unwrap();

    let error = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None)
        .expect_err("regional earthmesh would otherwise ignore the selector");
    assert!(error.to_string().contains(
        "supports oceanmesh/tri or landmesh/{hex,tri}/CoLM with a single close polygon only"
    ));
    assert!(!root
        .join("regional_earthmesh_reject/result/gridfile_NXP0003_hex.nc4")
        .exists());
}

#[test]
fn reverse_mode_publishes_a_dqx_mixed_level_mesh() {
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
        .replace(
            "NL%delivery='coupled'",
            "NL%delivery='coupled'\n  NL%angle_contract='domain_quality_38_to_82_v1'",
        )
        .replace("NL%search_budget=100", "NL%search_budget=4000");
    fs::write(&path, namelist).unwrap();

    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    assert_eq!(certified.initial_mother_subdivision, 6);
    assert_eq!(certified.mother_subdivision, 6);
    assert_eq!(certified.initial_mother_cells, 720);
    assert!(certified.mother_cells < certified.initial_mother_cells);
    assert!(certified.removed_vertices > 0);
    assert!(certified.removed_faces > 0);
    assert!(certified.fulfillment.mixed_levels_delivered);
    assert!(certified.fulfillment.components_committed > 0);
    assert_eq!(certified.topology_errors, 0);
    assert_eq!(certified.dual_errors, 0);
    assert_eq!(certified.physical_residuals, 0);
    assert_eq!(certified.balance_residuals, 0);
    assert_eq!(certified.remap_closure_errors, 0);
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.certificate).unwrap()).unwrap();
    assert_eq!(
        certificate["coarsening_strategy"],
        "elastic_component_epochs"
    );
    assert_eq!(certificate["angle_contract"], "domain_quality_38_to_82_v1");
    assert_eq!(
        certificate["dqx_execution_status"],
        "geometry_contract_only"
    );
    assert!(certificate["geometry"]["minimum_angle_deg"]
        .as_f64()
        .is_some_and(|angle| angle >= 38.0));
    assert!(certificate["geometry"]["maximum_angle_deg"]
        .as_f64()
        .is_some_and(|angle| angle <= 82.0));
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.resources).unwrap()).unwrap();
    let layers = &certificate["requirement_layers"];
    assert_eq!(*layers, resources["requirement_layers"]);
    assert_eq!(layers["raw_source_raster"]["status"], "available");
    assert_eq!(
        layers["graph_scheduling_target"]["histogram"],
        certificate["elastic_component_epochs"]["requested_histogram"]
    );
    assert_eq!(
        layers["graph_scheduling_target"]["gradation_rings_per_level"],
        3
    );
    let count = |hist: &serde_json::Value| {
        hist.as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>()
    };
    assert_eq!(
        count(&layers["effective_source_raster"]["histogram"]),
        720 * 360
    );
    assert_eq!(count(&layers["raw_source_raster"]["histogram"]), 720 * 360);
    assert_eq!(
        count(&layers["graph_scheduling_target"]["histogram"]),
        certificate["elastic_component_epochs"]["aggregate"]["initial_vertices"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(certificate["delivered_level_min"], 0);
    assert_eq!(certificate["delivered_level_max"], 1);
    assert!(
        certificate["elastic_component_epochs"]["aggregate"]["components_committed"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(certificate["coarsening"]["removed_faces"]
        .as_u64()
        .is_some_and(|removed| removed > 0));
    assert_eq!(
        certificate["physical_balance_scope"],
        "final_voronoi_cells_exact_raster_overlap"
    );
    let gridfile =
        earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&run.output.output)
            .unwrap();
    assert_eq!(gridfile.w_refine_level.len(), gridfile.w_lon.len());
    assert_eq!(gridfile.m_refine_level.len(), gridfile.m_lon.len());
    assert!(gridfile.w_refine_level.contains(&0));
    assert!(gridfile.w_refine_level.contains(&1));
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
        fs::read(forward.join("certified_safe_fallback_certificate.json")).unwrap(),
        fs::read(reverse.join("certified_safe_fallback_certificate.json")).unwrap()
    );
    assert_eq!(
        fs::read(forward.join("certified_safe_fallback_remap.csv")).unwrap(),
        fs::read(reverse.join("certified_safe_fallback_remap.csv")).unwrap()
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
            fs::read(certified.remap.unwrap()).unwrap(),
        ));
    }
    assert!(artifacts
        .windows(2)
        .all(|pair| pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1));
    assert!(mesh_counts.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn certified_regional_unimplemented_views_and_boundaries_fail_closed() {
    let root = temp_root("regional_unsupported");
    let landtype = root.join("ocean.nc");
    write_all_ocean(&landtype);
    let close = root.join("domain.nml");
    write_close_domain(&close);
    for (case, view, kind, source) in [
        ("hex_close", "hex", "close", close.display().to_string()),
        (
            "tri_bbox",
            "tri",
            "bbox",
            "inline:bbox:w=100,e=160,s=0,n=50".to_string(),
        ),
    ] {
        let path = root.join(format!("{case}.nml"));
        let contents = namelist(&root, case, 3, 1_000)
            .replace("NL%mesh_type='earthmesh'", "NL%mesh_type='oceanmesh'")
            .replace("NL%mode_grid='hex'", &format!("NL%mode_grid='{view}'"))
            .replace("NL%output_format='CoLM'", "NL%output_format='FVCOM'")
            .replace("NL%mask_domain_global=.true.", &format!("NL%mask_domain_global=.false.\nNL%mask_domain_type='{kind}'\nNL%mask_domain_fprefix='{source}'"))
            .replace("NL%landtype_file='none'", &format!("NL%landtype_file='{}'", landtype.display()));
        fs::write(&path, contents).unwrap();
        let error =
            earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert!(error.to_string().contains(
            "supports oceanmesh/tri or landmesh/{hex,tri}/CoLM with a single close polygon only"
        ));
        assert!(!root.join(case).join("result/certified_ready").exists());
    }
}

#[test]
fn certified_subraster_requirement_reports_the_conservative_bound_separately() {
    let root = temp_root("subraster_layers");
    let prefix = root.join("tiny");
    let landtype = root.join("ocean.nc");
    write_all_ocean(&landtype);
    earthmesh_cli::circle_close_mask_io::write_circle_mask_netcdf(
        root.join("tiny_001.nc4"),
        &earthmesh_cli::circle_close_mask_io::CircleMask {
            refine_degree: 1,
            points: vec![earthmesh_cli::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }],
            radius_km: vec![0.001],
        },
    )
    .unwrap();
    let path = root.join("cmrc.nml");
    // Exercise both region loaders; neither may call the fallback an expanded footprint.
    for calculated in [false, true] {
        let mut text = specified_circle_namelist(&root, "subraster_layers", &prefix);
        if calculated {
            text = text
                .replace("RL%refine_cal=.false.", "")
                .replace("refine_spc", "refine_cal")
                .replace("max_iter_spc", "max_iter_cal")
                .replace(
                    "RL%max_iter_cal=1",
                    "RL%max_iter_cal=1\n RL%refine_num_landtypes=.true.\n RL%th_num_landtypes=100",
                )
                .replace(
                    "NL%landtype_file='none'",
                    &format!("NL%landtype_file='{}'", landtype.display()),
                );
        }
        fs::write(&path, text).unwrap();
        let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
        let certified = run.certified_run.unwrap();
        let certificate: serde_json::Value =
            serde_json::from_slice(&fs::read(certified.certificate).unwrap()).unwrap();
        let layers = &certificate["requirement_layers"];
        assert_eq!(
            layers["raw_source_raster"]["histogram"],
            if calculated {
                serde_json::Value::Null
            } else {
                serde_json::json!({"0": 259200})
            }
        );
        assert_eq!(
            layers["effective_source_raster"]["histogram"],
            serde_json::json!({"1": 259200})
        );
        assert_eq!(
            layers["effective_source_raster"]["conservative_global_bound"],
            true
        );
        assert_eq!(
            layers["effective_source_raster"]["raised_samples_over_raw"],
            if calculated {
                serde_json::Value::Null
            } else {
                serde_json::json!(259200)
            }
        );
        assert_eq!(certified.physical_residuals, 0);
        assert_eq!(certified.chosen_level, 1);
    }
}

#[test]
fn certified_hydro_requirement_does_not_publish_partial_raw_provenance() {
    let root = temp_root("hydro_layers");
    let cells = root.join("cells.geojson");
    let levels = root.join("levels.json");
    fs::write(&cells, r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"center_lon":0,"center_lat":0},"geometry":{"type":"Polygon","coordinates":[[[-2,-2],[2,-2],[2,2],[-2,2],[-2,-2]]]}}]}"#).unwrap();
    fs::write(&levels, r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"target_level":1}]}"#).unwrap();
    let path = root.join("cmrc.nml");
    fs::write(&path, format!(
        "{}\n&hfield\n NL%hfield_nlon=36\n NL%hfield_nlat=18\n NL%hfield_target_cells_geojson='{}'\n NL%hfield_target_levels_json='{}'\n/\n",
        namelist(&root, "hydro_layers", 3, 1_000), cells.display(), levels.display(),
    )).unwrap();
    let run = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(certified.certificate).unwrap()).unwrap();
    let layers = &certificate["requirement_layers"];
    assert_eq!(
        layers["raw_source_raster"]["status"],
        "unavailable_threshold_or_hydro"
    );
    assert!(layers["raw_source_raster"]["histogram"].is_null());
    assert!(layers["effective_source_raster"]["raised_samples_over_raw"].is_null());
    assert_eq!(layers["policy"], "effective_raster_remains_hard");
    assert_eq!(certified.physical_residuals, 0);
    assert_eq!(certified.chosen_level, 1);
}

#[test]
fn unsupported_mother_family_rejects_before_reading_threshold_data() {
    let root = temp_root("unsupported_before_thresholds");
    let landtype = root.join("not_a_netcdf.nc");
    fs::write(&landtype, "invalid threshold payload must never be opened").unwrap();
    let path = root.join("cmrc.nml");
    fs::write(
        &path,
        landtype_namelist(&root, "unsupported_before_thresholds", &landtype)
            .replace("NL%NXP=3", "NL%NXP=7"),
    )
    .unwrap();
    let error = earthmesh_cli::run_refine_pipeline_namelist(&path, &root, 1_000, None).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(
        error
            .to_string()
            .contains("no certified mother subdivision"),
        "{error}"
    );
    assert!(!root.join("unsupported_before_thresholds/result").exists());
}
