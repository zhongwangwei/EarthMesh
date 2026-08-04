//! Where refinement is wanted, and how that becomes Method-C regions.
//!
//! Every criterion asks the same question of every source-raster cell: does
//! this cell need a finer mesh? A coastline asks it of a land/sea class map,
//! sea surface temperature of an SST field, land-cover of a category count,
//! and bathymetry will ask it of a depth field. The answer is always a boolean
//! raster, and reducing such a raster to circles does not depend on which
//! criterion produced it.
//!
//! So the two halves live apart. Producers ([`landtype`], [`threshold`]) turn a
//! data source into a [`RefinementDemand`]; [`reduce_demand_to_circles`] turns
//! any demand into circles. Adding a criterion means adding a producer, not
//! touching the reduction.
//!
//! The h-field is the other consumer of the same input: it takes the union of
//! criteria and gradient-limits it into a continuous `h(x)`, where this takes
//! the union and covers it with circles. Both start from demand, which is why
//! they can be compared on the same criterion.

pub mod landtype;
pub mod threshold;

use std::io;

use earthmesh_mesh::{AreaJudgeSourceBounds, LonLatDegrees, MethodCRefinementRegion};

/// Refinement demand over a window of the source raster.
///
/// Indices are the engine's global one-based source indices: index 1 sits at
/// -180 longitude and +90 latitude, and latitude runs north to south.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefinementDemand {
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    nlons: usize,
    nlats: usize,
    demanded: Vec<bool>,
}

impl RefinementDemand {
    /// An empty demand over `bounds`, with nothing yet marked.
    pub fn new(bounds: AreaJudgeSourceBounds, gridnum_perdegree: usize) -> io::Result<Self> {
        if gridnum_perdegree == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "gridnum_perdegree must be positive",
            ));
        }
        if bounds.maxlon_source < bounds.minlon_source
            || bounds.minlat_source < bounds.maxlat_source
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refinement demand bounds must be non-empty",
            ));
        }
        let nlons = bounds.maxlon_source - bounds.minlon_source + 1;
        let nlats = bounds.minlat_source - bounds.maxlat_source + 1;
        let len = nlons.checked_mul(nlats).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "refinement demand window overflows",
            )
        })?;
        Ok(Self {
            bounds,
            gridnum_perdegree,
            nlons,
            nlats,
            demanded: vec![false; len],
        })
    }

    pub fn bounds(&self) -> AreaJudgeSourceBounds {
        self.bounds
    }

    pub fn gridnum_perdegree(&self) -> usize {
        self.gridnum_perdegree
    }

    fn offset(&self, lon_index: usize, lat_index: usize) -> Option<usize> {
        if lon_index < self.bounds.minlon_source
            || lon_index > self.bounds.maxlon_source
            || lat_index < self.bounds.maxlat_source
            || lat_index > self.bounds.minlat_source
        {
            return None;
        }
        let lon_offset = lon_index - self.bounds.minlon_source;
        let lat_offset = lat_index - self.bounds.maxlat_source;
        Some(lat_offset * self.nlons + lon_offset)
    }

    /// Mark or clear one source cell. Indices outside the window are ignored,
    /// which lets a producer walk a halo without bounds arithmetic.
    pub fn set(&mut self, lon_index: usize, lat_index: usize, demanded: bool) {
        if let Some(offset) = self.offset(lon_index, lat_index) {
            self.demanded[offset] = demanded;
        }
    }

    pub fn is_demanded(&self, lon_index: usize, lat_index: usize) -> bool {
        self.offset(lon_index, lat_index)
            .is_some_and(|offset| self.demanded[offset])
    }

    /// How many source cells the window holds, demanded or not.
    pub fn bounds_cell_count(&self) -> usize {
        self.demanded.len()
    }

    pub fn demanded_count(&self) -> usize {
        self.demanded.iter().filter(|value| **value).count()
    }

    pub fn is_empty(&self) -> bool {
        self.demanded.iter().all(|value| !value)
    }

    /// Union with another demand over the same window, so several criteria can
    /// drive one reduction. Both the window and the sampling must agree.
    pub fn union_with(&mut self, other: &Self) -> io::Result<()> {
        if self.bounds != other.bounds || self.gridnum_perdegree != other.gridnum_perdegree {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refinement demands must share a window and sampling to be unioned",
            ));
        }
        for (target, source) in self.demanded.iter_mut().zip(&other.demanded) {
            *target |= *source;
        }
        Ok(())
    }

    fn lon_degrees(&self, lon_index_centre: f64) -> f64 {
        (lon_index_centre - 1.0) / self.gridnum_perdegree as f64 - 180.0
    }

    fn lat_degrees(&self, lat_index_centre: f64) -> f64 {
        90.0 - (lat_index_centre - 1.0) / self.gridnum_perdegree as f64
    }
}

