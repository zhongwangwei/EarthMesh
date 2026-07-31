use serde::Serialize;

use crate::{
    CloseBoundaryMode, CloseMaskFormat, DomainConfig, MeshCellKind, MeshDomainKind,
    MeshIntentPreset, MeshTargetConfig, ModelFormat, ThresholdField,
};

pub const PROJECT_DOMAIN_CLASS_COUNT: usize = 16;
pub const PROJECT_TARGET_TRIPLE_COUNT: usize = 40;
pub const PROJECT_SOURCE_PROFILE_COUNT: usize = 480;
pub const PROJECT_RAW_CAPABILITY_KEY_COUNT: usize =
    PROJECT_DOMAIN_CLASS_COUNT * PROJECT_TARGET_TRIPLE_COUNT * PROJECT_SOURCE_PROFILE_COUNT;

const DOMAIN_KINDS: [MeshDomainKind; 5] = [
    MeshDomainKind::Land,
    MeshDomainKind::Ocean,
    MeshDomainKind::Atmosphere,
    MeshDomainKind::Coupled,
    MeshDomainKind::Earth,
];
const CELL_KINDS: [MeshCellKind; 2] = [MeshCellKind::Hex, MeshCellKind::Tri];
const MODEL_FORMATS: [ModelFormat; 4] = [
    ModelFormat::CoLM,
    ModelFormat::Mpas,
    ModelFormat::MpasSimple,
    ModelFormat::Fvcom,
];
const CLOSE_FORMATS: [CloseMaskFormat; 4] = [
    CloseMaskFormat::PolygonShp,
    CloseMaskFormat::Nml,
    CloseMaskFormat::Netcdf,
    CloseMaskFormat::LonLatText,
];
const CLOSE_BOUNDARY_MODES: [ProjectCloseBoundaryMode; 3] = [
    ProjectCloseBoundaryMode::Polyline,
    ProjectCloseBoundaryMode::SphericalChaikin,
    ProjectCloseBoundaryMode::EnclosingCap,
];

