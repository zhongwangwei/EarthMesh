//! Typed preflight and rejection telemetry for CEC downstream evaluation.

use super::{extract_coupled_annulus, EssentialCycleKey, HierarchyComponent};
use crate::MotherGrid;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DownstreamRejectStage {
    DomainAdapter,
    StratifiedSectorization,
    DegreeReachability,
    FullPolygonEnumeration,
    GlobalLinkMerge,
    AnchorContract,
    EdgeIncidence,
    SearchIncomplete,
}

impl DownstreamRejectStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DomainAdapter => "DomainAdapter",
            Self::StratifiedSectorization => "StratifiedSectorization",
            Self::DegreeReachability => "DegreeReachability",
            Self::FullPolygonEnumeration => "FullPolygonEnumeration",
            Self::GlobalLinkMerge => "GlobalLinkMerge",
            Self::AnchorContract => "AnchorContract",
            Self::EdgeIncidence => "EdgeIncidence",
            Self::SearchIncomplete => "SearchIncomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownstreamRejectHistogram {
    pub by_stage: BTreeMap<DownstreamRejectStage, u64>,
    pub by_reason: BTreeMap<String, u64>,
    pub first_cycle_by_reason: BTreeMap<String, EssentialCycleKey>,
}

impl DownstreamRejectHistogram {
    pub(crate) fn record(
        &mut self,
        stage: DownstreamRejectStage,
        reason: impl Into<String>,
        cycle: &EssentialCycleKey,
    ) {
        let reason = reason.into();
        *self.by_stage.entry(stage).or_default() += 1;
        *self.by_reason.entry(reason.clone()).or_default() += 1;
        self.first_cycle_by_reason
            .entry(reason)
            .or_insert_with(|| cycle.clone());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownstreamContractStage {
    SourceHierarchy,
    ComponentBoundary,
    FixedOutsideIncidence,
    AnchorIdentity,
    GeometryGuardOnly,
}

impl DownstreamContractStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceHierarchy => "SourceHierarchy",
            Self::ComponentBoundary => "ComponentBoundary",
            Self::FixedOutsideIncidence => "FixedOutsideIncidence",
            Self::AnchorIdentity => "AnchorIdentity",
            Self::GeometryGuardOnly => "GeometryGuardOnly",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownstreamPreflightEvidence {
    pub plan_independent: bool,
    pub geometry_guard_deferred: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownstreamPreflightOutcome {
    Ready(DownstreamPreflightEvidence),
    ContractBlocked {
        stage: DownstreamContractStage,
        evidence: DownstreamPreflightEvidence,
    },
}

pub fn audit_legacy_downstream_preflight(
    source: &MotherGrid,
    component: &HierarchyComponent,
) -> DownstreamPreflightOutcome {
    match extract_coupled_annulus(source, component) {
        Ok(_) => DownstreamPreflightOutcome::Ready(DownstreamPreflightEvidence {
            plan_independent: true,
            geometry_guard_deferred: false,
            reason: None,
        }),
        Err(error) => {
            let reason = format!("{error:?}");
            let geometry_guard_only = reason.contains("inner_guard");
            DownstreamPreflightOutcome::ContractBlocked {
                stage: if geometry_guard_only {
                    DownstreamContractStage::GeometryGuardOnly
                } else {
                    DownstreamContractStage::ComponentBoundary
                },
                evidence: DownstreamPreflightEvidence {
                    plan_independent: true,
                    geometry_guard_deferred: geometry_guard_only,
                    reason: Some(reason),
                },
            }
        }
    }
}

pub(crate) fn classify_downstream_invalid(reason: &str) -> DownstreamRejectStage {
    if reason.contains("stratified annulus rejected") {
        DownstreamRejectStage::DomainAdapter
    } else if reason.contains("degree reachability") {
        DownstreamRejectStage::DegreeReachability
    } else if reason.contains("anchor") {
        DownstreamRejectStage::AnchorContract
    } else if reason.contains("edge") || reason.contains("manifold") {
        DownstreamRejectStage::EdgeIncidence
    } else if reason.contains("sector") || reason.contains("polygon") {
        DownstreamRejectStage::FullPolygonEnumeration
    } else {
        DownstreamRejectStage::GlobalLinkMerge
    }
}
