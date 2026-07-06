use super::*;
use earthmesh_project::{
    default_mask_sea_ratio, CloseMaskFormat, DomainConfig, MeshIntentPreset, ProjectConfig,
    ProjectLayerRole, RegionShape, ResolutionSpec, SpecifiedCircleRefinement,
    SpecifiedCloseRefinement, ViolationPolicy,
};
use std::{env, fs, path::Path, process};
#[test]
fn parses_quality_summary_fields() {
    let json = r#"{
        "verdict": "warn",
        "cell_view": "hex",
        "geometry": { "cell_count": 1200, "vertex_count": 640, "edge_count": 1830, "min_angle_deg": 22.5 },
        "topology": {
            "triangle_cell_count": 1200,
            "pentagon_cell_count": 2,
            "hexagon_cell_count": 1188,
            "heptagon_cell_count": 3
        },
        "gates": [
            { "metric": "min_angle_deg", "value": 22.5, "level": "warn" },
            { "metric": "aspect_ratio", "value": 2.0, "level": "pass" }
        ]
    }"#;
    let q = quality::parse_quality_summary(json, Path::new("/no/such/dir")).unwrap();
    assert_eq!(q.verdict, "warn");
    assert_eq!(q.cell_view, "hex");
    assert_eq!(q.cell_count, 1200);
    assert_eq!(q.vertex_count, 640);
    assert_eq!(q.min_angle_deg, 22.5);
    assert_eq!(q.gates.len(), 2);
    assert_eq!(q.gates[0].level, "warn");
    assert!(q.cell_sides.contains(&("triangle".to_string(), 1200)));
    assert!(q.cell_sides.contains(&("pentagon".to_string(), 2)));
    assert!(q.cell_sides.contains(&("hexagon".to_string(), 1188)));
    assert!(q.report_path.is_none());
}

#[test]
fn mesh_kind_rejects_invalid_values() {
    assert_eq!(mesh_outputs::checked_mesh_kind(None).unwrap(), "hex");
    assert_eq!(mesh_outputs::checked_mesh_kind(Some("tri")).unwrap(), "tri");
    assert_eq!(mesh_outputs::checked_mesh_kind(Some("hex")).unwrap(), "hex");
    assert!(mesh_outputs::checked_mesh_kind(Some("")).is_err());
    assert!(mesh_outputs::checked_mesh_kind(Some("square"))
        .unwrap_err()
        .contains("mesh kind must be tri or hex"));
    match mesh_quality("missing.nc".to_string(), Some("square".to_string())) {
        Err(e) => assert!(e.contains("mesh kind must be tri or hex")),
        Ok(_) => panic!("invalid mesh_quality kind should fail"),
    }
    match mesh_cell_polygons("missing.nc".to_string(), "square".to_string(), None) {
        Err(e) => assert!(e.contains("mesh kind must be tri or hex")),
        Ok(_) => panic!("invalid mesh_cell_polygons kind should fail"),
    }
}
fn preset_yaml(name: &str, intent: MeshIntentPreset) -> String {
    scaffold_project(
        name.to_string(),
        intent.id().to_string(),
        Some(40),
        None,
        None,
    )
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
        Some(100.0),
        None,
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, None);
    assert_eq!(summary.approx_km, Some(100.0));
    assert_eq!(summary.effective_nxp, 80);
}

#[test]
fn project_summary_reports_approx_degree_resolution() {
    let yaml = scaffold_project(
        "degree_test".to_string(),
        "HydrologyLand".to_string(),
        None,
        None,
        Some(100.0 / 111.32),
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, None);
    assert_eq!(summary.approx_km, None);
    assert_eq!(summary.approx_degree, Some(100.0 / 111.32));
    assert_eq!(summary.effective_nxp, 80);
}

#[test]
fn scaffold_project_defaults_to_olam_100km_nxp() {
    let yaml = scaffold_project(
        "default_resolution".to_string(),
        "HydrologyLand".to_string(),
        None,
        None,
        None,
    )
    .expect("scaffold project");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.nxp, Some(80));
    assert_eq!(summary.effective_nxp, 80);
}

#[test]
fn project_summary_reports_target_cell_and_model_format() {
    let yaml = preset_yaml("ocean_test", MeshIntentPreset::CoastalOcean);
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.cell, "tri");
    assert_eq!(summary.quality_mode, "tri-strict");
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
fn set_domain_shapefile_reports_watershed_path() {
    let yaml = hydrology_yaml("watershed_test");
    let yaml = set_domain_shapefile(yaml, "input/watershed.shp".to_string(), Some(0.4))
        .expect("set shapefile domain");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.domain, "regional");
    assert_eq!(summary.domain_shape, "shapefile");
    assert_eq!(
        summary.watershed_path,
        Some("input/watershed.shp".to_string())
    );
    assert_eq!(summary.sea_ratio, Some(0.4));
}

#[test]
fn set_domain_close_reports_mask_source() {
    let yaml = hydrology_yaml("close_test");
    let yaml = set_domain_close(
        yaml,
        "input/Ocean/Ocean_ChinaSea_boundary.nml".to_string(),
        "nml".to_string(),
        Some(0.3),
    )
    .expect("set close domain");
    let summary = project_summary(yaml).expect("summary");
    assert_eq!(summary.domain_shape, "close");
    assert_eq!(summary.close_format, Some("nml".to_string()));
    assert_eq!(
        summary.watershed_path,
        Some("input/Ocean/Ocean_ChinaSea_boundary.nml".to_string())
    );
    assert_eq!(summary.sea_ratio, Some(0.3));
}

