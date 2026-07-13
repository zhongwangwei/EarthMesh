//! Land-ocean coupled mesh (LOCmesh) classification, fractions, coupling map and a
//! coupling-quality report (MVP).
//!
//! The LOCmesh production path writes this report beside its coupling CSV/NetCDF and
//! records it in the CoLM package manifest. The module remains dependency-light so
//! standalone hydro workflows can reuse the same classification and gates.
//!
//! Pure + dependency-light (no NetCDF, no GIS dependency): the caller builds
//! [`CoupledCellInput`]s from its mesh + landtype/MERIT/CaMa data, and this module
//! classifies cells, validates fractions (mass conservation), builds a simple
//! land<->ocean coupling map, and emits a [`CouplingQualityReport`] + CSV/JSON/manifest.
//! It deliberately does **not** rewrite the coupling algorithm or change existing
//! output formats — `coupling.nc` remains the CLI's existing CoLM writer.

use crate::QualityLevel;
use earthmesh_geometry::safety::{validate_fraction_partition, GeometryQualityFlag};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoupledCellClass {
    Land,
    Ocean,
    MixedCoast,
    Estuary,
    RiverMouth,
    WetlandDelta,
    Island,
    Unknown,
}

impl CoupledCellClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            CoupledCellClass::Land => "land",
            CoupledCellClass::Ocean => "ocean",
            CoupledCellClass::MixedCoast => "mixed_coast",
            CoupledCellClass::Estuary => "estuary",
            CoupledCellClass::RiverMouth => "river_mouth",
            CoupledCellClass::WetlandDelta => "wetland_delta",
            CoupledCellClass::Island => "island",
            CoupledCellClass::Unknown => "unknown",
        }
    }
}

/// Conservative (area) fractions for one cell. `land + ocean` is the primary
/// partition (should sum to 1); river/wetland/estuary are overlapping attributes.
#[derive(Clone, Debug, Default)]
pub struct CoupledCellFractions {
    pub land_fraction: f64,
    pub ocean_fraction: f64,
    pub river_fraction: f64,
    pub wetland_fraction: f64,
    pub estuary_fraction: f64,
    pub source_features: Vec<String>,
    pub quality_flags: Vec<String>,
}

/// One cell's coupling input.
#[derive(Clone, Debug, Default)]
pub struct CoupledCellInput {
    pub fractions: CoupledCellFractions,
    pub neighbors: Vec<usize>,
    /// From CaMa: this cell holds an estuary.
    pub is_estuary: bool,
    /// From CaMa: this cell is a river mouth (land side of a river outlet).
    pub is_river_mouth: bool,
    /// Ocean cell this river mouth drains into (if matched).
    pub outlet_ocean_cell: Option<usize>,
}

/// One land<->ocean coupling relationship.
#[derive(Clone, Debug)]
pub struct CouplingMap {
    pub land_cell_id: usize,
    pub ocean_cell_id: usize,
    pub overlap_fraction: f64,
    pub exchange_weight: f64,
    pub coupling_type: CouplingType,
    pub source_reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CouplingType {
    Coastline,
    RiverOutlet,
    Estuary,
    Other,
}

impl CouplingType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CouplingType::Coastline => "coastline",
            CouplingType::RiverOutlet => "river_outlet",
            CouplingType::Estuary => "estuary",
            CouplingType::Other => "other",
        }
    }
}

/// Classification thresholds (conservative defaults; future config can override).
#[derive(Clone, Copy, Debug)]
pub struct CoupledThresholds {
    pub pure_fraction: f64,
    pub wetland_min: f64,
    pub fraction_tolerance: f64,
}

impl Default for CoupledThresholds {
    fn default() -> Self {
        Self {
            pure_fraction: 0.98,
            wetland_min: 0.5,
            fraction_tolerance: 1.0e-6,
        }
    }
}

