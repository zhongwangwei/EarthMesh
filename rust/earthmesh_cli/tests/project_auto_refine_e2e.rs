use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temp_root() -> PathBuf {
    // `as_nanos` reports whole microseconds on macOS, and the harness starts
    // these tests within the same microsecond, so pid+time alone handed two
    // tests the same directory: one test's write tore the other's read, and
    // whichever finished first deleted the other's outputs. The counter makes
    // the name unique within the process; pid+time still separates runs.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "earthmesh_project_auto_refine_e2e_{}_{}_{}",
        std::process::id(),
        nonce,
        sequence
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
    // Asserts h-field diagnostics reached the report from the real engine
    // namelist, and the h-field is opt-in now that point+radius is the default.
    let project = fs::read_to_string(&example_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", example_path.display()))
        .replace(
            "refinement:\n",
            "refinement:\n  hfield:\n    enabled: true\n",
        );
    fs::write(&project_path, project).unwrap();
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
        initial_quality.contains("\"verdict\": \"fail\"")
            && pass_2_quality.contains("\"verdict\": \"warn\""),
        "the compatible effective NXP case must improve the baseline verdict from fail to warn"
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
    assert!(decision.contains("\"baseline_verdict\": \"fail\""));
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
        // These exercise the h-field's own gates, and the h-field is opt-in now.
        .replace(
            "refinement:\n",
            "refinement:\n  hfield:\n    enabled: true\n",
        )
        .replace("!Nxp 40", "!Nxp 9")
        .replace("on_violation: AutoRefine", "on_violation: Block");
    fs::write(&project_path, project).unwrap();

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
fn project_cli_rejects_a_real_refined_candidate_when_guarded_quality_regresses() {
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
        .replace(
            "refinement:\n",
            "refinement:\n  hfield:\n    enabled: true\n",
        )
        .replace("  niter: 1", "  niter: 20")
        .replace("  niter_refine: 1", "  niter_refine: 20");
    fs::write(&project_path, project).unwrap();

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
        .expect("run Project AutoRefine rejection CLI");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr:\n{stderr}");
    assert!(
        stderr.contains("auto_refine applying") && stderr.contains("auto_refine rejected pass 2"),
        "the complete CLI path must generate and reject a real candidate:\n{stderr}"
    );

    let mut decisions = Vec::new();
    find_named(&root, "auto_refine_decision.json", &mut decisions);
    let decision_path = decisions
        .iter()
        .find(|path| {
            path.to_string_lossy()
                .contains("quality_auto_refine/pass_2")
        })
        .expect("pass-2 rejection decision");
    let decision = fs::read_to_string(decision_path).unwrap();
    assert!(decision.contains("\"schema_version\": 1"));
    assert!(decision.contains("\"decision\": \"rejected\""));
    assert!(decision.contains("\"baseline_verdict\": \"warn\""));
    assert!(decision.contains("\"candidate_verdict\": \"warn\""));
    assert!(decision.contains("\"selected_verdict\": \"warn\""));
    assert!(decision.contains("\"regressions\": ["));
    assert!(decision.contains("\"metric\": \"aspect_ratio.max\""));
    assert!(decision.contains("\"preferred\": \"lower\""));
    for key in ["baseline", "candidate", "delta"] {
        assert!(
            decision.contains(&format!("\"{key}\":")),
            "rejection manifest is missing {key}:\n{decision}"
        );
    }

    let baseline_gridfile = json_string(&decision, "baseline_gridfile");
    let candidate_gridfile = json_string(&decision, "candidate_gridfile");
    let selected_gridfile = json_string(&decision, "selected_gridfile");
    assert_eq!(selected_gridfile, baseline_gridfile);
    assert_ne!(selected_gridfile, candidate_gridfile);
    for path in [&baseline_gridfile, &candidate_gridfile, &selected_gridfile] {
        assert!(
            root.join(path).is_file(),
            "missing gridfile {}",
            root.join(path).display()
        );
    }

    let baseline_report = json_string(&decision, "baseline_quality_report");
    let candidate_report = json_string(&decision, "candidate_quality_report");
    let selected_report = json_string(&decision, "selected_quality_report");
    assert_eq!(selected_report, baseline_report);
    assert_ne!(selected_report, candidate_report);
    let baseline_quality = fs::read_to_string(root.join(&baseline_report)).unwrap();
    let candidate_quality = fs::read_to_string(root.join(&candidate_report)).unwrap();
    assert!(
        json_usize(&candidate_quality, "cell_count") > json_usize(&baseline_quality, "cell_count"),
        "the rejected candidate must be a genuinely refined engine output"
    );
    let guarded_shape_regressed = json_stat_max(&candidate_quality, "aspect_ratio")
        > json_stat_max(&baseline_quality, "aspect_ratio")
        || json_stat_max(&candidate_quality, "cell_edge_length_cv")
            > json_stat_max(&baseline_quality, "cell_edge_length_cv")
        || json_stat_max(&candidate_quality, "angle_deviation_deg")
            > json_stat_max(&baseline_quality, "angle_deviation_deg")
        || json_f64(&candidate_quality, "min_angle_deg")
            < json_f64(&baseline_quality, "min_angle_deg");
    assert!(
        guarded_shape_regressed,
        "a natural guarded-metric regression must explain the rejection"
    );
    assert_eq!(
        fs::read_to_string(root.join(selected_report)).unwrap(),
        baseline_quality,
        "the selected quality artifact must remain the baseline report"
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
    assert!(decisions.iter().any(|path| path
        .to_string_lossy()
        .contains("quality_auto_refine/pass_1")));

    let _ = fs::remove_dir_all(root);
}