#[test]
fn close_domain_masks_resolve_repo_relative_input_paths() {
    let root = env::temp_dir().join(format!("earthmesh_close_domain_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir temp");

    let (domain_prefix, refine_prefix) = mesh_runner::write_close_domain_masks(
        "input/Ocean/Ocean_ChinaSea_boundary.nml",
        CloseMaskFormat::Nml,
        &root,
        2,
        48,
    )
    .expect("write close masks")
    .expect("nml close should write masks");

    assert!(root.join("domain_close_001.nml").is_file());
    assert!(root.join("refine_close_001_001.nml").is_file());
    assert_eq!(domain_prefix, root.join("domain_close"));
    assert_eq!(refine_prefix, root.join("refine_close"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_close_domain_only_ignores_stale_refine_passes_when_refine_disabled() {
    let root = env::temp_dir().join(format!("earthmesh_close_domain_only_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("mkdir temp");

    let mut cfg = ProjectConfig::from_yaml(&preset_yaml(
        "close_no_refine",
        MeshIntentPreset::CoastalOcean,
    ))
    .expect("project");
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "input/Ocean/Ocean_ChinaSea_boundary.nml".to_string(),
            format: CloseMaskFormat::Nml,
        },
        sea_ratio: None,
    };
    cfg.refinement.enabled = false;
    cfg.refinement.max_passes = 3;
    let mut lowered = cfg.try_lower().expect("lower");
    let domain_prefix = mesh_runner::write_close_domain_only_masks(
        "input/Ocean/Ocean_ChinaSea_boundary.nml",
        CloseMaskFormat::Nml,
        &root,
    )
    .expect("write close domain mask")
    .expect("nml close should write masks");
    mesh_runner::configure_regional_close_domain_only(&mut lowered, &domain_prefix);

    assert!(!lowered.mkgrd.refine);
    assert!(!lowered.refine.refine_spc);
    assert!(!lowered.refine.refine_cal);
    assert_eq!(lowered.refine.max_iter_spc, 0);
    assert_eq!(lowered.mkgrd.mask_domain_type, "close");
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        root.join("domain_close").to_string_lossy()
    );
    let domain = fs::read_to_string(root.join("domain_close_001.nml")).expect("domain mask");
    assert!(domain.contains("close_refine = 0"));
    assert!(!root.join("refine_close_001_001.nml").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shapefile_polygon_converts_to_close_mask_nml() {
    let root = env::temp_dir().join(format!("earthmesh_studio_shp_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let shp = root.join("watershed.shp");
    write_test_polygon_shp(&shp, &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);

    let (domain_prefix, refine_prefix) =
        mesh_runner::write_shapefile_close_masks(&shp, &root, 2, 30).expect("convert shp");
    assert_eq!(domain_prefix, root.join("domain_shp"));
    assert_eq!(refine_prefix, root.join("refine_shp"));
    let nml = fs::read_to_string(root.join("domain_shp_001.nml")).expect("read close nml");
    assert!(nml.contains("close_num = 4"));
    assert!(nml.contains("close_refine = 2"));
    assert!(nml.contains("4.0000000000 4.0000000000"));
    let refine_nml = fs::read_to_string(root.join("refine_shp_001_001.nml")).expect("refine 1");
    assert!(refine_nml.contains("close_refine = 1"));
    let refine_nml = fs::read_to_string(root.join("refine_shp_002_001.nml")).expect("refine 2");
    assert!(refine_nml.contains("close_refine = 2"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn lonlat_text_converts_to_close_domain_masks() {
    let root = env::temp_dir().join(format!("earthmesh_studio_close_txt_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let txt = root.join("ring.txt");
    fs::write(&txt, "0 0\n4 0\n4 4\n0 4\n").expect("write txt");

    let (domain_prefix, refine_prefix) = mesh_runner::write_close_domain_masks(
        txt.to_str().unwrap(),
        CloseMaskFormat::LonLatText,
        &root,
        2,
        30,
    )
    .expect("convert txt")
    .expect("fast path");
    assert_eq!(domain_prefix, root.join("domain_close"));
    assert_eq!(refine_prefix, root.join("refine_close"));
    let nml = fs::read_to_string(root.join("domain_close_001.nml")).expect("read close nml");
    assert!(nml.contains("close_refine = 2"));
    assert!(nml.contains("4.0000000000 4.0000000000"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shapefile_close_mask_simplifies_dense_boundary() {
    let root = env::temp_dir().join(format!("earthmesh_studio_shp_dense_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let shp = root.join("watershed.shp");
    let mut ring = Vec::new();
    for i in 0..=1000 {
        ring.push((i as f64 / 250.0, 0.0));
    }
    for i in 1..=1000 {
        ring.push((4.0, i as f64 / 250.0));
    }
    for i in (0..1000).rev() {
        ring.push((i as f64 / 250.0, 4.0));
    }
    for i in (1..1000).rev() {
        ring.push((0.0, i as f64 / 250.0));
    }
    write_test_polygon_shp(&shp, &ring);

    mesh_runner::write_shapefile_close_masks(&shp, &root, 3, 50).expect("convert dense shp");
    let nml = fs::read_to_string(root.join("domain_shp_001.nml")).expect("read close nml");
    let close_num = nml
        .lines()
        .find_map(|line| line.strip_prefix("close_num = "))
        .and_then(|value| value.parse::<usize>().ok())
        .expect("close_num");
    assert!(
        close_num < 20,
        "dense rectangle should simplify before engine input, got {close_num}"
    );
    assert!(nml.contains("close_refine = 3"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn engine_input_path_resolves_project_relative_file() {
    let root = env::temp_dir().join(format!("earthmesh_studio_run_path_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    let input = root.join("input");
    fs::create_dir_all(&input).expect("input dir");
    fs::write(input.join("landtype_igbp_update.nc"), b"landcover").expect("write landcover");

    let resolved = mesh_runner::existing_run_file("input/landtype_igbp_update.nc", &root)
        .expect("relative input should resolve under project dir");
    assert_eq!(
        resolved,
        input
            .join("landtype_igbp_update.nc")
            .canonicalize()
            .expect("canonical input")
            .to_string_lossy()
            .into_owned()
    );
    assert!(mesh_runner::existing_run_file("input/missing.nc", &root).is_none());
    assert!(mesh_runner::existing_run_file("none", &root).is_none());
    let cargo_toml =
        mesh_runner::existing_run_file("Cargo.toml", &root).expect("repo-root fallback");
    assert_eq!(
        Path::new(&cargo_toml)
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str()),
        Some("EarthMesh")
    );

    let cfg = ProjectConfig::from_yaml(&hydrology_yaml("path_normalize")).expect("project");
    let mut lowered = cfg.try_lower().expect("lower");
    mesh_runner::normalize_engine_input_paths(&mut lowered, &root);
    assert_eq!(
        lowered.mkgrd.landtype_file,
        input
            .join("landtype_igbp_update.nc")
            .canonicalize()
            .expect("canonical landtype")
            .to_string_lossy()
            .into_owned()
    );
    assert_eq!(
        lowered.data_layers.layers[0].path,
        lowered.mkgrd.landtype_file
    );
    assert_eq!(lowered.mkgrd.gridnum_perdegree, 240);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_method_c_plan_keeps_local_target_resolution() {
    assert_eq!(mesh_runner::regional_method_c_plan(40), (20, 1));
    assert_eq!(mesh_runner::regional_method_c_plan(120), (30, 2));
    assert_eq!(mesh_runner::regional_method_c_plan(360), (45, 3));
}

#[test]
fn regional_method_c_project_plan_uses_user_refine_passes() {
    let mut cfg = ProjectConfig::from_yaml(&hydrology_yaml("regional_refine")).expect("project");
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 4;
    cfg.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 113.0,
        lat: 22.0,
        radius_km: 100.0,
    });

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(320, &cfg),
        (20, 4)
    );
}

#[test]
fn regional_method_c_project_plan_caps_user_passes_to_supported_target() {
    let mut cfg =
        ProjectConfig::from_yaml(&hydrology_yaml("regional_refine_cap")).expect("project");
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 9;
    cfg.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 113.0,
        lat: 22.0,
        radius_km: 100.0,
    });

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(40, &cfg),
        (10, 2)
    );
    assert_eq!(
        mesh_runner::regional_method_c_project_plan(768, &cfg),
        (24, 5)
    );
}

#[test]
fn regional_method_c_project_plan_ignores_disabled_refine_passes() {
    let mut cfg = ProjectConfig::from_yaml(&hydrology_yaml("regional_auto_plan")).expect("project");
    cfg.refinement.enabled = false;
    cfg.refinement.max_passes = 1;

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(320, &cfg),
        (40, 3)
    );
}

#[test]
fn regional_method_c_project_plan_uses_expert_spc_passes_without_recipe_refine() {
    let mut cfg =
        ProjectConfig::from_yaml(&hydrology_yaml("regional_expert_refine")).expect("project");
    cfg.refinement.enabled = false;
    cfg.refinement.max_passes = 0;
    cfg.expert.max_iter_spc = Some(2);

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(40, &cfg),
        (10, 2)
    );
}

#[test]
fn regional_method_c_project_plan_preserves_target_nxp_with_requested_level() {
    let mut cfg =
        ProjectConfig::from_yaml(&hydrology_yaml("regional_target_nxp")).expect("project");
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 2;
    cfg.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 114.0,
        lat: 22.0,
        radius_km: 400.0,
    });

    let (base_nxp, level) = mesh_runner::regional_method_c_project_plan(40, &cfg);
    assert_eq!((base_nxp, level), (10, 2));
    assert_eq!(base_nxp * (1_i32 << level), 40);
}

#[test]
fn regional_bbox_method_c_plan_uses_requested_nxp_as_base_resolution() {
    let mut cfg =
        ProjectConfig::from_yaml(&hydrology_yaml("regional_bbox_base_nxp")).expect("project");
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 2;
    cfg.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 114.0,
        lat: 22.0,
        radius_km: 400.0,
    });

    let (base_nxp, level) = mesh_runner::regional_bbox_method_c_project_plan(40, &cfg);
    assert_eq!((base_nxp, level), (40, 2));
    assert_eq!(base_nxp * (1_i32 << level), 160);
}

#[test]
fn regional_method_c_project_plan_ignores_stale_passes_without_refine_source() {
    let mut cfg = ProjectConfig::from_yaml(&preset_yaml(
        "regional_close",
        MeshIntentPreset::CoastalOcean,
    ))
    .expect("project");
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 3;

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(40, &cfg),
        (20, 1)
    );
}

#[test]
fn ocean_close_method_c_defaults_to_smooth_two_level_plan() {
    let mut cfg = ProjectConfig::from_yaml(&preset_yaml(
        "smooth_ocean_close",
        MeshIntentPreset::CoastalOcean,
    ))
    .expect("project");
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "input/Ocean/Ocean_ChinaSea_boundary.nml".to_string(),
            format: CloseMaskFormat::Nml,
        },
        sea_ratio: None,
    };
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 3;

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(400, &cfg),
        (100, 2)
    );
}

#[test]
fn ocean_close_method_c_respects_explicit_refinement_source() {
    let mut cfg = ProjectConfig::from_yaml(&preset_yaml(
        "explicit_ocean_close_refine",
        MeshIntentPreset::CoastalOcean,
    ))
    .expect("project");
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "input/Ocean/Ocean_ChinaSea_boundary.nml".to_string(),
            format: CloseMaskFormat::Nml,
        },
        sea_ratio: None,
    };
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 3;
    cfg.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 113.0,
        lat: 22.0,
        radius_km: 100.0,
    });

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(400, &cfg),
        (50, 3)
    );
}

#[test]
fn v2_ocean_o1_o2_o3_fixtures_pin_smooth_method_c_defaults() {
    let ocean_close = |name: &str, nxp| {
        let mut cfg = ProjectConfig::from_yaml(&preset_yaml(name, MeshIntentPreset::CoastalOcean))
            .expect("project");
        cfg.target.resolution = ResolutionSpec::Nxp(nxp);
        cfg.domain = DomainConfig::Regional {
            shape: RegionShape::Close {
                path: "input/Ocean/Ocean_ChinaSea_boundary.nml".to_string(),
                format: CloseMaskFormat::Nml,
            },
            sea_ratio: None,
        };
        cfg.refinement.enabled = true;
        cfg.refinement.max_passes = 3;
        cfg
    };

    let o1 = ocean_close("v2_o1_chinasea_lr", 192);
    assert_eq!(
        mesh_runner::regional_method_c_project_plan(192, &o1),
        (48, 2)
    );

    let o2 = ocean_close("v2_o2_chinasea_hr", 768);
    let (base_nxp, level) = mesh_runner::regional_method_c_project_plan(768, &o2);
    assert_eq!((base_nxp, level), (192, 2));

    let mut lowered = o2.try_lower().expect("lower");
    mesh_runner::enable_regional_method_c_fast_path(
        &mut lowered,
        "close",
        Path::new("/tmp/domain_close"),
        Path::new("/tmp/refine_close"),
        base_nxp,
        level,
    );
    assert_eq!(lowered.refine.max_iter_spc, 2);
    assert_eq!(lowered.refine.halo[1], 3);
    assert_eq!(lowered.refine.halo[2], 3);
    assert_eq!(lowered.refine.max_transition_row[1], 3);
    assert_eq!(lowered.refine.max_transition_row[2], 3);
    assert_eq!(lowered.refine.spring_global_type, 0);
    assert_eq!(lowered.refine.spring_regional_type, 1);
    assert_eq!(lowered.refine.niter_refine, 2000);

    let mut o3 = ocean_close("v2_o3_chinasea_vr", 192);
    o3.refinement.max_passes = 2;
    o3.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "input/Ocean/refine_spc_close01.nml".to_string(),
    });
    assert_eq!(
        mesh_runner::regional_method_c_project_plan(192, &o3),
        (48, 2)
    );
}

