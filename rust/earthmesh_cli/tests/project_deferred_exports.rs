mod support;

use earthmesh_project::{
    CloseBoundaryMode, CloseMaskFormat, DomainConfig, MeshCellKind, MeshDomainKind,
    MeshIntentPreset, ModelFormat, ProjectConfig, ProjectDataLayer, ProjectLayerRole,
    RefinementBackend, RegionShape, ResolutionSpec, ViolationPolicy,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("project_deferred_{name}_{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(&path).unwrap();
    path
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn field<'a>(stdout: &'a str, key: &str) -> &'a str {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .unwrap_or_else(|| panic!("missing {key}: {stdout}"))
}

fn run(root: &Path, project: &ProjectConfig) -> std::process::Output {
    let path = root.join("project.yaml");
    fs::write(&path, project.to_yaml().unwrap()).unwrap();
    support::output(
        Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(root)
            .arg("--project")
            .arg(&path)
            .args(["--max-tris", "100000", "--quiet"]),
    )
    .unwrap()
}

fn project() -> ProjectConfig {
    let mut p = ProjectConfig::scaffold(
        "deferred_exports",
        MeshIntentPreset::Custom,
        DomainConfig::Global,
        ResolutionSpec::Nxp(3),
    );
    p.target.kind = MeshDomainKind::Atmosphere;
    p.target.cell = MeshCellKind::Hex;
    p.target.model_format = ModelFormat::Mpas;
    p.refinement.backend = RefinementBackend::Certified;
    p.refinement.enabled = false;
    p.quality.on_violation = ViolationPolicy::Warn;
    p.data_layers.clear();
    p.expert.niter = Some(1);
    p
}

#[test]
fn project_cmrc_mpas_only_publishes_the_selected_final_model_bundle() {
    let root = root("cmrc_mpas");
    let output = run(&root, &project());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    let mesh = Path::new(field(&stdout, "mpas_mesh_input="));
    let graph = Path::new(field(&stdout, "mpas_graph_info="));
    assert!(mesh.is_file() && graph.is_file());
    assert!(
        stdout.find("project_final_quality=").unwrap() < stdout.find("mpas_mesh_input=").unwrap()
    );
    let paths = files(&root);
    assert_compiled_target(&paths, "MPAS");
    assert_deferred_manifest(&paths);
    let early = paths
        .into_iter()
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("MPASOUT_")
        })
        .collect::<Vec<_>>();
    assert!(
        early.is_empty(),
        "pre-admission MPAS artifacts remain: {early:?}\n{stdout}\n{stderr}"
    );
    fs::remove_dir_all(root).unwrap();
}

fn assert_compiled_target(paths: &[PathBuf], format: &str) {
    let nml = paths
        .iter()
        .find(|p| p.file_name().unwrap() == "project.nml")
        .unwrap();
    let config =
        earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&fs::read_to_string(nml).unwrap())
            .unwrap();
    assert!(config.defer_model_exports);
    assert_eq!(config.output_format, format);
}

fn assert_deferred_manifest(paths: &[PathBuf]) {
    let manifest = paths
        .iter()
        .filter(|p| p.to_string_lossy().ends_with("_manifest.json"))
        .map(|p| serde_json::from_slice::<serde_json::Value>(&fs::read(p).unwrap()).unwrap())
        .find(|m| m["backend"] == "certified")
        .unwrap();
    for key in ["mpas", "mpas_graph_info", "fvcom_2dm"] {
        assert!(manifest[key].is_null(), "{key}: {manifest}");
    }
    for key in ["gridfile", "certificate", "resources", "ready"] {
        assert!(
            Path::new(manifest[key].as_str().unwrap()).is_file(),
            "{key}: {manifest}"
        );
    }
    let resources: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest["resources"].as_str().unwrap()).unwrap())
            .unwrap();
    assert!(resources["mpas"].is_null());
    for key in ["mpas", "mpas_graph_info", "fvcom_2dm"] {
        assert!(
            resources["artifact_bytes"][key].is_null(),
            "{key}: {resources}"
        );
    }
}

