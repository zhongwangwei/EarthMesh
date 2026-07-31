use earthmesh_project::{
    classify_project_capability, CloseBoundaryMode, CloseMaskFormat, DomainConfig,
    HydroCoastConfig, MeshCellKind, MeshDomainKind, MeshIntentPreset, ModelFormat,
    ProjectCapability, ProjectCapabilityKey, ProjectCloseBoundaryMode, ProjectContractId,
    ProjectCoordinateMode, ProjectDomainClass, ProjectParameterizedTestId, ProjectRejectionReason,
    ProjectSourceProfile, ProjectSpecifiedSource, ProjectTargetTriple, ProjectValidationTestId,
    RegionShape, ResolutionSpec, SpecifiedBboxRefinement, SpecifiedCircleRefinement,
    SpecifiedCloseRefinement,
};

#[derive(Clone, Copy)]
struct SupportedCase {
    name: &'static str,
    domain: ProjectDomainClass,
    target: ProjectTargetTriple,
    sources: ProjectSourceProfile,
    contract: ProjectContractId,
    test_id: ProjectParameterizedTestId,
}

fn target(
    kind: MeshDomainKind,
    cell: MeshCellKind,
    model_format: ModelFormat,
) -> ProjectTargetTriple {
    ProjectTargetTriple {
        kind,
        cell,
        model_format,
    }
}

fn sources(specified: ProjectSpecifiedSource, bits: u8) -> ProjectSourceProfile {
    ProjectSourceProfile::from_bit_mask(specified, bits)
}

#[test]
fn representative_refinement_matrix_maps_to_declared_contracts() {
    use MeshCellKind::{Hex, Tri};
    use MeshDomainKind::{Atmosphere, Coupled, Earth, Land, Ocean};
    use ModelFormat::{CoLM, Fvcom, Mpas, MpasSimple};
    use ProjectParameterizedTestId as T;

    let cases = [
        SupportedCase {
            name: "global atmosphere + landcover + auto-refine",
            domain: ProjectDomainClass::Global,
            target: target(Atmosphere, Hex, Mpas),
            sources: sources(ProjectSpecifiedSource::None, 0b1_0010),
            contract: ProjectContractId::ClosedAtmosHex,
            test_id: T::ClosedAtmosHex,
        },
        SupportedCase {
            name: "polar regional atmosphere + circle",
            domain: ProjectDomainClass::RegionalCircle,
            target: target(Atmosphere, Hex, MpasSimple),
            sources: sources(ProjectSpecifiedSource::Circle, 0b0_0001),
            contract: ProjectContractId::OpenAtmosHex,
            test_id: T::OpenAtmosHex,
        },
        SupportedCase {
            name: "antimeridian regional atmosphere + bbox",
            domain: ProjectDomainClass::RegionalBbox,
            target: target(Atmosphere, Hex, Mpas),
            sources: sources(ProjectSpecifiedSource::Bbox, 0b0_0011),
            contract: ProjectContractId::OpenAtmosHex,
            test_id: T::OpenAtmosHex,
        },
        SupportedCase {
            name: "global earth hex",
            domain: ProjectDomainClass::Global,
            target: target(Earth, Hex, CoLM),
            sources: sources(ProjectSpecifiedSource::None, 0),
            contract: ProjectContractId::ClosedEarthHex,
            test_id: T::ClosedEarthHex,
        },
        SupportedCase {
            name: "custom global land",
            domain: ProjectDomainClass::Global,
            target: target(Land, Hex, CoLM),
            sources: sources(ProjectSpecifiedSource::None, 0b0_0010),
            contract: ProjectContractId::MaskedLandHex,
            test_id: T::MaskedLandHex,
        },
        SupportedCase {
            name: "custom regional land",
            domain: ProjectDomainClass::RegionalBbox,
            target: target(Land, Hex, CoLM),
            sources: sources(ProjectSpecifiedSource::Bbox, 0b0_0011),
            contract: ProjectContractId::OpenLandHex,
            test_id: T::OpenLandHex,
        },
        SupportedCase {
            name: "watershed land",
            domain: ProjectDomainClass::RegionalShapefile,
            target: target(Land, Hex, CoLM),
            sources: sources(
                ProjectSpecifiedSource::Close {
                    format: CloseMaskFormat::PolygonShp,
                    boundary: ProjectCloseBoundaryMode::Polyline,
                },
                0b0_0010,
            ),
            contract: ProjectContractId::BasinLandHex,
            test_id: T::BasinLandHex,
        },
        SupportedCase {
            name: "internal-boundary earth basin",
            domain: ProjectDomainClass::RegionalClose {
                format: CloseMaskFormat::Nml,
                boundary: ProjectCloseBoundaryMode::Polyline,
            },
            target: target(Earth, Hex, CoLM),
            sources: sources(
                ProjectSpecifiedSource::Close {
                    format: CloseMaskFormat::Nml,
                    boundary: ProjectCloseBoundaryMode::Polyline,
                },
                0b0_0001,
            ),
            contract: ProjectContractId::BasinLandHex,
            test_id: T::BasinLandHex,
        },
        SupportedCase {
            name: "global coastal ocean",
            domain: ProjectDomainClass::Global,
            target: target(Ocean, Tri, Fvcom),
            sources: sources(ProjectSpecifiedSource::None, 0b0_0010),
            contract: ProjectContractId::MaskedOceanTri,
            test_id: T::MaskedOceanTri,
        },
        SupportedCase {
            name: "regional coastal ocean",
            domain: ProjectDomainClass::RegionalCircle,
            target: target(Ocean, Tri, Fvcom),
            sources: sources(ProjectSpecifiedSource::Circle, 0b0_0011),
            contract: ProjectContractId::OpenOceanTri,
            test_id: T::OpenOceanTri,
        },
        SupportedCase {
            name: "coupled regional mesh",
            domain: ProjectDomainClass::RegionalBbox,
            target: target(Coupled, Hex, CoLM),
            sources: sources(ProjectSpecifiedSource::Bbox, 0b0_0010),
            contract: ProjectContractId::CoupledHex,
            test_id: T::CoupledHex,
        },
        SupportedCase {
            name: "hydro basin + river + coast",
            domain: ProjectDomainClass::RegionalShapefile,
            target: target(Coupled, Hex, CoLM),
            sources: sources(ProjectSpecifiedSource::None, 0b0_1110),
            contract: ProjectContractId::CoupledHex,
            test_id: T::CoupledHex,
        },
    ];

    for case in cases {
        let key = ProjectCapabilityKey {
            domain: case.domain,
            target: case.target,
            sources: case.sources,
            coordinate_mode: ProjectCoordinateMode::SphericalLonLat,
        };
        assert_eq!(
            classify_project_capability(key),
            ProjectCapability::Supported {
                contract_id: case.contract,
                parameterized_test_id: case.test_id,
            },
            "{}",
            case.name
        );
        assert_eq!(case.test_id.report_key(key), Some(key), "{}", case.name);
    }
}

