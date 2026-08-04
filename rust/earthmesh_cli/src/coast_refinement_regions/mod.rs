//! Derive Method-C refinement circles directly from a land-type raster.
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
//! computed up front from the parent cell size. This reduces the same land/sea
//! criterion the carve uses into such circles.

use std::path::Path;

use earthmesh_mesh::{AreaJudgeSourceBounds, LonLatDegrees, MethodCRefinementRegion};

use crate::mkgrd_data_preprocess_source::read_landtype_bbox_window_one_based;

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

/// Reduce the land/sea boundary inside the request to a chain of circles.
///
/// The raster is blocked at half the circle radius so consecutive circles
/// overlap and the coastline is covered without gaps — the failure mode that
/// fragments h-field demand. A block is coastal when it holds both land and
/// ocean under the engine's own rule (`landtype != 0` is land, matching
/// `classify_area_judge_landtype_one_based`), so this and the carve agree on
/// where the coast is.
pub fn coastal_refinement_circles(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    request: CoastRefinementRequest,
) -> std::io::Result<Vec<MethodCRefinementRegion>> {
    if gridnum_perdegree == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive",
        ));
    }
    if !request.radius_meters.is_finite() || request.radius_meters <= 0.0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "coastal refinement radius must be positive and finite",
        ));
    }
    if request.east_degrees <= request.west_degrees
        || request.north_degrees <= request.south_degrees
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "coastal refinement bounds must be non-empty",
        ));
    }

    let per_degree = gridnum_perdegree as f64;
    // Source indices are 1-based with index 1 at -180 / +90 (north-to-south).
    let lon_index = |lon: f64| ((lon + 180.0) * per_degree).floor().max(0.0) as usize + 1;
    let lat_index = |lat: f64| ((90.0 - lat) * per_degree).floor().max(0.0) as usize + 1;
    let bounds = AreaJudgeSourceBounds {
        minlon_source: lon_index(request.west_degrees),
        maxlon_source: lon_index(request.east_degrees),
        maxlat_source: lat_index(request.north_degrees),
        minlat_source: lat_index(request.south_degrees),
    };
    let window = read_landtype_bbox_window_one_based(landtype_file, gridnum_perdegree, bounds)?;

    // Half-radius blocks: consecutive circles then overlap by half, which is
    // what keeps the chain continuous along a curving coast.
    let meters_per_degree = std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 180.0;
    let block_degrees = (request.radius_meters / meters_per_degree) / 2.0;
    let block_cells = ((block_degrees * per_degree).round() as usize).max(1);

    let mut regions = Vec::new();
    let mut lat_block = window.bounds.maxlat_source;
    while lat_block <= window.bounds.minlat_source {
        let lat_end = (lat_block + block_cells - 1).min(window.bounds.minlat_source);
        let mut lon_block = window.bounds.minlon_source;
        while lon_block <= window.bounds.maxlon_source {
            let lon_end = (lon_block + block_cells - 1).min(window.bounds.maxlon_source);
            let mut saw_land = false;
            let mut saw_ocean = false;
            for lat in lat_block..=lat_end {
                for lon in lon_block..=lon_end {
                    match window.value_at_global(lon, lat) {
                        Some(0) => saw_ocean = true,
                        Some(_) => saw_land = true,
                        None => {}
                    }
                    if saw_land && saw_ocean {
                        break;
                    }
                }
                if saw_land && saw_ocean {
                    break;
                }
            }
            if saw_land && saw_ocean {
                let lon_centre = (lon_block + lon_end) as f64 / 2.0;
                let lat_centre = (lat_block + lat_end) as f64 / 2.0;
                regions.push(MethodCRefinementRegion::Circle {
                    center: LonLatDegrees::new(
                        (lon_centre - 1.0) / per_degree - 180.0,
                        90.0 - (lat_centre - 1.0) / per_degree,
                    ),
                    radius_meters: request.radius_meters,
                    level: request.level,
                });
            }
            lon_block = lon_end + 1;
        }
        lat_block = lat_end + 1;
    }
    Ok(regions)
}