fn classify_by_fractions(c: &CoupledCellInput, th: &CoupledThresholds) -> CoupledCellClass {
    let f = &c.fractions;
    if c.is_river_mouth {
        return CoupledCellClass::RiverMouth;
    }
    if c.is_estuary || f.estuary_fraction >= th.wetland_min {
        return CoupledCellClass::Estuary;
    }
    if f.wetland_fraction >= th.wetland_min {
        return CoupledCellClass::WetlandDelta;
    }
    if f.land_fraction >= th.pure_fraction {
        return CoupledCellClass::Land;
    }
    if f.ocean_fraction >= th.pure_fraction {
        return CoupledCellClass::Ocean;
    }
    if f.land_fraction > 0.0 && f.ocean_fraction > 0.0 {
        return CoupledCellClass::MixedCoast;
    }
    CoupledCellClass::Unknown
}

/// Two-pass classification: by fractions, then reclassify pure-Land cells whose every
/// neighbor is Ocean as Island.
pub fn classify_all(cells: &[CoupledCellInput], th: &CoupledThresholds) -> Vec<CoupledCellClass> {
    let mut classes: Vec<CoupledCellClass> =
        cells.iter().map(|c| classify_by_fractions(c, th)).collect();
    let first_pass = classes.clone();
    for (ci, cell) in cells.iter().enumerate() {
        if first_pass[ci] == CoupledCellClass::Land
            && !cell.neighbors.is_empty()
            && cell
                .neighbors
                .iter()
                .all(|&nb| matches!(first_pass.get(nb), Some(CoupledCellClass::Ocean)))
        {
            classes[ci] = CoupledCellClass::Island;
        }
    }
    classes
}

/// Build a simple land<->ocean coupling map: coastline pairs (mixed/coast land cell
/// adjacent to an ocean cell) and river-outlet pairs (river mouth -> outlet ocean cell).
pub fn build_coupling_map(
    cells: &[CoupledCellInput],
    classes: &[CoupledCellClass],
) -> Vec<CouplingMap> {
    let mut maps = Vec::new();
    for (ci, cell) in cells.iter().enumerate() {
        let class = classes
            .get(ci)
            .copied()
            .unwrap_or(CoupledCellClass::Unknown);
        // river-outlet coupling
        if matches!(
            class,
            CoupledCellClass::RiverMouth | CoupledCellClass::Estuary
        ) {
            if let Some(ocean) = cell.outlet_ocean_cell {
                maps.push(CouplingMap {
                    land_cell_id: ci,
                    ocean_cell_id: ocean,
                    overlap_fraction: cell
                        .fractions
                        .river_fraction
                        .max(cell.fractions.estuary_fraction),
                    exchange_weight: cell
                        .fractions
                        .ocean_fraction
                        .max(cell.fractions.river_fraction),
                    coupling_type: if matches!(class, CoupledCellClass::Estuary) {
                        CouplingType::Estuary
                    } else {
                        CouplingType::RiverOutlet
                    },
                    source_reason: "river/estuary outlet to ocean cell".to_string(),
                });
            }
        }
        // coastline coupling: mixed coast land cell <-> adjacent ocean cell
        if matches!(class, CoupledCellClass::MixedCoast) {
            let ocean_neighbors = cell
                .neighbors
                .iter()
                .copied()
                .filter(|&nb| {
                    nb != ci
                        && matches!(
                            classes.get(nb),
                            Some(CoupledCellClass::Ocean | CoupledCellClass::MixedCoast)
                        )
                })
                .collect::<Vec<_>>();
            let pair_fraction = if ocean_neighbors.is_empty() {
                0.0
            } else {
                cell.fractions.ocean_fraction / ocean_neighbors.len() as f64
            };
            for nb in ocean_neighbors {
                maps.push(CouplingMap {
                    land_cell_id: ci,
                    ocean_cell_id: nb,
                    overlap_fraction: pair_fraction,
                    exchange_weight: pair_fraction,
                    coupling_type: CouplingType::Coastline,
                    source_reason: "coastline adjacency".to_string(),
                });
            }
        }
    }
    maps
}

/// Max over source cells of `(sum of outgoing exchange weight) - 1`, clamped at
/// zero. Multiple land cells may legitimately exchange with the same ocean
/// cell; conservation constrains each source allocation, not the aggregate
/// incoming weight of an arbitrary shared neighbor.
pub fn max_ocean_oversubscription(maps: &[CouplingMap]) -> f64 {
    use std::collections::BTreeMap;
    let mut outgoing: BTreeMap<usize, f64> = BTreeMap::new();
    for m in maps {
        *outgoing.entry(m.land_cell_id).or_insert(0.0) += m.exchange_weight;
    }
    outgoing
        .values()
        .map(|&w| (w - 1.0).max(0.0))
        .fold(0.0_f64, f64::max)
}

