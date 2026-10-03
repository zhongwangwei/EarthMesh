//! The point+radius route's demand, level by level, and the account of a run
//! that served it.
//!
//! The criteria are asked once per level, at the cell size that level will
//! produce, and what they ask for is reduced to circles (`LevelCircles`). That
//! reduction reads rasters and lives in the input layer; what it returns, and
//! what a backend reports back after building from it (`NestPassReport`,
//! `AdaptiveNestReport`), are the contract between the two, so they live here
//! with the other demand types. The backends that build from circles --
//! Method-C's adaptive nest, red-green, stretch and ICON nests -- read these
//! without reading a raster.

use earthmesh_mesh::RefinementRegion;

/// What the threshold criteria read at a level. It tells "nothing met a
/// threshold" -- an answer -- from "nothing was read" -- a source with no
/// data over the domain, which must not pass for one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CriteriaEvidence {
    /// A threshold criterion was judged.
    pub judged: bool,
    /// Valid (non-fill) source samples the judged criteria read.
    pub valid_source_samples: usize,
}

impl CriteriaEvidence {
    /// Criteria were judged and read no valid sample at all.
    pub fn read_no_data(&self) -> bool {
        self.judged && self.valid_source_samples == 0
    }

    /// The evidence of two windows of one level: judged if either was, and
    /// the larger count -- windows share one support evaluation, so their
    /// counts are the same reading and must not be added.
    pub fn merge(self, other: Self) -> Self {
        Self {
            judged: self.judged || other.judged,
            valid_source_samples: self.valid_source_samples.max(other.valid_source_samples),
        }
    }
}

/// What one pass did, so a run can say why it stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct NestPassReport {
    pub level: usize,
    /// Circles this pass handed to `spawn_nest`. Kept so the quality report can
    /// ask the same question afterwards -- did the mesh reach the level these
    /// circles asked for -- without re-planning the demand.
    pub regions: Vec<RefinementRegion>,
    /// Cell size this pass was judging — the generation it refines away.
    pub cell_meters: f64,
    pub circle_count: usize,
    pub demanded_cells: usize,
    pub faces_before: usize,
    pub faces_after: usize,
}

/// The whole adaptive run.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNestReport {
    pub passes: Vec<NestPassReport>,
    /// Level the run stopped at, zero if nothing was ever demanded.
    pub deepest_level: usize,
    /// True when the run stopped because a level demanded nothing, rather than
    /// because it hit `max_level`. A caller that cares about resolution wants
    /// to know which.
    pub stopped_on_empty_demand: bool,
    /// Nest spring passes actually run, summed over the levels.
    ///
    /// Zero when no spring was configured. The run report prints this beside
    /// the iteration count it was asked for, and the two disagreeing is the
    /// only way a caller can tell that a spring it configured did not run.
    pub spring_passes: usize,
    /// What the criteria read at level 1, so a run that refined nothing can
    /// tell "nothing met a threshold" from "no data was read".
    pub first_level_evidence: CriteriaEvidence,
    /// The level every cell of the domain is asked for, over a regional
    /// mother: the requested resolution, counted from the mother. Zero
    /// without one.
    pub domain_floor_level: usize,
}

/// What the criteria ask for at one level, before any backend sees it.
pub struct LevelCircles {
    /// Whether any criterion asked for anything at this level at all.
    ///
    /// Kept apart from `circles` being empty because the reduction can drop
    /// demand it cannot cover with a circle, and "nobody asked" and "asked but
    /// nothing survived" are different answers to a backend deciding whether it
    /// can serve the run.
    pub demanded: bool,
    /// Source cells the criteria asked for, for a caller that wants to say how
    /// much demand a level's circles came from.
    pub demanded_cells: usize,
    /// Circle radius this level uses. Reported when a level cannot be built, so
    /// the message can say what size failed.
    pub radius_meters: f64,
    pub circles: Vec<RefinementRegion>,
    /// Stable criterion ids that contributed at least one source cell.
    pub criterion_ids: Vec<String>,
    /// What the threshold criteria read, over every window.
    pub evidence: CriteriaEvidence,
}

impl AdaptiveNestReport {
    /// Deepest level whose circles cover this point, zero where none do.
    ///
    /// This is the target-level function the quality report reconciles against
    /// the mesh's actual levels. It reads the circles the run actually emitted
    /// rather than re-deriving them, so a discrepancy is a refinement failure
    /// and never a planning difference.
    pub fn target_level_at(&self, lon_degrees: f64, lat_degrees: f64) -> u32 {
        let mut deepest = 0u32;
        for pass in &self.passes {
            for region in &pass.regions {
                let RefinementRegion::Circle {
                    center,
                    radius_meters,
                    level,
                } = region
                else {
                    continue;
                };
                let distance = earthmesh_hfield::great_circle_distance_m(
                    center.lon_degrees,
                    center.lat_degrees,
                    lon_degrees,
                    lat_degrees,
                );
                if distance <= *radius_meters {
                    deepest = deepest.max(*level as u32);
                }
            }
        }
        deepest
    }

    pub fn circle_count(&self) -> usize {
        self.passes.iter().map(|pass| pass.circle_count).sum()
    }
}

