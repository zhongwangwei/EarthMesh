//! Every enabled criterion, evaluated at one scale, unioned into one demand.
//!
//! This is where "refinement is not only coastlines" becomes code. The engine
//! already enumerates which numeric criteria a run has turned on
//! (`enabled_mean_threshold_field_specs` walks land, ocean and atmosphere), and
//! the land-type criteria come off the same raster the carve uses. Each one
//! produces a [`RefinementDemand`]; the union is what gets reduced to circles.
//!
//! The scale matters. A criterion that compares a value against a threshold
//! (`sst > 28`, `slope > 15`) gives the same answer whatever the cell size, so
//! evaluating it once is enough. A criterion about what a cell *contains*
//! (land-cover heterogeneity) does not: how many classes crowd into a cell
//! depends on how big the cell is, so it is asked again at every level with
//! that level's cell size. `cell_meters` is what carries that through, and it
//! is why this takes a scale rather than assuming one.

use std::io;
use std::path::Path;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::AreaJudgeSourceBounds;

use super::landtype::{coastal_demand, landcover_heterogeneity_demand};
use super::threshold::{threshold_demand, ThresholdSide};
use super::RefinementDemand;
use crate::area_judge_threshold_inputs::enabled_mean_threshold_field_specs;

/// What a demand plan needs to know that the namelist does not say.
#[derive(Clone, Debug)]
pub struct DemandPlanInputs<'a> {
    /// Window to evaluate over, in global one-based source indices.
    pub bounds: AreaJudgeSourceBounds,
    /// Source raster sampling, shared by every criterion.
    pub gridnum_perdegree: usize,
    /// Land-type raster; also the coastline source. `None` skips both
    /// land-type criteria rather than failing, so an ocean-only run with no
    /// land-type layer still plans.
    pub landtype_file: Option<&'a Path>,
    /// Mesh type, as the engine spells it, deciding which criteria apply.
    pub mesh_type: &'a str,
    /// Refine coastlines. Not a namelist flag: the engine expresses coastal
    /// demand through `th_sea_ratio`, and the caller decides whether the circle
    /// route should chase the boundary directly.
    pub refine_coastline: bool,
}

/// One criterion's contribution, kept separate so a caller can say which
/// criterion asked for what.
#[derive(Clone, Debug, PartialEq)]
pub struct DemandContribution {
    pub criterion: String,
    pub demanded_cells: usize,
}

/// The demand for one level, and who asked for it.
#[derive(Clone, Debug)]
pub struct LevelDemand {
    pub level: usize,
    pub demand: RefinementDemand,
    pub contributions: Vec<DemandContribution>,
}

impl LevelDemand {
    pub fn is_empty(&self) -> bool {
        self.demand.is_empty()
    }
}