#[derive(Clone, Debug, Default)]
pub struct CouplingQualityReport {
    pub total_land_cells: usize,
    pub total_ocean_cells: usize,
    pub mixed_coastline_cells: usize,
    pub coast_overlap_cells: usize,
    pub river_mouth_cells: usize,
    pub estuary_cells: usize,
    pub unresolved_fractional_area: f64,
    pub land_fraction_error: f64,
    pub sea_fraction_error: f64,
    pub coupling_row_count: usize,
    pub orphan_land_cells: usize,
    pub orphan_ocean_cells: usize,
    pub mass_conservation_residual: f64,
    pub outlet_matching_error: f64,
    pub coastline_preservation_score: f64,
    pub river_ocean_connectivity_score: f64,
    pub verdict: QualityLevel,
}

/// Out-of-[0,1] amount for a fraction (0 if in range).
fn range_error(f: f64) -> f64 {
    (-f).max(0.0) + (f - 1.0).max(0.0)
}

pub fn build_coupling_quality(
    cells: &[CoupledCellInput],
    classes: &[CoupledCellClass],
    maps: &[CouplingMap],
    th: &CoupledThresholds,
) -> CouplingQualityReport {
    let mut r = CouplingQualityReport {
        coupling_row_count: maps.len(),
        coastline_preservation_score: 1.0,
        river_ocean_connectivity_score: 1.0,
        verdict: QualityLevel::Pass,
        ..Default::default()
    };

    let mut mixed_with_ocean_neighbor = 0usize;
    let mut river_mouth_total = 0usize;
    let mut river_mouth_matched = 0usize;

    for (ci, cell) in cells.iter().enumerate() {
        let class = classes
            .get(ci)
            .copied()
            .unwrap_or(CoupledCellClass::Unknown);
        let f = &cell.fractions;
        match class {
            CoupledCellClass::Land | CoupledCellClass::Island | CoupledCellClass::WetlandDelta => {
                r.total_land_cells += 1
            }
            CoupledCellClass::Ocean => r.total_ocean_cells += 1,
            CoupledCellClass::MixedCoast => {
                r.total_land_cells += 1;
                r.mixed_coastline_cells += 1;
                if cell.neighbors.iter().any(|&nb| {
                    matches!(
                        classes.get(nb),
                        Some(CoupledCellClass::Ocean | CoupledCellClass::MixedCoast)
                    )
                }) {
                    mixed_with_ocean_neighbor += 1;
                }
            }
            CoupledCellClass::Estuary => {
                r.estuary_cells += 1;
                r.total_land_cells += 1;
            }
            CoupledCellClass::RiverMouth => {
                r.river_mouth_cells += 1;
                r.total_land_cells += 1;
            }
            CoupledCellClass::Unknown => {}
        }

        if f.land_fraction > 0.0 && f.ocean_fraction > 0.0 {
            r.coast_overlap_cells += 1;
            let resolved = cell.neighbors.iter().any(|&nb| {
                matches!(
                    classes.get(nb),
                    Some(CoupledCellClass::Ocean | CoupledCellClass::MixedCoast)
                )
            });
            if !resolved {
                r.unresolved_fractional_area += f.land_fraction.min(f.ocean_fraction);
            }
        }

        r.land_fraction_error = r.land_fraction_error.max(range_error(f.land_fraction));
        r.sea_fraction_error = r.sea_fraction_error.max(range_error(f.ocean_fraction));

        // mass conservation: land + ocean should sum to 1 (within tolerance)
        let resid = (1.0 - (f.land_fraction + f.ocean_fraction)).abs();
        r.mass_conservation_residual = r.mass_conservation_residual.max(resid);

        // orphan: a classified cell with no neighbors at all (disconnected)
        if cell.neighbors.is_empty() {
            match class {
                CoupledCellClass::Ocean => r.orphan_ocean_cells += 1,
                CoupledCellClass::Land
                | CoupledCellClass::MixedCoast
                | CoupledCellClass::Estuary
                | CoupledCellClass::RiverMouth
                | CoupledCellClass::WetlandDelta
                | CoupledCellClass::Island => r.orphan_land_cells += 1,
                CoupledCellClass::Unknown => {}
            }
        }

        if cell.is_river_mouth {
            river_mouth_total += 1;
            if cell.outlet_ocean_cell.is_some() {
                river_mouth_matched += 1;
            }
        }
    }

    if r.mixed_coastline_cells > 0 {
        r.coastline_preservation_score =
            mixed_with_ocean_neighbor as f64 / r.mixed_coastline_cells as f64;
    }
    if river_mouth_total > 0 {
        r.outlet_matching_error =
            (river_mouth_total - river_mouth_matched) as f64 / river_mouth_total as f64;
        r.river_ocean_connectivity_score = river_mouth_matched as f64 / river_mouth_total as f64;
    }

    // verdict
    let oversub = max_ocean_oversubscription(maps);
    if r.mass_conservation_residual > th.fraction_tolerance
        || r.land_fraction_error > th.fraction_tolerance
        || r.sea_fraction_error > th.fraction_tolerance
        || r.orphan_land_cells > 0
        || r.orphan_ocean_cells > 0
        || oversub > th.fraction_tolerance
    {
        r.verdict = QualityLevel::Fail;
    } else if r.outlet_matching_error > 0.0
        || r.coastline_preservation_score < 1.0
        || r.unresolved_fractional_area > 0.0
    {
        r.verdict = QualityLevel::Warn;
    }
    r
}

