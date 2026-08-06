//! Coastal refinement circles, composed from the generic demand layer.
//!
//! The h-field path expresses coastal demand as a gradient-limited raster, and
//! its usable resolution sits in a window that fails at both ends: too coarse
//! aliases the level map into a selection Method-C rejects, too fine resolves
//! demand narrower than one rad3 footprint, which can only be refined where a
//! footprint happens to fit. The deciding variable for that window is still
//! unidentified (see `docs/mesh_construction_technical_guide.md` section 8).
//!
//! Circles do not have that problem. A circle wider than a footprint is
//! materializable by construction, so the only constraint is one that can be
//! computed up front from the parent cell size.
//!
//! Nothing here is specific to circles or to blocks any more: this is the
//! coastline predicate ([`crate::refinement_demand::landtype::coastal_demand`]) fed to
//! the shared reduction ([`crate::refinement_demand::reduce_demand_to_circles`]). Sea
//! surface temperature, slope and bathymetry compose the same way with their
//! own predicate.

use std::path::Path;

use earthmesh_mesh::RefinementRegion;

use crate::refinement_demand::{
    landtype::coastal_demand, reduce_demand_to_circles, source_bounds_for_bbox,
};

pub use crate::refinement_demand::materializable_radius_meters;

/// Where coastal refinement is wanted, and how finely to chase the coastline.
#[derive(Clone, Copy, Debug)]
pub struct CoastRefinementRequest {
    pub west_degrees: f64,
    pub east_degrees: f64,
    pub south_degrees: f64,
    pub north_degrees: f64,
    /// Refinement level for the emitted circles.
    pub level: usize,
    /// Circle radius. Must clear one rad3 footprint at the parent generation or
    /// Method-C cannot seed inside the circle; see
    /// [`materializable_radius_meters`].
    pub radius_meters: f64,
}

/// Reduce the land/sea boundary inside the request to a chain of circles.
pub fn coastal_refinement_circles(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    request: CoastRefinementRequest,
) -> std::io::Result<Vec<RefinementRegion>> {
    if !request.radius_meters.is_finite() || request.radius_meters <= 0.0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "coastal refinement radius must be positive and finite",
        ));
    }
    let bounds = source_bounds_for_bbox(
        request.west_degrees,
        request.east_degrees,
        request.south_degrees,
        request.north_degrees,
        gridnum_perdegree,
    )?;
    let demand = coastal_demand(landtype_file, gridnum_perdegree, bounds)?;
    reduce_demand_to_circles(&demand, request.level, request.radius_meters)
}
