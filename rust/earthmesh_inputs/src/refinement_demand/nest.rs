//! Refine one level at a time, asking the criteria again before each pass.
//!
//! Whether a cell needs to be refined further is a question about that cell, so
//! it cannot be settled before the cell exists. The h-field settles everything
//! up front — one field, quantised once — which is why a criterion like
//! land-cover heterogeneity had nowhere to say "the cells I just made are still
//! too mixed". Here each pass re-plans against the generation it is about to
//! refine, and stops as soon as nothing asks for more.
//!
//! The demand for a level is reduced to circles and handed to `spawn_nest` on
//! its own, so the mesh grows one level per call. `spawn_nest` refines from
//! whatever it is given, so chaining is the same operation the engine already
//! performs internally between passes — the only difference is that the regions
//! for pass N+1 are computed after pass N instead of before pass 1.
//!
//! This is the regrid loop of structured AMR (Berger & Oliger 1984) applied to
//! a static field: there the grid is rebuilt because the solution moved, here
//! because the criterion's answer depends on the cell size it is asked at. See
//! the module docs of the parent for the full lineage.
//!
//! What this does **not** do is read the criterion off the refined mesh's own
//! cells. That needs raster-to-cell statistics the port does not have (see the
//! technical guide on `getref_mean_std`), so the scale is carried as a length
//! and the criterion is asked over a matching neighbourhood of the source
//! raster. The size is right; the placement is grid-aligned rather than
//! cell-aligned.

use std::io;

use earthmesh_mesh::RefinementRegion;

use super::ladder::nested_circle_radii_meters;
use super::plan::{plan_demand_at_scale_for_windows, DemandPlanInputs};
use super::reduce_demand_to_circles_on_blocks;
use earthmesh_core::RefineConfig;

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
}

/// Re-ask the criteria at the cell size this level will produce, and reduce what
/// they demand to circles.
///
/// This is the half of the point+radius route that does not depend on the
/// backend: it is raster work, and the circles that come out are an ordinary
/// region list. What is Method-C's is the other half -- turning those circles
/// into mesh -- which is the half that is suspended.
///
/// The levels nest by construction: every level blocks on the finest radius so
/// the centres coincide, and only the radius changes. A backend that holds a
/// deeper level inside the one above it can therefore take these at face value.
pub fn adaptive_demand_circles_for_level(
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    level: usize,
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<LevelCircles> {
    adaptive_demand_circles_for_level_windows(
        refine,
        std::slice::from_ref(inputs),
        level,
        base_cell_meters,
        max_level,
    )
}

pub fn adaptive_demand_circles_for_level_windows(
    refine: &RefineConfig,
    inputs: &[DemandPlanInputs<'_>],
    level: usize,
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<LevelCircles> {
    let radii = nested_circle_radii_meters(base_cell_meters, max_level)?;
    // The cell this pass refines away is the one the previous level left.
    let cell_meters = base_cell_meters / 2f64.powi((level - 1) as i32);
    adaptive_demand_circles_for_level_windows_at_radius(
        refine,
        inputs,
        level,
        cell_meters,
        radii[level - 1],
        radii[max_level - 1],
    )
}

/// Re-ask the criteria and cover their source cells with caller-sized circles.
///
/// Method-C needs its measured 2.5-cell seed radius; Red-Green does not. Keeping
/// the radius policy outside the shared raster scan prevents a backend's mesh
/// constraint from broadening another backend's requested region.
pub fn adaptive_demand_circles_for_level_windows_at_radius(
    refine: &RefineConfig,
    inputs: &[DemandPlanInputs<'_>],
    level: usize,
    cell_meters: f64,
    radius_meters: f64,
    block_radius_meters: f64,
) -> io::Result<LevelCircles> {
    let mut demanded_cells = 0usize;
    let mut circles = Vec::new();
    let mut criterion_ids = std::collections::BTreeSet::new();
    plan_demand_at_scale_for_windows(refine, inputs, level, cell_meters, |plan| {
        if plan.is_empty() {
            return Ok(());
        }
        demanded_cells += plan.demand.demanded_count();
        criterion_ids.extend(
            plan.contributions
                .iter()
                .filter(|contribution| contribution.demanded_cells > 0)
                .map(|contribution| contribution.criterion.clone()),
        );
        circles.extend(reduce_demand_to_circles_on_blocks(
            &plan.demand,
            level,
            radius_meters,
            block_radius_meters,
        )?);
        Ok(())
    })?;
    Ok(LevelCircles {
        demanded: demanded_cells > 0,
        demanded_cells,
        radius_meters,
        circles,
        criterion_ids: criterion_ids.into_iter().collect(),
    })
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

/// Name of the file a run leaves beside its gridfile describing what the
/// point+radius route asked for.
///
/// The quality step runs separately and cannot see the run's
/// [`AdaptiveNestReport`]. The artifact lives beside the selected gridfile;
/// the Project namelist can be in a different directory.
pub const ADAPTIVE_REFINEMENT_FILE: &str = "adaptive_refinement.json";

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
             \"stopped_on_empty_demand\":{},\"passes\":[{passes}]}}",
            self.deepest_level, self.stopped_on_empty_demand
        )
    }
}
