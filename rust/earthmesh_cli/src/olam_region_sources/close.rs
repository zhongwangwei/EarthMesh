use std::io;
use std::path::Path;

use earthmesh_mesh::{LonLatDegrees, OlamRefinementRegion};

use super::shared::olam_calculated_region_level;
use crate::{
    parse_close_mask_nml, read_close_mask_netcdf, source_extension, unsupported_mask_source,
    LonLatPoint,
};

pub(crate) fn read_olam_close_refinement_regions(
    source: &Path,
    max_level: usize,
    regions: &mut Vec<OlamRefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_close_mask_nml(source, max_level)?,
        Some("nc") | Some("nc4") => {
            let mask = read_close_mask_netcdf(source)?;
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
    regions.push(OlamRefinementRegion::Polygon {
        points: olam_close_mask_points_for_fortran_ngrdll(&mask.points),
        level: mask.refine_degree,
    });
    Ok(())
}

pub(crate) fn read_olam_calculated_close_refinement_regions(
    source: &Path,
    max_level: usize,
    regions: &mut Vec<OlamRefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_close_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_close_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    let Some(level) = olam_calculated_region_level(mask.refine_degree, max_level) else {
        return Ok(());
    };
    regions.push(OlamRefinementRegion::Polygon {
        points: olam_close_mask_points_for_fortran_ngrdll(&mask.points),
        level,
    });
    Ok(())
}

pub(crate) fn olam_close_mask_points_for_fortran_ngrdll(
    points: &[LonLatPoint],
) -> Vec<LonLatDegrees> {
    let mut converted = points
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    if converted.len() >= 3 && converted.first() != converted.last() {
        if let Some(first) = converted.first().cloned() {
            converted.push(first);
        }
    }
    converted
}
