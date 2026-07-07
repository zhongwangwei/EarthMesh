use std::{fs, process::Command, sync::Mutex};

use earthmesh_cli::{
    write_bbox_mask_netcdf, write_unstructured_mesh_netcdf,
    write_unstructured_mesh_netcdf_with_refine_levels, BBoxMask, BBoxPoint, LonLatPoint,
    UnstructuredMesh,
};

static MESH_QUALITY_TEST_LOCK: Mutex<()> = Mutex::new(());

fn fixture_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
        ],
        m_to_w: vec![[1, 2, 3], [1, 3, 4], [1, 2, 4], [2, 3, 4]],
        w_to_m: vec![
            vec![1, 2, 3, 4],
            vec![1, 2, 3, 4],
            vec![1, 2, 3, 4],
            vec![1, 2, 3, 4],
        ],
        n_w_to_m: vec![4, 4, 4, 4],
    }
}

#[test]
fn mesh_quality_cli_reports_tri_and_hex_views_without_repo_fixture() {
    let _guard = MESH_QUALITY_TEST_LOCK
        .lock()
        .expect("mesh quality test lock");
    let root = std::env::temp_dir().join(format!("earthmesh_quality_views_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");
    write_unstructured_mesh_netcdf(&gridfile, &fixture_mesh()).expect("write gridfile");

    for kind in ["tri", "hex"] {
        let out_dir = root.join(kind);
        let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg("--mesh-quality")
            .arg(&gridfile)
            .arg(&out_dir)
            .arg("--kind")
            .arg(kind)
            .output()
            .expect("run earthmesh_cli");
        assert!(
            output.status.success(),
            "{kind} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
        assert!(
            stdout.contains(&format!("mesh_quality_kind={kind}")),
            "{stdout}"
        );
        assert!(stdout.contains("mesh_quality_cell_sides="), "{stdout}");

        let json = fs::read_to_string(out_dir.join("quality_summary.json")).expect("json");
        assert!(
            json.contains(&format!("\"cell_view\": \"{kind}\"")),
            "{json}"
        );
        let csv = fs::read_to_string(out_dir.join("quality_summary.csv")).expect("csv");
        assert!(csv.contains(&format!("summary,cell_view,,{kind}")), "{csv}");
        let md = fs::read_to_string(out_dir.join("quality_report.md")).expect("md");
        assert!(md.contains(&format!("- cell view: `{kind}`")), "{md}");
        assert!(out_dir.join("worst_cells.geojson").is_file());
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mesh_quality_cli_attaches_hfield_diagnostics_from_full_namelist() {
    let _guard = MESH_QUALITY_TEST_LOCK
        .lock()
        .expect("mesh quality test lock");
    let root =
        std::env::temp_dir().join(format!("earthmesh_quality_hfield_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");
    let m_levels = [1, 1, 1, 1];
    let w_levels = [1, 1, 1, 1];
    write_unstructured_mesh_netcdf_with_refine_levels(
        &gridfile,
        &fixture_mesh(),
        Some(&m_levels),
        Some(&w_levels),
    )
    .expect("write gridfile");

    let bbox_prefix = root.join("hfield_bbox");
    write_bbox_mask_netcdf(
        root.join("hfield_bbox.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -10.0,
                east: 10.0,
                north: 10.0,
                south: -10.0,
            }],
        },
    )
    .expect("write bbox");
    let quality_nml = root.join("quality_full.nml");
    fs::write(
        &quality_nml,
        format!(
            r#"
&mkgrd
  NL%EXPNME = 'quality'
  NL%base_dir = '{}/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'tri'
  NL%output_format = 'MPAS'
  NL%NXP = 16
  NL%refine = .true.
  NL%gridnum_perdegree = 120
/
&mkrefine
  RL%Istransition = .true.
  RL%SpringGlobal_type = 0
  RL%SpringRegional_type = 0
  RL%refine_spc = .true.
  RL%refine_cal = .false.
  RL%max_iter_spc = 1
  RL%mask_refine_spc_type = 'bbox'
  RL%mask_refine_spc_fprefix = '{}'
/
&hfield
  NL%hfield_on = .true.
  NL%hfield_g = 0.2
  NL%hfield_max_level = 1
  NL%hfield_nlon = 32
  NL%hfield_nlat = 16
/
&quality
  NL%on_violation = 'warn'
/
"#,
            root.display(),
            bbox_prefix.display()
        ),
    )
    .expect("write namelist");

    let out_dir = root.join("report");
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--mesh-quality")
        .arg(&gridfile)
        .arg(&out_dir)
        .arg(&quality_nml)
        .arg("--kind")
        .arg("tri")
        .output()
        .expect("run earthmesh_cli");
    assert!(
        output.status.success(),
        "mesh-quality failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("mesh_quality_hfield=1"), "{stdout}");

    let json = fs::read_to_string(out_dir.join("quality_summary.json")).expect("json");
    assert!(json.contains("\"hfield\": {"), "{json}");
    assert!(
        json.contains("\"target_level_distribution\":[{\"level\":1"),
        "{json}"
    );
    assert!(
        json.contains("\"actual_refine_level_distribution\":[{\"level\":1"),
        "{json}"
    );
    assert!(
        json.contains("\"target_actual_mismatch_count\":0"),
        "{json}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mesh_quality_cli_reports_hfield_target_actual_mismatch() {
    let _guard = MESH_QUALITY_TEST_LOCK
        .lock()
        .expect("mesh quality test lock");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_quality_hfield_mismatch_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");
    write_unstructured_mesh_netcdf_with_refine_levels(
        &gridfile,
        &fixture_mesh(),
        Some(&[0, 0, 0, 0]),
        Some(&[0, 0, 0, 0]),
    )
    .expect("write gridfile");

    let bbox_prefix = root.join("hfield_bbox");
    write_bbox_mask_netcdf(
        root.join("hfield_bbox.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -10.0,
                east: 10.0,
                north: 10.0,
                south: -10.0,
            }],
        },
    )
    .expect("write bbox");
    let quality_nml = root.join("quality_full.nml");
    fs::write(
        &quality_nml,
        format!(
            r#"
&mkgrd
  NL%EXPNME = 'quality'
  NL%base_dir = '{}/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'tri'
  NL%output_format = 'MPAS'
  NL%NXP = 16
  NL%refine = .true.
  NL%gridnum_perdegree = 120
/
&mkrefine
  RL%Istransition = .true.
  RL%SpringGlobal_type = 0
  RL%SpringRegional_type = 0
  RL%refine_spc = .true.
  RL%refine_cal = .false.
  RL%max_iter_spc = 1
  RL%mask_refine_spc_type = 'bbox'
  RL%mask_refine_spc_fprefix = '{}'
/
&hfield
  NL%hfield_on = .true.
  NL%hfield_g = 0.2
  NL%hfield_max_level = 1
  NL%hfield_nlon = 32
  NL%hfield_nlat = 16
/
&quality
  NL%on_violation = 'warn'
/
"#,
            root.display(),
            bbox_prefix.display()
        ),
    )
    .expect("write namelist");

    let out_dir = root.join("report");
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--mesh-quality")
        .arg(&gridfile)
        .arg(&out_dir)
        .arg(&quality_nml)
        .arg("--kind")
        .arg("tri")
        .output()
        .expect("run earthmesh_cli");
    assert!(
        output.status.success(),
        "mesh-quality failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json = fs::read_to_string(out_dir.join("quality_summary.json")).expect("json");
    assert!(
        json.contains("\"target_actual_mismatch_count\":4"),
        "{json}"
    );
    assert!(json.contains("\"target_above_actual_count\":4"), "{json}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mesh_quality_cli_fails_loudly_when_hfield_regions_are_missing() {
    let _guard = MESH_QUALITY_TEST_LOCK
        .lock()
        .expect("mesh quality test lock");
    let root = std::env::temp_dir().join(format!(
        "earthmesh_quality_hfield_missing_regions_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");
    write_unstructured_mesh_netcdf_with_refine_levels(
        &gridfile,
        &fixture_mesh(),
        Some(&[0, 0, 0, 0]),
        Some(&[0, 0, 0, 0]),
    )
    .expect("write gridfile");

    let quality_nml = root.join("quality_full.nml");
    fs::write(
        &quality_nml,
        format!(
            r#"
&mkgrd
  NL%EXPNME = 'quality'
  NL%base_dir = '{}/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'tri'
  NL%output_format = 'MPAS'
  NL%NXP = 16
  NL%refine = .true.
/
&mkrefine
  RL%Istransition = .true.
  RL%SpringGlobal_type = 0
  RL%SpringRegional_type = 0
  RL%refine_spc = .true.
  RL%refine_cal = .false.
  RL%max_iter_spc = 1
  RL%mask_refine_spc_type = 'bbox'
  RL%mask_refine_spc_fprefix = '{}/does_not_exist'
/
&hfield
  NL%hfield_on = .true.
/
&quality
  NL%on_violation = 'warn'
/
"#,
            root.display(),
            root.display()
        ),
    )
    .expect("write namelist");

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .arg("--mesh-quality")
        .arg(&gridfile)
        .arg(root.join("report"))
        .arg(&quality_nml)
        .output()
        .expect("run earthmesh_cli");
    assert!(
        !output.status.success(),
        "mesh-quality should fail when h-field source discovery is empty"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("h-field diagnostics found no"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&root);
}
