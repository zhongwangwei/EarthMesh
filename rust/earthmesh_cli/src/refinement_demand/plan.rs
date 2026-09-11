//! Per-level shared statistical demand, projected to canonical source indices.
//! Supports and predicates are identical to the HField adapter. Only projection
//! and the separately requested geometric coastline producer belong here.

use std::io;
use std::path::Path;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::AreaJudgeSourceBounds;

use super::landtype::coastal_demand;
use super::threshold_support::{evaluate_threshold_support, threshold_level_cap};
use super::RefinementDemand;
use crate::GridRegion;

/// What a demand plan needs to know that the namelist does not say.
#[derive(Clone, Debug)]
pub struct DemandPlanInputs<'a> {
    /// Window to evaluate over, in global one-based source indices.
    pub bounds: AreaJudgeSourceBounds,
    /// Source raster sampling, shared by every criterion.
    pub gridnum_perdegree: usize,
    /// Optional landtype mask / coastline source. Required for categorical criteria.
    pub landtype_file: Option<&'a Path>,
    /// Mesh type, as the engine spells it, deciding which criteria apply.
    pub mesh_type: &'a str,
    /// Refine coastlines. Not a namelist flag: the engine expresses coastal
    /// demand through `th_sea_ratio`, and the caller decides whether the circle
    /// route should chase the boundary directly.
    pub refine_coastline: bool,
    /// Select statistical support centers; selected footprints may cross the edge.
    /// Geometric coastline demand retains its source-cell domain filter.
    pub domain_region: Option<&'a GridRegion>,
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
    /// Shared raw support evidence before source-window projection.
    pub raw_support: Vec<serde_json::Value>,
}

impl LevelDemand {
    pub fn is_empty(&self) -> bool {
        self.demand.is_empty()
    }
}

/// Evaluate every enabled criterion at `cell_meters` and union the results.
///
/// `cell_meters` is the nominal parent support size, not an actual mesh cell.
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
    let mut raw_support = Vec::new();
    if !(1..=5).contains(&level) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "demand level must be in 1..=5",
        ));
    }
    let cap = threshold_level_cap(refine, inputs.mesh_type, 5)?;
    if level <= cap {
        let raw = evaluate_threshold_support(
            refine,
            inputs.mesh_type,
            inputs.landtype_file,
            cell_meters,
            inputs.domain_region,
        )?;
        for criterion in &raw.criteria {
            let contribution =
                raw.project_source(&criterion.hits, inputs.bounds, inputs.gridnum_perdegree)?;
            contributions.push(DemandContribution {
                criterion: criterion.id.clone(),
                demanded_cells: contribution.demanded_count(),
            });
            demand.union_with(&contribution)?;
            raw_support.push(raw.criterion_report(criterion));
        }
    }
    if inputs.refine_coastline {
        if let Some(path) = inputs.landtype_file {
            let mut contribution = coastal_demand(path, inputs.gridnum_perdegree, inputs.bounds)?;
            filter_to_domain(&mut contribution, inputs.domain_region);
            contributions.push(DemandContribution {
                criterion: "coastline".into(),
                demanded_cells: contribution.demanded_count(),
            });
            demand.union_with(&contribution)?;
        }
    }

    Ok(LevelDemand {
        level,
        demand,
        contributions,
        raw_support,
    })
}

fn filter_to_domain(demand: &mut RefinementDemand, domain: Option<&GridRegion>) {
    let Some(domain) = domain else {
        return;
    };
    let per_degree = demand.gridnum_perdegree() as f64;
    demand.retain_where(|lon_index, lat_index| {
        let lon = (lon_index as f64 - 1.0) / per_degree - 180.0;
        let lat = 90.0 - (lat_index as f64 - 1.0) / per_degree;
        domain.contains(lon, lat)
    });
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
            domain_region: None,
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
            domain_region: None,
        };
        let plan = plan_demand_at_scale(&refine, &inputs, 1, 100_000.0).expect("plan");
        assert!(plan.is_empty());
        assert!(plan.contributions.is_empty());
        assert_eq!(plan.level, 1);
    }

    #[test]
    fn domain_filter_keeps_wrapped_bbox_demand_from_counting_outside_cells() {
        let bounds = super::super::source_bounds_for_bbox(-180.0, 180.0, -1.0, 1.0, 1).unwrap();
        let mut demand = RefinementDemand::new(bounds, 1).unwrap();
        demand.fill_par(|_, _| true);
        let region = GridRegion::Bbox {
            west: 170.0,
            east: -170.0,
            south: -1.0,
            north: 1.0,
        };
        filter_to_domain(&mut demand, Some(&region));

        assert!(demand.is_demanded(351, 90), "170E side remains demanded");
        assert!(demand.is_demanded(1, 90), "180W side remains demanded");
        assert!(
            !demand.is_demanded(181, 90),
            "0E is outside the wrapped bbox"
        );
        assert!(
            demand.demanded_count() < demand.bounds_cell_count() / 4,
            "outside-domain cells must not consume demand budget"
        );
    }

    /// The domain filter keeps only what the region actually contains.
    ///
    /// A wrapped domain is now split into windows rather than widened to the
    /// whole band, so the demand it produces is already close -- but a window is
    /// a rectangle and a circle or a closed curve is not, so what the window
    /// admits is still a superset. This is what takes the rest back out, and
    /// `GridRegion::contains` is what reads the seam correctly while doing it.
    ///
    /// Nothing covered it: the filter and the field arrived with seven tests
    /// beside them, none touching either.
    #[test]
    fn the_domain_filter_keeps_only_what_a_wrapped_region_contains() {
        let wrapped = GridRegion::Bbox {
            west: 170.0,
            east: -170.0,
            south: -10.0,
            north: 10.0,
        };

        assert!(wrapped.contains(175.0, 0.0), "west of the seam is inside");
        assert!(wrapped.contains(-175.0, 0.0), "east of the seam is inside");
        assert!(
            !wrapped.contains(0.0, 0.0),
            "the far side of the globe is not"
        );
        assert!(
            !wrapped.contains(175.0, 40.0),
            "and neither is a point outside in latitude"
        );

        // A circle is where the window stays a superset however it is split:
        // its corners are in the rectangle and not in the circle.
        let circle = GridRegion::Circle {
            lon: 0.0,
            lat: 0.0,
            radius_km: 100.0,
        };
        assert!(circle.contains(0.0, 0.0), "the centre");
        assert!(
            !circle.contains(0.9, 0.9),
            "a corner of its box is outside it, which is why the filter runs"
        );
    }
}
