use super::*;

fn sample() -> ProjectConfig {
    ProjectConfig {
        schema_version: "3.0.0".into(),
        metadata: ProjectMetadata {
            name: "gba".into(),
            authors: vec!["SYSU".into()],
            description: "GBA coupled mesh".into(),
        },
        domain: DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 112.0,
                e: 115.5,
                n: 23.5,
                s: 21.5,
            },
            sea_ratio: Some(0.25),
        },
        target: MeshTargetConfig {
            kind: MeshDomainKind::Coupled,
            cell: MeshCellKind::Hex,
            intent: MeshIntentPreset::MeritHydroCoast,
            resolution: ResolutionSpec::Nxp(40),
            model_format: ModelFormat::CoLM,
        },
        data_layers: vec![
            ProjectDataLayer {
                id: "lc".into(),
                role: ProjectLayerRole::LandType,
                path: "./in/landtype.nc".into(),
                enabled: true,
                threshold_value: None,
            },
            ProjectDataLayer {
                id: "lai".into(),
                role: ProjectLayerRole::Threshold(ThresholdField::Lai),
                path: "./th/lai.nc".into(),
                enabled: true,
                threshold_value: None,
            },
        ],
        refinement: RefinementRecipe {
            backend: crate::RefinementBackend::default(),
            enabled: true,
            threshold_enabled: true,
            max_passes: 3,
            threshold_criteria: Vec::new(),
            method_c: Default::default(),
            harp_dv: Default::default(),
            certified: Default::default(),
            adaptive: None,
            specified_circle: None,
            specified_bbox: None,
            specified_close: None,
            hfield: None,
        },
        quality: QualityConfig {
            min_angle_deg: 28.0,
            auto_refine_batch_cells: DEFAULT_AUTO_REFINE_BATCH_CELLS,
            on_violation: ViolationPolicy::Block,
            lepp_post_quality: None,
        },
        expert: ExpertOverrides {
            nxp: None,
            openmp: Some(8),
            niter: None,
            niter_refine: Some(120),
            max_iter_spc: None,
            max_iter_cal: None,
            halo: Some(vec![4, 4, 3]),
            max_transition_row: Some(vec![5, 4, 3]),
            set_dis_type: Some("linear".into()),
            num_rc: Some(1),
            vertex_pretect_layers: Some(2),
            spring_global_type: Some(0),
            spring_regional_type: Some(1),
            beta: Some(1.1),
            relax: Some(0.03),
            weak_concav_eliminate: Some(true),
            isolated_ocean: None,
        },
        hydro_coast: None,
        coupling: None,
    }
}

fn yaml_err(p: &ProjectConfig) -> String {
    ProjectConfig::from_yaml(&p.to_yaml().expect("yaml")).unwrap_err()
}

fn json_err(p: &ProjectConfig) -> String {
    ProjectConfig::from_json(&p.to_json().expect("json")).unwrap_err()
}

fn yaml_round_trip(p: &ProjectConfig) -> ProjectConfig {
    ProjectConfig::from_yaml(&p.to_yaml().expect("yaml")).expect("from yaml")
}

#[test]
fn json_round_trips() {
    let p = sample();
    let s = p.to_json().expect("to json");
    let back = ProjectConfig::from_json(&s).expect("from json");
    assert_eq!(p, back);
}

#[test]
fn yaml_round_trips() {
    let p = sample();
    let s = p.to_yaml().expect("to yaml");
    let back = ProjectConfig::from_yaml(&s).expect("from yaml");
    assert_eq!(p, back);
}

#[test]
fn quality_policy_omission_matches_the_whole_config_default() {
    assert_eq!(
        QualityConfig::default().on_violation,
        ViolationPolicy::AutoRefine
    );
    let preset = ProjectConfig::scaffold(
        "auto-refine-default",
        MeshIntentPreset::MeritHydroCoast,
        DomainConfig::Global,
        ResolutionSpec::ApproxKm(100.0),
    );
    assert_eq!(preset.quality.on_violation, ViolationPolicy::AutoRefine);
    assert!(!preset.refinement.enabled);
    assert!(!preset.refinement.threshold_enabled);
    assert_eq!(preset.refinement.max_passes, 0);

    let legacy = preset
        .to_yaml()
        .unwrap()
        .lines()
        .filter(|line| !line.trim_start().starts_with("on_violation:"))
        .collect::<Vec<_>>()
        .join("\n");
    let reopened = ProjectConfig::from_yaml(&legacy).unwrap();
    assert_eq!(reopened.quality.on_violation, ViolationPolicy::AutoRefine);
}

#[test]
fn threshold_refinement_is_opt_in_for_default_partial_and_scaffold_configs() {
    assert!(!RefinementRecipe::default().threshold_enabled);
    let partial: RefinementRecipe =
        serde_yaml::from_str("enabled: false\nmax_passes: 0\n").expect("partial refinement");
    assert!(!partial.threshold_enabled);

    let scaffold = ProjectConfig::scaffold(
        "threshold-opt-in",
        MeshIntentPreset::AtmosphereMpas,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    );
    assert!(!scaffold.refinement.threshold_enabled);
}

#[test]
fn project_validation_rejects_invalid_regional_shapes() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 112.0,
            e: 112.0,
            n: 23.5,
            s: 21.5,
        },
        sea_ratio: None,
    };
    let err = yaml_err(&p);
    assert!(err.contains("bbox west and east must differ"));

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 112.0,
            e: 115.0,
            n: 21.5,
            s: 21.5,
        },
        sea_ratio: None,
    };
    let err = yaml_err(&p);
    assert!(err.contains("bbox south must be < north"));

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Circle {
            lon: 113.0,
            lat: 22.0,
            radius_km: 0.0,
        },
        sea_ratio: None,
    };
    let err = json_err(&p);
    assert!(err.contains("circle radius_km must be > 0"));

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 112.0,
            e: 115.0,
            n: 23.5,
            s: 21.5,
        },
        sea_ratio: Some(1.5),
    };
    let err = json_err(&p);
    assert!(err.contains("domain sea_ratio must be between 0 and 1"));

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 112.0,
            e: 181.0,
            n: 23.5,
            s: 21.5,
        },
        sea_ratio: None,
    };
    let err = yaml_err(&p);
    assert!(err.contains("bbox longitudes must be between -180 and 180"));

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Circle {
            lon: 113.0,
            lat: -91.0,
            radius_km: 100.0,
        },
        sea_ratio: None,
    };
    let err = json_err(&p);
    assert!(err.contains("circle latitude must be between -90 and 90"));
}

#[test]
fn project_validation_rejects_empty_name() {
    let mut p = sample();
    p.metadata.name = "  ".into();
    let err = yaml_err(&p);
    assert!(err.contains("project metadata.name must not be empty"));

    p.metadata.name = "bad/name".into();
    let err = yaml_err(&p);
    assert!(err.contains("project metadata.name must not contain path separators"));

    let mut p = sample();
    p.metadata.name = "..".into();
    let err = yaml_err(&p);
    assert!(err.contains("project metadata.name must not be '.' or '..'"));
}

#[test]
fn project_validation_rejects_empty_schema_version() {
    let mut p = sample();
    p.schema_version = " ".into();
    let err = json_err(&p);
    assert!(err.contains("unsupported project schema_version"));
}

#[test]
fn project_validation_rejects_invalid_quality_gate() {
    let mut p = sample();
    p.quality.min_angle_deg = 0.0;
    let err = yaml_err(&p);
    assert!(err.contains("quality min_angle_deg must be > 0"));

    for min_angle_deg in [180.0, 181.0] {
        let mut p = sample();
        p.quality.min_angle_deg = min_angle_deg;
        let err = p.validate().unwrap_err();
        assert!(err.contains("quality min_angle_deg must be < 180"), "{err}");
        assert!(p.try_lower().is_err());
    }

    let mut p = sample();
    p.quality.auto_refine_batch_cells = 0;
    let err = yaml_err(&p);
    assert!(err.contains("quality auto_refine_batch_cells must be > 0"));
}

#[test]
fn project_validation_rejects_invalid_data_layers() {
    let mut p = sample();
    p.data_layers[0].path.clear();
    let err = yaml_err(&p);
    assert!(err.contains("data layer 'lc' is enabled but has no path"));

    let mut p = sample();
    p.data_layers[0].id = " ".into();
    let err = yaml_err(&p);
    assert!(err.contains("data layer id must not be empty"));

    let mut p = sample();
    p.data_layers[0].id = " lc".into();
    let err = yaml_err(&p);
    assert!(err.contains("data layer id must not have leading or trailing whitespace"));

    let mut p = sample();
    p.data_layers[1].id = p.data_layers[0].id.clone();
    let err = json_err(&p);
    assert!(err.contains("data layer id 'lc' is duplicated"));

    let mut p = sample();
    let mut duplicate = p.data_layers[1].clone();
    duplicate.id = "lai_second".into();
    p.data_layers.push(duplicate);
    let err = json_err(&p);
    assert!(err.contains("enabled threshold field 'lai' is duplicated"));

    let mut p = sample();
    p.data_layers.push(ProjectDataLayer {
        id: "alternate-landcover".into(),
        role: ProjectLayerRole::LandType,
        path: "./in/alternate-landtype.nc".into(),
        enabled: true,
        threshold_value: None,
    });
    let err = yaml_err(&p);
    assert!(err.contains("enabled LandType source is duplicated"));

    let mut p = sample();
    p.data_layers.push(ProjectDataLayer {
        id: "merit".into(),
        role: ProjectLayerRole::MeritHydro,
        path: "./merit".into(),
        enabled: true,
        threshold_value: Some(1.0),
    });
    let err = yaml_err(&p);
    assert!(err.contains("has a threshold value but is not a refinement layer"));

    let mut p = sample();
    p.data_layers[1].threshold_value = Some(f64::NAN);
    let err = yaml_err(&p);
    assert!(err.contains("threshold value must be finite"));

    let mut p = sample();
    p.data_layers[0].threshold_value = Some(0.0);
    let err = yaml_err(&p);
    assert!(err.contains("landcover class threshold must be > 0"));
}

#[test]
fn project_validation_accepts_all_thresholds_for_every_domain() {
    let cases = [
        (MeshDomainKind::Land, MeshCellKind::Hex, ModelFormat::CoLM),
        (MeshDomainKind::Ocean, MeshCellKind::Tri, ModelFormat::Fvcom),
        (
            MeshDomainKind::Atmosphere,
            MeshCellKind::Hex,
            ModelFormat::Mpas,
        ),
        (
            MeshDomainKind::Coupled,
            MeshCellKind::Hex,
            ModelFormat::CoLM,
        ),
        (MeshDomainKind::Earth, MeshCellKind::Hex, ModelFormat::CoLM),
    ];

    for (kind, cell, model_format) in cases {
        let mut project = sample();
        project.target.kind = kind;
        project.target.cell = cell;
        project.target.model_format = model_format;
        project.data_layers = criterion_catalog()
            .iter()
            .map(|criterion| ProjectDataLayer {
                id: criterion.field.stem().to_string(),
                role: ProjectLayerRole::Threshold(criterion.field),
                path: format!("./threshold/{}.nc", criterion.field.stem()),
                enabled: true,
                threshold_value: None,
            })
            .collect();
        project.data_layers.push(ProjectDataLayer {
            id: "landcover".into(),
            role: ProjectLayerRole::LandType,
            path: "./input/landtype_igbp_update.nc".into(),
            enabled: true,
            threshold_value: Some(12.0),
        });

        let lowered = project
            .try_lower()
            .unwrap_or_else(|error| panic!("{kind:?} rejected universal thresholds: {error}"));
        assert!(lowered.refine.refine_num_landtypes, "{kind:?}");
        earthmesh_core::RefineConfig::from_mkrefine_namelist(
            &lowered.to_namelist(),
            &lowered.mkgrd.mesh_type,
            &lowered.mkgrd.mode_grid,
        )
        .unwrap_or_else(|error| panic!("{kind:?} threshold namelist did not reparse: {error}"));
    }
}

