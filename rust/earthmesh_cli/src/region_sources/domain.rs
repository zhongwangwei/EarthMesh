use std::io;
use std::path::Path;

use earthmesh_core::EarthmeshConfig;
use earthmesh_project::{CloseBoundaryGeometry, CloseBoundaryMode};

use super::close::{log_close_boundary_report, transform_close_mask_points};
use super::shared::{parse_inline_mask_source, InlineMaskSource};
use crate::{
    discover_mask_sources, parse_bbox_mask_nml, parse_circle_mask_nml, parse_close_mask_nml,
    read_bbox_mask_netcdf, read_circle_mask_netcdf, read_close_mask_netcdf, source_extension,
    unsupported_mask_source, GridRegion,
};

pub(crate) fn read_method_c_domain_region(
    config: &EarthmeshConfig,
) -> io::Result<Option<GridRegion>> {
    if config.mask_domain_global {
        return Ok(None);
    }
    let prefix = config.mask_domain_fprefix.trim();
    if prefix.is_empty() || prefix == "none" || prefix == "/tmp" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "regional Method-C direct path requires NL%mask_domain_fprefix",
        ));
    }
    if let Some(source) = parse_inline_mask_source(prefix)? {
        return match (config.mask_domain_type.trim(), source) {
            (
                "bbox",
                InlineMaskSource::Bbox {
                    west,
                    east,
                    south,
                    north,
                },
            ) => Ok(Some(GridRegion::Bbox {
                west,
                east,
                south,
                north,
            })),
            (
                "circle",
                InlineMaskSource::Circle {
                    center,
                    radius_meters,
                },
            ) => Ok(Some(GridRegion::Circle {
                lon: center.lon_degrees,
                lat: center.lat_degrees,
                radius_km: radius_meters / 1_000.0,
            })),
            (kind, _) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("inline regional domain source does not match mask_domain_type {kind}"),
            )),
        };
    }
    let discovery = discover_mask_sources(prefix)?;
    let close_boundary = CloseBoundaryMode::from_engine_spec(&config.mask_domain_close_boundary)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    let mut regions = Vec::new();
    for source in discovery.files {
        match config.mask_domain_type.trim() {
            "bbox" => read_method_c_bbox_domain_regions(&source, &mut regions)?,
            "circle" => read_method_c_circle_domain_regions(&source, &mut regions)?,
            "close" => read_method_c_close_domain_regions(&source, &close_boundary, &mut regions)?,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Method-C direct regional domain does not support {other} masks"),
                ));
            }
        }
    }
    match regions.len() {
        0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no domain mask sources matched {prefix}"),
        )),
        1 => Ok(regions.pop()),
        _ => Ok(Some(GridRegion::Any(regions))),
    }
}

pub(crate) fn read_method_c_bbox_domain_regions(
    source: &Path,
    regions: &mut Vec<GridRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_bbox_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_bbox_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    for point in mask.points {
        regions.push(GridRegion::Bbox {
            west: point.west,
            east: point.east,
            north: point.north,
            south: point.south,
        });
    }
    Ok(())
}

pub(crate) fn read_method_c_circle_domain_regions(
    source: &Path,
    regions: &mut Vec<GridRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_circle_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_circle_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    for (point, radius_km) in mask.points.into_iter().zip(mask.radius_km) {
        regions.push(GridRegion::Circle {
            lon: point.lon,
            lat: point.lat,
            radius_km,
        });
    }
    Ok(())
}

pub(crate) fn read_method_c_close_domain_regions(
    source: &Path,
    boundary: &CloseBoundaryMode,
    regions: &mut Vec<GridRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_close_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_close_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    if let Some(mask) = mask {
        let transformed = transform_close_mask_points(&mask.points, boundary)?;
        log_close_boundary_report(source, boundary, &transformed.report);
        match transformed.geometry {
            CloseBoundaryGeometry::Polygon(points) => regions.push(GridRegion::Close {
                points: points
                    .into_iter()
                    .map(|point| crate::LonLatPoint {
                        lon: point.lon,
                        lat: point.lat,
                    })
                    .collect(),
            }),
            CloseBoundaryGeometry::EnclosingCap { center, radius_km } => {
                regions.push(GridRegion::Circle {
                    lon: center.lon,
                    lat: center.lat,
                    radius_km,
                });
            }
        }
    }
    Ok(())
}