/// Evaluate every enabled criterion at `cell_meters` and union the results.
///
/// `cell_meters` is the size of the cell being judged — the generation this
/// level would refine away. Criteria that do not depend on it ignore it.
pub fn plan_demand_at_scale(
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    level: usize,
    cell_meters: f64,
) -> io::Result<LevelDemand> {
    if !cell_meters.is_finite() || cell_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cell size for a demand plan must be positive and finite",
        ));
    }
    let mut demand = RefinementDemand::new(inputs.bounds, inputs.gridnum_perdegree)?;
    let mut contributions = Vec::new();
    let add = |demand: &mut RefinementDemand,
               criterion: &str,
               contribution: RefinementDemand,
               contributions: &mut Vec<DemandContribution>|
     -> io::Result<()> {
        let demanded_cells = contribution.demanded_count();
        demand.union_with(&contribution)?;
        contributions.push(DemandContribution {
            criterion: criterion.to_string(),
            demanded_cells,
        });
        Ok(())
    };

    let threshold_dir = Path::new(refine.threshold_dir.trim());
    for spec in enabled_mean_threshold_field_specs(refine, inputs.mesh_type) {
        // The engine's own comparison is `value > threshold` everywhere; a
        // below-threshold criterion (shallow water first, when bathymetry
        // lands) would come in as its own spec rather than by flipping this.
        let file = threshold_dir.join(format!("{}.nc", spec.file_stem));
        let contribution = threshold_demand(
            &file,
            &spec.var_name,
            inputs.gridnum_perdegree,
            inputs.bounds,
            ThresholdSide::Above,
            spec.threshold,
        )?;
        add(
            &mut demand,
            &spec.var_name,
            contribution,
            &mut contributions,
        )?;
    }

    if let Some(landtype_file) = inputs.landtype_file {
        if inputs.refine_coastline {
            let contribution =
                coastal_demand(landtype_file, inputs.gridnum_perdegree, inputs.bounds)?;
            add(&mut demand, "coastline", contribution, &mut contributions)?;
        }
        if refine.refine_num_landtypes {
            // Resolution-dependent: the neighbourhood is the cell this level
            // would refine away, so the same raster gives different answers at
            // different levels. That is the point.
            let radius_cells = source_cells_for_meters(inputs.gridnum_perdegree, cell_meters / 2.0);
            let contribution = landcover_heterogeneity_demand(
                landtype_file,
                inputs.gridnum_perdegree,
                inputs.bounds,
                radius_cells,
                refine.th_num_landtypes.max(0) as usize,
            )?;
            add(&mut demand, "landcover", contribution, &mut contributions)?;
        }
    }

    Ok(LevelDemand {
        level,
        demand,
        contributions,
    })
}

/// How many source cells span `meters`, at least one.
fn source_cells_for_meters(gridnum_perdegree: usize, meters: f64) -> usize {
    let meters_per_degree = std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 180.0;
    let cell_meters = meters_per_degree / gridnum_perdegree.max(1) as f64;
    if !cell_meters.is_finite() || cell_meters <= 0.0 {
        return 1;
    }
    ((meters / cell_meters).round() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_must_be_a_real_length() {
        let refine = RefineConfig::default();
        let inputs = DemandPlanInputs {
            bounds: super::super::source_bounds_for_bbox(100.0, 110.0, 10.0, 20.0, 1).unwrap(),
            gridnum_perdegree: 1,
            landtype_file: None,
            mesh_type: "earthmesh",
            refine_coastline: false,
        };
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(plan_demand_at_scale(&refine, &inputs, 1, bad).is_err());
        }
    }

    #[test]
    fn a_run_with_nothing_enabled_plans_an_empty_demand() {
        let refine = RefineConfig::default();
        let inputs = DemandPlanInputs {
            bounds: super::super::source_bounds_for_bbox(100.0, 110.0, 10.0, 20.0, 1).unwrap(),
            gridnum_perdegree: 1,
            landtype_file: None,
            mesh_type: "earthmesh",
            refine_coastline: false,
        };
        let plan = plan_demand_at_scale(&refine, &inputs, 1, 100_000.0).expect("plan");
        assert!(plan.is_empty());
        assert!(plan.contributions.is_empty());
        assert_eq!(plan.level, 1);
    }

    #[test]
    fn the_landcover_neighbourhood_follows_the_scale_it_is_given() {
        // Half a cell either side of a point is the cell, so the radius has to
        // track the level -- this is the arithmetic that makes the criterion
        // resolution-dependent rather than fixed to the raster.
        let per_degree = 4usize;
        let cell_deg = 1.0 / per_degree as f64;
        let meters_per_degree = std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 180.0;
        let one_cell_m = cell_deg * meters_per_degree;
        assert_eq!(source_cells_for_meters(per_degree, one_cell_m), 1);
        assert_eq!(source_cells_for_meters(per_degree, 8.0 * one_cell_m), 8);
        // Never zero: a level finer than the raster still asks about one cell.
        assert_eq!(source_cells_for_meters(per_degree, one_cell_m / 100.0), 1);
    }
}