#[test]
fn project_yaml_rejects_removed_specified_mask_role() {
    let yaml = r#"
schema_version: 3.0.0
metadata:
  name: bad_mask
domain: Global
target:
  kind: Land
  cell: Hex
  resolution: !Nxp 10
  model_format: CoLM
data_layers:
  - id: mask
    role: SpecifiedMask
    path: ./mask.nc4
    enabled: true
refinement:
  enabled: true
  max_passes: 1
"#;

    assert!(ProjectConfig::from_yaml(yaml)
        .unwrap_err()
        .contains("SpecifiedMask"));
}

#[test]
fn project_validation_rejects_invalid_resolution() {
    let mut p = sample();
    p.target.resolution = ResolutionSpec::Nxp(0);
    let err = yaml_err(&p);
    assert!(err.contains("target resolution Nxp must be > 0"));

    p.target.resolution = ResolutionSpec::ApproxKm(0.0);
    let err = json_err(&p);
    assert!(err.contains("target resolution ApproxKm must be > 0"));

    p.target.resolution = ResolutionSpec::ApproxDegree(0.0);
    let err = json_err(&p);
    assert!(err.contains("target resolution ApproxDegree must be > 0"));

    p.target.resolution = ResolutionSpec::Nxp(40);
    p.expert.nxp = Some(0);
    let err = yaml_err(&p);
    assert!(err.contains("expert nxp override must be > 0"));

    p.expert.nxp = None;
    p.expert.openmp = Some(0);
    let err = yaml_err(&p);
    assert!(err.contains("expert openmp override must be > 0"));

    p.expert.openmp = None;
    p.expert.niter = Some(0);
    let err = yaml_err(&p);
    assert!(err.contains("expert niter override must be > 0"));

    p.expert.niter = None;
    p.expert.niter_refine = Some(0);
    let err = yaml_err(&p);
    assert!(err.contains("expert niter_refine override must be > 0"));

    p.expert.niter_refine = None;
    p.expert.max_iter_spc = Some(6);
    let err = yaml_err(&p);
    assert!(err.contains("expert max_iter_spc override must be between 0 and 5"));

    p.expert.max_iter_spc = None;
    p.expert.halo = Some(vec![1; 10]);
    let err = yaml_err(&p);
    assert!(err.contains("expert HALO override must contain 1 to 9 values"));

    p.expert.halo = None;
    p.expert.max_transition_row = Some(vec![1; 10]);
    let err = yaml_err(&p);
    assert!(err.contains("expert max_transition_row override must contain 1 to 9 values"));

    p.expert.max_transition_row = None;
    p.expert.beta = Some(0.0);
    let err = yaml_err(&p);
    assert!(err.contains("expert beta override must be finite and > 0"));

    p.expert.beta = None;
    p.expert.relax = Some(0.0);
    let err = yaml_err(&p);
    assert!(err.contains("expert relax override must be finite and > 0"));
}

#[test]
fn enabled_refinement_rejects_zero_expert_level_overrides() {
    let mut calculated = sample();
    calculated.expert.max_iter_cal = Some(0);
    let err = yaml_err(&calculated);
    assert!(
        err.contains(
            "expert max_iter_cal override must be > 0 when calculated refinement is enabled"
        ),
        "{err}"
    );

    let mut specified = sample();
    specified.refinement.threshold_enabled = false;
    specified.refinement.specified_bbox = Some(SpecifiedBboxRefinement {
        w: 112.5,
        e: 113.0,
        s: 22.0,
        n: 22.5,
    });
    specified.expert.max_iter_spc = Some(0);
    let err = yaml_err(&specified);
    assert!(
        err.contains(
            "expert max_iter_spc override must be > 0 when specified refinement is enabled"
        ),
        "{err}"
    );

    // Canonical namelists keep the inactive source at level zero. Preserve that
    // representation when the corresponding source is not enabled.
    specified.expert.max_iter_spc = None;
    specified.expert.max_iter_cal = Some(0);
    ProjectConfig::from_yaml(&specified.to_yaml().expect("yaml"))
        .expect("zero is valid for the inactive calculated source");
}

#[test]
fn every_target_and_model_pairing_is_accepted_and_says_what_it_delivers() {
    // The canonical EarthMesh gridfile is written whatever the pairing, so a
    // combination with no adapter is a delivery fact, not a configuration
    // error. Refusing it made a user commit to a model before they knew which
    // one would consume the mesh, and produced nothing when they guessed wrong.
    let mut p = sample();
    for (kind, model_format) in [
        (MeshDomainKind::Atmosphere, ModelFormat::CoLM),
        (MeshDomainKind::Ocean, ModelFormat::CoLM),
        (MeshDomainKind::Coupled, ModelFormat::Fvcom),
        (MeshDomainKind::Land, ModelFormat::Icon),
        (MeshDomainKind::Ocean, ModelFormat::MpasOcean),
    ] {
        p.target.kind = kind;
        p.target.model_format = model_format;
        yaml_round_trip(&p);
    }

    // What a pairing costs is stated instead of refused. Cell shape is what
    // decides: the MPAS adapters need hexagons, ICON and FVCOM need triangles.
    p.target.cell = MeshCellKind::Hex;
    p.target.model_format = ModelFormat::Icon;
    let triple = ProjectTargetTriple::from(&p.target);
    assert_eq!(triple.output_delivery(), ProjectOutputDelivery::GridOnly);
    assert!(
        triple
            .skipped_adapter_reason()
            .is_some_and(|reason| reason.contains("triangular")),
        "{:?}",
        triple.skipped_adapter_reason()
    );

    p.target.cell = MeshCellKind::Tri;
    p.target.model_format = ModelFormat::Icon;
    let triple = ProjectTargetTriple::from(&p.target);
    assert_eq!(triple.output_delivery(), ProjectOutputDelivery::Full);
    assert_eq!(triple.skipped_adapter_reason(), None);
}

#[test]
fn project_validation_rejects_refinement_without_source() {
    let mut p = sample();
    p.data_layers.clear();
    let err = yaml_err(&p);
    assert!(err.contains("refinement is enabled but no refinement source is enabled"));

    p.data_layers.push(ProjectDataLayer {
        id: "merit".into(),
        role: ProjectLayerRole::MeritHydro,
        path: "./merit".into(),
        enabled: true,
        threshold_value: None,
    });
    let err = json_err(&p);
    assert!(err.contains("refinement is enabled but no refinement source is enabled"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "./merit".into(),
        cama_root: None,
        merit_stride: 1,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
        r3_upa_km2: 50_000.0,
        r2_upa_km2: 5_000.0,
        river_refinement_enabled: true,
        river_width_refinement_enabled: true,
        river_upstream_area_refinement_enabled: true,
        river_width_threshold_m: None,
        river_upstream_area_threshold_km2: None,
        coast_refinement_enabled: false,
        coast_buffer_km: 50.0,
        coast_land_refinement_enabled: true,
        coast_ocean_refinement_enabled: true,
    });
    p.data_layers.push(ProjectDataLayer {
        id: "landcover".into(),
        role: ProjectLayerRole::LandType,
        path: "./in/landtype.nc".into(),
        enabled: true,
        threshold_value: None,
    });
    ProjectConfig::from_json(&p.to_json().expect("json"))
        .expect("MERIT river thresholds with the coupled surface source must validate");
}

#[test]
fn project_validation_rejects_invalid_refinement_passes() {
    let mut p = sample();
    p.refinement.max_passes = 0;
    let err = yaml_err(&p);
    assert!(err.contains("refinement max_passes must be > 0"));

    p.refinement.max_passes = 6;
    let err = json_err(&p);
    assert!(err.contains("refinement max_passes must be <= 5"));
}

