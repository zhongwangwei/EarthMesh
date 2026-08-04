use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Minimal project-to-engine geometry IR.
///
/// It preserves the exact shape payload at the project boundary, then emits the
/// compatibility inline mask string only at the engine adapter edge.
///
/// ```
/// use earthmesh_project::GeometryIr;
///
/// let geom = GeometryIr::bbox(112.0, 115.5, 21.5, 23.5);
/// assert_eq!(geom.to_inline_mask_source().unwrap(), "inline:bbox:w=112,e=115.5,s=21.5,n=23.5");
/// assert_eq!(geom.regions[0].marker, 1);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryIr {
    pub primitive: GeometryPrimitive,
    #[serde(default)]
    pub points: Vec<GeometryPoint>,
    #[serde(default)]
    pub segments: Vec<GeometrySegment>,
    #[serde(default)]
    pub regions: Vec<GeometryRegion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GeometryPrimitive {
    Bbox {
        west: f64,
        east: f64,
        south: f64,
        north: f64,
    },
    Circle {
        lon: f64,
        lat: f64,
        radius_km: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryPoint {
    pub lon: f64,
    pub lat: f64,
}

impl GeometryPoint {
    pub const fn new(lon: f64, lat: f64) -> Self {
        Self { lon, lat }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeometrySegment {
    pub start: usize,
    pub end: usize,
    pub marker: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryRegion {
    pub pos: GeometryPoint,
    pub marker: i32,
}

impl GeometryIr {
    pub fn bbox(west: f64, east: f64, south: f64, north: f64) -> Self {
        let center_lon = {
            let raw_span = east - west;
            let eastward_span = if raw_span.abs() >= 360.0 - 1.0e-12 {
                360.0
            } else {
                raw_span.rem_euclid(360.0)
            };
            let center = west + eastward_span * 0.5;
            let wrapped = (center + 180.0).rem_euclid(360.0) - 180.0;
            if wrapped == -180.0 {
                180.0
            } else {
                wrapped
            }
        };
        let points = vec![
            GeometryPoint::new(west, south),
            GeometryPoint::new(east, south),
            GeometryPoint::new(east, north),
            GeometryPoint::new(west, north),
        ];
        Self {
            primitive: GeometryPrimitive::Bbox {
                west,
                east,
                south,
                north,
            },
            points,
            segments: closed_segments(4),
            regions: vec![GeometryRegion {
                pos: GeometryPoint::new(center_lon, (south + north) * 0.5),
                marker: 1,
            }],
        }
    }

    pub fn circle(lon: f64, lat: f64, radius_km: f64) -> Self {
        Self {
            primitive: GeometryPrimitive::Circle {
                lon,
                lat,
                radius_km,
            },
            points: vec![GeometryPoint::new(lon, lat)],
            segments: Vec::new(),
            regions: vec![GeometryRegion {
                pos: GeometryPoint::new(lon, lat),
                marker: 1,
            }],
        }
    }

    pub fn to_inline_mask_source(&self) -> Result<String, String> {
        match self.primitive {
            GeometryPrimitive::Bbox {
                west,
                east,
                south,
                north,
            } => Ok(format!("inline:bbox:w={west},e={east},s={south},n={north}")),
            GeometryPrimitive::Circle {
                lon,
                lat,
                radius_km,
            } => Ok(format!(
                "inline:circle:lon={lon},lat={lat},radius_km={radius_km}"
            )),
        }
    }

    /// Inline source for a chain of circles.
    ///
    /// `GeometryIr` carries one primitive, and a coastline needs many, so the
    /// chain is emitted as its own `inline:circles:` form with semicolon
    /// separated members rather than by widening the single-primitive IR.
    pub fn circles_inline_mask_source(
        circles: impl IntoIterator<Item = (f64, f64, f64)>,
    ) -> Result<String, String> {
        let members: Vec<String> = circles
            .into_iter()
            .map(|(lon, lat, radius_km)| {
                if !radius_km.is_finite() || radius_km <= 0.0 {
                    return Err(format!(
                        "circle radius_km must be positive, got {radius_km}"
                    ));
                }
                if !lon.is_finite() || !lat.is_finite() {
                    return Err("circle lon/lat must be finite".to_string());
                }
                Ok(format!("lon={lon},lat={lat},radius_km={radius_km}"))
            })
            .collect::<Result<_, _>>()?;
        if members.is_empty() {
            return Err("circle chain must not be empty".to_string());
        }
        Ok(format!("inline:circles:{}", members.join(";")))
    }

    pub fn parse_inline_mask_source(prefix: &str) -> Result<Option<Self>, String> {
        let Some(rest) = prefix.trim().strip_prefix("inline:") else {
            return Ok(None);
        };
        let Some((kind, values)) = rest.split_once(':') else {
            return Err(format!("invalid inline mask source {prefix}"));
        };
        let values = parse_inline_values(values)?;
        match kind {
            "bbox" => Ok(Some(Self::bbox(
                inline_f64(&values, "w")?,
                inline_f64(&values, "e")?,
                inline_f64(&values, "s")?,
                inline_f64(&values, "n")?,
            ))),
            "circle" => Ok(Some(Self::circle(
                inline_f64(&values, "lon")?,
                inline_f64(&values, "lat")?,
                inline_f64(&values, "radius_km")?,
            ))),
            other => Err(format!("unsupported inline mask source type {other}")),
        }
    }
}

fn closed_segments(len: usize) -> Vec<GeometrySegment> {
    if len < 2 {
        return Vec::new();
    }
    (0..len)
        .map(|start| GeometrySegment {
            start,
            end: (start + 1) % len,
            marker: 1,
        })
        .collect()
}

fn parse_inline_values(values: &str) -> Result<BTreeMap<&str, &str>, String> {
    let mut parsed = BTreeMap::new();
    for item in values.split(',') {
        let Some((key, value)) = item.split_once('=') else {
            return Err(format!("invalid inline mask key/value {item}"));
        };
        parsed.insert(key.trim(), value.trim());
    }
    Ok(parsed)
}

fn inline_f64(values: &BTreeMap<&str, &str>, key: &str) -> Result<f64, String> {
    let value = values
        .get(key)
        .ok_or_else(|| format!("inline mask source missing {key}"))?;
    value
        .parse::<f64>()
        .map_err(|err| format!("invalid inline mask {key}={value}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_ir_has_region_and_segments() {
        let ir = GeometryIr::bbox(112.0, 115.5, 21.5, 23.5);
        assert_eq!(ir.points.len(), 4);
        assert_eq!(ir.segments.len(), 4);
        assert_eq!(ir.regions[0].marker, 1);
        assert_eq!(
            ir.to_inline_mask_source().unwrap(),
            "inline:bbox:w=112,e=115.5,s=21.5,n=23.5"
        );
    }

    #[test]
    fn bbox_ir_places_antimeridian_seed_inside_directed_span() {
        let ir = GeometryIr::bbox(170.0, -170.0, -10.0, 10.0);
        assert_eq!(ir.regions[0].pos, GeometryPoint::new(180.0, 0.0));

        let wide = GeometryIr::bbox(-170.0, 170.0, -10.0, 10.0);
        assert_eq!(wide.regions[0].pos, GeometryPoint::new(0.0, 0.0));

        let global = GeometryIr::bbox(-180.0, 180.0, -10.0, 10.0);
        assert_eq!(global.regions[0].pos, GeometryPoint::new(0.0, 0.0));
    }

    #[test]
    fn inline_parser_round_trips_circle() {
        let ir =
            GeometryIr::parse_inline_mask_source("inline:circle:lon=113.5,lat=22.25,radius_km=80")
                .unwrap()
                .unwrap();
        assert_eq!(ir, GeometryIr::circle(113.5, 22.25, 80.0));
    }
}
