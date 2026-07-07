//! Mesh quality metrics + reports for EarthMesh v3 (MVP).
//!
//! Computes geometry / topology / refinement-transition metrics from a light,
//! engine-agnostic [`QualityMeshInput`] (so this crate stays free of the heavy
//! `netcdf`-linked crates and is unit-testable). Callers (CLI/GUI/mesh) build the
//! input from their own mesh representation. Output writers live in [`io`].
//!
//! Areas are **planar** (lon/lat degree²) via `earthmesh_geometry`; edge lengths are
//! great-circle km via haversine. See `earthmesh_geometry::safety` for the planar /
//! spherical caveats. The public API is kept self-contained so it can graduate into
//! its own `earthmesh_quality` deliverable without churn (it already is one crate).

use earthmesh_geometry::safety::{validate_polygon, GeometryQualityFlag};
use earthmesh_geometry::{haversine_km, polygon_area, signed_ring_area, Point};

pub mod coupling;
pub mod hydro_coast;
pub mod io;
pub mod topology;

/// Pass / warn / fail level for one gate or the whole report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QualityLevel {
    #[default]
    Pass,
    Warn,
    Fail,
}

impl QualityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityLevel::Pass => "pass",
            QualityLevel::Warn => "warn",
            QualityLevel::Fail => "fail",
        }
    }
    fn worse(self, other: QualityLevel) -> QualityLevel {
        use QualityLevel::*;
        match (self, other) {
            (Fail, _) | (_, Fail) => Fail,
            (Warn, _) | (_, Warn) => Warn,
            _ => Pass,
        }
    }
}

/// min/max/mean/std/CV of a sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stat5 {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std: f64,
    pub cv: f64,
}

impl Stat5 {
    pub fn from_slice(values: &[f64]) -> Stat5 {
        if values.is_empty() {
            return Stat5::default();
        }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std = var.sqrt();
        Stat5 {
            min: values.iter().cloned().fold(f64::INFINITY, f64::min),
            max: values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            mean,
            std,
            cv: if mean.abs() > 0.0 {
                std / mean.abs()
            } else {
                0.0
            },
        }
    }
}

/// One cell of the quality input mesh.
#[derive(Clone, Debug, Default)]
pub struct QualityCell {
    /// Indices into [`QualityMeshInput::vertices`], an open ring (no repeated closing vertex).
    pub vertices: Vec<usize>,
    /// Refinement level (base = lowest); enables refinement/transition metrics.
    pub refine_level: Option<u32>,
    /// Adjacent cell indices; enables neighbor-reciprocity / orphan / transition metrics.
    pub neighbors: Vec<usize>,
}

/// Engine-agnostic mesh for quality analysis. `vertices` are (lon, lat) degrees.
#[derive(Clone, Debug, Default)]
pub struct QualityMeshInput {
    pub vertices: Vec<Point>,
    pub cells: Vec<QualityCell>,
}

/// Geometry metrics (planar areas, great-circle edge lengths).
#[derive(Clone, Debug, Default)]
pub struct GeometryMetrics {
    pub cell_count: usize,
    pub vertex_count: usize,
    pub edge_count: usize,
    pub cell_area: Stat5,
    pub cell_area_ratio: f64,
    pub edge_length_km: Stat5,
    /// Per-cell edge-length CV; catches skew that a global edge statistic hides.
    pub cell_edge_length_cv: Stat5,
    pub min_angle_deg: f64,
    pub max_angle_deg: f64,
    /// Per-cell max absolute deviation from the regular n-gon interior angle.
    pub angle_deviation_deg: Stat5,
    /// Triangle-only Field eta quality: 1.0 is equilateral, lower is worse.
    pub triangle_eta: Stat5,
    /// Triangle-only normalized shape/radius ratio: 1.0 is equilateral.
    pub triangle_nsr: Stat5,
    pub aspect_ratio: Stat5,
    pub compactness: Stat5,
    pub zero_area_cell_count: usize,
    pub negative_area_cell_count: usize,
    /// Cells with a NaN/Inf vertex coordinate; excluded from all geometry stats.
    pub non_finite_cell_count: usize,
    pub self_intersection_count: usize,
    pub invalid_polygon_count: usize,
}

/// Topology + refinement-transition metrics.
#[derive(Clone, Debug, Default)]
pub struct TopologyMetrics {
    pub invalid_vertex_index_count: usize,
    pub invalid_cell_index_count: usize,
    pub duplicate_edge_count: usize,
    pub dangling_edge_count: usize,
    /// Edges with exactly one incident cell; informational for regional meshes.
    pub boundary_edge_count: usize,
    /// Shared edges traversed in the same direction by both incident cells.
    pub misoriented_shared_edge_count: usize,
    /// Closed cells whose declared neighbors do not match edge-derived neighbors.
    pub neighbor_degree_mismatch_count: usize,
    pub orphan_cell_count: usize,
    pub neighbor_reciprocity_failure_count: usize,
    pub abnormal_polygon_edge_count: usize,
    pub triangle_cell_count: usize,
    pub quadrilateral_cell_count: usize,
    pub pentagon_cell_count: usize,
    pub hexagon_cell_count: usize,
    pub heptagon_cell_count: usize,
    pub other_polygon_cell_count: usize,
    pub isolated_refined_cell_count: usize,
    pub max_adjacent_resolution_ratio: f64,
    pub transition_continuity_warning_count: usize,
}

/// One gate evaluation.
#[derive(Clone, Debug)]
pub struct GateResult {
    pub metric: String,
    pub value: f64,
    pub level: QualityLevel,
    pub detail: String,
}