#[test]
fn project_validation_rejects_invalid_hydro_coast_config() {
    let mut p = sample();
    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: " ".into(),
        cama_root: None,
        merit_stride: 1,
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
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast merit_root must not be empty"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: Some(" ".into()),
        merit_stride: 1,
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
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast cama_root must not be empty"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        merit_stride: 1,
        r3_width_m: 0.0,
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
    let err = json_err(&p);
    assert!(err.contains("hydro_coast widths must be > 0"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        merit_stride: 1,
        r3_width_m: 50.0,
        r2_width_m: 300.0,
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
    let err = json_err(&p);
    assert!(err.contains("hydro_coast r3_width_m must be >= r2_width_m"));

    let hydro = p.hydro_coast.as_mut().unwrap();
    hydro.r3_width_m = 300.0;
    hydro.r2_width_m = 50.0;
    hydro.r3_upa_km2 = 1_000.0;
    hydro.r2_upa_km2 = 5_000.0;
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast r3_upa_km2 must be >= r2_upa_km2"));

    let hydro = p.hydro_coast.as_mut().unwrap();
    hydro.r3_upa_km2 = 50_000.0;
    hydro.river_width_threshold_m = Some(49.0);
    let err = json_err(&p);
    assert!(err.contains("river_width_threshold_m must be >= the supported river width"));
    let hydro = p.hydro_coast.as_mut().unwrap();
    hydro.river_width_threshold_m = Some(300.0);
    hydro.river_upstream_area_threshold_km2 = Some(4_999.0);
    let err = yaml_err(&p);
    assert!(
        err.contains("river_upstream_area_threshold_km2 must be >= the supported upstream area")
    );
    let hydro = p.hydro_coast.as_mut().unwrap();
    hydro.river_upstream_area_threshold_km2 = Some(50_000.0);
    hydro.coast_buffer_km = -1.0;
    let err = json_err(&p);
    assert!(err.contains("hydro_coast coast_buffer_km must be finite and >= 0"));
    p.hydro_coast.as_mut().unwrap().coast_buffer_km = 1_001.0;
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast coast_buffer_km must be <= 1000"));
}

#[test]
fn legacy_hydro_yaml_defaults_upstream_thresholds_and_refinement_switches() {
    let mut p = sample();
    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        merit_stride: 1,
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
    let yaml = p
        .to_yaml()
        .unwrap()
        .lines()
        .filter(|line| {
            !line.contains("_upa_km2:")
                && !line.contains("river_refinement_enabled:")
                && !line.contains("river_width_refinement_enabled:")
                && !line.contains("river_upstream_area_refinement_enabled:")
                && !line.contains("coast_refinement_enabled:")
                && !line.contains("coast_buffer_km:")
                && !line.contains("coast_land_refinement_enabled:")
                && !line.contains("coast_ocean_refinement_enabled:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let loaded = ProjectConfig::from_yaml(&yaml).unwrap();
    let hydro = loaded.hydro_coast.unwrap();
    assert_eq!(hydro.r2_upa_km2, 5_000.0);
    assert_eq!(hydro.r3_upa_km2, 50_000.0);
    assert!(hydro.river_refinement_enabled);
    assert!(hydro.river_width_refinement_enabled);
    assert!(hydro.river_upstream_area_refinement_enabled);
    assert_eq!(hydro.river_width_threshold_m, None);
    assert_eq!(hydro.river_upstream_area_threshold_km2, None);
    assert_eq!(hydro.effective_river_width_threshold_m(), 300.0);
    assert_eq!(
        hydro.effective_river_upstream_area_threshold_km2(),
        50_000.0
    );
    assert!(hydro.coast_refinement_enabled);
    assert_eq!(hydro.coast_buffer_km, 0.0);
    assert!(hydro.coast_land_refinement_enabled);
    assert!(hydro.coast_ocean_refinement_enabled);
}

#[test]
fn lower_maps_to_engine_config() {
    let lowered = sample().lower();
    assert_eq!(lowered.mkgrd.mesh_type, "LOCmesh");
    assert_eq!(lowered.mkgrd.mode_grid, "hex");
    assert_eq!(lowered.mkgrd.output_format, "CoLM");
    assert_eq!(lowered.mkgrd.nxp, 42);
    assert!(!lowered.mkgrd.mask_domain_global);
    assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        "inline:bbox:w=112,e=115.5,s=21.5,n=23.5"
    );
    assert_eq!(lowered.mkgrd.mask_sea_ratio, 0.25);
    assert_eq!(lowered.mkgrd.experiment_name, "gba");
    assert_eq!(lowered.mkgrd.openmp, 8); // expert override
    assert_eq!(lowered.mkgrd.beta, 1.1);
    assert_eq!(lowered.mkgrd.relax, 0.03);

    // LandType supplies the mask only; LAI independently drives calculated refinement.
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
    assert!(!lowered.refine.refine_num_landtypes);
    assert_eq!(lowered.refine.th_num_landtypes, 12);
    assert!(lowered.refine.refine_onelayer_lnd[0] && lowered.refine.refine_onelayer_lnd[1]);
    assert_eq!(lowered.refine.th_onelayer_lnd[0], 1.0);
    assert_eq!(lowered.refine.th_onelayer_lnd[1], 1.0);
    assert!(lowered.refine.refine_cal);
    assert_eq!(lowered.refine.max_iter_cal, 3);
    assert_eq!(lowered.refine.niter_refine, 120);
    assert!(lowered.refine.niter_refine_specified);
    assert_eq!(&lowered.refine.halo[1..4], &[4, 4, 3]);
    assert_eq!(&lowered.refine.max_transition_row[1..4], &[5, 4, 3]);
    assert_eq!(lowered.refine.set_dis_type, "linear");
    assert_eq!(lowered.refine.num_rc, 1);
    assert_eq!(lowered.refine.vertex_pretect_layers, 2);
    assert_eq!(lowered.refine.spring_global_type, 0);
    assert_eq!(lowered.refine.spring_regional_type, 1);
    assert!(lowered.refine.weak_concav_eliminate);

    // quality
    assert_eq!(lowered.quality.min_angle_warn_deg, 28.0);
    assert_eq!(lowered.quality.on_violation, "block");

    // runnable namelist with all four blocks
    let nml = lowered.to_namelist();
    assert!(nml.contains("&mkgrd"));
    assert!(nml.contains("&mkrefine"));
    assert!(nml.contains("&adaptive"));
    assert!(nml.contains("&quality"));
    assert!(nml.contains("&datalayers"));

    let reparsed_quality = earthmesh_core::QualityNamelist::from_quality_namelist(&nml)
        .expect("compiled project quality config should reparse");
    assert_eq!(reparsed_quality, lowered.quality);

    let reparsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        &nml,
        &lowered.mkgrd.mesh_type,
        &lowered.mkgrd.mode_grid,
    )
    .expect("compiled project refine config should reparse");
    assert_eq!(&reparsed.halo[1..4], &[4, 4, 3]);
    assert_eq!(&reparsed.max_transition_row[1..4], &[5, 4, 3]);
}

#[test]
fn landtype_mask_and_landcover_criterion_are_independent_with_legacy_fallback() {
    let mask_and_lai = sample();
    let mask_only_criterion = mask_and_lai
        .effective_landcover_criterion()
        .expect("landcover source");
    assert!(mask_only_criterion.source_enabled);
    assert!(!mask_only_criterion.enabled);
    assert_eq!(mask_only_criterion.value, DEFAULT_LANDCOVER_CLASS_THRESHOLD);
    let lowered = mask_and_lai.lower();
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
    assert!(lowered.refine.refine_cal, "LAI remains a refinement source");
    assert!(!lowered.refine.refine_num_landtypes);
    let relowered = earthmesh_core::lower_datalayers_namelist(&lowered.to_namelist(), None)
        .expect("shared CLI lowering");
    let reparsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        &relowered.namelist,
        &lowered.mkgrd.mesh_type,
        &lowered.mkgrd.mode_grid,
    )
    .expect("reparse mask plus LAI");
    assert!(!reparsed.refine_num_landtypes);

    let mut explicit_landcover = sample();
    explicit_landcover
        .refinement
        .threshold_criteria
        .push(ThresholdCriterionConfig {
            id: LANDCOVER_CRITERION_ID.into(),
            enabled: true,
            value: Some(8.0),
        });
    let lowered = explicit_landcover.lower();
    assert!(explicit_landcover
        .effective_landcover_criterion()
        .is_some_and(|criterion| criterion.enabled && criterion.value == 8.0));
    assert!(lowered.refine.refine_num_landtypes);
    assert_eq!(lowered.refine.th_num_landtypes, 8);
    let relowered = earthmesh_core::lower_datalayers_namelist(&lowered.to_namelist(), None)
        .expect("shared CLI lowering");
    let reparsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        &relowered.namelist,
        &lowered.mkgrd.mesh_type,
        &lowered.mkgrd.mode_grid,
    )
    .expect("reparse explicit landcover criterion");
    assert!(reparsed.refine_num_landtypes);
    assert_eq!(reparsed.th_num_landtypes, 8);
    let landtype_layer = lowered
        .data_layers
        .layers
        .iter()
        .find(|layer| matches!(layer.role, earthmesh_core::DataLayerRole::LandType))
        .expect("landtype layer");
    assert!(landtype_layer.categorical_enabled);

    let mut legacy = sample();
    legacy.data_layers[0].threshold_value = Some(9.0);
    let lowered = legacy.lower();
    assert!(legacy
        .effective_landcover_criterion()
        .is_some_and(|criterion| criterion.enabled && criterion.value == 9.0));
    assert!(lowered.refine.refine_num_landtypes);
    assert_eq!(lowered.refine.th_num_landtypes, 9);

    let mut explicitly_disabled_legacy = legacy;
    explicitly_disabled_legacy
        .refinement
        .threshold_criteria
        .push(ThresholdCriterionConfig {
            id: LANDCOVER_CRITERION_ID.into(),
            enabled: false,
            value: None,
        });
    let lowered = explicitly_disabled_legacy.lower();
    assert!(explicitly_disabled_legacy
        .effective_landcover_criterion()
        .is_some_and(|criterion| {
            !criterion.enabled && criterion.value == DEFAULT_LANDCOVER_CLASS_THRESHOLD
        }));
    assert!(!lowered.refine.refine_num_landtypes);
    assert!(lowered.refine.refine_cal, "LAI still drives refinement");
}

#[test]
fn threshold_master_switch_keeps_landtype_available_without_calculated_refinement() {
    let mut project = sample();
    project.refinement.threshold_enabled = false;
    project.refinement.specified_circle =
        Some(SpecifiedCircleRefinements::One(SpecifiedCircleRefinement {
            lon: 113.0,
            lat: 22.5,
            radius_km: 100.0,
        }));

    let lowered = project.lower();
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
    assert!(!lowered.refine.refine_cal);
    assert!(!lowered.refine.refine_num_landtypes);
    assert!(!lowered.refine.refine_onelayer_lnd[0]);
    assert!(lowered.refine.refine_spc);
    assert_eq!(lowered.data_layers.layers.len(), 1);
    assert!(matches!(
        lowered.data_layers.layers[0].role,
        earthmesh_core::DataLayerRole::LandType
    ));
}

#[test]
fn geometry_lowering_preserves_project_shapes() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Circle {
            lon: 113.5,
            lat: 22.25,
            radius_km: 80.0,
        },
        sea_ratio: None,
    };
    p.data_layers.clear();
    p.refinement.specified_bbox = Some(SpecifiedBboxRefinement {
        w: 112.0,
        e: 115.5,
        s: 21.5,
        n: 23.5,
    });

    let lowered = p.lower();

    assert_eq!(lowered.mkgrd.mask_domain_type, "circle");
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        "inline:circle:lon=113.5,lat=22.25,radius_km=80"
    );
    assert!(lowered.refine.refine_spc);
    assert_eq!(lowered.refine.mask_refine_spc_type, "bbox");
    assert_eq!(
        lowered.refine.mask_refine_spc_fprefix,
        "inline:bbox:w=112,e=115.5,s=21.5,n=23.5"
    );
}

#[test]
fn specified_close_refinement_accepts_supported_source_files() {
    let mut p = sample();
    p.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "./masks/close_001.nml".into(),
        boundary: CloseBoundaryMode::Polyline,
    });

    let lowered = p.try_lower().expect("close nml refinement should validate");

    assert_eq!(lowered.refine.mask_refine_spc_type, "close");
    assert_eq!(
        lowered.refine.mask_refine_spc_fprefix,
        "./masks/close_001.nml"
    );

    p.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "./masks/close_001.txt".into(),
        boundary: CloseBoundaryMode::Polyline,
    });
    assert!(p.try_lower().is_ok());

    p.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "./masks/close_001.shp".into(),
        boundary: CloseBoundaryMode::Polyline,
    });
    assert!(p.try_lower().is_ok());
}

#[test]
fn close_boundary_modes_lower_into_domain_and_specified_engine_specs() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "./masks/domain_close.nml".into(),
            format: CloseMaskFormat::Nml,
            boundary: CloseBoundaryMode::SphericalChaikin {
                iterations: 2,
                max_segment_angle_deg: 0.25,
            },
        },
        sea_ratio: None,
    };
    p.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "./masks/refine_close.nml".into(),
        boundary: CloseBoundaryMode::EnclosingCap {
            margin_km: 20.0,
            max_radius_deg: 80.0,
            max_segment_angle_deg: 0.25,
        },
    });

    let lowered = p.try_lower().expect("close boundary modes lower");
    assert_eq!(
        lowered.mkgrd.mask_domain_close_boundary,
        "spherical_chaikin:iterations=2,max_segment_angle_deg=0.25"
    );
    assert_eq!(
        lowered.refine.mask_refine_spc_close_boundary,
        "enclosing_cap:margin_km=20,max_radius_deg=80,max_segment_angle_deg=0.25"
    );
}

#[test]
fn bbox_validation_allows_antimeridian_spans() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 170.0,
            e: -170.0,
            s: -10.0,
            n: 10.0,
        },
        sea_ratio: None,
    };

    let lowered = p.try_lower().expect("antimeridian bbox should validate");

    assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        "inline:bbox:w=170,e=-170,s=-10,n=10"
    );
}

#[test]
fn close_domain_validation_accepts_gui_convertible_formats() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "./masks/domain_close.nml".into(),
            format: CloseMaskFormat::Nml,
            boundary: CloseBoundaryMode::Polyline,
        },
        sea_ratio: None,
    };
    assert!(p.try_lower().is_ok());

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "./masks/domain_close.shp".into(),
            format: CloseMaskFormat::PolygonShp,
            boundary: CloseBoundaryMode::Polyline,
        },
        sea_ratio: None,
    };
    assert!(p.try_lower().is_ok());

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Close {
            path: "./masks/domain_close.txt".into(),
            format: CloseMaskFormat::LonLatText,
            boundary: CloseBoundaryMode::Polyline,
        },
        sea_ratio: None,
    };
    assert!(p.try_lower().is_ok());
}

#[test]
fn shapefile_domain_lowers_to_close_adapter_input() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Shapefile {
            path: "./watershed.shp".into(),
        },
        sea_ratio: None,
    };

    let lowered = p.try_lower().expect("watershed SHP should lower");
    assert_eq!(lowered.mkgrd.mask_domain_type, "close");
    assert_eq!(lowered.mkgrd.mask_domain_fprefix, "./watershed.shp");
}

