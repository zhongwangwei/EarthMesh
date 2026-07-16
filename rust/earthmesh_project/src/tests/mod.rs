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
            enabled: true,
            threshold_enabled: true,
            max_passes: 3,
            specified_circle: None,
            specified_bbox: None,
            specified_close: None,
            hfield: None,
        },
        quality: QualityConfig {
            min_angle_deg: 28.0,
            auto_refine_batch_cells: DEFAULT_AUTO_REFINE_BATCH_CELLS,
            on_violation: ViolationPolicy::Block,
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

    let mut p = sample();
    p.target.kind = MeshDomainKind::Land;
    p.data_layers.push(ProjectDataLayer {
        id: "sea_slope".into(),
        role: ProjectLayerRole::Threshold(ThresholdField::SeaSlope),
        path: "./in/sea_slope.nc".into(),
        enabled: true,
        threshold_value: None,
    });
    let err = yaml_err(&p);
    assert!(err.contains("threshold layer 'sea_slope' is not applicable to Land targets"));
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
    });
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast merit_root must not be empty"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: Some(" ".into()),
        merit_stride: 1,
        r3_width_m: 300.0,
        r2_width_m: 50.0,
    });
    let err = yaml_err(&p);
    assert!(err.contains("hydro_coast cama_root must not be empty"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        merit_stride: 1,
        r3_width_m: 0.0,
        r2_width_m: 50.0,
    });
    let err = json_err(&p);
    assert!(err.contains("hydro_coast widths must be > 0"));

    p.hydro_coast = Some(HydroCoastConfig {
        merit_root: "/data/merit".into(),
        cama_root: None,
        merit_stride: 1,
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
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        "inline:bbox:w=112,e=115.5,s=21.5,n=23.5"
    );
    assert_eq!(lowered.mkgrd.mask_sea_ratio, 0.25);
    assert_eq!(lowered.mkgrd.experiment_name, "gba");
    assert_eq!(lowered.mkgrd.openmp, 8); // expert override
    assert_eq!(lowered.mkgrd.beta, 1.1);
    assert_eq!(lowered.mkgrd.relax, 0.03);

    // landcover → landtype_file + landtype-count refine; lai → refine switch + refine_cal
    assert_eq!(lowered.mkgrd.landtype_file, "./in/landtype.nc");
    assert!(lowered.refine.refine_num_landtypes);
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
    assert!(nml.contains("&hfield"));
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
fn threshold_master_switch_keeps_landtype_available_without_calculated_refinement() {
    let mut project = sample();
    project.refinement.threshold_enabled = false;
    project.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 113.0,
        lat: 22.5,
        radius_km: 100.0,
    });

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
fn hfield_is_default_unless_explicit_compatibility() {
    let mut p = sample();
    p.refinement.hfield = None;
    assert!(p.lower().to_namelist().contains("&hfield"));

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
    assert!(h.criteria.contains(&ThresholdField::Dem));
    assert!(h.criteria.contains(&ThresholdField::SlopeMax));
    assert!(h.criteria.contains(&ThresholdField::KSolids));
    assert!(h.criteria.contains(&ThresholdField::Tksatu));
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
    assert!(!ProjectLayerRole::LandType.wants_folder());
    assert!(ProjectLayerRole::MeritHydro.wants_folder());
    assert!(ProjectLayerRole::Cama.wants_folder());
    assert!(!ProjectLayerRole::Threshold(ThresholdField::Slope).wants_folder());
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
fn quality_auto_refine_rejects_domains_that_cannot_be_repaired() {
    let mut p = sample();
    p.quality.on_violation = ViolationPolicy::AutoRefine;
    p.domain = DomainConfig::Global;
    assert!(p
        .validate()
        .unwrap_err()
        .contains("auto_refine requires a regional domain"));

    p.domain = sample().domain;
    p.refinement.enabled = false;
    assert!(p
        .validate()
        .unwrap_err()
        .contains("auto_refine requires refinement.enabled"));
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
fn criterion_lookup_and_domain_filter() {
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

    let ocean = criteria_for_domain(MeshDomainKind::Ocean);
    assert!(ocean.iter().any(|c| c.id == "sea_slope"));
    assert!(ocean.iter().all(|c| c.id != "lai")); // land-only excluded

    let atmosphere = criteria_for_domain(MeshDomainKind::Atmosphere);
    assert_eq!(atmosphere.len(), 1);
    assert_eq!(atmosphere[0].id, "typhoon");

    let earth = criteria_for_domain(MeshDomainKind::Earth);
    assert!(earth.iter().any(|c| c.id == "lai"));
    assert!(earth.iter().any(|c| c.id == "sea_slope"));
    assert!(earth.iter().any(|c| c.id == "typhoon"));

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
    assert_eq!(p.lower().mkgrd.nxp, 80);

    p.target.resolution =
        ResolutionSpec::ApproxDegree(100.0 / earthmesh_core::KM_PER_DEGREE_EQUATOR);
    assert_eq!(p.lower().mkgrd.nxp, 80);
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
    });
    let plan = p.hydro_execution_plan().unwrap().unwrap();
    assert_eq!(plan.merit_root, "/data/merit");
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