/// Smallest circle radius that can host a rad3 footprint on a mesh whose base
/// cells are `base_cell_meters` across.
///
/// rad3 marks three rings around a seed, so the circle has to admit a seed with
/// room to spread. Measured against `spawn_nest` on real coastal demand: at
/// NXP 21 (base cells ~381 km) a 150 km radius refines, which is 0.4 base cells.
/// This keeps that ratio rather than deriving it from the ring count, because
/// the selection marks faces by centre containment and then grows the footprint
/// outward — the circle does not have to contain the whole footprint itself.
pub fn materializable_radius_meters(base_cell_meters: f64) -> f64 {
    0.4 * base_cell_meters
}

/// Cover demand with a chain of circles at `level`.
///
/// The window is walked in blocks half a radius across, and a block holding any
/// demanded cell gets one circle on its centre. Half-radius blocking makes
/// consecutive circles overlap by half, which is what keeps a chain continuous
/// along a curving feature — the failure mode that fragments h-field demand.
///
/// `radius_meters` must clear [`materializable_radius_meters`] for the parent
/// generation or Method-C cannot seed inside the circle; that is the caller's
/// choice because only the caller knows the base cell size.
pub fn reduce_demand_to_circles(
    demand: &RefinementDemand,
    level: usize,
    radius_meters: f64,
) -> io::Result<Vec<MethodCRefinementRegion>> {
    if !radius_meters.is_finite() || radius_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement circle radius must be positive and finite",
        ));
    }
    let per_degree = demand.gridnum_perdegree as f64;
    let meters_per_degree = std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 180.0;
    let block_degrees = (radius_meters / meters_per_degree) / 2.0;
    let block_cells = ((block_degrees * per_degree).round() as usize).max(1);

    let mut regions = Vec::new();
    let mut lat_block = demand.bounds.maxlat_source;
    while lat_block <= demand.bounds.minlat_source {
        let lat_end = (lat_block + block_cells - 1).min(demand.bounds.minlat_source);
        let mut lon_block = demand.bounds.minlon_source;
        while lon_block <= demand.bounds.maxlon_source {
            let lon_end = (lon_block + block_cells - 1).min(demand.bounds.maxlon_source);
            let mut wanted = false;
            'block: for lat in lat_block..=lat_end {
                for lon in lon_block..=lon_end {
                    if demand.is_demanded(lon, lat) {
                        wanted = true;
                        break 'block;
                    }
                }
            }
            if wanted {
                let lon_centre = (lon_block + lon_end) as f64 / 2.0;
                let lat_centre = (lat_block + lat_end) as f64 / 2.0;
                regions.push(MethodCRefinementRegion::Circle {
                    center: LonLatDegrees::new(
                        demand.lon_degrees(lon_centre),
                        demand.lat_degrees(lat_centre),
                    ),
                    radius_meters,
                    level,
                });
            }
            lon_block = lon_end + 1;
        }
        lat_block = lat_end + 1;
    }
    Ok(regions)
}

/// Global one-based source indices for a geographic window.
pub fn source_bounds_for_bbox(
    west_degrees: f64,
    east_degrees: f64,
    south_degrees: f64,
    north_degrees: f64,
    gridnum_perdegree: usize,
) -> io::Result<AreaJudgeSourceBounds> {
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive",
        ));
    }
    if !(west_degrees.is_finite()
        && east_degrees.is_finite()
        && south_degrees.is_finite()
        && north_degrees.is_finite())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement demand bounds must be finite",
        ));
    }
    if east_degrees <= west_degrees || north_degrees <= south_degrees {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement demand bounds must be non-empty",
        ));
    }
    let per_degree = gridnum_perdegree as f64;
    // Clamped to the source dimensions: the floor-plus-one mapping puts exactly
    // 180 east and exactly -90 one cell past the last one, and a window ending
    // there would otherwise carry a column the raster cannot answer for.
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    let nlats_source = gridnum_perdegree.saturating_mul(180);
    let lon_index = |lon: f64| {
        (((lon + 180.0) * per_degree).floor().max(0.0) as usize + 1).clamp(1, nlons_source)
    };
    let lat_index = |lat: f64| {
        (((90.0 - lat) * per_degree).floor().max(0.0) as usize + 1).clamp(1, nlats_source)
    };
    Ok(AreaJudgeSourceBounds {
        minlon_source: lon_index(west_degrees),
        maxlon_source: lon_index(east_degrees),
        maxlat_source: lat_index(north_degrees),
        minlat_source: lat_index(south_degrees),
    })
}

#[cfg(test)]
mod tests;
