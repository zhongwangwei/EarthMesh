use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "earthmesh_project_auto_refine_e2e_{}_{}",
        std::process::id(),
        nonce
    ))
}

fn find_named(root: &Path, name: &str, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_named(&path, name, found);
        } else if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            found.push(path);
        }
    }
}

fn json_usize(text: &str, key: &str) -> usize {
    let marker = format!("\"{key}\":");
    let tail = text
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing {marker} in quality report"))
        .1
        .trim_start();
    tail.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or_else(|error| panic!("parse {key} from {tail:?}: {error}"))
}

fn json_f64(text: &str, key: &str) -> f64 {
    let marker = format!("\"{key}\":");
    let tail = text
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing {marker} in quality report"))
        .1
        .trim_start();
    tail.chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E'))
        .collect::<String>()
        .parse()
        .unwrap_or_else(|error| panic!("parse {key} from {tail:?}: {error}"))
}

fn json_string(text: &str, key: &str) -> String {
    let marker = format!("\"{key}\": \"");
    text.split_once(&marker)
        .unwrap_or_else(|| panic!("missing {marker} in decision manifest"))
        .1
        .split_once('"')
        .unwrap_or_else(|| panic!("unterminated {key} in decision manifest"))
        .0
        .to_string()
}

fn json_stat_max(text: &str, key: &str) -> f64 {
    let marker = format!("\"{key}\":");
    let tail = text
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing {marker} in quality report"))
        .1;
    json_f64(tail, "max")
}

