use std::io;
use std::path::Path;

use earthmesh_mesh::RefinementRegion;

use super::shared::{method_c_calculated_region_level, require_specified_region_level};
use crate::{
    parse_bbox_mask_nml, read_bbox_mask_netcdf, source_extension, unsupported_mask_source,
};

pub fn read_method_c_bbox_refinement_regions(
    source: &Path,
    max_level: usize,
    regions: &mut Vec<RefinementRegion>,
    refine: &earthmesh_core::RefineConfig,
    nxp: usize,
    apply_parent_halos: bool,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_bbox_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_bbox_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    require_specified_region_level(source, mask.refine_degree, max_level)?;
    if mask.refine_degree == 0 {
        return Ok(());
    }
    for point in &mask.points {
        push_method_c_bbox_region(
            regions,
            point,
            mask.refine_degree,
            refine,
            nxp,
            apply_parent_halos,
        );
    }
    Ok(())
}

/// Push `point` at `level`, and with `apply_parent_halos` a grown box at
/// every level above it.
///
/// A box asked for level 2 or deeper needs every level above it underneath:
/// Method-C nests each level inside the one before, and a global MPAS-Ocean
/// box at two levels was refused ("empty level 1") where a circle or closed
/// curve got its parents grown for it.
pub(super) fn push_method_c_bbox_region(
    regions: &mut Vec<RefinementRegion>,
    point: &crate::bbox_mask_io::BBoxPoint,
    level: usize,
    refine: &earthmesh_core::RefineConfig,
    nxp: usize,
    apply_parent_halos: bool,
) {
    if apply_parent_halos && nxp > 0 {
        let base_spacing =
            std::f64::consts::PI * 2.0 * earthmesh_core::EARTH_RADIUS_METERS / (5.0 * nxp as f64);
        for parent_level in 1..level {
            let halo = super::shared::method_c_parent_halo_meters(
                refine,
                parent_level,
                level,
                base_spacing,
            );
            regions.push(grown_bbox(point, halo, parent_level));
        }
    }
    regions.push(RefinementRegion::Bbox {
        west_degrees: point.west,
        east_degrees: point.east,
        south_degrees: point.south,
        north_degrees: point.north,
        level,
    });
}

/// `point` grown by `halo_meters` on every side: latitude by arc, longitude
/// by the arc at the box's poleward edge (the widest in degrees), wrapped
/// across the antimeridian, the whole circle once it spans 360 degrees.
fn grown_bbox(
    point: &crate::bbox_mask_io::BBoxPoint,
    halo_meters: f64,
    level: usize,
) -> RefinementRegion {
    let dlat = (halo_meters / earthmesh_core::EARTH_RADIUS_METERS).to_degrees();
    let south = (point.south - dlat).max(-90.0);
    let north = (point.north + dlat).min(90.0);
    let poleward = south.abs().max(north.abs()).min(85.0);
    let dlon = dlat / poleward.to_radians().cos();
    let span = (point.east - point.west).rem_euclid(360.0);
    let (west, east) = if span + 2.0 * dlon >= 360.0 {
        (-180.0, 180.0)
    } else {
        let wrap = |lon: f64| (lon + 180.0).rem_euclid(360.0) - 180.0;
        (wrap(point.west - dlon), wrap(point.east + dlon))
    };
    RefinementRegion::Bbox {
        west_degrees: west,
        east_degrees: east,
        south_degrees: south,
        north_degrees: north,
        level,
    }
}

pub fn read_method_c_calculated_bbox_refinement_regions(
    source: &Path,
    max_level: usize,
    zero_is_threshold_domain: bool,
    regions: &mut Vec<RefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_bbox_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_bbox_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    let Some(level) =
        method_c_calculated_region_level(mask.refine_degree, max_level, zero_is_threshold_domain)
    else {
        return Ok(());
    };
    for point in &mask.points {
        regions.push(RefinementRegion::Bbox {
            west_degrees: point.west,
            east_degrees: point.east,
            south_degrees: point.south,
            north_degrees: point.north,
            level,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::grown_bbox;
    use crate::bbox_mask_io::BBoxPoint;
    use earthmesh_mesh::RefinementRegion;

    fn bounds(region: RefinementRegion) -> (f64, f64, f64, f64, usize) {
        match region {
            RefinementRegion::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                level,
            } => (
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                level,
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_parent_box_grows_on_every_side_and_wraps_the_antimeridian() {
        let degree = earthmesh_core::EARTH_RADIUS_METERS.to_radians();
        let point = |west, east, south, north| BBoxPoint {
            west,
            east,
            south,
            north,
        };
        // One degree of arc: a degree of latitude, more of longitude at 40N.
        let (w, e, s, n, level) = bounds(grown_bbox(&point(120.0, 150.0, 10.0, 40.0), degree, 1));
        assert_eq!(level, 1);
        assert!((s - 9.0).abs() < 1e-9 && (n - 41.0).abs() < 1e-9);
        assert!(w < 119.0 && e > 151.0);
        // Across the antimeridian the box keeps west > east.
        let (w, e, ..) = bounds(grown_bbox(&point(175.0, -177.0, -20.0, -15.0), degree, 1));
        assert!(
            w > 170.0 && w < 175.0 && e > -177.0 && e < -170.0,
            "{w} {e}"
        );
        // Grown past a full turn it is the whole circle.
        let (w, e, ..) = bounds(grown_bbox(&point(-179.0, 179.0, 0.0, 10.0), degree, 1));
        assert_eq!((w, e), (-180.0, 180.0));
    }
}