#[test]
fn threshold_value_override_lowers_to_engine_arrays() {
    let mut p = sample();
    p.data_layers[0].threshold_value = Some(8.0);
    p.data_layers[1].threshold_value = Some(4.5);
    p.data_layers.push(ProjectDataLayer {
        id: "dem".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::Dem),
        path: "./th/dem.nc".into(),
        enabled: true,
        threshold_value: Some(123.0),
    });

    let lowered = p.lower();

    assert_eq!(lowered.refine.th_num_landtypes, 8);
    assert_eq!(lowered.refine.th_onelayer_lnd[0], 4.5);
    assert_eq!(lowered.refine.th_onelayer_lnd[1], 4.5);
    assert_eq!(lowered.refine.th_onelayer_lnd[4], 123.0);
    assert_eq!(lowered.refine.th_onelayer_lnd[5], 123.0);
}

#[test]
fn threshold_axes_lower_mean_and_std_independently_and_preserve_two_layer_slots() {
    let mut p = sample();
    p.data_layers[1].threshold_value = Some(4.5);
    p.data_layers.push(ProjectDataLayer {
        id: "k_s".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::Ks),
        path: "./th/k_s.nc".into(),
        enabled: true,
        threshold_value: Some(0.01),
    });

    let mut value = serde_yaml::to_value(&p).expect("project value");
    let refinement = value
        .get_mut("refinement")
        .and_then(serde_yaml::Value::as_mapping_mut)
        .expect("refinement mapping");
    refinement.insert(
        serde_yaml::Value::String("threshold_criteria".into()),
        serde_yaml::from_str(
            r#"
- id: lai_mean
  enabled: false
  value: 2.5
- id: lai_std
  enabled: true
  value: 7.5
- id: k_s_mean
  enabled: true
  value: 0.02
- id: k_s_std
  enabled: false
  value: 0.04
"#,
        )
        .expect("threshold axes"),
    );
    let configured =
        ProjectConfig::from_yaml(&serde_yaml::to_string(&value).expect("configured project yaml"))
            .expect("independent threshold axes must parse");

    let lowered = configured.lower();
    let lai_mean = configured
        .effective_threshold_criterion(ThresholdField::Lai, ThresholdStatistic::Mean)
        .expect("effective lai mean");
    assert_eq!(lai_mean.source_layer_id, "lai");
    assert!(lai_mean.source_enabled);
    assert!(!lai_mean.enabled);
    assert_eq!(lai_mean.value, 2.5);

    let mut source_disabled = configured.clone();
    source_disabled
        .data_layers
        .iter_mut()
        .find(|layer| layer.role == ProjectLayerRole::Threshold(ThresholdField::Ks))
        .expect("soil hydraulic conductivity source")
        .enabled = false;
    let soil_ks_mean = source_disabled
        .effective_threshold_criterion(ThresholdField::Ks, ThresholdStatistic::Mean)
        .expect("effective soil hydraulic conductivity mean");
    assert!(!soil_ks_mean.source_enabled);
    assert!(soil_ks_mean.enabled);

    let mut replacement_source = configured.clone();
    replacement_source
        .data_layers
        .iter_mut()
        .find(|layer| layer.role == ProjectLayerRole::Threshold(ThresholdField::Lai))
        .expect("canonical lai source")
        .enabled = false;
    replacement_source.data_layers.push(ProjectDataLayer {
        id: "custom_lai".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::Lai),
        path: "./custom/lai.nc".into(),
        enabled: true,
        threshold_value: None,
    });
    let replacement_lai = replacement_source
        .effective_threshold_criterion(ThresholdField::Lai, ThresholdStatistic::Mean)
        .expect("replacement lai source must win");
    assert_eq!(replacement_lai.source_layer_id, "custom_lai");
    assert!(replacement_lai.source_enabled);
    assert!(!lowered.refine.refine_onelayer_lnd[0]);
    assert!(lowered.refine.refine_onelayer_lnd[1]);
    assert_eq!(lowered.refine.th_onelayer_lnd[0], 2.5);
    assert_eq!(lowered.refine.th_onelayer_lnd[1], 7.5);
    assert!(lowered.refine.refine_twolayer_lnd[0]);
    assert!(!lowered.refine.refine_twolayer_lnd[1]);
    assert_eq!(lowered.refine.th_twolayer_lnd[0], [0.02, 0.02]);
    assert_eq!(lowered.refine.th_twolayer_lnd[1], [0.04, 0.04]);

    let reparsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        &lowered.to_namelist(),
        &lowered.mkgrd.mesh_type,
        &lowered.mkgrd.mode_grid,
    )
    .expect("axis-aware datalayers must not re-enable disabled axes");
    assert!(!reparsed.refine_onelayer_lnd[0]);
    assert!(reparsed.refine_onelayer_lnd[1]);
    assert!(reparsed.refine_twolayer_lnd[0]);
    assert!(!reparsed.refine_twolayer_lnd[1]);

    let relowered = earthmesh_core::lower_datalayers_namelist(&lowered.to_namelist(), None)
        .expect("axis-aware datalayers must survive the CLI lowering pass");
    let reparsed = earthmesh_core::RefineConfig::from_mkrefine_namelist(
        &relowered.namelist,
        &lowered.mkgrd.mesh_type,
        &lowered.mkgrd.mode_grid,
    )
    .expect("CLI-lowered independent threshold axes");
    assert!(!reparsed.refine_onelayer_lnd[0]);
    assert!(reparsed.refine_onelayer_lnd[1]);
    assert!(reparsed.refine_twolayer_lnd[0]);
    assert!(!reparsed.refine_twolayer_lnd[1]);
}

#[test]
fn explicit_blank_threshold_axis_uses_its_default_not_the_legacy_shared_value() {
    let mut project = sample();
    project.data_layers[1].threshold_value = Some(99.0);
    project
        .refinement
        .threshold_criteria
        .push(ThresholdCriterionConfig {
            id: "lai_mean".into(),
            enabled: true,
            value: None,
        });

    let expected_default = threshold_criterion_by_id("lai_mean")
        .expect("cataloged LAI mean")
        .gui
        .default;
    let mean = project
        .effective_threshold_criterion(ThresholdField::Lai, ThresholdStatistic::Mean)
        .expect("effective LAI mean");
    let std = project
        .effective_threshold_criterion(ThresholdField::Lai, ThresholdStatistic::Std)
        .expect("effective LAI std");

    assert_eq!(mean.value, expected_default);
    assert_eq!(std.value, 99.0, "the omitted sibling keeps legacy fallback");
}

#[test]
fn landtype_is_required_for_surface_targets_but_skipped_for_idle_atmosphere() {
    for (kind, format) in [
        (MeshDomainKind::Land, ModelFormat::CoLM),
        (MeshDomainKind::Ocean, ModelFormat::Fvcom),
    ] {
        let mut p = sample();
        p.target.kind = kind;
        p.target.model_format = format;
        p.data_layers[0].enabled = false;
        let error = p
            .try_lower()
            .expect_err("surface targets require a landtype carve source");
        assert!(error.contains("landtype"), "{kind:?}: {error}");
    }

    let mut coupled = sample();
    coupled.data_layers[0].enabled = false;
    coupled.coupling = Some(CoupledMeshConfig::default());
    let error = coupled
        .try_lower()
        .expect_err("active coupling requires a landtype source");
    assert!(error.contains("landtype"), "{error}");

    let mut atmosphere = sample();
    atmosphere.target.kind = MeshDomainKind::Atmosphere;
    atmosphere.target.model_format = ModelFormat::Mpas;
    atmosphere.refinement.enabled = false;
    atmosphere.refinement.threshold_enabled = false;
    atmosphere.refinement.max_passes = 0;
    let lowered = atmosphere
        .try_lower()
        .expect("idle atmosphere must not require or lower landtype");
    assert_eq!(lowered.mkgrd.landtype_file, "none");
    assert!(lowered.data_layers.layers.iter().any(|layer| {
        matches!(layer.role, earthmesh_core::DataLayerRole::LandType) && !layer.enabled
    }));

    atmosphere.refinement.enabled = true;
    atmosphere.refinement.threshold_enabled = true;
    atmosphere.refinement.max_passes = 1;
    atmosphere.data_layers[1].enabled = false;
    atmosphere
        .refinement
        .threshold_criteria
        .push(ThresholdCriterionConfig {
            id: LANDCOVER_CRITERION_ID.into(),
            enabled: true,
            value: None,
        });
    let lowered = atmosphere
        .try_lower()
        .expect("landcover criterion must keep landtype active for atmosphere");
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
}

#[test]
fn the_point_radius_route_is_lowered_for_both_backends() {
    // Its criteria half is raster work that produces an ordinary circle list,
    // and both backends consume it; only turning circles into mesh is
    // per-backend. Gating the section on Method-C would take the criteria away
    // from the one backend that can actually build a coastline -- which is the
    // whole reason red-green exists.
    let mut p = sample();
    p.refinement.hfield = None;
    p.refinement.adaptive = None;

    p.refinement.backend = crate::RefinementBackend::MethodC;
    let nml = p.lower().to_namelist();
    assert!(nml.contains("&adaptive"), "{nml}");

    p.refinement.backend = crate::RefinementBackend::RedGreen;
    let nml = p.lower().to_namelist();
    assert!(nml.contains("red_green"), "{nml}");
    assert!(nml.contains("&adaptive"), "{nml}");
}

#[test]
fn point_radius_is_the_default_and_the_h_field_is_opt_in() {
    // A run refines one way or the other, and point+radius is the one that can
    // re-ask a criterion after the cells it judges exist.
    let mut p = sample();
    p.refinement.hfield = None;
    let nml = p.lower().to_namelist();
    assert!(nml.contains("&adaptive"), "{nml}");
    assert!(!nml.contains("&hfield"), "{nml}");

    // Asking for the h-field turns the adaptive route off rather than running
    // both; two backends refining the same mesh is not a state that means
    // anything.
    p.refinement.hfield = Some(HfieldRefinementRecipe::default());
    let nml = p.lower().to_namelist();
    assert!(nml.contains("&hfield"), "{nml}");
    assert!(!nml.contains("&adaptive"), "{nml}");

    // Turning the adaptive route off without asking for the h-field leaves the
    // run on the plain region path.
    p.refinement.hfield = None;
    p.refinement.adaptive = Some(AdaptiveRefinementRecipe {
        enabled: false,
        ..AdaptiveRefinementRecipe::default()
    });
    let nml = p.lower().to_namelist();
    assert!(!nml.contains("&adaptive"), "{nml}");
    assert!(!nml.contains("&hfield"), "{nml}");
    p.refinement.adaptive = None;

    p.refinement.hfield = Some(HfieldRefinementRecipe {
        enabled: false,
        ..HfieldRefinementRecipe::default()
    });
    assert!(!p.lower().to_namelist().contains("&hfield"));

    let parsed: HfieldRefinementRecipe = serde_yaml::from_str("g: 0.3\n").expect("hfield yaml");
    assert!(parsed.enabled);
    assert_eq!(parsed.g, 0.3);

    p.refinement.hfield = Some(HfieldRefinementRecipe {
        origin_lon: Some(120.0),
        origin_lat: Some(30.0),
        ..HfieldRefinementRecipe::default()
    });
    let nml = p.lower().to_namelist();
    assert!(nml.contains("hfield_origin_lon = 120"));
    assert!(nml.contains("hfield_origin_lat = 30"));
}

#[test]
fn hex_mesh_lowers_with_is_transition_and_one_spring() {
    // The engine rejects hex meshes unless Istransition=true with exactly one
    // spring type > 0 (core validate_like_read_nl).
    let lowered = sample().lower();
    assert_eq!(lowered.mkgrd.mode_grid, "hex");
    assert!(lowered.refine.is_transition, "hex needs is_transition=true");
    let g = lowered.refine.spring_global_type > 0;
    let r = lowered.refine.spring_regional_type > 0;
    assert!(
        g ^ r,
        "exactly one spring type > 0, got global={} regional={}",
        lowered.refine.spring_global_type,
        lowered.refine.spring_regional_type
    );
}

