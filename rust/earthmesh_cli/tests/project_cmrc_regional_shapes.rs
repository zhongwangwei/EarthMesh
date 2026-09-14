mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use earthmesh_project::{
    CertifiedDeliveryMode, CertifiedMode, CloseBoundaryMode, CloseMaskFormat,
    ColmMeshDeliveryConfig, DomainConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset,
    ModelFormat, ProjectConfig, ProjectDataLayer, ProjectDeliveryConfig, ProjectLayerRole,
    RefinementBackend, RegionShape, ResolutionSpec, SpecifiedBboxRefinement, ViolationPolicy,
};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "earthmesh_project_cmrc_regional_shapes_{name}_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn write_all_land(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let ppd = 120_usize;
    let mut file = earthmesh_cli::create_netcdf_quiet(path).unwrap();
    file.add_dimension("longitude", 360 * ppd).unwrap();
    file.add_dimension("latitude", 180 * ppd).unwrap();
    let mut var = file
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .unwrap();
    let start_lon = ((100.0 + 180.0) * ppd as f64) as usize;
    let end_lon = ((160.0 + 180.0) * ppd as f64) as usize + 1;
    let start_lat = ((90.0 - 50.0) * ppd as f64) as usize;
    let end_lat = ((90.0 - 0.0) * ppd as f64) as usize + 1;
    let values = vec![1_i8; (end_lon - start_lon) * (end_lat - start_lat)];
    var.put_values(&values, (start_lon..end_lon, start_lat..end_lat))
        .unwrap();
}

fn closed_rect(w: f64, e: f64, s: f64, n: f64) -> Vec<(f64, f64)> {
    vec![(w, s), (e, s), (e, n), (w, n), (w, s)]
}

fn write_polygon_shp(path: &Path, parts: &[Vec<(f64, f64)>]) {
    let total_points: usize = parts.iter().map(Vec::len).sum();
    let mut xmin = f64::INFINITY;
    let mut ymin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for (x, y) in parts.iter().flatten() {
        xmin = xmin.min(*x);
        ymin = ymin.min(*y);
        xmax = xmax.max(*x);
        ymax = ymax.max(*y);
    }
    let content_len = 44 + parts.len() * 4 + total_points * 16;
    let file_len = 100 + 8 + content_len;
    let mut file = Vec::with_capacity(file_len);
    file.extend(9994_i32.to_be_bytes());
    file.extend([0_u8; 20]);
    file.extend(((file_len / 2) as i32).to_be_bytes());
    file.extend(1000_i32.to_le_bytes());
    file.extend(5_i32.to_le_bytes());
    for value in [xmin, ymin, xmax, ymax, 0.0, 0.0, 0.0, 0.0] {
        file.extend(value.to_le_bytes());
    }
    file.extend(1_i32.to_be_bytes());
    file.extend(((content_len / 2) as i32).to_be_bytes());
    file.extend(5_i32.to_le_bytes());
    for value in [xmin, ymin, xmax, ymax] {
        file.extend(value.to_le_bytes());
    }
    file.extend((parts.len() as i32).to_le_bytes());
    file.extend((total_points as i32).to_le_bytes());
    let mut start = 0_i32;
    for part in parts {
        file.extend(start.to_le_bytes());
        start += part.len() as i32;
    }
    for part in parts {
        for (x, y) in part {
            file.extend(x.to_le_bytes());
            file.extend(y.to_le_bytes());
        }
    }
    fs::write(path, file).unwrap();
}

fn write_union_shapefile(root: &Path) -> PathBuf {
    let path = root.join("domain_union.shp");
    write_polygon_shp(
        &path,
        &[
            closed_rect(100.0, 115.0, 0.0, 20.0),
            closed_rect(145.0, 160.0, 30.0, 50.0),
        ],
    );
    path
}

fn domain(root: &Path, shape: &str) -> DomainConfig {
    let sea_ratio = Some(0.5);
    match shape {
        "bbox" => DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 100.0,
                e: 160.0,
                s: 0.0,
                n: 50.0,
            },
            sea_ratio,
        },
        "circle" => DomainConfig::Regional {
            shape: RegionShape::Circle {
                lon: 130.0,
                lat: 25.0,
                radius_km: 3500.0,
            },
            sea_ratio,
        },
        "close" => {
            let path = root.join("domain_close.nml");
            fs::write(
                &path,
                "close_num = 4\nclose_refine = 0\n100 0\n160 0\n160 50\n100 50\n",
            )
            .unwrap();
            DomainConfig::Regional {
                shape: RegionShape::Close {
                    path: path.display().to_string(),
                    format: CloseMaskFormat::Nml,
                    boundary: CloseBoundaryMode::Polyline,
                },
                sea_ratio,
            }
        }
        "union" => DomainConfig::Regional {
            shape: RegionShape::Shapefile {
                path: write_union_shapefile(root).display().to_string(),
            },
            sea_ratio,
        },
        _ => panic!("unknown shape {shape}"),
    }
}

