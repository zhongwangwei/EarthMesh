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

use super::landtype::{
    coastal_demand, dominant_class_demand, landcover_heterogeneity_demand, sea_ratio_demand,
};
use super::threshold::{threshold_demand, threshold_stddev_demand, ThresholdSide};
use super::RefinementDemand;
use crate::area_judge_threshold_inputs::{
    enabled_mean_threshold_field_specs, enabled_std_threshold_field_specs,
};
use crate::GridRegion;

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
    /// Optional project domain. Source windows are just cheap read bounds; this
    /// is the semantic gate that prevents cells outside a regional domain from
    /// consuming demand budget or becoming refinement circles.
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
               mut contribution: RefinementDemand,
               contributions: &mut Vec<DemandContribution>|
     -> io::Result<()> {
        filter_to_domain(&mut contribution, inputs.domain_region);
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

    // The odd half of every mean/std flag pair: refine where the field varies,
    // not where it is large. The h-field applies both halves; reading only the
    // mean half here made the two routes disagree about what the same namelist
    // asked for, in silence.
    let stddev_radius_cells = source_cells_for_meters(inputs.gridnum_perdegree, cell_meters / 2.0)?;
    for spec in enabled_std_threshold_field_specs(refine, inputs.mesh_type) {
        let file = threshold_dir.join(format!("{}.nc", spec.file_stem));
        let contribution = threshold_stddev_demand(
            &file,
            &spec.var_name,
            inputs.gridnum_perdegree,
            inputs.bounds,
            stddev_radius_cells,
            spec.threshold,
        )?;
        add(
            &mut demand,
            &format!("{}_stddev", spec.var_name),
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
        // Resolution-dependent, all three: the neighbourhood is the cell this
        // level would refine away, so the same raster gives different answers at
        // different levels. That is the point of asking again per pass.
        //
        // The mesh-type gate mirrors the h-field's `supports_threshold_hfield`.
        // Without it the two routes would disagree about whether a land-type
        // criterion applies to a given mesh, and a project switching backends
        // would silently get a different mesh for no stated reason.
        let radius_cells = source_cells_for_meters(inputs.gridnum_perdegree, cell_meters / 2.0)?;
        if supports_landtype_criteria(inputs.mesh_type) {
            if refine.refine_num_landtypes {
                let contribution = landcover_heterogeneity_demand(
                    landtype_file,
                    inputs.gridnum_perdegree,
                    inputs.bounds,
                    radius_cells,
                    refine.th_num_landtypes.max(0) as usize,
                )?;
                add(&mut demand, "landcover", contribution, &mut contributions)?;
            }
            if refine.refine_area_mainland {
                let contribution = dominant_class_demand(
                    landtype_file,
                    inputs.gridnum_perdegree,
                    inputs.bounds,
                    radius_cells,
                    refine.th_area_mainland,
                )?;
                add(
                    &mut demand,
                    "area_mainland",
                    contribution,
                    &mut contributions,
                )?;
            }
            if refine.refine_sea_ratio {
                let contribution = sea_ratio_demand(
                    landtype_file,
                    inputs.gridnum_perdegree,
                    inputs.bounds,
                    radius_cells,
                    refine.th_sea_ratio[0],
                    refine.th_sea_ratio[1],
                )?;
                add(&mut demand, "sea_ratio", contribution, &mut contributions)?;
            }
        }
    }

    Ok(LevelDemand {
        level,
        demand,
        contributions,
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

/// Mesh types the land-type criteria apply to.
///
/// Kept identical to the h-field's `supports_threshold_hfield`; the two routes
/// answering differently would be a difference nobody asked for.
fn supports_landtype_criteria(mesh_type: &str) -> bool {
    matches!(
        mesh_type.trim(),
        "landmesh" | "oceanmesh" | "atmos" | "atmosmesh" | "LOCmesh" | "earthmesh"
    )
}

/// How many source cells span `meters`, at least one.
fn source_cells_for_meters(gridnum_perdegree: usize, meters: f64) -> io::Result<usize> {
    if gridnum_perdegree == 0 || !meters.is_finite() || meters < 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source-cell radius must be finite and non-negative, with positive grid sampling",
        ));
    }
    let nlons_source = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 360 overflows usize",
        )
    })?;
    let max_unique_radius = nlons_source.min(isize::MAX as usize) / 2;
    let meters_per_degree = std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 180.0;
    let cell_meters = meters_per_degree / gridnum_perdegree as f64;
    let cells = (meters / cell_meters).round();
    if !cells.is_finite() || cells > max_unique_radius as f64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "source-cell radius {cells:.0} exceeds the unique periodic longitude neighbourhood limit {max_unique_radius}"
            ),
        ));
    }
    Ok((cells as usize).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every criterion the h-field consumes must reach this planner too.
    ///
    /// The two routes read the same `RefineConfig`. When one honours a flag and
    /// the other ignores it, a project switching backends silently gets a
    /// different mesh and nothing says why -- which is what happened: this
    /// planner handled `refine_num_landtypes` and quietly dropped
    /// `refine_area_mainland` and `refine_sea_ratio`.
    #[test]
    fn every_criterion_the_h_field_reads_is_read_here_too() {
        let refine = RefineConfig {
            refine_num_landtypes: true,
            refine_area_mainland: true,
            refine_sea_ratio: true,
            ..RefineConfig::default()
        };
        let planned = criteria_this_planner_consumes(&refine, "earthmesh");
        for flag in ["landcover", "area_mainland", "sea_ratio"] {
            assert!(
                planned.contains(&flag),
                "{flag} is dropped, planned {planned:?}"
            );
        }

        // And the mesh-type gate has to agree with the h-field's, or the same
        // project gets different meshes from the two routes.
        assert!(supports_landtype_criteria("landmesh"));
        assert!(supports_landtype_criteria("oceanmesh"));
        assert!(supports_landtype_criteria("atmosmesh"));
        assert!(!supports_landtype_criteria("something_else"));
    }

    /// Which land-type criteria [`plan_demand_at_scale`] would act on.
    ///
    /// Mirrors the branch structure rather than running it, so the check needs
    /// no raster; running it is what the end-to-end tests do.
    fn criteria_this_planner_consumes(refine: &RefineConfig, mesh_type: &str) -> Vec<&'static str> {
        let mut planned = Vec::new();
        if supports_landtype_criteria(mesh_type) {
            if refine.refine_num_landtypes {
                planned.push("landcover");
            }
            if refine.refine_area_mainland {
                planned.push("area_mainland");
            }
            if refine.refine_sea_ratio {
                planned.push("sea_ratio");
            }
        }
        planned
    }

    /// The same is true of the layered mean/std threshold catalogue.
    ///
    /// `refine_onelayer_*` and `refine_twolayer_*` are mean/std pairs -- even
    /// slot compares the value, odd slot compares how much it varies -- and the
    /// h-field applies both halves. This planner read only the even half, so a
    /// project asking for refinement where a field is rough got a uniform mesh
    /// and no message. Checked against the spec builders themselves rather than
    /// a hand-written list, so a criterion added to the catalogue cannot be
    /// added to one route only.
    #[test]
    fn both_halves_of_every_layered_threshold_pair_reach_this_planner() {
        let refine = RefineConfig {
            refine_onelayer_lnd: [true; 8],
            refine_onelayer_ocn: [true; 8],
            refine_onelayer_atmos: [true; 2],
            refine_twolayer_lnd: [true; 10],
            ..RefineConfig::default()
        };
        for mesh_type in ["landmesh", "oceanmesh", "atmosmesh", "earthmesh"] {
            let mean = enabled_mean_threshold_field_specs(&refine, mesh_type);
            let stddev = enabled_std_threshold_field_specs(&refine, mesh_type);
            assert!(!mean.is_empty(), "{mesh_type} has no mean specs to compare");
            assert!(
                !stddev.is_empty(),
                "{mesh_type} std half vanished from the catalogue"
            );
        }
    }

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

    #[test]
    fn huge_finite_cell_radius_is_rejected_before_it_can_wrap_indices() {
        let err = source_cells_for_meters(1, f64::MAX).expect_err("huge radius must fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
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
        assert_eq!(source_cells_for_meters(per_degree, one_cell_m).unwrap(), 1);
        assert_eq!(
            source_cells_for_meters(per_degree, 8.0 * one_cell_m).unwrap(),
            8
        );
        // Never zero: a level finer than the raster still asks about one cell.
        assert_eq!(
            source_cells_for_meters(per_degree, one_cell_m / 100.0).unwrap(),
            1
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
