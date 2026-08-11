//! Experimental score-based / physics-aware refinement planner for EarthMesh v3.
//!
//! **INTEGRATION STATUS:** Hydro-delivery and project hydro closed-loop workflows
//! produce and consume score plans through the h-field adapter. General non-hydro
//! feature extraction and engine lowering remain future work.
//!
//! This does NOT replace the existing refinement workflow — it is an additive layer
//! that turns per-cell features into a `target_level` map via pluggable
//! [`RefinementCriterion`]s, a weighted composite score, a cell budget, and quality
//! constraints, then emits a decision report. The transition / isolation / max-jump
//! guarantees reuse `earthmesh_quality::topology` repair hooks. No optimizer, no
//! large external data dependency: criteria read pre-extracted feature columns.

use std::collections::{BTreeMap, BTreeSet};

use earthmesh_geometry::{haversine_km, Point};
use earthmesh_quality::topology::{
    enforce_max_adjacent_level_jump, remove_isolated_refined_cells, smooth_target_levels,
};

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
            } => {
                let full_longitude = (*east - *west).abs() >= 360.0;
                let normalize_lon = |lon: f64| (lon + 180.0).rem_euclid(360.0) - 180.0;
                let lon = normalize_lon(p.x);
                let west = normalize_lon(*west);
                let east = normalize_lon(*east);
                let inside_lon = if full_longitude {
                    true
                } else if west <= east {
                    lon >= west && lon <= east
                } else {
                    lon >= west || lon <= east
                };
                inside_lon && p.y >= *south && p.y <= *north
            }
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
    /// Stable mesh-cell identities. These survive filtering/reordering and are
    /// the join key for refinement plans; vector position is never identity.
    pub cell_ids: Vec<String>,
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

/// Copy-align one per-cell field to a target mesh order using stable cell IDs.
/// Both sides must describe the same identity set; interpolation and
/// recomputation belong in their own stage-specific operations.
pub fn align_cell_values_by_id<T: Clone>(
    source_ids: &[String],
    source_values: &[T],
    target_ids: &[String],
) -> Result<Vec<T>, String> {
    if source_ids.len() != source_values.len() {
        return Err(format!(
            "cell field has {} identities but {} values",
            source_ids.len(),
            source_values.len()
        ));
    }
    let target_set = target_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if target_set.len() != target_ids.len() {
        return Err("target mesh contains duplicate cell_id".into());
    }
    let mut by_id = BTreeMap::<&str, &T>::new();
    for (cell_id, value) in source_ids.iter().zip(source_values) {
        if cell_id.trim().is_empty() {
            return Err("cell field contains an empty cell_id".into());
        }
        if !target_set.contains(cell_id.as_str()) {
            return Err(format!("cell field references unknown cell_id {cell_id}"));
        }
        if by_id.insert(cell_id, value).is_some() {
            return Err(format!("cell field contains duplicate cell_id {cell_id}"));
        }
    }
    target_ids
        .iter()
        .map(|cell_id| {
            by_id
                .get(cell_id.as_str())
                .map(|value| (*value).clone())
                .ok_or_else(|| format!("cell field is missing target cell_id {cell_id}"))
        })
        .collect()
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
    pub reason: String,
}