#[test]
fn tri_mesh_also_lowers_with_is_transition_and_one_spring() {
    // Tri is not *required* to run with Istransition, but leaving it unset made
    // RefineConfig::validate zero both spring types, so method_c_spring_iterations
    // returned 0 and Method-C transition rows were never smoothed. Measured on a
    // global 100 km CoastalOcean project, enabling the spring moved
    // angle_deviation_deg.max from 40.87 to 27.05 and cleared its warn gate.
    let mut project = sample();
    project.target.cell = MeshCellKind::Tri;
    // The fixture pins both spring fields; clear them so this exercises the
    // automatic derivation rather than the expert override that runs after it.
    project.expert.spring_global_type = None;
    project.expert.spring_regional_type = None;
    let lowered = project.lower();

    assert_eq!(lowered.mkgrd.mode_grid, "tri");
    assert!(lowered.refine.is_transition, "tri must smooth too");
    // Regional domain => regional spring; both set so expert overrides of a
    // single field cannot collide with the other one's default of 1.
    assert_eq!(lowered.refine.spring_global_type, 0);
    assert_eq!(lowered.refine.spring_regional_type, 1);
}

#[test]
fn global_tri_mesh_lowers_to_the_global_spring() {
    let mut project = sample();
    project.target.cell = MeshCellKind::Tri;
    project.domain = DomainConfig::Global;
    project.expert.spring_global_type = None;
    project.expert.spring_regional_type = None;
    let lowered = project.lower();

    assert_eq!(lowered.refine.spring_global_type, 1);
    assert_eq!(lowered.refine.spring_regional_type, 0);
}

#[test]
fn harp_dv_does_not_lower_the_generic_spring() {
    let mut project = sample();
    project.refinement.backend = crate::RefinementBackend::HarpDv;
    let lowered = project.lower();

    assert_eq!(lowered.refine.spring_global_type, 0);
    assert_eq!(lowered.refine.spring_regional_type, 0);
}

#[test]
fn hfield_raster_targets_eight_base_cells_per_raster_cell() {
    // Measured window, in base cells per raster cell: 4 fails (aliased), 6.9-12
    // passes at both resolutions, 32 fails (fragmented). Target the middle.
    let mut project = sample();
    project.target.cell = MeshCellKind::Tri;
    project.target.resolution = ResolutionSpec::Nxp(81);
    project.refinement.max_passes = 1;
    project.expert.max_iter_cal = None;
    project.expert.max_iter_spc = None;
    // The raster only exists on the h-field route, which is now opt-in.
    project.refinement.hfield = Some(HfieldRefinementRecipe::default());
    let nml = project.lower().to_namelist();

    assert!(
        nml.contains("NL%hfield_nlat = 1620"),
        "NXP 81 must derive 20*NXP; got:\n{nml}"
    );
    assert!(nml.contains("NL%hfield_nlon = 3240"));

    // Deeper refinement must NOT change it. An earlier level-dependent rule
    // derived a failing raster for low-NXP multi-level runs.
    project.refinement.max_passes = 3;
    let deeper = project.lower().to_namelist();
    assert!(
        deeper.contains("NL%hfield_nlat = 1620"),
        "raster must not depend on level; got:\n{deeper}"
    );

    // The case the level-dependent rule regressed: NXP 21 two-level derived
    // nlat 840, which fails, while 420 passes.
    project.target.resolution = ResolutionSpec::Nxp(21);
    project.refinement.max_passes = 2;
    let coarse = project.lower().to_namelist();
    assert!(
        coarse.contains("NL%hfield_nlat = 420"),
        "NXP 21 must stay at 20*NXP; got:\n{coarse}"
    );
}

#[test]
fn explicit_hfield_raster_overrides_the_derivation() {
    let mut project = sample();
    project.refinement.hfield = Some(crate::HfieldRefinementRecipe {
        enabled: true,
        nlon: Some(512),
        nlat: Some(256),
        ..crate::HfieldRefinementRecipe::default()
    });
    let nml = project.lower().to_namelist();
    assert!(nml.contains("NL%hfield_nlon = 512"));
    assert!(nml.contains("NL%hfield_nlat = 256"));
}

#[test]
fn an_existing_single_circle_yaml_still_parses() {
    // Projects written before the chain existed must keep loading unchanged.
    let refinement: RefinementRecipe = serde_yaml::from_str(
        "enabled: true\nspecified_circle:\n  lon: 114.0\n  lat: 22.0\n  radius_km: 200.0\n",
    )
    .expect("single-circle yaml");
    let circles = refinement
        .specified_circle
        .as_ref()
        .expect("circle present")
        .as_slice();
    assert_eq!(circles.len(), 1);
    assert_eq!(circles[0].lon, 114.0);
}

#[test]
fn a_circle_chain_yaml_parses_as_a_list() {
    let refinement: RefinementRecipe = serde_yaml::from_str(
        "enabled: true\nspecified_circle:\n  - lon: 114.0\n    lat: 22.0\n    radius_km: 200.0\n  - lon: 115.5\n    lat: 22.5\n    radius_km: 200.0\n",
    )
    .expect("chain yaml");
    let circles = refinement
        .specified_circle
        .as_ref()
        .expect("circles present")
        .as_slice();
    assert_eq!(circles.len(), 2);
    assert_eq!(circles[1].lon, 115.5);
}

#[test]
fn a_mistyped_circle_key_still_names_itself() {
    // The hand-written deserializer exists for this: `#[serde(untagged)]` would
    // report "data did not match any variant", naming neither the bad key nor
    // the good ones.
    let error = serde_yaml::from_str::<RefinementRecipe>(
        "enabled: true\nspecified_circle:\n  lonn: 114.0\n  lat: 22.0\n  radius_km: 200.0\n",
    )
    .expect_err("typo must not parse");
    assert!(
        error.to_string().contains("unknown field `lonn`"),
        "got {error}"
    );

    let error = serde_yaml::from_str::<RefinementRecipe>(
        "enabled: true\nspecified_circle:\n  - lonn: 114.0\n    lat: 22.0\n    radius_km: 200.0\n",
    )
    .expect_err("typo in a chain must not parse");
    assert!(
        error.to_string().contains("unknown field `lonn`"),
        "got {error}"
    );
}

#[test]
fn a_single_specified_circle_lowers_exactly_as_before() {
    let mut project = sample();
    project.refinement.specified_circle =
        Some(SpecifiedCircleRefinements::One(SpecifiedCircleRefinement {
            lon: 114.0,
            lat: 22.0,
            radius_km: 200.0,
        }));
    let lowered = project.lower();
    assert_eq!(
        lowered.refine.mask_refine_spc_fprefix, "inline:circle:lon=114,lat=22,radius_km=200",
        "the one-circle form must stay byte-identical"
    );
    assert_eq!(lowered.refine.mask_refine_spc_type, "circle");
    assert!(lowered.refine.refine_spc);
}

#[test]
fn a_circle_chain_lowers_to_the_chain_form() {
    // What reducing a coastline to point+radius demand actually produces.
    let mut project = sample();
    project.refinement.specified_circle = Some(SpecifiedCircleRefinements::Many(vec![
        SpecifiedCircleRefinement {
            lon: 114.0,
            lat: 22.0,
            radius_km: 200.0,
        },
        SpecifiedCircleRefinement {
            lon: 115.5,
            lat: 22.5,
            radius_km: 200.0,
        },
    ]));
    let lowered = project.lower();
    assert_eq!(
        lowered.refine.mask_refine_spc_fprefix,
        "inline:circles:lon=114,lat=22,radius_km=200;lon=115.5,lat=22.5,radius_km=200"
    );
}

#[test]
fn an_empty_circle_chain_is_rejected() {
    let mut project = sample();
    project.refinement.specified_circle = Some(SpecifiedCircleRefinements::Many(Vec::new()));
    let error = project.try_lower().expect_err("empty chain must not lower");
    assert!(error.contains("at least one circle"), "got {error}");
}

#[test]
fn baseline_grid_without_refinement_omits_mkrefine() {
    // A baseline grid (refine off) must omit &mkrefine — the engine would
    // validate it and reject the hex+no-Istransition / no-refine_spc combo.
    let mut lowered = sample().lower();
    lowered.mkgrd.refine = false;
    let nml = lowered.to_namelist();
    assert!(nml.contains("&mkgrd"), "{nml}");
    assert!(
        !nml.contains("&mkrefine"),
        "baseline grid must omit &mkrefine:\n{nml}"
    );
}

