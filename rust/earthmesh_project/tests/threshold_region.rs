use earthmesh_project::{
    CloseBoundaryMode, CloseMaskFormat, DomainConfig, MeshDomainKind, MeshIntentPreset,
    ProjectConfig, ProjectDataLayer, ProjectLayerRole, RefinementBackend, RegionShape,
    ResolutionSpec, ThresholdField,
};

fn project() -> ProjectConfig {
    let mut cfg = ProjectConfig::scaffold(
        "threshold_region",
        MeshIntentPreset::MultiObjectiveBalanced,
        DomainConfig::Global,
        ResolutionSpec::Nxp(12),
    );
    cfg.target.kind = MeshDomainKind::Earth;
    cfg.data_layers = vec![ProjectDataLayer {
        id: "lai".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::Lai),
        path: "data/lai.nc".into(),
        enabled: true,
        threshold_value: Some(1.0),
    }];
    cfg.refinement.enabled = true;
    cfg.refinement.threshold_enabled = true;
    cfg.refinement.max_passes = 2;
    cfg.refinement.backend = RefinementBackend::Certified;
    cfg
}

fn with_region(cfg: &ProjectConfig, region: &RegionShape) -> Result<ProjectConfig, String> {
    let mut json = serde_json::to_value(cfg).unwrap();
    json["refinement"]["threshold_region"] = serde_json::to_value(region).unwrap();
    ProjectConfig::from_json(&json.to_string())
}

#[test]
fn threshold_region_is_optional_and_independent_of_delivery_and_hard_refinement() {
    let mut cfg = project();
    cfg.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 100.0,
            e: 120.0,
            s: 10.0,
            n: 30.0,
        },
        sea_ratio: None,
    };
    let baseline = cfg.try_lower().unwrap();
    assert_eq!(baseline.refine.mask_refine_cal_fprefix, "/tmp");
    assert!(!baseline.refine.refine_spc);
    let shapes = [
        RegionShape::Bbox {
            w: 170.0,
            e: -170.0,
            s: -10.0,
            n: 10.0,
        },
        RegionShape::Circle {
            lon: 110.0,
            lat: 20.0,
            radius_km: 500.0,
        },
        RegionShape::Shapefile {
            path: "input/window.shp".into(),
        },
        RegionShape::Close {
            path: "input/window.nml".into(),
            format: CloseMaskFormat::Nml,
            boundary: CloseBoundaryMode::Polyline,
        },
    ];
    for shape in shapes {
        let scoped = with_region(&cfg, &shape).unwrap();
        assert_eq!(
            ProjectConfig::from_yaml(&scoped.to_yaml().unwrap()).unwrap(),
            scoped
        );
        let lowered = scoped.try_lower().unwrap();
        assert_eq!(lowered.mkgrd, baseline.mkgrd);
        assert!(!lowered.refine.refine_spc);
        assert_eq!(lowered.refine.max_iter_cal, baseline.refine.max_iter_cal);
        assert_ne!(lowered.refine.mask_refine_cal_fprefix, "/tmp");
        assert_eq!(
            lowered.refine.mask_refine_spc_fprefix,
            baseline.refine.mask_refine_spc_fprefix
        );
    }
    let mut json = serde_json::to_value(&cfg).unwrap();
    json["refinement"]["threshold_region"] = serde_json::Value::Null;
    assert_eq!(
        ProjectConfig::from_json(&json.to_string()).unwrap().lower(),
        baseline
    );
}

#[test]
fn threshold_region_geometry_is_validated_even_when_refinement_is_disabled() {
    let mut cfg = project();
    cfg.refinement.enabled = false;
    let invalid = RegionShape::Bbox {
        w: 100.0,
        e: 100.0,
        s: 10.0,
        n: 30.0,
    };
    let error = with_region(&cfg, &invalid).unwrap_err();
    assert!(
        error.contains("threshold_region") && error.contains("west and east"),
        "{error}"
    );
    let valid = RegionShape::Bbox {
        w: 100.0,
        e: 120.0,
        s: 10.0,
        n: 30.0,
    };
    let stored = with_region(&cfg, &valid).unwrap().lower();
    assert_eq!(stored.refine.mask_refine_cal_fprefix, "/tmp");
}

#[test]
fn threshold_region_requires_a_statistical_demand_consumer() {
    let shape = RegionShape::Circle {
        lon: 110.0,
        lat: 20.0,
        radius_km: 500.0,
    };
    let mut cfg = project();
    cfg.refinement.backend = RefinementBackend::MethodC;
    assert!(with_region(&cfg, &shape)
        .unwrap_err()
        .contains("threshold_region requires"));
    for backend in [RefinementBackend::RedGreen, RefinementBackend::MethodC] {
        cfg.refinement.backend = backend;
        cfg.refinement.method_c.algorithm = if backend == RefinementBackend::MethodC {
            earthmesh_project::MethodCAlgorithm::LeppDelaunay
        } else {
            earthmesh_project::MethodCAlgorithm::Canonical
        };
        for adaptive in [
            None,
            Some(earthmesh_project::AdaptiveRefinementRecipe::default()),
        ] {
            cfg.refinement.adaptive = adaptive;
            let scoped = with_region(&cfg, &shape).unwrap().try_lower().unwrap();
            assert!(scoped.adaptive.is_some());
            assert!(scoped.hfield.is_none());
            assert!(!scoped.refine.refine_spc);
        }
        cfg.refinement.adaptive = Some(earthmesh_project::AdaptiveRefinementRecipe {
            enabled: false,
            ..Default::default()
        });
        let error = with_region(&cfg, &shape).unwrap_err();
        assert!(
            error.contains("threshold_region requires") && error.contains("adaptive"),
            "{error}"
        );
    }
    cfg.refinement.adaptive = None;
    cfg.refinement.backend = RefinementBackend::MethodC;
    cfg.refinement.method_c.algorithm = earthmesh_project::MethodCAlgorithm::Canonical;
    cfg.refinement.hfield = Some(earthmesh_project::HfieldRefinementRecipe::default());
    let scoped = with_region(&cfg, &shape).unwrap().lower();
    assert!(scoped.hfield.is_some());
    assert!(scoped.adaptive.is_none());

    // Switching thresholds off preserves stored geometry, not an active mask.
    cfg.refinement.threshold_enabled = false;
    cfg.refinement.specified_bbox = Some(earthmesh_project::SpecifiedBboxRefinement {
        w: -20.0,
        e: 20.0,
        s: -20.0,
        n: 20.0,
    });
    let stored = with_region(&cfg, &shape).unwrap().lower();
    assert_eq!(stored.refine.mask_refine_cal_fprefix, "/tmp");
    assert!(stored.refine.refine_spc);

    cfg.refinement.threshold_enabled = true;
    cfg.data_layers.clear();
    assert!(with_region(&cfg, &shape)
        .unwrap_err()
        .contains("active statistical threshold"));
}

#[test]
fn threshold_region_rejects_unimplemented_boundary_transforms() {
    let shape = RegionShape::Close {
        path: "input/window.nml".into(),
        format: CloseMaskFormat::Nml,
        boundary: CloseBoundaryMode::EnclosingCap {
            margin_km: 0.0,
            max_radius_deg: 80.0,
            max_segment_angle_deg: 0.25,
        },
    };
    assert!(with_region(&project(), &shape)
        .unwrap_err()
        .contains("polyline close boundaries"));
}
