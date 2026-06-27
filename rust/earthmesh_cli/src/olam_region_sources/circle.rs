use std::io;
use std::path::Path;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::{LonLatDegrees, OlamRefinementRegion};

use super::shared::olam_calculated_region_level;
use crate::{
    parse_circle_mask_nml, read_circle_mask_netcdf, source_extension, unsupported_mask_source,
};

pub(crate) fn read_olam_circle_refinement_regions(
    source: &Path,
    refine: &RefineConfig,
    max_level: usize,
    nxp: usize,
    regions: &mut Vec<OlamRefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_circle_mask_nml(source, max_level)?,
        Some("nc") | Some("nc4") => {
            let mask = read_circle_mask_netcdf(source)?;
            if mask.refine_degree > max_level {
                None
            } else {
                Some(mask)
            }
        }
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    if mask.refine_degree == 0 {
        return Ok(());
    }
    let points = mask
        .points
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    let radius_meters = mask
        .radius_km
        .iter()
        .map(|radius_km| radius_km * 1_000.0)
        .collect::<Vec<_>>();
    push_olam_circle_or_corridor_region_with_parent_halos(
        regions,
        points,
        radius_meters,
        mask.refine_degree,
        refine,
        nxp,
    )?;
    Ok(())
}

pub(crate) fn push_olam_circle_or_corridor_region_with_parent_halos(
    regions: &mut Vec<OlamRefinementRegion>,
    points: Vec<LonLatDegrees>,
    radius_meters: Vec<f64>,
    level: usize,
    refine: &RefineConfig,
    nxp: usize,
) -> io::Result<()> {
    if nxp == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OLAM specified circle halo expansion requires positive NXP",
        ));
    }
    let base_spacing =
        std::f64::consts::PI * 2.0 * earthmesh_core::EARTH_RADIUS_METERS / (5.0 * nxp as f64);
    for parent_level in 1..level {
        let mut halo_meters = 0.0;
        for transition_level in parent_level..level {
            let halo_rows = refine.halo.get(transition_level).copied().unwrap_or(0).max(
                refine
                    .max_transition_row
                    .get(transition_level)
                    .copied()
                    .unwrap_or(0),
            );
            if halo_rows > 0 {
                halo_meters +=
                    halo_rows as f64 * base_spacing / 2.0_f64.powi((transition_level - 1) as i32);
            }
        }
        let expanded_radius_meters = radius_meters
            .iter()
            .map(|radius_meters| radius_meters + halo_meters)
            .collect::<Vec<_>>();
        push_olam_circle_or_corridor_region(
            regions,
            points.clone(),
            expanded_radius_meters,
            parent_level,
        );
    }
    push_olam_circle_or_corridor_region(regions, points, radius_meters, level);
    Ok(())
}

pub(crate) fn push_olam_circle_or_corridor_region(
    regions: &mut Vec<OlamRefinementRegion>,
    points: Vec<LonLatDegrees>,
    radius_meters: Vec<f64>,
    level: usize,
) {
    if points.len() == 1 && radius_meters.len() == 1 {
        regions.push(OlamRefinementRegion::Circle {
            center: points[0],
            radius_meters: radius_meters[0],
            level,
        });
    } else {
        regions.push(OlamRefinementRegion::Corridor {
            points,
            radius_meters,
            level,
        });
    }
}

pub(crate) fn merge_olam_method_c_regions_by_shape(regions: &mut Vec<OlamRefinementRegion>) {
    let mut merged = Vec::<OlamRefinementRegion>::with_capacity(regions.len());
    'next_region: for region in regions.drain(..) {
        match region {
            OlamRefinementRegion::Circle {
                center,
                radius_meters,
                level,
            } => {
                for existing in &mut merged {
                    let OlamRefinementRegion::Circle {
                        center: existing_center,
                        radius_meters: existing_radius,
                        level: existing_level,
                    } = existing
                    else {
                        continue;
                    };
                    if *existing_level == level
                        && existing_center.lon_degrees == center.lon_degrees
                        && existing_center.lat_degrees == center.lat_degrees
                    {
                        *existing_radius = existing_radius.max(radius_meters);
                        continue 'next_region;
                    }
                }
                merged.push(OlamRefinementRegion::Circle {
                    center,
                    radius_meters,
                    level,
                });
            }
            OlamRefinementRegion::Corridor {
                points,
                radius_meters,
                level,
            } => {
                for existing in &mut merged {
                    let OlamRefinementRegion::Corridor {
                        points: existing_points,
                        radius_meters: existing_radius,
                        level: existing_level,
                    } = existing
                    else {
                        continue;
                    };
                    if *existing_level == level
                        && *existing_points == points
                        && existing_radius.len() == radius_meters.len()
                    {
                        for (existing_radius, radius_meters) in
                            existing_radius.iter_mut().zip(radius_meters.iter())
                        {
                            *existing_radius = existing_radius.max(*radius_meters);
                        }
                        continue 'next_region;
                    }
                }
                merged.push(OlamRefinementRegion::Corridor {
                    points,
                    radius_meters,
                    level,
                });
            }
            other => merged.push(other),
        }
    }
    *regions = merged;
}

pub(crate) fn read_olam_calculated_circle_refinement_regions(
    source: &Path,
    max_level: usize,
    regions: &mut Vec<OlamRefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_circle_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_circle_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    let Some(level) = olam_calculated_region_level(mask.refine_degree, max_level) else {
        return Ok(());
    };
    let points = mask
        .points
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    let radius_meters = mask
        .radius_km
        .iter()
        .map(|radius_km| radius_km * 1_000.0)
        .collect::<Vec<_>>();
    push_olam_circle_or_corridor_region(regions, points, radius_meters, level);
    Ok(())
}