#[test]
fn every_preset_scaffolds_the_full_disabled_threshold_catalog() {
    use ProjectLayerRole::{Cama, LandType, MeritHydro};

    let recommended_roles: &[(MeshIntentPreset, &[ProjectLayerRole])] = &[
        (MeshIntentPreset::Custom, &[LandType]),
        (MeshIntentPreset::HydrologyLand, &[LandType, MeritHydro]),
        (MeshIntentPreset::CarbonLand, &[LandType]),
        (MeshIntentPreset::SnowPermafrostLand, &[LandType]),
        (MeshIntentPreset::UrbanLand, &[LandType]),
        (MeshIntentPreset::CoastalOcean, &[LandType]),
        (MeshIntentPreset::Estuary, &[LandType, Cama]),
        (MeshIntentPreset::RiverNetwork, &[LandType, MeritHydro]),
        (MeshIntentPreset::MeritHydroCoast, &[LandType, MeritHydro]),
        (MeshIntentPreset::LandOceanCoupled, &[LandType]),
        (MeshIntentPreset::AtmosphereMpas, &[LandType]),
        (MeshIntentPreset::MultiObjectiveBalanced, &[LandType]),
    ];
    let expected_fields = criterion_catalog()
        .iter()
        .map(|criterion| criterion.field)
        .collect::<Vec<_>>();

    assert_eq!(recommended_roles.len(), MeshIntentPreset::all().len());
    for &(intent, extra_roles) in recommended_roles {
        let defaults = intent.defaults();
        assert_eq!(
            defaults.criteria,
            expected_fields,
            "{} criteria",
            intent.id()
        );
        assert_eq!(defaults.extra_roles, extra_roles, "{} roles", intent.id());

        let project = ProjectConfig::scaffold(
            intent.id(),
            intent,
            DomainConfig::Global,
            ResolutionSpec::Nxp(80),
        );
        let thresholds = project
            .data_layers
            .iter()
            .filter(|layer| matches!(layer.role, ProjectLayerRole::Threshold(_)))
            .collect::<Vec<_>>();
        assert_eq!(
            thresholds
                .iter()
                .map(|layer| match layer.role {
                    ProjectLayerRole::Threshold(field) => field,
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>(),
            expected_fields,
            "{}",
            intent.id()
        );
        assert!(thresholds.iter().all(|layer| layer.path.is_empty()
            && !layer.enabled
            && layer.threshold_value.is_none()));
        for role in [LandType, MeritHydro, Cama] {
            let layer = project
                .data_layers
                .iter()
                .find(|layer| layer.role == role)
                .unwrap_or_else(|| panic!("{} missing {role:?}", intent.id()));
            let was_recommended = extra_roles.contains(&role);
            if role == LandType && was_recommended {
                assert_eq!(layer.path, "input/landtype_igbp_update.nc");
                assert!(layer.enabled);
            } else {
                assert!(layer.path.is_empty());
                assert!(!layer.enabled);
            }
        }
        assert!(!project.refinement.enabled);
        assert_eq!(project.refinement.max_passes, 0);
    }

    let atmosphere = MeshIntentPreset::AtmosphereMpas.defaults();
    assert_eq!(atmosphere.kind, MeshDomainKind::Atmosphere);
    assert_eq!(atmosphere.model_format, ModelFormat::Mpas);
    let ocean = MeshIntentPreset::CoastalOcean.defaults();
    assert_eq!(ocean.kind, MeshDomainKind::Ocean);
    assert_eq!(ocean.cell, MeshCellKind::Tri);
}

#[test]
fn intent_catalog_is_single_source_for_gui_labels() {
    let presets = MeshIntentPreset::all();
    assert_eq!(presets.len(), 12);
    assert_eq!(presets[0], MeshIntentPreset::Custom);

    let mut ids: Vec<&str> = presets.iter().map(|preset| preset.id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), presets.len(), "intent ids are unique");

    let river = MeshIntentPreset::from_id("RiverNetwork").expect("RiverNetwork id");
    assert_eq!(river.defaults().kind, MeshDomainKind::Land);
    assert_eq!(river.label(), "Land · River network");
    assert_eq!(MeshIntentPreset::Custom.label(), "Land · Basic");
    assert_eq!(
        MeshIntentPreset::Custom.defaults().kind,
        MeshDomainKind::Land
    );

    assert_eq!(
        MeshIntentPreset::from_id(DEPRECATED_ATMOSPHERE_TYPHOON_INTENT_ID),
        Some(MeshIntentPreset::AtmosphereMpas)
    );

    let project = ProjectConfig::scaffold(
        "deprecated_atmosphere_alias",
        MeshIntentPreset::AtmosphereMpas,
        DomainConfig::Global,
        ResolutionSpec::Nxp(40),
    );
    let deprecated_yaml = project
        .to_yaml()
        .expect("serialize atmosphere project")
        .replace("AtmosphereMpas", DEPRECATED_ATMOSPHERE_TYPHOON_INTENT_ID);
    let parsed =
        ProjectConfig::from_yaml(&deprecated_yaml).expect("deprecated atmosphere intent alias");
    assert_eq!(parsed.target.intent, MeshIntentPreset::AtmosphereMpas);
}

#[test]
fn layer_role_labels_are_schema_owned() {
    assert_eq!(ProjectLayerRole::LandType.label(), "land type");
    assert_eq!(ProjectLayerRole::MeritHydro.label(), "MERIT-Hydro");
    assert_eq!(
        ProjectLayerRole::Threshold(ThresholdField::Slope).label(),
        "threshold · Slope"
    );
    assert!(!ProjectLayerRole::LandType.wants_folder());
    assert!(ProjectLayerRole::MeritHydro.wants_folder());
    assert!(ProjectLayerRole::Cama.wants_folder());
    assert!(!ProjectLayerRole::Threshold(ThresholdField::Slope).wants_folder());
}

#[test]
fn quality_warning_and_harp_transaction_floor_have_independent_defaults() {
    assert_eq!(
        QualityConfig::default().min_angle_deg,
        DEFAULT_MIN_ANGLE_DEG
    );
    assert_eq!(
        MeshIntentPreset::HydrologyLand.defaults().min_angle_deg,
        DEFAULT_MIN_ANGLE_DEG
    );
    assert_eq!(
        HarpDvRefinementRecipe::default().minimum_triangle_angle_deg,
        0.0,
        "HARP-DV reports the warning but does not enforce it by default"
    );
}

#[test]
fn quality_auto_refine_policy_lowers_to_namelist() {
    let mut p = sample();
    p.quality.on_violation = ViolationPolicy::AutoRefine;

    let lowered = p.lower();

    assert_eq!(lowered.quality.on_violation, "auto_refine");
    assert_eq!(
        lowered.quality.repair_batch_limit,
        p.quality.auto_refine_batch_cells as i32
    );
    assert!(lowered
        .to_namelist()
        .contains("NL%on_violation = 'auto_refine'"));
}

#[test]
fn quality_auto_refine_accepts_global_regional_and_uniform_baselines() {
    let mut p = sample();
    p.quality.on_violation = ViolationPolicy::AutoRefine;
    p.domain = DomainConfig::Global;
    assert!(p.validate().is_ok());

    p.domain = sample().domain;
    p.refinement.enabled = false;
    p.refinement.max_passes = 0;
    p.refinement.specified_circle = None;
    p.refinement.specified_bbox = None;
    p.refinement.specified_close = None;
    assert!(p.validate().is_ok());
}

#[test]
fn scaffold_builds_lowerable_project() {
    let p = ProjectConfig::scaffold(
        "hydro_test",
        MeshIntentPreset::HydrologyLand,
        DomainConfig::Global,
        ResolutionSpec::Nxp(40),
    );
    assert_eq!(p.target.kind, MeshDomainKind::Land);
    assert_eq!(p.target.intent, MeshIntentPreset::HydrologyLand);
    // landcover is a default relative input; other optional layers stay disabled.
    let landcover = p
        .data_layers
        .iter()
        .find(|l| l.id == "landcover")
        .expect("landcover layer");
    assert!(landcover.enabled);
    assert_eq!(landcover.path, "input/landtype_igbp_update.nc");
    assert!(p.data_layers.iter().any(|l| l.id == "slope_avg"));
    assert!(p.data_layers.iter().any(|l| l.id == "dem"));
    assert!(p.data_layers.iter().any(|l| l.id == "slope_max"));
    assert!(p.data_layers.iter().any(|l| l.id == "k_solids"));
    assert!(p.data_layers.iter().any(|l| l.id == "tksatu"));
    assert!(p
        .data_layers
        .iter()
        .filter(|l| l.id != "landcover")
        .all(|l| !l.enabled));

    // round-trips through yaml, and lowers to engine config
    let back = yaml_round_trip(&p);
    assert_eq!(p, back);
    assert_eq!(p.lower().mkgrd.mesh_type, "landmesh");

    let ocean = ProjectConfig::scaffold(
        "ocean_test",
        MeshIntentPreset::CoastalOcean,
        DomainConfig::Global,
        ResolutionSpec::Nxp(40),
    );
    let lowered = ocean.lower();
    assert!(!lowered.refine.refine_num_landtypes);
    assert!(!lowered.mkgrd.refine);
}

#[test]
fn lowered_project_uses_runnable_file_defaults() {
    let project = ProjectConfig::scaffold(
        "runnable",
        MeshIntentPreset::AtmosphereMpas,
        DomainConfig::Global,
        ResolutionSpec::Nxp(16),
    );
    let lowered = project.lower();
    assert_eq!(lowered.mkgrd.base_dir, "./");
    assert_eq!(lowered.mkgrd.mode_file, "none");
    assert_eq!(lowered.mkgrd.mode_file_description, "none");
    assert_eq!(lowered.mkgrd.landtype_file, "none");
}

#[test]
fn criterion_catalog_is_unique_and_self_describing() {
    let cat = criterion_catalog();
    for c in cat {
        assert!(
            !c.physical_process.is_empty(),
            "{} missing physical_process",
            c.id
        );
        assert!(!c.gui.label.is_empty());
        assert!(c.gui.range.0 <= c.gui.range.1);
    }
    let mut ids: Vec<&str> = cat.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 14, "criterion ids are unique");
}

#[test]
fn threshold_criterion_catalog_expands_each_source_into_named_mean_and_std_axes() {
    let catalog = threshold_criterion_catalog();
    assert_eq!(catalog.len(), criterion_catalog().len() * 2);
    let lai_mean = catalog
        .iter()
        .find(|criterion| criterion.id == "lai_mean")
        .expect("lai mean criterion");
    assert_eq!(lai_mean.label, "LAI mean");
    assert_eq!(lai_mean.source_field, ThresholdField::Lai);
    assert_eq!(lai_mean.statistic, ThresholdStatistic::Mean);
    let lai_std = threshold_criterion_by_id("lai_std").expect("lai std criterion");
    assert_eq!(lai_std.label, "LAI std");
    assert_eq!(lai_std.source_field, ThresholdField::Lai);
    assert_eq!(lai_std.statistic, ThresholdStatistic::Std);
}

#[test]
fn criterion_lookup_and_universal_domain_catalog() {
    let slope = criterion_by_id("slope").expect("slope criterion");
    assert_eq!(slope.field, ThresholdField::Slope);
    assert_eq!(
        criterion_by_id("dem").expect("dem criterion").field,
        ThresholdField::Dem
    );
    assert_eq!(
        criterion_by_id("slope_max")
            .expect("slope_max criterion")
            .field,
        ThresholdField::SlopeMax
    );
    assert!(criterion_by_id("nope").is_none());

    let expected = criterion_catalog()
        .iter()
        .map(|criterion| criterion.id)
        .collect::<Vec<_>>();
    for kind in [
        MeshDomainKind::Land,
        MeshDomainKind::Ocean,
        MeshDomainKind::Atmosphere,
        MeshDomainKind::Coupled,
        MeshDomainKind::Earth,
    ] {
        assert_eq!(
            criteria_for_domain(kind)
                .into_iter()
                .map(|criterion| criterion.id)
                .collect::<Vec<_>>(),
            expected,
            "{kind:?}"
        );
        assert!(criterion_catalog()
            .iter()
            .all(|criterion| criterion.applicable.contains(&kind)));
    }

    let layer = slope.to_data_layer("./th/slope_avg.nc", true);
    assert_eq!(
        layer.role,
        ProjectLayerRole::Threshold(ThresholdField::Slope)
    );
    assert_eq!(layer.id, "slope_avg");
    assert!(layer.enabled);
}

#[test]
fn earth_target_accepts_ocean_and_atmos_threshold_layers() {
    let mut p = sample();
    p.target.kind = MeshDomainKind::Earth;
    p.target.model_format = ModelFormat::CoLM;
    p.data_layers = vec![
        ProjectDataLayer {
            id: "sea_slope".into(),
            role: ProjectLayerRole::Threshold(ThresholdField::SeaSlope),
            path: "./th/sea_slope.nc".into(),
            enabled: true,
            threshold_value: Some(3.0),
        },
        ProjectDataLayer {
            id: "typhoon".into(),
            role: ProjectLayerRole::Threshold(ThresholdField::Typhoon),
            path: "./th/typhoon.nc".into(),
            enabled: true,
            threshold_value: Some(0.5),
        },
    ];

    let lowered = p
        .try_lower()
        .expect("earth target should accept coupled threshold axes");
    assert!(lowered.refine.refine_cal);
    assert_eq!(lowered.mkgrd.mesh_type, "earthmesh");
    assert_eq!(lowered.refine.th_onelayer_ocn[6], 3.0);
    assert_eq!(lowered.refine.th_onelayer_atmos[0], 0.5);
}

#[test]
fn km_resolution_and_coupling_round_trip() {
    assert_eq!(km_to_nxp(100.0), 80);
    assert!((nxp_to_km(72) - 111.20).abs() < 0.01);
    assert_eq!(km_to_nxp(0.0), 1); // guard
    assert!(km_to_nxp(500.0) >= 1 && km_to_nxp(500.0) < 80);

    let mut p = sample();
    p.target.resolution = ResolutionSpec::ApproxKm(100.0);
    p.coupling = Some(CoupledMeshConfig {
        fraction_method: FractionMethod::PointSample,
        identify_coastline: false,
        identify_river_mouth: false,
        cama_root: None,
    });
    let lowered = p.try_lower().expect("point-sample coupling should lower");
    assert_eq!(lowered.mkgrd.mesh_type, "LOCmesh");
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
    assert_eq!(lowered.mkgrd.output_format, "CoLM");

    let back = yaml_round_trip(&p);
    assert_eq!(p, back);
    assert_eq!(p.lower().mkgrd.nxp, 81);

    p.target.resolution =
        ResolutionSpec::ApproxDegree(100.0 / earthmesh_core::KM_PER_DEGREE_EQUATOR);
    assert_eq!(p.lower().mkgrd.nxp, 81);
}

