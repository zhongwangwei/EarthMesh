pub mod interval;

use crate::{mother_grid::MotherGrid, outcome::FinalCertificationEvidence};
use earthmesh_mesh::{
    in_circle_on_sphere, magnitude, orientation_on_sphere, CartesianPoint, MeshState, Sign,
};
use interval::{next_down, next_up, Interval};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const GEOMETRY_INTERIOR_MARGIN_DEGREES: f64 = 0.2;
pub const CERTIFICATE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AngleContractId {
    #[default]
    LegacyStrict40To80,
    DomainQuality38To82V1,
}

impl AngleContractId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyStrict40To80 => "legacy_strict_40_to_80",
            Self::DomainQuality38To82V1 => "domain_quality_38_to_82_v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngleWindow {
    pub minimum_degrees: f64,
    pub maximum_degrees: f64,
}

impl AngleWindow {
    pub fn contains_range(self, minimum_degrees: f64, maximum_degrees: f64) -> bool {
        minimum_degrees <= maximum_degrees
            && minimum_degrees >= self.minimum_degrees
            && maximum_degrees <= self.maximum_degrees
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngleContract {
    pub id: AngleContractId,
    pub internal: AngleWindow,
    pub final_delivery: AngleWindow,
    pub preferred: Option<AngleWindow>,
}

impl AngleContract {
    pub const fn for_id(id: AngleContractId) -> Self {
        match id {
            AngleContractId::LegacyStrict40To80 => Self {
                id,
                internal: AngleWindow {
                    minimum_degrees: 40.2,
                    maximum_degrees: 79.8,
                },
                final_delivery: AngleWindow {
                    minimum_degrees: 40.0,
                    maximum_degrees: 80.0,
                },
                preferred: None,
            },
            AngleContractId::DomainQuality38To82V1 => Self {
                id,
                internal: AngleWindow {
                    minimum_degrees: 38.2,
                    maximum_degrees: 81.8,
                },
                final_delivery: AngleWindow {
                    minimum_degrees: 38.0,
                    maximum_degrees: 82.0,
                },
                preferred: Some(AngleWindow {
                    minimum_degrees: 40.0,
                    maximum_degrees: 80.0,
                }),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Certificate {
    pub min_angle_degrees: f64,
    pub max_angle_degrees: f64,
}

impl Certificate {
    pub fn final_delivery() -> Self {
        Self::final_delivery_for(AngleContractId::default())
    }

    pub fn internal() -> Self {
        Self::internal_for(AngleContractId::default())
    }

    pub fn final_delivery_for(id: AngleContractId) -> Self {
        Self::from_window(AngleContract::for_id(id).final_delivery)
    }

    pub fn internal_for(id: AngleContractId) -> Self {
        Self::from_window(AngleContract::for_id(id).internal)
    }

    pub fn angle_contract_id(&self) -> Result<AngleContractId, CertificateError> {
        match (self.min_angle_degrees, self.max_angle_degrees) {
            (40.0, 80.0) | (40.2, 79.8) => Ok(AngleContractId::LegacyStrict40To80),
            (38.0, 82.0) | (38.2, 81.8) => Ok(AngleContractId::DomainQuality38To82V1),
            _ => Err(CertificateError::CriterionNotCertifiable(format!(
                "no registered angle contract for [{}, {}] degrees",
                self.min_angle_degrees, self.max_angle_degrees
            ))),
        }
    }

    fn from_window(window: AngleWindow) -> Self {
        Self {
            min_angle_degrees: window.minimum_degrees,
            max_angle_degrees: window.maximum_degrees,
        }
    }

    pub fn verify_mother_grid(
        &self,
        grid: &MotherGrid,
    ) -> Result<GeometryCertificateReport, CertificateError> {
        let angle_contract_id = self.angle_contract_id()?;
        let angle_gate = SupportedMotherAngleGate::verify(grid.subdivision, &grid.mesh, self)?;
        let topology = topology(&grid.mesh);
        check_topology(&topology)?;
        let delaunay_violations = delaunay_violations(&grid.mesh)?;
        if delaunay_violations != 0 {
            return Err(CertificateError::Delaunay(delaunay_violations));
        }
        let dual = verify_dual(&grid.mesh)?;
        let report = GeometryCertificateReport {
            schema_version: CERTIFICATE_SCHEMA_VERSION,
            angle_contract_id,
            vertices: topology.vertices,
            edges: topology.edges,
            faces: topology.faces,
            euler: topology.euler,
            charge: topology.charge,
            min_angle_degrees: angle_gate.observed_min_degrees,
            max_angle_degrees: angle_gate.observed_max_degrees,
            angle_gate: Some(angle_gate),
            open_edges: topology.open_edges,
            topology_errors: topology_error_count(&topology),
            degree_outside_window: topology.bad_degrees.len(),
            delaunay_violations,
            voronoi_cells: dual.cells,
            voronoi_invalid_cells: dual.invalid_cells,
            voronoi_reciprocal_errors: dual.reciprocal_errors,
        };
        report.require_geometry_gates()?;
        Ok(report)
    }

    pub fn verify_geometry(
        &self,
        mesh: &MeshState,
    ) -> Result<GeometryCertificateReport, CertificateError> {
        let angle_contract_id = self.angle_contract_id()?;
        prove_angle_window_with_outward_intervals(mesh, self)?;
        let angles = fast_angle_filter(mesh)?;
        if angles.min < self.min_angle_degrees || angles.max > self.max_angle_degrees {
            return Err(CertificateError::AngleOutOfRange {
                min_angle: angles.min,
                max_angle: angles.max,
            });
        }
        let topology = topology(mesh);
        check_topology(&topology)?;
        let delaunay_violations = delaunay_violations(mesh)?;
        if delaunay_violations != 0 {
            return Err(CertificateError::Delaunay(delaunay_violations));
        }
        let dual = verify_dual(mesh)?;
        let report = GeometryCertificateReport {
            schema_version: CERTIFICATE_SCHEMA_VERSION,
            angle_contract_id,
            vertices: topology.vertices,
            edges: topology.edges,
            faces: topology.faces,
            euler: topology.euler,
            charge: topology.charge,
            min_angle_degrees: angles.min,
            max_angle_degrees: angles.max,
            angle_gate: None,
            open_edges: topology.open_edges,
            topology_errors: topology_error_count(&topology),
            degree_outside_window: topology.bad_degrees.len(),
            delaunay_violations,
            voronoi_cells: dual.cells,
            voronoi_invalid_cells: dual.invalid_cells,
            voronoi_reciprocal_errors: dual.reciprocal_errors,
        };
        report.require_geometry_gates()?;
        Ok(report)
    }

    pub fn verify_geometry_region(
        &self,
        mesh: &MeshState,
        faces: &BTreeSet<usize>,
    ) -> Result<GeometryRegionCertificateReport, CertificateError> {
        if faces.is_empty() {
            return Err(CertificateError::CriterionNotCertifiable(
                "geometry region is empty".to_string(),
            ));
        }
        if let Some(&face) = faces.iter().find(|&&face| !mesh.is_triangle_live(face)) {
            return Err(CertificateError::CriterionNotCertifiable(format!(
                "geometry region contains inactive face {face}"
            )));
        }

        let interval_boxes = prove_angle_window_for_triangles(mesh, self, faces.iter().copied())?;
        let angles = angle_filter_for_triangles(mesh, faces.iter().copied())?;
        if angles.min < self.min_angle_degrees || angles.max > self.max_angle_degrees {
            return Err(CertificateError::AngleOutOfRange {
                min_angle: angles.min,
                max_angle: angles.max,
            });
        }

        let mut orientation_errors = 0;
        for &face in faces {
            let [a, b, c] = mesh.triangles()[face].map(|site| mesh.vertices()[site]);
            match orientation_on_sphere(a, b, c) {
                Ok(Sign::Positive) => {}
                Ok(Sign::Negative | Sign::Zero) => orientation_errors += 1,
                Err(e) => return Err(CertificateError::GeometricPredicate(e.to_string())),
            }
        }

        let sites = mesh.sites_touching(faces);
        let degree_outside_window = local_degree_errors(mesh, &sites)?;
        let delaunay_violations = local_delaunay_violations(mesh, &sites)?;
        let dual = verify_dual_sites(mesh, &sites)?;
        let report = GeometryRegionCertificateReport {
            faces: faces.len(),
            sites: sites.len(),
            edges: local_incident_edges(mesh, &sites)?.len(),
            interval_boxes,
            min_angle_degrees: angles.min,
            max_angle_degrees: angles.max,
            orientation_errors,
            degree_outside_window,
            delaunay_violations,
            voronoi_cells: dual.cells,
            voronoi_invalid_cells: dual.invalid_cells,
            voronoi_reciprocal_errors: dual.reciprocal_errors,
        };
        report.require_geometry_gates()?;
        Ok(report)
    }

    pub(crate) fn geometry_region_passes(&self, mesh: &MeshState, faces: &BTreeSet<usize>) -> bool {
        self.geometry_penalty_for_region(
            mesh,
            faces,
            self.min_angle_degrees,
            self.max_angle_degrees,
        ) == Some(0.0)
    }

    pub(crate) fn geometry_penalty_in(
        &self,
        mesh: &MeshState,
        faces: &BTreeSet<usize>,
    ) -> Option<f64> {
        self.geometry_penalty_for_region(
            mesh,
            faces,
            self.min_angle_degrees + GEOMETRY_INTERIOR_MARGIN_DEGREES,
            self.max_angle_degrees - GEOMETRY_INTERIOR_MARGIN_DEGREES,
        )
    }

    fn geometry_penalty_for_region(
        &self,
        mesh: &MeshState,
        faces: &BTreeSet<usize>,
        minimum_angle: f64,
        maximum_angle: f64,
    ) -> Option<f64> {
        let mut penalty = 0.0;
        let mut seeds = Vec::with_capacity(faces.len().saturating_mul(3));
        for &triangle in faces {
            if !mesh.is_triangle_live(triangle) {
                continue;
            }
            let corners = mesh.triangles()[triangle];
            for vertex in corners {
                seeds.push((vertex, triangle));
            }
            for angle in spherical_triangle_angles(corners.map(|vertex| mesh.vertices()[vertex]))? {
                let violation = (minimum_angle - angle)
                    .max(0.0)
                    .max((angle - maximum_angle).max(0.0));
                penalty += violation * violation;
            }
        }
        seeds.sort_unstable_by_key(|&(site, _)| site);
        seeds.dedup_by_key(|(site, _)| *site);
        for (site, seed) in seeds {
            let degree = mesh.triangle_fan_from(site, seed).ok()?.len();
            let violation = 5usize.saturating_sub(degree).max(degree.saturating_sub(7));
            penalty += 10_000.0 * (violation * violation) as f64;
        }
        Some(penalty)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngleGateReport {
    pub supported_subdivision: usize,
    /// Diagnostic only; the proof does not depend on these scanned extrema.
    pub observed_min_degrees: f64,
    pub observed_max_degrees: f64,
    pub proof_method: &'static str,
}

struct SupportedMotherAngleGate;

impl SupportedMotherAngleGate {
    const SUPPORTED: [usize; 13] = [1, 2, 3, 4, 6, 8, 12, 20, 40, 80, 160, 320, 640];

    fn verify(
        n: usize,
        mesh: &MeshState,
        certificate: &Certificate,
    ) -> Result<AngleGateReport, CertificateError> {
        if !Self::SUPPORTED.contains(&n) {
            return Err(CertificateError::UnsupportedMotherSubdivision(n));
        }
        prove_angle_window_with_outward_intervals(mesh, certificate)?;
        let angles = fast_angle_filter(mesh)?;
        let report = AngleGateReport {
            supported_subdivision: n,
            observed_min_degrees: angles.min,
            observed_max_degrees: angles.max,
            proof_method: "runtime outward interval threshold proof",
        };
        Ok(report)
    }
}

#[derive(Clone, Copy)]
struct IntervalPoint {
    x: Interval,
    y: Interval,
    z: Interval,
}

impl IntervalPoint {
    fn exact(point: CartesianPoint) -> Self {
        Self {
            x: Interval::point(point.x),
            y: Interval::point(point.y),
            z: Interval::point(point.z),
        }
    }

    fn scaled(self, scale: Interval) -> Self {
        Self {
            x: self.x.mul_out(scale),
            y: self.y.mul_out(scale),
            z: self.z.mul_out(scale),
        }
    }

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x.sub_out(rhs.x),
            y: self.y.sub_out(rhs.y),
            z: self.z.sub_out(rhs.z),
        }
    }

    fn dot(self, rhs: Self) -> Interval {
        self.x
            .mul_out(rhs.x)
            .add_out(self.y.mul_out(rhs.y))
            .add_out(self.z.mul_out(rhs.z))
    }
}

fn prove_angle_window_with_outward_intervals(
    mesh: &MeshState,
    certificate: &Certificate,
) -> Result<(), CertificateError> {
    prove_angle_window_for_triangles(mesh, certificate, mesh.active_triangle_slots()).map(|_| ())
}

fn prove_angle_window_for_triangles(
    mesh: &MeshState,
    certificate: &Certificate,
    triangles: impl IntoIterator<Item = usize>,
) -> Result<usize, CertificateError> {
    // Decimal constants are deliberately inside/outside the true cosine
    // values, respectively. This avoids relying on libm trigonometric
    // rounding inside the proof; only IEEE-754 +,-,* and nextafter widening
    // participate in the interval comparisons.
    let (min_angle_cos_lower, max_angle_cos_upper) = outward_interval_thresholds(certificate)?;
    let min_cos_sq_lower = next_down(min_angle_cos_lower * min_angle_cos_lower);
    let max_cos_sq_upper = next_up(max_angle_cos_upper * max_angle_cos_upper);
    let mut interval_boxes = 0;
    for triangle in triangles {
        let corners = mesh.triangles()[triangle];
        for corner in 0..3 {
            let a = IntervalPoint::exact(mesh.vertices()[corners[corner]]);
            let b = IntervalPoint::exact(mesh.vertices()[corners[(corner + 1) % 3]]);
            let c = IntervalPoint::exact(mesh.vertices()[corners[(corner + 2) % 3]]);
            let aa = a.dot(a);
            let tangent_b = b.scaled(aa).sub(a.scaled(a.dot(b)));
            let tangent_c = c.scaled(aa).sub(a.scaled(a.dot(c)));
            let dot = tangent_b.dot(tangent_c);
            let norm_b_sq = tangent_b.dot(tangent_b);
            let norm_c_sq = tangent_c.dot(tangent_c);
            interval_boxes += 1;
            if dot.lo <= 0.0 || norm_b_sq.lo <= 0.0 || norm_c_sq.lo <= 0.0 {
                return Err(CertificateError::CriterionNotCertifiable(format!(
                    "triangle {triangle} corner {corner} has an interval crossing a degenerate or non-acute angle"
                )));
            }
            let dot_sq_lower = next_down(dot.lo * dot.lo);
            let dot_sq_upper = next_up(dot.hi * dot.hi);
            let norms_lower = next_down(norm_b_sq.lo * norm_c_sq.lo);
            let norms_upper = next_up(norm_b_sq.hi * norm_c_sq.hi);
            let minimum_angle_rhs = next_down(min_cos_sq_lower * norms_lower);
            let maximum_angle_rhs = next_up(max_cos_sq_upper * norms_upper);
            if dot_sq_upper > minimum_angle_rhs || dot_sq_lower < maximum_angle_rhs {
                return Err(CertificateError::CriterionNotCertifiable(format!(
                    "triangle {triangle} corner {corner} interval cannot prove the [{}, {}] degree window",
                    certificate.min_angle_degrees, certificate.max_angle_degrees
                )));
            }
        }
    }
    Ok(interval_boxes)
}

fn outward_interval_thresholds(certificate: &Certificate) -> Result<(f64, f64), CertificateError> {
    match (certificate.min_angle_degrees, certificate.max_angle_degrees) {
        (40.0, 80.0) => Ok((0.766_044_443_118, 0.173_648_177_667)),
        (40.2, 79.8) => Ok((0.763_796_028_634, 0.177_084_740_320)),
        (38.0, 82.0) => Ok((0.788_010_753_606, 0.139_173_100_961)),
        (38.2, 81.8) => Ok((0.785_856_893_175, 0.142_628_933_706)),
        _ => Err(CertificateError::CriterionNotCertifiable(format!(
            "no outward interval threshold constants for [{}, {}] degrees",
            certificate.min_angle_degrees, certificate.max_angle_degrees
        ))),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryRegionCertificateReport {
    pub faces: usize,
    pub sites: usize,
    pub edges: usize,
    pub interval_boxes: usize,
    pub min_angle_degrees: f64,
    pub max_angle_degrees: f64,
    pub orientation_errors: usize,
    pub degree_outside_window: usize,
    pub delaunay_violations: usize,
    pub voronoi_cells: usize,
    pub voronoi_invalid_cells: usize,
    pub voronoi_reciprocal_errors: usize,
}

impl GeometryRegionCertificateReport {
    pub fn require_geometry_gates(&self) -> Result<(), CertificateError> {
        let failed = self.orientation_errors
            + self.degree_outside_window
            + self.delaunay_violations
            + self.voronoi_invalid_cells
            + self.voronoi_reciprocal_errors;
        if failed == 0 {
            Ok(())
        } else {
            Err(CertificateError::GeometryGateResiduals(failed))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometryCertificateReport {
    pub schema_version: u32,
    pub angle_contract_id: AngleContractId,
    pub vertices: usize,
    pub edges: usize,
    pub faces: usize,
    pub euler: isize,
    pub charge: isize,
    pub min_angle_degrees: f64,
    pub max_angle_degrees: f64,
    pub angle_gate: Option<AngleGateReport>,
    pub open_edges: usize,
    pub topology_errors: usize,
    pub degree_outside_window: usize,
    pub delaunay_violations: usize,
    pub voronoi_cells: usize,
    pub voronoi_invalid_cells: usize,
    pub voronoi_reciprocal_errors: usize,
}

impl GeometryCertificateReport {
    pub fn into_final(
        self,
        evidence: FinalCertificationEvidence,
    ) -> Result<FinalCertificateReport, CertificateError> {
        let report = FinalCertificateReport {
            geometry: self,
            physical_residuals: evidence.physical_residuals,
            balance_residuals: evidence.balance_residuals,
            remap_closure_errors: evidence.remap_closure_errors,
        };
        report.require_final_gates()?;
        Ok(report)
    }

    pub fn require_geometry_gates(&self) -> Result<(), CertificateError> {
        let failed = self.open_edges
            + self.topology_errors
            + self.degree_outside_window
            + self.delaunay_violations
            + self.voronoi_invalid_cells
            + self.voronoi_reciprocal_errors;
        if failed == 0 {
            Ok(())
        } else {
            Err(CertificateError::GeometryGateResiduals(failed))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FinalCertificateReport {
    pub geometry: GeometryCertificateReport,
    pub physical_residuals: usize,
    pub balance_residuals: usize,
    pub remap_closure_errors: usize,
}

impl FinalCertificateReport {
    pub fn require_final_gates(&self) -> Result<(), CertificateError> {
        self.geometry.require_geometry_gates()?;
        let failed = self.physical_residuals + self.balance_residuals + self.remap_closure_errors;
        if failed == 0 {
            Ok(())
        } else {
            Err(CertificateError::FinalGateResiduals(failed))
        }
    }
}

pub type CertificateReport = GeometryCertificateReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCertificate {
    residuals: usize,
}

impl PhysicalCertificate {
    pub fn certify_uniform_level(
        required_levels: &[usize],
        delivered_level: usize,
    ) -> Result<Self, CertificateError> {
        let residuals = required_levels
            .iter()
            .filter(|&&required| delivered_level < required)
            .count();
        if residuals == 0 {
            Ok(Self { residuals })
        } else {
            Err(CertificateError::PhysicalResiduals(residuals))
        }
    }

    pub fn residuals(&self) -> usize {
        self.residuals
    }

    pub fn from_final_cells(
        report: &crate::requirement::FinalCellRequirementReport,
    ) -> Result<Self, CertificateError> {
        let residuals = report.physical_residuals();
        if residuals == 0 {
            Ok(Self { residuals })
        } else {
            Err(CertificateError::PhysicalResiduals(residuals))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceCertificate {
    residuals: usize,
}

impl BalanceCertificate {
    pub fn certify_levels_cover_envelope(
        delivered_levels: &[usize],
        envelope: &[usize],
    ) -> Result<Self, CertificateError> {
        if delivered_levels.len() != envelope.len() {
            return Err(CertificateError::BalanceResiduals(
                delivered_levels.len().abs_diff(envelope.len()),
            ));
        }
        let residuals = delivered_levels
            .iter()
            .zip(envelope)
            .filter(|&(&delivered, &required)| delivered < required)
            .count();
        if residuals == 0 {
            Ok(Self { residuals })
        } else {
            Err(CertificateError::BalanceResiduals(residuals))
        }
    }

    pub fn residuals(&self) -> usize {
        self.residuals
    }

    pub fn from_final_cells(
        report: &crate::requirement::FinalCellRequirementReport,
    ) -> Result<Self, CertificateError> {
        let residuals = report.balance_residuals();
        if residuals == 0 {
            Ok(Self { residuals })
        } else {
            Err(CertificateError::BalanceResiduals(residuals))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CertificateError {
    DegenerateTriangle { triangle: usize },
    AngleOutOfRange { min_angle: f64, max_angle: f64 },
    UnsupportedMotherSubdivision(usize),
    CriterionNotCertifiable(String),
    GeometryGateResiduals(usize),
    FinalGateResiduals(usize),
    PhysicalResiduals(usize),
    BalanceResiduals(usize),
    RemapRows { expected: usize, actual: usize },
    EvidenceMeshMismatch,
    OpenEdges(usize),
    Euler(isize),
    Charge(isize),
    Degree { vertex: usize, degree: usize },
    Delaunay(usize),
    GeometricPredicate(String),
    Dual(String),
}

impl std::fmt::Display for CertificateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DegenerateTriangle { triangle } => write!(f, "triangle {triangle} is degenerate"),
            Self::AngleOutOfRange {
                min_angle,
                max_angle,
            } => write!(
                f,
                "triangle angles [{min_angle}, {max_angle}] are outside the certificate window"
            ),
            Self::UnsupportedMotherSubdivision(n) => write!(
                f,
                "mother subdivision n={n} is not in the certified support table"
            ),
            Self::CriterionNotCertifiable(reason) => f.write_str(reason),
            Self::GeometryGateResiduals(n) => write!(f, "geometry certificate has {n} residuals"),
            Self::FinalGateResiduals(n) => {
                write!(f, "final certificate has {n} hard-gate residuals")
            }
            Self::PhysicalResiduals(n) => write!(f, "physical certificate has {n} residuals"),
            Self::BalanceResiduals(n) => write!(f, "balance certificate has {n} residuals"),
            Self::RemapRows { expected, actual } => write!(
                f,
                "remap has {actual} target rows, but the certified Voronoi mesh has {expected} cells"
            ),
            Self::EvidenceMeshMismatch => {
                f.write_str("final-cell/remap evidence targets a different mesh")
            }
            Self::OpenEdges(n) => write!(f, "mesh has {n} open directed edges"),
            Self::Euler(v) => write!(f, "Euler characteristic is {v}, not 2"),
            Self::Charge(v) => write!(f, "degree charge is {v}, not 12"),
            Self::Degree { vertex, degree } => {
                write!(f, "vertex {vertex} has degree {degree}, outside 5..=7")
            }
            Self::Delaunay(n) => write!(f, "mesh has {n} spherical Delaunay violations"),
            Self::GeometricPredicate(e) | Self::Dual(e) => f.write_str(e),
        }
    }
}
impl std::error::Error for CertificateError {}

struct Topology {
    vertices: usize,
    edges: usize,
    faces: usize,
    euler: isize,
    charge: isize,
    open_edges: usize,
    bad_degrees: Vec<(usize, usize)>,
}

fn check_topology(topology: &Topology) -> Result<(), CertificateError> {
    if topology.open_edges != 0 {
        return Err(CertificateError::OpenEdges(topology.open_edges));
    }
    if topology.euler != 2 {
        return Err(CertificateError::Euler(topology.euler));
    }
    if topology.charge != 12 {
        return Err(CertificateError::Charge(topology.charge));
    }
    if let Some((vertex, degree)) = topology.bad_degrees.first().copied() {
        return Err(CertificateError::Degree { vertex, degree });
    }
    Ok(())
}

fn topology_error_count(topology: &Topology) -> usize {
    usize::from(topology.open_edges != 0 || topology.euler != 2 || topology.charge != 12)
}

fn topology(mesh: &MeshState) -> Topology {
    let mut degrees = vec![0usize; mesh.vertices().len()];
    let mut faces = 0usize;
    let mut edge_count = 0usize;
    let mut open_edges = 0usize;
    for triangle in mesh.active_triangle_slots() {
        faces += 1;
        let [a, b, c] = mesh.triangles()[triangle];
        for (corner, u) in [a, b, c].into_iter().enumerate() {
            degrees[u] += 1;
            let other = mesh.neighbours()[triangle][corner];
            if other == 0 {
                open_edges += 1;
                edge_count += 1;
            } else if triangle < other {
                edge_count += 1;
            }
        }
    }
    let bad_degrees = degrees
        .iter()
        .enumerate()
        .filter_map(|(v, &d)| (d != 0 && !(5..=7).contains(&d)).then_some((v, d)))
        .collect();
    let vertices = mesh.vertex_count();
    Topology {
        vertices,
        edges: edge_count,
        faces,
        euler: vertices as isize - edge_count as isize + faces as isize,
        charge: degrees
            .iter()
            .filter(|&&d| d != 0)
            .map(|&d| 6isize - d as isize)
            .sum(),
        open_edges,
        bad_degrees,
    }
}

struct AngleSummary {
    min: f64,
    max: f64,
}

fn fast_angle_filter(mesh: &MeshState) -> Result<AngleSummary, CertificateError> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for triangle in mesh.active_triangle_slots() {
        let corners = mesh.triangles()[triangle];
        let angles = spherical_triangle_angles(corners.map(|v| mesh.vertices()[v]))
            .ok_or(CertificateError::DegenerateTriangle { triangle })?;
        for angle in angles {
            min = min.min(angle);
            max = max.max(angle);
        }
    }
    Ok(AngleSummary { min, max })
}

fn angle_filter_for_triangles(
    mesh: &MeshState,
    triangles: impl IntoIterator<Item = usize>,
) -> Result<AngleSummary, CertificateError> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for triangle in triangles {
        let corners = mesh.triangles()[triangle];
        let angles = spherical_triangle_angles(corners.map(|v| mesh.vertices()[v]))
            .ok_or(CertificateError::DegenerateTriangle { triangle })?;
        for angle in angles {
            min = min.min(angle);
            max = max.max(angle);
        }
    }
    Ok(AngleSummary { min, max })
}

fn local_degree_errors(
    mesh: &MeshState,
    sites: &BTreeMap<usize, usize>,
) -> Result<usize, CertificateError> {
    let mut errors = 0;
    for (&site, &seed) in sites {
        let degree = mesh
            .triangle_fan_from(site, seed)
            .map_err(|e| CertificateError::Dual(e.to_string()))?
            .len();
        if !(5..=7).contains(&degree) {
            errors += 1;
        }
    }
    Ok(errors)
}

fn local_incident_edges(
    mesh: &MeshState,
    sites: &BTreeMap<usize, usize>,
) -> Result<BTreeSet<(usize, usize)>, CertificateError> {
    let mut edges = BTreeSet::new();
    for (&site, &seed) in sites {
        for triangle in mesh
            .triangle_fan_from(site, seed)
            .map_err(|e| CertificateError::Dual(e.to_string()))?
        {
            for &other in &mesh.triangles()[triangle] {
                if other != site {
                    edges.insert(if site < other {
                        (site, other)
                    } else {
                        (other, site)
                    });
                }
            }
        }
    }
    Ok(edges)
}

fn local_delaunay_violations(
    mesh: &MeshState,
    sites: &BTreeMap<usize, usize>,
) -> Result<usize, CertificateError> {
    let mut checked = BTreeSet::new();
    let mut violations = 0;
    for (&site, &seed) in sites {
        for triangle in mesh
            .triangle_fan_from(site, seed)
            .map_err(|e| CertificateError::Dual(e.to_string()))?
        {
            let tri = mesh.triangles()[triangle];
            for corner in 0..3 {
                let other = mesh.neighbours()[triangle][corner];
                if other == 0 || !mesh.is_triangle_live(other) {
                    continue;
                }
                let edge = [tri[(corner + 1) % 3], tri[(corner + 2) % 3]];
                if !edge.contains(&site) {
                    continue;
                }
                let key = if edge[0] < edge[1] {
                    (edge[0], edge[1])
                } else {
                    (edge[1], edge[0])
                };
                if !checked.insert(key) {
                    continue;
                }
                let opposite = mesh.triangles()[other]
                    .iter()
                    .copied()
                    .find(|v| !edge.contains(v));
                if let Some(d) = opposite {
                    match in_circle_on_sphere(
                        mesh.vertices()[tri[0]],
                        mesh.vertices()[tri[1]],
                        mesh.vertices()[tri[2]],
                        mesh.vertices()[d],
                    ) {
                        Ok(Sign::Positive) => violations += 1,
                        Ok(Sign::Negative | Sign::Zero) => {}
                        Err(e) => return Err(CertificateError::GeometricPredicate(e.to_string())),
                    }
                }
            }
        }
    }
    Ok(violations)
}

fn verify_dual_sites(
    mesh: &MeshState,
    sites: &BTreeMap<usize, usize>,
) -> Result<DualReport, CertificateError> {
    let mut invalid_cells = 0;
    let mut reciprocal_errors = 0;
    for (&site, &seed) in sites {
        let cell = mesh
            .voronoi_cell_from(site, seed)
            .map_err(|e| CertificateError::Dual(e.to_string()))?;
        if !(5..=7).contains(&cell.degree())
            || cell.area_on_unit_sphere().unwrap_or(0.0) <= 0.0
            || !voronoi_cell_is_convex_and_contains_site(mesh, &cell)
        {
            invalid_cells += 1;
        }
        for (&triangle, &center) in cell.triangles.iter().zip(&cell.corners) {
            if !mesh.triangles()[triangle].contains(&site) {
                reciprocal_errors += 1;
            }
            let corners = mesh.triangles()[triangle].map(|p| mesh.vertices()[p]);
            let ds = corners.map(|p| chord(center, p));
            let scale = ds[0].abs().max(ds[1].abs()).max(ds[2].abs()).max(1.0);
            if (ds[0] - ds[1]).abs() > 1.0e-10 * scale
                || (ds[0] - ds[2]).abs() > 1.0e-10 * scale
                || ds[0] <= 0.0
            {
                reciprocal_errors += 1;
            }
        }
    }
    if invalid_cells != 0 || reciprocal_errors != 0 {
        return Err(CertificateError::Dual(format!(
            "invalid Voronoi cells={invalid_cells}, reciprocal errors={reciprocal_errors}"
        )));
    }
    Ok(DualReport {
        cells: sites.len(),
        invalid_cells,
        reciprocal_errors,
    })
}

fn delaunay_violations(mesh: &MeshState) -> Result<usize, CertificateError> {
    let mut violations = 0;
    for triangle in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[triangle];
        for corner in 0..3 {
            let other = mesh.neighbours()[triangle][corner];
            if other == 0 || triangle > other {
                continue;
            }
            let edge = [tri[(corner + 1) % 3], tri[(corner + 2) % 3]];
            let opposite = mesh.triangles()[other]
                .iter()
                .copied()
                .find(|v| !edge.contains(v));
            if let Some(d) = opposite {
                match in_circle_on_sphere(
                    mesh.vertices()[tri[0]],
                    mesh.vertices()[tri[1]],
                    mesh.vertices()[tri[2]],
                    mesh.vertices()[d],
                ) {
                    Ok(Sign::Positive) => violations += 1,
                    Ok(Sign::Negative | Sign::Zero) => {}
                    Err(e) => return Err(CertificateError::GeometricPredicate(e.to_string())),
                }
            }
        }
    }
    Ok(violations)
}

struct DualReport {
    cells: usize,
    invalid_cells: usize,
    reciprocal_errors: usize,
}

fn verify_dual(mesh: &MeshState) -> Result<DualReport, CertificateError> {
    let mut seeds = vec![None; mesh.vertices().len()];
    for triangle in mesh.active_triangle_slots() {
        for site in mesh.triangles()[triangle] {
            seeds[site].get_or_insert(triangle);
        }
    }

    let mut invalid_cells = 0;
    let mut reciprocal_errors = 0;
    let mut cells = 0;
    for site in mesh.active_vertex_slots() {
        let seed = seeds[site]
            .ok_or_else(|| CertificateError::Dual(format!("site {site} is in no triangle")))?;
        let cell = mesh
            .voronoi_cell_from(site, seed)
            .map_err(|e| CertificateError::Dual(e.to_string()))?;
        cells += 1;
        if !(5..=7).contains(&cell.degree())
            || cell.area_on_unit_sphere().unwrap_or(0.0) <= 0.0
            || !voronoi_cell_is_convex_and_contains_site(mesh, &cell)
        {
            invalid_cells += 1;
        }
        for (&triangle, &center) in cell.triangles.iter().zip(&cell.corners) {
            if !mesh.triangles()[triangle].contains(&site) {
                reciprocal_errors += 1;
            }
            let corners = mesh.triangles()[triangle].map(|v| mesh.vertices()[v]);
            let ds = corners.map(|p| chord(center, p));
            let scale = ds[0].abs().max(ds[1].abs()).max(ds[2].abs()).max(1.0);
            if (ds[0] - ds[1]).abs() > 1.0e-10 * scale
                || (ds[0] - ds[2]).abs() > 1.0e-10 * scale
                || ds[0] <= 0.0
            {
                reciprocal_errors += 1;
            }
        }
    }
    if invalid_cells != 0 || reciprocal_errors != 0 {
        return Err(CertificateError::Dual(format!(
            "invalid Voronoi cells={invalid_cells}, reciprocal errors={reciprocal_errors}"
        )));
    }
    Ok(DualReport {
        cells,
        invalid_cells,
        reciprocal_errors,
    })
}

pub(crate) fn voronoi_cell_is_convex_and_contains_site(
    mesh: &MeshState,
    cell: &earthmesh_mesh::VoronoiCell,
) -> bool {
    let normalize = |point: CartesianPoint| {
        let norm = magnitude(point);
        (norm > 0.0).then(|| CartesianPoint::new(point.x / norm, point.y / norm, point.z / norm))
    };
    let Some(site) = normalize(mesh.vertices()[cell.site]) else {
        return false;
    };
    let Some(corners) = cell
        .corners
        .iter()
        .copied()
        .map(normalize)
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    for index in 0..corners.len() {
        let left = corners[index];
        let right = corners[(index + 1) % corners.len()];
        let normal = CartesianPoint::new(
            left.y * right.z - left.z * right.y,
            left.z * right.x - left.x * right.z,
            left.x * right.y - left.y * right.x,
        );
        let side = normal.x * site.x + normal.y * site.y + normal.z * site.z;
        if side.abs() <= 1.0e-14
            || corners.iter().any(|corner| {
                side.signum() * (normal.x * corner.x + normal.y * corner.y + normal.z * corner.z)
                    < -1.0e-12
            })
        {
            return false;
        }
    }
    true
}

fn chord(a: earthmesh_mesh::CartesianPoint, b: earthmesh_mesh::CartesianPoint) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

pub(crate) fn spherical_triangle_angles(
    p: [earthmesh_mesh::CartesianPoint; 3],
) -> Option<[f64; 3]> {
    Some([
        angle_at(p[0], p[1], p[2])?,
        angle_at(p[1], p[2], p[0])?,
        angle_at(p[2], p[0], p[1])?,
    ])
}

fn angle_at(
    a: earthmesh_mesh::CartesianPoint,
    b: earthmesh_mesh::CartesianPoint,
    c: earthmesh_mesh::CartesianPoint,
) -> Option<f64> {
    let ab = tangent(a, b)?;
    let ac = tangent(a, c)?;
    let cos = (ab.x * ac.x + ab.y * ac.y + ab.z * ac.z).clamp(-1.0, 1.0);
    Some(cos.acos().to_degrees())
}

fn tangent(
    a: earthmesh_mesh::CartesianPoint,
    b: earthmesh_mesh::CartesianPoint,
) -> Option<earthmesh_mesh::CartesianPoint> {
    let dot = a.x * b.x + a.y * b.y + a.z * b.z;
    let t = earthmesh_mesh::CartesianPoint::new(b.x - dot * a.x, b.y - dot * a.y, b.z - dot * a.z);
    let m = magnitude(t);
    (m > 0.0).then(|| earthmesh_mesh::CartesianPoint::new(t.x / m, t.y / m, t.z / m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mother_grid::MotherGrid;

    #[test]
    fn legacy_40_80_unchanged() {
        let contract = AngleContract::for_id(AngleContractId::LegacyStrict40To80);
        assert_eq!(
            contract.internal,
            AngleWindow {
                minimum_degrees: 40.2,
                maximum_degrees: 79.8,
            }
        );
        assert_eq!(
            contract.final_delivery,
            AngleWindow {
                minimum_degrees: 40.0,
                maximum_degrees: 80.0,
            }
        );
        assert_eq!(contract.preferred, None);
        assert_eq!(
            Certificate::internal().angle_contract_id().unwrap(),
            contract.id
        );
        assert_eq!(
            Certificate::final_delivery().angle_contract_id().unwrap(),
            contract.id
        );
    }

    #[test]
    fn dqx_38_82_accepts_39_278_80_721() {
        let range = (39.278_499_430_048, 80.721_500_570_507);
        let legacy = AngleContract::for_id(AngleContractId::LegacyStrict40To80);
        let dqx = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        assert!(!legacy.final_delivery.contains_range(range.0, range.1));
        assert!(dqx.final_delivery.contains_range(range.0, range.1));
    }

    #[test]
    fn dqx_rejects_37_999() {
        let dqx = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        assert!(!dqx.final_delivery.contains_range(37.999, 80.0));
    }

    #[test]
    fn dqx_rejects_82_001() {
        let dqx = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        assert!(!dqx.final_delivery.contains_range(40.0, 82.001));
        assert!(!dqx.final_delivery.contains_range(80.0, 40.0));
    }

    #[test]
    fn internal_margin_is_stricter() {
        for id in [
            AngleContractId::LegacyStrict40To80,
            AngleContractId::DomainQuality38To82V1,
        ] {
            let contract = AngleContract::for_id(id);
            assert!(contract.final_delivery.contains_range(
                contract.internal.minimum_degrees,
                contract.internal.maximum_degrees
            ));
            assert!(!contract.internal.contains_range(
                contract.final_delivery.minimum_degrees,
                contract.final_delivery.maximum_degrees
            ));
        }
    }

    #[test]
    fn interval_constants_are_outward() {
        let registered = [
            Certificate::final_delivery(),
            Certificate::internal(),
            Certificate::final_delivery_for(AngleContractId::DomainQuality38To82V1),
            Certificate::internal_for(AngleContractId::DomainQuality38To82V1),
        ];
        for certificate in registered {
            let (minimum_lower, maximum_upper) = outward_interval_thresholds(&certificate).unwrap();
            assert!(minimum_lower <= certificate.min_angle_degrees.to_radians().cos());
            assert!(maximum_upper >= certificate.max_angle_degrees.to_radians().cos());
        }

        assert_eq!(
            outward_interval_thresholds(&Certificate::final_delivery()).unwrap(),
            (0.766_044_443_118, 0.173_648_177_667)
        );
        assert_eq!(
            outward_interval_thresholds(&Certificate::internal()).unwrap(),
            (0.763_796_028_634, 0.177_084_740_320)
        );
    }

    #[test]
    fn unregistered_thresholds_are_not_certifiable() {
        let certificate = Certificate {
            min_angle_degrees: 39.0,
            max_angle_degrees: 81.0,
        };
        assert!(matches!(
            outward_interval_thresholds(&certificate),
            Err(CertificateError::CriterionNotCertifiable(_))
        ));
    }

    #[test]
    fn tangled_n12_range_still_fails_dqx() {
        let dqx = AngleContract::for_id(AngleContractId::DomainQuality38To82V1);
        assert!(!dqx
            .final_delivery
            .contains_range(1.337_009_876_734, 173.470_265_136_292));
    }

    #[test]
    fn explicit_dqx_report_carries_versioned_contract() {
        let grid = MotherGrid::generate(2).unwrap();
        Certificate::internal_for(AngleContractId::DomainQuality38To82V1)
            .verify_mother_grid(&grid)
            .unwrap();
        let report = Certificate::final_delivery_for(AngleContractId::DomainQuality38To82V1)
            .verify_mother_grid(&grid)
            .unwrap();
        assert_eq!(report.schema_version, CERTIFICATE_SCHEMA_VERSION);
        assert_eq!(
            report.angle_contract_id,
            AngleContractId::DomainQuality38To82V1
        );
    }

    #[test]
    fn final_certificate_accepts_supported_mothers_as_geometry_only() {
        for n in [1, 2, 3, 4, 6, 8, 12] {
            let grid = MotherGrid::generate(n).unwrap();
            let report = Certificate::final_delivery()
                .verify_mother_grid(&grid)
                .unwrap();
            assert_eq!(
                (report.vertices, report.edges, report.faces),
                (10 * n * n + 2, 30 * n * n, 20 * n * n)
            );
            assert_eq!(report.euler, 2);
            assert_eq!(report.charge, 12);
            assert_eq!(report.open_edges, 0);
            assert_eq!(report.delaunay_violations, 0);
            assert_eq!(report.voronoi_cells, report.vertices);
            assert_eq!(report.topology_errors + report.degree_outside_window, 0);
            assert_eq!(report.angle_gate.as_ref().unwrap().supported_subdivision, n);
        }
    }

    #[test]
    fn internal_margin_accepts_required_test_levels() {
        for n in [1, 2, 3, 4, 6, 8, 12, 20, 40, 80, 160] {
            let grid = MotherGrid::generate(n).unwrap();
            Certificate::internal().verify_mother_grid(&grid).unwrap();
        }
    }

    #[test]
    fn linearized_certificate_paths_keep_mother_counts() {
        let n = 20;
        let grid = MotherGrid::generate(n).unwrap();
        let report = Certificate::final_delivery()
            .verify_mother_grid(&grid)
            .unwrap();

        assert_eq!(
            (report.vertices, report.edges, report.faces),
            (10 * n * n + 2, 30 * n * n, 20 * n * n)
        );
        assert_eq!(report.euler, 2);
        assert_eq!(report.charge, 12);
        assert_eq!(report.open_edges, 0);
        assert_eq!(report.delaunay_violations, 0);
        assert_eq!(report.voronoi_cells, report.vertices);
        assert_eq!(report.topology_errors + report.degree_outside_window, 0);
    }

    #[test]
    fn support_table_reaches_the_default_cell_budget_ceiling() {
        assert_eq!(SupportedMotherAngleGate::SUPPORTED.last(), Some(&640));
    }

    #[test]
    fn local_region_certificate_accepts_supported_mother_faces() {
        let grid = MotherGrid::generate(2).unwrap();
        let faces = grid.mesh.active_triangle_slots().take(4).collect();
        let report = Certificate::internal()
            .verify_geometry_region(&grid.mesh, &faces)
            .unwrap();

        assert_eq!(report.faces, 4);
        assert_eq!(report.interval_boxes, 12);
        assert_eq!(report.orientation_errors, 0);
        assert_eq!(report.degree_outside_window, 0);
        assert_eq!(report.delaunay_violations, 0);
        assert_eq!(
            report.voronoi_invalid_cells + report.voronoi_reciprocal_errors,
            0
        );
    }

    #[test]
    fn internal_interval_margin_rejects_an_angle_between_40_and_40_2_degrees() {
        let mut mesh = MotherGrid::generate(2).unwrap().mesh;
        let face = mesh.active_triangle_slots().next().unwrap();
        let sites = mesh.triangles()[face];
        let epsilon = 0.1;
        let angle = 40.1_f64.to_radians();
        let normalize = |point: CartesianPoint| {
            let scale = 1.0 / magnitude(point);
            CartesianPoint::new(point.x * scale, point.y * scale, point.z * scale)
        };
        for (site, point) in sites.into_iter().zip([
            CartesianPoint::new(0.0, 0.0, 1.0),
            normalize(CartesianPoint::new(epsilon, 0.0, 1.0)),
            normalize(CartesianPoint::new(
                epsilon * angle.cos(),
                epsilon * angle.sin(),
                1.0,
            )),
        ]) {
            mesh.move_vertex(site, point);
        }

        assert_eq!(
            prove_angle_window_for_triangles(
                &mesh,
                &Certificate::final_delivery(),
                std::iter::once(face),
            )
            .unwrap(),
            3
        );
        assert!(matches!(
            prove_angle_window_for_triangles(
                &mesh,
                &Certificate::internal(),
                std::iter::once(face),
            ),
            Err(CertificateError::CriterionNotCertifiable(_))
        ));
    }

    #[test]
    fn local_region_certificate_rejects_distorted_or_inactive_faces() {
        let grid = MotherGrid::generate(2).unwrap();
        let faces: BTreeSet<_> = grid.mesh.active_triangle_slots().take(1).collect();
        let face = *faces.first().unwrap();
        let mut distorted = grid.mesh.clone();
        let [a, b, _] = distorted.triangles()[face];
        distorted.move_vertex(a, distorted.vertices()[b]);

        assert!(Certificate::final_delivery()
            .verify_geometry_region(&distorted, &faces)
            .is_err());

        let inactive = BTreeSet::from([usize::MAX]);
        assert!(matches!(
            Certificate::final_delivery().verify_geometry_region(&grid.mesh, &inactive),
            Err(CertificateError::CriterionNotCertifiable(_))
        ));
    }

    #[test]
    fn unsupported_mother_is_not_strictly_certified_by_support_table() {
        let grid = MotherGrid::generate(5).unwrap();
        assert!(matches!(
            Certificate::final_delivery().verify_mother_grid(&grid),
            Err(CertificateError::UnsupportedMotherSubdivision(5))
        ));
    }
}
