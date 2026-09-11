use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use earthmesh_project::{
    ColmMeshDeliveryConfig, DomainConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset,
    ModelFormat, ProjectConfig, ProjectDeliveryConfig, ResolutionSpec, ViolationPolicy,
};

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "earthmesh_project_colm_delivery_{name}_{}_{}_{}",
        std::process::id(),
        nonce,
        sequence
    ))
}

fn base_project(name: &str, cell: MeshCellKind) -> ProjectConfig {
    let mut project = ProjectConfig::scaffold(
        name,
        MeshIntentPreset::Custom,
        DomainConfig::Global,
        ResolutionSpec::Nxp(10),
    );
    project.domain = DomainConfig::Global;
    project.target.kind = MeshDomainKind::Earth;
    project.target.cell = cell;
    project.target.model_format = ModelFormat::CoLM;
    project.target.resolution = ResolutionSpec::Nxp(10);
    project.data_layers.clear();
    project.refinement.enabled = false;
    project.refinement.threshold_enabled = false;
    project.refinement.max_passes = 0;
    project.quality.on_violation = ViolationPolicy::Warn;
    project.expert.openmp = Some(1);
    project.expert.niter = Some(1);
    project
}

fn run_project(root: &Path, project: &ProjectConfig) -> Output {
    fs::create_dir_all(root).unwrap();
    let project_path = root.join("project.yaml");
    fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(root)
        .args([
            "--project",
            project_path.to_str().unwrap(),
            "--max-tris",
            "100000",
            "--quiet",
        ])
        .output()
        .expect("run earthmesh_cli --project")
}

fn stdout_line<'a>(stdout: &'a str, prefix: &str) -> Option<&'a str> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
}

fn find_colm_meshes(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_colm_meshes(&path, found);
        } else if path.file_name().is_some_and(|name| {
            name.to_string_lossy().starts_with("CoLM_")
                && name.to_string_lossy().ends_with("_mesh.nc")
        }) {
            found.push(path);
        }
    }
}

fn assert_colm_mesh_report(path: &Path, expected_nlon: usize, expected_nlat: usize) {
    let file = netcdf::open(path).unwrap();
    assert_eq!(
        file.dimension("nlon").unwrap().len(),
        expected_nlon,
        "nlon for {}",
        path.display()
    );
    assert_eq!(
        file.dimension("nlat").unwrap().len(),
        expected_nlat,
        "nlat for {}",
        path.display()
    );
    let elmindex = file.variable("elmindex").unwrap();
    let dims = elmindex
        .dimensions()
        .iter()
        .map(|dim| dim.name())
        .collect::<Vec<_>>();
    assert_eq!(dims, ["nlat", "nlon"], "CoLM disk dimension order");
    let values = elmindex.get_values::<i32, _>(..).unwrap();
    assert!(
        values.iter().any(|value| *value > 0),
        "export should assign at least one positive pixel"
    );
    let cell_ids = file
        .variable("cell_id")
        .unwrap()
        .get_values::<i32, _>(..)
        .unwrap();
    assert!(
        cell_ids.iter().any(|value| *value > 0),
        "export should contain native cell IDs"
    );
}

#[test]
fn project_cli_colm_mesh_delivery_emits_only_after_successful_project_gate() {
    for (case, cell) in [("hex", MeshCellKind::Hex), ("tri", MeshCellKind::Tri)] {
        let root = temp_root(case);
        let mut project = base_project(case, cell);
        project.delivery = ProjectDeliveryConfig {
            colm_mesh: Some(ColmMeshDeliveryConfig {
                pixels_per_degree: 1,
            }),
        };

        let output = run_project(&root, &project);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{case} delivery should succeed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        let exported = stdout_line(&stdout, "colm_mesh_input=")
            .unwrap_or_else(|| panic!("{case} stdout missing colm_mesh_input line:\n{stdout}"));
        let exported = PathBuf::from(exported);
        assert!(
            exported.exists(),
            "{case} reported CoLM mesh should exist: {}",
            exported.display()
        );
        assert!(
            exported.to_string_lossy().contains("/standard/CoLM_"),
            "{case} export should be under selected grid standard delivery dir: {}",
            exported.display()
        );
        assert_colm_mesh_report(&exported, 360, 180);
        assert!(
            stdout.contains("colm_mesh_pixels_per_degree=1 colm_mesh_shape=360x180"),
            "{case} stdout should report requested PPD and actual shape:\n{stdout}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    let legacy_root = temp_root("legacy_off");
    let legacy = base_project("legacy_off", MeshCellKind::Hex);
    let legacy_output = run_project(&legacy_root, &legacy);
    let legacy_stdout = String::from_utf8_lossy(&legacy_output.stdout);
    let legacy_stderr = String::from_utf8_lossy(&legacy_output.stderr);
    assert!(
        legacy_output.status.success(),
        "legacy-off project should still succeed\nstdout:\n{legacy_stdout}\nstderr:\n{legacy_stderr}"
    );
    assert!(
        !legacy_stdout.contains("colm_mesh_input="),
        "legacy project must not emit CoLM delivery without opt-in:\n{legacy_stdout}"
    );
    let mut legacy_meshes = Vec::new();
    find_colm_meshes(&legacy_root, &mut legacy_meshes);
    assert!(
        legacy_meshes.is_empty(),
        "legacy project must not write CoLM meshes: {legacy_meshes:?}"
    );
    fs::remove_dir_all(legacy_root).unwrap();

    let blocked_root = temp_root("block_fail");
    fs::create_dir_all(&blocked_root).unwrap();
    let project_path = blocked_root.join("project.yaml");
    let example_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/auto_refine.yaml");
    let blocked = fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()))
        .replace("model_format: Mpas", "model_format: CoLM")
        .replace(
            "refinement:\n",
            "refinement:\n  hfield:\n    enabled: true\n    max_level: 2\n    base_m: 1000.0\n",
        )
        .replace("!Nxp 40", "!Nxp 9")
        .replace("max_passes: 1", "max_passes: 2")
        .replace("on_violation: AutoRefine", "on_violation: Block")
        .replace(
            "data_layers: []\n",
            "data_layers: []\ndelivery:\n  colm_mesh:\n    pixels_per_degree: 1\n",
        );
    fs::write(&project_path, blocked).unwrap();
    let blocked_output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(&blocked_root)
        .args([
            "--project",
            project_path.to_str().unwrap(),
            "--max-tris",
            "100000",
            "--quiet",
        ])
        .output()
        .expect("run Project Block delivery CLI");
    let blocked_stdout = String::from_utf8_lossy(&blocked_output.stdout);
    let blocked_stderr = String::from_utf8_lossy(&blocked_output.stderr);
    assert!(
        !blocked_output.status.success(),
        "failing Project path should stop before delivery\nstdout:\n{blocked_stdout}\nstderr:\n{blocked_stderr}"
    );
    assert!(
        blocked_stderr.contains("h-field") || blocked_stderr.contains("project quality gate failed"),
        "failure should come from the Project refinement/quality path, not CoLM delivery:\n{blocked_stderr}"
    );
    assert!(
        !blocked_stdout.contains("colm_mesh_input="),
        "blocked project must not report CoLM delivery:\n{blocked_stdout}"
    );
    let mut blocked_meshes = Vec::new();
    find_colm_meshes(&blocked_root, &mut blocked_meshes);
    assert!(
        blocked_meshes.is_empty(),
        "blocked project must not write CoLM meshes: {blocked_meshes:?}"
    );
    fs::remove_dir_all(blocked_root).unwrap();
}
