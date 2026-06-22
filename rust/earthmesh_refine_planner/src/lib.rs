//! Score-based / physics-aware refinement planning **skeleton** for EarthMesh v3.
//!
//! This does NOT replace the existing refinement workflow — it is an additive layer
//! that turns per-cell features into a `target_level` map via pluggable
//! [`RefinementCriterion`]s, a weighted composite score, a cell budget, and quality
//! constraints, then emits a decision report. The transition / isolation / max-jump
//! guarantees reuse `earthmesh_quality::topology` repair hooks (R5). No optimizer, no
//! large external data dependency: criteria read pre-extracted feature columns.

use std::collections::BTreeMap;

use earthmesh_geometry::{haversine_km, Point};
use earthmesh_quality::topology::{
    enforce_max_adjacent_level_jump, remove_isolated_refined_cells, smooth_target_levels,
};

pub mod io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshDomain {
    Land,
    Ocean,
    Atmosphere,
    Coupled,
    Any,
}

/// A specified refinement region (concrete; not a placeholder).
#[derive(Clone, Debug)]
pub enum RegionSpec {
    Bbox {
        west: f64,
        east: f64,
        south: f64,
        north: f64,
    },
    Circle {
        lon: f64,
        lat: f64,
        radius_km: f64,
    },
}

impl RegionSpec {
    pub fn contains(&self, p: Point) -> bool {
        match self {
            RegionSpec::Bbox {
                west,
                east,
                south,
                north,
            } => p.x >= *west && p.x <= *east && p.y >= *south && p.y <= *north,
            RegionSpec::Circle {
                lon,
                lat,
                radius_km,
            } => haversine_km(p, Point::new(*lon, *lat)) <= *radius_km,
        }
    }
}

/// Engine-agnostic per-cell feature table fed to criteria.
#[derive(Clone, Debug, Default)]
pub struct CellFeatureTable {
    pub cell_count: usize,
    pub centroids: Vec<Point>,
    /// Named per-cell feature columns (e.g. "landcover_entropy", "distance_to_river_km").
    pub columns: BTreeMap<String, Vec<f64>>,
    /// Adjacency for transition / smoothing / isolation guarantees.
    pub neighbors: Vec<Vec<usize>>,
    /// Specified refinement regions (for `SpecifiedRegionCriterion`).
    pub regions: Vec<RegionSpec>,
}

impl CellFeatureTable {
    pub fn column(&self, key: &str) -> Option<&[f64]> {
        self.columns.get(key).map(|v| v.as_slice())
    }
}

#[derive(Clone, Debug)]
pub struct CriterionMetadata {
    pub id: String,
    pub display_name: String,
    /// Physical process this criterion targets (forces an explicit, GUI-showable why).
    pub physical_process: String,
    pub applicable_domains: Vec<MeshDomain>,
    pub units: String,
    pub version: String,
}

pub struct CriterionContext<'a> {
    pub features: &'a CellFeatureTable,
    pub domain: MeshDomain,
}

/// Per-cell score from one criterion. `demand` is self-normalized to 0..1.
#[derive(Clone, Debug, Default)]
pub struct CellScore {
    pub raw: f64,
    pub demand: f64,
    pub confidence: f64,
    pub reason: String,
}

