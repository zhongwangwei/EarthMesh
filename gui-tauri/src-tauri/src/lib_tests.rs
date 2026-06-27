use super::*;
use earthmesh_project::{
    default_mask_sea_ratio, DomainConfig, MeshIntentPreset, ProjectConfig, ProjectLayerRole,
    RegionShape, ResolutionSpec,
};
use std::{env, fs, path::Path, process};
#[test]
fn parses_quality_summary_fields_and_warnings() {
    let json = r#"{
        "verdict": "warn",
        "geometry": { "cell_count": 1200, "vertex_count": 640, "edge_count": 1830, "min_angle_deg": 22.5 },
        "gates": [
            { "metric": "min_angle_deg", "value": 22.5, "level": "warn" },
            { "metric": "aspect_ratio", "value": 2.0, "level": "pass" }
        ],
        "topology_issues": [
            { "issue_type": "duplicate_edge", "severity": "fail", "message": "x" }
        ]
    }"#;
    let q = quality::parse_quality_summary(json, Path::new("/no/such/dir")).unwrap();
    assert_eq!(q.verdict, "warn");
    assert_eq!(q.cell_count, 1200);
    assert_eq!(q.vertex_count, 640);
    assert_eq!(q.min_angle_deg, 22.5);
    assert!(q
        .warnings
        .iter()
        .any(|w| w.contains("min_angle_deg [warn]")));
    assert!(q
        .warnings
        .iter()
        .any(|w| w.contains("topology: duplicate_edge [fail]")));
    // pass-level gate must not show up as a warning
    assert!(!q.warnings.iter().any(|w| w.contains("aspect_ratio")));
    assert!(q.report_path.is_none());
}
fn preset_yaml(name: &str, intent: MeshIntentPreset) -> String {
    scaffold_project(name.to_string(), intent.id().to_string(), Some(40), None)
        .expect("scaffold project")
}
fn hydrology_yaml(name: &str) -> String {
    preset_yaml(name, MeshIntentPreset::HydrologyLand)
}
fn circle_project(name: &str) -> ProjectConfig {
    ProjectConfig::scaffold(
        name,
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Regional {
            shape: RegionShape::Circle {
                lon: 113.0,
                lat: 22.5,
                radius_km: 100.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(40),
    )
}
#[test]
fn set_domain_bbox_rejects_invalid_coordinates() {
    let yaml = hydrology_yaml("bbox_test");
    let err = set_domain_bbox(yaml, 115.0, 112.0, 21.5, 23.5, None).unwrap_err();
    assert!(err.contains("bbox west must be < east"));
    let yaml = hydrology_yaml("bbox_test");
    let err = set_domain_bbox(yaml, 112.0, 112.0, 21.5, 23.5, None).unwrap_err();
    assert!(err.contains("bbox west must be < east"));
    let yaml = hydrology_yaml("bbox_test");
    let err = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 21.5, None).unwrap_err();
    assert!(err.contains("bbox south must be < north"));
}
#[test]
fn set_domain_bbox_rejects_invalid_sea_ratio() {
    let yaml = preset_yaml("sea_ratio_test", MeshIntentPreset::CoastalOcean);
    let err = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 23.5, Some(1.5)).unwrap_err();
    assert!(err.contains("domain sea_ratio must be between 0 and 1"));
}
#[test]
fn project_summary_reports_regional_sea_ratio() {
    let yaml = preset_yaml("sea_ratio_test", MeshIntentPreset::CoastalOcean);
    let yaml = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 23.5, Some(0.25)).expect("set bbox");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.sea_ratio, Some(0.25));
}
#[test]
fn set_domain_bbox_uses_engine_default_sea_ratio_when_ui_omits_it() {
    let yaml = preset_yaml("default_sea_ratio_test", MeshIntentPreset::CoastalOcean);
    let yaml = set_domain_bbox(yaml, 112.0, 115.0, 21.5, 23.5, None).expect("set bbox");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.sea_ratio, Some(default_mask_sea_ratio()));
}
#[test]
fn project_summary_reports_approx_km_resolution() {
    let yaml = scaffold_project(
        "km_test".to_string(),
        "HydrologyLand".to_string(),
        None,
        Some(9.0),
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, None);
    assert_eq!(summary.approx_km, Some(9.0));
    assert_eq!(summary.effective_nxp, 40);
}
#[test]
fn project_summary_reports_target_cell_and_model_format() {
    let yaml = preset_yaml("ocean_test", MeshIntentPreset::CoastalOcean);
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.cell, "tri");
    assert_eq!(summary.model_format, "FVCOM");
    let landcover = summary
        .layers
        .iter()
        .find(|layer| layer.id == "landcover")
        .expect("landcover layer");
    assert_eq!(landcover.role_kind, "landcover");
    assert_eq!(landcover.role, "land type");
    let threshold = summary
        .layers
        .iter()
        .find(|layer| layer.role_kind == "threshold")
        .expect("threshold layer");
    assert!(threshold.role.starts_with("threshold · "));
}
#[test]
fn project_summary_reports_hidden_regional_shape() {
    let cfg = circle_project("circle_test");
    let summary = project_summary(cfg.to_yaml().expect("yaml")).expect("summary");
    assert_eq!(summary.domain, "regional");
    assert_eq!(summary.domain_shape, "circle");
    assert_eq!(summary.bbox, None);
}
#[test]
fn scaffold_project_rejects_invalid_approx_km() {
    let err = scaffold_project(
        "bad_km".to_string(),
        "HydrologyLand".to_string(),
        None,
        Some(0.0),
    )
    .unwrap_err();
    assert!(err.contains("target resolution ApproxKm must be > 0"));
    let err = scaffold_project(
        "bad_nxp".to_string(),
        "HydrologyLand".to_string(),
        Some(0),
        None,
    )
    .unwrap_err();
    assert!(err.contains("target resolution Nxp must be > 0"));
    let err = scaffold_project(
        "bad_intent".to_string(),
        "TypoHydrologyLand".to_string(),
        Some(40),
        None,
    )
    .unwrap_err();
    assert!(err.contains("unknown mesh intent 'TypoHydrologyLand'"));
}
#[test]
fn set_project_metadata_writes_visible_project_fields() {
    let yaml = hydrology_yaml("old");
    let yaml = set_project_metadata(
        yaml,
        "new".to_string(),
        vec![" Alice ".to_string(), "".to_string()],
        "saved from UI".to_string(),
    )
    .expect("set metadata");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.name, "new");
    assert_eq!(summary.authors, vec!["Alice"]);
    assert_eq!(summary.description, "saved from UI");
}
#[test]
fn preserve_unexposed_project_fields_keeps_hidden_opened_config() {
    let mut base = circle_project("opened");
    base.metadata.authors = vec!["EarthMesh team".to_string()];
    base.metadata.description = "advanced settings".to_string();
    base.target.kind = earthmesh_project::MeshDomainKind::Ocean;
    base.target.cell = earthmesh_project::MeshCellKind::Tri;
    base.target.model_format = earthmesh_project::ModelFormat::Fvcom;
    base.expert.openmp = Some(8);
    base.hydro_coast = Some(earthmesh_project::HydroCoastConfig {
        merit_root: "/data/merit".to_string(),
        cama_root: Some("/data/cama".to_string()),
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    base.coupling = Some(earthmesh_project::CoupledMeshConfig {
        fraction_method: earthmesh_project::FractionMethod::ConservativeOverlay,
        identify_coastline: true,
        identify_river_mouth: true,
    });
    base.data_layers.push(earthmesh_project::ProjectDataLayer {
        id: "custom_threshold".to_string(),
        role: ProjectLayerRole::Threshold(earthmesh_project::ThresholdField::Lai),
        path: String::new(),
        enabled: false,
    });
    let edited = ProjectConfig::scaffold(
        "edited",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );
    let merged = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        true,
    )
    .expect("preserve hidden fields");
    let merged = ProjectConfig::from_yaml(&merged).expect("merged yaml");
    assert_eq!(merged.metadata.name, "edited");
    assert_eq!(merged.target.resolution, ResolutionSpec::Nxp(80));
    assert_eq!(merged.target.cell, earthmesh_project::MeshCellKind::Tri);
    assert_eq!(
        merged.target.model_format,
        earthmesh_project::ModelFormat::Fvcom
    );
    assert_eq!(merged.expert.openmp, Some(8));
    assert!(merged.hydro_coast.is_some());
    assert!(merged.coupling.is_some());
    assert!(matches!(
        merged.domain,
        DomainConfig::Regional {
            shape: RegionShape::Circle { .. },
            ..
        }
    ));
    assert!(merged
        .data_layers
        .iter()
        .any(|layer| layer.id == "custom_threshold"));
}
#[test]
fn preserve_unexposed_project_fields_keeps_user_bbox_edit_over_hidden_circle() {
    let base = circle_project("opened");
    let edited = ProjectConfig::scaffold(
        "edited",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 108.0,
                e: 120.0,
                s: 18.0,
                n: 26.0,
            },
            sea_ratio: Some(0.25),
        },
        ResolutionSpec::Nxp(80),
    );
    let merged = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        true,
    )
    .expect("preserve hidden fields");
    let merged = ProjectConfig::from_yaml(&merged).expect("merged yaml");
    assert!(matches!(
        merged.domain,
        DomainConfig::Regional {
            shape: RegionShape::Bbox { .. },
            sea_ratio: Some(0.25),
        }
    ));
}
#[test]
fn preserve_unexposed_project_fields_allows_global_override_of_hidden_circle() {
    let base = circle_project("opened");
    let edited = ProjectConfig::scaffold(
        "edited",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );
    let merged = preserve_unexposed_project_fields(
        base.to_yaml().expect("base yaml"),
        edited.to_yaml().expect("edited yaml"),
        false,
    )
    .expect("preserve hidden fields");
    let merged = ProjectConfig::from_yaml(&merged).expect("merged yaml");
    assert!(matches!(merged.domain, DomainConfig::Global));
}
#[test]
fn set_layer_path_rejects_enabled_empty_path() {
    let yaml = hydrology_yaml("layer_test");
    let err =
        set_layer_path(yaml.clone(), "landcover".to_string(), "".to_string(), true).unwrap_err();
    assert!(err.contains("data layer 'landcover' is enabled but has no path"));
    let yaml = set_layer_path(yaml, "landcover".to_string(), "".to_string(), false)
        .expect("disabled empty path is allowed");
    assert!(yaml.contains("enabled: false"));
}
#[test]
fn stage_threshold_layers_uses_engine_stems() {
    let root = env::temp_dir().join(format!(
        "earthmesh_studio_threshold_stage_{}",
        process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root");
    let src = root.join("terrain_slope_source.nc");
    fs::write(&src, b"slope").expect("write source");
    let mut cfg = ProjectConfig::scaffold(
        "threshold_stage",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(40),
    );
    let layer = cfg
        .data_layers
        .iter_mut()
        .find(|layer| matches!(layer.role, ProjectLayerRole::Threshold(_)))
        .expect("threshold layer");
    layer.path = src.to_string_lossy().into_owned();
    layer.enabled = true;
    let threshold_dir = root.join("threshold");
    assert!(engine::stage_threshold_layers(&cfg, &threshold_dir).expect("stage"));
    assert_eq!(
        fs::read(threshold_dir.join("slope_avg.nc")).expect("read staged"),
        b"slope"
    );
    let _ = fs::remove_dir_all(&root);
}
#[test]
fn set_quality_rejects_invalid_min_angle() {
    let yaml = hydrology_yaml("quality_test");
    let err = set_quality(yaml, 0.0, false).unwrap_err();
    assert!(err.contains("quality min_angle_deg must be > 0"));
}
#[test]
fn set_refinement_rejects_too_many_passes() {
    let yaml = hydrology_yaml("refine_test");
    let err = set_refinement(yaml, true, 10).unwrap_err();
    assert!(err.contains("refinement max_passes must be <= 9"));
}
#[test]
fn set_refinement_rejects_zero_passes_when_enabled() {
    let yaml = hydrology_yaml("refine_test");
    let err = set_refinement(yaml, true, 0).unwrap_err();
    assert!(err.contains("refinement max_passes must be > 0"));
}
#[test]
fn set_refinement_allows_zero_passes_when_disabled() {
    let yaml = hydrology_yaml("refine_test");
    let yaml = set_refinement(yaml, false, 8).expect("disabled refinement ignores pass count");
    let summary = project_summary(yaml).expect("summary");
    assert!(!summary.refine_enabled);
    assert_eq!(summary.max_passes, 0);
}
#[test]
fn list_criteria_reports_frontend_fields() {
    let criteria = list_criteria();
    let slope = criteria
        .iter()
        .find(|c| c.stem == "slope_avg")
        .expect("slope");
    assert_eq!(slope.label, "Slope");
    assert_eq!(slope.unit, "deg");
    assert_eq!(slope.physical_process, "orographic / runoff routing");
}