#[test]
fn project_cli_accepts_candidate_when_guarded_quality_strictly_improves() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("project.yaml");
    let example_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/auto_refine.yaml");
    fs::copy(&example_path, &project_path).unwrap_or_else(|error| {
        panic!("copy {}: {error}", example_path.display());
    });
    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(&root)
        .args([
            "--project",
            project_path.to_str().unwrap(),
            "--max-tris",
            "100000",
            "--quiet",
        ])
        .output()
        .expect("run Project AutoRefine CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(
        stderr.contains("auto_refine applying") && stderr.contains("pass 2"),
        "the complete CLI path must execute a local quality-repair retry:\n{stderr}"
    );
    let mut adapters = Vec::new();
    find_named(&root, "adapter.nml", &mut adapters);
    let adapter = adapters
        .iter()
        .find(|path| {
            path.to_string_lossy()
                .contains("quality_auto_refine/pass_2")
        })
        .unwrap_or_else(|| panic!("missing pass-2 adapter under {}", root.display()));
    let adapter_text = fs::read_to_string(adapter).unwrap();
    assert!(adapter_text.contains("RL%refine_spc = .TRUE."));
    assert!(adapter_text.contains("hfield_target_levels_json"));
    let mut quality_reports = Vec::new();
    find_named(&root, "quality_summary.json", &mut quality_reports);
    assert!(
        quality_reports.len() >= 2,
        "expected quality reports before and after repair, found {quality_reports:?}"
    );
    let initial_report = quality_reports
        .iter()
        .find(|path| !path.to_string_lossy().contains("quality_auto_refine"))
        .expect("initial quality report");
    let pass_2_report = quality_reports
        .iter()
        .find(|path| {
            path.to_string_lossy()
                .contains("quality_auto_refine/pass_2")
        })
        .expect("pass-2 quality report");
    let initial_quality = fs::read_to_string(initial_report).unwrap();
    let pass_2_quality = fs::read_to_string(pass_2_report).unwrap();
    for (label, quality) in [
        ("baseline", initial_quality.as_str()),
        ("pass-2 candidate", pass_2_quality.as_str()),
    ] {
        assert!(
            quality.contains("\"hfield\": {\"enabled\":true"),
            "{label} AutoRefine quality must include diagnostics from its actual engine namelist: {quality}"
        );
        assert_eq!(
            json_usize(quality, "target_above_actual_count"),
            0,
            "{label} must satisfy the complete original-plus-repair HField demand"
        );
    }
    assert!(
        json_usize(&pass_2_quality, "cell_count") > json_usize(&initial_quality, "cell_count"),
        "local repair must refine the measured mesh"
    );
    assert!(
        initial_quality.contains("\"verdict\": \"warn\"")
            && pass_2_quality.contains("\"verdict\": \"warn\""),
        "the compatible effective NXP case must remain publishable while guarded metrics improve"
    );
    assert!(
        json_stat_max(&pass_2_quality, "aspect_ratio")
            < json_stat_max(&initial_quality, "aspect_ratio")
            && json_stat_max(&pass_2_quality, "cell_edge_length_cv")
                < json_stat_max(&initial_quality, "cell_edge_length_cv"),
        "the pass-2 candidate must improve both guarded shape maxima"
    );
    assert_eq!(
        json_usize(&pass_2_quality, "non_manifold_vertex_fan_count"),
        0,
        "a single connected repair target must not introduce a non-manifold vertex fan"
    );

    let mut repair_plans = Vec::new();
    find_named(&root, "quality_repair_plan.json", &mut repair_plans);
    let initial_plan = repair_plans
        .iter()
        .find(|path| !path.to_string_lossy().contains("quality_auto_refine"))
        .expect("initial repair plan");
    assert!(
        fs::read_to_string(initial_plan)
            .unwrap()
            .contains("\"total_cells\": 1"),
        "AutoRefine must make one attributable local change per default pass"
    );

    let mut decisions = Vec::new();
    find_named(&root, "auto_refine_decision.json", &mut decisions);
    let decision = decisions
        .iter()
        .find(|path| {
            path.to_string_lossy()
                .contains("quality_auto_refine/pass_2")
        })
        .expect("pass-2 selection decision");
    let decision = fs::read_to_string(decision).unwrap();
    assert!(decision.contains("\"schema_version\": 1"));
    assert!(decision.contains("\"decision\": \"accepted\""));
    assert!(decision.contains("\"baseline_verdict\": \"warn\""));
    assert!(decision.contains("\"candidate_verdict\": \"warn\""));
    assert!(decision.contains("\"selected_verdict\": \"warn\""));
    assert!(decision.contains("\"regressions\": []"));
    for key in [
        "baseline_quality_report",
        "candidate_quality_report",
        "selected_quality_report",
    ] {
        let path = PathBuf::from(json_string(&decision, key));
        let path = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
        assert!(path.is_file(), "{key} does not exist: {}", path.display());
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_block_quality_includes_hfield_gates() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("project.yaml");
    let example_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/auto_refine.yaml");
    let project = fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()))
        .replace("!Nxp 40", "!Nxp 9")
        .replace("on_violation: AutoRefine", "on_violation: Block");
    fs::write(&project_path, &project).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(&root)
        .args([
            "--project",
            project_path.to_str().unwrap(),
            "--max-tris",
            "100000",
            "--quiet",
        ])
        .output()
        .expect("run Project Block CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr:\n{stderr}");
    let mut quality_reports = Vec::new();
    find_named(&root, "quality_summary.json", &mut quality_reports);
    let quality = fs::read_to_string(
        quality_reports
            .iter()
            .find(|path| !path.to_string_lossy().contains("quality_auto_refine"))
            .expect("Block quality report"),
    )
    .unwrap();
    assert!(
        quality.contains("\"hfield\": {\"enabled\":true"),
        "Block quality must include HField diagnostics and gates: {quality}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_cli_does_not_deepen_a_conforming_hfield_transition_warning() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("project.yaml");
    let example_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/auto_refine.yaml");
    let example = fs::read_to_string(&example_path).unwrap_or_else(|error| {
        panic!("read {}: {error}", example_path.display());
    });
    assert!(example.contains("  niter: 1"));
    assert!(example.contains("  niter_refine: 1"));
    let project = example
        .replace("  niter: 1", "  niter: 20")
        .replace("  niter_refine: 1", "  niter_refine: 20");
    fs::write(&project_path, &project).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(&root)
        .args([
            "--project",
            project_path.to_str().unwrap(),
            "--max-tris",
            "100000",
            "--quiet",
        ])
        .output()
        .expect("run Project AutoRefine no-op CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(
        stderr.contains("auto_refine kept the conforming HField mesh at pass 1")
            && !stderr.contains("auto_refine applying"),
        "a satisfied HField with only transition edge-CV warning must not be refined deeper:\n{stderr}"
    );

    let mut decisions = Vec::new();
    find_named(&root, "auto_refine_decision.json", &mut decisions);
    assert_eq!(
        decisions.len(),
        1,
        "no candidate decision should be emitted"
    );
    let decision_path = &decisions[0];
    let decision = fs::read_to_string(decision_path).unwrap();
    assert!(decision.contains("\"schema_version\": 1"));
    assert!(decision.contains("\"decision\": \"kept\""));
    assert!(decision.contains(
        "\"reason\": \"conforming HField transition warnings are not safely repaired by adding a refinement level\""
    ));
    assert!(decision.contains("\"baseline_verdict\": null"));
    assert!(decision.contains("\"candidate_verdict\": \"warn\""));
    assert!(decision.contains("\"selected_verdict\": \"warn\""));
    let candidate_gridfile = json_string(&decision, "candidate_gridfile");
    let selected_gridfile = json_string(&decision, "selected_gridfile");
    assert_eq!(selected_gridfile, candidate_gridfile);
    assert!(Path::new(&selected_gridfile).is_file());

    let warn_root = root.join("warn");
    fs::create_dir_all(&warn_root).unwrap();
    let warn_project_path = warn_root.join("project.yaml");
    fs::write(
        &warn_project_path,
        project.replace("on_violation: AutoRefine", "on_violation: Warn"),
    )
    .unwrap();
    let warn_output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(&warn_root)
        .args([
            "--project",
            warn_project_path.to_str().unwrap(),
            "--max-tris",
            "100000",
            "--quiet",
        ])
        .output()
        .expect("run matching Project Warn CLI");
    assert!(
        warn_output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&warn_output.stderr)
    );
    let mut warn_quality_reports = Vec::new();
    find_named(
        &warn_root,
        "quality_summary.json",
        &mut warn_quality_reports,
    );
    let warn_result_dir = warn_quality_reports
        .iter()
        .find(|path| !path.to_string_lossy().contains("quality_auto_refine"))
        .and_then(|path| path.parent())
        .expect("Warn result directory");
    let warn_gridfile = fs::read_dir(warn_result_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().and_then(|value| value.to_str()) == Some("nc4")
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.starts_with("gridfile_"))
        })
        .expect("Warn final gridfile");
    assert_eq!(
        fs::read(&selected_gridfile).unwrap(),
        fs::read(&warn_gridfile).unwrap(),
        "quality-policy strictness must not alter a conforming HField mesh when the bounded repair plan contains only transition edge-CV defects"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn project_cli_repairs_a_global_uniform_baseline_from_any_working_directory() {
    let root = temp_root();
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("project.yaml");
    let quickstart = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects/quickstart.yaml"),
    )
    .unwrap();
    let project = quickstart
        .replace("min_angle_deg: 25.0", "min_angle_deg: 120.0")
        .replace("on_violation: Warn", "on_violation: AutoRefine")
        .replace("expert: {}", "expert:\n  niter: 1\n  niter_refine: 1");
    fs::write(&project_path, project).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--project", project_path.to_str().unwrap(), "--quiet"])
        .output()
        .expect("run global uniform AutoRefine project");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(
        stderr.contains("auto_refine quality=warn level=0")
            && stderr.contains("auto_refine applying 1 local quality targets at pass 1"),
        "pass-zero uniform baseline must produce a pass-one local repair:\n{stderr}"
    );
    let mut decisions = Vec::new();
    find_named(&root, "auto_refine_decision.json", &mut decisions);
    assert!(decisions.iter().any(|path| {
        path.to_string_lossy()
            .contains("quality_auto_refine/pass_1")
    }));

    let _ = fs::remove_dir_all(root);
}
