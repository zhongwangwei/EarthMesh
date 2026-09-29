use std::io;
use std::path::Path;

use earthmesh_mesh::LonLatDegrees;
use earthmesh_project::{GeometryIr, GeometryPrimitive};

#[derive(Clone, Debug, PartialEq)]
pub enum InlineMaskSource {
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

pub fn parse_inline_mask_source(prefix: &str) -> io::Result<Option<InlineMaskSource>> {
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

pub fn method_c_calculated_region_level(
    mask_refine_degree: usize,
    max_level: usize,
    zero_is_threshold_domain: bool,
) -> Option<usize> {
    if mask_refine_degree == 0 {
        (!zero_is_threshold_domain).then_some(max_level)
    } else if mask_refine_degree <= max_level {
        Some(mask_refine_degree)
    } else {
        None
    }
}

pub fn require_specified_region_level(
    source: &Path,
    requested: usize,
    max_level: usize,
) -> io::Result<()> {
    if requested > max_level {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "specified refinement source {} requested level {requested} exceeds max_iter_spc {max_level}",
                source.display()
            ),
        ));
    }
    Ok(())
}

/// How far a named region's parent at `parent_level` must reach beyond the
/// region itself at `level`: the transition rows of every level in between,
/// each as wide as that level's spacing.
///
/// `halo` / `max_transition_row` let a hand-written namelist declare the rows.
/// Both default to zero, which makes every parent the size of its child and
/// every chain fail -- a project's 1000 km circle at two levels stopped at
/// level 1 on a 200 km grid. An unset width takes the rows measured against
/// `spawn_nest` for the criteria ladder.
pub(crate) fn method_c_parent_halo_meters(
    refine: &earthmesh_core::RefineConfig,
    parent_level: usize,
    level: usize,
    base_spacing_meters: f64,
) -> f64 {
    (parent_level..level)
        .map(|transition_level| {
            let configured_rows = refine.halo.get(transition_level).copied().unwrap_or(0).max(
                refine
                    .max_transition_row
                    .get(transition_level)
                    .copied()
                    .unwrap_or(0),
            );
            let rows = if configured_rows > 0 {
                f64::from(configured_rows)
            } else {
                crate::refinement_demand::ladder::MEASURED_PARENT_HALO_ROWS
            };
            rows * base_spacing_meters / 2.0_f64.powi((transition_level - 1) as i32)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refinement_demand::ladder::MEASURED_PARENT_HALO_ROWS;
    use earthmesh_mesh::RefinementRegion;

    #[test]
    fn an_unset_parent_halo_takes_the_measured_rows() {
        let refine = earthmesh_core::RefineConfig::default();
        assert!(refine.halo.iter().all(|&rows| rows == 0));
        let spacing = 190_000.0;
        // One transition level between a level-2 region and its level-1 parent.
        assert_eq!(
            method_c_parent_halo_meters(&refine, 1, 2, spacing),
            MEASURED_PARENT_HALO_ROWS * spacing
        );
        // Two, the finer at half the spacing.
        assert_eq!(
            method_c_parent_halo_meters(&refine, 1, 3, spacing),
            MEASURED_PARENT_HALO_ROWS * spacing * 1.5
        );
        // A declared width is taken as declared.
        let mut declared = refine.clone();
        declared.halo[1] = 5;
        assert_eq!(
            method_c_parent_halo_meters(&declared, 1, 2, spacing),
            5.0 * spacing
        );
    }

    #[test]
    fn a_specified_circle_gets_a_wider_parent_without_declared_rows() {
        // With both namelist fields at their zero default the level-1 parent
        // was the circle itself, and Method-C could not nest level 2 in it.
        let mut regions = Vec::new();
        super::super::circle::push_method_c_circle_or_corridor_region_with_parent_halos(
            &mut regions,
            vec![LonLatDegrees::new(110.0, 30.0)],
            vec![1_000_000.0],
            2,
            &earthmesh_core::RefineConfig::default(),
            40,
        )
        .unwrap();
        let radius = |wanted: usize| {
            regions
                .iter()
                .find_map(|region| match region {
                    RefinementRegion::Circle {
                        radius_meters,
                        level,
                        ..
                    } if *level == wanted => Some(*radius_meters),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(radius(2), 1_000_000.0);
        assert!(radius(1) > radius(2) + 500_000.0, "{}", radius(1));
    }
}
