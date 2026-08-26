use std::io;
use std::path::Path;

use earthmesh_mesh::RefinementRegion;

use super::shared::{method_c_calculated_region_level, require_specified_region_level};
use crate::{
    parse_bbox_mask_nml, read_bbox_mask_netcdf, source_extension, unsupported_mask_source,
};

pub(crate) fn read_method_c_bbox_refinement_regions(
    source: &Path,
    max_level: usize,
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
    require_specified_region_level(source, mask.refine_degree, max_level)?;
    if mask.refine_degree == 0 {
        return Ok(());
    }
    for point in &mask.points {
        regions.push(RefinementRegion::Bbox {
            west_degrees: point.west,
            east_degrees: point.east,
            south_degrees: point.south,
            north_degrees: point.north,
            level: mask.refine_degree,
        });
    }
    Ok(())
}

pub(crate) fn read_method_c_calculated_bbox_refinement_regions(
    source: &Path,
    max_level: usize,
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
    let Some(level) = method_c_calculated_region_level(mask.refine_degree, max_level) else {
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
