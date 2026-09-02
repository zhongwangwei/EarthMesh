use crate::content_addressed_stage_key;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityPolicy {
    #[default]
    Legacy,
    DomainExport,
}

impl QualityPolicy {
    pub(crate) fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityDomainKind {
    Global,
    Region,
    Watershed,
    Land,
    Ocean,
    CustomMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityZone {
    TargetCore,
    BoundaryProtection,
    ExportCorridor,
    DeepExterior,
    GlobalNeutral,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceSpec {
    GraphRings(usize),
    Kilometres(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

/// Content identity and topology facts produced by a mask/boundary adapter.
/// Paths are deliberately absent so renaming an input does not change its key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainSource {
    pub content_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub part_sha256: Vec<String>,
    #[serde(default)]
    pub has_holes: bool,
    #[serde(default)]
    pub crosses_antimeridian: bool,
    #[serde(default)]
    pub includes_north_pole: bool,
    #[serde(default)]
    pub includes_south_pole: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpatialQualityDomainSpec {
    Global,
    Region {
        source: DomainSource,
        boundary_protection: DistanceSpec,
        export_halo: DistanceSpec,
    },
    Watershed {
        watershed: DomainSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        river_network: Option<DomainSource>,
        outlets: Vec<LonLat>,
        boundary_protection: DistanceSpec,
        export_halo: DistanceSpec,
    },
    Land {
        land_mask: DomainSource,
        coast_protection: DistanceSpec,
        deep_ocean_start: DistanceSpec,
    },
    Ocean {
        ocean_mask: DomainSource,
        coast_protection: DistanceSpec,
        deep_land_start: DistanceSpec,
    },
    Custom {
        priority_raster: DomainSource,
    },
}

impl SpatialQualityDomainSpec {
    pub fn kind(&self) -> QualityDomainKind {
        match self {
            Self::Global => QualityDomainKind::Global,
            Self::Region { .. } => QualityDomainKind::Region,
            Self::Watershed { .. } => QualityDomainKind::Watershed,
            Self::Land { .. } => QualityDomainKind::Land,
            Self::Ocean { .. } => QualityDomainKind::Ocean,
            Self::Custom { .. } => QualityDomainKind::CustomMask,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialQualityDomain {
    pub spec: SpatialQualityDomainSpec,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub critical_features: Vec<DomainSource>,
    pub working_halo: DistanceSpec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainKey(String);

impl DomainKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainValidationError {
    InvalidSha256(String),
    EmptySourceField(&'static str),
    InvalidDistance(&'static str),
    MixedDistanceUnits,
    InsufficientWorkingHalo,
    DeepExteriorStartsInsideProtection,
    MissingWatershedOutlet,
    InvalidOutlet(usize),
}

impl fmt::Display for DomainValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSha256(value) => write!(f, "invalid domain source SHA-256 {value:?}"),
            Self::EmptySourceField(field) => write!(f, "domain source {field} must not be empty"),
            Self::InvalidDistance(field) => write!(f, "domain {field} must be finite and positive"),
            Self::MixedDistanceUnits => f.write_str("domain distances must use one unit"),
            Self::InsufficientWorkingHalo => {
                f.write_str("domain working_halo must cover boundary protection and export halo")
            }
            Self::DeepExteriorStartsInsideProtection => {
                f.write_str("domain deep exterior must start beyond coast protection")
            }
            Self::MissingWatershedOutlet => f.write_str("watershed domain requires an outlet"),
            Self::InvalidOutlet(index) => write!(f, "watershed outlet {index} is invalid"),
        }
    }
}

impl std::error::Error for DomainValidationError {}

impl SpatialQualityDomain {
    pub fn validate(&self) -> Result<(), DomainValidationError> {
        validate_distance(self.working_halo, "working_halo", false)?;
        for source in &self.critical_features {
            validate_source(source)?;
        }
        match &self.spec {
            SpatialQualityDomainSpec::Global => Ok(()),
            SpatialQualityDomainSpec::Region {
                source,
                boundary_protection,
                export_halo,
            } => {
                validate_source(source)?;
                validate_regional_halo(*boundary_protection, *export_halo, self.working_halo)
            }
            SpatialQualityDomainSpec::Watershed {
                watershed,
                river_network,
                outlets,
                boundary_protection,
                export_halo,
            } => {
                validate_source(watershed)?;
                if let Some(source) = river_network {
                    validate_source(source)?;
                }
                if outlets.is_empty() {
                    return Err(DomainValidationError::MissingWatershedOutlet);
                }
                for (index, outlet) in outlets.iter().enumerate() {
                    if !outlet.lon.is_finite()
                        || !outlet.lat.is_finite()
                        || !(-180.0..=180.0).contains(&outlet.lon)
                        || !(-90.0..=90.0).contains(&outlet.lat)
                    {
                        return Err(DomainValidationError::InvalidOutlet(index));
                    }
                }
                validate_regional_halo(*boundary_protection, *export_halo, self.working_halo)
            }
            SpatialQualityDomainSpec::Land {
                land_mask,
                coast_protection,
                deep_ocean_start,
            } => {
                validate_source(land_mask)?;
                validate_deep_exterior(*coast_protection, *deep_ocean_start)
            }
            SpatialQualityDomainSpec::Ocean {
                ocean_mask,
                coast_protection,
                deep_land_start,
            } => {
                validate_source(ocean_mask)?;
                validate_deep_exterior(*coast_protection, *deep_land_start)
            }
            SpatialQualityDomainSpec::Custom { priority_raster } => {
                validate_source(priority_raster)
            }
        }
    }

    pub fn domain_key(&self) -> Result<DomainKey, DomainValidationError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonicalize_spec(&mut canonical.spec);
        for source in &mut canonical.critical_features {
            canonicalize_source(source);
        }
        canonical.critical_features.sort_by_key(source_key);
        canonical.critical_features.dedup();
        let bytes = serde_json::to_vec(&canonical).expect("quality domain serialization");
        Ok(DomainKey(content_addressed_stage_key(
            "cmrc-dqx-domain-v1",
            &[("domain", &bytes)],
        )))
    }
}

fn validate_source(source: &DomainSource) -> Result<(), DomainValidationError> {
    for hash in std::iter::once(&source.content_sha256).chain(&source.part_sha256) {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DomainValidationError::InvalidSha256(hash.clone()));
        }
    }
    for (field, value) in [
        ("variable", source.variable.as_deref()),
        ("classification", source.classification.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(DomainValidationError::EmptySourceField(field));
        }
    }
    Ok(())
}

fn validate_distance(
    distance: DistanceSpec,
    field: &'static str,
    positive: bool,
) -> Result<(), DomainValidationError> {
    let valid = match distance {
        DistanceSpec::GraphRings(value) => !positive || value > 0,
        DistanceSpec::Kilometres(value) => {
            value.is_finite() && value >= 0.0 && (!positive || value > 0.0)
        }
    };
    valid
        .then_some(())
        .ok_or(DomainValidationError::InvalidDistance(field))
}

fn validate_regional_halo(
    boundary: DistanceSpec,
    export: DistanceSpec,
    working: DistanceSpec,
) -> Result<(), DomainValidationError> {
    validate_distance(boundary, "boundary_protection", true)?;
    validate_distance(export, "export_halo", true)?;
    match (boundary, export, working) {
        (
            DistanceSpec::GraphRings(boundary),
            DistanceSpec::GraphRings(export),
            DistanceSpec::GraphRings(working),
        ) if boundary
            .checked_add(export)
            .is_some_and(|required| working >= required) =>
        {
            Ok(())
        }
        (
            DistanceSpec::Kilometres(boundary),
            DistanceSpec::Kilometres(export),
            DistanceSpec::Kilometres(working),
        ) if working >= boundary + export => Ok(()),
        (DistanceSpec::GraphRings(_), DistanceSpec::GraphRings(_), DistanceSpec::GraphRings(_))
        | (DistanceSpec::Kilometres(_), DistanceSpec::Kilometres(_), DistanceSpec::Kilometres(_)) => {
            Err(DomainValidationError::InsufficientWorkingHalo)
        }
        _ => Err(DomainValidationError::MixedDistanceUnits),
    }
}

fn validate_deep_exterior(
    protection: DistanceSpec,
    deep_start: DistanceSpec,
) -> Result<(), DomainValidationError> {
    validate_distance(protection, "coast_protection", true)?;
    validate_distance(deep_start, "deep_exterior_start", true)?;
    match (protection, deep_start) {
        (DistanceSpec::GraphRings(protection), DistanceSpec::GraphRings(deep))
            if deep > protection =>
        {
            Ok(())
        }
        (DistanceSpec::Kilometres(protection), DistanceSpec::Kilometres(deep))
            if deep > protection =>
        {
            Ok(())
        }
        (DistanceSpec::GraphRings(_), DistanceSpec::GraphRings(_))
        | (DistanceSpec::Kilometres(_), DistanceSpec::Kilometres(_)) => {
            Err(DomainValidationError::DeepExteriorStartsInsideProtection)
        }
        _ => Err(DomainValidationError::MixedDistanceUnits),
    }
}

fn canonicalize_spec(spec: &mut SpatialQualityDomainSpec) {
    match spec {
        SpatialQualityDomainSpec::Global => {}
        SpatialQualityDomainSpec::Region { source, .. } => canonicalize_source(source),
        SpatialQualityDomainSpec::Watershed {
            watershed,
            river_network,
            outlets,
            ..
        } => {
            canonicalize_source(watershed);
            if let Some(source) = river_network {
                canonicalize_source(source);
            }
            for outlet in outlets.iter_mut() {
                outlet.lon = canonical_zero(outlet.lon);
                outlet.lat = canonical_zero(outlet.lat);
            }
            outlets.sort_by(|a, b| a.lon.total_cmp(&b.lon).then(a.lat.total_cmp(&b.lat)));
            outlets.dedup();
        }
        SpatialQualityDomainSpec::Land { land_mask, .. } => canonicalize_source(land_mask),
        SpatialQualityDomainSpec::Ocean { ocean_mask, .. } => canonicalize_source(ocean_mask),
        SpatialQualityDomainSpec::Custom { priority_raster } => {
            canonicalize_source(priority_raster)
        }
    }
}

fn canonicalize_source(source: &mut DomainSource) {
    source.content_sha256.make_ascii_lowercase();
    for hash in &mut source.part_sha256 {
        hash.make_ascii_lowercase();
    }
    source.part_sha256.sort();
    source.part_sha256.dedup();
}

fn source_key(source: &DomainSource) -> Vec<u8> {
    serde_json::to_vec(source).expect("domain source serialization")
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(hash: char) -> DomainSource {
        DomainSource {
            content_sha256: hash.to_string().repeat(64),
            variable: Some("mask".into()),
            classification: Some("positive_is_target".into()),
            part_sha256: vec!["b".repeat(64), "a".repeat(64)],
            has_holes: true,
            crosses_antimeridian: true,
            includes_north_pole: true,
            includes_south_pole: false,
        }
    }

    fn region() -> SpatialQualityDomain {
        SpatialQualityDomain {
            spec: SpatialQualityDomainSpec::Region {
                source: source('c'),
                boundary_protection: DistanceSpec::GraphRings(2),
                export_halo: DistanceSpec::GraphRings(3),
            },
            critical_features: vec![source('e'), source('d')],
            working_halo: DistanceSpec::GraphRings(5),
        }
    }

    #[test]
    fn same_input_produces_stable_domain_key() {
        let mut reordered = region();
        reordered.critical_features.reverse();
        if let SpatialQualityDomainSpec::Region { source, .. } = &mut reordered.spec {
            source.part_sha256.reverse();
            source.content_sha256.make_ascii_uppercase();
        }
        assert_eq!(region().domain_key(), reordered.domain_key());
        assert_eq!(
            region().domain_key().unwrap().as_str(),
            "f3fd589c77d82d917175168b5ed6299a2a66ab6f98f226f69ab860a63118a861"
        );
    }

    #[test]
    fn changed_mask_changes_domain_key() {
        let mut changed = region();
        if let SpatialQualityDomainSpec::Region { source, .. } = &mut changed.spec {
            source.content_sha256 = "f".repeat(64);
        }
        assert_ne!(region().domain_key(), changed.domain_key());
    }

    #[test]
    fn regional_work_halo_is_a_hard_preflight() {
        let mut domain = region();
        domain.working_halo = DistanceSpec::GraphRings(4);
        assert_eq!(
            domain.validate(),
            Err(DomainValidationError::InsufficientWorkingHalo)
        );
        domain.working_halo = DistanceSpec::Kilometres(500.0);
        assert_eq!(
            domain.validate(),
            Err(DomainValidationError::MixedDistanceUnits)
        );
    }

    #[test]
    fn every_domain_kind_has_a_stable_preflight() {
        let cases = [
            SpatialQualityDomainSpec::Global,
            SpatialQualityDomainSpec::Watershed {
                watershed: source('1'),
                river_network: Some(source('2')),
                outlets: vec![LonLat { lon: 0.0, lat: 1.0 }],
                boundary_protection: DistanceSpec::Kilometres(10.0),
                export_halo: DistanceSpec::Kilometres(20.0),
            },
            SpatialQualityDomainSpec::Land {
                land_mask: source('3'),
                coast_protection: DistanceSpec::GraphRings(2),
                deep_ocean_start: DistanceSpec::GraphRings(3),
            },
            SpatialQualityDomainSpec::Ocean {
                ocean_mask: source('4'),
                coast_protection: DistanceSpec::Kilometres(50.0),
                deep_land_start: DistanceSpec::Kilometres(100.0),
            },
            SpatialQualityDomainSpec::Custom {
                priority_raster: source('5'),
            },
        ];
        for spec in cases {
            SpatialQualityDomain {
                spec,
                critical_features: Vec::new(),
                working_halo: DistanceSpec::Kilometres(30.0),
            }
            .domain_key()
            .unwrap();
        }
    }
}