#[test]
fn project_ocean_defers_cmrc_and_gridinit_fvcom_until_selected_final_delivery() {
    let root = root("regional");
    let landtype = root.join("landtype.nc4");
    let mut f = earthmesh_cli::create_netcdf_quiet(&landtype).unwrap();
    // The Project CLI validates the real 120/degree source dimensions. Write
    // compressed ocean values explicitly; missing/fill values are not ocean.
    f.add_dimension("longitude", 43_200).unwrap();
    f.add_dimension("latitude", 21_600).unwrap();
    let mut land = f
        .add_variable::<i8>("landtype", &["longitude", "latitude"])
        .unwrap();
    land.set_chunking(&[360, 180]).unwrap();
    land.set_compression(1, true).unwrap();
    let stripe = vec![0_i8; 360 * 21_600];
    for lon in (0..43_200).step_by(360) {
        land.put_values(&stripe, (lon..lon + 360, ..)).unwrap();
    }
    f.close().unwrap();
    let close = root.join("domain.nml");
    fs::write(
        &close,
        "close_num=4\nclose_refine=0\n100 0\n160 0\n160 50\n100 50\n",
    )
    .unwrap();
    for backend in [RefinementBackend::Certified, RefinementBackend::MethodC] {
        for format in [ModelFormat::Fvcom, ModelFormat::Mpas] {
            let case = root.join(format!("{backend:?}_{format:?}"));
            fs::create_dir(&case).unwrap();
            let mut p = project();
            p.target.kind = MeshDomainKind::Ocean;
            p.target.cell = MeshCellKind::Tri;
            p.target.model_format = format;
            p.refinement.backend = backend;
            p.domain = DomainConfig::Regional {
                shape: RegionShape::Close {
                    path: close.display().to_string(),
                    format: CloseMaskFormat::Nml,
                    boundary: CloseBoundaryMode::Polyline,
                },
                sea_ratio: None,
            };
            p.data_layers = vec![ProjectDataLayer {
                id: "landtype".into(),
                role: ProjectLayerRole::LandType,
                path: landtype.display().to_string(),
                enabled: true,
                threshold_value: None,
            }];
            let output = run(&case, &p);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success(),
                "{backend:?}/{format:?}: {stdout}\n{stderr}"
            );
            let paths = files(&case);
            assert_compiled_target(
                &paths,
                if format == ModelFormat::Fvcom {
                    "FVCOM"
                } else {
                    "MPAS"
                },
            );
            if backend == RefinementBackend::Certified {
                assert_deferred_manifest(&paths);
            }
            let models: Vec<_> = paths
                .iter()
                .filter(|p| p.extension().is_some_and(|e| e == "2dm"))
                .collect();
            if format == ModelFormat::Fvcom {
                assert_eq!(models.len(), 1, "premature FVCOM: {models:?}");
                let delivered = Path::new(field(&stdout, "fvcom_mesh_input="));
                assert_eq!(
                    models[0].canonicalize().unwrap(),
                    delivered.canonicalize().unwrap()
                );
                assert!(
                    stdout.find("project_final_quality=").unwrap()
                        < stdout.find("fvcom_mesh_input=").unwrap()
                );
                let native_quality: serde_json::Value = serde_json::from_slice(
                    &fs::read(field(&stdout, "project_final_quality=")).unwrap(),
                )
                .unwrap();
                let native = Path::new(native_quality["mesh_name"].as_str().unwrap());
                let context =
                    earthmesh_cli::obc_boundary_io::read_gridfile_obc_order(native).unwrap();
                assert!(context.is_some(), "regional native mesh lost OBC context");
            } else {
                assert!(
                    models.is_empty(),
                    "grid-only target leaked FVCOM: {models:?}"
                );
                assert!(!stdout.contains("mpas_mesh_input="));
            }
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_hfield_block_keeps_diagnostics_without_model_artifacts() {
    let root = root("blocked");
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/auto_refine.yaml"),
    )
    .unwrap()
    .replace(
        "refinement:\n",
        "refinement:\n  hfield:\n    enabled: true\n    max_level: 2\n    base_m: 1000.0\n",
    )
    .replace("!Nxp 40", "!Nxp 9")
    .replace("max_passes: 1", "max_passes: 2")
    .replace("on_violation: AutoRefine", "on_violation: Block");
    let output = run(&root, &ProjectConfig::from_yaml(&text).unwrap());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("h-field") || stderr.contains("project quality gate failed"),
        "{stderr}"
    );
    assert!(!stdout.contains("mpas_mesh_input="));
    let paths = files(&root);
    // This HField failure occurs before final-quality artifacts are written;
    // the existing failure cleanup keeps the run manifest and stderr diagnostic.
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("run_manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "failed");
    assert!(!manifest["warnings"].as_array().unwrap().is_empty());
    assert!(!paths.iter().any(|p| {
        let name = p.file_name().unwrap().to_string_lossy();
        name.starts_with("MPASOUT_") || name == "mesh.nc4" || name == "graph.info"
    }));
    fs::remove_dir_all(root).unwrap();
}
