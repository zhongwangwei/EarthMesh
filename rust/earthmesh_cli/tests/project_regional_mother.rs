mod support;

use earthmesh_project::{
    DomainConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset, ProjectConfig, RefinementBackend,
    RegionShape, ResolutionSpec, ViolationPolicy,
};
use std::fs;

fn regional(backend: RefinementBackend, mother_levels: Option<u8>) -> ProjectConfig {
    let mut p = ProjectConfig::scaffold(
        "regional_mother",
        MeshIntentPreset::Custom,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 100.0,
                e: 112.0,
                s: 20.0,
                n: 32.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(102),
    );
    p.target.cell = MeshCellKind::Hex;
    p.target.kind = MeshDomainKind::Earth;
    p.quality.on_violation = ViolationPolicy::Warn;
    p.refinement.enabled = false;
    p.refinement.backend = backend;
    p.expert.regional_mother_levels = mother_levels;
    p.expert.niter = Some(200);
    p
}

/// Run a project; return its final quality summary and the run's stderr.
fn run(p: &ProjectConfig, name: &str) -> (serde_json::Value, String) {
    let root = std::env::temp_dir().join(format!("regional_mother_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("project.yaml");
    fs::write(&project_path, p.to_yaml().unwrap()).unwrap();
    let result = support::output(
        std::process::Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .current_dir(&root)
            .args(["--project", project_path.to_str().unwrap()]),
    )
    .unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
    assert!(result.status.success(), "{stdout}\n{stderr}");
    let quality = stdout
        .lines()
        .find_map(|line| line.strip_prefix("project_final_quality="))
        .unwrap_or_else(|| panic!("no final quality: {stdout}\n{stderr}"));
    let quality = serde_json::from_slice(&fs::read(quality).unwrap()).unwrap();
    let _ = fs::remove_dir_all(&root);
    (quality, stderr)
}

fn levels(quality: &serde_json::Value) -> Vec<(u64, u64)> {
    quality["refine_level_groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            (
                group["refine_level"].as_u64().unwrap(),
                group["cell_count"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_coarse_mother_refined_over_the_domain_matches_the_mesh_built_at_resolution() {
    let (flat, flat_log) = run(&regional(RefinementBackend::MethodC, Some(0)), "flat");
    assert!(!flat_log.contains("regional mother"), "{flat_log}");
    let flat_cells = flat["geometry"]["cell_count"].as_u64().unwrap();

    for backend in [RefinementBackend::MethodC, RefinementBackend::RedGreen] {
        let (mother, log) = run(&regional(backend, None), "mother");
        assert!(
            log.contains("regional mother NXP 51 (1 halvings coarser)"),
            "{backend:?}: {log}"
        );
        // Every cell of the domain is at the requested resolution: one level
        // below the mother, none coarser, none finer.
        let groups = levels(&mother);
        assert_eq!(groups.len(), 1, "{backend:?}: {groups:?}");
        let (level, cells) = groups[0];
        assert_eq!(level, 1, "{backend:?}");
        assert_eq!(Some(cells), mother["geometry"]["cell_count"].as_u64());
        let ratio = cells as f64 / flat_cells as f64;
        assert!(
            (0.9..=1.1).contains(&ratio),
            "{backend:?}: {cells} cells against {flat_cells} built at resolution"
        );
        assert_eq!(
            mother["hfield"]["target_actual_mismatch_count"], 0,
            "{backend:?}"
        );
        assert_ne!(mother["verdict"], "fail", "{backend:?}");
    }
}

#[test]
fn autorefine_repairs_a_regional_mother_at_the_levels_its_cells_record() {
    // The quality repair reads its target levels off the refined mesh, which
    // counts them from the mother: a repair one level deeper than the domain
    // must land one level deeper, not `regional_mother_levels` more.
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/projects/auto_refine.yaml");
    let project = fs::read_to_string(example)
        .unwrap()
        .replace(
            "refinement:\n",
            "refinement:\n  hfield:\n    enabled: true\n",
        )
        .replace("model_format: Mpas", "model_format: CoLM")
        .replace(
            "expert:\n",
            "expert:\n  hex_cell_evening: false\n  regional_mother_levels: 1\n",
        );
    let p = ProjectConfig::from_yaml(&project).unwrap();
    let (quality, log) = run(&p, "autorefine");
    assert!(
        log.contains("regional mother NXP 21 (1 halvings coarser)"),
        "{log}"
    );
    assert!(log.contains("auto_refine applying"), "{log}");
    let groups = levels(&quality);
    let deepest = groups.iter().map(|(level, _)| *level).max().unwrap();
    assert_eq!(
        deepest, 2,
        "domain at 1, circle and repairs at 2: {groups:?}"
    );
    assert!(
        groups
            .iter()
            .any(|&(level, cells)| level == 1 && cells > 1000),
        "{groups:?}"
    );
}