#[test]
fn geometric_edge_projects_validate_and_lower_without_running_the_engine() {
    let mut polar = earthmesh_project::ProjectConfig::scaffold(
        "polar-atmosphere",
        MeshIntentPreset::AtmosphereMpas,
        DomainConfig::Regional {
            shape: RegionShape::Circle {
                lon: 30.0,
                lat: 88.0,
                radius_km: 150.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(81),
    );
    polar.target.model_format = ModelFormat::MpasSimple;
    polar.refinement.enabled = true;
    polar.refinement.max_passes = 2;
    polar.refinement.specified_circle = Some(SpecifiedCircleRefinement {
        lon: 30.0,
        lat: 89.0,
        radius_km: 50.0,
    });
    let lowered = polar.try_lower().expect("polar atmosphere contract");
    assert_eq!(lowered.mkgrd.mask_domain_type, "circle");
    assert_eq!(lowered.mkgrd.mode_grid, "hex");
    assert!(lowered.refine.refine_spc);

    let mut dateline = earthmesh_project::ProjectConfig::scaffold(
        "dateline-atmosphere",
        MeshIntentPreset::AtmosphereMpas,
        DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 170.0,
                e: -170.0,
                s: -20.0,
                n: 20.0,
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(81),
    );
    dateline.refinement.enabled = true;
    dateline.refinement.max_passes = 2;
    dateline.refinement.specified_bbox = Some(SpecifiedBboxRefinement {
        w: 175.0,
        e: -175.0,
        s: -10.0,
        n: 10.0,
    });
    let lowered = dateline.try_lower().expect("antimeridian bbox contract");
    assert_eq!(
        lowered.mkgrd.mask_domain_fprefix,
        "inline:bbox:w=170,e=-170,s=-20,n=20"
    );
    assert_eq!(lowered.refine.mask_refine_spc_type, "bbox");

    let mut basin = earthmesh_project::ProjectConfig::scaffold(
        "basin-land",
        MeshIntentPreset::Custom,
        DomainConfig::Regional {
            shape: RegionShape::Shapefile {
                path: "input/watershed_with_holes.shp".into(),
            },
            sea_ratio: None,
        },
        ResolutionSpec::Nxp(81),
    );
    basin.refinement.enabled = true;
    basin.refinement.max_passes = 2;
    basin.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "input/refine_internal_boundaries.shp".into(),
        boundary: CloseBoundaryMode::Polyline,
    });
    let lowered = basin.try_lower().expect("basin/internal-boundary contract");
    assert_eq!(lowered.mkgrd.mask_domain_type, "close");
    assert_eq!(lowered.refine.mask_refine_spc_type, "close");

    let mut coastal = earthmesh_project::ProjectConfig::scaffold(
        "coastal-ocean",
        MeshIntentPreset::CoastalOcean,
        DomainConfig::Regional {
            shape: RegionShape::Close {
                path: "input/coast_with_islands.nml".into(),
                format: CloseMaskFormat::Nml,
                boundary: CloseBoundaryMode::Polyline,
            },
            sea_ratio: Some(0.5),
        },
        ResolutionSpec::Nxp(81),
    );
    coastal.refinement.enabled = true;
    coastal.refinement.max_passes = 2;
    coastal.refinement.specified_close = Some(SpecifiedCloseRefinement {
        path: "input/coastal_refinement.nml".into(),
        boundary: CloseBoundaryMode::Polyline,
    });
    let lowered = coastal.try_lower().expect("coastal tri contract");
    assert_eq!(lowered.mkgrd.mode_grid, "tri");
    assert_eq!(lowered.mkgrd.output_format, "FVCOM");

    let mut hydro = earthmesh_project::ProjectConfig::scaffold(
        "hydro-coupled-basin",
        MeshIntentPreset::MeritHydroCoast,
        DomainConfig::Regional {
            shape: RegionShape::Shapefile {
                path: "input/basin.shp".into(),
            },
            sea_ratio: Some(0.25),
        },
        ResolutionSpec::Nxp(81),
    );
    let merit = hydro
        .data_layers
        .iter_mut()
        .find(|layer| layer.role == earthmesh_project::ProjectLayerRole::MeritHydro)
        .expect("MERIT scaffold layer");
    merit.path = "input/merit".into();
    merit.enabled = true;
    hydro.hydro_coast = Some(HydroCoastConfig {
        merit_root: "input/merit".into(),
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
        coast_buffer_km: 25.0,
        coast_land_refinement_enabled: true,
        coast_ocean_refinement_enabled: true,
    });
    hydro.refinement.enabled = true;
    hydro.refinement.threshold_enabled = true;
    hydro.refinement.max_passes = 2;
    hydro.try_lower().expect("coupled hydro contract");
    assert_eq!(
        hydro
            .hydro_execution_plan()
            .expect("hydro plan")
            .expect("enabled hydro")
            .max_level,
        2
    );
}

