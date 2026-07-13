use std::io;

use earthmesh_mesh::LonLatDegrees;
use earthmesh_project::{GeometryIr, GeometryPrimitive};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InlineMaskSource {
    Bbox {
        west: f64,
        east: f64,
        south: f64,
        north: f64,
    },
    Circle {
        center: LonLatDegrees,
        radius_meters: f64,
    },
}

pub(crate) fn parse_inline_mask_source(prefix: &str) -> io::Result<Option<InlineMaskSource>> {
    let Some(ir) = GeometryIr::parse_inline_mask_source(prefix)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?
    else {
        return Ok(None);
    };
    match ir.primitive {
        GeometryPrimitive::Bbox {
            west,
            east,
            south,
            north,
        } => Ok(Some(InlineMaskSource::Bbox {
            west,
            east,
            south,
            north,
        })),
        GeometryPrimitive::Circle {
            lon,
            lat,
            radius_km,
        } => Ok(Some(InlineMaskSource::Circle {
            center: LonLatDegrees::new(lon, lat),
            radius_meters: radius_km * 1_000.0,
        })),
    }
}

pub(crate) fn method_c_calculated_region_level(
    mask_refine_degree: usize,
    max_level: usize,
) -> Option<usize> {
    if mask_refine_degree == 0 {
        Some(max_level)
    } else if mask_refine_degree <= max_level {
        Some(mask_refine_degree)
    } else {
        None
    }
}