#[test]
fn regional_method_c_project_plan_auto_refine_does_not_double_count_gui_loop() {
    let mut cfg =
        ProjectConfig::from_yaml(&hydrology_yaml("regional_auto_refine")).expect("project");
    cfg.refinement.enabled = true;
    cfg.refinement.max_passes = 2;
    cfg.quality.on_violation = ViolationPolicy::AutoRefine;

    assert_eq!(
        mesh_runner::regional_method_c_project_plan(320, &cfg),
        (40, 3)
    );
}

#[test]
fn regional_bbox_circle_mask_family_writes_domain_and_local_refine_masks() {
    let root = env::temp_dir().join(format!("earthmesh_bbox_circle_family_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let (lon, lat, radius_km) = (114.0_f64, 22.0_f64, 400.32_f64);
    assert_eq!(lon, 114.0);
    assert_eq!(lat, 22.0);
    assert!((radius_km - 400.32).abs() < 0.001);

    let (domain_prefix, refine_prefix) = mesh_runner::write_regional_bbox_circle_mask_family(
        108.0,
        120.0,
        26.0,
        18.0,
        lon,
        lat,
        radius_km,
        &root,
        "domain_bbox",
        "refine_circle",
        2,
    )
    .expect("write bbox/circle mask family");

    assert_eq!(domain_prefix, root.join("domain_bbox"));
    assert_eq!(refine_prefix, root.join("refine_circle"));
    let domain = fs::read_to_string(root.join("domain_bbox_001.nml")).expect("domain mask");
    let refine = fs::read_to_string(root.join("refine_circle_001.nml")).expect("refine mask");
    assert!(domain.contains("bbox_refine = 0"));
    assert!(refine.contains("circle_refine = 2"));
    assert!(domain.contains("108.0000000000 120.0000000000 26.0000000000 18.0000000000"));
    assert!(refine.contains("114.0000000000 22.0000000000 400.3200000000"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_bbox_inset_mask_family_writes_rectangular_default_refine_mask() {
    let root = env::temp_dir().join(format!("earthmesh_bbox_inset_family_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let (rw, re, rn, rs) = mesh_runner::default_regional_bbox_refine_bbox(80.0, 140.0, 26.0, -26.0);
    assert_eq!((rw, re, rn, rs), (85.0, 135.0, 21.0, -21.0));

    let (domain_prefix, refine_prefix) = mesh_runner::write_regional_bbox_inset_mask_family(
        80.0,
        140.0,
        26.0,
        -26.0,
        rw,
        re,
        rn,
        rs,
        &root,
        "domain_bbox",
        "refine_bbox",
        3,
    )
    .expect("write bbox/inset mask family");

    assert_eq!(domain_prefix, root.join("domain_bbox"));
    assert_eq!(refine_prefix, root.join("refine_bbox"));
    let domain = fs::read_to_string(root.join("domain_bbox_001.nml")).expect("domain mask");
    let refine = fs::read_to_string(root.join("refine_bbox_001.nml")).expect("refine mask");
    assert!(domain.contains("bbox_refine = 0"));
    assert!(refine.contains("bbox_refine = 3"));
    assert!(refine.contains("85.0000000000 135.0000000000 21.0000000000 -21.0000000000"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_bbox_domain_only_ignores_stale_refine_passes_when_refine_disabled() {
    let root = env::temp_dir().join(format!("earthmesh_bbox_domain_only_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let mut cfg =
        ProjectConfig::from_yaml(&hydrology_yaml("regional_bbox_no_refine")).expect("project");
    cfg.refinement.enabled = false;
    cfg.refinement.max_passes = 3;
    let mut lowered = cfg.try_lower().expect("lower");
    mesh_runner::configure_regional_bbox_domain_only(
        &mut lowered,
        80.0,
        140.0,
        26.0,
        -26.0,
        &root,
        "domain_bbox",
    )
    .expect("write domain-only bbox");

    assert!(!lowered.mkgrd.refine);
    assert!(!lowered.refine.refine_spc);
    assert!(!lowered.refine.refine_cal);
    assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        root.join("domain_bbox").to_string_lossy()
    );
    let domain = fs::read_to_string(root.join("domain_bbox_001.nml")).expect("domain mask");
    assert!(domain.contains("bbox_refine = 0"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regional_method_c_fast_path_sets_domain_and_refine_mask() {
    let cfg = ProjectConfig::from_yaml(&hydrology_yaml("fast_region")).expect("project");
    let mut lowered = cfg.try_lower().expect("lower");
    mesh_runner::enable_regional_method_c_fast_path(
        &mut lowered,
        "bbox",
        Path::new("/tmp/domain_bbox"),
        Path::new("/tmp/domain_bbox"),
        30,
        2,
    );
    assert_eq!(lowered.mkgrd.nxp, 30);
    assert!(lowered.mkgrd.refine);
    assert!(!lowered.mkgrd.mask_domain_global);
    assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
    assert_eq!(lowered.mkgrd.mask_domain_fprefix, "/tmp/domain_bbox");
    assert!(lowered.refine.refine_spc);
    assert!(!lowered.refine.refine_cal);
    assert_eq!(lowered.refine.max_iter_spc, 2);
    assert_eq!(lowered.refine.halo[1], 4);
    assert_eq!(lowered.refine.halo[2], 4);
    assert_eq!(lowered.refine.max_transition_row[1], 4);
    assert_eq!(lowered.refine.max_transition_row[2], 4);
    assert_eq!(lowered.refine.mask_refine_spc_type, "bbox");
    assert_eq!(lowered.refine.mask_refine_spc_fprefix, "/tmp/domain_bbox");
    assert!(lowered.refine.weak_concav_eliminate);
    assert_eq!(lowered.refine.spring_global_type, 0);
    assert_eq!(lowered.refine.spring_regional_type, 1);
    assert_eq!(lowered.refine.niter_refine, 2000);
    assert!(lowered.refine.niter_refine_specified);
}

#[test]
fn regional_method_c_fast_path_can_use_separate_refine_mask_type() {
    let cfg = ProjectConfig::from_yaml(&hydrology_yaml("fast_region_circle")).expect("project");
    let mut lowered = cfg.try_lower().expect("lower");
    mesh_runner::enable_regional_method_c_fast_path_with_refine_type(
        &mut lowered,
        "bbox",
        "circle",
        Path::new("/tmp/domain_bbox"),
        Path::new("/tmp/refine_circle"),
        20,
        2,
    );
    assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
    assert_eq!(lowered.mkgrd.mask_domain_fprefix, "/tmp/domain_bbox");
    assert_eq!(lowered.refine.mask_refine_spc_type, "circle");
    assert_eq!(lowered.refine.mask_refine_spc_fprefix, "/tmp/refine_circle");
}

#[test]
fn regional_method_c_fast_path_uses_regional_spring_for_tri_meshes() {
    let cfg = ProjectConfig::from_yaml(&preset_yaml("fast_ocean", MeshIntentPreset::CoastalOcean))
        .expect("project");
    let mut lowered = cfg.try_lower().expect("lower");
    mesh_runner::enable_regional_method_c_fast_path(
        &mut lowered,
        "close",
        Path::new("/tmp/domain_close"),
        Path::new("/tmp/refine_close"),
        20,
        1,
    );
    assert_eq!(lowered.mkgrd.mode_grid, "tri");
    assert!(lowered.refine.weak_concav_eliminate);
    assert_eq!(lowered.refine.halo[1], 3);
    assert_eq!(lowered.refine.max_transition_row[1], 3);
    assert_eq!(lowered.refine.niter_refine, 2000);
    assert!(lowered.refine.niter_refine_specified);
    assert_eq!(lowered.refine.spring_global_type, 0);
    assert_eq!(lowered.refine.spring_regional_type, 1);
}

#[test]
fn shapefile_boundary_geojson_returns_polygon_outline() {
    let root = env::temp_dir().join(format!("earthmesh_studio_shp_geojson_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let shp = root.join("watershed.shp");
    write_test_polygon_shp(&shp, &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);

    let geojson =
        mesh_runner::shapefile_boundary_geojson(shp.to_string_lossy().into_owned()).unwrap();
    assert_eq!(geojson["type"], "FeatureCollection");
    let features = geojson["features"].as_array().expect("features");
    assert_eq!(features.len(), 1);
    assert_eq!(features[0]["geometry"]["type"], "Polygon");
    assert_eq!(
        features[0]["geometry"]["coordinates"][0]
            .as_array()
            .expect("outer ring")
            .len(),
        5
    );

    let _ = fs::remove_dir_all(&root);
}

fn write_test_polygon_shp(path: &Path, ring: &[(f64, f64)]) {
    let mut points = ring.to_vec();
    points.push(ring[0]);
    let content_bytes = 44 + 4 + points.len() * 16;
    let file_bytes = 100 + 8 + content_bytes;
    let mut out = Vec::with_capacity(file_bytes);

    out.extend(9994_i32.to_be_bytes());
    out.extend([0_u8; 20]);
    out.extend(((file_bytes / 2) as i32).to_be_bytes());
    out.extend(1000_i32.to_le_bytes());
    out.extend(5_i32.to_le_bytes());
    for value in [0.0_f64, 0.0, 4.0, 4.0, 0.0, 0.0, 0.0, 0.0] {
        out.extend(value.to_le_bytes());
    }

    out.extend(1_i32.to_be_bytes());
    out.extend(((content_bytes / 2) as i32).to_be_bytes());
    out.extend(5_i32.to_le_bytes());
    for value in [0.0_f64, 0.0, 4.0, 4.0] {
        out.extend(value.to_le_bytes());
    }
    out.extend(1_i32.to_le_bytes());
    out.extend((points.len() as i32).to_le_bytes());
    out.extend(0_i32.to_le_bytes());
    for (x, y) in points {
        out.extend(x.to_le_bytes());
        out.extend(y.to_le_bytes());
    }
    fs::write(path, out).expect("write test shp");
}
#[test]
fn scaffold_project_rejects_invalid_approx_km() {
    let err = scaffold_project(
        "bad_km".to_string(),
        "HydrologyLand".to_string(),
        None,
        Some(0.0),
        None,
    )
    .unwrap_err();
    assert!(err.contains("target resolution ApproxKm must be > 0"));
    let err = scaffold_project(
        "bad_nxp".to_string(),
        "HydrologyLand".to_string(),
        Some(0),
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("target resolution Nxp must be > 0"));
    let err = scaffold_project(
        "bad_intent".to_string(),
        "TypoHydrologyLand".to_string(),
        Some(40),
        None,
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
fn set_target_cell_updates_project_cell_shape() {
    let yaml = hydrology_yaml("cell_shape");
    let yaml = set_target_cell(yaml, "tri".to_string()).expect("set cell");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert_eq!(summary.cell, "tri");
    assert_eq!(summary.quality_mode, "tri-strict");
    let yaml = set_target_cell(yaml, "hex".to_string()).expect("set hex cell");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert_eq!(summary.cell, "hex");
    assert_eq!(summary.quality_mode, "hex-cgrid");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("yaml");
    assert_eq!(cfg.target.cell, earthmesh_project::MeshCellKind::Hex);
    assert!(set_target_cell(yaml, "square".to_string()).is_err());
}

#[test]
fn set_expert_updates_custom_overrides() {
    let yaml = hydrology_yaml("expert");
    let yaml = set_expert(
        yaml,
        Some(80),
        Some(4),
        Some(200),
        Some(120),
        Some(2),
        Some(3),
        Some(vec![4, 4, 3]),
        Some(vec![5, 4, 3]),
        Some("linear".to_string()),
        Some(1),
        Some(2),
        Some(0),
        Some(1),
        Some(1.1),
        Some(0.03),
        Some(true),
    )
    .expect("set expert");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert_eq!(summary.expert_nxp, Some(80));
    assert_eq!(summary.expert_openmp, Some(4));
    assert_eq!(summary.expert_niter, Some(200));
    assert_eq!(summary.expert_niter_refine, Some(120));
    assert_eq!(summary.expert_max_iter_spc, Some(2));
    assert_eq!(summary.expert_max_iter_cal, Some(3));
    assert_eq!(summary.expert_halo, Some(vec![4, 4, 3]));
    assert_eq!(summary.expert_max_transition_row, Some(vec![5, 4, 3]));
    assert_eq!(summary.expert_set_dis_type, Some("linear".to_string()));
    assert_eq!(summary.expert_num_rc, Some(1));
    assert_eq!(summary.expert_vertex_pretect_layers, Some(2));
    assert_eq!(summary.expert_spring_global_type, Some(0));
    assert_eq!(summary.expert_spring_regional_type, Some(1));
    assert_eq!(summary.expert_beta, Some(1.1));
    assert_eq!(summary.expert_relax, Some(0.03));
    assert_eq!(summary.expert_weak_concav_eliminate, Some(true));
    assert!(set_expert(
        yaml,
        None,
        None,
        Some(0),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None
    )
    .is_err());
}

#[test]
fn set_specified_refinement_updates_project() {
    let yaml = hydrology_yaml("specified_refine");
    let yaml = set_specified_refinement(
        yaml,
        true,
        Some("radius".to_string()),
        Some(113.5),
        Some(22.0),
        Some(80.0),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("set specified refinement");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(summary.specified_refine_enabled);
    assert_eq!(summary.specified_refine_kind, "radius");
    assert_eq!(summary.specified_refine_lon, Some(113.5));
    assert_eq!(summary.specified_refine_lat, Some(22.0));
    assert_eq!(summary.specified_refine_radius_km, Some(80.0));
    assert!(set_specified_refinement(
        yaml,
        true,
        Some("radius".to_string()),
        Some(181.0),
        Some(22.0),
        Some(80.0),
        None,
        None,
        None,
        None,
        None
    )
    .is_err());
}

#[test]
fn set_specified_refinement_accepts_bbox_region() {
    let yaml = hydrology_yaml("specified_refine_bbox");
    let yaml = set_specified_refinement(
        yaml,
        true,
        Some("bbox".to_string()),
        None,
        None,
        None,
        Some(112.0),
        Some(115.0),
        Some(21.0),
        Some(24.0),
        None,
    )
    .expect("set specified bbox refinement");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(summary.specified_refine_enabled);
    assert_eq!(summary.specified_refine_kind, "bbox");
    assert_eq!(
        summary.specified_refine_bbox,
        Some([112.0, 115.0, 21.0, 24.0])
    );
    assert!(set_specified_refinement(
        yaml,
        true,
        Some("bbox".to_string()),
        None,
        None,
        None,
        Some(115.0),
        Some(112.0),
        Some(21.0),
        Some(24.0),
        None,
    )
    .is_err());
}

#[test]
fn hfield_defaults_on_and_legacy_can_disable_it() {
    let yaml = hydrology_yaml("hfield_default");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(summary.hfield_enabled);

    let yaml = set_hfield_refinement(yaml, false, None, None, None).expect("legacy hfield off");
    let summary = project_summary(yaml.clone()).expect("summary");
    assert!(!summary.hfield_enabled);
    let lowered = ProjectConfig::from_yaml(&yaml).expect("yaml").lower();
    assert!(!lowered.to_namelist().contains("&hfield"));
}

#[test]
fn set_specified_refinement_accepts_close_shapefile() {
    let root = env::temp_dir().join(format!(
        "earthmesh_studio_specified_close_{}",
        process::id()
    ));
    fs::create_dir_all(&root).expect("test dir");
    let shp = root.join("region.shp");
    write_test_polygon_shp(&shp, &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);

    let yaml = set_specified_refinement(
        hydrology_yaml("specified_refine_close"),
        true,
        Some("close".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(shp.to_string_lossy().into_owned()),
    )
    .expect("set specified close refinement");
    let summary = project_summary(yaml).expect("summary");
    assert!(summary.specified_refine_enabled);
    assert_eq!(summary.specified_refine_kind, "close");
    assert_eq!(
        summary.specified_refine_path,
        Some(shp.to_string_lossy().into_owned())
    );
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
        threshold_value: None,
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
fn autofill_data_layers_from_folder_matches_v2_source_data_names() {
    let root = env::temp_dir().join(format!("earthmesh_studio_source_data_{}", process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("temp root");
    fs::write(root.join("landtype_usgs_update.nc"), b"land").expect("landtype");
    fs::write(root.join("LAI_BNU_161.nc"), b"lai").expect("lai");
    fs::write(root.join("k_s.nc"), b"ks").expect("ks");
    fs::write(root.join("k_solids.nc"), b"k_solids").expect("k_solids");
    fs::write(root.join("tkdry.nc"), b"tkdry").expect("tkdry");
    fs::write(root.join("tksatf.nc"), b"tksatf").expect("tksatf");
    fs::write(root.join("tksatu.nc"), b"tksatu").expect("tksatu");
    fs::write(root.join("slope_avg.nc"), b"slope").expect("slope");
    fs::write(root.join("dem.nc"), b"dem").expect("dem");
    fs::write(root.join("slope_max.nc"), b"slope_max").expect("slope_max");

    let yaml = preset_yaml("source_data", MeshIntentPreset::CarbonLand);
    let yaml = autofill_data_layers_from_folder(yaml, root.to_string_lossy().into_owned())
        .expect("autofill source_data");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("parse yaml");

    let land = cfg
        .data_layers
        .iter()
        .find(|layer| matches!(layer.role, ProjectLayerRole::LandType))
        .expect("land layer");
    assert!(land.enabled);
    assert!(land.path.ends_with("landtype_usgs_update.nc"));

    let lai = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "lai")
        .expect("lai layer");
    assert!(lai.enabled);
    assert!(lai.path.ends_with("LAI_BNU_161.nc"));

    let ks = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "k_s")
        .expect("k_s layer");
    assert!(ks.enabled);
    assert!(ks.path.ends_with("k_s.nc"));
    for id in [
        "k_solids",
        "tkdry",
        "tksatf",
        "tksatu",
        "slope_avg",
        "dem",
        "slope_max",
    ] {
        let layer = cfg
            .data_layers
            .iter()
            .find(|layer| layer.id == id)
            .unwrap_or_else(|| panic!("{id} layer"));
        assert!(layer.enabled, "{id} should be enabled");
        assert!(
            layer.path.ends_with(&format!("{id}.nc")),
            "{id}: {}",
            layer.path
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn set_threshold_value_updates_threshold_layer() {
    let yaml = hydrology_yaml("threshold_value");
    let yaml =
        set_threshold_value(yaml, "slope_avg".to_string(), Some(7.5)).expect("set threshold");
    let cfg = ProjectConfig::from_yaml(&yaml).expect("parse yaml");
    let slope = cfg
        .data_layers
        .iter()
        .find(|layer| layer.id == "slope_avg")
        .expect("slope layer");
    assert_eq!(slope.threshold_value, Some(7.5));
    let err = set_threshold_value(yaml, "landcover".to_string(), Some(1.0)).unwrap_err();
    assert!(err.contains("is not a threshold layer"));
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
        .find(|layer| layer.id == "slope_avg")
        .expect("threshold layer");
    layer.path = src.to_string_lossy().into_owned();
    layer.enabled = true;
    let threshold_dir = root.join("threshold");
    assert!(engine::stage_threshold_layers(&cfg, &threshold_dir, &root).expect("stage"));
    assert_eq!(
        fs::read(threshold_dir.join("slope_avg.nc")).expect("read staged"),
        b"slope"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn stage_threshold_layers_resolves_project_relative_sources() {
    let root = env::temp_dir().join(format!(
        "earthmesh_studio_threshold_relative_{}",
        process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let input = root.join("input");
    fs::create_dir_all(&input).expect("input dir");
    fs::write(input.join("terrain_slope_source.nc"), b"slope").expect("write source");
    let mut cfg = ProjectConfig::scaffold(
        "threshold_stage",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(40),
    );
    let layer = cfg
        .data_layers
        .iter_mut()
        .find(|layer| layer.id == "slope_avg")
        .expect("threshold layer");
    layer.path = "input/terrain_slope_source.nc".to_string();
    layer.enabled = true;
    let threshold_dir = root.join("threshold");
    assert!(engine::stage_threshold_layers(&cfg, &threshold_dir, &root).expect("stage"));
    assert_eq!(
        fs::read(threshold_dir.join("slope_avg.nc")).expect("read staged"),
        b"slope"
    );
    let _ = fs::remove_dir_all(&root);
}
#[test]
fn set_quality_rejects_invalid_min_angle() {
    let yaml = hydrology_yaml("quality_test");
    let err = set_quality(yaml, 0.0, "warn".to_string()).unwrap_err();
    assert!(err.contains("quality min_angle_deg must be > 0"));
}

#[test]
fn set_quality_accepts_auto_refine_policy() {
    let yaml = hydrology_yaml("quality_auto_refine_test");
    let yaml = set_quality(yaml, 25.0, "auto_refine".to_string()).unwrap();
    let summary = project_summary(yaml).unwrap();

    assert_eq!(summary.on_violation, "auto_refine");
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
fn mesh_merit_cells_rejects_invalid_bbox_before_io() {
    let err = mesh_outputs::mesh_merit_cells(
        "/no/grid.nc".to_string(),
        "hex".to_string(),
        "/no/merit".to_string(),
        120.0,
        108.0,
        18.0,
        26.0,
        Some(1),
        None,
    )
    .unwrap_err();
    assert!(err.contains("invalid MERIT-Hydro mesh bbox"));
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
    assert_eq!(slope.default_value, 5.0);
    assert_eq!(slope.range_min, 0.0);
    assert_eq!(slope.range_max, 45.0);
    assert_eq!(slope.physical_process, "orographic / runoff routing");
    let dem = criteria.iter().find(|c| c.stem == "dem").expect("dem");
    assert_eq!(dem.label, "DEM");
    assert_eq!(dem.unit, "m");
    assert_eq!(dem.default_value, 500.0);
}
