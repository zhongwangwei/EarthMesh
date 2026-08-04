use std::io;

use earthmesh_mesh::LonLatDegrees;
use earthmesh_project::{GeometryIr, GeometryPrimitive};

#[derive(Clone, Debug, PartialEq)]
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
    /// A chain of circles, which is what reducing a coastline or river network
    /// to point+radius demand produces. `GeometryIr` carries one primitive, so
    /// this form is parsed here rather than through it.
    Circles(Vec<(LonLatDegrees, f64)>),
}

pub(crate) fn parse_inline_mask_source(prefix: &str) -> io::Result<Option<InlineMaskSource>> {
    if let Some(rest) = prefix.trim().strip_prefix("inline:circles:") {
        let mut circles = Vec::new();
        for member in rest.split(';').filter(|item| !item.trim().is_empty()) {
            let (mut lon, mut lat, mut radius_km) = (None, None, None);
            for pair in member.split(',') {
                let Some((key, value)) = pair.split_once('=') else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid inline circle key/value {pair}"),
                    ));
                };
                let parsed = value.trim().parse::<f64>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid inline circle number {value}"),
                    )
                })?;
                match key.trim() {
                    "lon" => lon = Some(parsed),
                    "lat" => lat = Some(parsed),
                    "radius_km" => radius_km = Some(parsed),
                    other => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("unsupported inline circle key {other}"),
                        ))
                    }
                }
            }
            match (lon, lat, radius_km) {
                (Some(lon), Some(lat), Some(radius_km))
                    if radius_km.is_finite()
                        && radius_km > 0.0
                        && lon.is_finite()
                        && lat.is_finite() =>
                {
                    circles.push((LonLatDegrees::new(lon, lat), radius_km * 1_000.0))
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("inline circle needs lon, lat and a positive radius_km: {member}"),
                    ))
                }
            }
        }
        if circles.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "inline circle chain must not be empty",
            ));
        }
        return Ok(Some(InlineMaskSource::Circles(circles)));
    }
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