/// Quality summary for cells sharing the same refinement level.
#[derive(Clone, Debug, Default)]
pub struct RefineLevelQualitySummary {
    pub refine_level: Option<u32>,
    pub cell_count: usize,
    pub cell_area: Stat5,
    pub cell_edge_length_cv: Stat5,
    pub angle_deviation_deg: Stat5,
    pub triangle_eta: Stat5,
    pub triangle_nsr: Stat5,
}

/// Count of cells assigned to one h-field/refinement level.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelCount {
    pub level: u32,
    pub count: usize,
}

/// Effective h-field controls recorded with a quality report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HfieldConfigDiagnostics {
    pub enabled: bool,
    pub g: Option<f64>,
    pub max_level: Option<u32>,
    pub base_m: Option<f64>,
}

/// Optional diagnostics for h-field driven refinement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HfieldDiagnostics {
    pub config: HfieldConfigDiagnostics,
    pub cell_count: usize,
    pub target_level_distribution: Vec<LevelCount>,
    pub actual_refine_level_distribution: Vec<LevelCount>,
    pub missing_target_level_count: usize,
    pub extra_target_level_count: usize,
    pub missing_actual_refine_level_count: usize,
    pub target_actual_mismatch_count: usize,
    pub target_above_actual_count: usize,
    pub actual_above_target_count: usize,
    pub max_target_actual_delta: u32,
    pub max_adjacent_target_level_jump: u32,
    pub target_level_jump_gt_one_count: usize,
    pub max_adjacent_actual_level_jump: u32,
    pub actual_level_jump_gt_one_count: usize,
}

/// A worst-offending cell for the GeoJSON layer.
#[derive(Clone, Debug)]
pub struct WorstCell {
    pub cell_index: usize,
    pub centroid: Point,
    pub ring: Vec<Point>,
    pub metric: String,
    pub value: f64,
    pub level: QualityLevel,
}

/// Conservative default thresholds; future config can override fields.
#[derive(Clone, Copy, Debug)]
pub struct QualityThresholds {
    pub min_angle_warn_deg: f64,
    pub min_angle_fail_deg: f64,
    pub angle_deviation_warn_deg: f64,
    pub aspect_ratio_warn: f64,
    pub aspect_ratio_fail: f64,
    pub cell_edge_cv_warn: f64,
    pub area_cv_warn: f64,
    pub max_adjacent_resolution_ratio_warn: f64,
    pub worst_cells_limit: usize,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        // Conservative: catastrophic topology = fail; suspicious geometry = warn.
        Self {
            min_angle_warn_deg: 20.0,
            min_angle_fail_deg: 5.0,
            angle_deviation_warn_deg: 35.0,
            aspect_ratio_warn: 4.0,
            aspect_ratio_fail: 10.0,
            cell_edge_cv_warn: 0.35,
            area_cv_warn: 1.5,
            max_adjacent_resolution_ratio_warn: 2.0,
            worst_cells_limit: 50,
        }
    }
}

/// Full quality report. `to_*` serializers are in [`io`].
#[derive(Clone, Debug)]
pub struct MeshQualityReport {
    pub mesh_name: String,
    /// Which cell view was measured (`tri`, `hex`, or empty when the caller does
    /// not provide that context).
    pub cell_view: String,
    pub tool_version: String,
    pub geometry: GeometryMetrics,
    pub topology: TopologyMetrics,
    /// Per-refinement-level quality rollup; `None` means the input carried no level.
    pub refine_level_groups: Vec<RefineLevelQualitySummary>,
    /// Optional h-field target-vs-actual diagnostics, attached by callers that
    /// have sampled target levels for the measured cell view.
    pub hfield: Option<HfieldDiagnostics>,
    pub gates: Vec<GateResult>,
    pub worst_cells: Vec<WorstCell>,
    /// Structured topology problems from [`topology::MeshTopologyValidator`].
    pub topology_issues: Vec<topology::TopologyIssue>,
    pub verdict: QualityLevel,
}

fn cell_ring(input: &QualityMeshInput, cell: &QualityCell) -> Option<Vec<Point>> {
    let mut ring = Vec::with_capacity(cell.vertices.len());
    for &i in &cell.vertices {
        ring.push(*input.vertices.get(i)?);
    }
    Some(ring)
}

/// Copy of `ring` with longitudes unwrapped to within ±180° of the first
/// vertex, so dateline-crossing cells are measured as compact polygons rather
/// than world-spanning slivers (raw lon averaging/shoelace flips sign there).
fn unwrap_ring_lon(ring: &[Point]) -> Vec<Point> {
    let Some(first) = ring.first() else {
        return Vec::new();
    };
    let lon0 = first.x;
    ring.iter()
        .map(|p| {
            let mut lon = p.x;
            while lon - lon0 > 180.0 {
                lon -= 360.0;
            }
            while lon - lon0 < -180.0 {
                lon += 360.0;
            }
            Point::new(lon, p.y)
        })
        .collect()
}

fn centroid(ring: &[Point]) -> Point {
    if ring.is_empty() {
        return Point::new(0.0, 0.0);
    }
    // Average in unwrapped-longitude space so dateline-crossing cells do not
    // land on the wrong side of the globe, then wrap back to [-180, 180].
    let unwrapped = unwrap_ring_lon(ring);
    let n = unwrapped.len() as f64;
    let mut lon = unwrapped.iter().map(|p| p.x).sum::<f64>() / n;
    while lon > 180.0 {
        lon -= 360.0;
    }
    while lon < -180.0 {
        lon += 360.0;
    }
    Point::new(lon, unwrapped.iter().map(|p| p.y).sum::<f64>() / n)
}

