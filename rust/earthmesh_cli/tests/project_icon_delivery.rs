mod support;

use earthmesh_project::{
    DomainConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset, MethodCAlgorithm, ModelFormat,
    ProjectConfig, RefinementBackend, RegionShape, ResolutionSpec, SpecifiedCircleRefinement,
    SpecifiedCircleRefinements, ViolationPolicy,
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("project_icon_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn field<'a>(stdout: &'a str, key: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .unwrap_or_else(|| panic!("missing {key}: {stdout}"))
}

fn maybe_field<'a>(stdout: &'a str, key: &str) -> Option<&'a str> {
    stdout.lines().find_map(|line| line.strip_prefix(key))
}

fn run_project(root: &Path, project: &ProjectConfig, name: &str) -> std::process::Output {
    let path = root.join(format!("{name}.yaml"));
    fs::write(&path, project.to_yaml().unwrap()).unwrap();
    support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(root)
            .args([
                "--project",
                path.to_str().unwrap(),
                "--max-tris",
                "100000",
                "--quiet",
            ]),
    )
    .unwrap_or_else(|error| panic!("run Project CLI: {error}"))
}

fn icon_project(cell: MeshCellKind, domain: DomainConfig, nxp: usize) -> ProjectConfig {
    let mut project = ProjectConfig::scaffold(
        "icon_final",
        MeshIntentPreset::Custom,
        domain,
        ResolutionSpec::Nxp(nxp as i32),
    );
    project.target.cell = cell;
    project.target.kind = MeshDomainKind::Earth;
    project.target.model_format = ModelFormat::Icon;
    project.quality.on_violation = ViolationPolicy::Warn;
    project.refinement.enabled = false;
    project.expert.niter = Some(1);
    project
}

fn refined_icon_project(backend: RefinementBackend) -> ProjectConfig {
    let mut project = icon_project(MeshCellKind::Tri, DomainConfig::Global, 10);
    project.refinement.enabled = true;
    project.refinement.max_passes = 1;
    project.refinement.backend = backend;
    project.refinement.method_c.algorithm = MethodCAlgorithm::Canonical;
    project.refinement.specified_circle =
        Some(SpecifiedCircleRefinements::One(SpecifiedCircleRefinement {
            lon: 110.0,
            lat: 20.0,
            radius_km: 500.0,
        }));
    project.expert.niter_refine = Some(1);
    project
}

fn find_icon_artifacts(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_icon_artifacts(&path, found);
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("ICON_") && name.ends_with(".nc4"))
        {
            found.push(path);
        }
    }
}

fn assert_no_icon_artifact(root: &Path, stdout: &str) {
    assert!(
        maybe_field(stdout, "icon_mesh_input=").is_none(),
        "unexpected ICON stdout artifact: {stdout}"
    );
    let mut icons = Vec::new();
    find_icon_artifacts(root, &mut icons);
    assert!(icons.is_empty(), "unexpected ICON artifact: {icons:?}");
}

fn stdout_token<'a>(stdout: &'a str, key: &str) -> &'a str {
    stdout
        .split_whitespace()
        .find_map(|token| token.strip_prefix(key))
        .unwrap_or_else(|| panic!("missing {key}: {stdout}"))
}

fn stdout_usize(stdout: &str, key: &str) -> usize {
    stdout_token(stdout, key)
        .parse()
        .unwrap_or_else(|error| panic!("parse {key}: {error}"))
}

fn normalized_lon_rad(lon_rad: f64) -> f64 {
    lon_rad.rem_euclid(std::f64::consts::TAU)
}

fn rounded_pair(lon_rad: f64, lat_rad: f64) -> (i64, i64) {
    (
        (normalized_lon_rad(lon_rad) * 1.0e12).round() as i64,
        (lat_rad * 1.0e12).round() as i64,
    )
}

fn read_f64(file: &netcdf::File, name: &str) -> Vec<f64> {
    file.variable(name)
        .unwrap_or_else(|| panic!("missing ICON variable {name}"))
        .get_values::<f64, _>(..)
        .unwrap()
}

