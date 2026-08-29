//! What a criterion says about a cell, and what saying it means for stopping.
//!
//! # Two traits, on purpose
//!
//! `earthmesh_refine_planner` also defines a `RefinementCriterion`. It is not
//! this one and should not be merged into it. That one scores a cell *by index*
//! against a precomputed feature table and allocates a budget across the
//! result; this one evaluates a cell's *geometry* against a data source and
//! returns evidence with a stopping semantics attached. Index-into-a-table and
//! measure-this-polygon are different jobs, and a single trait covering both
//! would serve neither.
//!
//! What was wrong was that they were in different crates with the same name and
//! no note saying so. They are neighbours now, and this is the note.

use earthmesh_mesh::LonLatDegrees;

/// Conservative source footprint covered by one mother-grid patch.
#[derive(Clone, Debug, PartialEq)]
pub struct SphericalPatch {
    pub vertices: Vec<LonLatDegrees>,
}

/// Read-only geometry supplied when a requirement is checked on a final cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CertifiedCellView<'a> {
    pub site: LonLatDegrees,
    pub vertices: &'a [LonLatDegrees],
    pub level: u8,
}

/// Conservative minimum level and the source that forced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementBound {
    pub minimum_level: u8,
    pub source_id: String,
}

/// Whether a physical criterion can participate in strict certification.
#[derive(Clone, Debug, PartialEq)]
pub enum CriterionCertifiability {
    GuaranteedFinite { maximum_level: Option<u8> },
    RequiresSourceResolution { minimum_scale_m: f64 },
    RequiresBoundaryConformity,
    EmpiricalFinalCertificationOnly,
    Unsupported { reason: String },
}

/// Final-cell result kept separate from the planning level bound.
#[derive(Clone, Debug, PartialEq)]
pub struct CriterionCertificate {
    pub criterion_id: String,
    pub passed: bool,
    pub witness: Option<LonLatDegrees>,
    pub residual: f64,
}

/// Shared strict-requirement contract used by CMRC without depending on any
/// existing backend.
pub trait CertifiedRequirementSource {
    fn id(&self) -> &str;
    fn required_level_over(&self, patch: &SphericalPatch) -> RequirementBound;
    fn certify_final_cell(&self, cell: &CertifiedCellView<'_>) -> CriterionCertificate;
    fn certifiability(&self) -> CriterionCertifiability;
}

/// Why a criterion asks for a finer cell, which decides what satisfies it.
///
/// This distinction is what makes repeated cycles terminate. A slope of twenty
/// degrees stays twenty degrees however fine the mesh gets, so a criterion
/// reading it can only name a target size; treating it as an error to drive
/// down would refine for ever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CriterionSemantics {
    /// The measured value does not fall as the mesh refines. Satisfied when the
    /// cell reaches the requested scale.
    TargetScale,
    /// The measured value falls as the mesh refines. Satisfied under tolerance.
    ErrorTolerance,
    /// A feature has to be resolved by enough cells across it.
    FeatureCoverage,
    /// The mesh itself is the problem, and moving points may fix it without
    /// adding any.
    MeshQuality,
}

impl CriterionSemantics {
    /// Whether refining can be expected to reduce the measured value.
    ///
    /// False for `TargetScale`, which is the case a naive loop refines for ever
    /// on.
    pub fn value_falls_with_refinement(self) -> bool {
        matches!(self, Self::ErrorTolerance | Self::FeatureCoverage)
    }
}

/// Why a criterion stopped asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceStopReason {
    AlreadySatisfied,
    /// The cell is at or below what the source data can distinguish. Refining
    /// further would be reading noise.
    SourceResolutionReached,
    MinimumScaleReached,
    UnsupportedGeometry,
    InsufficientData,
}

/// One criterion's answer about one cell.
///
/// Carries the measurement and the threshold, not only the verdict, so an
/// unresolved demand can say how far off it was and a report can be read
/// without re-running anything.
#[derive(Clone, Debug, PartialEq)]
pub struct DemandEvidence {
    pub criterion_id: String,
    pub semantics: CriterionSemantics,
    pub measured_value: f64,
    pub threshold: f64,
    /// How far past the threshold, scaled so criteria can be compared.
    pub normalized_violation: f64,
    /// The cell width this criterion wants, when it can name one.
    pub requested_scale_m: Option<f64>,
    /// Where in the cell the evidence points, for a candidate to be placed.
    pub witness: Option<LonLatDegrees>,
    pub confidence: f64,
    pub source_resolution_m: Option<f64>,
    /// A demand that must be met, not one that would be nice to meet.
    pub hard_requirement: bool,
    /// False when nothing this run can do would satisfy it.
    pub satisfiable: bool,
    pub stop_reason: Option<EvidenceStopReason>,
}

impl DemandEvidence {
    /// Evidence that asks for nothing, because there is nothing to ask for.
    pub fn satisfied(criterion_id: impl Into<String>, semantics: CriterionSemantics) -> Self {
        Self {
            criterion_id: criterion_id.into(),
            semantics,
            measured_value: 0.0,
            threshold: 0.0,
            normalized_violation: 0.0,
            requested_scale_m: None,
            witness: None,
            confidence: 1.0,
            source_resolution_m: None,
            hard_requirement: false,
            satisfiable: true,
            stop_reason: Some(EvidenceStopReason::AlreadySatisfied),
        }
    }

    /// Whether this evidence asks a driver to do anything.
    pub fn demands_work(&self) -> bool {
        self.satisfiable && self.normalized_violation > 0.0
    }
}
