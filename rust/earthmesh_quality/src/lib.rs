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
use earthmesh_geometry::{haversine_km, polygon_area, Point};

pub mod io;
pub mod topology;

/// Pass / warn / fail level for one gate or the whole report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityLevel {
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
    pub edge_length_km: Stat5,
    pub min_angle_deg: f64,
    pub max_angle_deg: f64,
    pub aspect_ratio: Stat5,
    pub compactness: Stat5,
    pub zero_area_cell_count: usize,
    pub negative_area_cell_count: usize,
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
    pub orphan_cell_count: usize,
    pub neighbor_reciprocity_failure_count: usize,
    pub abnormal_polygon_edge_count: usize,
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
    pub aspect_ratio_warn: f64,
    pub aspect_ratio_fail: f64,
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
            aspect_ratio_warn: 4.0,
            aspect_ratio_fail: 10.0,
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
    pub tool_version: String,
    pub geometry: GeometryMetrics,
    pub topology: TopologyMetrics,
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

fn centroid(ring: &[Point]) -> Point {
    if ring.is_empty() {
        return Point::new(0.0, 0.0);
    }
    let n = ring.len() as f64;
    Point::new(
        ring.iter().map(|p| p.x).sum::<f64>() / n,
        ring.iter().map(|p| p.y).sum::<f64>() / n,
    )
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
    let mut aspects = Vec::new();
    let mut compactnesses = Vec::new();
    let mut min_angle = f64::INFINITY;
    let mut max_angle = f64::NEG_INFINITY;

    // edge -> count of incident cells; track degenerate / invalid edges.
    use std::collections::BTreeMap;
    let mut edge_cells: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let nv = input.vertices.len();

    for (ci, cell) in input.cells.iter().enumerate() {
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
            edge_cells.entry(edge_key(a, b)).or_default().push(ci);
            // great-circle length
            edge_lengths.push(haversine_km(input.vertices[a], input.vertices[b]));
        }

        // per-cell geometry (only when ring is resolvable)
        if let Some(ring) = cell_ring(input, cell) {
            let flags = validate_polygon(&ring);
            if flags.contains(&GeometryQualityFlag::SelfIntersection) {
                geom.self_intersection_count += 1;
            }
            if flags.contains(&GeometryQualityFlag::InvalidPolygon) {
                geom.invalid_polygon_count += 1;
            }
            let area = polygon_area(&ring);
            if area <= 1.0e-12 {
                geom.zero_area_cell_count += 1;
            } else {
                areas.push(area);
                // aspect ratio = longest/shortest edge in great-circle km (the ratio is
                // unit-free and, unlike planar lon/lat lengths, dateline/pole-safe).
                let mut km_lens = Vec::new();
                let mut planar_lens = Vec::new();
                for k in 0..ring.len() {
                    let p = ring[k];
                    let q = ring[(k + 1) % ring.len()];
                    km_lens.push(haversine_km(p, q));
                    planar_lens.push(((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt());
                }
                let emin = km_lens.iter().cloned().fold(f64::INFINITY, f64::min);
                let emax = km_lens.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if emin > 0.0 {
                    aspects.push(emax / emin);
                }
                // compactness = 4*pi*A / P^2 (planar area + planar perimeter for
                // dimensional consistency; report-only, not gated).
                let perim: f64 = planar_lens.iter().sum();
                if perim > 0.0 {
                    compactnesses.push(4.0 * std::f64::consts::PI * area / (perim * perim));
                }
                for ang in interior_angles_deg(&ring) {
                    min_angle = min_angle.min(ang);
                    max_angle = max_angle.max(ang);
                }
            }
        }

        // neighbor index validity
        for &n in &cell.neighbors {
            if n >= input.cells.len() || n == ci {
                topo.invalid_cell_index_count += 1;
            }
        }
    }

    geom.edge_count = edge_cells.len();
    geom.cell_area = Stat5::from_slice(&areas);
    geom.edge_length_km = Stat5::from_slice(&edge_lengths);
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
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        geometry: geom,
        topology: topo,
        gates,
        worst_cells,
        topology_issues,
        verdict,
    }
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

    let aspect_level = if geom.aspect_ratio.max >= th.aspect_ratio_fail {
        QualityLevel::Fail
    } else if geom.aspect_ratio.max >= th.aspect_ratio_warn {
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
