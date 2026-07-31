//! Positive-area coverage of the immutable, regularized HField support raster.

use std::io;

use earthmesh_geometry::{
    clip_convex_polygon, intersection_area, polygon_area,
    polygon_triple_intersection_area_even_odd, Point, EARTH_RADIUS_KM,
};
use earthmesh_quality::QualityMeshInput;

use crate::hydro_delivery_intersections::{LocalEqualArea, SphericalCap};
use crate::GridRegion;

const PARALLEL_STEP_DEGREES: f64 = 0.1;
const ARC_EPSILON_RADIANS: f64 = 1.0e-10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SupportCoverage {
    pub active_bin_count: usize,
    pub candidate_pair_count: usize,
    pub convex_clip_pair_count: usize,
    pub generic_clip_pair_count: usize,
    pub positive_overlap_count: usize,
    pub covered_bin_count: usize,
    pub covered_bins: Vec<bool>,
    pub adequately_covered_bin_count: usize,
}

/// Persisted intended-domain support in row-major raster order. Only nonzero
/// bins are evaluated; gradient apron outside a regional output domain stays
/// false and therefore cannot become an uncovered-demand failure.
pub(crate) fn intended_domain_support_mask(
    nlon: usize,
    nlat: usize,
    hard_levels: &[u8],
    domain: Option<&GridRegion>,
) -> io::Result<Vec<bool>> {
    let expected = nlon
        .checked_mul(nlat)
        .ok_or_else(|| invalid("HField support raster dimensions overflow usize"))?;
    if hard_levels.len() != expected {
        return Err(invalid(format!(
            "HField support raster has {} levels, expected {nlon}x{nlat}={expected}",
            hard_levels.len()
        )));
    }
    let Some(domain) = domain else {
        return Ok(vec![true; expected]);
    };

    let mut mask = vec![false; expected];
    for jlat in 0..nlat {
        for ilon in 0..nlon {
            let index = jlat * nlon + ilon;
            if hard_levels[index] != 0 {
                mask[index] = grid_region_overlaps_hfield_bin(domain, nlon, nlat, ilon, jlat)?;
            }
        }
    }
    Ok(mask)
}

