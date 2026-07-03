//! `&hfield` namelist group: compose the OLAM specified-refine regions into a
//! continuous cell-width field (`earthmesh_hfield`) and drive Method-C from
//! quantized target levels instead of per-region geometry.
//!
//! Namelist keys (all inside a `&hfield ... /` group, `NL%` prefix like every
//! other group):
//!   hfield_on         .true./.false.  master switch (block absent == off)
//!   hfield_g          gradation limit |∇h| <= g          (default 0.2)
//!   hfield_max_level  quantization depth, 1..=5; 0 = use the run's max level
//!   hfield_base_m     background cell size in meters; 0/absent = 2πR/(5·NXP)
//!   hfield_nlon/nlat  field raster size                  (default 720 x 360)

use std::io;

use earthmesh_hfield::{HField, HRegion};
use earthmesh_mesh::OlamRefinementRegion;

use crate::olam_native_parser::olam_namelist_assignments;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HfieldRefineOptions {
    pub g: f64,
    /// `None` = follow the run's computed max refinement level.
    pub max_level: Option<usize>,
    /// `None` = derive from NXP (`2πR / (5·NXP)`).
    pub base_m: Option<f64>,
    pub nlon: usize,
    pub nlat: usize,
}

fn invalid(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

fn parse_hfield_f64(field: &str, value: &str) -> io::Result<f64> {
    let cleaned = value
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .replace(['d', 'D'], "e");
    cleaned
        .parse::<f64>()
        .map_err(|_| invalid(format!("&hfield {field} value '{value}' is not a number")))
}

fn parse_hfield_usize(field: &str, value: &str) -> io::Result<usize> {
    let cleaned = value.trim().trim_matches('\'').trim_matches('"');
    cleaned.parse::<usize>().map_err(|_| {
        invalid(format!(
            "&hfield {field} value '{value}' is not a non-negative integer"
        ))
    })
}

fn parse_hfield_bool(field: &str, value: &str) -> io::Result<bool> {
    let cleaned = value
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .trim_matches('.')
        .to_ascii_lowercase();
    match cleaned.as_str() {
        "true" | "t" => Ok(true),
        "false" | "f" => Ok(false),
        _ => Err(invalid(format!(
            "&hfield {field} value '{value}' is not a Fortran logical"
        ))),
    }
}

/// Read the `&hfield` group. Absent group or `hfield_on = .false.` yields
/// `Ok(None)` (feature off).
pub fn read_hfield_refine_options(contents: &str) -> io::Result<Option<HfieldRefineOptions>> {
    if !contents.to_ascii_lowercase().contains("&hfield") {
        return Ok(None);
    }
    let mut enabled = true;
    let mut g = 0.2_f64;
    let mut max_level = 0usize;
    let mut base_m = 0.0_f64;
    let mut nlon = 720usize;
    let mut nlat = 360usize;
    for assignment in olam_namelist_assignments(contents, "hfield")? {
        match assignment.field.as_str() {
            "hfield_on" => enabled = parse_hfield_bool(&assignment.field, &assignment.value)?,
            "hfield_g" => g = parse_hfield_f64(&assignment.field, &assignment.value)?,
            "hfield_max_level" => {
                max_level = parse_hfield_usize(&assignment.field, &assignment.value)?
            }
            "hfield_base_m" => base_m = parse_hfield_f64(&assignment.field, &assignment.value)?,
            "hfield_nlon" => nlon = parse_hfield_usize(&assignment.field, &assignment.value)?,
            "hfield_nlat" => nlat = parse_hfield_usize(&assignment.field, &assignment.value)?,
            _ => {}
        }
    }
    if !enabled {
        return Ok(None);
    }
    if !g.is_finite() || g <= 0.0 {
        return Err(invalid(format!(
            "hfield_g must be positive and finite, got {g}"
        )));
    }
    if max_level > 5 {
        return Err(invalid(format!(
            "hfield_max_level must be in 0..=5 (0 = auto), got {max_level}"
        )));
    }
    if !base_m.is_finite() || base_m < 0.0 {
        return Err(invalid(format!(
            "hfield_base_m must be non-negative and finite, got {base_m}"
        )));
    }
    if nlon < 4 || nlat < 2 {
        return Err(invalid(format!(
            "hfield raster {nlon}x{nlat} too small (need >= 4x2)"
        )));
    }
    Ok(Some(HfieldRefineOptions {
        g,
        max_level: if max_level == 0 {
            None
        } else {
            Some(max_level)
        },
        base_m: if base_m > 0.0 { Some(base_m) } else { None },
        nlon,
        nlat,
    }))
}

/// Compose the specified-refine regions into a gradient-limited cell-width
/// field: each level-L region pins `h = base / 2^L` inside its footprint, the
/// pointwise minimum wins on overlap, and `limit_gradient(g)` builds the
/// slope-g transition skirts that make nested level sets legal by construction.
pub fn build_hfield_from_regions(
    regions: &[OlamRefinementRegion],
    base_m: f64,
    g: f64,
    nlon: usize,
    nlat: usize,
) -> io::Result<HField> {
    if !base_m.is_finite() || base_m <= 0.0 {
        return Err(invalid(format!(
            "h-field base cell size must be positive, got {base_m}"
        )));
    }
    let mut field = HField::uniform(nlon, nlat, base_m)?;
    for region in regions {
        let level = region.level().min(5) as i32;
        if level < 1 {
            continue;
        }
        let h_inside = base_m / 2f64.powi(level);
        let hregion = match region {
            OlamRefinementRegion::Circle {
                center,
                radius_meters,
                ..
            } => HRegion::Circle {
                lon: center.lon_degrees,
                lat: center.lat_degrees,
                radius_m: *radius_meters,
            },
            OlamRefinementRegion::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => HRegion::Bbox {
                west: *west_degrees,
                east: *east_degrees,
                south: *south_degrees,
                north: *north_degrees,
            },
            OlamRefinementRegion::Corridor {
                points,
                radius_meters,
                ..
            } => {
                let radius_m = radius_meters.iter().cloned().fold(0.0_f64, f64::max);
                if points.is_empty() || radius_m <= 0.0 {
                    continue;
                }
                HRegion::Corridor {
                    points: points
                        .iter()
                        .map(|p| (p.lon_degrees, p.lat_degrees))
                        .collect(),
                    radius_m,
                }
            }
            OlamRefinementRegion::Polygon { points, .. } => HRegion::Polygon {
                points: points
                    .iter()
                    .map(|p| (p.lon_degrees, p.lat_degrees))
                    .collect(),
            },
        };
        field.min_with_region(&hregion, h_inside)?;
    }
    field.limit_gradient(g)?;
    Ok(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_mesh::LonLatDegrees;

    #[test]
    fn absent_group_and_disabled_group_read_as_none() {
        assert!(read_hfield_refine_options("&mkgrd\n NL%nxp = 6\n/\n")
            .unwrap()
            .is_none());
        let off = "&hfield\n NL%hfield_on = .false.\n NL%hfield_g = 0.3\n/\n";
        assert!(read_hfield_refine_options(off).unwrap().is_none());
    }

    #[test]
    fn present_group_parses_values_and_defaults() {
        let text =
            "&hfield\n NL%hfield_on = .true.\n NL%hfield_g = 0.15\n NL%hfield_max_level = 3\n/\n";
        let options = read_hfield_refine_options(text).unwrap().unwrap();
        assert!((options.g - 0.15).abs() < 1e-12);
        assert_eq!(options.max_level, Some(3));
        assert_eq!(options.base_m, None);
        assert_eq!((options.nlon, options.nlat), (720, 360));

        let bad = "&hfield\n NL%hfield_g = -1.0\n/\n";
        assert!(read_hfield_refine_options(bad).is_err());
    }

    #[test]
    fn regions_pin_levels_and_field_is_graded() {
        let regions = [OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 500_000.0,
            level: 2,
        }];
        let base = 100_000.0;
        let field = build_hfield_from_regions(&regions, base, 0.2, 360, 180).unwrap();
        let center = field.sample(115.0, 25.0);
        assert!(
            (center - base / 4.0).abs() < 1.0,
            "level-2 center should pin base/4, got {center}"
        );
        let far = field.sample(-65.0, -25.0);
        assert!((far - base).abs() < 1.0, "far field keeps base, got {far}");
        assert_eq!(field.level_at(115.0, 25.0, base, 5), 2);
    }
}