/// A pluggable refinement driver.
pub trait RefinementCriterion: Send + Sync {
    fn metadata(&self) -> CriterionMetadata;
    fn score(&self, ctx: &CriterionContext, cell: usize) -> CellScore;
    fn required_column(&self) -> Option<&str> {
        None
    }
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
pub struct RefinementBudget {
    /// Max number of cells allowed to be refined (target_level > 0); None = unlimited.
    pub max_refined_cells: Option<usize>,
    pub max_adjacent_level_jump: u32,
}

impl Default for RefinementBudget {
    fn default() -> Self {
        Self {
            max_refined_cells: None,
            max_adjacent_level_jump: 1,
        }
    }
}

/// Planner eligibility and transition controls. Final mesh quality must be
/// measured after the plan is applied by an engine.
#[derive(Clone, Copy, Debug)]
pub struct QualityConstraint {
    /// Eligibility floor applied to measured input/candidate-mesh values from
    /// `cell_min_angle_deg`. This does not predict post-refinement geometry.
    pub min_input_angle_for_refinement_deg: Option<f64>,
    pub min_cell_area_m2: f64,
    pub max_adjacent_resolution_ratio: f64,
    pub no_isolated_refined: bool,
    pub smooth_transition: bool,
}

impl Default for QualityConstraint {
    fn default() -> Self {
        Self {
            min_input_angle_for_refinement_deg: None,
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
        let p = ctx.features.centroids[cell];
        let inside = ctx.features.regions.iter().any(|r| r.contains(p));
        CellScore {
            raw: inside as i32 as f64,
            demand: if inside { 1.0 } else { 0.0 },
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
        let raw = ctx.features.columns[&self.column][cell];
        let demand = raw.clamp(0.0, 1.0);
        CellScore {
            raw,
            demand,
            reason: format!("{}={:.3}", self.column, raw),
        }
    }

    fn required_column(&self) -> Option<&str> {
        Some(&self.column)
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
        let d = ctx.features.columns[&self.column][cell];
        let (demand, raw) = if self.length_km > 0.0 {
            ((-d / self.length_km).exp().clamp(0.0, 1.0), d)
        } else {
            (0.0, f64::NAN)
        };
        CellScore {
            raw,
            demand,
            reason: format!("{}={:.2}km", self.column, raw),
        }
    }

    fn required_column(&self) -> Option<&str> {
        Some(&self.column)
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
) -> Result<RefinementReport, String> {
    let n = features.cell_count;
    if features.cell_ids.len() != n {
        return Err(format!(
            "feature cell_ids length {} must equal cell_count {n}",
            features.cell_ids.len()
        ));
    }
    let mut unique_ids = BTreeSet::new();
    for (cell, id) in features.cell_ids.iter().enumerate() {
        if id.trim().is_empty() {
            return Err(format!("feature cell_id at row {cell} must not be empty"));
        }
        if !unique_ids.insert(id) {
            return Err(format!("feature cell_id '{id}' is duplicated"));
        }
    }
    if features.centroids.len() != n {
        return Err(format!(
            "feature centroids length {} must equal cell_count {n}",
            features.centroids.len()
        ));
    }
    if features
        .centroids
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err("feature centroids must contain only finite coordinates".into());
    }
    if features.neighbors.len() != n {
        return Err(format!(
            "feature neighbors length {} must equal cell_count {n}",
            features.neighbors.len()
        ));
    }
    for (cell, neighbors) in features.neighbors.iter().enumerate() {
        let mut unique_neighbors = BTreeSet::new();
        for &neighbor in neighbors {
            if neighbor >= n {
                return Err(format!(
                    "cell {cell} has neighbor index {neighbor} outside cell_count {n}"
                ));
            }
            if neighbor == cell {
                return Err(format!("cell {cell} must not list itself as a neighbor"));
            }
            if !unique_neighbors.insert(neighbor) {
                return Err(format!(
                    "cell {cell} lists neighbor {neighbor} more than once"
                ));
            }
            if !features.neighbors[neighbor].contains(&cell) {
                return Err(format!(
                    "cell {cell} lists neighbor {neighbor}, but cell {neighbor} does not reciprocate"
                ));
            }
        }
    }
    if let Some(min_angle_deg) = quality.min_input_angle_for_refinement_deg {
        if !min_angle_deg.is_finite() || min_angle_deg <= 0.0 || min_angle_deg >= 180.0 {
            return Err(
                "quality min_input_angle_for_refinement_deg must be finite and between 0 and 180"
                    .into(),
            );
        }
        let angles = features.columns.get("cell_min_angle_deg").ok_or_else(|| {
            "quality min_input_angle_for_refinement_deg requires missing measured feature column 'cell_min_angle_deg'".to_string()
        })?;
        if angles.len() != n {
            return Err(format!(
                "quality min_input_angle_for_refinement_deg requires exactly {n} measured values in feature column 'cell_min_angle_deg', found {}",
                angles.len()
            ));
        }
        if angles
            .iter()
            .any(|angle| !angle.is_finite() || *angle <= 0.0 || *angle > 180.0)
        {
            return Err(
                "quality min_input_angle_for_refinement_deg requires finite measured 'cell_min_angle_deg' values in (0, 180]"
                    .into(),
            );
        }
    }
    if !quality.min_cell_area_m2.is_finite() || quality.min_cell_area_m2 < 0.0 {
        return Err("quality min_cell_area_m2 must be finite and non-negative".into());
    }
    if !quality.max_adjacent_resolution_ratio.is_finite()
        || quality.max_adjacent_resolution_ratio < 2.0
    {
        return Err(
            "quality max_adjacent_resolution_ratio must be finite and at least 2 for discrete refinement levels"
                .into(),
        );
    }
    if quality.min_cell_area_m2 > 0.0 {
        let areas = features.columns.get("cell_area_m2").ok_or_else(|| {
            "quality min_cell_area_m2 requires missing feature column 'cell_area_m2'".to_string()
        })?;
        if areas.len() != n {
            return Err(format!(
                "quality min_cell_area_m2 requires exactly {n} values in feature column 'cell_area_m2', found {}",
                areas.len()
            ));
        }
        if areas.iter().any(|area| !area.is_finite() || *area <= 0.0) {
            return Err(
                "quality min_cell_area_m2 requires finite positive 'cell_area_m2' values".into(),
            );
        }
    }
    let mut criterion_ids = BTreeSet::new();
    for criterion in criteria {
        let id = criterion.metadata().id;
        if !criterion_ids.insert(id.clone()) {
            return Err(format!("criterion id '{id}' is duplicated"));
        }
    }
    let mut weight_ids = BTreeSet::new();
    for (id, weight) in &cfg.weights {
        if !criterion_ids.contains(id) {
            return Err(format!("weight references unknown criterion '{id}'"));
        }
        if !weight_ids.insert(id.as_str()) {
            return Err(format!("criterion weight '{id}' is duplicated"));
        }
        if !weight.is_finite() {
            return Err(format!("criterion weight '{id}' must be finite"));
        }
        if *weight < 0.0 {
            return Err(format!("criterion weight '{id}' must be non-negative"));
        }
    }

    let ctx = CriterionContext { features, domain };
    let weight_of = |id: &str| cfg.weights.iter().find(|(c, _)| c == id).map(|(_, w)| *w);
    let total_weight: f64 = cfg.weights.iter().map(|(_, w)| *w).sum::<f64>().max(1e-12);

    for criterion in criteria {
        let metadata = criterion.metadata();
        if weight_of(&metadata.id).is_none_or(|weight| weight <= 0.0) {
            continue;
        }
        if !metadata.applicable_domains.contains(&MeshDomain::Any)
            && !metadata.applicable_domains.contains(&domain)
        {
            return Err(format!(
                "criterion '{}' is not applicable to domain {domain:?}",
                metadata.id
            ));
        }
        if let Some(column) = criterion.required_column() {
            let values = features.columns.get(column).ok_or_else(|| {
                format!(
                    "criterion '{}' requires missing feature column '{column}'",
                    metadata.id
                )
            })?;
            if values.len() != n {
                return Err(format!(
                    "criterion '{}' requires exactly {n} values in feature column '{column}', found {}",
                    metadata.id,
                    values.len()
                ));
            }
            if values.iter().any(|value| !value.is_finite()) {
                return Err(format!(
                    "criterion '{}' requires finite values in feature column '{column}'",
                    metadata.id
                ));
            }
        }
    }

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
            if !s.raw.is_finite() {
                return Err(format!(
                    "criterion '{}' cell {cell} raw must be finite",
                    meta.id
                ));
            }
            if !s.demand.is_finite() {
                return Err(format!(
                    "criterion '{}' cell {cell} demand must be finite",
                    meta.id
                ));
            }
            if !(0.0..=1.0).contains(&s.demand) {
                return Err(format!(
                    "criterion '{}' cell {cell} demand must be between 0 and 1",
                    meta.id
                ));
            }
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
            let weighted_demand = w * s.demand;
            if weighted_demand > top.0 {
                top = (weighted_demand, meta.id.clone(), s.reason.clone());
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
        let area = &features.columns["cell_area_m2"];
        for d in decisions.iter_mut() {
            if d.final_level == 0 {
                continue;
            }
            let cell_area = area[d.cell];
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

    // The angle column describes the input/candidate mesh only. It is an
    // eligibility filter for refinement, not a claim about future child angles.
    if let Some(min_angle_deg) = quality.min_input_angle_for_refinement_deg {
        let angles = &features.columns["cell_min_angle_deg"];
        for decision in &mut decisions {
            if decision.final_level > 0 && angles[decision.cell] < min_angle_deg {
                decision.final_level = 0;
                decision.rejection_reason = Some("input_min_angle_for_refinement".into());
            }
        }
    }

    // 3. smoothing + transition + de-isolation reuse the quality topology repair hooks.
    let mut levels: Vec<u32> = decisions.iter().map(|d| d.final_level as u32).collect();
    if quality.smooth_transition {
        smooth_target_levels(&mut levels, &features.neighbors);
    }
    let ratio_jump = quality.max_adjacent_resolution_ratio.log2().floor() as u32;
    enforce_max_adjacent_level_jump(
        &mut levels,
        &features.neighbors,
        budget.max_adjacent_level_jump.min(ratio_jump),
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

    // Keep the budget as a postcondition even if future transition hooks add
    // bridge cells. Current hooks only lower levels, so this is intentionally a
    // cheap invariant guard rather than a new allocation algorithm.
    if let Some(max_refined) = budget.max_refined_cells {
        let mut refined: Vec<usize> = (0..n).filter(|&c| decisions[c].final_level > 0).collect();
        refined.sort_by(|&a, &b| {
            decisions[b]
                .composite_score
                .partial_cmp(&decisions[a].composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for &cell in refined.iter().skip(max_refined) {
            decisions[cell].final_level = 0;
            decisions[cell].rejection_reason = Some("budget".into());
            budget_hit = true;
        }
    }

    let cells_refined_after = decisions.iter().filter(|d| d.final_level > 0).count();
    let target_levels = TargetLevelMap {
        level: decisions.iter().map(|d| d.final_level).collect(),
        source: decisions.iter().map(|d| d.top_reason.clone()).collect(),
    };

    Ok(RefinementReport {
        decisions,
        target_levels,
        budget_used: BudgetUsage {
            cells_refined_before,
            cells_refined_after,
            budget_hit,
        },
        max_passes: cfg.max_passes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_field_alignment_uses_identity_not_row_order() {
        let source_ids = vec!["east".into(), "west".into()];
        let target_ids = vec!["west".into(), "east".into()];
        let aligned = align_cell_values_by_id(&source_ids, &[0_u8, 2], &target_ids)
            .expect("stable IDs align a per-cell field");
        assert_eq!(aligned, vec![2, 0]);
    }

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
            cell_ids: (0..n).map(|cell| cell.to_string()).collect(),
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

    struct FixedScoreCriterion(CellScore);

    impl RefinementCriterion for FixedScoreCriterion {
        fn metadata(&self) -> CriterionMetadata {
            CriterionMetadata {
                id: "fixed_score".into(),
                display_name: "Fixed score".into(),
                physical_process: "validation test".into(),
                applicable_domains: vec![MeshDomain::Any],
                units: "test".into(),
                version: "test".into(),
            }
        }

        fn score(&self, _ctx: &CriterionContext, _cell: usize) -> CellScore {
            self.0.clone()
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
        )
        .expect("valid feature table");
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
        )
        .expect("valid feature table");
        // cell0: entropy 1 + exp(0)=1 -> composite 1.0; cell1: entropy 0 + exp(-10)~0 -> ~0
        assert!((r.decisions[0].composite_score - 1.0).abs() < 1e-9);
        assert!(r.decisions[1].composite_score < 0.01);
    }

    #[test]
    fn planner_rejects_unknown_duplicate_or_nonfinite_weights() {
        let mut f = grid_features(1);
        f.columns.insert("landcover_entropy".into(), vec![1.0]);
        let criteria = vec![land_cover_entropy_criterion()];

        for (weights, expected) in [
            (vec![("unknown", 1.0)], "unknown criterion"),
            (
                vec![("land_cover_entropy", 1.0), ("land_cover_entropy", 2.0)],
                "duplicated",
            ),
            (vec![("land_cover_entropy", f64::NAN)], "finite"),
            (vec![("land_cover_entropy", -1.0)], "non-negative"),
        ] {
            let error = plan(
                &f,
                &criteria,
                &cfg(weights, 3),
                &RefinementBudget::default(),
                &QualityConstraint::default(),
                MeshDomain::Land,
            )
            .expect_err("invalid planner weights must be rejected");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn planner_rejects_nonfinite_criterion_scores_with_cell_context() {
        let mut features = grid_features(1);
        features
            .columns
            .insert("distance_to_river_km".into(), vec![0.0]);
        let criteria = vec![distance_to_river_criterion(0.0)];

        let error = plan(
            &features,
            &criteria,
            &cfg(vec![("distance_to_river", 1.0)], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Land,
        )
        .expect_err("non-finite criterion scores must not fail open");

        assert!(error.contains("distance_to_river"), "{error}");
        assert!(error.contains("cell 0"), "{error}");
        assert!(error.contains("raw must be finite"), "{error}");
    }

    #[test]
    fn planner_rejects_invalid_normalized_criterion_scores() {
        for (score, expected) in [
            (
                CellScore {
                    demand: f64::NAN,
                    ..Default::default()
                },
                "demand must be finite",
            ),
            (
                CellScore {
                    demand: -0.1,
                    ..Default::default()
                },
                "demand must be between 0 and 1",
            ),
            (
                CellScore {
                    demand: 1.1,
                    ..Default::default()
                },
                "demand must be between 0 and 1",
            ),
        ] {
            let criteria: Vec<Box<dyn RefinementCriterion>> =
                vec![Box::new(FixedScoreCriterion(score))];
            let error = plan(
                &grid_features(1),
                &criteria,
                &cfg(vec![("fixed_score", 1.0)], 1),
                &RefinementBudget::default(),
                &QualityConstraint::default(),
                MeshDomain::Any,
            )
            .expect_err("invalid normalized criterion score must be rejected");
            assert!(error.contains("fixed_score"), "{error}");
            assert!(error.contains("cell 0"), "{error}");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn dominant_reason_uses_weighted_contribution() {
        let mut f = grid_features(1);
        f.columns.insert("landcover_entropy".into(), vec![1.0]);
        f.columns
            .insert("distance_to_river_km".into(), vec![1.0536051566]);
        let criteria = vec![
            land_cover_entropy_criterion(),
            distance_to_river_criterion(10.0),
        ];
        let report = plan(
            &f,
            &criteria,
            &cfg(
                vec![("land_cover_entropy", 0.01), ("distance_to_river", 1.0)],
                3,
            ),
            &RefinementBudget::default(),
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect("valid weighted plan");

        assert!(
            report.decisions[0]
                .top_reason
                .starts_with("distance_to_river:"),
            "{}",
            report.decisions[0].top_reason
        );
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
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect("valid feature table");
        assert!(r.budget_used.budget_hit);
        assert_eq!(r.budget_used.cells_refined_after, 2);
        // lowest-score cells rejected for budget
        assert!(r.decisions[3].rejection_reason.as_deref() == Some("budget"));
    }

    #[test]
    fn missing_required_feature_column_is_an_error() {
        let f = grid_features(2);
        let err = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 2),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Land,
        )
        .expect_err("missing criterion input must not silently score as zero");

        assert!(err.contains("landcover_entropy"), "{err}");
    }

    #[test]
    fn missing_min_cell_area_column_is_an_error() {
        let mut f = grid_features(1);
        f.columns.insert("landcover_entropy".into(), vec![1.0]);
        let err = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 1),
            &RefinementBudget::default(),
            &QualityConstraint {
                min_cell_area_m2: 10.0,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect_err("enabled min-cell-area constraint requires its feature column");
        assert!(err.contains("cell_area_m2"), "{err}");
    }

    #[test]
    fn min_angle_constraint_requires_measured_feature_column() {
        let mut f = grid_features(1);
        f.columns.insert("landcover_entropy".into(), vec![1.0]);
        let err = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 1),
            &RefinementBudget::default(),
            &QualityConstraint {
                min_input_angle_for_refinement_deg: Some(20.0),
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect_err("min-angle constraint requires measured cell angles");
        assert!(err.contains("cell_min_angle_deg"), "{err}");
    }

    #[test]
    fn min_angle_constraint_rejects_only_ineligible_input_cells() {
        let mut f = grid_features(2);
        f.columns.insert("landcover_entropy".into(), vec![1.0, 1.0]);
        f.columns
            .insert("cell_min_angle_deg".into(), vec![10.0, 30.0]);
        let report = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 1),
            &RefinementBudget {
                max_adjacent_level_jump: 1,
                ..Default::default()
            },
            &QualityConstraint {
                min_input_angle_for_refinement_deg: Some(20.0),
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect("measured angle column satisfies the planner contract");

        assert_eq!(report.decisions[0].final_level, 0);
        assert_eq!(
            report.decisions[0].rejection_reason.as_deref(),
            Some("input_min_angle_for_refinement")
        );
        assert_eq!(report.decisions[1].final_level, 1);
    }

    #[test]
    fn planner_rejects_incomplete_or_invalid_feature_topology() {
        let mut f = grid_features(2);
        f.cell_ids[1] = f.cell_ids[0].clone();
        let err = plan(
            &f,
            &[],
            &cfg(vec![], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Any,
        )
        .expect_err("stable cell identities must be unique");
        assert!(err.contains("duplicated"), "{err}");

        let mut f = grid_features(2);
        f.centroids.pop();
        let err = plan(
            &f,
            &[],
            &cfg(vec![], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Any,
        )
        .expect_err("centroids must cover every cell");
        assert!(err.contains("centroids"), "{err}");

        let mut f = grid_features(2);
        f.neighbors.pop();
        let err = plan(
            &f,
            &[],
            &cfg(vec![], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Any,
        )
        .expect_err("neighbors must cover every cell");
        assert!(err.contains("neighbors length"), "{err}");

        let mut f = grid_features(2);
        f.neighbors[0] = vec![2];
        let err = plan(
            &f,
            &[],
            &cfg(vec![], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Any,
        )
        .expect_err("neighbor indices must be in range");
        assert!(err.contains("neighbor index 2"), "{err}");

        let mut f = grid_features(2);
        f.neighbors = vec![vec![1], vec![]];
        let err = plan(
            &f,
            &[],
            &cfg(vec![], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Any,
        )
        .expect_err("adjacency must be reciprocal before transition repair");
        assert!(err.contains("does not reciprocate"), "{err}");
    }

    #[test]
    fn criterion_domain_contract_is_enforced() {
        let mut f = grid_features(1);
        f.columns.insert("landcover_entropy".into(), vec![1.0]);
        let err = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 1),
            &RefinementBudget::default(),
            &QualityConstraint::default(),
            MeshDomain::Ocean,
        )
        .expect_err("land-only criterion must not run for ocean");
        assert!(err.contains("not applicable"), "{err}");
    }

    #[test]
    fn directed_bbox_contains_dateline_crossing_longitudes() {
        let bbox = RegionSpec::Bbox {
            west: 170.0,
            east: -170.0,
            south: -10.0,
            north: 10.0,
        };
        assert!(bbox.contains(Point::new(175.0, 0.0)));
        assert!(bbox.contains(Point::new(-175.0, 0.0)));
        assert!(!bbox.contains(Point::new(0.0, 0.0)));
    }

    #[test]
    fn full_longitude_bbox_does_not_collapse_to_one_meridian() {
        let bbox = RegionSpec::Bbox {
            west: -180.0,
            east: 180.0,
            south: -10.0,
            north: 10.0,
        };
        for lon in [-179.0, -45.0, 0.0, 120.0, 179.0] {
            assert!(bbox.contains(Point::new(lon, 0.0)), "lon={lon}");
        }
    }

    #[test]
    fn budget_remains_strict_with_transition_repairs_enabled() {
        let mut f = grid_features(5);
        f.columns
            .insert("landcover_entropy".into(), vec![1.0, 0.9, 0.8, 0.7, 0.6]);
        let r = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 3),
            &RefinementBudget {
                max_refined_cells: Some(2),
                max_adjacent_level_jump: 1,
            },
            &QualityConstraint {
                no_isolated_refined: false,
                smooth_transition: true,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect("valid feature table");

        assert!(r.budget_used.cells_refined_after <= 2);
        assert!(
            r.target_levels
                .level
                .iter()
                .filter(|&&level| level > 0)
                .count()
                <= 2
        );
    }

    #[test]
    fn quality_resolution_ratio_limits_adjacent_level_jump() {
        let mut f = grid_features(3);
        f.columns
            .insert("landcover_entropy".into(), vec![1.0, 0.0, 0.0]);
        let r = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 4),
            &RefinementBudget {
                max_adjacent_level_jump: 4,
                ..Default::default()
            },
            &QualityConstraint {
                max_adjacent_resolution_ratio: 2.0,
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .expect("valid feature table");
        assert!(r
            .target_levels
            .level
            .windows(2)
            .all(|pair| pair[0].abs_diff(pair[1]) <= 1));
    }

    #[test]
    fn quality_resolution_ratio_below_two_is_rejected_instead_of_flattening_refinement() {
        let mut f = grid_features(2);
        f.columns.insert("landcover_entropy".into(), vec![1.0, 0.0]);
        let error = plan(
            &f,
            &[land_cover_entropy_criterion()],
            &cfg(vec![("land_cover_entropy", 1.0)], 2),
            &RefinementBudget {
                max_adjacent_level_jump: 2,
                ..Default::default()
            },
            &QualityConstraint {
                max_adjacent_resolution_ratio: 1.5,
                no_isolated_refined: false,
                smooth_transition: false,
                ..Default::default()
            },
            MeshDomain::Land,
        )
        .unwrap_err();

        assert!(
            error.contains("at least 2 for discrete refinement levels"),
            "{error}"
        );
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
        )
        .expect("valid feature table");
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
        )
        .expect("valid feature table");
        // after smoothing/transition, adjacent levels differ by <= 1
        let lv = &r.target_levels.level;
        assert!(lv[0].abs_diff(lv[1]) <= 1);
        assert!(r.decisions[0].final_level < r.decisions[0].target_level);
    }
}