pub const PROJECT_THRESHOLD_FIELDS: [ThresholdField; 14] = [
    ThresholdField::Lai,
    ThresholdField::Slope,
    ThresholdField::Dem,
    ThresholdField::SlopeMax,
    ThresholdField::Ks,
    ThresholdField::KSolids,
    ThresholdField::Tkdry,
    ThresholdField::Tksatf,
    ThresholdField::Tksatu,
    ThresholdField::Sst,
    ThresholdField::Ssh,
    ThresholdField::Eke,
    ThresholdField::SeaSlope,
    ThresholdField::Typhoon,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectCoordinateMode {
    SphericalLonLat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectCloseBoundaryMode {
    Polyline,
    SphericalChaikin,
    EnclosingCap,
}

impl From<&CloseBoundaryMode> for ProjectCloseBoundaryMode {
    fn from(value: &CloseBoundaryMode) -> Self {
        match value {
            CloseBoundaryMode::Polyline => Self::Polyline,
            CloseBoundaryMode::SphericalChaikin { .. } => Self::SphericalChaikin,
            CloseBoundaryMode::EnclosingCap { .. } => Self::EnclosingCap,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectDomainClass {
    Global,
    RegionalBbox,
    RegionalCircle,
    RegionalShapefile,
    RegionalClose {
        format: CloseMaskFormat,
        boundary: ProjectCloseBoundaryMode,
    },
}

impl ProjectDomainClass {
    pub fn from_domain(domain: &DomainConfig) -> Self {
        match domain {
            DomainConfig::Global => Self::Global,
            DomainConfig::Regional { shape, .. } => match shape {
                crate::RegionShape::Bbox { .. } => Self::RegionalBbox,
                crate::RegionShape::Circle { .. } => Self::RegionalCircle,
                crate::RegionShape::Shapefile { .. } => Self::RegionalShapefile,
                crate::RegionShape::Close {
                    format, boundary, ..
                } => Self::RegionalClose {
                    format: *format,
                    boundary: boundary.into(),
                },
            },
        }
    }

    fn is_global(self) -> bool {
        self == Self::Global
    }

    fn is_basin(self) -> bool {
        matches!(self, Self::RegionalShapefile | Self::RegionalClose { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectTargetTriple {
    pub kind: MeshDomainKind,
    pub cell: MeshCellKind,
    pub model_format: ModelFormat,
}

impl From<&MeshTargetConfig> for ProjectTargetTriple {
    fn from(value: &MeshTargetConfig) -> Self {
        Self {
            kind: value.kind,
            cell: value.cell,
            model_format: value.model_format,
        }
    }
}

impl ProjectTargetTriple {
    pub fn rejection_reason(self) -> Option<ProjectRejectionReason> {
        let expected_cell = match self.kind {
            MeshDomainKind::Ocean => MeshCellKind::Tri,
            MeshDomainKind::Land
            | MeshDomainKind::Atmosphere
            | MeshDomainKind::Coupled
            | MeshDomainKind::Earth => MeshCellKind::Hex,
        };
        if self.cell != expected_cell {
            return Some(match self.kind {
                MeshDomainKind::Land => ProjectRejectionReason::LandRequiresHex,
                MeshDomainKind::Ocean => ProjectRejectionReason::OceanRequiresTri,
                MeshDomainKind::Atmosphere => ProjectRejectionReason::AtmosphereRequiresHex,
                MeshDomainKind::Coupled => ProjectRejectionReason::CoupledRequiresHex,
                MeshDomainKind::Earth => ProjectRejectionReason::EarthRequiresHex,
            });
        }

        let format_supported = match self.kind {
            MeshDomainKind::Land | MeshDomainKind::Coupled | MeshDomainKind::Earth => {
                self.model_format == ModelFormat::CoLM
            }
            MeshDomainKind::Ocean => self.model_format == ModelFormat::Fvcom,
            MeshDomainKind::Atmosphere => {
                matches!(
                    self.model_format,
                    ModelFormat::Mpas | ModelFormat::MpasSimple
                )
            }
        };
        if format_supported {
            None
        } else {
            Some(match self.kind {
                MeshDomainKind::Land => ProjectRejectionReason::LandRequiresColm,
                MeshDomainKind::Ocean => ProjectRejectionReason::OceanRequiresFvcom,
                MeshDomainKind::Atmosphere => ProjectRejectionReason::AtmosphereRequiresMpas,
                MeshDomainKind::Coupled => ProjectRejectionReason::CoupledRequiresColm,
                MeshDomainKind::Earth => ProjectRejectionReason::EarthRequiresColm,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectSpecifiedSource {
    None,
    Circle,
    Bbox,
    Close {
        format: CloseMaskFormat,
        boundary: ProjectCloseBoundaryMode,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectSourceAtom {
    ContinuousThreshold,
    Landcover,
    HydroRiver,
    HydroCoast,
    AutoRefineEpoch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectSourceProfile {
    pub specified: ProjectSpecifiedSource,
    pub continuous_threshold: bool,
    pub landcover: bool,
    pub hydro_river: bool,
    pub hydro_coast: bool,
    pub auto_refine_epoch: bool,
}

impl ProjectSourceProfile {
    pub const fn none() -> Self {
        Self::from_bit_mask(ProjectSpecifiedSource::None, 0)
    }

    pub const fn from_bit_mask(specified: ProjectSpecifiedSource, bits: u8) -> Self {
        Self {
            specified,
            continuous_threshold: bits & 0b00001 != 0,
            landcover: bits & 0b00010 != 0,
            hydro_river: bits & 0b00100 != 0,
            hydro_coast: bits & 0b01000 != 0,
            auto_refine_epoch: bits & 0b10000 != 0,
        }
    }

    pub const fn bit_mask(self) -> u8 {
        self.continuous_threshold as u8
            | ((self.landcover as u8) << 1)
            | ((self.hydro_river as u8) << 2)
            | ((self.hydro_coast as u8) << 3)
            | ((self.auto_refine_epoch as u8) << 4)
    }

    pub const fn contains(self, atom: ProjectSourceAtom) -> bool {
        match atom {
            ProjectSourceAtom::ContinuousThreshold => self.continuous_threshold,
            ProjectSourceAtom::Landcover => self.landcover,
            ProjectSourceAtom::HydroRiver => self.hydro_river,
            ProjectSourceAtom::HydroCoast => self.hydro_coast,
            ProjectSourceAtom::AutoRefineEpoch => self.auto_refine_epoch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectCapabilityKey {
    pub domain: ProjectDomainClass,
    pub target: ProjectTargetTriple,
    pub sources: ProjectSourceProfile,
    pub coordinate_mode: ProjectCoordinateMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectContractId {
    #[serde(rename = "P_CLOSED_ATMOS_HEX")]
    ClosedAtmosHex,
    #[serde(rename = "P_OPEN_ATMOS_HEX")]
    OpenAtmosHex,
    #[serde(rename = "P_CLOSED_EARTH_HEX")]
    ClosedEarthHex,
    #[serde(rename = "P_MASKED_LAND_HEX")]
    MaskedLandHex,
    #[serde(rename = "P_OPEN_LAND_HEX")]
    OpenLandHex,
    #[serde(rename = "P_BASIN_LAND_HEX")]
    BasinLandHex,
    #[serde(rename = "P_MASKED_OCEAN_TRI")]
    MaskedOceanTri,
    #[serde(rename = "P_OPEN_OCEAN_TRI")]
    OpenOceanTri,
    #[serde(rename = "P_COUPLED_HEX")]
    CoupledHex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectParameterizedTestId {
    ClosedAtmosHex,
    OpenAtmosHex,
    ClosedEarthHex,
    MaskedLandHex,
    OpenLandHex,
    BasinLandHex,
    MaskedOceanTri,
    OpenOceanTri,
    CoupledHex,
}

impl ProjectParameterizedTestId {
    fn accepts(self, key: ProjectCapabilityKey) -> bool {
        let ProjectCapabilityKey { domain, target, .. } = key;
        let supported_atmosphere = target.kind == MeshDomainKind::Atmosphere
            && target.cell == MeshCellKind::Hex
            && matches!(
                target.model_format,
                ModelFormat::Mpas | ModelFormat::MpasSimple
            );
        let supported_land_or_earth =
            matches!(target.kind, MeshDomainKind::Land | MeshDomainKind::Earth)
                && target.cell == MeshCellKind::Hex
                && target.model_format == ModelFormat::CoLM;
        match self {
            Self::ClosedAtmosHex => domain.is_global() && supported_atmosphere,
            Self::OpenAtmosHex => !domain.is_global() && supported_atmosphere,
            Self::ClosedEarthHex => {
                domain.is_global()
                    && target.kind == MeshDomainKind::Earth
                    && target.cell == MeshCellKind::Hex
                    && target.model_format == ModelFormat::CoLM
            }
            Self::MaskedLandHex => {
                domain.is_global()
                    && target.kind == MeshDomainKind::Land
                    && target.cell == MeshCellKind::Hex
                    && target.model_format == ModelFormat::CoLM
            }
            Self::OpenLandHex => {
                !domain.is_global() && !domain.is_basin() && supported_land_or_earth
            }
            Self::BasinLandHex => domain.is_basin() && supported_land_or_earth,
            Self::MaskedOceanTri => {
                domain.is_global()
                    && target.kind == MeshDomainKind::Ocean
                    && target.cell == MeshCellKind::Tri
                    && target.model_format == ModelFormat::Fvcom
            }
            Self::OpenOceanTri => {
                !domain.is_global()
                    && target.kind == MeshDomainKind::Ocean
                    && target.cell == MeshCellKind::Tri
                    && target.model_format == ModelFormat::Fvcom
            }
            Self::CoupledHex => {
                target.kind == MeshDomainKind::Coupled
                    && target.cell == MeshCellKind::Hex
                    && target.model_format == ModelFormat::CoLM
            }
        }
    }

    pub fn report_key(self, key: ProjectCapabilityKey) -> Option<ProjectCapabilityKey> {
        self.accepts(key).then_some(key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectValidationTestId {
    LandCell,
    OceanCell,
    AtmosphereCell,
    CoupledCell,
    EarthCell,
    LandFormat,
    OceanFormat,
    AtmosphereFormat,
    CoupledFormat,
    EarthFormat,
}

impl ProjectValidationTestId {
    fn accepts(self, key: ProjectCapabilityKey) -> bool {
        let target = key.target;
        let expected_cell = match target.kind {
            MeshDomainKind::Ocean => MeshCellKind::Tri,
            MeshDomainKind::Land
            | MeshDomainKind::Atmosphere
            | MeshDomainKind::Coupled
            | MeshDomainKind::Earth => MeshCellKind::Hex,
        };
        match self {
            Self::LandCell => target.kind == MeshDomainKind::Land && target.cell != expected_cell,
            Self::OceanCell => target.kind == MeshDomainKind::Ocean && target.cell != expected_cell,
            Self::AtmosphereCell => {
                target.kind == MeshDomainKind::Atmosphere && target.cell != expected_cell
            }
            Self::CoupledCell => {
                target.kind == MeshDomainKind::Coupled && target.cell != expected_cell
            }
            Self::EarthCell => target.kind == MeshDomainKind::Earth && target.cell != expected_cell,
            Self::LandFormat => {
                target.kind == MeshDomainKind::Land
                    && target.cell == expected_cell
                    && target.model_format != ModelFormat::CoLM
            }
            Self::OceanFormat => {
                target.kind == MeshDomainKind::Ocean
                    && target.cell == expected_cell
                    && target.model_format != ModelFormat::Fvcom
            }
            Self::AtmosphereFormat => {
                target.kind == MeshDomainKind::Atmosphere
                    && target.cell == expected_cell
                    && !matches!(
                        target.model_format,
                        ModelFormat::Mpas | ModelFormat::MpasSimple
                    )
            }
            Self::CoupledFormat => {
                target.kind == MeshDomainKind::Coupled
                    && target.cell == expected_cell
                    && target.model_format != ModelFormat::CoLM
            }
            Self::EarthFormat => {
                target.kind == MeshDomainKind::Earth
                    && target.cell == expected_cell
                    && target.model_format != ModelFormat::CoLM
            }
        }
    }

    pub fn report_key(self, key: ProjectCapabilityKey) -> Option<ProjectCapabilityKey> {
        self.accepts(key).then_some(key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectRejectionReason {
    LandRequiresHex,
    OceanRequiresTri,
    AtmosphereRequiresHex,
    CoupledRequiresHex,
    EarthRequiresHex,
    LandRequiresColm,
    OceanRequiresFvcom,
    AtmosphereRequiresMpas,
    CoupledRequiresColm,
    EarthRequiresColm,
}

impl ProjectRejectionReason {
    pub const fn validation_test_id(self) -> ProjectValidationTestId {
        match self {
            Self::LandRequiresHex => ProjectValidationTestId::LandCell,
            Self::OceanRequiresTri => ProjectValidationTestId::OceanCell,
            Self::AtmosphereRequiresHex => ProjectValidationTestId::AtmosphereCell,
            Self::CoupledRequiresHex => ProjectValidationTestId::CoupledCell,
            Self::EarthRequiresHex => ProjectValidationTestId::EarthCell,
            Self::LandRequiresColm => ProjectValidationTestId::LandFormat,
            Self::OceanRequiresFvcom => ProjectValidationTestId::OceanFormat,
            Self::AtmosphereRequiresMpas => ProjectValidationTestId::AtmosphereFormat,
            Self::CoupledRequiresColm => ProjectValidationTestId::CoupledFormat,
            Self::EarthRequiresColm => ProjectValidationTestId::EarthFormat,
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::LandRequiresHex => "land target cell must be Hex",
            Self::OceanRequiresTri => "ocean target cell must be Tri",
            Self::AtmosphereRequiresHex => "atmosphere target cell must be Hex",
            Self::CoupledRequiresHex => "coupled target cell must be Hex",
            Self::EarthRequiresHex => "earth target cell must be Hex",
            Self::LandRequiresColm => "land target model_format must be CoLM",
            Self::OceanRequiresFvcom => "ocean target model_format must be FVCOM",
            Self::AtmosphereRequiresMpas => {
                "atmosphere target model_format must be MPAS or MPAS-Simple"
            }
            Self::CoupledRequiresColm => "coupled target model_format must be CoLM",
            Self::EarthRequiresColm => "earth target model_format must be CoLM",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ProjectCapability {
    Supported {
        contract_id: ProjectContractId,
        parameterized_test_id: ProjectParameterizedTestId,
    },
    Rejected {
        reason_code: ProjectRejectionReason,
        validation_test_id: ProjectValidationTestId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProjectCapabilityEntry {
    pub key: ProjectCapabilityKey,
    pub capability: ProjectCapability,
}

pub fn project_domain_classes() -> impl Iterator<Item = ProjectDomainClass> {
    [
        ProjectDomainClass::Global,
        ProjectDomainClass::RegionalBbox,
        ProjectDomainClass::RegionalCircle,
        ProjectDomainClass::RegionalShapefile,
    ]
    .into_iter()
    .chain(CLOSE_FORMATS.into_iter().flat_map(|format| {
        CLOSE_BOUNDARY_MODES
            .into_iter()
            .map(move |boundary| ProjectDomainClass::RegionalClose { format, boundary })
    }))
}

pub fn project_target_triples() -> impl Iterator<Item = ProjectTargetTriple> {
    DOMAIN_KINDS.into_iter().flat_map(|kind| {
        CELL_KINDS.into_iter().flat_map(move |cell| {
            MODEL_FORMATS
                .into_iter()
                .map(move |model_format| ProjectTargetTriple {
                    kind,
                    cell,
                    model_format,
                })
        })
    })
}

pub fn project_specified_sources() -> impl Iterator<Item = ProjectSpecifiedSource> {
    [
        ProjectSpecifiedSource::None,
        ProjectSpecifiedSource::Circle,
        ProjectSpecifiedSource::Bbox,
    ]
    .into_iter()
    .chain(CLOSE_FORMATS.into_iter().flat_map(|format| {
        CLOSE_BOUNDARY_MODES
            .into_iter()
            .map(move |boundary| ProjectSpecifiedSource::Close { format, boundary })
    }))
}

pub fn project_source_profiles() -> impl Iterator<Item = ProjectSourceProfile> {
    project_specified_sources().flat_map(|specified| {
        (0..32).map(move |bits| ProjectSourceProfile::from_bit_mask(specified, bits))
    })
}

pub fn classify_project_capability(key: ProjectCapabilityKey) -> ProjectCapability {
    if let Some(reason_code) = key.target.rejection_reason() {
        return ProjectCapability::Rejected {
            reason_code,
            validation_test_id: reason_code.validation_test_id(),
        };
    }

    let contract_id = match key.target.kind {
        MeshDomainKind::Atmosphere if key.domain.is_global() => ProjectContractId::ClosedAtmosHex,
        MeshDomainKind::Atmosphere => ProjectContractId::OpenAtmosHex,
        MeshDomainKind::Earth if key.domain.is_global() => ProjectContractId::ClosedEarthHex,
        MeshDomainKind::Land if key.domain.is_global() => ProjectContractId::MaskedLandHex,
        MeshDomainKind::Land | MeshDomainKind::Earth if key.domain.is_basin() => {
            ProjectContractId::BasinLandHex
        }
        MeshDomainKind::Land | MeshDomainKind::Earth => ProjectContractId::OpenLandHex,
        MeshDomainKind::Ocean if key.domain.is_global() => ProjectContractId::MaskedOceanTri,
        MeshDomainKind::Ocean => ProjectContractId::OpenOceanTri,
        MeshDomainKind::Coupled => ProjectContractId::CoupledHex,
    };
    let parameterized_test_id = match contract_id {
        ProjectContractId::ClosedAtmosHex => ProjectParameterizedTestId::ClosedAtmosHex,
        ProjectContractId::OpenAtmosHex => ProjectParameterizedTestId::OpenAtmosHex,
        ProjectContractId::ClosedEarthHex => ProjectParameterizedTestId::ClosedEarthHex,
        ProjectContractId::MaskedLandHex => ProjectParameterizedTestId::MaskedLandHex,
        ProjectContractId::OpenLandHex => ProjectParameterizedTestId::OpenLandHex,
        ProjectContractId::BasinLandHex => ProjectParameterizedTestId::BasinLandHex,
        ProjectContractId::MaskedOceanTri => ProjectParameterizedTestId::MaskedOceanTri,
        ProjectContractId::OpenOceanTri => ProjectParameterizedTestId::OpenOceanTri,
        ProjectContractId::CoupledHex => ProjectParameterizedTestId::CoupledHex,
    };
    ProjectCapability::Supported {
        contract_id,
        parameterized_test_id,
    }
}

pub fn project_capability_registry() -> impl Iterator<Item = ProjectCapabilityEntry> {
    project_domain_classes().flat_map(|domain| {
        project_target_triples().flat_map(move |target| {
            project_source_profiles().map(move |sources| {
                let key = ProjectCapabilityKey {
                    domain,
                    target,
                    sources,
                    coordinate_mode: ProjectCoordinateMode::SphericalLonLat,
                };
                ProjectCapabilityEntry {
                    key,
                    capability: classify_project_capability(key),
                }
            })
        })
    })
}

pub fn preset_capability_key(preset: MeshIntentPreset) -> ProjectCapabilityKey {
    let defaults = preset.defaults();
    ProjectCapabilityKey {
        domain: ProjectDomainClass::Global,
        target: ProjectTargetTriple {
            kind: defaults.kind,
            cell: defaults.cell,
            model_format: defaults.model_format,
        },
        sources: ProjectSourceProfile::none(),
        coordinate_mode: ProjectCoordinateMode::SphericalLonLat,
    }
}

pub const fn threshold_topology_source_atom(field: ThresholdField) -> ProjectSourceAtom {
    match field {
        ThresholdField::Lai
        | ThresholdField::Slope
        | ThresholdField::Dem
        | ThresholdField::SlopeMax
        | ThresholdField::Ks
        | ThresholdField::KSolids
        | ThresholdField::Tkdry
        | ThresholdField::Tksatf
        | ThresholdField::Tksatu
        | ThresholdField::Sst
        | ThresholdField::Ssh
        | ThresholdField::Eke
        | ThresholdField::SeaSlope
        | ThresholdField::Typhoon => ProjectSourceAtom::ContinuousThreshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DomainConfig, ProjectConfig, ProjectLayerRole, ResolutionSpec, ThresholdStatistic,
    };

    #[test]
    fn project_registry_classifies_all_307_200_raw_keys_without_unknowns() {
        assert_eq!(project_domain_classes().count(), PROJECT_DOMAIN_CLASS_COUNT);
        assert_eq!(
            project_target_triples().count(),
            PROJECT_TARGET_TRIPLE_COUNT
        );
        assert_eq!(
            project_source_profiles().count(),
            PROJECT_SOURCE_PROFILE_COUNT
        );
        assert_eq!(project_specified_sources().count(), 15);
        for specified in project_specified_sources() {
            let mut seen_masks = [false; 32];
            for profile in
                project_source_profiles().filter(|profile| profile.specified == specified)
            {
                let mask = usize::from(profile.bit_mask());
                seen_masks[mask] = true;
                for (index, atom) in [
                    ProjectSourceAtom::ContinuousThreshold,
                    ProjectSourceAtom::Landcover,
                    ProjectSourceAtom::HydroRiver,
                    ProjectSourceAtom::HydroCoast,
                    ProjectSourceAtom::AutoRefineEpoch,
                ]
                .into_iter()
                .enumerate()
                {
                    assert_eq!(profile.contains(atom), mask & (1 << index) != 0);
                }
            }
            assert!(seen_masks.into_iter().all(|seen| seen));
        }

        let mut total = 0;
        let mut supported = 0;
        let mut rejected = 0;
        for entry in project_capability_registry() {
            total += 1;
            let reported = match entry.capability {
                ProjectCapability::Supported {
                    parameterized_test_id,
                    ..
                } => {
                    supported += 1;
                    parameterized_test_id
                        .report_key(entry.key)
                        .expect("supported test id must report its declared key")
                }
                ProjectCapability::Rejected {
                    validation_test_id, ..
                } => {
                    rejected += 1;
                    validation_test_id
                        .report_key(entry.key)
                        .expect("validation test id must report its declared key")
                }
            };
            assert_eq!(reported, entry.key);
        }

        assert_eq!(total, PROJECT_RAW_CAPABILITY_KEY_COUNT);
        assert_eq!(supported + rejected, total);
        assert_eq!(total - supported - rejected, 0, "Unknown must remain zero");
    }

    #[test]
    fn capability_reporters_are_independent_and_partition_every_raw_key() {
        let supported_reporters = [
            ProjectParameterizedTestId::ClosedAtmosHex,
            ProjectParameterizedTestId::OpenAtmosHex,
            ProjectParameterizedTestId::ClosedEarthHex,
            ProjectParameterizedTestId::MaskedLandHex,
            ProjectParameterizedTestId::OpenLandHex,
            ProjectParameterizedTestId::BasinLandHex,
            ProjectParameterizedTestId::MaskedOceanTri,
            ProjectParameterizedTestId::OpenOceanTri,
            ProjectParameterizedTestId::CoupledHex,
        ];
        let rejected_reporters = [
            ProjectValidationTestId::LandCell,
            ProjectValidationTestId::OceanCell,
            ProjectValidationTestId::AtmosphereCell,
            ProjectValidationTestId::CoupledCell,
            ProjectValidationTestId::EarthCell,
            ProjectValidationTestId::LandFormat,
            ProjectValidationTestId::OceanFormat,
            ProjectValidationTestId::AtmosphereFormat,
            ProjectValidationTestId::CoupledFormat,
            ProjectValidationTestId::EarthFormat,
        ];

        for entry in project_capability_registry() {
            let supported_matches = supported_reporters
                .iter()
                .copied()
                .filter(|reporter| reporter.report_key(entry.key) == Some(entry.key))
                .collect::<Vec<_>>();
            let rejected_matches = rejected_reporters
                .iter()
                .copied()
                .filter(|reporter| reporter.report_key(entry.key) == Some(entry.key))
                .collect::<Vec<_>>();
            match entry.capability {
                ProjectCapability::Supported {
                    parameterized_test_id,
                    ..
                } => {
                    assert_eq!(
                        supported_matches,
                        vec![parameterized_test_id],
                        "{:?}",
                        entry.key
                    );
                    assert!(rejected_matches.is_empty(), "{:?}", entry.key);
                }
                ProjectCapability::Rejected {
                    validation_test_id, ..
                } => {
                    assert!(supported_matches.is_empty(), "{:?}", entry.key);
                    assert_eq!(
                        rejected_matches,
                        vec![validation_test_id],
                        "{:?}",
                        entry.key
                    );
                }
            }
        }
    }

    #[test]
    fn only_the_six_declared_project_target_triples_validate() {
        let mut supported = 0;
        let mut rejected = 0;
        for target in project_target_triples() {
            let mut project = ProjectConfig::scaffold(
                "capability-target",
                MeshIntentPreset::Custom,
                DomainConfig::Global,
                ResolutionSpec::Nxp(81),
            );
            project.target.kind = target.kind;
            project.target.cell = target.cell;
            project.target.model_format = target.model_format;

            match target.rejection_reason() {
                None => {
                    supported += 1;
                    project
                        .validate()
                        .unwrap_or_else(|error| panic!("{target:?} rejected: {error}"));
                }
                Some(reason) => {
                    rejected += 1;
                    let error = project.validate().expect_err("target must be rejected");
                    assert_eq!(error, reason.message(), "{target:?}");
                }
            }
        }
        assert_eq!(supported, 6);
        assert_eq!(rejected, 34);
    }

    #[test]
    fn supported_keys_map_to_the_nine_domain_topology_contracts() {
        let mut seen = [false; 9];
        for domain in project_domain_classes() {
            for target in
                project_target_triples().filter(|target| target.rejection_reason().is_none())
            {
                let key = ProjectCapabilityKey {
                    domain,
                    target,
                    sources: ProjectSourceProfile::none(),
                    coordinate_mode: ProjectCoordinateMode::SphericalLonLat,
                };
                let ProjectCapability::Supported { contract_id, .. } =
                    classify_project_capability(key)
                else {
                    panic!("{key:?} must be supported");
                };
                seen[match contract_id {
                    ProjectContractId::ClosedAtmosHex => 0,
                    ProjectContractId::OpenAtmosHex => 1,
                    ProjectContractId::ClosedEarthHex => 2,
                    ProjectContractId::MaskedLandHex => 3,
                    ProjectContractId::OpenLandHex => 4,
                    ProjectContractId::BasinLandHex => 5,
                    ProjectContractId::MaskedOceanTri => 6,
                    ProjectContractId::OpenOceanTri => 7,
                    ProjectContractId::CoupledHex => 8,
                }] = true;
            }
        }
        assert!(seen.into_iter().all(|present| present));

        for (domain, target, expected) in [
            (
                ProjectDomainClass::Global,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Atmosphere,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::Mpas,
                },
                ProjectContractId::ClosedAtmosHex,
            ),
            (
                ProjectDomainClass::RegionalBbox,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Atmosphere,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::MpasSimple,
                },
                ProjectContractId::OpenAtmosHex,
            ),
            (
                ProjectDomainClass::Global,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Earth,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::CoLM,
                },
                ProjectContractId::ClosedEarthHex,
            ),
            (
                ProjectDomainClass::Global,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Land,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::CoLM,
                },
                ProjectContractId::MaskedLandHex,
            ),
            (
                ProjectDomainClass::RegionalCircle,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Land,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::CoLM,
                },
                ProjectContractId::OpenLandHex,
            ),
            (
                ProjectDomainClass::RegionalShapefile,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Land,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::CoLM,
                },
                ProjectContractId::BasinLandHex,
            ),
            (
                ProjectDomainClass::Global,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Ocean,
                    cell: MeshCellKind::Tri,
                    model_format: ModelFormat::Fvcom,
                },
                ProjectContractId::MaskedOceanTri,
            ),
            (
                ProjectDomainClass::RegionalShapefile,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Ocean,
                    cell: MeshCellKind::Tri,
                    model_format: ModelFormat::Fvcom,
                },
                ProjectContractId::OpenOceanTri,
            ),
            (
                ProjectDomainClass::Global,
                ProjectTargetTriple {
                    kind: MeshDomainKind::Coupled,
                    cell: MeshCellKind::Hex,
                    model_format: ModelFormat::CoLM,
                },
                ProjectContractId::CoupledHex,
            ),
        ] {
            let key = ProjectCapabilityKey {
                domain,
                target,
                sources: ProjectSourceProfile::none(),
                coordinate_mode: ProjectCoordinateMode::SphericalLonLat,
            };
            assert!(matches!(
                classify_project_capability(key),
                ProjectCapability::Supported { contract_id, .. } if contract_id == expected
            ));
        }
    }

    #[test]
    fn all_presets_map_to_supported_keys_and_intent_does_not_change_lowering() {
        assert_eq!(MeshIntentPreset::all().len(), 12);
        for preset in MeshIntentPreset::all().iter().copied() {
            let key = preset_capability_key(preset);
            assert!(matches!(
                classify_project_capability(key),
                ProjectCapability::Supported { .. }
            ));

            let project = ProjectConfig::scaffold(
                preset.id(),
                preset,
                DomainConfig::Global,
                ResolutionSpec::Nxp(81),
            );
            assert_eq!(ProjectTargetTriple::from(&project.target), key.target);
            let mut different_intent = project.clone();
            different_intent.target.intent = if preset == MeshIntentPreset::Custom {
                MeshIntentPreset::AtmosphereMpas
            } else {
                MeshIntentPreset::Custom
            };
            assert_eq!(
                project.try_lower().expect("preset lowering"),
                different_intent.try_lower().expect("intent-free lowering"),
                "{}",
                preset.id()
            );
        }
    }

    #[test]
    fn all_threshold_fields_lower_and_share_continuous_topology_semantics() {
        assert_eq!(PROJECT_THRESHOLD_FIELDS.len(), 14);
        for field in PROJECT_THRESHOLD_FIELDS {
            assert_eq!(
                threshold_topology_source_atom(field),
                ProjectSourceAtom::ContinuousThreshold
            );

            let mut project = ProjectConfig::scaffold(
                field.stem(),
                MeshIntentPreset::Custom,
                DomainConfig::Global,
                ResolutionSpec::Nxp(81),
            );
            project.refinement.enabled = true;
            project.refinement.threshold_enabled = true;
            project.refinement.max_passes = 1;
            let layer = project
                .data_layers
                .iter_mut()
                .find(|layer| layer.role == ProjectLayerRole::Threshold(field))
                .expect("scaffolded threshold layer");
            layer.path = format!("input/{}.nc", field.stem());
            layer.enabled = true;
            layer.threshold_value = Some(42.0);

            let lowered = project.try_lower().expect("threshold lowering");
            assert!(lowered.data_layers.layers.iter().any(|layer| {
                layer.role == earthmesh_core::DataLayerRole::ThresholdField(field.to_core())
                    && layer.enabled
            }));
            assert_threshold_axis(&lowered.refine, field, 42.0);
            assert!(project
                .effective_threshold_criterion(field, ThresholdStatistic::Mean)
                .is_some());
            assert!(project
                .effective_threshold_criterion(field, ThresholdStatistic::Std)
                .is_some());
        }
    }

    fn assert_threshold_axis(
        refine: &earthmesh_core::RefineConfig,
        field: ThresholdField,
        expected: f64,
    ) {
        match field {
            ThresholdField::Lai => assert_axis(
                &refine.refine_onelayer_lnd,
                &refine.th_onelayer_lnd,
                0,
                expected,
            ),
            ThresholdField::Slope => assert_axis(
                &refine.refine_onelayer_lnd,
                &refine.th_onelayer_lnd,
                2,
                expected,
            ),
            ThresholdField::Dem => assert_axis(
                &refine.refine_onelayer_lnd,
                &refine.th_onelayer_lnd,
                4,
                expected,
            ),
            ThresholdField::SlopeMax => assert_axis(
                &refine.refine_onelayer_lnd,
                &refine.th_onelayer_lnd,
                6,
                expected,
            ),
            ThresholdField::Ks => assert_layer_axis(
                &refine.refine_twolayer_lnd,
                &refine.th_twolayer_lnd,
                0,
                expected,
            ),
            ThresholdField::KSolids => assert_layer_axis(
                &refine.refine_twolayer_lnd,
                &refine.th_twolayer_lnd,
                2,
                expected,
            ),
            ThresholdField::Tkdry => assert_layer_axis(
                &refine.refine_twolayer_lnd,
                &refine.th_twolayer_lnd,
                4,
                expected,
            ),
            ThresholdField::Tksatf => assert_layer_axis(
                &refine.refine_twolayer_lnd,
                &refine.th_twolayer_lnd,
                6,
                expected,
            ),
            ThresholdField::Tksatu => assert_layer_axis(
                &refine.refine_twolayer_lnd,
                &refine.th_twolayer_lnd,
                8,
                expected,
            ),
            ThresholdField::Sst => assert_axis(
                &refine.refine_onelayer_ocn,
                &refine.th_onelayer_ocn,
                0,
                expected,
            ),
            ThresholdField::Ssh => assert_axis(
                &refine.refine_onelayer_ocn,
                &refine.th_onelayer_ocn,
                2,
                expected,
            ),
            ThresholdField::Eke => assert_axis(
                &refine.refine_onelayer_ocn,
                &refine.th_onelayer_ocn,
                4,
                expected,
            ),
            ThresholdField::SeaSlope => assert_axis(
                &refine.refine_onelayer_ocn,
                &refine.th_onelayer_ocn,
                6,
                expected,
            ),
            ThresholdField::Typhoon => assert_axis(
                &refine.refine_onelayer_atmos,
                &refine.th_onelayer_atmos,
                0,
                expected,
            ),
        }
    }

    fn assert_axis<const N: usize>(
        enabled: &[bool; N],
        values: &[f64; N],
        start: usize,
        expected: f64,
    ) {
        assert!(enabled[start]);
        assert!(enabled[start + 1]);
        assert_eq!(values[start], expected);
        assert_eq!(values[start + 1], expected);
    }

    fn assert_layer_axis(
        enabled: &[bool; 10],
        values: &[[f64; 2]; 10],
        start: usize,
        expected: f64,
    ) {
        assert!(enabled[start]);
        assert!(enabled[start + 1]);
        assert_eq!(values[start], [expected; 2]);
        assert_eq!(values[start + 1], [expected; 2]);
    }
}