pub(crate) fn grid_region_overlaps_hfield_bin(
    domain: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> io::Result<bool> {
    if nlon == 0 || nlat == 0 || ilon >= nlon || jlat >= nlat {
        return Err(invalid("invalid HField bin coordinates for domain mask"));
    }
    let (west, east, south, north) = raster_bin_bounds(nlon, nlat, ilon, jlat);
    match domain {
        GridRegion::Bbox {
            west: domain_west,
            east: domain_east,
            north: domain_north,
            south: domain_south,
        } => {
            let south_overlap = domain_south.min(*domain_north).max(south);
            let north_overlap = domain_south.max(*domain_north).min(north);
            Ok(north_overlap > south_overlap + 1.0e-13
                && cyclic_intervals_have_positive_overlap(*domain_west, *domain_east, west, east))
        }
        GridRegion::Circle {
            lon,
            lat,
            radius_km,
        } => {
            if !lon.is_finite() || !lat.is_finite() || !radius_km.is_finite() || *radius_km <= 0.0 {
                return Ok(false);
            }
            let radius = radius_km / EARTH_RADIUS_KM;
            if radius >= std::f64::consts::PI {
                return Ok(true);
            }
            let distance =
                minimum_angular_distance_to_lonlat_bin(*lon, *lat, west, east, south, north);
            let tolerance = 64.0 * f64::EPSILON * radius.max(1.0);
            Ok(distance + tolerance < radius)
        }
        GridRegion::Close { points } => {
            let ring = points
                .iter()
                .map(|point| Point::new(point.lon, point.lat))
                .collect::<Vec<_>>();
            polygon_components_overlap_hfield_bin(&[vec![ring]], nlon, nlat, ilon, jlat)
        }
        GridRegion::Any(regions) => {
            for region in regions {
                if grid_region_overlaps_hfield_bin(region, nlon, nlat, ilon, jlat)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

/// Positive-area intersection of a source region, an output domain, and one
/// HField bin. Proving the two region/bin overlaps independently is
/// insufficient: two disjoint sub-bin slivers would otherwise refine the same
/// coarse bin. This routine clips both canonical supports in one local
/// equal-area plane and tests their actual triple intersection.
#[cfg(test)]
fn grid_regions_intersection_overlaps_hfield_bin(
    source: &GridRegion,
    domain: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> io::Result<bool> {
    grid_regions_intersection_overlaps_hfield_bin_impl(
        source, domain, nlon, nlat, ilon, jlat, false,
    )
}

pub(crate) fn grid_regions_intersection_overlaps_active_hfield_bin(
    source: &GridRegion,
    domain: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> io::Result<bool> {
    grid_regions_intersection_overlaps_hfield_bin_impl(source, domain, nlon, nlat, ilon, jlat, true)
}

fn grid_regions_intersection_overlaps_hfield_bin_impl(
    source: &GridRegion,
    domain: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
    domain_bin_is_active: bool,
) -> io::Result<bool> {
    if let GridRegion::Any(regions) = source {
        for region in regions {
            if grid_regions_intersection_overlaps_hfield_bin_impl(
                region,
                domain,
                nlon,
                nlat,
                ilon,
                jlat,
                domain_bin_is_active,
            )? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if let GridRegion::Any(regions) = domain {
        for region in regions {
            if grid_regions_intersection_overlaps_hfield_bin_impl(
                source, region, nlon, nlat, ilon, jlat, false,
            )? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if let (
        GridRegion::Bbox {
            west: source_west,
            east: source_east,
            south: source_south,
            north: source_north,
        },
        GridRegion::Bbox {
            west: domain_west,
            east: domain_east,
            south: domain_south,
            north: domain_north,
        },
    ) = (source, domain)
    {
        let (bin_west, bin_east, bin_south, bin_north) = raster_bin_bounds(nlon, nlat, ilon, jlat);
        let south = source_south
            .min(*source_north)
            .max(domain_south.min(*domain_north))
            .max(bin_south);
        let north = source_south
            .max(*source_north)
            .min(domain_south.max(*domain_north))
            .min(bin_north);
        if north <= south + 1.0e-13 {
            return Ok(false);
        }
        let source_intervals =
            cyclic_interval_intersections_with_bin(*source_west, *source_east, bin_west, bin_east);
        let domain_intervals =
            cyclic_interval_intersections_with_bin(*domain_west, *domain_east, bin_west, bin_east);
        return Ok(source_intervals.iter().any(|&(source_west, source_east)| {
            domain_intervals.iter().any(|&(domain_west, domain_east)| {
                source_east.min(domain_east) > source_west.max(domain_west) + 1.0e-13
            })
        }));
    }
    if let (
        GridRegion::Circle {
            lon,
            lat,
            radius_km,
        },
        GridRegion::Bbox {
            west,
            east,
            south,
            north,
        },
    )
    | (
        GridRegion::Bbox {
            west,
            east,
            south,
            north,
        },
        GridRegion::Circle {
            lon,
            lat,
            radius_km,
        },
    ) = (source, domain)
    {
        if !lon.is_finite() || !lat.is_finite() || !radius_km.is_finite() || *radius_km <= 0.0 {
            return Ok(false);
        }
        let radius = radius_km / EARTH_RADIUS_KM;
        if radius >= std::f64::consts::PI {
            return grid_region_overlaps_hfield_bin(
                &GridRegion::Bbox {
                    west: *west,
                    east: *east,
                    south: *south,
                    north: *north,
                },
                nlon,
                nlat,
                ilon,
                jlat,
            );
        }
        let (bin_west, bin_east, bin_south, bin_north) = raster_bin_bounds(nlon, nlat, ilon, jlat);
        let clipped_south = south.min(*north).max(bin_south);
        let clipped_north = south.max(*north).min(bin_north);
        if clipped_north <= clipped_south + 1.0e-13 {
            return Ok(false);
        }
        let tolerance = 64.0 * f64::EPSILON * radius.max(1.0);
        return Ok(
            cyclic_interval_intersections_with_bin(*west, *east, bin_west, bin_east)
                .into_iter()
                .any(|(clipped_west, clipped_east)| {
                    minimum_angular_distance_to_lonlat_bin(
                        *lon,
                        *lat,
                        clipped_west,
                        clipped_east,
                        clipped_south,
                        clipped_north,
                    ) + tolerance
                        < radius
                }),
        );
    }
    if !grid_region_overlaps_hfield_bin(source, nlon, nlat, ilon, jlat)?
        || (!domain_bin_is_active
            && !grid_region_overlaps_hfield_bin(domain, nlon, nlat, ilon, jlat)?)
    {
        return Ok(false);
    }

    let bin = raster_bin_ring(nlon, nlat, ilon, jlat);
    let source_within_bin = source_bbox_is_within_hfield_bin(source, nlon, nlat, ilon, jlat);
    if source_within_bin {
        if let GridRegion::Bbox {
            west,
            east,
            south,
            north,
        } = source
        {
            let west = wrap_lon(*west);
            let width = (wrap_lon(*east) - west).rem_euclid(360.0);
            let center_lon = wrap_lon(west + 0.5 * width);
            let center_lat = 0.5 * (south + north);
            if domain.contains(center_lon, center_lat) {
                return Ok(true);
            }
        }
    }
    let source_rings = region_rings_in_hfield_bin(source, nlon, nlat, ilon, jlat)?;
    let domain_rings = region_rings_in_hfield_bin(domain, nlon, nlat, ilon, jlat)?;
    for source_ring in &source_rings {
        for domain_ring in &domain_rings {
            let projection =
                LocalEqualArea::for_rings(&[bin.clone(), source_ring.clone(), domain_ring.clone()])
                    .ok_or_else(|| {
                        invalid("source/domain intersection cannot form a projection")
                    })?;
            let projected_bin = projection
                .project_ring(&bin)
                .ok_or_else(|| invalid("HField bin cannot be projected"))?;
            let projected_source = projection
                .project_ring(source_ring)
                .ok_or_else(|| invalid("source region cannot be projected"))?;
            let projected_domain = projection
                .project_ring(domain_ring)
                .ok_or_else(|| invalid("output domain cannot be projected"))?;
            let component_area = polygon_area(&projected_source);
            let numerical_floor = 1.0e-16_f64.max(1.0e-12 * component_area);
            let triple_area = if source_within_bin {
                polygon_overlap_area(
                    &projected_source,
                    PlanarBounds::for_ring(&projected_source),
                    polygon_is_convex(&projected_source),
                    &projected_domain,
                )
                .0
            } else {
                projected_triple_intersection_area(
                    &projected_source,
                    &projected_domain,
                    &projected_bin,
                )
            };
            if triple_area > numerical_floor {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn source_bbox_is_within_hfield_bin(
    source: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> bool {
    let GridRegion::Bbox {
        west,
        east,
        south,
        north,
    } = source
    else {
        return false;
    };
    let (bin_west, bin_east, bin_south, bin_north) = raster_bin_bounds(nlon, nlat, ilon, jlat);
    let source_south = south.min(*north);
    let source_north = south.max(*north);
    if source_south < bin_south - 1.0e-13 || source_north > bin_north + 1.0e-13 {
        return false;
    }
    let source_width = if (*east - *west).abs() >= 360.0 - 1.0e-12 {
        360.0
    } else {
        (wrap_lon(*east) - wrap_lon(*west)).rem_euclid(360.0)
    };
    let overlap_width = cyclic_interval_intersections_with_bin(*west, *east, bin_west, bin_east)
        .into_iter()
        .map(|(west, east)| east - west)
        .sum::<f64>();
    source_width > 1.0e-13 && overlap_width >= source_width - 1.0e-13
}

fn region_rings_in_hfield_bin(
    region: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> io::Result<Vec<Vec<Point>>> {
    let (bin_west, bin_east, bin_south, bin_north) = raster_bin_bounds(nlon, nlat, ilon, jlat);
    match region {
        GridRegion::Bbox {
            west,
            east,
            south,
            north,
        } => {
            let domain_south = (*south).min(*north);
            let domain_north = (*south).max(*north);
            let south = domain_south.max(bin_south);
            let north = domain_north.min(bin_north);
            if north <= south + 1.0e-13 {
                return Ok(Vec::new());
            }
            Ok(
                cyclic_interval_intersections_with_bin(*west, *east, bin_west, bin_east)
                    .into_iter()
                    .map(|(west, east)| lonlat_rectangle_ring(west, east, south, north))
                    .collect(),
            )
        }
        GridRegion::Circle {
            lon,
            lat,
            radius_km,
        } => {
            if !lon.is_finite() || !lat.is_finite() || !radius_km.is_finite() || *radius_km <= 0.0 {
                return Ok(Vec::new());
            }
            let angular = radius_km / EARTH_RADIUS_KM;
            if angular >= std::f64::consts::PI {
                return Ok(vec![raster_bin_ring(nlon, nlat, ilon, jlat)]);
            }
            Ok(vec![spherical_circle_ring(*lon, *lat, angular)])
        }
        GridRegion::Close { points } => {
            if points.len() < 3
                || points
                    .iter()
                    .any(|point| !point.lon.is_finite() || !point.lat.is_finite())
            {
                return Err(invalid(
                    "close source/domain requires at least three finite lon/lat points",
                ));
            }
            Ok(vec![points
                .iter()
                .map(|point| Point::new(point.lon, point.lat))
                .collect()])
        }
        GridRegion::Any(_) => Err(invalid(
            "union source/domain must be expanded before geometric clipping",
        )),
    }
}

fn cyclic_interval_intersections_with_bin(
    west: f64,
    east: f64,
    bin_west: f64,
    bin_east: f64,
) -> Vec<(f64, f64)> {
    if !west.is_finite() || !east.is_finite() {
        return Vec::new();
    }
    if (east - west).abs() >= 360.0 - 1.0e-12 {
        return vec![(bin_west, bin_east)];
    }
    let start = wrap_lon(west);
    let span = (wrap_lon(east) - start).rem_euclid(360.0);
    if span <= 1.0e-13 {
        return Vec::new();
    }
    let mut intersections: Vec<(f64, f64)> = Vec::new();
    for shift in [-360.0, 0.0, 360.0] {
        let start = start + shift;
        let end = start + span;
        let overlap_west = start.max(bin_west);
        let overlap_east = end.min(bin_east);
        if overlap_east > overlap_west + 1.0e-13
            && !intersections.iter().any(|&(existing_west, existing_east)| {
                (existing_west - overlap_west).abs() <= 1.0e-13
                    && (existing_east - overlap_east).abs() <= 1.0e-13
            })
        {
            intersections.push((overlap_west, overlap_east));
        }
    }
    intersections
}

fn lonlat_rectangle_ring(west: f64, east: f64, south: f64, north: f64) -> Vec<Point> {
    let center_lon = 0.5 * (west + east);
    if south <= -90.0 + 1.0e-13 {
        let mut ring = vec![Point::new(center_lon, -90.0), Point::new(east, north)];
        append_parallel(&mut ring, north, east, west);
        return ring;
    }
    if north >= 90.0 - 1.0e-13 {
        let mut ring = vec![Point::new(west, south)];
        append_parallel(&mut ring, south, west, east);
        ring.push(Point::new(center_lon, 90.0));
        return ring;
    }
    let mut ring = vec![Point::new(west, south)];
    append_parallel(&mut ring, south, west, east);
    ring.push(Point::new(east, north));
    append_parallel(&mut ring, north, east, west);
    ring
}

fn spherical_circle_ring(center_lon: f64, center_lat: f64, angular_radius: f64) -> Vec<Point> {
    let circumference = 2.0 * std::f64::consts::PI * angular_radius.sin().abs();
    let segments = (circumference / PARALLEL_STEP_DEGREES.to_radians())
        .ceil()
        .clamp(64.0, 4096.0) as usize;
    let lon1 = center_lon.to_radians();
    let lat1 = center_lat.to_radians();
    (0..segments)
        .map(|segment| {
            let bearing = 2.0 * std::f64::consts::PI * segment as f64 / segments as f64;
            let lat2 = (lat1.sin() * angular_radius.cos()
                + lat1.cos() * angular_radius.sin() * bearing.cos())
            .clamp(-1.0, 1.0)
            .asin();
            let lon2 = lon1
                + (bearing.sin() * angular_radius.sin() * lat1.cos())
                    .atan2(angular_radius.cos() - lat1.sin() * lat2.sin());
            Point::new(wrap_lon(lon2.to_degrees()), lat2.to_degrees())
        })
        .collect()
}

/// Positive-area overlap between one HField bin and Polygon/MultiPolygon
/// components. Each component contains an exterior ring followed by zero or
/// more holes. Merely touching an edge or vertex is not active support.
pub(crate) fn polygon_components_overlap_hfield_bin(
    components: &[Vec<Vec<Point>>],
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> io::Result<bool> {
    if nlon == 0 || nlat == 0 || ilon >= nlon || jlat >= nlat {
        return Err(invalid(
            "invalid HField bin coordinates for polygon support",
        ));
    }
    let (west, east, south, north) = raster_bin_bounds(nlon, nlat, ilon, jlat);
    let bin = raster_bin_ring(nlon, nlat, ilon, jlat);
    let bin_cap = SphericalCap::for_rings(std::slice::from_ref(&bin))
        .ok_or_else(|| invalid("HField bin cannot form a spherical cap"))?;
    for component in components {
        let Some(exterior) = component.first().filter(|ring| ring.len() >= 3) else {
            return Err(invalid(
                "polygon support component requires an exterior ring with at least three points",
            ));
        };
        if component
            .iter()
            .flatten()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid(
                "polygon support requires finite longitude/latitude points",
            ));
        }
        let exterior_window = ring_window_bounds(exterior)?;
        if !exterior_window.has_positive_overlap(west, east, south, north) {
            continue;
        }
        let Some(exterior_cap) = SphericalCap::for_rings(std::slice::from_ref(exterior)) else {
            return Err(invalid("polygon exterior cannot form a spherical cap"));
        };
        if !bin_cap.overlaps(exterior_cap) {
            continue;
        }
        let projection = LocalEqualArea::for_rings(&[bin.clone(), exterior.clone()])
            .ok_or_else(|| invalid("polygon support cannot form an equal-area projection"))?;
        let projected_bin = projection
            .project_ring(&bin)
            .ok_or_else(|| invalid("HField bin cannot be projected"))?;
        let projected_exterior = projection
            .project_ring(exterior)
            .ok_or_else(|| invalid("polygon exterior cannot be projected"))?;
        let exterior_area = polygon_area(&projected_exterior);
        let (outer_overlap, _) = polygon_overlap_area(
            &projected_exterior,
            PlanarBounds::for_ring(&projected_exterior),
            polygon_is_convex(&projected_exterior),
            &projected_bin,
        );
        if outer_overlap <= 0.0 {
            continue;
        }
        let mut hole_overlap = 0.0;
        let mut hole_area = 0.0;
        for hole in component.iter().skip(1) {
            if hole.len() < 3 {
                return Err(invalid(
                    "polygon support hole requires at least three points",
                ));
            }
            let projected_hole = projection
                .project_ring(hole)
                .ok_or_else(|| invalid("polygon support hole cannot be projected"))?;
            hole_area += polygon_area(&projected_hole);
            let (overlap, _) = polygon_overlap_area(
                &projected_hole,
                PlanarBounds::for_ring(&projected_hole),
                polygon_is_convex(&projected_hole),
                &projected_bin,
            );
            hole_overlap += overlap;
        }
        let positive_area = (outer_overlap - hole_overlap).max(0.0);
        let component_area = (exterior_area - hole_area).max(0.0);
        let numerical_floor = 1.0e-16_f64.max(1.0e-12 * component_area);
        if positive_area > numerical_floor {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn positive_area_hfield_bins_for_polygon_components(
    components: &[Vec<Vec<Point>>],
    nlon: usize,
    nlat: usize,
) -> io::Result<Vec<(usize, usize)>> {
    let mut bins = std::collections::BTreeSet::new();
    for component in components {
        let Some(exterior) = component.first().filter(|ring| ring.len() >= 3) else {
            return Err(invalid(
                "polygon support component requires an exterior ring with at least three points",
            ));
        };
        let window = candidate_window(exterior, nlon, nlat)?;
        for jlat in window.rows {
            for &ilon in &window.columns {
                if polygon_components_overlap_hfield_bin(
                    std::slice::from_ref(component),
                    nlon,
                    nlat,
                    ilon,
                    jlat,
                )? {
                    bins.insert((ilon, jlat));
                }
            }
        }
    }
    Ok(bins.into_iter().collect())
}

pub(crate) fn polygon_components_and_region_overlap_hfield_bin(
    components: &[Vec<Vec<Point>>],
    domain: &GridRegion,
    nlon: usize,
    nlat: usize,
    ilon: usize,
    jlat: usize,
) -> io::Result<bool> {
    if let GridRegion::Any(regions) = domain {
        for region in regions {
            if polygon_components_and_region_overlap_hfield_bin(
                components, region, nlon, nlat, ilon, jlat,
            )? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if !grid_region_overlaps_hfield_bin(domain, nlon, nlat, ilon, jlat)? {
        return Ok(false);
    }
    let bin = raster_bin_ring(nlon, nlat, ilon, jlat);
    let domain_rings = region_rings_in_hfield_bin(domain, nlon, nlat, ilon, jlat)?;
    for component in components {
        let Some(exterior) = component.first().filter(|ring| ring.len() >= 3) else {
            return Err(invalid(
                "polygon support component requires an exterior ring with at least three points",
            ));
        };
        for domain_ring in &domain_rings {
            let mut projection_rings = vec![bin.clone(), exterior.clone(), domain_ring.clone()];
            projection_rings.extend(component.iter().skip(1).cloned());
            let projection = LocalEqualArea::for_rings(&projection_rings)
                .ok_or_else(|| invalid("polygon/domain intersection cannot form a projection"))?;
            let projected_bin = projection
                .project_ring(&bin)
                .ok_or_else(|| invalid("HField bin cannot be projected"))?;
            let projected_exterior = projection
                .project_ring(exterior)
                .ok_or_else(|| invalid("polygon exterior cannot be projected"))?;
            let projected_domain = projection
                .project_ring(domain_ring)
                .ok_or_else(|| invalid("output domain cannot be projected"))?;
            let outer_overlap = projected_triple_intersection_area(
                &projected_exterior,
                &projected_domain,
                &projected_bin,
            );
            if outer_overlap <= 0.0 {
                continue;
            }
            let mut hole_overlap = 0.0;
            let mut hole_area = 0.0;
            for hole in component.iter().skip(1) {
                let projected_hole = projection
                    .project_ring(hole)
                    .ok_or_else(|| invalid("polygon support hole cannot be projected"))?;
                hole_area += polygon_area(&projected_hole);
                hole_overlap += projected_triple_intersection_area(
                    &projected_hole,
                    &projected_domain,
                    &projected_bin,
                );
            }
            let component_area = (polygon_area(&projected_exterior) - hole_area).max(0.0);
            let numerical_floor = 1.0e-16_f64.max(1.0e-12 * component_area);
            if (outer_overlap - hole_overlap).max(0.0) > numerical_floor {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn projected_triple_intersection_area(first: &[Point], second: &[Point], third: &[Point]) -> f64 {
    if polygon_is_convex(first) && polygon_is_convex(second) && polygon_is_convex(third) {
        let first_second = clip_convex_polygon(first, second);
        return polygon_area(&clip_convex_polygon(&first_second, third));
    }
    polygon_triple_intersection_area_even_odd(first, second, third)
}

fn raster_bin_bounds(nlon: usize, nlat: usize, ilon: usize, jlat: usize) -> (f64, f64, f64, f64) {
    let dlon = 360.0 / nlon as f64;
    let dlat = 180.0 / nlat as f64;
    let west = -180.0 + ilon as f64 * dlon;
    let south = -90.0 + jlat as f64 * dlat;
    (west, west + dlon, south, south + dlat)
}

fn cyclic_intervals_have_positive_overlap(
    first_west: f64,
    first_east: f64,
    second_west: f64,
    second_east: f64,
) -> bool {
    if !first_west.is_finite()
        || !first_east.is_finite()
        || !second_west.is_finite()
        || !second_east.is_finite()
    {
        return false;
    }
    let first_span = if (first_east - first_west).abs() >= 360.0 - 1.0e-12 {
        360.0
    } else {
        (wrap_lon(first_east) - wrap_lon(first_west)).rem_euclid(360.0)
    };
    let second_span = if (second_east - second_west).abs() >= 360.0 - 1.0e-12 {
        360.0
    } else {
        (wrap_lon(second_east) - wrap_lon(second_west)).rem_euclid(360.0)
    };
    if first_span <= 1.0e-13 || second_span <= 1.0e-13 {
        return false;
    }
    if first_span >= 360.0 - 1.0e-12 || second_span >= 360.0 - 1.0e-12 {
        return true;
    }
    let first_start = wrap_lon(first_west);
    let first_end = first_start + first_span;
    let second_start = wrap_lon(second_west);
    for shift in [-360.0, 0.0, 360.0] {
        let second_start = second_start + shift;
        let second_end = second_start + second_span;
        if first_end.min(second_end) > first_start.max(second_start) + 1.0e-13 {
            return true;
        }
    }
    false
}

fn cyclic_interval_contains(longitude: f64, west: f64, east: f64) -> bool {
    let longitude = wrap_lon(longitude);
    let west = wrap_lon(west);
    let span = (wrap_lon(east) - west).rem_euclid(360.0);
    if span >= 360.0 - 1.0e-12 {
        return true;
    }
    for shift in [-360.0, 0.0, 360.0] {
        let longitude = longitude + shift;
        if longitude >= west - 1.0e-13 && longitude <= west + span + 1.0e-13 {
            return true;
        }
    }
    false
}

fn minimum_angular_distance_to_lonlat_bin(
    longitude: f64,
    latitude: f64,
    west: f64,
    east: f64,
    south: f64,
    north: f64,
) -> f64 {
    let latitude = latitude.clamp(-90.0, 90.0).to_radians();
    let south = south.to_radians();
    let north = north.to_radians();
    let mut minimum = f64::INFINITY;

    if cyclic_interval_contains(longitude, west, east) {
        minimum = minimum.min((latitude - latitude.clamp(south, north)).abs());
    }
    for meridian in [west, east] {
        let delta = normalize_delta(meridian - longitude).to_radians();
        let a = latitude.sin();
        let b = latitude.cos() * delta.cos();
        let mut maximum_dot =
            (a * south.sin() + b * south.cos()).max(a * north.sin() + b * north.cos());
        let optimum = a.atan2(b);
        for candidate in [
            optimum - std::f64::consts::PI,
            optimum,
            optimum + std::f64::consts::PI,
        ] {
            if candidate >= south && candidate <= north {
                maximum_dot = maximum_dot.max(a * candidate.sin() + b * candidate.cos());
            }
        }
        minimum = minimum.min(maximum_dot.clamp(-1.0, 1.0).acos());
    }
    minimum
}

struct BinGeometry {
    ring: Vec<Point>,
    cap: SphericalCap,
}

#[derive(Clone, Copy)]
struct PlanarBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl PlanarBounds {
    fn for_ring(ring: &[Point]) -> Self {
        ring.iter().fold(
            Self {
                min_x: f64::INFINITY,
                max_x: f64::NEG_INFINITY,
                min_y: f64::INFINITY,
                max_y: f64::NEG_INFINITY,
            },
            |bounds, point| Self {
                min_x: bounds.min_x.min(point.x),
                max_x: bounds.max_x.max(point.x),
                min_y: bounds.min_y.min(point.y),
                max_y: bounds.max_y.max(point.y),
            },
        )
    }

    fn has_positive_overlap(self, other: Self) -> bool {
        self.max_x > other.min_x
            && other.max_x > self.min_x
            && self.max_y > other.min_y
            && other.max_y > self.min_y
    }
}

#[derive(Debug)]
struct CellWindow {
    rows: std::ops::RangeInclusive<usize>,
    columns: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
struct RingWindowBounds {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
    encloses_pole: bool,
}

impl RingWindowBounds {
    fn has_positive_overlap(self, west: f64, east: f64, south: f64, north: f64) -> bool {
        if self.max_lat.min(north) <= self.min_lat.max(south) + 1.0e-13 {
            return false;
        }
        if self.encloses_pole || self.max_lon - self.min_lon >= 360.0 - 1.0e-12 {
            return true;
        }
        [-720.0, -360.0, 0.0, 360.0, 720.0]
            .into_iter()
            .any(|shift| self.max_lon.min(east + shift) > self.min_lon.max(west + shift) + 1.0e-13)
    }
}

/// Required level for each final quality cell.
///
/// A nonzero regularized HField bin contributes only when its spherical support
/// has positive area inside the final cell polygon. Point and edge contacts do
/// not contribute. Candidate work is bounded to raster row/column bands before
/// the spherical-cap and equal-area clipping checks.
#[cfg(test)]
fn target_levels_for_positive_support(
    input: &QualityMeshInput,
    nlon: usize,
    nlat: usize,
    levels: &[u8],
    intended_support: &[bool],
) -> io::Result<(Vec<u32>, SupportCoverage)> {
    target_levels_with_hard_coverage(input, nlon, nlat, levels, levels, intended_support)
}

pub(crate) fn target_levels_with_hard_coverage(
    input: &QualityMeshInput,
    nlon: usize,
    nlat: usize,
    regularized_levels: &[u8],
    hard_levels: &[u8],
    intended_support: &[bool],
) -> io::Result<(Vec<u32>, SupportCoverage)> {
    if nlon < 4 || nlat < 2 {
        return Err(invalid(format!(
            "HField support raster {nlon}x{nlat} is too small"
        )));
    }
    let expected = nlon
        .checked_mul(nlat)
        .ok_or_else(|| invalid("HField support raster dimensions overflow usize"))?;
    if hard_levels.len() != expected || regularized_levels.len() != expected {
        return Err(invalid(format!(
            "HField support rasters have {}/{} levels, expected {nlon}x{nlat}={expected}",
            hard_levels.len(),
            regularized_levels.len()
        )));
    }
    if intended_support.len() != expected {
        return Err(invalid(format!(
            "HField intended-support raster has {} values, expected {nlon}x{nlat}={expected}",
            intended_support.len()
        )));
    }

    let active_bin_count = hard_levels
        .iter()
        .zip(intended_support)
        .filter(|(level, intended)| **level != 0 && **intended)
        .count();
    let mut targets = vec![0_u32; input.cells.len()];
    let target_bin_count = regularized_levels
        .iter()
        .zip(intended_support)
        .filter(|(level, intended)| **level != 0 && **intended)
        .count();
    if active_bin_count == 0 && target_bin_count == 0 {
        return Ok((
            targets,
            SupportCoverage {
                covered_bins: vec![false; expected],
                ..SupportCoverage::default()
            },
        ));
    }

    let mut bin_cache = (0..expected).map(|_| None).collect::<Vec<_>>();
    let mut covered_bins = vec![false; expected];
    let mut adequately_covered_bins = vec![false; expected];
    let mut candidate_pair_count = 0_usize;
    let mut convex_clip_pair_count = 0_usize;
    let mut generic_clip_pair_count = 0_usize;
    let mut positive_overlap_count = 0_usize;

    for (cell_index, cell) in input.cells.iter().enumerate() {
        let ring = cell
            .vertices
            .iter()
            .map(|&vertex| {
                input.vertices.get(vertex).copied().ok_or_else(|| {
                    invalid(format!(
                        "quality cell {cell_index} references missing vertex {vertex}"
                    ))
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        if ring.len() < 3 {
            return Err(invalid(format!(
                "quality cell {cell_index} has fewer than three vertices"
            )));
        }

        let cell_cap = SphericalCap::for_rings(std::slice::from_ref(&ring)).ok_or_else(|| {
            invalid(format!(
                "quality cell {cell_index} cannot form a spherical broad-phase cap"
            ))
        })?;
        let projection =
            LocalEqualArea::for_rings(std::slice::from_ref(&ring)).ok_or_else(|| {
                invalid(format!(
                    "quality cell {cell_index} cannot form a local equal-area projection"
                ))
            })?;
        let projected_cell = projection.project_ring(&ring).ok_or_else(|| {
            invalid(format!(
                "quality cell {cell_index} cannot be projected for HField coverage"
            ))
        })?;
        let projected_cell_area = polygon_area(&projected_cell);
        if projected_cell_area <= 0.0 {
            return Err(invalid(format!(
                "quality cell {cell_index} has zero projected area"
            )));
        }
        let projected_cell_is_convex = polygon_is_convex(&projected_cell);
        let projected_cell_bounds = PlanarBounds::for_ring(&projected_cell);

        let window = candidate_window(&ring, nlon, nlat)?;
        for jlat in window.rows {
            for &ilon in &window.columns {
                let raster_index = jlat * nlon + ilon;
                let target_level = regularized_levels[raster_index];
                let hard_level = hard_levels[raster_index];
                if (target_level == 0 && hard_level == 0) || !intended_support[raster_index] {
                    continue;
                }
                candidate_pair_count += 1;

                let bin_geometry = match bin_cache[raster_index].as_ref() {
                    Some(geometry) => geometry,
                    None => {
                        let ring = raster_bin_ring(nlon, nlat, ilon, jlat);
                        let cap = SphericalCap::for_rings(std::slice::from_ref(&ring)).ok_or_else(
                            || {
                                invalid(format!(
                                    "HField bin ({ilon}, {jlat}) cannot form a spherical cap"
                                ))
                            },
                        )?;
                        bin_cache[raster_index] = Some(BinGeometry { ring, cap });
                        bin_cache[raster_index]
                            .as_ref()
                            .expect("HField bin geometry was just inserted")
                    }
                };
                if !cell_cap.overlaps(bin_geometry.cap) {
                    continue;
                }

                let projected_bin = projection.project_ring(&bin_geometry.ring).ok_or_else(|| {
                    invalid(format!(
                        "HField bin ({ilon}, {jlat}) cannot be projected near quality cell {cell_index}"
                    ))
                })?;
                let (overlap, used_convex_clip) = polygon_overlap_area(
                    &projected_cell,
                    projected_cell_bounds,
                    projected_cell_is_convex,
                    &projected_bin,
                );
                if used_convex_clip {
                    convex_clip_pair_count += 1;
                } else {
                    generic_clip_pair_count += 1;
                }
                let numerical_floor = 1.0e-16_f64.max(1.0e-12 * projected_cell_area);
                if overlap <= numerical_floor {
                    continue;
                }

                targets[cell_index] = targets[cell_index].max(u32::from(target_level));
                if hard_level != 0 {
                    covered_bins[raster_index] = true;
                    if cell
                        .refine_level
                        .is_some_and(|level| level >= u32::from(hard_level))
                    {
                        adequately_covered_bins[raster_index] = true;
                    }
                }
                positive_overlap_count += 1;
            }
        }
    }

    let covered_bin_count = covered_bins.iter().filter(|covered| **covered).count();
    let adequately_covered_bin_count = adequately_covered_bins
        .iter()
        .filter(|covered| **covered)
        .count();
    Ok((
        targets,
        SupportCoverage {
            active_bin_count,
            candidate_pair_count,
            convex_clip_pair_count,
            generic_clip_pair_count,
            positive_overlap_count,
            covered_bin_count,
            covered_bins,
            adequately_covered_bin_count,
        },
    ))
}

fn polygon_overlap_area(
    left: &[Point],
    left_bounds: PlanarBounds,
    left_is_convex: bool,
    right: &[Point],
) -> (f64, bool) {
    if !left_bounds.has_positive_overlap(PlanarBounds::for_ring(right)) {
        return (0.0, true);
    }
    let right_is_convex = polygon_is_convex(right);
    if left_is_convex && right_is_convex {
        return (polygon_area(&clip_convex_polygon(right, left)), true);
    }
    let left_fan = (!left_is_convex).then(|| exact_fan_center(left)).flatten();
    let right_fan = (!right_is_convex)
        .then(|| exact_fan_center(right))
        .flatten();
    if left_is_convex {
        if let Some(center) = right_fan {
            return (fan_against_convex_area(right, center, left), true);
        }
    } else if right_is_convex {
        if let Some(center) = left_fan {
            return (fan_against_convex_area(left, center, right), true);
        }
    } else if let (Some(left_center), Some(right_center)) = (left_fan, right_fan) {
        return (
            fan_against_fan_area(left, left_center, right, right_center),
            true,
        );
    }
    (intersection_area(left, right), false)
}

/// A point in the polygon kernel yields a disjoint fan triangulation. Requiring
/// every fan triangle to have the polygon orientation and the absolute areas
/// to sum back to the polygon area makes this a checked fast path rather than
/// assuming that a non-convex projected ring is safe to clip as one piece.
fn exact_fan_center(polygon: &[Point]) -> Option<Point> {
    if polygon.len() < 3 {
        return None;
    }
    let center = Point::new(
        polygon.iter().map(|point| point.x).sum::<f64>() / polygon.len() as f64,
        polygon.iter().map(|point| point.y).sum::<f64>() / polygon.len() as f64,
    );
    let signed_area_twice = polygon
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = polygon[(index + 1) % polygon.len()];
            point.x * next.y - next.x * point.y
        })
        .sum::<f64>();
    if signed_area_twice.abs() <= 1.0e-24 {
        return None;
    }
    let orientation = signed_area_twice.signum();
    let scale = polygon.iter().fold(1.0_f64, |scale, point| {
        scale.max(point.x.abs()).max(point.y.abs())
    });
    let triangle_floor = 256.0 * f64::EPSILON * scale * scale;
    let mut fan_area = 0.0;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let cross =
            (start.x - center.x) * (end.y - center.y) - (start.y - center.y) * (end.x - center.x);
        if orientation * cross < -triangle_floor {
            return None;
        }
        fan_area += 0.5 * cross.abs();
    }
    let area = polygon_area(polygon);
    let tolerance = 1.0e-10 * area.max(1.0e-16);
    ((fan_area - area).abs() <= tolerance).then_some(center)
}

fn fan_against_convex_area(subject: &[Point], center: Point, clip: &[Point]) -> f64 {
    (0..subject.len())
        .map(|index| {
            let triangle = [center, subject[index], subject[(index + 1) % subject.len()]];
            polygon_area(&clip_convex_polygon(&triangle, clip))
        })
        .sum()
}

fn fan_against_fan_area(
    left: &[Point],
    left_center: Point,
    right: &[Point],
    right_center: Point,
) -> f64 {
    let mut area = 0.0;
    for left_index in 0..left.len() {
        let left_triangle = [
            left_center,
            left[left_index],
            left[(left_index + 1) % left.len()],
        ];
        for right_index in 0..right.len() {
            let right_triangle = [
                right_center,
                right[right_index],
                right[(right_index + 1) % right.len()],
            ];
            area += polygon_area(&clip_convex_polygon(&left_triangle, &right_triangle));
        }
    }
    area
}

fn polygon_is_convex(polygon: &[Point]) -> bool {
    if polygon.len() < 3 || polygon_area(polygon) <= 0.0 {
        return false;
    }
    let scale = polygon.iter().fold(1.0_f64, |scale, point| {
        scale.max(point.x.abs()).max(point.y.abs())
    });
    let tolerance = 256.0 * f64::EPSILON * scale * scale;
    let mut sign = 0_i8;
    for index in 0..polygon.len() {
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let current = polygon[index];
        let next = polygon[(index + 1) % polygon.len()];
        let cross = (current.x - previous.x) * (next.y - current.y)
            - (current.y - previous.y) * (next.x - current.x);
        if cross.abs() <= tolerance {
            continue;
        }
        let current_sign = if cross > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = current_sign;
        } else if sign != current_sign {
            return false;
        }
    }
    sign != 0
}

fn candidate_window(ring: &[Point], nlon: usize, nlat: usize) -> io::Result<CellWindow> {
    let bounds = ring_window_bounds(ring)?;
    let dlat = 180.0 / nlat as f64;
    let scaled_tolerance = 1.0e-10;
    let start_row = (((bounds.min_lat + 90.0) / dlat - scaled_tolerance).floor() as isize)
        .clamp(0, nlat as isize - 1) as usize;
    let end_row = (((bounds.max_lat + 90.0) / dlat + scaled_tolerance).floor() as isize)
        .clamp(0, nlat as isize - 1) as usize;

    let columns = if bounds.encloses_pole {
        (0..nlon).collect()
    } else {
        candidate_columns(bounds.min_lon, bounds.max_lon, nlon)
    };
    Ok(CellWindow {
        rows: start_row..=end_row,
        columns,
    })
}

fn ring_window_bounds(ring: &[Point]) -> io::Result<RingWindowBounds> {
    if ring.len() < 3 {
        return Err(invalid(
            "spherical support ring requires at least three vertices",
        ));
    }
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut normalized_lons = Vec::with_capacity(ring.len());
    for (vertex, point) in ring.iter().enumerate() {
        if !point.x.is_finite() || !point.y.is_finite() || !(-90.0..=90.0).contains(&point.y) {
            return Err(invalid(format!(
                "quality-cell vertex {vertex} must contain finite lon/lat with latitude in [-90, 90]"
            )));
        }
        min_lat = min_lat.min(point.y);
        max_lat = max_lat.max(point.y);
        normalized_lons.push(wrap_lon(point.x));
    }

    for index in 0..ring.len() {
        extend_latitude_for_minor_arc(
            ring[index],
            ring[(index + 1) % ring.len()],
            &mut min_lat,
            &mut max_lat,
        )?;
    }

    let mut unwrapped_lons = vec![normalized_lons[0]];
    let mut winding = 0.0;
    for &longitude in &normalized_lons[1..] {
        let delta = normalize_delta(longitude - *unwrapped_lons.last().unwrap());
        winding += delta;
        unwrapped_lons.push(*unwrapped_lons.last().unwrap() + delta);
    }
    winding += normalize_delta(normalized_lons[0] - *unwrapped_lons.last().unwrap());
    let encloses_pole = winding.abs() > 180.0;
    if encloses_pole {
        if ring.iter().map(|point| point.y).sum::<f64>() >= 0.0 {
            max_lat = 90.0;
        } else {
            min_lat = -90.0;
        }
    }

    Ok(RingWindowBounds {
        min_lat,
        max_lat,
        min_lon: unwrapped_lons.iter().copied().fold(f64::INFINITY, f64::min),
        max_lon: unwrapped_lons
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max),
        encloses_pole,
    })
}

fn candidate_columns(min_lon: f64, max_lon: f64, nlon: usize) -> Vec<usize> {
    let dlon = 360.0 / nlon as f64;
    let scaled_tolerance = 1.0e-10;
    let start = ((min_lon + 180.0) / dlon - scaled_tolerance).floor() as isize;
    let end = ((max_lon + 180.0) / dlon + scaled_tolerance).floor() as isize;
    if end - start + 1 >= nlon as isize {
        return (0..nlon).collect();
    }
    let mut columns = Vec::with_capacity((end - start + 1).max(0) as usize);
    for column in start..=end {
        let column = column.rem_euclid(nlon as isize) as usize;
        if !columns.contains(&column) {
            columns.push(column);
        }
    }
    columns
}

fn extend_latitude_for_minor_arc(
    start: Point,
    end: Point,
    min_lat: &mut f64,
    max_lat: &mut f64,
) -> io::Result<()> {
    let start = lonlat_to_unit(start);
    let end = lonlat_to_unit(end);
    let arc = angular_distance(start, end);
    if arc >= std::f64::consts::PI - ARC_EPSILON_RADIANS {
        return Err(invalid(
            "quality-cell edge is antipodal and has no unique minor arc",
        ));
    }
    let Some(normal) = normalize(cross(start, end)) else {
        return Ok(());
    };
    let north = [0.0, 0.0, 1.0];
    let projected_north = [
        north[0] - dot(north, normal) * normal[0],
        north[1] - dot(north, normal) * normal[1],
        north[2] - dot(north, normal) * normal[2],
    ];
    let Some(extreme) = normalize(projected_north) else {
        return Ok(());
    };
    for candidate in [extreme, [-extreme[0], -extreme[1], -extreme[2]]] {
        if angular_distance(start, candidate) + angular_distance(candidate, end)
            <= arc + ARC_EPSILON_RADIANS
        {
            let latitude = candidate[2].clamp(-1.0, 1.0).asin().to_degrees();
            *min_lat = min_lat.min(latitude);
            *max_lat = max_lat.max(latitude);
        }
    }
    Ok(())
}

fn raster_bin_ring(nlon: usize, nlat: usize, ilon: usize, jlat: usize) -> Vec<Point> {
    let dlon = 360.0 / nlon as f64;
    let dlat = 180.0 / nlat as f64;
    let west = -180.0 + ilon as f64 * dlon;
    let east = west + dlon;
    let south = -90.0 + jlat as f64 * dlat;
    let north = south + dlat;
    let center_lon = west + 0.5 * dlon;

    if jlat == 0 {
        let mut ring = vec![Point::new(center_lon, -90.0), Point::new(east, north)];
        append_parallel(&mut ring, north, east, west);
        return ring;
    }
    if jlat + 1 == nlat {
        let mut ring = vec![Point::new(west, south)];
        append_parallel(&mut ring, south, west, east);
        ring.push(Point::new(center_lon, 90.0));
        return ring;
    }

    let mut ring = vec![Point::new(west, south)];
    append_parallel(&mut ring, south, west, east);
    ring.push(Point::new(east, north));
    append_parallel(&mut ring, north, east, west);
    ring
}

fn append_parallel(ring: &mut Vec<Point>, latitude: f64, start_lon: f64, end_lon: f64) {
    let segments = ((end_lon - start_lon).abs() / PARALLEL_STEP_DEGREES)
        .ceil()
        .max(1.0) as usize;
    for segment in 1..=segments {
        let t = segment as f64 / segments as f64;
        ring.push(Point::new(start_lon + t * (end_lon - start_lon), latitude));
    }
}

fn wrap_lon(longitude: f64) -> f64 {
    (longitude + 180.0).rem_euclid(360.0) - 180.0
}

fn normalize_delta(delta: f64) -> f64 {
    let normalized = (delta + 180.0).rem_euclid(360.0) - 180.0;
    if normalized == -180.0 && delta > 0.0 {
        180.0
    } else {
        normalized
    }
}

type Vec3 = [f64; 3];

fn lonlat_to_unit(point: Point) -> Vec3 {
    let longitude = point.x.to_radians();
    let latitude = point.y.to_radians();
    [
        latitude.cos() * longitude.cos(),
        latitude.cos() * longitude.sin(),
        latitude.sin(),
    ]
}

fn dot(left: Vec3, right: Vec3) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: Vec3, right: Vec3) -> Vec3 {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize(vector: Vec3) -> Option<Vec3> {
    let length = dot(vector, vector).sqrt();
    (length > 64.0 * f64::EPSILON)
        .then(|| [vector[0] / length, vector[1] / length, vector[2] / length])
}

fn angular_distance(left: Vec3, right: Vec3) -> f64 {
    dot(left, right).clamp(-1.0, 1.0).acos()
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_quality::QualityCell;

    fn mesh(rings: &[Vec<Point>]) -> QualityMeshInput {
        let mut vertices = Vec::new();
        let mut cells = Vec::new();
        for ring in rings {
            let start = vertices.len();
            vertices.extend_from_slice(ring);
            cells.push(QualityCell {
                vertices: (start..vertices.len()).collect(),
                refine_level: Some(0),
                neighbors: Vec::new(),
            });
        }
        QualityMeshInput { vertices, cells }
    }

    fn raster(nlon: usize, nlat: usize, bins: &[(usize, usize, u8)]) -> Vec<u8> {
        let mut levels = vec![0; nlon * nlat];
        for &(ilon, jlat, level) in bins {
            levels[jlat * nlon + ilon] = level;
        }
        levels
    }

    fn all_intended(levels: &[u8]) -> Vec<bool> {
        vec![true; levels.len()]
    }

    #[test]
    fn regional_bbox_mask_is_row_major_and_excludes_boundary_only_apron_bins() {
        let levels = vec![1; 8 * 4];
        let region = GridRegion::Bbox {
            west: -135.0,
            east: -90.0,
            south: -45.0,
            north: 0.0,
        };

        let mask = intended_domain_support_mask(8, 4, &levels, Some(&region)).unwrap();

        assert!(mask[1 * 8 + 1], "positive-area domain bin");
        assert!(!mask[1 * 8 + 2], "longitude boundary touch is not support");
        assert!(!mask[2 * 8 + 1], "latitude boundary touch is not support");
        assert!(!mask[0], "gradient apron outside the output domain");
    }

    #[test]
    fn regional_circle_mask_keeps_intersections_not_just_bin_centers() {
        let levels = vec![1; 8 * 4];
        let region = GridRegion::Circle {
            lon: 0.0,
            lat: 0.0,
            radius_km: 500.0,
        };

        let mask = intended_domain_support_mask(8, 4, &levels, Some(&region)).unwrap();

        for (ilon, jlat) in [(3, 1), (4, 1), (3, 2), (4, 2)] {
            assert!(
                mask[jlat * 8 + ilon],
                "circle has positive support in bin ({ilon}, {jlat})"
            );
        }
        assert!(!mask[0], "distant apron bin");
    }

    #[test]
    fn regional_close_mask_skips_zero_hard_level_bins() {
        let levels = vec![0; 8 * 4];
        let invalid_if_evaluated = GridRegion::Close {
            points: vec![
                crate::LonLatPoint {
                    lon: f64::NAN,
                    lat: 0.0,
                },
                crate::LonLatPoint { lon: 1.0, lat: 0.0 },
                crate::LonLatPoint { lon: 0.0, lat: 1.0 },
            ],
        };

        let mask =
            intended_domain_support_mask(8, 4, &levels, Some(&invalid_if_evaluated)).unwrap();

        assert_eq!(mask, vec![false; 8 * 4]);
    }

    #[test]
    fn sub_bin_bbox_circle_and_close_have_positive_support_without_a_bin_center() {
        let bbox = GridRegion::Bbox {
            west: 1.0,
            east: 2.0,
            south: 1.0,
            north: 2.0,
        };
        let circle = GridRegion::Circle {
            lon: 1.5,
            lat: 1.5,
            radius_km: 10.0,
        };
        let close = GridRegion::Close {
            points: vec![
                crate::LonLatPoint { lon: 1.0, lat: 1.0 },
                crate::LonLatPoint { lon: 2.0, lat: 1.0 },
                crate::LonLatPoint { lon: 2.0, lat: 2.0 },
                crate::LonLatPoint { lon: 1.0, lat: 2.0 },
            ],
        };

        for region in [&bbox, &circle, &close] {
            assert!(
                grid_region_overlaps_hfield_bin(region, 36, 18, 18, 9).unwrap(),
                "{region:?} lies wholly inside [0,10]x[0,10], away from its center"
            );
        }
    }

    #[test]
    fn region_edge_contact_is_not_positive_support() {
        let bbox = GridRegion::Bbox {
            west: 1.0,
            east: 10.0,
            south: 1.0,
            north: 2.0,
        };
        let close = GridRegion::Close {
            points: vec![
                crate::LonLatPoint { lon: 1.0, lat: 1.0 },
                crate::LonLatPoint {
                    lon: 10.0,
                    lat: 1.0,
                },
                crate::LonLatPoint {
                    lon: 10.0,
                    lat: 2.0,
                },
                crate::LonLatPoint { lon: 1.0, lat: 2.0 },
            ],
        };

        assert!(!grid_region_overlaps_hfield_bin(&bbox, 36, 18, 19, 9).unwrap());
        assert!(!grid_region_overlaps_hfield_bin(&close, 36, 18, 19, 9).unwrap());
    }

    #[test]
    fn same_bin_disjoint_source_and_domain_do_not_create_hard_support() {
        let source = GridRegion::Bbox {
            west: 1.0,
            east: 2.0,
            south: 1.0,
            north: 2.0,
        };
        let domain = GridRegion::Bbox {
            west: 8.0,
            east: 9.0,
            south: 1.0,
            north: 2.0,
        };

        assert!(grid_region_overlaps_hfield_bin(&source, 36, 18, 18, 9).unwrap());
        assert!(grid_region_overlaps_hfield_bin(&domain, 36, 18, 18, 9).unwrap());
        assert!(
            !grid_regions_intersection_overlaps_hfield_bin(&source, &domain, 36, 18, 18, 9)
                .unwrap()
        );
    }

    #[test]
    fn source_domain_triple_intersection_requires_area_not_boundary_touch() {
        let source = GridRegion::Bbox {
            west: 1.0,
            east: 5.0,
            south: 1.0,
            north: 2.0,
        };
        let touching = GridRegion::Bbox {
            west: 5.0,
            east: 9.0,
            south: 1.0,
            north: 2.0,
        };
        let overlapping = GridRegion::Bbox {
            west: 4.0,
            east: 9.0,
            south: 1.0,
            north: 2.0,
        };

        assert!(
            !grid_regions_intersection_overlaps_hfield_bin(&source, &touching, 36, 18, 18, 9)
                .unwrap()
        );
        assert!(grid_regions_intersection_overlaps_hfield_bin(
            &source,
            &overlapping,
            36,
            18,
            18,
            9,
        )
        .unwrap());
    }

    #[test]
    fn source_pixel_center_inside_close_has_positive_area_support() {
        let source = GridRegion::Bbox {
            west: 1.25,
            east: 1.75,
            south: 1.25,
            north: 1.75,
        };
        let domain = GridRegion::Close {
            points: vec![
                crate::LonLatPoint { lon: 1.0, lat: 1.0 },
                crate::LonLatPoint { lon: 2.0, lat: 1.0 },
                crate::LonLatPoint { lon: 2.0, lat: 2.0 },
                crate::LonLatPoint { lon: 1.0, lat: 2.0 },
            ],
        };

        assert!(
            grid_regions_intersection_overlaps_hfield_bin(&source, &domain, 36, 18, 18, 9,)
                .unwrap()
        );
    }

    #[test]
    fn source_inside_bridged_hole_domain_keeps_positive_support() {
        let source = GridRegion::Close {
            points: vec![
                crate::LonLatPoint {
                    lon: 110.1,
                    lat: 20.1,
                },
                crate::LonLatPoint {
                    lon: 111.1,
                    lat: 20.1,
                },
                crate::LonLatPoint {
                    lon: 111.1,
                    lat: 23.9,
                },
                crate::LonLatPoint {
                    lon: 110.1,
                    lat: 23.9,
                },
            ],
        };
        let domain = GridRegion::Close {
            points: [
                (110.0, 20.0),
                (114.0, 20.0),
                (114.0, 22.75),
                (112.75, 22.75),
                (112.75, 21.25),
                (111.25, 21.25),
                (111.25, 22.75),
                (112.75, 22.75),
                (114.0, 22.75),
                (114.0, 24.0),
                (110.0, 24.0),
            ]
            .into_iter()
            .map(|(lon, lat)| crate::LonLatPoint { lon, lat })
            .collect(),
        };

        assert!(
            grid_regions_intersection_overlaps_hfield_bin(&source, &domain, 720, 360, 580, 220)
                .unwrap(),
            "the western source strip overlaps the shell outside the interior hole"
        );
        assert!(
            !grid_regions_intersection_overlaps_hfield_bin(
                &GridRegion::Bbox {
                    west: 111.5,
                    east: 112.5,
                    south: 21.5,
                    north: 22.5,
                },
                &domain,
                720,
                360,
                583,
                223,
            )
            .unwrap(),
            "the doubled bridge must retain the interior hole"
        );
    }

    #[test]
    fn circle_and_close_are_clipped_by_the_actual_sub_bin_domain() {
        let circle = GridRegion::Circle {
            lon: 1.5,
            lat: 1.5,
            radius_km: 20.0,
        };
        let close = GridRegion::Close {
            points: vec![
                crate::LonLatPoint { lon: 1.0, lat: 1.0 },
                crate::LonLatPoint { lon: 2.0, lat: 1.0 },
                crate::LonLatPoint { lon: 2.0, lat: 2.0 },
                crate::LonLatPoint { lon: 1.0, lat: 2.0 },
            ],
        };
        let disjoint = GridRegion::Bbox {
            west: 8.0,
            east: 9.0,
            south: 1.0,
            north: 2.0,
        };
        let overlapping = GridRegion::Bbox {
            west: 1.25,
            east: 1.75,
            south: 1.25,
            north: 1.75,
        };

        for source in [&circle, &close] {
            assert!(
                !grid_regions_intersection_overlaps_hfield_bin(source, &disjoint, 36, 18, 18, 9)
                    .unwrap(),
                "{source:?}"
            );
            assert!(
                grid_regions_intersection_overlaps_hfield_bin(source, &overlapping, 36, 18, 18, 9,)
                    .unwrap(),
                "{source:?}"
            );
        }
    }

    #[test]
    fn polygon_support_handles_antimeridian_and_polar_components() {
        let antimeridian = vec![vec![
            Point::new(179.0, 1.0),
            Point::new(-179.0, 1.0),
            Point::new(-179.0, 2.0),
            Point::new(179.0, 2.0),
        ]];
        let polar = vec![vec![
            Point::new(-120.0, 89.0),
            Point::new(0.0, 89.0),
            Point::new(120.0, 89.0),
        ]];

        let seam_bins =
            positive_area_hfield_bins_for_polygon_components(&[antimeridian], 36, 18).unwrap();
        assert!(seam_bins.contains(&(35, 9)));
        assert!(seam_bins.contains(&(0, 9)));

        let polar_bins =
            positive_area_hfield_bins_for_polygon_components(&[polar], 36, 18).unwrap();
        assert!(!polar_bins.is_empty());
        assert!(
            polar_bins.iter().all(|(_, jlat)| *jlat == 17),
            "{polar_bins:?}"
        );
    }

    #[test]
    fn polygon_support_preserves_holes_and_disconnected_components() {
        let holed = vec![
            vec![
                Point::new(-2.0, -2.0),
                Point::new(2.0, -2.0),
                Point::new(2.0, 2.0),
                Point::new(-2.0, 2.0),
            ],
            vec![
                Point::new(-1.0, -1.0),
                Point::new(1.0, -1.0),
                Point::new(1.0, 1.0),
                Point::new(-1.0, 1.0),
            ],
        ];
        let detached = vec![vec![
            Point::new(100.1, 1.1),
            Point::new(100.9, 1.1),
            Point::new(100.9, 1.9),
            Point::new(100.1, 1.9),
        ]];

        let bins =
            positive_area_hfield_bins_for_polygon_components(&[holed, detached], 360, 180).unwrap();

        assert!(
            !bins.contains(&(180, 90)),
            "the [0,1]x[0,1] bin is wholly inside the hole"
        );
        assert!(bins.contains(&(181, 90)), "exterior support remains active");
        assert!(
            bins.contains(&(280, 91)),
            "the detached MultiPolygon component is retained"
        );
    }

    #[test]
    fn unintended_active_bin_is_not_an_uncovered_support_failure() {
        let input = QualityMeshInput::default();
        let levels = raster(8, 4, &[(7, 3, 2)]);
        let intended = vec![false; levels.len()];

        let (targets, coverage) =
            target_levels_for_positive_support(&input, 8, 4, &levels, &intended).unwrap();

        assert!(targets.is_empty());
        assert_eq!(coverage.active_bin_count, 0);
        assert_eq!(coverage.covered_bin_count, 0);
    }

    #[test]
    fn wide_nonpolar_polygon_does_not_expand_to_all_longitudes() {
        let ring = vec![
            Point::new(-80.0, -10.0),
            Point::new(80.0, -10.0),
            Point::new(80.0, 10.0),
            Point::new(-80.0, 10.0),
        ];

        let window = candidate_window(&ring, 360, 180).unwrap();

        assert!(window.columns.len() < 200, "{:?}", window.columns.len());
        assert!(window.columns.contains(&100));
        assert!(window.columns.contains(&260));
    }

    #[test]
    fn tri_support_inside_cell_away_from_center_is_required() {
        let input = mesh(&[vec![
            Point::new(-20.0, -10.0),
            Point::new(20.0, -10.0),
            Point::new(-10.0, 20.0),
        ]]);
        let levels = raster(36, 18, &[(18, 9, 2)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [2]);
        assert_eq!(coverage.covered_bin_count, 1);
    }

    #[test]
    fn regularized_targets_do_not_hide_a_deleted_hard_component() {
        let input = mesh(&[vec![
            Point::new(-20.0, -10.0),
            Point::new(20.0, -10.0),
            Point::new(-10.0, 20.0),
        ]]);
        let regularized = raster(36, 18, &[(18, 9, 1)]);
        let hard = raster(36, 18, &[(30, 9, 2)]);
        let intended = all_intended(&regularized);

        let (targets, coverage) =
            target_levels_with_hard_coverage(&input, 36, 18, &regularized, &hard, &intended)
                .unwrap();

        assert_eq!(targets, [1], "target comparison uses regularized demand");
        assert_eq!(coverage.active_bin_count, 1);
        assert_eq!(
            coverage.covered_bin_count, 0,
            "the missing hard component remains a quality failure"
        );
    }

    #[test]
    fn hex_support_inside_cell_away_from_all_corners_is_required() {
        let input = mesh(&[vec![
            Point::new(-20.0, -10.0),
            Point::new(-10.0, -20.0),
            Point::new(10.0, -20.0),
            Point::new(20.0, -10.0),
            Point::new(10.0, 20.0),
            Point::new(-10.0, 20.0),
        ]]);
        let levels = raster(36, 18, &[(18, 9, 3)]);

        let intended = all_intended(&levels);
        let (targets, _) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [3]);
    }

    #[test]
    fn disconnected_bins_do_not_alias_to_one_nearest_center() {
        let input = mesh(&[
            vec![
                Point::new(-5.0, -5.0),
                Point::new(15.0, -5.0),
                Point::new(15.0, 15.0),
                Point::new(-5.0, 15.0),
            ],
            vec![
                Point::new(15.0, -5.0),
                Point::new(35.0, -5.0),
                Point::new(35.0, 15.0),
                Point::new(15.0, 15.0),
            ],
        ]);
        let levels = raster(36, 18, &[(18, 9, 1), (20, 9, 3)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [1, 3]);
        assert_eq!(coverage.covered_bin_count, 2);
    }

    #[test]
    fn antimeridian_bin_is_counted_once() {
        let input = mesh(&[vec![
            Point::new(175.0, -5.0),
            Point::new(-175.0, -5.0),
            Point::new(-175.0, 5.0),
            Point::new(175.0, 5.0),
        ]]);
        let levels = raster(36, 18, &[(35, 9, 2)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [2]);
        assert_eq!(coverage.positive_overlap_count, 1);
    }

    #[test]
    fn north_and_south_polar_bins_are_supported() {
        let input = mesh(&[
            vec![
                Point::new(-20.0, 80.0),
                Point::new(20.0, 80.0),
                Point::new(0.0, 90.0),
            ],
            vec![
                Point::new(-20.0, -80.0),
                Point::new(0.0, -90.0),
                Point::new(20.0, -80.0),
            ],
        ]);
        let levels = raster(36, 18, &[(18, 17, 2), (18, 0, 3)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [2, 3]);
        assert_eq!(coverage.covered_bin_count, 2);
    }

    #[test]
    fn edge_contact_has_no_positive_support_area() {
        let input = mesh(&[vec![
            Point::new(-10.0, 0.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(-10.0, 10.0),
        ]]);
        let levels = raster(36, 18, &[(18, 9, 2)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [0]);
        assert_eq!(coverage.positive_overlap_count, 0);
    }

    #[test]
    fn numerical_near_touch_below_area_floor_has_no_support() {
        let input = mesh(&[vec![
            Point::new(-10.0, 0.0),
            Point::new(1.0e-14, 0.0),
            Point::new(1.0e-14, 10.0),
            Point::new(-10.0, 10.0),
        ]]);
        let levels = raster(36, 18, &[(18, 9, 2)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [0]);
        assert_eq!(coverage.positive_overlap_count, 0);
    }

    #[test]
    fn active_bin_in_an_output_hole_remains_uncovered() {
        let input = mesh(&[vec![
            Point::new(-40.0, -20.0),
            Point::new(-20.0, -20.0),
            Point::new(-20.0, 0.0),
            Point::new(-40.0, 0.0),
        ]]);
        let levels = raster(36, 18, &[(18, 9, 2)]);

        let intended = all_intended(&levels);
        let (targets, coverage) =
            target_levels_for_positive_support(&input, 36, 18, &levels, &intended).unwrap();

        assert_eq!(targets, [0]);
        assert_eq!(coverage.active_bin_count, 1);
        assert_eq!(coverage.covered_bin_count, 0);
    }

    #[test]
    #[ignore = "manual NXP81/720x360 quality-stage performance guard"]
    fn nxp81_scale_exact_support_finishes_under_sixty_seconds() {
        let mesh_nlon = 512;
        let mesh_nlat = 256;
        let mut rings = Vec::with_capacity(mesh_nlon * mesh_nlat);
        for jlat in 0..mesh_nlat {
            let south = -90.0 + 180.0 * jlat as f64 / mesh_nlat as f64;
            let north = -90.0 + 180.0 * (jlat + 1) as f64 / mesh_nlat as f64;
            for ilon in 0..mesh_nlon {
                let west = -180.0 + 360.0 * ilon as f64 / mesh_nlon as f64;
                let east = -180.0 + 360.0 * (ilon + 1) as f64 / mesh_nlon as f64;
                rings.push(vec![
                    Point::new(west, south),
                    Point::new(east, south),
                    Point::new(east, north),
                    Point::new(west, north),
                ]);
            }
        }
        let input = mesh(&rings);
        drop(rings);
        let levels = vec![1; 720 * 360];
        let intended = vec![true; levels.len()];
        let started = std::time::Instant::now();

        let (targets, coverage) =
            target_levels_for_positive_support(&input, 720, 360, &levels, &intended).unwrap();
        let elapsed = started.elapsed();
        let estimated_input_mib = (input.vertices.len() * std::mem::size_of::<Point>()
            + input.cells.len() * std::mem::size_of::<QualityCell>())
            / (1024 * 1024);
        eprintln!(
            "NXP81 support coverage: cells={} active_bins={} candidate_pairs={} convex_pairs={} generic_pairs={} positive_pairs={} covered_bins={} elapsed={elapsed:?} input_lower_bound={}MiB",
            input.cells.len(),
            coverage.active_bin_count,
            coverage.candidate_pair_count,
            coverage.convex_clip_pair_count,
            coverage.generic_clip_pair_count,
            coverage.positive_overlap_count,
            coverage.covered_bin_count,
            estimated_input_mib,
        );

        assert!(targets.iter().all(|target| *target == 1));
        assert_eq!(coverage.covered_bin_count, levels.len());
        assert!(
            coverage.candidate_pair_count < 5_000_000,
            "candidate broad phase regressed: {}",
            coverage.candidate_pair_count
        );
        assert!(
            elapsed < std::time::Duration::from_secs(60),
            "exact support coverage took {elapsed:?}"
        );
    }
}