/// (lon, lat) degrees -> unit sphere (x, y, z). Dateline/pole-safe corner geometry.
fn lonlat_to_unit(p: Point) -> [f64; 3] {
    let lon = p.x.to_radians();
    let lat = p.y.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn vsub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Unsigned corner angles (degrees) at each ring vertex, computed from 3D
/// unit-sphere chord vectors so dateline-crossing / polar cells are not measured
/// as planar (lon, lat) slivers (which produced spurious near-zero angles on global
/// meshes). This is a chord approximation of the true spherical corner angle.
fn interior_angles_deg(ring: &[Point]) -> Vec<f64> {
    let n = ring.len();
    if n < 3 {
        return Vec::new();
    }
    let xyz: Vec<[f64; 3]> = ring.iter().map(|p| lonlat_to_unit(*p)).collect();
    let mut angles = Vec::with_capacity(n);
    for i in 0..n {
        let cur = xyz[i];
        let a = vsub(xyz[(i + n - 1) % n], cur);
        let b = vsub(xyz[(i + 1) % n], cur);
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let mag = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
            * (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
        if mag > 0.0 {
            angles.push((dot / mag).clamp(-1.0, 1.0).acos().to_degrees());
        }
    }
    angles
}

/// Sorted, deduplicated edge key for a vertex pair.
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn triangle_quality(area: f64, lens: &[f64]) -> Option<(f64, f64)> {
    if lens.len() != 3 || area <= 0.0 || lens.iter().any(|v| *v <= 0.0) {
        return None;
    }
    let (a, b, c) = (lens[0], lens[1], lens[2]);
    let eta = 4.0 * 3.0_f64.sqrt() * area / lens.iter().map(|v| v * v).sum::<f64>();
    let inradius = 2.0 * area / (a + b + c);
    let circumradius = 0.25 * a * b * c / area;
    (circumradius > 0.0).then_some((eta, 2.0 * inradius / circumradius))
}

#[derive(Default)]
struct RefineLevelAccumulator {
    cell_count: usize,
    areas: Vec<f64>,
    edge_cvs: Vec<f64>,
    angle_deviations: Vec<f64>,
    triangle_etas: Vec<f64>,
    triangle_nsrs: Vec<f64>,
}

impl RefineLevelAccumulator {
    fn finish(self, refine_level: Option<u32>) -> RefineLevelQualitySummary {
        RefineLevelQualitySummary {
            refine_level,
            cell_count: self.cell_count,
            cell_area: Stat5::from_slice(&self.areas),
            cell_edge_length_cv: Stat5::from_slice(&self.edge_cvs),
            angle_deviation_deg: Stat5::from_slice(&self.angle_deviations),
            triangle_eta: Stat5::from_slice(&self.triangle_etas),
            triangle_nsr: Stat5::from_slice(&self.triangle_nsrs),
        }
    }
}

/// Compute the full quality report for `input` under `thresholds`.
pub fn compute(input: &QualityMeshInput, thresholds: &QualityThresholds) -> MeshQualityReport {
    let mut geom = GeometryMetrics {
        cell_count: input.cells.len(),
        vertex_count: input.vertices.len(),
        ..Default::default()
    };
    let mut topo = TopologyMetrics::default();

    let mut areas = Vec::new();
    let mut edge_lengths = Vec::new();
    let mut cell_edge_cvs = Vec::new();
    let mut angle_deviations = Vec::new();
    let mut triangle_etas = Vec::new();
    let mut triangle_nsrs = Vec::new();
    let mut aspects = Vec::new();
    let mut compactnesses = Vec::new();
    let mut min_angle = f64::INFINITY;
    let mut max_angle = f64::NEG_INFINITY;

    // edge -> count of incident cells; track degenerate / invalid edges.
    use std::collections::BTreeMap;
    type EdgeKey = (usize, usize);
    type DirectedEdgeUse = (usize, usize, usize);
    let mut edge_cells: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let mut edge_orientations: BTreeMap<EdgeKey, Vec<DirectedEdgeUse>> = BTreeMap::new();
    let mut refine_groups: BTreeMap<Option<u32>, RefineLevelAccumulator> = BTreeMap::new();
    let nv = input.vertices.len();

    for (ci, cell) in input.cells.iter().enumerate() {
        refine_groups
            .entry(cell.refine_level)
            .or_default()
            .cell_count += 1;
        let unique_idx: Vec<usize> = {
            let mut seen = Vec::new();
            for &i in &cell.vertices {
                if !seen.contains(&i) {
                    seen.push(i);
                }
            }
            seen
        };
        if unique_idx.len() < 3 {
            topo.abnormal_polygon_edge_count += 1;
        } else {
            match unique_idx.len() {
                3 => topo.triangle_cell_count += 1,
                4 => topo.quadrilateral_cell_count += 1,
                5 => topo.pentagon_cell_count += 1,
                6 => topo.hexagon_cell_count += 1,
                7 => topo.heptagon_cell_count += 1,
                _ => topo.other_polygon_cell_count += 1,
            }
        }
        for &i in &cell.vertices {
            if i >= nv {
                topo.invalid_vertex_index_count += 1;
            }
        }
        // edges
        let m = cell.vertices.len();
        for k in 0..m {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % m];
            if a >= nv || b >= nv {
                topo.dangling_edge_count += 1;
                continue;
            }
            if a == b {
                topo.dangling_edge_count += 1;
                continue;
            }
            let key = edge_key(a, b);
            edge_cells.entry(key).or_default().push(ci);
            edge_orientations.entry(key).or_default().push((ci, a, b));
            // great-circle length; skip non-finite endpoints so one bad vertex
            // cannot poison the edge-length mean/std with NaN.
            let (pa, pb) = (input.vertices[a], input.vertices[b]);
            if pa.x.is_finite() && pa.y.is_finite() && pb.x.is_finite() && pb.y.is_finite() {
                edge_lengths.push(haversine_km(pa, pb));
            }
        }

        // neighbor index validity (kept before the geometry block: that block
        // may `continue` on non-finite cells and must not skip this check)
        for &n in &cell.neighbors {
            if n >= input.cells.len() || n == ci {
                topo.invalid_cell_index_count += 1;
            }
        }

        // per-cell geometry (only when ring is resolvable)
        if let Some(ring) = cell_ring(input, cell) {
            let flags = validate_polygon(&ring);
            if flags.contains(&GeometryQualityFlag::NonFiniteCoordinate) {
                // NaN/Inf vertex: count it and skip all geometry stats for this
                // cell. IEEE makes `NaN <= eps` false, so without this guard a
                // NaN area silently lands in `areas` and turns mean/std/cv into
                // NaN while min/max stay finite (f64::min/max skip NaN).
                geom.non_finite_cell_count += 1;
                continue;
            }
            if flags.contains(&GeometryQualityFlag::SelfIntersection) {
                geom.self_intersection_count += 1;
            }
            if flags.contains(&GeometryQualityFlag::InvalidPolygon) {
                geom.invalid_polygon_count += 1;
            }
            // Winding check on the unwrapped ring (`polygon_area` is unsigned,
            // so it can never feed this counter): CCW is the mesh convention,
            // a clockwise ring is a catastrophic orientation error.
            if signed_ring_area(&unwrap_ring_lon(&ring)) < -1.0e-12 {
                geom.negative_area_cell_count += 1;
            }
            let area = polygon_area(&ring);
            if area <= 1.0e-12 {
                geom.zero_area_cell_count += 1;
            } else {
                areas.push(area);
                let group = refine_groups.entry(cell.refine_level).or_default();
                group.areas.push(area);
                // aspect ratio = longest/shortest edge in great-circle km (the ratio is
                // unit-free and, unlike planar lon/lat lengths, dateline/pole-safe).
                let mut km_lens = Vec::new();
                let mut planar_lens = Vec::new();
                let planar_ring = unwrap_ring_lon(&ring);
                for k in 0..ring.len() {
                    let p = ring[k];
                    let q = ring[(k + 1) % ring.len()];
                    km_lens.push(haversine_km(p, q));
                    let pp = planar_ring[k];
                    let qq = planar_ring[(k + 1) % planar_ring.len()];
                    planar_lens.push(((pp.x - qq.x).powi(2) + (pp.y - qq.y).powi(2)).sqrt());
                }
                let emin = km_lens.iter().cloned().fold(f64::INFINITY, f64::min);
                let emax = km_lens.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if emin > 0.0 {
                    aspects.push(emax / emin);
                }
                let edge_cv = Stat5::from_slice(&km_lens).cv;
                cell_edge_cvs.push(edge_cv);
                group.edge_cvs.push(edge_cv);
                // compactness = 4*pi*A / P^2 (planar area + planar perimeter for
                // dimensional consistency; report-only, not gated).
                let perim: f64 = planar_lens.iter().sum();
                if perim > 0.0 {
                    compactnesses.push(4.0 * std::f64::consts::PI * area / (perim * perim));
                }
                if let Some((eta, nsr)) = triangle_quality(polygon_area(&planar_ring), &planar_lens)
                {
                    triangle_etas.push(eta);
                    triangle_nsrs.push(nsr);
                    group.triangle_etas.push(eta);
                    group.triangle_nsrs.push(nsr);
                }
                let angles = interior_angles_deg(&ring);
                if !angles.is_empty() {
                    let ideal = (ring.len() as f64 - 2.0) * 180.0 / ring.len() as f64;
                    let angle_deviation = angles
                        .iter()
                        .map(|ang| (ang - ideal).abs())
                        .fold(0.0, f64::max);
                    angle_deviations.push(angle_deviation);
                    group.angle_deviations.push(angle_deviation);
                }
                for ang in angles {
                    min_angle = min_angle.min(ang);
                    max_angle = max_angle.max(ang);
                }
            }
        }
    }

    geom.edge_count = edge_cells.len();
    geom.cell_area = Stat5::from_slice(&areas);
    geom.cell_area_ratio = if geom.cell_area.min > 0.0 {
        geom.cell_area.max / geom.cell_area.min
    } else {
        0.0
    };
    geom.edge_length_km = Stat5::from_slice(&edge_lengths);
    geom.cell_edge_length_cv = Stat5::from_slice(&cell_edge_cvs);
    geom.angle_deviation_deg = Stat5::from_slice(&angle_deviations);
    geom.triangle_eta = Stat5::from_slice(&triangle_etas);
    geom.triangle_nsr = Stat5::from_slice(&triangle_nsrs);
    geom.aspect_ratio = Stat5::from_slice(&aspects);
    geom.compactness = Stat5::from_slice(&compactnesses);
    geom.min_angle_deg = if min_angle.is_finite() {
        min_angle
    } else {
        0.0
    };
    geom.max_angle_deg = if max_angle.is_finite() {
        max_angle
    } else {
        0.0
    };

    // non-manifold edges (shared by > 2 cells)
    topo.duplicate_edge_count = edge_cells.values().filter(|c| c.len() > 2).count();
    topo.boundary_edge_count = edge_cells.values().filter(|c| c.len() == 1).count();
    topo.misoriented_shared_edge_count = edge_orientations
        .values()
        .filter(|occ| occ.len() == 2 && occ[0].1 == occ[1].1 && occ[0].2 == occ[1].2)
        .count();

    // orphan cells: share no edge with any other cell
    for (ci, cell) in input.cells.iter().enumerate() {
        let m = cell.vertices.len();
        let mut shares = false;
        for k in 0..m {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % m];
            if a >= nv || b >= nv || a == b {
                continue;
            }
            if let Some(cells) = edge_cells.get(&edge_key(a, b)) {
                if cells.iter().any(|&other| other != ci) {
                    shares = true;
                    break;
                }
            }
        }
        if !shares && m >= 3 {
            topo.orphan_cell_count += 1;
        }
    }

    // Closed-cell adjacency sanity: if every valid edge has exactly one opposite
    // cell, the declared neighbor set should match the edge-derived one. Boundary
    // cells are intentionally skipped because regional/filter meshes are valid.
    for (ci, cell) in input.cells.iter().enumerate() {
        let m = cell.vertices.len();
        if m < 3 {
            continue;
        }
        let mut derived_neighbors = Vec::new();
        let mut closed = true;
        for k in 0..m {
            let a = cell.vertices[k];
            let b = cell.vertices[(k + 1) % m];
            if a >= nv || b >= nv || a == b {
                closed = false;
                break;
            }
            let Some(cells) = edge_cells.get(&edge_key(a, b)) else {
                closed = false;
                break;
            };
            if cells.len() != 2 {
                closed = false;
                break;
            }
            if let Some(&other) = cells.iter().find(|&&other| other != ci) {
                if !derived_neighbors.contains(&other) {
                    derived_neighbors.push(other);
                }
            } else {
                closed = false;
                break;
            }
        }
        if !closed {
            continue;
        }
        derived_neighbors.sort_unstable();
        let mut declared_neighbors: Vec<usize> = cell
            .neighbors
            .iter()
            .copied()
            .filter(|&nb| nb < input.cells.len() && nb != ci)
            .collect();
        declared_neighbors.sort_unstable();
        declared_neighbors.dedup();
        if declared_neighbors != derived_neighbors {
            topo.neighbor_degree_mismatch_count += 1;
        }
    }

    // neighbor reciprocity + transition + isolated refined
    for (ci, cell) in input.cells.iter().enumerate() {
        for &nb in &cell.neighbors {
            if nb >= input.cells.len() {
                continue;
            }
            if !input.cells[nb].neighbors.contains(&ci) {
                topo.neighbor_reciprocity_failure_count += 1;
            }
            if let (Some(la), Some(lb)) = (cell.refine_level, input.cells[nb].refine_level) {
                let diff = la.abs_diff(lb);
                let ratio = 2f64.powi(diff as i32);
                topo.max_adjacent_resolution_ratio = topo.max_adjacent_resolution_ratio.max(ratio);
                if diff > 1 {
                    topo.transition_continuity_warning_count += 1;
                }
            }
        }
        // isolated refined: refined cell whose every neighbor is strictly coarser
        if let Some(la) = cell.refine_level {
            if la > 0
                && !cell.neighbors.is_empty()
                && cell.neighbors.iter().all(|&nb| {
                    input
                        .cells
                        .get(nb)
                        .and_then(|c| c.refine_level)
                        .map(|lb| lb < la)
                        .unwrap_or(false)
                })
            {
                topo.isolated_refined_cell_count += 1;
            }
        }
    }

    let (gates, worst_cells, gate_verdict) = evaluate(input, &geom, &topo, thresholds);
    let refine_level_groups = refine_groups
        .into_iter()
        .map(|(level, acc)| acc.finish(level))
        .collect();

    // Run the detailed topology validator and fold its worst severity into the
    // verdict (catastrophic connectivity = Fail; transition degradation = Warn).
    let topology_issues = topology::MeshTopologyValidator::new(input).validate_all();
    let validator_level = match topology::worst_severity(&topology_issues) {
        Some(topology::Severity::Fail) => QualityLevel::Fail,
        Some(topology::Severity::Warn) => QualityLevel::Warn,
        None => QualityLevel::Pass,
    };
    let verdict = gate_verdict.worse(validator_level);

    MeshQualityReport {
        mesh_name: String::new(),
        cell_view: String::new(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        geometry: geom,
        topology: topo,
        refine_level_groups,
        hfield: None,
        gates,
        worst_cells,
        topology_issues,
        verdict,
    }
}

fn level_counts_from_map(map: std::collections::BTreeMap<u32, usize>) -> Vec<LevelCount> {
    map.into_iter()
        .map(|(level, count)| LevelCount { level, count })
        .collect()
}

/// Compute h-field diagnostics from per-cell target levels plus actual
/// refinement levels carried by [`QualityCell::refine_level`].
///
/// `target_levels[i]` is interpreted as the h-field target for `input.cells[i]`.
/// Extra targets are reported and ignored; missing target/actual values are
/// counted separately and included in the mismatch total when only one side is
/// present.
pub fn compute_hfield_diagnostics(
    input: &QualityMeshInput,
    target_levels: &[u32],
    config: HfieldConfigDiagnostics,
) -> HfieldDiagnostics {
    use std::collections::{BTreeMap, BTreeSet};

    let cell_count = input.cells.len();
    let mut target_hist = BTreeMap::<u32, usize>::new();
    let mut actual_hist = BTreeMap::<u32, usize>::new();

    for &level in target_levels.iter().take(cell_count) {
        *target_hist.entry(level).or_default() += 1;
    }
    for cell in &input.cells {
        if let Some(level) = cell.refine_level {
            *actual_hist.entry(level).or_default() += 1;
        }
    }

    let missing_target_level_count = cell_count.saturating_sub(target_levels.len());
    let extra_target_level_count = target_levels.len().saturating_sub(cell_count);
    let missing_actual_refine_level_count = input
        .cells
        .iter()
        .filter(|cell| cell.refine_level.is_none())
        .count();

    let mut target_actual_mismatch_count = 0usize;
    let mut target_above_actual_count = 0usize;
    let mut actual_above_target_count = 0usize;
    let mut max_target_actual_delta = 0u32;

    for (ci, cell) in input.cells.iter().enumerate() {
        match (target_levels.get(ci).copied(), cell.refine_level) {
            (Some(target), Some(actual)) => {
                if target != actual {
                    target_actual_mismatch_count += 1;
                    if target > actual {
                        target_above_actual_count += 1;
                    } else {
                        actual_above_target_count += 1;
                    }
                    max_target_actual_delta = max_target_actual_delta.max(target.abs_diff(actual));
                }
            }
            (Some(_), None) | (None, Some(_)) => {
                target_actual_mismatch_count += 1;
            }
            (None, None) => {}
        }
    }

    let mut pairs = BTreeSet::<(usize, usize)>::new();
    for (ci, cell) in input.cells.iter().enumerate() {
        for &nb in &cell.neighbors {
            if nb < cell_count && nb != ci {
                pairs.insert((ci.min(nb), ci.max(nb)));
            }
        }
    }

    let mut max_adjacent_target_level_jump = 0u32;
    let mut target_level_jump_gt_one_count = 0usize;
    let mut max_adjacent_actual_level_jump = 0u32;
    let mut actual_level_jump_gt_one_count = 0usize;
    for (a, b) in pairs {
        if let (Some(la), Some(lb)) = (target_levels.get(a), target_levels.get(b)) {
            let diff = la.abs_diff(*lb);
            max_adjacent_target_level_jump = max_adjacent_target_level_jump.max(diff);
            if diff > 1 {
                target_level_jump_gt_one_count += 1;
            }
        }
        if let (Some(la), Some(lb)) = (input.cells[a].refine_level, input.cells[b].refine_level) {
            let diff = la.abs_diff(lb);
            max_adjacent_actual_level_jump = max_adjacent_actual_level_jump.max(diff);
            if diff > 1 {
                actual_level_jump_gt_one_count += 1;
            }
        }
    }

    HfieldDiagnostics {
        config,
        cell_count,
        target_level_distribution: level_counts_from_map(target_hist),
        actual_refine_level_distribution: level_counts_from_map(actual_hist),
        missing_target_level_count,
        extra_target_level_count,
        missing_actual_refine_level_count,
        target_actual_mismatch_count,
        target_above_actual_count,
        actual_above_target_count,
        max_target_actual_delta,
        max_adjacent_target_level_jump,
        target_level_jump_gt_one_count,
        max_adjacent_actual_level_jump,
        actual_level_jump_gt_one_count,
    }
}

/// Attach h-field diagnostics to a report and add non-failing warning gates for
/// mismatches or level jumps. The base geometry/topology verdict is preserved
/// unless one of these h-field diagnostics raises the report to `Warn`.
pub fn attach_hfield_diagnostics(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    target_levels: &[u32],
    config: HfieldConfigDiagnostics,
) {
    let diagnostics = compute_hfield_diagnostics(input, target_levels, config);
    let mut add_gate = |metric: &str, value: usize, detail: &str| {
        let level = if value > 0 {
            QualityLevel::Warn
        } else {
            QualityLevel::Pass
        };
        report.gates.push(GateResult {
            metric: metric.to_string(),
            value: value as f64,
            level,
            detail: detail.to_string(),
        });
        report.verdict = report.verdict.worse(level);
    };
    add_gate(
        "hfield_target_actual_mismatch_count",
        diagnostics.target_actual_mismatch_count,
        "h-field target level differs from actual refinement level",
    );
    add_gate(
        "hfield_target_level_jump_gt_one_count",
        diagnostics.target_level_jump_gt_one_count,
        "adjacent h-field target level jump > 1",
    );
    add_gate(
        "hfield_actual_level_jump_gt_one_count",
        diagnostics.actual_level_jump_gt_one_count,
        "adjacent actual refinement level jump > 1",
    );
    report.hfield = Some(diagnostics);
}

fn evaluate(
    input: &QualityMeshInput,
    geom: &GeometryMetrics,
    topo: &TopologyMetrics,
    th: &QualityThresholds,
) -> (Vec<GateResult>, Vec<WorstCell>, QualityLevel) {
    let mut gates = Vec::new();
    let mut push = |metric: &str, value: f64, level: QualityLevel, detail: &str| {
        gates.push(GateResult {
            metric: metric.to_string(),
            value,
            level,
            detail: detail.to_string(),
        });
    };

    // Catastrophic topology -> Fail.
    for (name, count) in [
        (
            "invalid_vertex_index_count",
            topo.invalid_vertex_index_count,
        ),
        ("invalid_cell_index_count", topo.invalid_cell_index_count),
        ("duplicate_edge_count", topo.duplicate_edge_count),
        ("dangling_edge_count", topo.dangling_edge_count),
        (
            "misoriented_shared_edge_count",
            topo.misoriented_shared_edge_count,
        ),
        (
            "neighbor_degree_mismatch_count",
            topo.neighbor_degree_mismatch_count,
        ),
        ("orphan_cell_count", topo.orphan_cell_count),
        (
            "neighbor_reciprocity_failure_count",
            topo.neighbor_reciprocity_failure_count,
        ),
        (
            "abnormal_polygon_edge_count",
            topo.abnormal_polygon_edge_count,
        ),
        ("self_intersection_count", geom.self_intersection_count),
        ("invalid_polygon_count", geom.invalid_polygon_count),
        ("zero_area_cell_count", geom.zero_area_cell_count),
        ("negative_area_cell_count", geom.negative_area_cell_count),
        ("non_finite_cell_count", geom.non_finite_cell_count),
    ] {
        push(
            name,
            count as f64,
            if count > 0 {
                QualityLevel::Fail
            } else {
                QualityLevel::Pass
            },
            if count > 0 {
                "catastrophic topology/geometry error"
            } else {
                ""
            },
        );
    }

    // Suspicious geometry degradation -> Warn (fail only when extreme).
    let min_angle_level = if geom.min_angle_deg < th.min_angle_fail_deg {
        QualityLevel::Fail
    } else if geom.min_angle_deg < th.min_angle_warn_deg {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "min_angle_deg",
        geom.min_angle_deg,
        min_angle_level,
        "smallest interior angle",
    );

    // Strict comparisons on both graded gates so a value landing exactly on a
    // threshold stays in the less severe tier, matching the min_angle gate.
    let aspect_level = if geom.aspect_ratio.max > th.aspect_ratio_fail {
        QualityLevel::Fail
    } else if geom.aspect_ratio.max > th.aspect_ratio_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "aspect_ratio_max",
        geom.aspect_ratio.max,
        aspect_level,
        "max cell aspect ratio",
    );

    let edge_cv_level = if geom.cell_edge_length_cv.max > th.cell_edge_cv_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "cell_edge_length_cv_max",
        geom.cell_edge_length_cv.max,
        edge_cv_level,
        "max per-cell edge-length coefficient of variation",
    );

    let angle_dev_level = if geom.angle_deviation_deg.max > th.angle_deviation_warn_deg {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "angle_deviation_deg_max",
        geom.angle_deviation_deg.max,
        angle_dev_level,
        "max deviation from regular n-gon angle",
    );

    let cv_level = if geom.cell_area.cv >= th.area_cv_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "cell_area_cv",
        geom.cell_area.cv,
        cv_level,
        "cell area coefficient of variation",
    );
    push(
        "cell_area_ratio",
        geom.cell_area_ratio,
        QualityLevel::Pass,
        "max/min positive cell area",
    );

    let res_level = if topo.max_adjacent_resolution_ratio > th.max_adjacent_resolution_ratio_warn {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "max_adjacent_resolution_ratio",
        topo.max_adjacent_resolution_ratio,
        res_level,
        "abrupt refinement transition",
    );

    let trans_level = if topo.transition_continuity_warning_count > 0 {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "transition_continuity_warning_count",
        topo.transition_continuity_warning_count as f64,
        trans_level,
        "adjacent level jump > 1",
    );

    let isolated_level = if topo.isolated_refined_cell_count > 0 {
        QualityLevel::Warn
    } else {
        QualityLevel::Pass
    };
    push(
        "isolated_refined_cell_count",
        topo.isolated_refined_cell_count as f64,
        isolated_level,
        "refined cell with only coarser neighbors",
    );

    let verdict = gates
        .iter()
        .fold(QualityLevel::Pass, |acc, g| acc.worse(g.level));
    let worst_cells = collect_worst_cells(input, th);
    (gates, worst_cells, verdict)
}

