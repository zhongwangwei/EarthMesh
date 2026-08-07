//! What a criterion says, and what saying it means for when to stop.

/// Why a criterion asks for a finer cell, which decides what satisfies it.
///
/// The distinction is what makes multiple cycles terminate. A slope of twenty
/// degrees stays twenty degrees however fine the mesh gets, so a criterion that
/// reads it can only ever name a target size; treating it as an error to be
/// driven down would refine for ever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CriterionSemantics {
    /// The measured value does not fall as the mesh refines. Satisfied when the
    /// cell reaches the requested scale.
    TargetScale,
    /// The measured value falls as the mesh refines. Satisfied when it is under
    /// tolerance.
    ErrorTolerance,
    /// A feature has to be resolved by enough cells across.
    FeatureCoverage,
    /// The mesh itself is the problem, and moving points may fix it without
    /// adding any.
    MeshQuality,
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
    pub witness: Option<earthmesh_mesh::LonLatDegrees>,
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

    /// Whether this evidence asks the driver to do anything.
    pub fn demands_work(&self) -> bool {
        self.satisfiable && self.normalized_violation > 0.0
    }
}
