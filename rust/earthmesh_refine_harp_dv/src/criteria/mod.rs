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

use earthmesh_boundary::SphericalBoundaryModel;
use earthmesh_mesh::{
    lonlat_degrees_to_unit_xyz, xyz_to_lonlat_degrees, CartesianPoint, LonLatDegrees, MeshState,
    RefinementRegion, RefinementRegionIndex, VoronoiCell,
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
    /// Any circle in an indexed set.
    ///
    /// Adaptive runs can contain tens of thousands of overlapping demand
    /// circles. Treating each circle as a separate criterion makes every cell
    /// retain one evidence object per circle; the index keeps one criterion
    /// per target level and prunes circles by latitude before distance tests.
    Circles {
        index: RefinementRegionIndex<'static>,
    },
    /// Inside a closed curve, holes and all.
    ///
    /// The shape a closed-curve refinement mask describes. Held as a
    /// [`SphericalBoundaryModel`] rather than as a point list because the
    /// question a criterion asks -- is this cell inside? -- is the model's
    /// subject, and a lake inside an island has to answer "no" without the
    /// criterion knowing what a lake is.
    ///
    /// Before this, a region that was not a circle was refused outright: the
    /// backend served the one shape it could measure and said so rather than
    /// quietly serving less. The model is what makes the other shape
    /// measurable.
    Polygon { boundary: SphericalBoundaryModel },
}

impl TargetRegion {
    pub fn circles(regions: Vec<RefinementRegion>) -> Self {
        Self::Circles {
            index: RefinementRegionIndex::from_owned(regions),
        }
    }

    fn contains(&self, point: LonLatDegrees, radius_m: f64) -> bool {
        match self {
            Self::Global => true,
            Self::Polygon { boundary } => boundary.contains(point.lon_degrees, point.lat_degrees),
            Self::Circles { index } => index.contains_lonlat_great_circle(point, 0),
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

/// Refine while any triangle at a cell has an angle below `min_angle_deg`.
///
/// Section 8.2's `MeshQuality` semantics, and the reason it belongs here rather
/// than in a post-processing pass: quality is a criterion like any other, so it
/// goes through the same demands, the same ladder, the same gates.
///
/// This is Ruppert's algorithm expressed in this backend's terms. Ruppert
/// refines by inserting at the circumcentre of a badly shaped triangle and
/// proves an angle bound for the result; the ladder's second rung is the
/// farthest cell corner, which *is* a circumcentre, and its third is an
/// off-centre -- Ungor's refinement of the same idea, which reaches the bound
/// with fewer points.
///
/// So the machinery for a quality guarantee was already here. What was missing
/// was something to ask for it: with only `TargetScale` running, a badly shaped
/// cell that is small enough never becomes a demand, and nothing downstream
/// ever looks at its shape. Guide 11.12 measured the result -- min angle 17
/// degrees against Method-C's 76.
pub struct MinAngle {
    pub id: String,
    /// The angle every triangle at a cell must reach.
    ///
    /// **Measured to diverge above about 20 degrees, and not for the reason
    /// the textbook bound suggests.** Ruppert terminates only if encroached
    /// boundary segments are split before any circumcentre goes in; nothing
    /// here does that, so near a refinement region's edge the refinement
    /// subdivides without end. A run at 25 degrees produced 6649 degree
    /// refusals, hit the cycle limit, and ended with a degenerate
    /// circumcentre at the writer -- with the degree bound at 7, 9 and 12
    /// alike, so it is the missing encroachment rule and not the degree wall.
    ///
    /// At 20 degrees it is safe and does almost nothing: min angle 32.35
    /// against 32.33 without it. Guide 11.25.
    pub min_angle_deg: f64,
}

/// The smallest angle of a spherical triangle, in degrees, for tests.
#[cfg(test)]
pub(crate) fn smallest_angle_deg_for_test(points: [CartesianPoint; 3]) -> f64 {
    smallest_angle_deg(points)
}

/// The smallest angle of a spherical triangle, in degrees.
///
/// Shared with the transaction gates: a sliver is what the gridfile writer
/// refuses, so the gate that keeps one out measures the same thing the
/// criterion does.
pub(crate) fn smallest_triangle_angle_deg(points: [CartesianPoint; 3]) -> f64 {
    smallest_angle_deg(points)
}

/// The smallest angle of a spherical triangle, in degrees.
fn smallest_angle_deg(points: [CartesianPoint; 3]) -> f64 {
    let mut smallest = f64::MAX;
    for corner in 0..3 {
        let apex = points[corner];
        let a = points[(corner + 1) % 3];
        let b = points[(corner + 2) % 3];
        // The angle at `apex` between the two great-circle arcs leaving it, via
        // the tangent directions there.
        let to = |other: CartesianPoint| {
            let dot = (apex.x * other.x + apex.y * other.y + apex.z * other.z)
                / (apex.x * apex.x + apex.y * apex.y + apex.z * apex.z);
            CartesianPoint::new(
                other.x - apex.x * dot,
                other.y - apex.y * dot,
                other.z - apex.z * dot,
            )
        };
        let (u, v) = (to(a), to(b));
        let lengths =
            (u.x * u.x + u.y * u.y + u.z * u.z).sqrt() * (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        if lengths <= 0.0 {
            return 0.0;
        }
        let cosine = ((u.x * v.x + u.y * v.y + u.z * v.z) / lengths).clamp(-1.0, 1.0);
        smallest = smallest.min(cosine.acos().to_degrees());
    }
    smallest
}

impl CellCriterion for MinAngle {
    fn id(&self) -> &str {
        &self.id
    }

    fn semantics(&self) -> CriterionSemantics {
        CriterionSemantics::MeshQuality
    }

    fn evaluate(&self, cell: &CellView<'_>) -> Result<DemandEvidence> {
        let state = cell.state;
        let mut worst = f64::MAX;
        let mut worst_triangle = None;
        for &triangle in &cell.cell.triangles {
            let corners = state.triangles()[triangle];
            let angle = smallest_angle_deg([
                state.vertices()[corners[0]],
                state.vertices()[corners[1]],
                state.vertices()[corners[2]],
            ]);
            if angle < worst {
                worst = angle;
                worst_triangle = Some(triangle);
            }
        }
        if worst >= self.min_angle_deg || worst_triangle.is_none() {
            return Ok(DemandEvidence::satisfied(&self.id, self.semantics()));
        }

        // The witness is the offending triangle's circumcentre: Ruppert's
        // insertion point, and the one that provably improves the angle rather
        // than merely subdividing near it.
        let witness = worst_triangle
            .and_then(|triangle| state.circumcentre(triangle).ok())
            .map(xyz_to_lonlat_degrees);

        Ok(DemandEvidence {
            criterion_id: self.id.clone(),
            semantics: self.semantics(),
            measured_value: worst,
            threshold: self.min_angle_deg,
            // Falls as the mesh improves, like every other criterion here.
            normalized_violation: (self.min_angle_deg - worst) / self.min_angle_deg,
            requested_scale_m: None,
            witness,
            confidence: 1.0,
            source_resolution_m: None,
            hard_requirement: false,
            satisfiable: true,
            stop_reason: None,
        })
    }
}

#[cfg(test)]
mod tests;
