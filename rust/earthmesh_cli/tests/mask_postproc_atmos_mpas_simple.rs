use earthmesh_cli::unstructured_mesh_support::UnstructuredMesh;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn source_mesh() -> UnstructuredMesh {
    let state = earthmesh_mesh::gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100).unwrap();
    earthmesh_cli::mesh_conversion_gridfile_state::gridfile_mesh_from_one_based_state(
        &state.grid,
        &state.tabs,
    )
    .unwrap()
}

fn assert_no_delivery_staging(root: &Path) {
    fn walk(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            assert!(
                !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".earthmesh-delivery-"),
                "staging path leaked: {}",
                path.display()
            );
            if path.is_dir() {
                walk(&path);
            }
        }
    }
    if root.exists() {
        walk(root);
    }
}

fn seed_atmos_case(case: &Path, mode: &str) -> (PathBuf, PathBuf, UnstructuredMesh) {
    let result = case.join("result");
    fs::create_dir_all(&result).unwrap();
    let mesh = source_mesh();
    let grid = result.join(format!("gridfile_NXP0001_{mode}.nc4"));
    earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&grid, &mesh).unwrap();
    let widths = (0..mesh.w_points.len())
        .map(|i| 12.0 * (1 + i % 3) as f64)
        .collect::<Vec<_>>();
    let width_file = result.join("cellwidth_NXP0001_global.nc4");
    write_cellwidth_fixture(&width_file, &widths);
    (grid, width_file, mesh)
}

fn deliver(root: &Path, mode: &str, format: &str) -> std::io::Result<PathBuf> {
    if format == "MPAS-Simple" {
        earthmesh_cli::mask_postproc_atmos::write_mask_postproc_atmos_mpas_simple_netcdf(
            root,
            1,
            mode,
            "atmosmesh",
            format,
        )
        .map(|report| report.output)
    } else {
        earthmesh_cli::mask_postproc_atmos::write_mask_postproc_atmos_mpas_netcdf(
            root,
            1,
            1,
            mode,
            "atmosmesh",
            format,
        )
        .map(|report| report.mesh.output)
    }
}