#[test]
fn project_icon_delivers_the_admitted_global_triangles() {
    let root = root("global_tri");
    let project = icon_project(MeshCellKind::Tri, DomainConfig::Global, 3);
    let result = run_project(&root, &project, "project");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stdout}\n{stderr}");

    assert!(
        stdout.find("project_final_quality=").unwrap() < stdout.find("icon_mesh_input=").unwrap(),
        "selected final admission must precede ICON delivery:\n{stdout}"
    );
    let final_quality = field(&stdout, "project_final_quality=");
    let icon_path = field(&stdout, "icon_mesh_input=");
    assert!(Path::new(icon_path).is_file());
    assert!(Path::new(icon_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("ICON_"));

    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(final_quality).unwrap()).unwrap();
    assert_eq!(report["verdict"], "pass");
    let selected = Path::new(report["mesh_name"].as_str().unwrap());
    assert!(
        selected.is_file(),
        "selected native missing: {}",
        selected.display()
    );

    let selected_mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(selected)
            .expect("read selected native");
    let nc = netcdf::open(icon_path).unwrap();
    let icon_cells = nc.dimension("cell").unwrap().len();
    let icon_vertices = nc.dimension("vertex").unwrap().len();
    let icon_edges = nc.dimension("edge").unwrap().len();
    assert_eq!(
        icon_cells,
        report["geometry"]["cell_count"].as_u64().unwrap() as usize
    );
    assert_eq!(icon_cells, stdout_usize(&stdout, "icon_cells="));
    assert_eq!(icon_vertices, stdout_usize(&stdout, "icon_vertices="));
    assert_eq!(icon_edges, stdout_usize(&stdout, "icon_edges="));
    assert_eq!(stdout_token(&stdout, "icon_global_grid="), "true");

    let clon = read_f64(&nc, "clon");
    let clat = read_f64(&nc, "clat");
    let icon_cell_coords = clon
        .iter()
        .zip(clat.iter())
        .map(|(&lon, &lat)| rounded_pair(lon, lat))
        .collect::<BTreeSet<_>>();
    let selected_cell_coords = selected_mesh
        .m_points
        .iter()
        .skip(1)
        .map(|point| rounded_pair(point.lon.to_radians(), point.lat.to_radians()))
        .collect::<BTreeSet<_>>();
    assert_eq!(icon_cell_coords, selected_cell_coords);

    let vertex_of_cell = nc
        .variable("vertex_of_cell")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    assert_eq!(vertex_of_cell.len(), icon_cells * 3);
    let selected_triangles = selected_mesh
        .m_to_w
        .iter()
        .skip(1)
        .map(|row| row.iter().map(|&id| id - 1).collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
    let icon_triangles = vertex_of_cell
        .chunks_exact(icon_cells)
        .fold(vec![Vec::new(); icon_cells], |mut by_cell, row| {
            for (cell, &vertex) in row.iter().enumerate() {
                by_cell[cell].push(vertex);
            }
            by_cell
        })
        .into_iter()
        .map(|row| row.into_iter().collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
    assert_eq!(icon_triangles, selected_triangles);
    nc.close().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_icon_hex_target_remains_grid_only_without_icon_artifact() {
    let root = root("hex_grid_only");
    let project = icon_project(MeshCellKind::Hex, DomainConfig::Global, 3);
    let result = run_project(&root, &project, "hex");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stdout}\n{stderr}");
    assert_no_icon_artifact(&root, &stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_icon_block_policy_allows_warn_quality_and_still_delivers_icon() {
    let root = root("blocked_quality_warn");
    let mut project = icon_project(MeshCellKind::Tri, DomainConfig::Global, 3);
    project.quality.min_angle_deg = 90.0;
    project.quality.on_violation = ViolationPolicy::Block;

    let result = run_project(&root, &project, "warn_block");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "Block policy intentionally permits warn-only quality:\n{stdout}\n{stderr}"
    );
    assert_eq!(field(&stdout, "project_final_quality_verdict="), "warn");
    assert!(Path::new(field(&stdout, "icon_mesh_input=")).is_file());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_icon_hfield_block_failure_publishes_no_icon_artifact() {
    let root = root("hfield_block_fail");
    let project_path = root.join("project.yaml");
    let example_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/auto_refine.yaml");
    let blocked = fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()))
        .replace("cell: Hex", "cell: Tri")
        .replace("model_format: Mpas", "model_format: Icon")
        .replace(
            "refinement:\n",
            "refinement:\n  hfield:\n    enabled: true\n    max_level: 2\n    base_m: 1000.0\n",
        )
        .replace("!Nxp 40", "!Nxp 9")
        .replace("max_passes: 1", "max_passes: 2")
        .replace("on_violation: AutoRefine", "on_violation: Block");
    fs::write(&project_path, blocked).unwrap();

    let output = support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(&root)
            .args([
                "--project",
                project_path.to_str().unwrap(),
                "--max-tris",
                "100000",
                "--quiet",
            ]),
    )
    .expect("run HField Block ICON Project CLI");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "HField Block Project should stop before ICON delivery:\n{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("h-field") || stderr.contains("project quality gate failed"),
        "failure must come from Project refinement/quality path, not ICON delivery:\n{stderr}"
    );
    assert_no_icon_artifact(&root, &stdout);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_icon_refined_tri_degree_seven_fails_closed_without_icon_artifact() {
    for (label, backend) in [
        ("method_c", RefinementBackend::MethodC),
        ("red_green", RefinementBackend::RedGreen),
    ] {
        let root = root(label);
        let project = refined_icon_project(backend);
        let result = run_project(&root, &project, label);
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "{label} unexpectedly succeeded:\n{stdout}\n{stderr}"
        );
        assert!(
            maybe_field(&stdout, "project_final_quality=").is_some(),
            "{label} must reach final admission before ICON schema rejection:\n{stdout}"
        );
        assert!(
            stderr.contains("ICON ne=6 cannot represent") && stderr.contains("degree 7"),
            "{label} must fail explicitly on ICON degree-7 schema, got:\n{stderr}"
        );
        assert_no_icon_artifact(&root, &stdout);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn project_icon_regional_tri_delivers_selected_native_triangles_with_parent_geometry() {
    let root = root("regional_tri");
    let project = icon_project(
        MeshCellKind::Tri,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: -150.0,
                e: -60.0,
                s: -50.0,
                n: 50.0,
            },
            sea_ratio: None,
        },
        6,
    );
    let result = run_project(&root, &project, "regional");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        result.status.success(),
        "regional ICON should deliver using an explicit closed parent:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.find("project_final_quality=").unwrap() < stdout.find("icon_mesh_input=").unwrap(),
        "selected final admission must precede ICON delivery:\n{stdout}"
    );
    let final_quality = field(&stdout, "project_final_quality=");
    let parent = field(&stdout, "icon_parent_gridfile=");
    let icon_path = field(&stdout, "icon_mesh_input=");
    assert!(
        Path::new(parent).is_file(),
        "missing ICON parent gridfile: {parent}"
    );
    assert!(Path::new(icon_path).is_file());
    assert_eq!(stdout_token(&stdout, "icon_global_grid="), "false");

    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(final_quality).unwrap()).unwrap();
    assert_eq!(report["verdict"], "pass");
    let selected = Path::new(report["mesh_name"].as_str().unwrap());
    let selected_points = earthmesh_cli::grid_quality_pipeline::read_gridfile_mesh_points(selected)
        .expect("read selected native");
    let selected_quality =
        earthmesh_cli::grid_quality_pipeline::quality_input_from_gridfile(&selected_points)
            .expect("selected triangle quality input");
    let nc = netcdf::open(icon_path).unwrap();
    let icon_cells = nc.dimension("cell").unwrap().len();
    let icon_vertices = nc.dimension("vertex").unwrap().len();
    assert_eq!(
        icon_cells,
        report["geometry"]["cell_count"].as_u64().unwrap() as usize
    );
    assert_eq!(icon_cells, stdout_usize(&stdout, "icon_cells="));
    assert_eq!(icon_vertices, stdout_usize(&stdout, "icon_vertices="));
    assert!(nc.attribute("earthmesh_geometry_parent").is_some());
    assert_eq!(
        nc.attribute("earthmesh_dual_area_scope")
            .unwrap()
            .value()
            .unwrap(),
        netcdf::AttributeValue::Str("full_parent_dual".into())
    );

    let clon = read_f64(&nc, "clon");
    let clat = read_f64(&nc, "clat");
    let icon_cell_coords = clon
        .iter()
        .zip(clat.iter())
        .map(|(&lon, &lat)| rounded_pair(lon, lat))
        .collect::<BTreeSet<_>>();
    let selected_cell_coords = selected_points
        .m_to_w
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row.iter().all(|&id| id > 0) && row[0] != row[1] && row[1] != row[2] && row[0] != row[2]
        })
        .map(|(row, _)| {
            rounded_pair(
                selected_points.m_lon[row].to_radians(),
                selected_points.m_lat[row].to_radians(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(icon_cell_coords, selected_cell_coords);

    let vlon = read_f64(&nc, "vlon");
    let vlat = read_f64(&nc, "vlat");
    let icon_vertex_coords = vlon
        .iter()
        .zip(vlat.iter())
        .map(|(&lon, &lat)| rounded_pair(lon, lat))
        .collect::<BTreeSet<_>>();
    let selected_vertex_coords = selected_quality
        .cells
        .iter()
        .flat_map(|cell| cell.vertices.iter().copied())
        .map(|row| selected_quality.vertices[row])
        .map(|point| rounded_pair(point.x.to_radians(), point.y.to_radians()))
        .collect::<BTreeSet<_>>();
    assert_eq!(icon_vertex_coords, selected_vertex_coords);

    let vertex_of_cell = nc
        .variable("vertex_of_cell")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    assert_eq!(vertex_of_cell.len(), icon_cells * 3);
    let icon_triangles = vertex_of_cell
        .chunks_exact(icon_cells)
        .fold(vec![Vec::new(); icon_cells], |mut by_cell, row| {
            for (cell, &vertex) in row.iter().enumerate() {
                let vertex = vertex as usize - 1;
                by_cell[cell].push(rounded_pair(vlon[vertex], vlat[vertex]));
            }
            by_cell
        })
        .into_iter()
        .map(|row| row.into_iter().collect::<BTreeSet<_>>())
        .collect::<BTreeSet<_>>();
    let selected_triangles = selected_quality
        .cells
        .iter()
        .map(|cell| {
            cell.vertices
                .iter()
                .map(|&row| {
                    let point = selected_quality.vertices[row];
                    rounded_pair(point.x.to_radians(), point.y.to_radians())
                })
                .collect::<BTreeSet<_>>()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(icon_triangles, selected_triangles);

    let adjacency = nc
        .variable("adjacent_cell_of_edge")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    let distances = read_f64(&nc, "edge_cell_distance");
    assert_eq!(adjacency.len(), distances.len());
    assert!(adjacency.contains(&-1));
    for (&cell, &distance) in adjacency.iter().zip(distances.iter()) {
        assert!(cell == -1 || (1..=icon_cells as i32).contains(&cell));
        if cell == -1 {
            assert_eq!(distance, 0.0);
        }
    }
    nc.close().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_regional_base_retains_parent_for_mpas_delivery() {
    let root = root("regional_base_mpas");
    let mut project = icon_project(
        MeshCellKind::Hex,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: -150.0,
                e: -60.0,
                s: -50.0,
                n: 50.0,
            },
            sea_ratio: None,
        },
        6,
    );
    project.target.model_format = ModelFormat::Mpas;
    let result = run_project(&root, &project, "regional_base_mpas");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stdout}\n{stderr}");
    let parent = Path::new(field(&stdout, "mpas_parent_gridfile="));
    let output = Path::new(field(&stdout, "mpas_mesh_input="));
    assert!(parent.is_file() && output.is_file());
    assert!(Path::new(field(&stdout, "mpas_graph_info=")).is_file());
    assert!(
        stdout.find("project_final_quality=").unwrap() < stdout.find("mpas_mesh_input=").unwrap()
    );
    let quality: serde_json::Value =
        serde_json::from_slice(&fs::read(field(&stdout, "project_final_quality=")).unwrap())
            .unwrap();
    let selected = Path::new(quality["mesh_name"].as_str().unwrap());
    assert_ne!(parent, selected);
    let context = earthmesh_cli::mpas_gridfile_context::read_mpas_gridfile_context(selected)
        .unwrap()
        .unwrap();
    assert_eq!(context.source, "gridinit_uniform_base");
    assert_eq!(
        context.cellwidth_km,
        vec![1280.0; context.cellwidth_km.len()]
    );
    let mesh =
        earthmesh_cli::unstructured_mesh_io::read_unstructured_mesh_netcdf(selected).unwrap();
    let file = netcdf::open(output).unwrap();
    assert_eq!(
        file.dimension("nCells").unwrap().len(),
        mesh.n_w_to_m.iter().filter(|&&n| n > 1).count()
    );
    assert!(file
        .variable("meshDensity")
        .unwrap()
        .get_values::<f64, _>(..)
        .unwrap()
        .iter()
        .all(|&d| d == 1.0));
    drop(file);
    fs::remove_dir_all(root).unwrap();
}
