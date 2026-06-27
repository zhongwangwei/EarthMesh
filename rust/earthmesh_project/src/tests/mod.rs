use super::*;
use std::{env, fs, process};

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
            },
            ProjectDataLayer {
                id: "lai".into(),
                role: ProjectLayerRole::Threshold(ThresholdField::Lai),
                path: "./th/lai.nc".into(),
                enabled: true,
            },
        ],
        refinement: RefinementRecipe {
            enabled: true,
            max_passes: 3,
        },
        quality: QualityConfig {
            min_angle_deg: 28.0,
            on_violation: ViolationPolicy::Block,
        },
        expert: ExpertOverrides {
            nxp: None,
            openmp: Some(8),
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
fn project_validation_rejects_invalid_regional_shapes() {
    let mut p = sample();
    p.domain = DomainConfig::Regional {
        shape: RegionShape::Bbox {
            w: 115.0,
            e: 112.0,
            n: 23.5,
            s: 21.5,
        },
        sea_ratio: None,
    };
    let err = yaml_err(&p);
    assert!(err.contains("bbox west must be < east"));

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
    assert!(err.contains("bbox west must be < east"));

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
}

#[test]
fn project_validation_rejects_empty_schema_version() {
    let mut p = sample();
    p.schema_version = " ".into();
    let err = json_err(&p);
    assert!(err.contains("project schema_version must not be empty"));
}

#[test]
fn project_validation_rejects_invalid_quality_gate() {
    let mut p = sample();
    p.quality.min_angle_deg = 0.0;
    let err = yaml_err(&p);
    assert!(err.contains("quality min_angle_deg must be > 0"));
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
    p.target.kind = MeshDomainKind::Land;
    p.data_layers.push(ProjectDataLayer {
        id: "sea_slope".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::SeaSlope),
        path: "./in/sea_slope.nc".into(),
        enabled: true,
    });
    let err = yaml_err(&p);
    assert!(err.contains("threshold layer 'sea_slope' is not applicable to Land targets"));
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

    p.target.resolution = ResolutionSpec::Nxp(40);
    p.expert.nxp = Some(0);
    let err = yaml_err(&p);
    assert!(err.contains("expert nxp override must be > 0"));

    p.expert.nxp = None;
    p.expert.openmp = Some(0);
    let err = yaml_err(&p);
    assert!(err.contains("expert openmp override must be > 0"));
}

#[test]
fn project_validation_rejects_engine_incompatible_target_format() {
    let mut p = sample();
    p.target.kind = MeshDomainKind::Atmosphere;
    p.target.model_format = ModelFormat::CoLM;
    let err = yaml_err(&p);
    assert!(err.contains("atmosphere target model_format must be MPAS or MPAS-Simple"));

    p.target.kind = MeshDomainKind::Ocean;
    p.target.model_format = ModelFormat::CoLM;
    let err = yaml_err(&p);
    assert!(err.contains("ocean target model_format must be FVCOM"));

    p.target.kind = MeshDomainKind::Coupled;
    p.target.model_format = ModelFormat::Fvcom;
    let err = json_err(&p);
    assert!(err.contains("coupled target model_format must be CoLM"));

    p.target.kind = MeshDomainKind::Land;
    p.target.model_format = ModelFormat::Olam;
    let err = yaml_err(&p);
    assert!(err.contains("project model_format OLAM is deprecated"));

    let err = p.try_lower().expect_err("deprecated OLAM must not lower");
    assert!(err.contains("project model_format OLAM is deprecated"));
}

#[test]
fn project_validation_rejects_invalid_refinement_passes() {
    let mut p = sample();
    p.refinement.max_passes = 0;
    let err = yaml_err(&p);
    assert!(err.contains("refinement max_passes must be > 0"));

    p.refinement.max_passes = 10;
    let err = json_err(&p);
    assert!(err.contains("refinement max_passes must be <= 9"));
}

#[test]
fn project_validation_rejects_invalid_hydro_coast_config() {
    let mut p = sample();
    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: " ".into(),
        cama_root: None,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast merit_root must not be empty"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: Some(" ".into()),
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast cama_root must not be empty"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        r3_width_m: 0.0,
        r2_width_m: 50.0,
    });
    let err = json_err(&p);
    assert!(err.contains("hydro_coast widths must be > 0"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        r3_width_m: 50.0,
        r2_width_m: 300.0,
    });
    let err = json_err(&p);
    assert!(err.contains("hydro_coast r3_width_m must be >= r2_width_m"));
}