fn project(
    root: &Path,
    shape: &str,
    kind: MeshDomainKind,
    cell: MeshCellKind,
    model: ModelFormat,
    colm_ppd: Option<usize>,
    landtype: Option<&Path>,
) -> ProjectConfig {
    fs::create_dir_all(root).unwrap();
    let name = root.file_name().unwrap().to_string_lossy();
    let mut p = ProjectConfig::scaffold(
        name.as_ref(),
        MeshIntentPreset::Custom,
        domain(root, shape),
        ResolutionSpec::Nxp(6),
    );
    p.target.kind = kind;
    p.target.cell = cell;
    p.target.model_format = model;
    p.target.resolution = ResolutionSpec::Nxp(6);
    p.data_layers = landtype
        .map(|landtype| {
            vec![ProjectDataLayer {
                id: "landtype".into(),
                role: ProjectLayerRole::LandType,
                path: landtype.display().to_string(),
                enabled: true,
                threshold_value: None,
            }]
        })
        .unwrap_or_default();
    p.refinement.enabled = true;
    p.refinement.threshold_enabled = false;
    p.refinement.specified_bbox = Some(SpecifiedBboxRefinement {
        w: 110.0,
        e: 150.0,
        s: 10.0,
        n: 40.0,
    });
    p.refinement.max_passes = 1;
    p.refinement.backend = RefinementBackend::Certified;
    p.refinement.certified.mode = CertifiedMode::SafeMotherOnly;
    p.refinement.certified.delivery = match cell {
        MeshCellKind::Tri => CertifiedDeliveryMode::Tri,
        MeshCellKind::Hex => CertifiedDeliveryMode::Hex,
    };
    p.refinement.certified.maximum_level = 1;
    p.refinement.certified.maximum_cells = 50_000;
    p.refinement.certified.search_budget = 100;
    p.refinement.adaptive = None;
    p.refinement.hfield = None;
    p.quality.on_violation = ViolationPolicy::Warn;
    p.expert.openmp = Some(1);
    p.expert.niter = Some(1);
    p.delivery = ProjectDeliveryConfig {
        colm_mesh: colm_ppd.map(|pixels_per_degree| ColmMeshDeliveryConfig { pixels_per_degree }),
    };
    p
}

fn run_project(root: &Path, p: &ProjectConfig) -> Output {
    fs::create_dir_all(root).unwrap();
    let yaml = root.join("project.yaml");
    fs::write(&yaml, p.to_yaml().unwrap()).unwrap();
    support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(root)
            .args([
                "--project",
                yaml.to_str().unwrap(),
                "--max-tris",
                "100000",
                "--quiet",
            ]),
    )
    .unwrap()
}

fn stdout_path(stdout: &str, key: &str) -> PathBuf {
    PathBuf::from(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| panic!("stdout missing {key}:\n{stdout}"))
            .trim(),
    )
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn quality_report(report: &serde_json::Value) -> serde_json::Value {
    read_json(&PathBuf::from(
        report["final_quality"]["report"].as_str().unwrap(),
    ))
}

fn assert_boundary_quality(report: &serde_json::Value) {
    let q = quality_report(report);
    assert_eq!(q["verdict"], "pass");
    assert!(q["topology"]["boundary_edge_count"].as_u64().unwrap() > 0);
}

fn assert_union_quality(report: &serde_json::Value) {
    let q = quality_report(report);
    assert!(
        matches!(q["verdict"].as_str(), Some("pass" | "warn")),
        "union final quality must not hard-fail: {q}"
    );
    assert!(q["topology"]["boundary_edge_count"].as_u64().unwrap() > 0);
    assert!(
        q["topology"]["connected_component_count"]
            .as_u64()
            .unwrap_or_default()
            > 1
            || q["topology"]["boundary_loop_count"]
                .as_u64()
                .unwrap_or_default()
                > 1,
        "union should expose multiple regional components or boundary loops: {q}"
    );
}