/// `coupling.csv`: one row per cell (class + fractions).
pub fn to_coupling_csv(cells: &[CoupledCellInput], classes: &[CoupledCellClass]) -> String {
    let mut s = String::from(
        "cell_id,class,land_fraction,ocean_fraction,river_fraction,wetland_fraction,estuary_fraction\n",
    );
    for (ci, cell) in cells.iter().enumerate() {
        let f = &cell.fractions;
        s.push_str(&format!(
            "{ci},{},{},{},{},{},{}\n",
            classes
                .get(ci)
                .copied()
                .unwrap_or(CoupledCellClass::Unknown)
                .as_str(),
            f.land_fraction,
            f.ocean_fraction,
            f.river_fraction,
            f.wetland_fraction,
            f.estuary_fraction
        ));
    }
    s
}

/// `coupling_quality.json`.
pub fn to_coupling_quality_json(r: &CouplingQualityReport) -> String {
    let n = |v: f64| {
        if v.is_finite() {
            format!("{v}")
        } else {
            "null".into()
        }
    };
    format!(
        "{{\n  \"kind\": \"earthmesh_coupling_quality\",\n  \"signal_scope\": \"landtype_grid_only\",\n  \"hydro_semantics_included\": false,\n  \"verdict\": \"{}\",\n  \
         \"total_land_cells\": {},\n  \"total_ocean_cells\": {},\n  \"mixed_coastline_cells\": {},\n  \
         \"coast_overlap_cells\": {},\n  \"river_mouth_cells\": {},\n  \"estuary_cells\": {},\n  \
         \"unresolved_fractional_area\": {},\n  \"land_fraction_error\": {},\n  \"sea_fraction_error\": {},\n  \
         \"coupling_row_count\": {},\n  \"orphan_land_cells\": {},\n  \"orphan_ocean_cells\": {},\n  \
         \"mass_conservation_residual\": {},\n  \"outlet_matching_error\": {},\n  \
         \"coastline_preservation_score\": {},\n  \"river_ocean_connectivity_score\": {}\n}}\n",
        r.verdict.as_str(),
        r.total_land_cells,
        r.total_ocean_cells,
        r.mixed_coastline_cells,
        r.coast_overlap_cells,
        r.river_mouth_cells,
        r.estuary_cells,
        n(r.unresolved_fractional_area),
        n(r.land_fraction_error),
        n(r.sea_fraction_error),
        r.coupling_row_count,
        r.orphan_land_cells,
        r.orphan_ocean_cells,
        n(r.mass_conservation_residual),
        n(r.outlet_matching_error),
        n(r.coastline_preservation_score),
        n(r.river_ocean_connectivity_score),
    )
}

