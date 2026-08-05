use crate::{MeshCellKind, MeshDomainKind, ModelFormat, ProjectLayerRole, ThresholdField};
use earthmesh_core::{DataLayerRole, ThresholdVar};

impl MeshDomainKind {
    pub(crate) fn engine_str(self) -> &'static str {
        match self {
            MeshDomainKind::Land => "landmesh",
            MeshDomainKind::Ocean => "oceanmesh",
            MeshDomainKind::Atmosphere => "atmosmesh",
            MeshDomainKind::Coupled => "LOCmesh",
            MeshDomainKind::Earth => "earthmesh",
        }
    }
}

impl MeshCellKind {
    pub fn engine_str(self) -> &'static str {
        match self {
            MeshCellKind::Hex => "hex",
            MeshCellKind::Tri => "tri",
        }
    }
}

impl ModelFormat {
    pub fn engine_str(self) -> &'static str {
        match self {
            ModelFormat::CoLM => "CoLM",
            ModelFormat::Icon => "ICON",
            ModelFormat::Mpas => "MPAS",
            ModelFormat::MpasOcean => "MPAS-Ocean",
            ModelFormat::MpasSimple => "MPAS-Simple",
            ModelFormat::Fvcom => "FVCOM",
        }
    }
}

impl ThresholdField {
    pub(crate) fn to_core(self) -> ThresholdVar {
        match self {
            ThresholdField::Lai => ThresholdVar::Lai,
            ThresholdField::Slope => ThresholdVar::Slope,
            ThresholdField::Dem => ThresholdVar::Dem,
            ThresholdField::SlopeMax => ThresholdVar::SlopeMax,
            ThresholdField::Ks => ThresholdVar::Ks,
            ThresholdField::KSolids => ThresholdVar::KSolids,
            ThresholdField::Tkdry => ThresholdVar::Tkdry,
            ThresholdField::Tksatf => ThresholdVar::Tksatf,
            ThresholdField::Tksatu => ThresholdVar::Tksatu,
            ThresholdField::Sst => ThresholdVar::Sst,
            ThresholdField::Ssh => ThresholdVar::Ssh,
            ThresholdField::Eke => ThresholdVar::Eke,
            ThresholdField::SeaSlope => ThresholdVar::SeaSlope,
            ThresholdField::Typhoon => ThresholdVar::Typhoon,
        }
    }
}

impl ProjectLayerRole {
    pub(crate) fn to_core(self) -> DataLayerRole {
        match self {
            ProjectLayerRole::LandType => DataLayerRole::LandType,
            ProjectLayerRole::Threshold(t) => DataLayerRole::ThresholdField(t.to_core()),
            ProjectLayerRole::MeritHydro => DataLayerRole::MeritHydroRoot,
            ProjectLayerRole::Cama => DataLayerRole::CamaReach,
        }
    }
}