fn assert_colm(path: &Path, ppd: usize) {
    let file = netcdf::open(path).unwrap();
    let nlon = file.dimension("nlon").unwrap().len();
    let nlat = file.dimension("nlat").unwrap().len();
    assert!(nlon > 0 && nlon <= 360 * ppd, "nlon={nlon}");
    assert!(nlat > 0 && nlat <= 180 * ppd, "nlat={nlat}");
    let elmindex = file.variable("elmindex").unwrap();
    assert_eq!(
        elmindex
            .dimensions()
            .iter()
            .map(|dim| dim.name())
            .collect::<Vec<_>>(),
        ["nlat", "nlon"]
    );
    assert!(elmindex
        .get_values::<i32, _>(..)
        .unwrap()
        .iter()
        .any(|v| *v > 0));
    assert!(file
        .variable("cell_id")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap()
        .iter()
        .any(|v| *v > 0));
}

fn assert_graph(graph: &Path, mesh: &Path) {
    let file = netcdf::open(mesh).unwrap();
    let n_cells = file.dimension("nCells").unwrap().len();
    let cells_on_edge = file
        .variable("cellsOnEdge")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    let interior = cells_on_edge
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|edge| edge[0] > 0 && edge[1] > 0)
        .count();
    let header = fs::read_to_string(graph)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(header, vec![n_cells, interior]);
}

fn run_case(
    root: &Path,
    shape: &str,
    kind: MeshDomainKind,
    cell: MeshCellKind,
    model: ModelFormat,
    ppd: Option<usize>,
    landtype: Option<&Path>,
) {
    let name = format!("{shape}_{kind:?}_{cell:?}_{model:?}");
    let case_root = root.join(&name);
    let output = run_project(
        &case_root,
        &project(&case_root, shape, kind, cell, model, ppd, landtype),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{name}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout_path(&stdout, "project_final_gridfile=").exists());
    let report = read_json(&stdout_path(&stdout, "project_delivery_report="));
    assert_eq!(report["model_delivery_status"], "model_delivered");
    if shape == "union" {
        assert_union_quality(&report);
    } else {
        assert_boundary_quality(&report);
    }
    match model {
        ModelFormat::CoLM => assert_colm(
            &PathBuf::from(
                report["model_artifacts"]["colm_mesh_input"]
                    .as_str()
                    .unwrap(),
            ),
            ppd.unwrap(),
        ),
        ModelFormat::Mpas => {
            assert!(stdout.contains("mpas_parent_gridfile="));
            let mesh = PathBuf::from(
                report["model_artifacts"]["mpas_mesh_input"]
                    .as_str()
                    .unwrap(),
            );
            let graph = PathBuf::from(
                report["model_artifacts"]["mpas_graph_info"]
                    .as_str()
                    .unwrap(),
            );
            assert_graph(&graph, &mesh);
        }
        ModelFormat::Icon => {
            assert!(stdout.contains("icon_parent_gridfile="));
            assert!(PathBuf::from(
                report["model_artifacts"]["icon_mesh_input"]
                    .as_str()
                    .unwrap()
            )
            .exists());
        }
        _ => unreachable!(),
    }
}

fn standalone_land_nml(root: &Path, landtype: &Path) -> String {
    format!(
        "&mkgrd\n  NL%EXPNME='standalone_bbox_mpas'\n  NL%base_dir='{}/'\n  NL%NXP=6\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='MPAS'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%refine_backend='certified'\n  NL%mask_domain_global=.false.\n  NL%mask_domain_type='bbox'\n  NL%mask_domain_fprefix='inline:bbox:w=100,e=160,s=0,n=50'\n  NL%landtype_file='{}'\n  NL%openmp=1\n/\n&certified\n  NL%mode='safe_mother_only'\n  NL%delivery='hex'\n  NL%maximum_level=1\n  NL%maximum_cells=50000\n  NL%gradation_rings_per_level=3\n  NL%search_budget=100\n/\n",
        root.display(),
        landtype.display()
    )
}