#[test]
fn lower_maps_to_engine_config() {
    let lowered = sample().lower();
    assert_eq!(lowered.mkgrd.mesh_type, "LOCmesh");
    assert_eq!(lowered.mkgrd.mode_grid, "hex");
    assert_eq!(lowered.mkgrd.output_format, "CoLM");
    assert_eq!(lowered.mkgrd.nxp, 40);
    assert!(!lowered.mkgrd.mask_domain_global);
    assert_eq!(lowered.mkgrd.mask_domain_type, "bbox");
    assert_eq!(lowered.mkgrd.mask_sea_ratio, 0.25);
    assert_eq!(lowered.mkgrd.experiment_name, "gba");
    assert_eq!(lowered.mkgrd.openmp, 8); // expert override

    // landcover → landtype_file; lai → refine switch + refine_cal
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
    assert!(lowered.refine.refine_onelayer_lnd[0] && lowered.refine.refine_onelayer_lnd[1]);
    assert!(lowered.refine.refine_cal);
    assert_eq!(lowered.refine.max_iter_cal, 3);

    // quality
    assert_eq!(lowered.quality.min_angle_warn_deg, 28.0);
    assert_eq!(lowered.quality.on_violation, "block");

    // runnable namelist with all four blocks
    let nml = lowered.to_namelist();
    assert!(nml.contains("&mkgrd"));
    assert!(nml.contains("&mkrefine"));
    assert!(nml.contains("&quality"));
    assert!(nml.contains("&datalayers"));
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
fn preset_defaults_pick_sensible_criteria() {
    let h = MeshIntentPreset::HydrologyLand.defaults();
    assert_eq!(h.kind, MeshDomainKind::Land);
    assert!(h.criteria.contains(&ThresholdField::Slope));
    assert!(h.extra_roles.contains(&ProjectLayerRole::MeritHydro));

    let a = MeshIntentPreset::AtmosphereMpas.defaults();
    assert_eq!(a.kind, MeshDomainKind::Atmosphere);
    assert_eq!(a.model_format, ModelFormat::Mpas);
    assert!(a.criteria.is_empty());

    let p = ProjectConfig::scaffold(
        "atmosphere",
        MeshIntentPreset::AtmosphereMpas,
        DomainConfig::Global,
        ResolutionSpec::Nxp(40),
    );
    assert!(!p.refinement.enabled);
    assert_eq!(p.refinement.max_passes, 0);

    let c = MeshIntentPreset::CoastalOcean.defaults();
    assert_eq!(c.kind, MeshDomainKind::Ocean);
    assert_eq!(c.cell, MeshCellKind::Tri);
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
}

#[test]
fn quality_default_is_single_project_source() {
    assert_eq!(
        QualityConfig::default().min_angle_deg,
        DEFAULT_MIN_ANGLE_DEG
    );
    assert_eq!(
        MeshIntentPreset::HydrologyLand.defaults().min_angle_deg,
        DEFAULT_MIN_ANGLE_DEG
    );
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
    // landcover + merit + slope entries start disabled with no path.
    assert!(p.data_layers.iter().any(|l| l.id == "landcover"));
    assert!(p.data_layers.iter().any(|l| l.id == "slope_avg"));
    assert!(p.data_layers.iter().all(|l| !l.enabled));

    // round-trips through yaml, and lowers to engine config
    let back = yaml_round_trip(&p);
    assert_eq!(p, back);
    assert_eq!(p.lower().mkgrd.mesh_type, "landmesh");
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
    assert_eq!(ids.len(), 11, "criterion ids are unique");
}

#[test]
fn criterion_lookup_and_domain_filter() {
    let slope = criterion_by_id("slope").expect("slope criterion");
    assert_eq!(slope.field, ThresholdField::Slope);
    assert!(criterion_by_id("nope").is_none());

    let ocean = criteria_for_domain(MeshDomainKind::Ocean);
    assert!(ocean.iter().any(|c| c.id == "sea_slope"));
    assert!(ocean.iter().all(|c| c.id != "lai")); // land-only excluded

    let atmosphere = criteria_for_domain(MeshDomainKind::Atmosphere);
    assert!(atmosphere.is_empty());

    let layer = slope.to_data_layer("./th/slope_avg.nc", true);
    assert_eq!(
        layer.role,
        ProjectLayerRole::Threshold(ThresholdField::Slope)
    );
    assert_eq!(layer.id, "slope_avg");
    assert!(layer.enabled);
}

#[test]
fn km_resolution_and_hydro_coupling_round_trip() {
    assert_eq!(km_to_nxp(9.0), 40); // anchored on the GUI default
    assert_eq!(km_to_nxp(0.0), 1); // guard
    assert!(km_to_nxp(60.0) >= 1 && km_to_nxp(60.0) < 40);

    let mut p = sample();
    p.target.resolution = ResolutionSpec::ApproxKm(9.0);
    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: Some("/data/cama".into()),
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    p.coupling = Some(CoupledMeshConfig {
        fraction_method: FractionMethod::ConservativeOverlay,
        identify_coastline: true,
        identify_river_mouth: true,
    });

    let back = yaml_round_trip(&p);
    assert_eq!(p, back);
    assert_eq!(p.lower().mkgrd.nxp, 40); // ApproxKm(9) → NXP 40
}

#[test]
fn reproducibility_manifest_hashes_inputs() {
    let dir = env::temp_dir().join(format!("em_repro_{}", process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    let f = dir.join("lai.nc");
    fs::write(&f, b"hello").expect("write input");

    let mut p = sample();
    for l in p.data_layers.iter_mut() {
        if l.id == "lai" {
            l.path = f.display().to_string();
            l.enabled = true;
        }
    }
    let m = p.reproducibility_manifest();
    assert_eq!(m.schema_version, "3.0.0");
    assert!(!m.tool_version.is_empty());
    assert!(m.lowered_namelist.contains("&mkgrd"));

    let lai = m
        .inputs
        .iter()
        .find(|i| i.path.ends_with("lai.nc"))
        .expect("lai input hashed");
    assert_eq!(lai.bytes, 5);
    // sha256("hello")
    assert_eq!(
        lai.sha256,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    assert!(m.to_json().expect("json").contains("sha256"));

    let _ = fs::remove_dir_all(&dir);
}