#[test]
fn unsupported_target_matrix_matches_validation_reason_codes() {
    use MeshCellKind::{Hex, Tri};
    use MeshDomainKind::{Atmosphere, Coupled, Earth, Land, Ocean};
    use ModelFormat::{CoLM, Fvcom, Mpas};

    let cases = [
        (
            target(Land, Tri, CoLM),
            ProjectRejectionReason::LandRequiresHex,
            ProjectValidationTestId::LandCell,
        ),
        (
            target(Ocean, Hex, Fvcom),
            ProjectRejectionReason::OceanRequiresTri,
            ProjectValidationTestId::OceanCell,
        ),
        (
            target(Atmosphere, Tri, Mpas),
            ProjectRejectionReason::AtmosphereRequiresHex,
            ProjectValidationTestId::AtmosphereCell,
        ),
        (
            target(Coupled, Tri, CoLM),
            ProjectRejectionReason::CoupledRequiresHex,
            ProjectValidationTestId::CoupledCell,
        ),
        (
            target(Earth, Tri, CoLM),
            ProjectRejectionReason::EarthRequiresHex,
            ProjectValidationTestId::EarthCell,
        ),
        (
            target(Land, Hex, Fvcom),
            ProjectRejectionReason::LandRequiresColm,
            ProjectValidationTestId::LandFormat,
        ),
        (
            target(Ocean, Tri, CoLM),
            ProjectRejectionReason::OceanRequiresFvcom,
            ProjectValidationTestId::OceanFormat,
        ),
        (
            target(Atmosphere, Hex, CoLM),
            ProjectRejectionReason::AtmosphereRequiresMpas,
            ProjectValidationTestId::AtmosphereFormat,
        ),
        (
            target(Coupled, Hex, Mpas),
            ProjectRejectionReason::CoupledRequiresColm,
            ProjectValidationTestId::CoupledFormat,
        ),
        (
            target(Earth, Hex, Mpas),
            ProjectRejectionReason::EarthRequiresColm,
            ProjectValidationTestId::EarthFormat,
        ),
    ];

    for (target, reason, validation_test_id) in cases {
        let key = ProjectCapabilityKey {
            domain: ProjectDomainClass::Global,
            target,
            sources: ProjectSourceProfile::none(),
            coordinate_mode: ProjectCoordinateMode::SphericalLonLat,
        };
        assert_eq!(
            classify_project_capability(key),
            ProjectCapability::Rejected {
                reason_code: reason,
                validation_test_id,
            },
            "{target:?}"
        );
        assert_eq!(validation_test_id.report_key(key), Some(key), "{target:?}");

        let mut project = earthmesh_project::ProjectConfig::scaffold(
            "unsupported-target",
            MeshIntentPreset::Custom,
            DomainConfig::Global,
            ResolutionSpec::Nxp(81),
        );
        project.target.kind = target.kind;
        project.target.cell = target.cell;
        project.target.model_format = target.model_format;
        assert_eq!(
            project.validate().expect_err("target must be rejected"),
            reason.message(),
            "{target:?}"
        );
    }
}
