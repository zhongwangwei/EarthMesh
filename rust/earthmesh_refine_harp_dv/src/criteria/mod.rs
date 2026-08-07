//! Reading a criterion against the cells that exist now.
//!
//! This is what separates HARP-DV from the other two backends. Method-C and
//! red-green turn demand into geometry once -- a raster becomes a circle list,
//! and the mesh is fitted to it. HARP-DV re-reads the criterion against each
//! Voronoi cell after every cycle, so a cell that has become small enough stops
//! asking and a cell whose neighbours moved is measured again.
//!
//! # Why the trait is here and the vocabulary is not
//!
//! [`DemandEvidence`], [`RefinementDemand`] and the rest come from
//! `earthmesh_refine`, where all three backends read them. Per-cell evaluation
//! has exactly one implementor, so it stays here: an abstraction shared by one
//! caller is a guess about the second, and the guess is what makes it wrong
//! when the second arrives.
//!
//! # Effective scale
//!
//! Specification section 14: a cell's effective scale is `sqrt(A / pi)`, the
//! radius of the disc with its area. It is what "this cell is too coarse" is
//! measured in, and what neighbouring-scale balance compares.

use earthmesh_mesh::{
    lonlat_degrees_to_unit_xyz, xyz_to_lonlat_degrees, LonLatDegrees, MeshState, VoronoiCell,
};
use earthmesh_refine::{CriterionSemantics, DemandEvidence, EvidenceStopReason};

use crate::error::Result;

/// One cell, as a criterion sees it.
pub struct CellView<'a> {
    pub site: usize,
    pub cell: &'a VoronoiCell,
    pub state: &'a MeshState,
    /// The sphere's radius here, so an area in steradians becomes one in metres.
    pub radius_m: f64,
}

impl CellView<'_> {
    /// Where the site is.
    pub fn centre(&self) -> LonLatDegrees {
        xyz_to_lonlat_degrees(self.state.vertices()[self.site])
    }

    /// The cell's area in square metres.
    pub fn area_m2(&self) -> Option<f64> {
        self.cell
            .area_on_unit_sphere()
            .map(|area| area * self.radius_m * self.radius_m)
    }

    /// `sqrt(A / pi)`: the radius of the disc with the same area.
    ///
    /// Section 14's effective scale. A length rather than an area, so a target
    /// resolution in metres can be compared against it directly.
    pub fn effective_scale_m(&self) -> Option<f64> {
        self.area_m2()
            .map(|area| (area / std::f64::consts::PI).sqrt())
    }
}

/// Something that can say whether a cell is good enough.
pub trait CellCriterion {
    /// Stable across a run, and what appears in every piece of evidence.
    fn id(&self) -> &str;

    fn semantics(&self) -> CriterionSemantics;

    /// Read this cell. Returning satisfied evidence is how a criterion says
    /// "not here", and is not the same as an error.
    fn evaluate(&self, cell: &CellView<'_>) -> Result<DemandEvidence>;
}

/// Refine until every cell inside a region is at most `target_scale_m` across.
///
/// The simplest of section 8.2's four stopping semantics, and the one that
/// stops on its own: the measured value falls as the mesh refines, so a cell
/// that reaches the target stops asking without anyone tracking that it did.
pub struct TargetScale {
    pub id: String,
    /// The scale a cell must reach, in metres.
    pub target_scale_m: f64,
    /// Where the target applies. A cell whose site is outside is satisfied.
    pub region: TargetRegion,
    /// The finest scale the data behind this target can justify.
    ///
    /// A target below it is a request the data cannot support, and the
    /// evidence says so with `SourceResolutionReached` rather than refining until
    /// a budget runs out.
    pub source_resolution_m: Option<f64>,
}

/// Where a target applies.
pub enum TargetRegion {
    /// Everywhere.
    Global,
    /// Within `radius_m` of a point, measured along the sphere.
    Circle {
        centre: LonLatDegrees,
        radius_m: f64,
    },
}

impl TargetRegion {
    fn contains(&self, point: LonLatDegrees, radius_m: f64) -> bool {
        match self {
            Self::Global => true,
            Self::Circle {
                centre,
                radius_m: reach,
            } => {
                let a = lonlat_degrees_to_unit_xyz(*centre);
                let b = lonlat_degrees_to_unit_xyz(point);
                let dot = (a.x * b.x + a.y * b.y + a.z * b.z).clamp(-1.0, 1.0);
                dot.acos() * radius_m <= *reach
            }
        }
    }
}

impl CellCriterion for TargetScale {
    fn id(&self) -> &str {
        &self.id
    }

    fn semantics(&self) -> CriterionSemantics {
        CriterionSemantics::TargetScale
    }

    fn evaluate(&self, cell: &CellView<'_>) -> Result<DemandEvidence> {
        let centre = cell.centre();
        if !self.region.contains(centre, cell.radius_m) {
            return Ok(DemandEvidence::satisfied(&self.id, self.semantics()));
        }
        let Some(scale) = cell.effective_scale_m() else {
            return Ok(DemandEvidence::satisfied(&self.id, self.semantics()));
        };
        if scale <= self.target_scale_m {
            return Ok(DemandEvidence::satisfied(&self.id, self.semantics()));
        }

        // A target the data cannot justify is reported as unsatisfiable rather
        // than pursued. Refining past what the source can resolve produces a
        // finer mesh carrying no more information, which is the kind of result
        // that looks like success.
        let (satisfiable, stop_reason) = match self.source_resolution_m {
            Some(floor) if self.target_scale_m < floor => {
                (false, Some(EvidenceStopReason::SourceResolutionReached))
            }
            _ => (true, None),
        };

        Ok(DemandEvidence {
            criterion_id: self.id.clone(),
            semantics: self.semantics(),
            measured_value: scale,
            threshold: self.target_scale_m,
            normalized_violation: (scale - self.target_scale_m) / self.target_scale_m,
            requested_scale_m: Some(self.target_scale_m),
            witness: Some(centre),
            confidence: 1.0,
            source_resolution_m: self.source_resolution_m,
            hard_requirement: false,
            satisfiable,
            stop_reason,
        })
    }
}

#[cfg(test)]
mod tests;
