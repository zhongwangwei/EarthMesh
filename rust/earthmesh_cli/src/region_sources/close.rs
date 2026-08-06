use std::io;
use std::path::Path;

use earthmesh_mesh::{LonLatDegrees, RefinementRegion};
use earthmesh_project::{
    transform_close_boundary, CloseBoundaryGeometry, CloseBoundaryMode, CloseBoundaryReport,
    GeometryPoint,
};

use super::shared::method_c_calculated_region_level;
use crate::{
    parse_close_mask_nml, read_close_mask_netcdf, source_extension, unsupported_mask_source,
    LonLatPoint,
};

pub(crate) fn read_method_c_close_refinement_regions(
    source: &Path,
    max_level: usize,
    boundary: &CloseBoundaryMode,
    regions: &mut Vec<RefinementRegion>,
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
    let transformed = transform_close_mask_points(&mask.points, boundary)?;
    log_close_boundary_report(source, boundary, &transformed.report);
    match transformed.geometry {
        CloseBoundaryGeometry::Polygon(points) => {
            regions.push(RefinementRegion::Polygon {
                points: method_c_geometry_points_for_canonical_ngrdll(&points),
                level: mask.refine_degree,
            });
        }
        CloseBoundaryGeometry::EnclosingCap { center, radius_km } => {
            regions.push(RefinementRegion::Circle {
                center: LonLatDegrees::new(center.lon, center.lat),
                radius_meters: radius_km * 1_000.0,
                level: mask.refine_degree,
            });
        }
    }
    Ok(())
}

pub(crate) fn read_method_c_calculated_close_refinement_regions(
    source: &Path,
    max_level: usize,
    regions: &mut Vec<RefinementRegion>,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_close_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_close_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    let Some(level) = method_c_calculated_region_level(mask.refine_degree, max_level) else {
        return Ok(());
    };
    regions.push(RefinementRegion::Polygon {
        points: method_c_close_mask_points_for_canonical_ngrdll(&mask.points),
        level,
    });
    Ok(())
}

pub(crate) fn method_c_close_mask_points_for_canonical_ngrdll(
    points: &[LonLatPoint],
) -> Vec<LonLatDegrees> {
    let points = points
        .iter()
        .map(|point| GeometryPoint::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    method_c_geometry_points_for_canonical_ngrdll(&points)
}

pub(crate) fn transform_close_mask_points(
    points: &[LonLatPoint],
    boundary: &CloseBoundaryMode,
) -> io::Result<earthmesh_project::CloseBoundaryTransform> {
    let points = points
        .iter()
        .map(|point| GeometryPoint::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    transform_close_boundary(&points, boundary)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))
}

pub(crate) fn log_close_boundary_report(
    source: &Path,
    boundary: &CloseBoundaryMode,
    report: &CloseBoundaryReport,
) {
    if matches!(boundary, CloseBoundaryMode::Polyline) {
        return;
    }
    eprintln!(
        "earthmesh_cli: close boundary {} mode={} points {}→{} area {:.3}→{:.3} km² delta={:.3} km²{}{}",
        source.display(),
        boundary.to_engine_spec(),
        report.input_points,
        report.output_points,
        report.input_area_km2,
        report.output_area_km2,
        report.area_delta_km2,
        report
            .max_vertex_displacement_km
            .map(|value| format!(" max_vertex_displacement={value:.3} km"))
            .unwrap_or_default(),
        report
            .radius_km
            .map(|value| format!(" radius={value:.3} km"))
            .unwrap_or_default(),
    );
}

fn method_c_geometry_points_for_canonical_ngrdll(points: &[GeometryPoint]) -> Vec<LonLatDegrees> {
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
