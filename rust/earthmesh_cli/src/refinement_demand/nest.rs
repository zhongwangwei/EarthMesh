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
//! What this does **not** do is read the criterion off the refined mesh's own
//! cells. That needs raster-to-cell statistics the port does not have (see the
//! technical guide on `getref_mean_std`), so the scale is carried as a length
//! and the criterion is asked over a matching neighbourhood of the source
//! raster. The size is right; the placement is grid-aligned rather than
//! cell-aligned.

use std::io;

use earthmesh_mesh::MethodCDelaunayMesh;

use super::ladder::nested_circle_radii_meters;
use super::plan::{plan_demand_at_scale, DemandPlanInputs, LevelDemand};
use super::reduce_demand_to_circles_on_blocks;
use earthmesh_core::RefineConfig;

/// What one pass did, so a run can say why it stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct NestPassReport {
    pub level: usize,
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
}

/// Refine `mesh` up to `max_level`, re-planning demand before every pass.
pub fn spawn_nest_adaptive(
    mesh: &MethodCDelaunayMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<(MethodCDelaunayMesh, AdaptiveNestReport)> {
    if !base_cell_meters.is_finite() || base_cell_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "base cell size must be positive and finite",
        ));
    }
    let radii = nested_circle_radii_meters(base_cell_meters, max_level)?;
    let mut current = mesh.clone();
    let mut passes = Vec::new();
    let mut deepest_level = 0usize;
    let mut stopped_on_empty_demand = false;

    for level in 1..=max_level {
        // The cell this pass refines away is the one the previous level left.
        let cell_meters = base_cell_meters / 2f64.powi((level - 1) as i32);
        let plan: LevelDemand = plan_demand_at_scale(refine, inputs, level, cell_meters)?;
        if plan.is_empty() {
            stopped_on_empty_demand = true;
            break;
        }
        // Every level blocks on the finest radius so the centres coincide and
        // the levels come out concentric; only the radius changes per level.
        let regions = reduce_demand_to_circles_on_blocks(
            &plan.demand,
            level,
            radii[level - 1],
            radii[max_level - 1],
        )?;
        if regions.is_empty() {
            stopped_on_empty_demand = true;
            break;
        }
        let faces_before = face_count(&current);
        current = current.spawn_nest(&regions, level).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "adaptive refinement level {level} with {} circles of radius {:.0} m: {error}",
                    regions.len(),
                    radii[level - 1]
                ),
            )
        })?;
        passes.push(NestPassReport {
            level,
            cell_meters,
            circle_count: regions.len(),
            demanded_cells: plan.demand.demanded_count(),
            faces_before,
            faces_after: face_count(&current),
        });
        deepest_level = level;
    }

    Ok((
        current,
        AdaptiveNestReport {
            passes,
            deepest_level,
            stopped_on_empty_demand,
        },
    ))
}

fn face_count(mesh: &MethodCDelaunayMesh) -> usize {
    mesh.w_faces.len().saturating_sub(2)
}