fn standalone_atmos_nml(root: &Path, case: &str, cell: MeshCellKind, model: ModelFormat) -> String {
    let (mode_grid, output_format, delivery) = match (cell, model) {
        (MeshCellKind::Hex, ModelFormat::Mpas) => ("hex", "MPAS", "hex"),
        (MeshCellKind::Tri, ModelFormat::Icon) => ("tri", "ICON", "tri"),
        _ => unreachable!(),
    };
    format!(
        "&mkgrd
  NL%EXPNME='{case}'
  NL%base_dir='{}/'
  NL%NXP=6
  NL%mesh_type='atmosmesh'
  NL%mode_grid='{mode_grid}'
  NL%output_format='{output_format}'
  NL%mode_file='none'
  NL%mode_file_description='none'
  NL%refine=.true.
  NL%refine_backend='certified'
  NL%mask_domain_global=.false.
  NL%mask_domain_type='bbox'
  NL%mask_domain_fprefix='inline:bbox:w=100,e=160,s=0,n=50'
  NL%landtype_file='none'
  NL%openmp=1
/
&certified
  NL%mode='safe_mother_only'
  NL%delivery='{delivery}'
  NL%maximum_level=1
  NL%maximum_cells=50000
  NL%gradation_rings_per_level=3
  NL%search_budget=100
/
",
        root.display()
    )
}

fn standalone_atmos_multi_bbox_nml(root: &Path, bbox: &Path) -> String {
    standalone_atmos_nml(
        root,
        "standalone_atmos_multi_bbox_mpas",
        MeshCellKind::Hex,
        ModelFormat::Mpas,
    )
    .replace(
        "inline:bbox:w=100,e=160,s=0,n=50",
        &bbox.display().to_string(),
    )
}

fn find_legacy_delivery(root: &Path) -> PathBuf {
    let mut stack = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .is_some_and(|n| n == "legacy_delivery.json")
            {
                found.push(path);
            }
        }
    }
    assert_eq!(found.len(), 1, "{found:?}");
    found.pop().unwrap()
}