/// A pluggable refinement driver.
pub trait RefinementCriterion: Send + Sync {
    fn metadata(&self) -> CriterionMetadata;
    fn score(&self, ctx: &CriterionContext, cell: usize) -> CellScore;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombineRule {
    WeightedSum,
    WeightedMax,
}

#[derive(Clone, Debug)]
pub struct CompositeScoreConfig {
    /// `(criterion_id, weight)` — weights need not sum to 1.
    pub weights: Vec<(String, f64)>,
    pub combine: CombineRule,
    pub max_passes: u8,
}

#[derive(Clone, Copy, Debug)]
pub enum AllocationMethod {
    /// Keep the highest-composite cells until the budget is spent.
    TopByScore,
}

#[derive(Clone, Copy, Debug)]
pub struct RefinementBudget {
    /// Max number of cells allowed to be refined (target_level > 0); None = unlimited.
    pub max_refined_cells: Option<usize>,
    pub max_adjacent_level_jump: u32,
    pub allocation: AllocationMethod,
}

impl Default for RefinementBudget {
    fn default() -> Self {
        Self {
            max_refined_cells: None,
            max_adjacent_level_jump: 1,
            allocation: AllocationMethod::TopByScore,
        }
    }
}

/// Quality constraints the plan must respect (representable; enforced where possible).
#[derive(Clone, Copy, Debug)]
pub struct QualityConstraint {
    pub min_angle_deg: f64,
    pub min_cell_area_m2: f64,
    pub max_adjacent_resolution_ratio: f64,
    pub no_isolated_refined: bool,
    pub smooth_transition: bool,
}

impl Default for QualityConstraint {
    fn default() -> Self {
        Self {
            min_angle_deg: 20.0,
            min_cell_area_m2: 0.0,
            max_adjacent_resolution_ratio: 2.0,
            no_isolated_refined: true,
            smooth_transition: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TargetLevelMap {
    pub level: Vec<u8>,
    /// Dominant criterion id per cell (the "why refine here").
    pub source: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RefinementDecision {
    pub cell: usize,
    pub raw_scores: Vec<(String, f64)>,
    pub normalized_scores: Vec<(String, f64)>,
    pub composite_score: f64,
    pub target_level: u8,
    pub final_level: u8,
    /// Why `final_level < target_level` (budget / min_cell_area / transition / isolated).
    pub rejection_reason: Option<String>,
    /// Dominant criterion id + its reason (GUI "why refine here").
    pub top_reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct BudgetUsage {
    pub cells_refined_before: usize,
    pub cells_refined_after: usize,
    pub budget_hit: bool,
}

#[derive(Clone, Debug)]
pub struct RefinementReport {
    pub decisions: Vec<RefinementDecision>,
    pub target_levels: TargetLevelMap,
    pub budget_used: BudgetUsage,
    pub max_passes: u8,
}

// ---------------------------------------------------------------------------
// Mock / simple criteria
// ---------------------------------------------------------------------------

/// Demand = 1.0 inside any specified region, else 0.0 (concrete, not a placeholder).
pub struct SpecifiedRegionCriterion;

impl RefinementCriterion for SpecifiedRegionCriterion {
    fn metadata(&self) -> CriterionMetadata {
        CriterionMetadata {
            id: "specified_region".into(),
            display_name: "Specified region".into(),
            physical_process: "user-specified area of interest".into(),
            applicable_domains: vec![MeshDomain::Any],
            units: "boolean".into(),
            version: "0.1.0".into(),
        }
    }
    fn score(&self, ctx: &CriterionContext, cell: usize) -> CellScore {
        let p = ctx
            .features
            .centroids
            .get(cell)
            .copied()
            .unwrap_or(Point::new(0.0, 0.0));
        let inside = ctx.features.regions.iter().any(|r| r.contains(p));
        CellScore {
            raw: inside as i32 as f64,
            demand: if inside { 1.0 } else { 0.0 },
            confidence: 1.0,
            reason: if inside {
                "inside specified region".into()
            } else {
                "outside".into()
            },
        }
    }
}

/// Reads a pre-extracted feature column, clamps to 0..1 as the demand (placeholder).
struct ColumnCriterion {
    id: String,
    display_name: String,
    physical_process: String,
    column: String,
    domains: Vec<MeshDomain>,
}

impl RefinementCriterion for ColumnCriterion {
    fn metadata(&self) -> CriterionMetadata {
        CriterionMetadata {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            physical_process: self.physical_process.clone(),
            applicable_domains: self.domains.clone(),
            units: "normalized".into(),
            version: "0.1.0".into(),
        }
    }
    fn score(&self, ctx: &CriterionContext, cell: usize) -> CellScore {
        let raw = ctx
            .features
            .column(&self.column)
            .and_then(|c| c.get(cell).copied())
            .unwrap_or(0.0);
        let demand = raw.clamp(0.0, 1.0);
        CellScore {
            raw,
            demand,
            confidence: if ctx.features.column(&self.column).is_some() {
                1.0
            } else {
                0.0
            },
            reason: format!("{}={:.3}", self.column, raw),
        }
    }
}

/// Distance-decay criterion: demand = exp(-distance/length) from a km-distance column.
struct DistanceCriterion {
    id: String,
    display_name: String,
    physical_process: String,
    column: String,
    length_km: f64,
    domains: Vec<MeshDomain>,
}

impl RefinementCriterion for DistanceCriterion {
    fn metadata(&self) -> CriterionMetadata {
        CriterionMetadata {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            physical_process: self.physical_process.clone(),
            applicable_domains: self.domains.clone(),
            units: "km".into(),
            version: "0.1.0".into(),
        }
    }
    fn score(&self, ctx: &CriterionContext, cell: usize) -> CellScore {
        let d = ctx
            .features
            .column(&self.column)
            .and_then(|c| c.get(cell).copied());
        let (demand, raw) = match d {
            Some(dist) if self.length_km > 0.0 => {
                ((-dist / self.length_km).exp().clamp(0.0, 1.0), dist)
            }
            _ => (0.0, f64::NAN),
        };
        CellScore {
            raw,
            demand,
            confidence: if d.is_some() { 1.0 } else { 0.0 },
            reason: format!("{}={:.2}km", self.column, raw),
        }
    }
}

pub fn land_cover_entropy_criterion() -> Box<dyn RefinementCriterion> {
    Box::new(ColumnCriterion {
        id: "land_cover_entropy".into(),
        display_name: "Land cover diversity".into(),
        physical_process: "surface-flux heterogeneity".into(),
        column: "landcover_entropy".into(),
        domains: vec![MeshDomain::Land],
    })
}
pub fn distance_to_river_criterion(length_km: f64) -> Box<dyn RefinementCriterion> {
    Box::new(DistanceCriterion {
        id: "distance_to_river".into(),
        display_name: "Proximity to rivers".into(),
        physical_process: "river routing / hydrology".into(),
        column: "distance_to_river_km".into(),
        length_km,
        domains: vec![MeshDomain::Land, MeshDomain::Coupled],
    })
}
pub fn distance_to_coast_criterion(length_km: f64) -> Box<dyn RefinementCriterion> {
    Box::new(DistanceCriterion {
        id: "distance_to_coast".into(),
        display_name: "Coastal proximity".into(),
        physical_process: "land-sea interaction / shallow dynamics".into(),
        column: "distance_to_coast_km".into(),
        length_km,
        domains: vec![MeshDomain::Ocean, MeshDomain::Coupled],
    })
}
pub fn hydro_coast_score_criterion() -> Box<dyn RefinementCriterion> {
    Box::new(ColumnCriterion {
        id: "hydro_coast_score".into(),
        display_name: "Hydro-coast score".into(),
        physical_process: "MERIT-Hydro river/coast importance".into(),
        column: "hydro_coast_score".into(),
        domains: vec![MeshDomain::Coupled],
    })
}
pub fn coupled_coast_criterion() -> Box<dyn RefinementCriterion> {
    Box::new(ColumnCriterion {
        id: "coupled_coast".into(),
        display_name: "Coupled coastline".into(),
        physical_process: "land-ocean coupling / coastline fidelity".into(),
        column: "coupled_coast_score".into(),
        domains: vec![MeshDomain::Coupled],
    })
}

// ---------------------------------------------------------------------------
// Planner
// ---------------------------------------------------------------------------

/// Plan a `target_level` map from features + criteria under a budget and quality
/// constraints. Returns a [`RefinementReport`] with per-cell decisions and reasons.
pub fn plan(
    features: &CellFeatureTable,
    criteria: &[Box<dyn RefinementCriterion>],
    cfg: &CompositeScoreConfig,
    budget: &RefinementBudget,
    quality: &QualityConstraint,
    domain: MeshDomain,
) -> RefinementReport {
    let n = features.cell_count;
    let ctx = CriterionContext { features, domain };
    let weight_of = |id: &str| cfg.weights.iter().find(|(c, _)| c == id).map(|(_, w)| *w);
    let total_weight: f64 = cfg.weights.iter().map(|(_, w)| *w).sum::<f64>().max(1e-12);

    let mut decisions: Vec<RefinementDecision> = Vec::with_capacity(n);
    for cell in 0..n {
        let mut raw_scores = Vec::new();
        let mut normalized = Vec::new();
        let mut composite = 0.0;
        let mut top: (f64, String, String) = (-1.0, String::new(), String::new());
        for crit in criteria {
            let meta = crit.metadata();
            let Some(w) = weight_of(&meta.id) else {
                continue;
            };
            if w <= 0.0 {
                continue;
            }
            let s = crit.score(&ctx, cell);
            raw_scores.push((meta.id.clone(), s.raw));
            normalized.push((meta.id.clone(), s.demand));
            let contribution = match cfg.combine {
                CombineRule::WeightedSum => w * s.demand,
                CombineRule::WeightedMax => (w * s.demand).max(composite * total_weight),
            };
            match cfg.combine {
                CombineRule::WeightedSum => composite += contribution / total_weight,
                CombineRule::WeightedMax => {
                    composite = (contribution / total_weight).max(composite)
                }
            }
            if s.demand > top.0 {
                top = (s.demand, meta.id.clone(), s.reason.clone());
            }
        }
        let composite = composite.clamp(0.0, 1.0);
        let target_level = (composite * cfg.max_passes as f64).round() as u8;
        decisions.push(RefinementDecision {
            cell,
            raw_scores,
            normalized_scores: normalized,
            composite_score: composite,
            target_level,
            final_level: target_level,
            rejection_reason: None,
            top_reason: if top.0 > 0.0 {
                format!("{}: {}", top.1, top.2)
            } else {
                "no criterion demand".into()
            },
        });
    }

    let cells_refined_before = decisions.iter().filter(|d| d.target_level > 0).count();

    // 1. budget: keep the highest-composite refined cells.
    let mut budget_hit = false;
    if let Some(max_refined) = budget.max_refined_cells {
        let mut refined: Vec<usize> = (0..n).filter(|&c| decisions[c].target_level > 0).collect();
        refined.sort_by(|&a, &b| {
            decisions[b]
                .composite_score
                .partial_cmp(&decisions[a].composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for &c in refined.iter().skip(max_refined) {
            decisions[c].final_level = 0;
            decisions[c].rejection_reason = Some("budget".into());
            budget_hit = true;
        }
    }

    // 2. quality floor: refining below min cell area is rejected (4^level area heuristic).
    if quality.min_cell_area_m2 > 0.0 {
        if let Some(area) = features.column("cell_area_m2") {
            for d in decisions.iter_mut() {
                if d.final_level == 0 {
                    continue;
                }
                let cell_area = area.get(d.cell).copied().unwrap_or(f64::INFINITY);
                let mut lvl = d.final_level;
                while lvl > 0 && cell_area / 4f64.powi(lvl as i32) < quality.min_cell_area_m2 {
                    lvl -= 1;
                }
                if lvl < d.final_level {
                    d.final_level = lvl;
                    if d.rejection_reason.is_none() {
                        d.rejection_reason = Some("min_cell_area".into());
                    }
                }
            }
        }
    }

    // 3. smoothing + transition + de-isolation reuse the quality topology repair hooks.
    let mut levels: Vec<u32> = decisions.iter().map(|d| d.final_level as u32).collect();
    if quality.smooth_transition {
        smooth_target_levels(&mut levels, &features.neighbors);
    }
    enforce_max_adjacent_level_jump(
        &mut levels,
        &features.neighbors,
        budget.max_adjacent_level_jump,
    );
    if quality.no_isolated_refined {
        remove_isolated_refined_cells(&mut levels, &features.neighbors);
    }
    for d in decisions.iter_mut() {
        let new = levels[d.cell] as u8;
        if new < d.final_level {
            if d.rejection_reason.is_none() {
                d.rejection_reason = Some("transition/isolation constraint".into());
            }
            d.final_level = new;
        } else if new > d.final_level {
            // a smoothing/transition pass may raise a level to bridge a gap
            d.final_level = new;
        }
    }

    let cells_refined_after = decisions.iter().filter(|d| d.final_level > 0).count();
    let target_levels = TargetLevelMap {
        level: decisions.iter().map(|d| d.final_level).collect(),
        source: decisions.iter().map(|d| d.top_reason.clone()).collect(),
    };

    RefinementReport {
        decisions,
        target_levels,
        budget_used: BudgetUsage {
            cells_refined_before,
            cells_refined_after,
            budget_hit,
        },
        max_passes: cfg.max_passes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_features(n: usize) -> CellFeatureTable {
        // a 1-D chain of cells, neighbors = prev/next
        let centroids = (0..n).map(|i| Point::new(i as f64, 0.0)).collect();
        let neighbors = (0..n)
            .map(|i| {
                let mut v = Vec::new();
                if i > 0 {
                    v.push(i - 1);
                }
                if i + 1 < n {
                    v.push(i + 1);
                }
                v
            })
            .collect();
        CellFeatureTable {
            cell_count: n,
            centroids,
            neighbors,
            ..Default::default()
        }
    }

    fn cfg(weights: Vec<(&str, f64)>, max_passes: u8) -> CompositeScoreConfig {
        CompositeScoreConfig {
            weights: weights
                .into_iter()
                .map(|(a, b)| (a.to_string(), b))
                .collect(),
            combine: CombineRule::WeightedSum,
            max_passes,
        }
    }

    #[test]
    fn one_criterion_score() {
        let mut f = grid_features(3);
        f.regions = vec![RegionSpec::Bbox {
            west: -0.5,
            east: 0.5,
            south: -1.0,
            north: 1.0,
        }];
        let crits: Vec<Box<dyn RefinementCriterion>> = vec![Box::new(SpecifiedRegionCriterion)];
        let r = plan(
            &f,
            &crits,
            &cfg(vec![("specified_region", 1.0)], 3),
            &RefinementBudget {
                max_adjacent_level_jump: 3,
                ..Default::default()
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Any,
        );
        assert_eq!(r.decisions[0].composite_score, 1.0);
        assert_eq!(r.decisions[0].target_level, 3);
        assert!(r.decisions[0].top_reason.contains("specified_region"));
        assert_eq!(r.decisions[2].composite_score, 0.0);
    }

    #[test]
    fn multiple_criteria_weighted_score() {
        let mut f = grid_features(2);
        f.columns.insert("landcover_entropy".into(), vec![1.0, 0.0]);
        f.columns
            .insert("distance_to_river_km".into(), vec![0.0, 100.0]); // cell0 on river
        let crits = vec![
            land_cover_entropy_criterion(),
            distance_to_river_criterion(10.0),
        ];
        let r = plan(
            &f,
            &crits,
            &cfg(
                vec![("land_cover_entropy", 1.0), ("distance_to_river", 1.0)],
                4,
            ),
            &RefinementBudget {
                max_adjacent_level_jump: 4,
                ..Default::default()
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        );
        // cell0: entropy 1 + exp(0)=1 -> composite 1.0; cell1: entropy 0 + exp(-10)~0 -> ~0
        assert!((r.decisions[0].composite_score - 1.0).abs() < 1e-9);
        assert!(r.decisions[1].composite_score < 0.01);
    }

    #[test]
    fn budget_constraint_caps_refined_cells() {
        let mut f = grid_features(4);
        f.columns
            .insert("landcover_entropy".into(), vec![0.9, 0.8, 0.7, 0.6]);
        let crits = vec![land_cover_entropy_criterion()];
        let r = plan(
            &f,
            &crits,
            &cfg(vec![("land_cover_entropy", 1.0)], 2),
            &RefinementBudget {
                max_refined_cells: Some(2),
                max_adjacent_level_jump: 4,
                ..Default::default()
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        );
        assert!(r.budget_used.budget_hit);
        assert_eq!(r.budget_used.cells_refined_after, 2);
        // lowest-score cells rejected for budget
        assert!(r.decisions[3].rejection_reason.as_deref() == Some("budget"));
    }

    #[test]
    fn quality_constraint_rejection_min_area() {
        let mut f = grid_features(1);
        f.columns.insert("landcover_entropy".into(), vec![1.0]);
        f.columns.insert("cell_area_m2".into(), vec![100.0]); // refining (÷4) -> 25 < min 50
        let crits = vec![land_cover_entropy_criterion()];
        let r = plan(
            &f,
            &crits,
            &cfg(vec![("land_cover_entropy", 1.0)], 1),
            &RefinementBudget {
                max_adjacent_level_jump: 4,
                ..Default::default()
            },
            &QualityConstraint {
                min_cell_area_m2: 50.0,
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        );
        assert_eq!(r.decisions[0].target_level, 1);
        assert_eq!(r.decisions[0].final_level, 0);
        assert_eq!(
            r.decisions[0].rejection_reason.as_deref(),
            Some("min_cell_area")
        );
    }

    #[test]
    fn target_level_smoothing_limits_adjacent_jump() {
        // cell0 wants level 3, neighbors want 0 -> enforce max jump 1 lowers cell0
        let mut f = grid_features(3);
        f.columns
            .insert("landcover_entropy".into(), vec![1.0, 0.0, 0.0]);
        let crits = vec![land_cover_entropy_criterion()];
        let r = plan(
            &f,
            &crits,
            &cfg(vec![("land_cover_entropy", 1.0)], 3),
            &RefinementBudget {
                max_adjacent_level_jump: 1,
                ..Default::default()
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: true,
                ..Default::default()
            },
            MeshDomain::Land,
        );
        // after smoothing/transition, adjacent levels differ by <= 1
        let lv = &r.target_levels.level;
        assert!(lv[0].abs_diff(lv[1]) <= 1);
        assert!(r.decisions[0].final_level < r.decisions[0].target_level);
    }

    #[test]
    fn report_output_csv_geojson_json() {
        let mut f = grid_features(2);
        f.regions = vec![RegionSpec::Bbox {
            west: -0.5,
            east: 0.5,
            south: -1.0,
            north: 1.0,
        }];
        let crits: Vec<Box<dyn RefinementCriterion>> = vec![Box::new(SpecifiedRegionCriterion)];
        let r = plan(
            &f,
            &crits,
            &cfg(vec![("specified_region", 1.0)], 2),
            &RefinementBudget {
                max_adjacent_level_jump: 2,
                ..Default::default()
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Any,
        );
        let csv = io::to_refinement_score_csv(&r);
        assert!(csv.starts_with(
            "cell,composite_score,target_level,final_level,rejection_reason,top_reason\n"
        ));
        let geojson = io::to_target_levels_geojson(&r, &f);
        assert!(geojson.contains("FeatureCollection"));
        assert!(geojson.contains("earthmesh_target_levels"));
        let json = io::to_refinement_decision_report_json(&r);
        assert!(json.contains("earthmesh_refinement_decision"));
        assert!(json.contains("cells_refined_after"));
    }
}
