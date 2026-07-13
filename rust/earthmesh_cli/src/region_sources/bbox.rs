use std::io;
use std::path::Path;

use earthmesh_mesh::MethodCRefinementRegion;

use super::shared::method_c_calculated_region_level;
use crate::{
    parse_bbox_mask_nml, read_bbox_mask_netcdf, source_extension, unsupported_mask_source,
};

pub(crate) fn read_method_c_bbox_refinement_regions(
    source: &Path,
    max_level: usize,
    regions: &mut Vec<MethodCRefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_bbox_mask_nml(source, max_level)?,
        Some("nc") | Some("nc4") => {
            let mask = read_bbox_mask_netcdf(source)?;
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
    for point in &mask.points {
        regions.push(MethodCRefinementRegion::Bbox {
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
    regions: &mut Vec<MethodCRefinementRegion>,
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
        regions.push(MethodCRefinementRegion::Bbox {
            west_degrees: point.west,
            east_degrees: point.east,
            south_degrees: point.south,
            north_degrees: point.north,
            level,
        });
    }
    Ok(())
}