#[test]
fn project_cmrc_land_regional_shapes_deliver_model_artifacts() {
    let root = temp_root("matrix");
    fs::create_dir_all(&root).unwrap();
    let landtype = root.join("landtype_all_land.nc4");
    write_all_land(&landtype);
    let before = fs::metadata(&landtype).unwrap().len();
    for shape in ["bbox", "circle", "close"] {
        run_case(
            &root,
            shape,
            MeshDomainKind::Land,
            MeshCellKind::Tri,
            ModelFormat::CoLM,
            Some(1),
            Some(&landtype),
        );
        run_case(
            &root,
            shape,
            MeshDomainKind::Land,
            MeshCellKind::Hex,
            ModelFormat::CoLM,
            Some(1),
            Some(&landtype),
        );
    }
    for shape in ["bbox", "circle"] {
        run_case(
            &root,
            shape,
            MeshDomainKind::Land,
            MeshCellKind::Hex,
            ModelFormat::Mpas,
            None,
            Some(&landtype),
        );
        run_case(
            &root,
            shape,
            MeshDomainKind::Land,
            MeshCellKind::Tri,
            ModelFormat::Icon,
            None,
            Some(&landtype),
        );
    }
    assert_eq!(fs::metadata(&landtype).unwrap().len(), before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_cmrc_unmasked_regional_shapes_deliver_model_artifacts() {
    let root = temp_root("unmasked_matrix");
    fs::create_dir_all(&root).unwrap();
    for kind in [MeshDomainKind::Earth, MeshDomainKind::Atmosphere] {
        for shape in ["bbox", "circle", "close"] {
            run_case(
                &root,
                shape,
                kind,
                MeshCellKind::Hex,
                ModelFormat::Mpas,
                None,
                None,
            );
            run_case(
                &root,
                shape,
                kind,
                MeshCellKind::Tri,
                ModelFormat::Icon,
                None,
                None,
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_cmrc_region_union_shapes_deliver_model_artifacts() {
    let root = temp_root("union_matrix");
    fs::create_dir_all(&root).unwrap();
    let landtype = root.join("landtype_all_land.nc4");
    write_all_land(&landtype);
    for kind in [MeshDomainKind::Earth, MeshDomainKind::Atmosphere] {
        run_case(
            &root,
            "union",
            kind,
            MeshCellKind::Hex,
            ModelFormat::Mpas,
            None,
            None,
        );
        run_case(
            &root,
            "union",
            kind,
            MeshCellKind::Tri,
            ModelFormat::Icon,
            None,
            None,
        );
    }
    run_case(
        &root,
        "union",
        MeshDomainKind::Land,
        MeshCellKind::Tri,
        ModelFormat::CoLM,
        Some(1),
        Some(&landtype),
    );
    run_case(
        &root,
        "union",
        MeshDomainKind::Land,
        MeshCellKind::Hex,
        ModelFormat::CoLM,
        Some(1),
        Some(&landtype),
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_cmrc_atmos_multi_bbox_uses_final_delivery() {
    let root = temp_root("standalone_atmos_multi_bbox_mpas");
    fs::create_dir_all(&root).unwrap();
    let bbox = root.join("domain_bbox.nml");
    fs::write(
        &bbox,
        "bbox_num = 2\nbbox_refine = 0\n100 115 20 0\n145 160 50 30\n",
    )
    .unwrap();
    let nml = root.join("standalone.nml");
    fs::write(&nml, standalone_atmos_multi_bbox_nml(&root, &bbox)).unwrap();
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(&root)
            .args([nml.to_str().unwrap(), "--max-tris", "100000", "--quiet"]),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let marker = find_legacy_delivery(&root);
    let record = read_json(&marker);
    assert_eq!(record["model_delivery_status"], "model_delivered");
    assert_eq!(record["scope"], "regional_or_masked");
    assert_union_quality(&record);
    let mesh = PathBuf::from(
        record["model_artifacts"]["mpas_mesh_input"]
            .as_str()
            .unwrap(),
    );
    let graph = PathBuf::from(
        record["model_artifacts"]["mpas_graph_info"]
            .as_str()
            .unwrap(),
    );
    assert_graph(&graph, &mesh);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_cmrc_atmos_bbox_uses_final_delivery_without_landtype() {
    for (case, cell, model) in [
        (
            "standalone_atmos_bbox_mpas",
            MeshCellKind::Hex,
            ModelFormat::Mpas,
        ),
        (
            "standalone_atmos_bbox_icon",
            MeshCellKind::Tri,
            ModelFormat::Icon,
        ),
    ] {
        let root = temp_root(case);
        fs::create_dir_all(&root).unwrap();
        let nml = root.join("standalone.nml");
        fs::write(&nml, standalone_atmos_nml(&root, case, cell, model)).unwrap();
        let output = support::output(
            Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
                .current_dir(&root)
                .args([nml.to_str().unwrap(), "--max-tris", "100000", "--quiet"]),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "{case}
{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let marker = find_legacy_delivery(&root);
        let record = read_json(&marker);
        assert_eq!(record["model_delivery_status"], "model_delivered", "{case}");
        assert_eq!(record["scope"], "regional_or_masked", "{case}");
        assert_boundary_quality(&record);
        match model {
            ModelFormat::Mpas => {
                let mesh = PathBuf::from(
                    record["model_artifacts"]["mpas_mesh_input"]
                        .as_str()
                        .unwrap(),
                );
                let graph = PathBuf::from(
                    record["model_artifacts"]["mpas_graph_info"]
                        .as_str()
                        .unwrap(),
                );
                assert_graph(&graph, &mesh);
            }
            ModelFormat::Icon => assert!(PathBuf::from(
                record["model_artifacts"]["icon_mesh_input"]
                    .as_str()
                    .unwrap()
            )
            .exists()),
            _ => unreachable!(),
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn standalone_cmrc_land_bbox_uses_final_delivery_for_mpas() {
    let root = temp_root("standalone_bbox_mpas");
    fs::create_dir_all(&root).unwrap();
    let landtype = root.join("landtype_all_land.nc4");
    write_all_land(&landtype);
    let nml = root.join("standalone.nml");
    fs::write(&nml, standalone_land_nml(&root, &landtype)).unwrap();
    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(&root)
            .args([nml.to_str().unwrap(), "--max-tris", "100000", "--quiet"]),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let marker = find_legacy_delivery(&root);
    let record = read_json(&marker);
    assert_eq!(record["model_delivery_status"], "model_delivered");
    assert_eq!(record["scope"], "regional_or_masked");
    assert_boundary_quality(&record);
    let mesh = PathBuf::from(
        record["model_artifacts"]["mpas_mesh_input"]
            .as_str()
            .unwrap(),
    );
    let graph = PathBuf::from(
        record["model_artifacts"]["mpas_graph_info"]
            .as_str()
            .unwrap(),
    );
    assert_graph(&graph, &mesh);
    let marker_mtime = fs::metadata(marker).unwrap().modified().unwrap();
    assert!(marker_mtime >= fs::metadata(mesh).unwrap().modified().unwrap());
    assert!(marker_mtime >= fs::metadata(graph).unwrap().modified().unwrap());
    fs::remove_dir_all(root).unwrap();
}
