use std::io;
use std::path::Path;

use earthmesh_core::EarthmeshConfig;

use crate::{
    discover_mask_sources, parse_bbox_mask_nml, parse_circle_mask_nml, parse_close_mask_nml,
    read_bbox_mask_netcdf, read_circle_mask_netcdf, read_close_mask_netcdf, source_extension,
    unsupported_mask_source, GridRegion,
};

pub(crate) fn read_olam_domain_region(config: &EarthmeshConfig) -> io::Result<Option<GridRegion>> {
    if config.mask_domain_global {
        return Ok(None);
    }
    let prefix = config.mask_domain_fprefix.trim();
    if prefix.is_empty() || prefix == "none" || prefix == "/tmp" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "regional OLAM direct path requires NL%mask_domain_fprefix",
        ));
    }
    let discovery = discover_mask_sources(prefix)?;
    let mut regions = Vec::new();
    for source in discovery.files {
        match config.mask_domain_type.trim() {
            "bbox" => read_olam_bbox_domain_regions(&source, &mut regions)?,
            "circle" => read_olam_circle_domain_regions(&source, &mut regions)?,
            "close" => read_olam_close_domain_regions(&source, &mut regions)?,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("OLAM direct regional domain does not support {other} masks"),
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

pub(crate) fn read_olam_bbox_domain_regions(
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

pub(crate) fn read_olam_circle_domain_regions(
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

pub(crate) fn read_olam_close_domain_regions(
    source: &Path,
    regions: &mut Vec<GridRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_close_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_close_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    if let Some(mask) = mask {
        regions.push(GridRegion::Close {
            points: mask.points,
        });
    }
    Ok(())
}
