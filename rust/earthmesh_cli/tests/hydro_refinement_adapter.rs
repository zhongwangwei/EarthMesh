use std::fs;
use std::path::PathBuf;

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_hydro_refinement_engine_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create hydro adapter test root");
    root
}

#[test]
fn hydro_target_levels_execute_the_real_method_c_pipeline() {
    let root = temp_root();
    let source = root.join("project.nml");
    let cells = root.join("intersections.geojson");
    let levels = root.join("refinement_plan.json");
    let adapter = root.join("hydro_adapter.nml");
    let initial_gridfile =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hydro_method_c_parent.nc4");
    assert!(initial_gridfile.is_file());
    fs::write(
        &source,
        format!(
            "&mkgrd\n  NL%EXPNME='hydro_adapter'\n  NL%base_dir='{}/'\n  NL%NXP=80\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.false.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.25\n  NL%landtype_file='none'\n  NL%mask_domain_global=.true.\n  NL%mask_patch_on=.false.\n  NL%output_format='CoLM'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%niter_refine=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.false.\n/\n&hfield\n  NL%hfield_on=.true.\n  NL%hfield_g=0.2\n  NL%hfield_origin_lon=110.5\n  NL%hfield_origin_lat=22.25\n  NL%hfield_nlon=360\n  NL%hfield_nlat=180\n/\n",
            root.display()
        ),
    )
    .expect("write source namelist");
    // Keep this integration case local to one measured parent-grid cell. A broad
    // synthetic rectangle turns the second Method-C pass into an unrelated
    // continent-scale perimeter-repair stress test and can exhaust the CI timeout.
    fs::write(
        &cells,
        r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"1","center_lon":115.3779068,"center_lat":25.1378422},"geometry":{"type":"Polygon","coordinates":[[[115.1,24.9],[115.65,24.9],[115.65,25.4],[115.1,25.4],[115.1,24.9]]]}}]}"#,
    )
    .expect("write target cells");
    fs::write(
        &levels,
        r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells_refined":1,"max_level":1,"cells":[{"cell":0,"target_level":1}]}"#,
    )
    .expect("write target levels");
    let coarse_sentinel = root.join("hydro_adapter/result/coarse-grid-preserved.txt");
    fs::create_dir_all(coarse_sentinel.parent().unwrap()).expect("create coarse result dir");
    fs::write(&coarse_sentinel, "coarse").expect("write coarse sentinel");

    let report = earthmesh_cli::hydro_refinement_adapter::run_hydro_refinement_adapter(
        &source,
        &initial_gridfile,
        &cells,
        &levels,
        &adapter,
        &root,
        20_000,
        None,
    )
    .expect("hydro plan should execute through Method-C");

    assert_eq!(report.target.max_level, 1);
    assert_eq!(report.pipeline.max_level, 1);
    assert_eq!(
        report.pipeline.gridinit.gridfile.lbx_points, 140,
        "adapter must ingest the measured parent gridfile instead of rebuilding from NXP"
    );
    assert!(report.pipeline.output.output.is_file());
    assert!(
        report
            .final_gridfile()
            .starts_with(root.join("engine/hydro_refined")),
        "second-pass outputs must be isolated from the coarse Project run: {}",
        report.final_gridfile().display()
    );
    assert_eq!(
        fs::read_to_string(&coarse_sentinel).expect("coarse result survives"),
        "coarse"
    );
    assert!(
        report.pipeline.output.lbx_points > report.pipeline.gridinit.gridfile.lbx_points,
        "hydro target field must add real Method-C cells (base={}, final={})",
        report.pipeline.gridinit.gridfile.lbx_points,
        report.pipeline.output.lbx_points
    );
    let adapter_text = fs::read_to_string(&adapter).expect("read adapter namelist");
    assert!(
        adapter_text.contains("hfield_target_levels_json"),
        "adapter evidence must name the target-level input"
    );
    assert!(adapter_text.contains("hfield_origin_lon = 110.5"));
    assert!(adapter_text.contains("hfield_origin_lat = 22.25"));
    assert!(adapter_text.contains("NL%mode_file_description = 'EarthMesh'"));
    assert!(
        adapter_text.contains("NL%hfield_g = 0.2"),
        "the normal adapter pass must preserve the source HField gradation"
    );
    assert!(adapter_text.contains(
        &initial_gridfile
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
    ));

    let first_parent = report.pipeline.refinement_parent_gridfile().to_path_buf();
    let first_cells = report.pipeline.output.lbx_points;
    let persisted = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(&first_parent)
        .expect("read persisted Method-C metadata");
    assert_eq!(persisted.m_refine_level_orig.len(), persisted.m_lon.len());
    assert_eq!(persisted.m_ngr.len(), persisted.m_lon.len());
    assert_eq!(persisted.w_refine_level_orig.len(), persisted.w_lon.len());
    assert_eq!(persisted.w_ngr.len(), persisted.w_lon.len());
    assert!(
        persisted
            .m_ngr
            .iter()
            .chain(&persisted.w_ngr)
            .any(|&ngr| ngr > 1),
        "first local pass must persist non-base Method-C ownership"
    );
    fs::write(
        &levels,
        r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"target_level":2}]}"#,
    )
    .expect("write second-pass target levels");
    let second = earthmesh_cli::hydro_refinement_adapter::run_hydro_refinement_adapter(
        &source,
        &first_parent,
        &cells,
        &levels,
        root.join("quality_pass_2/adapter.nml"),
        &root,
        40_000,
        None,
    )
    .expect("persisted Method-C ownership must support a second local pass");
    assert_eq!(second.target.max_level, 2);
    assert_eq!(second.pipeline.gridinit.gridfile.lbx_points, first_cells);
    assert!(
        second.pipeline.output.lbx_points > first_cells,
        "second absolute target level must refine the persisted parent"
    );
    let _ = fs::remove_dir_all(root);
}