#[test]
fn method_c_local_refinement_rounds_nxp_up_to_stride_three() {
    let mut project = sample();
    project.target.resolution = ResolutionSpec::Nxp(80);
    assert_eq!(project.lower().mkgrd.nxp, 81);

    project.target.resolution = ResolutionSpec::Nxp(81);
    assert_eq!(project.lower().mkgrd.nxp, 81);

    project.target.resolution = ResolutionSpec::Nxp(82);
    assert_eq!(project.lower().mkgrd.nxp, 84);

    project.refinement.enabled = false;
    project.refinement.threshold_enabled = false;
    project.refinement.max_passes = 0;
    project.quality.on_violation = ViolationPolicy::Warn;
    project.target.resolution = ResolutionSpec::Nxp(80);
    assert_eq!(project.lower().mkgrd.nxp, 80);

    project.quality.on_violation = ViolationPolicy::AutoRefine;
    assert_eq!(project.lower().mkgrd.nxp, 81);

    project.refinement.enabled = true;
    project.refinement.threshold_enabled = true;
    project.refinement.max_passes = 3;
    project.refinement.method_c.algorithm = crate::MethodCAlgorithm::LeppDelaunay;
    assert_eq!(project.lower().mkgrd.nxp, 81);

    project.quality.on_violation = ViolationPolicy::Warn;
    assert_eq!(
        project.lower().mkgrd.nxp,
        80,
        "LEPP without canonical quality repair bypasses Method-C's stride-three lattice"
    );

    project.refinement.method_c.algorithm = crate::MethodCAlgorithm::Canonical;
    project.refinement.backend = crate::RefinementBackend::HarpDv;
    project.quality.on_violation = ViolationPolicy::AutoRefine;
    assert_eq!(
        project.lower().mkgrd.nxp,
        80,
        "HARP-DV owns its quality repair and must not inherit Method-C's stride-three lattice"
    );
}

#[test]
fn hydro_only_local_refinement_rounds_parent_nxp_to_stride_three() {
    let mut project = sample();
    project.target.resolution = ResolutionSpec::Nxp(80);
    project.quality.on_violation = ViolationPolicy::Warn;
    project.refinement.backend = crate::RefinementBackend::HarpDv;
    project.data_layers = vec![
        ProjectDataLayer {
            id: "merit".into(),
            role: ProjectLayerRole::MeritHydro,
            path: "./merit".into(),
            enabled: true,
            threshold_value: None,
        },
        ProjectDataLayer {
            id: "landcover".into(),
            role: ProjectLayerRole::LandType,
            path: "./in/landtype.nc".into(),
            enabled: true,
            threshold_value: None,
        },
    ];
    project.hydro_coast = Some(HydroCoastConfig {
        merit_root: "./merit".into(),
        cama_root: None,
        merit_stride: 1,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
        r3_upa_km2: 50_000.0,
        r2_upa_km2: 5_000.0,
        river_refinement_enabled: true,
        river_width_refinement_enabled: true,
        river_upstream_area_refinement_enabled: false,
        river_width_threshold_m: Some(300.0),
        river_upstream_area_threshold_km2: None,
        coast_refinement_enabled: false,
        coast_buffer_km: 0.0,
        coast_land_refinement_enabled: true,
        coast_ocean_refinement_enabled: true,
    });

    let lowered = project.try_lower().expect("hydro-only Project lowering");
    assert!(!lowered.mkgrd.refine, "hydro executes after the base mesh");
    assert!(
        lowered.hfield.is_none(),
        "no generic refinement source exists"
    );
    assert_eq!(
        lowered.mkgrd.nxp, 81,
        "the later hydro HField adapter still needs a stride-compatible parent"
    );
}

#[test]
fn coupling_config_lowers_overlay_and_feature_detection_options() {
    let mut p = sample();
    p.coupling = Some(CoupledMeshConfig {
        fraction_method: FractionMethod::ConservativeOverlay,
        identify_coastline: false,
        identify_river_mouth: false,
        cama_root: None,
    });
    let lowered = p
        .try_lower()
        .expect("conservative overlay should lower into the production coupling path");
    assert_eq!(
        lowered.mkgrd.coupling_fraction_method,
        "conservative_overlay"
    );

    p.coupling = Some(CoupledMeshConfig {
        fraction_method: FractionMethod::PointSample,
        identify_coastline: true,
        identify_river_mouth: false,
        cama_root: None,
    });
    let lowered = p
        .try_lower()
        .expect("coastline identification should lower into the production coupling path");
    assert!(lowered.mkgrd.coupling_identify_coastline);

    p.coupling = Some(CoupledMeshConfig {
        fraction_method: FractionMethod::PointSample,
        identify_coastline: true,
        identify_river_mouth: true,
        cama_root: None,
    });
    assert!(p.try_lower().unwrap_err().contains("cama_root"));
    p.coupling.as_mut().unwrap().cama_root = Some("/data/cama".into());
    let lowered = p
        .try_lower()
        .expect("river-mouth identification should lower when CaMa data is configured");
    assert!(lowered.mkgrd.coupling_identify_river_mouth);
    assert_eq!(lowered.mkgrd.coupling_cama_root, "/data/cama");

    p.coupling = Some(CoupledMeshConfig::default());
    p.data_layers
        .retain(|layer| layer.role != ProjectLayerRole::LandType);
    assert!(p.try_lower().unwrap_err().contains("landtype layer"));
}

#[test]
fn project_rejects_unsupported_schema_versions_and_unknown_fields() {
    let mut p = sample();
    p.schema_version = "4.0.0".into();
    assert!(p
        .validate()
        .unwrap_err()
        .contains("unsupported project schema_version"));

    let yaml = sample().to_yaml().expect("sample yaml");
    let unknown_top_level = format!("{yaml}future_engine: enabled\n");
    assert!(ProjectConfig::from_yaml(&unknown_top_level)
        .unwrap_err()
        .contains("unknown field"));

    let unknown_nested = yaml.replacen(
        "  description:",
        "  future_metadata: preserved\n  description:",
        1,
    );
    assert!(ProjectConfig::from_yaml(&unknown_nested)
        .unwrap_err()
        .contains("unknown field"));
}

#[test]
fn regional_bbox_hydro_coast_builds_an_explicit_postprocess_plan() {
    let mut p = sample();
    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        merit_stride: 1,
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
    let plan = p.hydro_execution_plan().unwrap().unwrap();
    assert_eq!(plan.merit_root, "/data/merit");
    assert_eq!(plan.r2_upa_km2, 5_000.0);
    assert_eq!(plan.r3_upa_km2, 50_000.0);
    assert!(plan.river_refinement_enabled);
    assert!(plan.river_width_refinement_enabled);
    assert!(plan.river_upstream_area_refinement_enabled);
    assert_eq!(plan.river_width_threshold_m, 300.0);
    assert_eq!(plan.river_upstream_area_threshold_km2, 50_000.0);
    assert!(plan.coast_refinement_enabled);
    assert_eq!(plan.coast_buffer_km, 50.0);
    assert!(plan.coast_land_refinement_enabled);
    assert!(plan.coast_ocean_refinement_enabled);
    assert_eq!(
        plan.domain,
        RegionShape::Bbox {
            w: 112.0,
            e: 115.5,
            s: 21.5,
            n: 23.5,
        }
    );
    assert_eq!(
        plan.include_classes,
        vec!["R2", "R3", "COAST_LAND", "COAST_OCEAN"]
    );
    assert_eq!(plan.max_level, 3);
    p.hydro_coast
        .as_mut()
        .unwrap()
        .river_width_refinement_enabled = false;
    let upstream_only = p.hydro_execution_plan().unwrap().unwrap();
    assert!(!upstream_only.river_width_refinement_enabled);
    assert!(upstream_only.river_upstream_area_refinement_enabled);
    assert_eq!(upstream_only.max_level, 3);
    p.hydro_coast
        .as_mut()
        .unwrap()
        .river_upstream_area_refinement_enabled = false;
    p.hydro_coast.as_mut().unwrap().coast_refinement_enabled = false;
    assert_eq!(p.hydro_execution_plan().unwrap().unwrap().max_level, 0);
    p.hydro_coast
        .as_mut()
        .unwrap()
        .river_width_refinement_enabled = true;
    p.hydro_coast
        .as_mut()
        .unwrap()
        .river_upstream_area_refinement_enabled = true;
    p.hydro_coast.as_mut().unwrap().river_refinement_enabled = false;
    p.hydro_coast.as_mut().unwrap().coast_refinement_enabled = false;
    assert_eq!(p.hydro_execution_plan().unwrap().unwrap().max_level, 0);
    p.hydro_coast.as_mut().unwrap().coast_refinement_enabled = true;
    p.hydro_coast
        .as_mut()
        .unwrap()
        .coast_land_refinement_enabled = false;
    p.hydro_coast
        .as_mut()
        .unwrap()
        .coast_ocean_refinement_enabled = false;
    assert_eq!(p.hydro_execution_plan().unwrap().unwrap().max_level, 0);
    p.hydro_coast.as_mut().unwrap().river_refinement_enabled = true;
    p.refinement.enabled = false;
    p.refinement.max_passes = 0;
    assert_eq!(p.hydro_execution_plan().unwrap().unwrap().max_level, 0);
    p.refinement.enabled = true;
    p.refinement.max_passes = 3;
    p.refinement.threshold_enabled = false;
    assert_eq!(p.hydro_execution_plan().unwrap().unwrap().max_level, 0);
    p.refinement.threshold_enabled = true;
    p.target.resolution = ResolutionSpec::ApproxDegree(1.0);
    assert_eq!(
        p.hydro_execution_plan().unwrap().unwrap().target_dx_km,
        earthmesh_core::KM_PER_DEGREE_EQUATOR
    );
    p.hydro_coast.as_mut().unwrap().merit_stride = 2;
    assert!(p
        .validate()
        .unwrap_err()
        .contains("physical coast adjacency"));
    p.hydro_coast.as_mut().unwrap().merit_stride = 1;
    p.refinement.max_passes = 6;
    assert_eq!(
        p.hydro_execution_plan().unwrap().unwrap().max_level,
        METHOD_C_MAX_AUTO_REFINE_LEVEL
    );
    p.refinement.max_passes = METHOD_C_MAX_AUTO_REFINE_LEVEL;
    p.try_lower().expect("hydro routing is a post-mesh stage");
    assert_eq!(
        project_hydro_output_dir("/runs/gba/output/gridfile.nc4"),
        std::path::Path::new("/runs/gba/output/hydro_project")
    );
}

#[test]
fn hydro_plan_accepts_cama_antimeridian_and_non_bbox_regional_domains() {
    let mut p = sample();
    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: Some("/data/cama".into()),
        merit_stride: 1,
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
    let plan = p.hydro_execution_plan().unwrap().unwrap();
    assert_eq!(plan.cama_root.as_deref(), Some("/data/cama"));
    p.domain = DomainConfig::Global;
    assert!(p
        .validate()
        .unwrap_err()
        .contains("requires a regional domain"));

    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 170.0,
            e: -170.0,
            s: -10.0,
            n: 10.0,
        },
        sea_ratio: None,
    };
    p.validate().expect("antimeridian bbox should route");
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Circle {
            lon: 179.0,
            lat: 0.0,
            radius_km: 500.0,
        },
        sea_ratio: None,
    };
    p.validate().expect("circle should route");
    if let DomainConfig::Regional {
        shape: RegionShape::Circle { radius_km, .. },
        ..
    } = &mut p.domain
    {
        *radius_km = 11_000.0;
    }
    assert!(p
        .validate()
        .unwrap_err()
        .contains("minor-hemisphere domains"));
}