#[test]
fn atmos_mpas_dispatch_admits_exported_w_cells_and_records_current_artifacts() {
    let root = std::env::temp_dir().join(format!("legacy_atmos_delivery_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let mesh = source_mesh();
    for mode in ["tri", "hex"] {
        let case = root.join(mode);
        let result = case.join("result");
        fs::create_dir_all(&result).unwrap();
        let grid = result.join(format!("gridfile_NXP0001_{mode}.nc4"));
        earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&grid, &mesh).unwrap();
        let original = fs::read(&grid).unwrap();
        // Legacy tri is a source filename label: both adapters export dual W cells.
        let widths = (0..mesh.w_points.len())
            .map(|i| 12.0 * (1 + i % 3) as f64)
            .collect::<Vec<_>>();
        let width_file = result.join("cellwidth_NXP0001_global.nc4");
        write_cellwidth_fixture(&width_file, &widths);
        let width_bytes = fs::read(&width_file).unwrap();
        for format in ["MPAS", "MPAS-Simple"] {
            let output = deliver(&case, mode, format).unwrap();
            let suffix = if format == "MPAS" { "" } else { "_Simple" };
            assert_eq!(
                output,
                result.join(format!("MPASOUT_NXP0001_global{suffix}.nc4"))
            );
            let file = netcdf::open(&output).unwrap();
            assert_eq!(
                file.dimension("nCells").unwrap().len(),
                mesh.w_points.len() - 1
            );
            let expected = earthmesh_cli::mpas_unstructured_mesh_builders::build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &widths).unwrap();
            assert_eq!(read_f64(&file, "meshDensity"), expected.mesh_density[1..]);
            drop(file);
            let quality_dir = result.join("final_quality").join(format);
            let record: serde_json::Value = serde_json::from_slice(
                &fs::read(quality_dir.join("legacy_delivery.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(record["kind"], "earthmesh_legacy_delivery");
            assert_eq!(record["source_mode_grid"], mode);
            assert_eq!(record["target"]["cell"], "Hex");
            assert_eq!(record["model_delivery_status"], "model_delivered");
            assert_eq!(record["gridfile"], grid.to_str().unwrap());
            assert_eq!(
                record["model_artifacts"]["mpas_mesh_input"],
                output.to_str().unwrap()
            );
            assert_eq!(
                record["model_artifacts"].as_object().unwrap().len(),
                if format == "MPAS" { 2 } else { 1 }
            );
            let quality: serde_json::Value = serde_json::from_slice(
                &fs::read(quality_dir.join("quality_summary.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(quality["cell_view"], "hex");
            assert_eq!(quality["topology"]["boundary_edge_count"], 0);
            assert!(quality["gates"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["metric"] == "final_mesh_admission" && g["level"] == "pass"));
        }
        assert_eq!(fs::read(&grid).unwrap(), original);
        assert_eq!(fs::read(&width_file).unwrap(), width_bytes);
        write_cellwidth_fixture(&width_file, &[12.0]);
        for format in ["MPAS", "MPAS-Simple"] {
            let error = deliver(&case, mode, format).unwrap_err().to_string();
            assert!(error.contains("cellwidth"), "{error}");
            assert!(!result
                .join("final_quality")
                .join(format)
                .join("legacy_delivery.json")
                .exists());
        }
        assert_eq!(fs::read(&grid).unwrap(), original);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atmos_final_admission_blocks_bad_native_w_rings_before_model_writes() {
    let root = std::env::temp_dir().join(format!("legacy_atmos_reject_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for defect in ["degree", "winding", "open"] {
        for format in ["MPAS", "MPAS-Simple"] {
            let case = root.join(defect).join(format);
            let result = case.join("result");
            fs::create_dir_all(&result).unwrap();
            let mut mesh = source_mesh();
            if defect == "degree" {
                mesh.w_to_m[1].truncate(4);
                mesh.n_w_to_m[1] = 4;
            } else if defect == "winding" {
                let corners = usize::try_from(mesh.n_w_to_m[1]).unwrap();
                mesh.w_to_m[1][..corners].reverse();
            }
            let grid = result.join("gridfile_NXP0001_tri.nc4");
            earthmesh_cli::unstructured_mesh_io::write_unstructured_mesh_netcdf(&grid, &mesh)
                .unwrap();
            if defect == "open" {
                let source = result.join("parent.nc4");
                fs::copy(&grid, &source).unwrap();
                let region = earthmesh_cli::coordinate_types::GridRegion::Bbox {
                    west: -180.0,
                    east: 180.0,
                    south: 0.0,
                    north: 90.0,
                };
                earthmesh_cli::regional_gridfile_writers::write_regional_gridfile(
                    &source, &grid, &region, "hex",
                )
                .unwrap();
            }
            let original = fs::read(&grid).unwrap();
            // No cellwidth file: final admission must fail BEFORE the adapter reads it.
            let quality_dir = result.join("final_quality").join(format);
            fs::create_dir_all(&quality_dir).unwrap();
            let record = quality_dir.join("legacy_delivery.json");
            fs::write(&record, br#"{"kind":"earthmesh_legacy_delivery","model_delivery_status":"model_delivered"}"#).unwrap();
            let error = deliver(&case, "tri", format).unwrap_err().to_string();
            let expected = match defect {
                "degree" => "5..=7",
                "winding" => "misoriented_shared_edge",
                _ => "closed sphere",
            };
            assert!(error.contains(expected), "{defect}/{format}: {error}");
            assert!(quality_dir.join("quality_summary.json").is_file());
            assert!(
                !record.exists(),
                "stale success must not survive a failed final attempt"
            );
            assert!(!result.join("MPASOUT_NXP0001_global.nc4").exists());
            assert!(!result.join("MPASOUT_NXP0001_global.graph.info").exists());
            assert!(!result.join("MPASOUT_NXP0001_global_Simple.nc4").exists());
            assert_eq!(fs::read(&grid).unwrap(), original);
            assert_no_delivery_staging(&case);
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atmos_adapter_failures_preserve_previous_model_files_and_live_diagnostics() {
    let root = std::env::temp_dir().join(format!("legacy_atmos_preserve_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);

    for format in ["MPAS", "MPAS-Simple"] {
        let mode = "tri";
        let case = root.join(format);
        let (grid, width_file, _) = seed_atmos_case(&case, mode);
        let output = deliver(&case, mode, format).unwrap();
        let output_bytes = fs::read(&output).unwrap();
        let graph = case.join("result/MPASOUT_NXP0001_global.graph.info");
        let graph_bytes = graph.exists().then(|| fs::read(&graph).unwrap());
        let grid_bytes = fs::read(&grid).unwrap();

        write_cellwidth_fixture(&width_file, &[12.0]);
        let bad_width_bytes = fs::read(&width_file).unwrap();
        let error = deliver(&case, mode, format).unwrap_err().to_string();
        assert!(error.contains("cellwidth"), "{error}");
        assert_eq!(fs::read(&output).unwrap(), output_bytes);
        if let Some(bytes) = graph_bytes {
            assert_eq!(fs::read(&graph).unwrap(), bytes);
        }
        assert_eq!(fs::read(&grid).unwrap(), grid_bytes);
        assert_eq!(fs::read(&width_file).unwrap(), bad_width_bytes);
        let quality_dir = case.join("result/final_quality").join(format);
        assert!(!quality_dir.join("legacy_delivery.json").exists());
        assert!(quality_dir.join("quality_summary.json").is_file());
        assert_no_delivery_staging(&case);
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atmos_full_mpas_graph_failure_preserves_previous_mesh_output() {
    let root = std::env::temp_dir().join(format!("legacy_atmos_graph_fail_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (grid, width_file, _) = seed_atmos_case(&root, "tri");
    let output = deliver(&root, "tri", "MPAS").unwrap();
    let output_bytes = fs::read(&output).unwrap();
    let graph = root.join("result/MPASOUT_NXP0001_global.graph.info");
    let graph_bytes = fs::read(&graph).unwrap();
    let grid_bytes = fs::read(&grid).unwrap();
    let width_bytes = fs::read(&width_file).unwrap();

    fs::remove_file(&graph).unwrap();
    fs::create_dir(&graph).unwrap();
    let error = deliver(&root, "tri", "MPAS").unwrap_err().to_string();
    assert!(
        error.contains("graph") || error.contains("directory") || error.contains("Is a directory"),
        "{error}"
    );
    assert_eq!(fs::read(&output).unwrap(), output_bytes);
    assert!(graph.is_dir());
    assert_eq!(fs::read(&grid).unwrap(), grid_bytes);
    assert_eq!(fs::read(&width_file).unwrap(), width_bytes);
    let quality_dir = root.join("result/final_quality/MPAS");
    assert!(!quality_dir.join("legacy_delivery.json").exists());
    assert!(quality_dir.join("quality_summary.json").is_file());
    assert_no_delivery_staging(&root);

    fs::remove_dir(&graph).unwrap();
    fs::write(&graph, graph_bytes).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn atmos_output_input_alias_rejects_after_retiring_readiness() {
    let root =
        std::env::temp_dir().join(format!("legacy_atmos_output_alias_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (grid, _, _) = seed_atmos_case(&root, "tri");
    let output = root.join("result/MPASOUT_NXP0001_global_Simple.nc4");
    std::fs::hard_link(&grid, &output).unwrap();
    let grid_bytes = fs::read(&grid).unwrap();
    let quality_dir = root.join("result/final_quality/MPAS-Simple");
    fs::create_dir_all(&quality_dir).unwrap();
    let marker = quality_dir.join("legacy_delivery.json");
    fs::write(&marker, "old success").unwrap();

    let error = deliver(&root, "tri", "MPAS-Simple")
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("input") || error.contains("alias") || error.contains("exists"),
        "{error}"
    );
    assert_eq!(fs::read(&grid).unwrap(), grid_bytes);
    assert!(!marker.exists());
    assert_no_delivery_staging(&root);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn atmos_mpas_simple_dispatch_rejects_wrong_branch() {
    let err = earthmesh_cli::mask_postproc_atmos::write_mask_postproc_atmos_mpas_simple_netcdf(
        std::env::temp_dir(),
        9,
        "tri",
        "earthmesh",
        "MPAS-Simple",
    )
    .expect_err("non-atmos rejected");
    assert!(err.to_string().contains("atmosmesh"));

    let err = earthmesh_cli::mask_postproc_atmos::write_mask_postproc_atmos_mpas_simple_netcdf(
        std::env::temp_dir(),
        9,
        "tri",
        "atmosmesh",
        "MPAS",
    )
    .expect_err("full MPAS rejected");
    assert!(err.to_string().contains("MPAS-Simple"));
}

fn write_cellwidth_fixture(path: &std::path::Path, values: &[f64]) {
    let mut file = earthmesh_cli::create_netcdf_quiet(path).expect("create cellwidth fixture");
    file.add_dimension("num_dbx", values.len())
        .expect("num_dbx dim");
    let mut var = file
        .add_variable::<f64>("cellwidth", &["num_dbx"])
        .expect("cellwidth var");
    var.put_values(values, ..).expect("cellwidth values");
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
}