/// `coupling_manifest.json`: lists produced artifacts + a one-line summary.
pub fn to_coupling_manifest_json(products: &[(&str, &str)], verdict: QualityLevel) -> String {
    let mut s = String::from("{\n  \"kind\": \"earthmesh_coupling_manifest\",\n");
    s.push_str(&format!(
        "  \"verdict\": \"{}\",\n  \"products\": {{",
        verdict.as_str()
    ));
    if products.is_empty() {
        s.push_str("}\n}\n");
        return s;
    }
    s.push('\n');
    for (i, (role, path)) in products.iter().enumerate() {
        let comma = if i + 1 < products.len() { "," } else { "" };
        s.push_str(&format!(
            "    \"{}\": \"{}\"{}\n",
            role,
            path.replace('\\', "\\\\").replace('"', "\\\""),
            comma
        ));
    }
    s.push_str("  }\n}\n");
    s
}

/// Annotate a cell's fractions with a sum-tolerance quality flag (does not mutate input).
pub fn fraction_quality_flags(
    f: &CoupledCellFractions,
    tolerance: f64,
) -> Vec<GeometryQualityFlag> {
    validate_fraction_partition(&[f.land_fraction, f.ocean_fraction], tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(land: f64, ocean: f64) -> CoupledCellInput {
        CoupledCellInput {
            fractions: CoupledCellFractions {
                land_fraction: land,
                ocean_fraction: ocean,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn pure_land_ocean_mixed_classification() {
        let th = CoupledThresholds::default();
        assert_eq!(
            classify_by_fractions(&cell(1.0, 0.0), &th),
            CoupledCellClass::Land
        );
        assert_eq!(
            classify_by_fractions(&cell(0.0, 1.0), &th),
            CoupledCellClass::Ocean
        );
        assert_eq!(
            classify_by_fractions(&cell(0.5, 0.5), &th),
            CoupledCellClass::MixedCoast
        );
    }

    #[test]
    fn island_when_land_surrounded_by_ocean() {
        let cells = vec![
            CoupledCellInput {
                neighbors: vec![1, 2],
                ..cell(1.0, 0.0)
            }, // land surrounded
            cell(0.0, 1.0),
            cell(0.0, 1.0),
        ];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        assert_eq!(classes[0], CoupledCellClass::Island);
    }

    #[test]
    fn fraction_sum_tolerance_flagged() {
        let f = CoupledCellFractions {
            land_fraction: 0.6,
            ocean_fraction: 0.6,
            ..Default::default()
        };
        let flags = fraction_quality_flags(&f, 1.0e-6);
        assert!(flags.contains(&GeometryQualityFlag::UnresolvedFractionSumError));
    }

    #[test]
    fn mass_conservation_residual_is_fail_when_sum_off() {
        let cells = vec![cell(0.6, 0.6)]; // sum 1.2
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let r = build_coupling_quality(&cells, &classes, &[], &CoupledThresholds::default());
        assert!(r.mass_conservation_residual > 1.0e-6);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn orphan_land_cell_detected() {
        let cells = vec![CoupledCellInput {
            neighbors: vec![],
            ..cell(1.0, 0.0)
        }];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let r = build_coupling_quality(&cells, &classes, &[], &CoupledThresholds::default());
        assert_eq!(r.orphan_land_cells, 1);
        assert_eq!(r.verdict, QualityLevel::Fail);
    }

    #[test]
    fn orphan_ocean_cell_detected() {
        let cells = vec![CoupledCellInput {
            neighbors: vec![],
            ..cell(0.0, 1.0)
        }];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let r = build_coupling_quality(&cells, &classes, &[], &CoupledThresholds::default());
        assert_eq!(r.orphan_ocean_cells, 1);
    }

    #[test]
    fn simple_coupling_map_mass_conservation() {
        // one mixed-coast land cell coupled to one ocean cell, weight 0.5 -> balanced
        let cells = vec![
            CoupledCellInput {
                neighbors: vec![1],
                ..cell(0.5, 0.5)
            },
            CoupledCellInput {
                neighbors: vec![0],
                ..cell(0.0, 1.0)
            },
        ];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let maps = build_coupling_map(&cells, &classes);
        assert!(!maps.is_empty());
        assert!(maps
            .iter()
            .any(|m| m.coupling_type == CouplingType::Coastline));
        assert_eq!(max_ocean_oversubscription(&maps), 0.0);
    }

    #[test]
    fn coastline_exchange_is_partitioned_per_source_not_rejected_per_shared_ocean() {
        let cells = vec![
            CoupledCellInput {
                neighbors: vec![2],
                ..cell(0.2, 0.8)
            },
            CoupledCellInput {
                neighbors: vec![2],
                ..cell(0.2, 0.8)
            },
            CoupledCellInput {
                neighbors: vec![0, 1],
                ..cell(0.0, 1.0)
            },
        ];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let maps = build_coupling_map(&cells, &classes);
        assert_eq!(maps.len(), 2);
        assert!(maps.iter().all(|map| map.exchange_weight == 0.8));
        assert_eq!(max_ocean_oversubscription(&maps), 0.0);
        let report = build_coupling_quality(&cells, &classes, &maps, &CoupledThresholds::default());
        assert_eq!(report.unresolved_fractional_area, 0.0);
        assert_eq!(report.coastline_preservation_score, 1.0);
        assert_eq!(report.verdict, QualityLevel::Pass);
    }

    #[test]
    fn river_mouth_to_ocean_matching_smoke() {
        let cells = vec![
            CoupledCellInput {
                fractions: CoupledCellFractions {
                    land_fraction: 0.7,
                    ocean_fraction: 0.3,
                    river_fraction: 0.4,
                    ..Default::default()
                },
                neighbors: vec![1],
                is_estuary: false,
                is_river_mouth: true,
                outlet_ocean_cell: Some(1),
            },
            cell(0.0, 1.0),
        ];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        assert_eq!(classes[0], CoupledCellClass::RiverMouth);
        let maps = build_coupling_map(&cells, &classes);
        assert!(maps
            .iter()
            .any(|m| m.coupling_type == CouplingType::RiverOutlet && m.ocean_cell_id == 1));
        let r = build_coupling_quality(&cells, &classes, &maps, &CoupledThresholds::default());
        assert_eq!(r.river_mouth_cells, 1);
        assert_eq!(r.outlet_matching_error, 0.0);
        assert_eq!(r.river_ocean_connectivity_score, 1.0);
    }

    #[test]
    fn unmatched_river_mouth_lowers_connectivity() {
        let cells = vec![CoupledCellInput {
            fractions: CoupledCellFractions {
                land_fraction: 0.7,
                ocean_fraction: 0.3,
                ..Default::default()
            },
            neighbors: vec![],
            is_estuary: false,
            is_river_mouth: true,
            outlet_ocean_cell: None,
        }];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let r = build_coupling_quality(&cells, &classes, &[], &CoupledThresholds::default());
        assert!(r.outlet_matching_error > 0.0);
        assert!(r.river_ocean_connectivity_score < 1.0);
    }

    #[test]
    fn coupling_outputs_have_fields() {
        let cells = vec![cell(0.5, 0.5), cell(0.0, 1.0)];
        let classes = classify_all(&cells, &CoupledThresholds::default());
        let maps = build_coupling_map(&cells, &classes);
        let r = build_coupling_quality(&cells, &classes, &maps, &CoupledThresholds::default());
        let csv = to_coupling_csv(&cells, &classes);
        assert!(csv.starts_with("cell_id,class,land_fraction"));
        assert!(csv.contains("mixed_coast"));
        let json = to_coupling_quality_json(&r);
        assert!(json.contains("earthmesh_coupling_quality"));
        assert!(json.contains("mass_conservation_residual"));
        let manifest = to_coupling_manifest_json(&[("coupling_csv", "/x/coupling.csv")], r.verdict);
        assert!(manifest.contains("earthmesh_coupling_manifest"));
        assert!(manifest.contains("coupling_csv"));
    }
}