impl AdaptiveNestReport {
    /// Serialize the circles this run emitted, for the quality step to read.
    pub fn to_json(&self, max_level: usize, base_meters: f64, coastline: bool) -> String {
        let passes = self
            .passes
            .iter()
            .map(|pass| {
                let circles = pass
                    .regions
                    .iter()
                    .filter_map(|region| match region {
                        RefinementRegion::Circle {
                            center,
                            radius_meters,
                            ..
                        } => Some(format!(
                            "{{\"lon\":{},\"lat\":{},\"radius_m\":{}}}",
                            center.lon_degrees, center.lat_degrees, radius_meters
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"level\":{},\"cell_meters\":{},\"demanded_cells\":{},\"circles\":[{circles}]}}",
                    pass.level, pass.cell_meters, pass.demanded_cells
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"enabled\":true,\"max_level\":{max_level},\"base_m\":{base_meters},\
             \"coastline\":{coastline},\"deepest_level\":{},\
             \"stopped_on_empty_demand\":{},\"floor_level\":{},\"passes\":[{passes}]}}",
            self.deepest_level, self.stopped_on_empty_demand, self.domain_floor_level
        )
    }
}

/// The regions level `level` marks: the named regions (asked for at
/// `>= level`, as always), this level's criteria circles, and every deeper
/// level's circles widened by the halo the rounds between will erode.
///
/// Round `k + 1` keeps only the marks that sit `halo(k + 1)` rings of the
/// round-`k` triangles (edge about `base / 2^k`) inside what round `k`
/// refined. A deeper circle marked at the same radius here would have its rim
/// cancelled later; widened by those rings, the level above holds it whole.
pub fn nested_criteria_regions(
    named_regions: &[RefinementRegion],
    planned_circles: &[LevelCircles],
    level: usize,
    base_cell_meters: f64,
    halo: impl Fn(usize) -> usize,
    widen_named: bool,
) -> Vec<RefinementRegion> {
    let margin = |deeper: usize| {
        (level..deeper)
            .map(|round| halo(round + 1) as f64 * base_cell_meters / 2f64.powi(round as i32))
            .sum::<f64>()
    };
    // Named regions are asked for at `>= level`, so a deeper one is marked
    // here too -- widened for the same reason as a deeper circle. Stretch
    // passes false: it marks nothing round by round.
    let mut regions = named_regions
        .iter()
        .flat_map(|region| {
            if widen_named && region.level() > level {
                widened_region(region, margin(region.level()))
            } else {
                vec![region.clone()]
            }
        })
        .collect::<Vec<_>>();
    for (index, demand) in planned_circles.iter().enumerate() {
        let planned_level = index + 1;
        if planned_level < level {
            continue;
        }
        let margin = margin(planned_level);
        regions.extend(
            demand
                .circles
                .iter()
                .flat_map(|circle| widened_region(circle, margin)),
        );
    }
    regions
}

/// `region` grown by `margin_meters` on every side, as the regions whose
/// union is the grown region.
///
/// Circles and corridors widen their radii; a bbox grows by the margin in
/// latitude and by the margin at its poleward edge in longitude (the whole
/// circle of longitude when that edge is near a pole). A polygon is kept and
/// joined by a corridor of that radius along its closed boundary: the two
/// together are exactly the polygon buffered outward by the margin.
pub fn widened_region(region: &RefinementRegion, margin_meters: f64) -> Vec<RefinementRegion> {
    if !(margin_meters.is_finite() && margin_meters > 0.0) {
        return vec![region.clone()];
    }
    let widened = match region {
        RefinementRegion::Circle {
            center,
            radius_meters,
            level,
        } => RefinementRegion::Circle {
            center: *center,
            radius_meters: radius_meters + margin_meters,
            level: *level,
        },
        RefinementRegion::Corridor {
            points,
            radius_meters,
            level,
        } => RefinementRegion::Corridor {
            points: points.clone(),
            radius_meters: radius_meters.iter().map(|r| r + margin_meters).collect(),
            level: *level,
        },
        RefinementRegion::Bbox {
            west_degrees,
            east_degrees,
            south_degrees,
            north_degrees,
            level,
        } => {
            let dlat = (margin_meters / earthmesh_hfield::EARTH_RADIUS_METERS).to_degrees();
            let south = (south_degrees - dlat).max(-90.0);
            let north = (north_degrees + dlat).min(90.0);
            let poleward = south.abs().max(north.abs());
            let span = east_degrees - west_degrees;
            let (west, east) = if poleward >= 89.0 {
                (-180.0, 180.0)
            } else {
                let dlon = dlat / poleward.to_radians().cos();
                let widened = if span >= 0.0 { span } else { span + 360.0 } + 2.0 * dlon;
                if widened >= 360.0 {
                    (-180.0, 180.0)
                } else {
                    (west_degrees - dlon, east_degrees + dlon)
                }
            };
            RefinementRegion::Bbox {
                west_degrees: west,
                east_degrees: east,
                south_degrees: south,
                north_degrees: north,
                level: *level,
            }
        }
        RefinementRegion::Polygon { points, level } => {
            let mut ring = points.clone();
            if let Some(&first) = points.first() {
                if points.last() != Some(&first) {
                    ring.push(first);
                }
            }
            if ring.len() < 2 {
                return vec![region.clone()];
            }
            let band = RefinementRegion::Corridor {
                radius_meters: vec![margin_meters; ring.len()],
                points: ring,
                level: *level,
            };
            return vec![region.clone(), band];
        }
    };
    vec![widened]
}