#[test]
fn auto_refine_progression_is_bounded_by_target_resolution() {
    assert_eq!(auto_refine_level_cap(40), 2);
    assert_eq!(next_auto_refine_pass(1, 40), Some(2));
    assert_eq!(next_auto_refine_pass(2, 40), None);
    assert_eq!(effective_auto_refine_pass(3, 40), 2);
    assert_eq!(effective_auto_refine_pass(0, 40), 1);
    assert_eq!(auto_refine_level_cap(768), 5);
}

#[test]
fn project_declares_only_topology_expectations_known_before_masking() {
    let mut project = sample();
    assert_eq!(project.expected_euler_characteristic(), None);

    project.domain = DomainConfig::Global;
    project.target.kind = MeshDomainKind::Earth;
    assert_eq!(project.expected_euler_characteristic(), Some(2));
    project.target.kind = MeshDomainKind::Atmosphere;
    assert_eq!(project.expected_euler_characteristic(), Some(2));

    project.target.kind = MeshDomainKind::Land;
    assert_eq!(project.expected_euler_characteristic(), None);
    project.target.kind = MeshDomainKind::Ocean;
    assert_eq!(project.expected_euler_characteristic(), None);
    project.target.kind = MeshDomainKind::Coupled;
    assert_eq!(project.expected_euler_characteristic(), None);
}

#[test]
fn auto_refine_state_machine_covers_pass_retry_cap_and_engine_failure() {
    let mut uniform = AutoRefineState::new(0, 40);
    assert_eq!(uniform.current_pass(), 0);
    assert_eq!(
        uniform.transition(AutoRefineEvent::QualityViolation),
        AutoRefineAction::Retry { next_pass: 1 }
    );

    let mut passed = AutoRefineState::new(1, 40);
    assert_eq!(
        passed.transition(AutoRefineEvent::QualityPassed),
        AutoRefineAction::Complete { pass: 1 }
    );

    let mut retry = AutoRefineState::new(1, 40);
    assert_eq!(
        retry.transition(AutoRefineEvent::QualityViolation),
        AutoRefineAction::Retry { next_pass: 2 }
    );
    assert_eq!(retry.current_pass(), 2);
    assert_eq!(
        retry.transition(AutoRefineEvent::QualityViolation),
        AutoRefineAction::CapReached { pass: 2, cap: 2 }
    );

    let mut failed = AutoRefineState::new(1, 40);
    assert_eq!(
        failed.transition(AutoRefineEvent::EngineFailed("engine exploded".into())),
        AutoRefineAction::AbortEngine {
            pass: 1,
            message: "engine exploded".into(),
        }
    );
    assert_eq!(failed.current_pass(), 1);
}

#[test]
fn coupling_cama_root_round_trips_with_compatibility_alias() {
    let yaml = "fraction_method: PointSample\nidentify_coastline: false\nidentify_river_mouth: true\nriver_mouth_cama_root: /data/cama\n";
    let parsed: CoupledMeshConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(parsed.cama_root.as_deref(), Some("/data/cama"));
}

#[test]
fn a_backend_that_cannot_serve_the_h_field_is_refused_at_validation() {
    // The run refuses this at the dispatch, but a project is edited and saved
    // long before it is run. Without the refusal here, the GUI would happily
    // save a project whose only symptom is a run that dies -- and before the
    // dispatch grew its guard, harp_dv took the pair and produced 6450 cells
    // having never read the field.
    let mut p = sample();
    p.refinement.hfield = Some(crate::HfieldRefinementRecipe {
        enabled: true,
        ..Default::default()
    });

    p.refinement.backend = crate::RefinementBackend::MethodC;
    p.validate().expect("Method-C serves the h-field");

    p.refinement.method_c.algorithm = crate::MethodCAlgorithm::LeppDelaunay;
    let error = p.validate().expect_err("LEPP does not read the h-field");
    assert!(
        error.contains("does not consume the gradient-limited h-field"),
        "{error}"
    );
    p.refinement.method_c.algorithm = crate::MethodCAlgorithm::Canonical;

    for (backend, name) in [
        (crate::RefinementBackend::RedGreen, "red_green"),
        (crate::RefinementBackend::HarpDv, "harp_dv"),
        (crate::RefinementBackend::Certified, "certified"),
    ] {
        p.refinement.backend = backend;
        let error = p.validate().expect_err("refused");
        assert!(error.contains(name), "{error}");
        assert!(error.contains("h-field"), "{error}");
    }

    // Turning the h-field off is what makes the project runnable again, and the
    // refusal must not outlive the thing it objects to.
    p.refinement.hfield = None;
    p.validate().expect("no h-field, no objection");
}

#[test]
fn lepp_post_quality_is_explicit_global_method_c_configuration() {
    let mut project = sample();
    project.domain = DomainConfig::Global;
    project.quality.lepp_post_quality = Some(crate::LeppPostQualityConfig {
        maximum_insertions: 12,
        maximum_edge_km: Some(75.0),
    });

    let lowered = project.try_lower().expect("global Method-C post-quality");
    assert!(lowered.quality.lepp_post_quality);
    assert_eq!(lowered.quality.lepp_post_quality_max_insertions, 12);
    assert_eq!(lowered.quality.lepp_post_quality_max_edge_km, 75.0);
    let reparsed = earthmesh_core::QualityNamelist::from_quality_namelist(&lowered.to_namelist())
        .expect("lowered quality block");
    assert_eq!(reparsed, lowered.quality);

    project.refinement.backend = crate::RefinementBackend::RedGreen;
    assert!(project
        .validate()
        .expect_err("backend mismatch")
        .contains("requires refinement.backend=method_c"));

    project.refinement.backend = crate::RefinementBackend::MethodC;
    project.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 112.0,
            e: 115.0,
            s: 21.0,
            n: 24.0,
        },
        sea_ratio: None,
    };
    assert!(project
        .validate()
        .expect_err("boundary phase is later")
        .contains("requires a global closed domain"));
}

#[test]
fn lepp_post_quality_limits_are_validated() {
    let mut project = sample();
    project.domain = DomainConfig::Global;
    project.quality.lepp_post_quality = Some(crate::LeppPostQualityConfig {
        maximum_insertions: 0,
        maximum_edge_km: None,
    });
    assert!(project
        .validate()
        .expect_err("zero limit")
        .contains("maximum_insertions must be > 0"));

    project.quality.lepp_post_quality = Some(crate::LeppPostQualityConfig {
        maximum_insertions: 1,
        maximum_edge_km: Some(f64::NAN),
    });
    assert!(project
        .validate()
        .expect_err("non-finite target")
        .contains("maximum_edge_km must be positive"));
}

#[test]
fn method_c_lepp_algorithm_lowers_explicit_bounded_config() {
    let mut project = sample();
    project.refinement.method_c = crate::MethodCRefinementRecipe {
        algorithm: crate::MethodCAlgorithm::LeppDelaunay,
        max_cycles: 6,
        maximum_insertions_per_cycle: 123,
        minimum_triangle_angle_deg: 20.0,
        ..Default::default()
    };

    let lowered = project.try_lower().expect("Method-C LEPP project");
    let namelist = lowered.to_namelist();
    assert!(namelist.contains("&method_c"));
    assert!(namelist.contains("NL%algorithm = 'lepp_delaunay'"));
    assert!(namelist.contains("NL%max_cycles = 6"));
    assert!(namelist.contains("NL%maximum_insertions_per_cycle = 123"));
    assert!(namelist.contains("NL%minimum_triangle_angle_deg = 20"));

    project.refinement.backend = crate::RefinementBackend::RedGreen;
    assert!(project
        .validate()
        .expect_err("backend mismatch")
        .contains("requires refinement.backend=method_c"));
}

#[test]
fn method_c_lepp_algorithm_rejects_invalid_limits_and_post_quality_composition() {
    let mut project = sample();
    project.refinement.method_c.algorithm = crate::MethodCAlgorithm::LeppDelaunay;
    project.refinement.method_c.max_cycles = 0;
    assert!(project
        .validate()
        .expect_err("zero cycles")
        .contains("max_cycles must be > 0"));

    project.refinement.method_c.max_cycles = 1;
    project.domain = DomainConfig::Global;
    project.quality.lepp_post_quality = Some(crate::LeppPostQualityConfig {
        maximum_insertions: 1,
        maximum_edge_km: None,
    });
    assert!(project
        .validate()
        .expect_err("two LEPP owners")
        .contains("cannot be combined"));
}

#[test]
fn harp_dv_algorithm_lowers_every_exposed_control() {
    let mut project = sample();
    project.refinement.backend = crate::RefinementBackend::HarpDv;
    project.refinement.harp_dv = crate::HarpDvRefinementRecipe {
        max_cycles: 3,
        minimum_cell_width_m: 2_000.0,
        maximum_cells: 9_000,
        maximum_patch_cells: 800,
        maximum_neighbor_scale_ratio: 1.5,
        minimum_candidate_separation_m: 2.0,
        maximum_vertex_degree: 6,
        minimum_triangle_angle_deg: 25.0,
        criterion_minimum_angle_deg: 10.0,
    };

    let namelist = project.try_lower().expect("HARP-DV project").to_namelist();
    assert!(namelist.contains("&harp_dv"));
    assert!(namelist.contains("NL%max_cycles = 3"));
    assert!(namelist.contains("NL%minimum_cell_width_m = 2000"));
    assert!(namelist.contains("NL%maximum_cells = 9000"));
    assert!(namelist.contains("NL%maximum_patch_cells = 800"));
    assert!(namelist.contains("NL%maximum_neighbor_scale_ratio = 1.5"));
    assert!(namelist.contains("NL%minimum_candidate_separation_m = 2"));
    assert!(namelist.contains("NL%maximum_vertex_degree = 6"));
    assert!(namelist.contains("NL%minimum_triangle_angle_deg = 25"));
    assert!(namelist.contains("RL%harp_min_angle_deg = 10"));

    project.refinement.harp_dv.maximum_patch_cells = 9_001;
    assert!(project
        .validate()
        .expect_err("patch budget exceeds mesh budget")
        .contains("maximum_patch_cells"));
}

#[test]
fn certified_algorithm_is_a_parallel_backend_and_lowers_its_strict_bounds() {
    let mut project = sample();
    project.refinement.backend = crate::RefinementBackend::Certified;
    project.refinement.certified = crate::CertifiedRefinementRecipe {
        mode: crate::CertifiedMode::ReverseCoarsening,
        delivery: crate::CertifiedDeliveryMode::Coupled,
        maximum_level: 4,
        maximum_cells: 900_000,
        gradation_rings_per_level: 5,
        search_budget: 12_000,
    };

    let yaml = project.to_yaml().expect("certified project yaml");
    let reparsed = ProjectConfig::from_yaml(&yaml).expect("certified yaml round trip");
    assert_eq!(
        reparsed.refinement.backend,
        crate::RefinementBackend::Certified
    );

    let namelist = reparsed.try_lower().expect("CMRC project").to_namelist();
    assert!(namelist.contains("NL%refine_backend = 'certified'"));
    assert!(namelist.contains("&certified"));
    assert!(namelist.contains("NL%mode = 'reverse_coarsening'"));
    assert!(namelist.contains("NL%delivery = 'coupled'"));
    assert!(namelist.contains("NL%maximum_level = 4"));
    assert!(namelist.contains("NL%maximum_cells = 900000"));
    assert!(namelist.contains("NL%gradation_rings_per_level = 5"));
    assert!(namelist.contains("NL%search_budget = 12000"));
    assert!(!namelist.contains("legacy"));
}