/// Worst cells: the validate_polygon-flagged ones first, then lowest min-angle.
fn collect_worst_cells(input: &QualityMeshInput, th: &QualityThresholds) -> Vec<WorstCell> {
    let mut scored: Vec<WorstCell> = Vec::new();
    for (ci, cell) in input.cells.iter().enumerate() {
        let Some(ring) = cell_ring(input, cell) else {
            continue;
        };
        let flags = validate_polygon(&ring);
        if flags.contains(&GeometryQualityFlag::NonFiniteCoordinate) {
            // A NaN/Inf ring has no drawable centroid/ring for the GeoJSON layer.
            continue;
        }
        let (metric, value, level) = if flags.contains(&GeometryQualityFlag::SelfIntersection) {
            ("self_intersection".to_string(), 1.0, QualityLevel::Fail)
        } else if flags.contains(&GeometryQualityFlag::ZeroAreaCell) {
            ("zero_area".to_string(), 0.0, QualityLevel::Fail)
        } else if flags.contains(&GeometryQualityFlag::InvalidPolygon) {
            ("invalid_polygon".to_string(), 0.0, QualityLevel::Fail)
        } else {
            let angles = interior_angles_deg(&ring);
            let min_a = angles.iter().cloned().fold(f64::INFINITY, f64::min);
            if min_a.is_finite() && min_a < th.min_angle_warn_deg {
                (
                    "min_angle_deg".to_string(),
                    min_a,
                    if min_a < th.min_angle_fail_deg {
                        QualityLevel::Fail
                    } else {
                        QualityLevel::Warn
                    },
                )
            } else {
                continue;
            }
        };
        scored.push(WorstCell {
            cell_index: ci,
            centroid: centroid(&ring),
            ring,
            metric,
            value,
            level,
        });
    }
    // Fail before Warn; within a level, lower value first (worse).
    scored.sort_by(|a, b| {
        let la = matches!(a.level, QualityLevel::Fail) as i32;
        let lb = matches!(b.level, QualityLevel::Fail) as i32;
        lb.cmp(&la).then(
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    scored.truncate(th.worst_cells_limit);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_cell(a: usize, b: usize, c: usize, d: usize) -> QualityCell {
        QualityCell {
            vertices: vec![a, b, c, d],
            refine_level: Some(0),
            neighbors: Vec::new(),
        }
    }

    /// 2 unit squares sharing an edge.
    fn two_square_mesh() -> QualityMeshInput {
        QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(0.0, 1.0),
                Point::new(2.0, 0.0),
                Point::new(2.0, 1.0),
            ],
            cells: vec![
                QualityCell {
                    vertices: vec![0, 1, 2, 3],
                    refine_level: Some(0),
                    neighbors: vec![1],
                },
                QualityCell {
                    vertices: vec![1, 4, 5, 2],
                    refine_level: Some(0),
                    neighbors: vec![0],
                },
            ],
        }
    }

    #[test]
    fn tiny_valid_mesh_passes() {
        let r = compute(&two_square_mesh(), &QualityThresholds::default());
        assert_eq!(r.verdict, QualityLevel::Pass);
        assert_eq!(r.geometry.cell_count, 2);
        assert_eq!(r.geometry.vertex_count, 6);
        assert_eq!(r.geometry.edge_count, 7); // 4+4 minus 1 shared
        assert_eq!(r.topology.duplicate_edge_count, 0);
        assert_eq!(r.topology.orphan_cell_count, 0);
        assert_eq!(r.topology.neighbor_reciprocity_failure_count, 0);
        assert_eq!(r.topology.quadrilateral_cell_count, 2);
        // 3D chord corner angle of a 1°×1° equatorial square is ~90° (not exactly,
        // since the chord vectors live on the sphere) — sane, not a planar artifact.
        assert!((r.geometry.min_angle_deg - 90.0).abs() < 1.0);
    }

    #[test]
    fn invalid_vertex_index_is_fail() {
        let mut m = two_square_mesh();
        m.cells[0].vertices = vec![0, 1, 2, 99];
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.invalid_vertex_index_count >= 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn duplicate_edge_non_manifold_is_fail() {
        // three cells sharing the same edge (0,1)
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.5, 1.0),
                Point::new(0.5, -1.0),
                Point::new(0.5, 2.0),
            ],
            cells: vec![
                square_cell(0, 1, 2, 2),
                QualityCell {
                    vertices: vec![0, 1, 3],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
                QualityCell {
                    vertices: vec![0, 1, 4],
                    refine_level: Some(0),
                    neighbors: vec![],
                },
            ],
        };
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.duplicate_edge_count >= 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn zero_area_cell_is_fail() {
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(2.0, 0.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        };
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.zero_area_cell_count, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn bad_neighbor_reciprocity_is_fail() {
        let mut m = two_square_mesh();
        m.cells[1].neighbors = vec![]; // cell0 claims 1 as neighbor; 1 doesn't reciprocate
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.neighbor_reciprocity_failure_count >= 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn clockwise_cell_counts_negative_area_and_fails() {
        let mut m = two_square_mesh();
        // Reverse cell 0's ring: CCW -> CW winding.
        m.cells[0].vertices = vec![3, 2, 1, 0];
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.negative_area_cell_count, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn ccw_dateline_cell_is_not_negative_area() {
        // CCW quad straddling the antimeridian; raw-longitude shoelace would
        // wrongly report it as clockwise/negative without lon unwrapping.
        let m = QualityMeshInput {
            vertices: vec![
                Point::new(179.0, 0.0),
                Point::new(-179.0, 0.0),
                Point::new(-179.0, 1.0),
                Point::new(179.0, 1.0),
            ],
            cells: vec![QualityCell {
                vertices: vec![0, 1, 2, 3],
                refine_level: Some(0),
                neighbors: vec![],
            }],
        };
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.negative_area_cell_count, 0);
        // Centroid of the worst-cells layer must sit near the dateline, not lon~0.
        let c = centroid(&[
            Point::new(179.0, 0.0),
            Point::new(-179.0, 0.0),
            Point::new(-179.0, 1.0),
            Point::new(179.0, 1.0),
        ]);
        assert!(c.x.abs() > 179.0, "centroid lon {} should hug ±180", c.x);
    }

    #[test]
    fn non_finite_vertex_is_counted_and_does_not_poison_stats() {
        let mut m = two_square_mesh();
        m.vertices[0] = Point::new(f64::NAN, 0.0);
        let r = compute(&m, &QualityThresholds::default());
        assert_eq!(r.geometry.non_finite_cell_count, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
        // Cell 1 is untouched; its stats must stay finite.
        assert!(r.geometry.cell_area.mean.is_finite());
        assert!(r.geometry.cell_area.std.is_finite());
        assert!(r.geometry.edge_length_km.mean.is_finite());
    }

    #[test]
    fn abrupt_transition_warns() {
        let mut m = two_square_mesh();
        m.cells[0].refine_level = Some(0);
        m.cells[1].refine_level = Some(3);
        let r = compute(&m, &QualityThresholds::default());
        assert!(r.topology.max_adjacent_resolution_ratio >= 4.0);
        assert!(r.topology.transition_continuity_warning_count >= 1);
        assert_ne!(r.verdict, QualityLevel::Pass); // at least Warn
    }
}
