//! Opt-in external-data E2E for a real MERIT-Hydro tile, a production
//! EarthMesh gridfile, and (when configured) a real CaMa map directory.
//!
//! Run through `scripts/run_real_hydro_e2e.sh`, or set the documented
//! `EARTHMESH_REAL_*` environment variables and invoke this ignored test.

use std::env;
use std::fs;
use std::path::PathBuf;

use earthmesh_project::{
    DomainConfig, HydroCoastConfig, MeshCellKind, MeshIntentPreset, ProjectConfig,
    ProjectLayerRole, RegionShape, ResolutionSpec,
};

fn required_path(name: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must point to a real external-data asset"))
}

fn env_f64(name: &str, default: f64) -> f64 {
    env::var(name)
        .ok()
        .map(|value| value.parse().unwrap_or_else(|_| panic!("invalid {name}")))
        .unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .map(|value| value.parse().unwrap_or_else(|_| panic!("invalid {name}")))
        .unwrap_or(default)
}

fn env_optional_usize(name: &str) -> Option<usize> {
    env::var(name)
        .ok()
        .map(|value| value.parse().unwrap_or_else(|_| panic!("invalid {name}")))
}

fn env_bool(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
}

#[test]
#[ignore = "requires real MERIT-Hydro/CaMa data and a production gridfile"]
fn real_merit_cama_and_production_gridfile_complete_project_hydro_stage() {
    let merit_root = required_path("EARTHMESH_REAL_MERIT_ROOT");
    let cama_root = env::var_os("EARTHMESH_REAL_CAMA_ROOT").map(PathBuf::from);
    let gridfile = required_path("EARTHMESH_REAL_GRIDFILE");
    let production_namelist = required_path("EARTHMESH_REAL_SOURCE_NAMELIST");
    let landtype = required_path("EARTHMESH_REAL_LANDTYPE");
    assert!(merit_root.is_dir(), "missing {}", merit_root.display());
    assert!(gridfile.is_file(), "missing {}", gridfile.display());
    assert!(
        production_namelist.is_file(),
        "missing {}",
        production_namelist.display()
    );
    assert!(landtype.is_file(), "missing {}", landtype.display());
    if let Some(path) = &cama_root {
        assert!(path.is_dir(), "missing {}", path.display());
    }

    // A native-resolution Pearl River sub-window containing a real CaMa R3
    // river mouth. Keeping the footprint small bounds overlay runtime without
    // sparse MERIT sampling or synthetic data.
    let west = env_f64("EARTHMESH_REAL_BBOX_W", 113.25);
    let south = env_f64("EARTHMESH_REAL_BBOX_S", 22.0);
    let east = env_f64("EARTHMESH_REAL_BBOX_E", 113.5);
    let north = env_f64("EARTHMESH_REAL_BBOX_N", 22.25);
    let expected_coarse_cell_count = env_usize("EARTHMESH_REAL_EXPECT_CELL_COUNT", 1);
    let stride = env::var("EARTHMESH_REAL_MERIT_STRIDE")
        .ok()
        .map(|value| value.parse().expect("invalid EARTHMESH_REAL_MERIT_STRIDE"))
        .unwrap_or(1);
    let cell = match env::var("EARTHMESH_REAL_CELL_KIND").as_deref() {
        Ok("tri") => MeshCellKind::Tri,
        Ok("hex") | Err(_) => MeshCellKind::Hex,
        Ok(other) => panic!("EARTHMESH_REAL_CELL_KIND must be hex or tri, got {other}"),
    };

    let mut project = ProjectConfig::scaffold(
        "real-hydro-e2e",
        MeshIntentPreset::MeritHydroCoast,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: west,
                e: east,
                s: south,
                n: north,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(40),
    );
    project.target.cell = cell;
    project
        .data_layers
        .iter_mut()
        .find(|layer| layer.role == ProjectLayerRole::LandType)
        .expect("hydro preset landtype layer")
        .path = landtype.display().to_string();
    let merit_layer = project
        .data_layers
        .iter_mut()
        .find(|layer| layer.role == ProjectLayerRole::MeritHydro)
        .expect("hydro preset MERIT layer");
    merit_layer.path = merit_root.display().to_string();
    merit_layer.enabled = true;
    // Keep the real-data acceptance depth explicit and bounded; the durable
    // script uses two passes to prove the adapter/engine refinement boundary.
    project.refinement.enabled = true;
    project.refinement.threshold_enabled = true;
    project.refinement.max_passes = u8::try_from(env_usize("EARTHMESH_REAL_MAX_PASSES", 1))
        .expect("EARTHMESH_REAL_MAX_PASSES exceeds u8");
    let expected_final_cell_count = env_optional_usize("EARTHMESH_REAL_EXPECT_FINAL_CELL_COUNT")
        .or((project.refinement.max_passes == 1).then_some(expected_coarse_cell_count));
    project.hydro_coast = Some(HydroCoastConfig {
        merit_root: merit_root.display().to_string(),
        cama_root: cama_root.as_ref().map(|path| path.display().to_string()),
        merit_stride: stride,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
        r3_upa_km2: 50_000.0,
        r2_upa_km2: 5_000.0,
        river_refinement_enabled: true,
        river_width_refinement_enabled: true,
        river_upstream_area_refinement_enabled: true,
        river_width_threshold_m: None,
        river_upstream_area_threshold_km2: None,
        coast_refinement_enabled: true,
        coast_buffer_km: 50.0,
        coast_land_refinement_enabled: true,
        coast_ocean_refinement_enabled: true,
    });
    project.validate().expect("real E2E Project config");

    let root = env::temp_dir().join(format!("earthmesh-real-hydro-e2e-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let project_path = root.join("project.yaml");
    fs::write(&project_path, project.to_yaml().unwrap()).unwrap();
    let source_namelist = root.join("mkgrd.nml");
    let production_source = fs::read_to_string(&production_namelist).unwrap();
    let source = if env_bool("EARTHMESH_REAL_KEEP_PRODUCTION_NITER") {
        production_source
    } else {
        // The default bounded E2E skips the source mesh's expensive initial
        // spring. Set EARTHMESH_REAL_KEEP_PRODUCTION_NITER=1 for parameter-parity.
        production_source.replace("NL%niter = 5000", "NL%niter = 0")
    };
    fs::write(&source_namelist, source).unwrap();
    let output = root.join("hydro_project");
    let report = earthmesh_cli::project_hydro_closed_loop::run_project_hydro_closed_loop(
        &project,
        &project_path,
        &source_namelist,
        &gridfile,
        &gridfile,
        &output,
        &root,
        200_000,
        Some(240),
    )
    .expect("run real Project hydro closed-loop E2E")
    .expect("configured hydro report");

    let analysis = report
        .final_analysis
        .as_ref()
        .expect("real hydro demand must execute final recomputation");
    assert_eq!(
        report.coarse.cell_count, expected_coarse_cell_count,
        "coarse bbox export must not include antipodal ghost cells"
    );
    if let Some(expected) = expected_final_cell_count {
        assert_eq!(
            analysis.cell_count, expected,
            "final bbox export must not include antipodal ghost cells"
        );
    }
    assert!(analysis.cells_geojson.is_file());
    assert!(analysis.corridors_geojson.is_file());
    assert!(report.manifest_path.is_file());
    assert!(analysis.hydro.manifest_path.is_file());
    assert!(analysis.hydro.intersection_cells > 0);
    assert!(
        analysis.hydro.estuary_coupling_rows > 0,
        "real CaMa estuary must be counted in the hydro-specific coupling summary"
    );
    assert!(report.refinement.is_some());
    assert!(report.final_gridfile.is_file());
    assert!(report
        .final_quality_dir
        .join("quality_summary.json")
        .is_file());
    let quality_summary =
        fs::read_to_string(report.final_quality_dir.join("quality_summary.json")).unwrap();
    assert!(
        !quality_summary.contains("\"hfield\": null"),
        "closed-loop quality must retain target-vs-actual HField diagnostics"
    );
    assert!(analysis
        .hydro
        .coupling_quality_path
        .as_ref()
        .is_some_and(|path| path.is_file()));
    assert_ne!(
        report.final_coupling_quality_verdict.as_deref(),
        Some("fail"),
        "production landtype coupling quality must not fail"
    );
    assert_ne!(
        report.final_quality_verdict,
        earthmesh_quality::QualityLevel::Fail,
        "refining the measured production parent grid must remain valid"
    );
    if project.refinement.max_passes >= 3 {
        assert!(
            report.quality_retry_applied,
            "deep real-data refinement must exercise the quality-gated HField retry"
        );
        let baseline_quality_report = output
            .join("deep_quality_retry")
            .join("baseline_quality")
            .join("quality_summary.json");
        assert!(
            baseline_quality_report.is_file(),
            "deep retry must preserve the baseline quality report"
        );
        let baseline_quality_summary = fs::read_to_string(&baseline_quality_report).unwrap();
        assert!(baseline_quality_summary.contains("\"verdict\": \"warn\""));
        assert_ne!(
            baseline_quality_summary, quality_summary,
            "accepted retry must not overwrite its auditable baseline report"
        );
        let decision = fs::read_to_string(
            output
                .join("deep_quality_retry")
                .join("candidate_quality")
                .join("auto_refine_decision.json"),
        )
        .unwrap();
        assert!(decision.contains(&format!(
            "\"baseline_quality_report\": \"{}\"",
            baseline_quality_report.display()
        )));
        assert!(decision.contains("\"regressions\": []"));
    }
    assert_ne!(report.final_gridfile, gridfile);
    if cama_root.is_some() {
        assert!(analysis.cama_reaches_geojson.as_ref().unwrap().is_file());
        assert!(analysis
            .cama_river_mouths_geojson
            .as_ref()
            .unwrap()
            .is_file());
        assert!(analysis.cama_reach_count > 0);
        assert!(
            analysis.cama_river_mouth_count > 0,
            "real CaMa E2E bbox must exercise the river-mouth path"
        );
        let coupling = fs::read_to_string(&analysis.hydro.coupling_csv_path).unwrap();
        assert!(
            coupling.lines().skip(1).any(|line| {
                line.contains("CaMa-Flood") && line.contains(",true,") && line.contains("cama-")
            }),
            "production CoLM coupling must retain a non-zero real CaMa estuary contribution"
        );
    }
    let manifest = fs::read_to_string(&report.manifest_path).unwrap();
    assert!(manifest.contains("earthmesh_project_hydro_closed_loop"));
    assert!(manifest.contains("\"plan_applied\": true"));
    assert!(manifest.contains("final_quality_verdict"));
    assert!(manifest.contains("final_coupling_quality_verdict"));
    eprintln!(
        "real_project_hydro_manifest={}",
        report.manifest_path.display()
    );
}
