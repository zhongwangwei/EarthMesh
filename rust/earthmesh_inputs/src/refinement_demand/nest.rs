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

use earthmesh_refine::nest::{CriteriaEvidence, LevelCircles};

use super::ladder::nested_circle_radii_meters;
use super::plan::{plan_demand_at_scale_for_windows, DemandPlanInputs};
use super::reduce_demand_to_circles_on_blocks;
use earthmesh_core::RefineConfig;

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
    let mut evidence = CriteriaEvidence::default();
    plan_demand_at_scale_for_windows(refine, inputs, level, cell_meters, |plan| {
        evidence = evidence.merge(plan.evidence);
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
        evidence,
    })
}

/// Name of the file a run leaves beside its gridfile describing what the
/// point+radius route asked for.
///
/// The quality step runs separately and cannot see the run's
/// [`AdaptiveNestReport`](earthmesh_refine::nest::AdaptiveNestReport). The artifact lives beside the selected gridfile;
/// the Project namelist can be in a different directory.
pub const ADAPTIVE_REFINEMENT_FILE: &str = "adaptive_refinement.json";
