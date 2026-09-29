use std::io;
use std::path::Path;

use earthmesh_mesh::{LonLatDegrees, RefinementRegion};
use earthmesh_project::{
    transform_close_boundary, CloseBoundaryGeometry, CloseBoundaryMode, CloseBoundaryReport,
    GeometryPoint,
};

use super::shared::{method_c_calculated_region_level, require_specified_region_level};
use crate::{
    parse_close_mask_nml, read_close_mask_netcdf, source_extension, unsupported_mask_source,
    LonLatPoint,
};

pub fn read_method_c_close_refinement_regions(
    source: &Path,
    max_level: usize,
    boundary: &CloseBoundaryMode,
    regions: &mut Vec<RefinementRegion>,
    refine: &earthmesh_core::RefineConfig,
    nxp: usize,
    apply_parent_halos: bool,
) -> io::Result<()> {
    let mask = match source_extension(source).as_deref() {
        Some("nml") => parse_close_mask_nml(source, usize::MAX)?,
        Some("nc") | Some("nc4") => Some(read_close_mask_netcdf(source)?),
        _ => return Err(unsupported_mask_source(source)),
    };
    let Some(mask) = mask else {
        return Ok(());
    };
    require_specified_region_level(source, mask.refine_degree, max_level)?;
    if mask.refine_degree == 0 {
        return Ok(());
    }
    let transformed = transform_close_mask_points(&mask.points, boundary)?;
    log_close_boundary_report(source, boundary, &transformed.report);
    match transformed.geometry {
        CloseBoundaryGeometry::Polygon(points) => {
            let points = method_c_geometry_points_for_canonical_ngrdll(&points);
            // The h-field route settles every level from one field and wants
            // the curve as given; only the nesting route needs the parents.
            if apply_parent_halos {
                push_close_polygon_region_with_parent_halos(
                    regions,
                    points,
                    mask.refine_degree,
                    refine,
                    nxp,
                )?;
            } else {
                regions.push(RefinementRegion::Polygon {
                    points,
                    level: mask.refine_degree,
                });
            }
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

pub fn read_method_c_calculated_close_refinement_regions(
    source: &Path,
    max_level: usize,
    zero_is_threshold_domain: bool,
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
    let Some(level) =
        method_c_calculated_region_level(mask.refine_degree, max_level, zero_is_threshold_domain)
    else {
        return Ok(());
    };
    regions.push(RefinementRegion::Polygon {
        points: method_c_close_mask_points_for_canonical_ngrdll(&mask.points),
        level,
    });
    Ok(())
}

pub fn method_c_close_mask_points_for_canonical_ngrdll(
    points: &[LonLatPoint],
) -> Vec<LonLatDegrees> {
    let points = points
        .iter()
        .map(|point| GeometryPoint::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    method_c_geometry_points_for_canonical_ngrdll(&points)
}

pub fn transform_close_mask_points(
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

pub fn log_close_boundary_report(
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

/// Emit a closed curve at its own level and at every level above it, each one
/// grown outward by the transition band that level needs.
///
/// The counterpart of `push_method_c_circle_or_corridor_region_with_parent_halos`,
/// which a circle has had all along and a closed curve had not. Method-C nests:
/// a level-2 region must sit inside a level-1 one, or its perimeter has no
/// ground to transition through, and `method_c_spawn_internal` refuses the pass
/// by name -- `pass 2 polygon regions require explicit parent-level halo`.
/// Measured before this: the same mask that red-green served
/// stopped Method-C, which is the default backend.
///
/// # Growing a ring
///
/// A circle grows by adding to its radius. A ring grows by moving each vertex
/// outward from the ring's centroid -- the same operation, and it works while
/// the ring is star-shaped about that centroid. When it is not, radial growth
/// is not growth: a ray from the centroid leaves and re-enters the ring, and
/// the "parent" comes back not containing the child.
///
/// Measured, and it is why the check below is on the property rather than on a
/// proxy for it. A concave L runs fine, because it is still star-shaped. A deep
/// C, whose centroid sits in its own mouth, produced a parent that failed far
/// downstream with `Current nested grid crosses the parent boundary at M point
/// 2795` -- a perimeter Method-C could not walk, reported nowhere near the
/// place that built it.
///
/// So the grown ring is checked for what a parent has to do: contain every
/// vertex of the child. A ring that fails gets no parent, and the pass refusal
/// downstream still names what is missing -- which is a worse message but an
/// honest one, in the right place.
fn push_close_polygon_region_with_parent_halos(
    regions: &mut Vec<RefinementRegion>,
    points: Vec<LonLatDegrees>,
    level: usize,
    refine: &earthmesh_core::RefineConfig,
    nxp: usize,
) -> io::Result<()> {
    if nxp == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C close polygon halo expansion requires positive NXP",
        ));
    }
    let base_spacing =
        std::f64::consts::PI * 2.0 * earthmesh_core::EARTH_RADIUS_METERS / (5.0 * nxp as f64);
    for parent_level in 1..level {
        let halo_meters =
            super::shared::method_c_parent_halo_meters(refine, parent_level, level, base_spacing);
        match grown_ring(&points, halo_meters) {
            Some(grown) => regions.push(RefinementRegion::Polygon {
                points: grown,
                level: parent_level,
            }),
            // A ring this cannot grow honestly gets no parent, and the pass
            // refusal downstream still names what is missing. Better than a
            // parent that folds over itself.
            None => break,
        }
    }
    regions.push(RefinementRegion::Polygon { points, level });
    Ok(())
}

/// Move every vertex outward from the ring's centroid by `halo_meters`.
///
/// `None` when the result would not be a simple ring: a vertex at the centroid
/// has no outward direction, and a ring wider than a quarter of the sphere has
/// no centroid worth measuring from.
fn grown_ring(points: &[LonLatDegrees], halo_meters: f64) -> Option<Vec<LonLatDegrees>> {
    if points.len() < 3 || !halo_meters.is_finite() || halo_meters <= 0.0 {
        return None;
    }
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let unit: Vec<[f64; 3]> = points
        .iter()
        .map(|point| {
            let p = earthmesh_mesh::lonlat_degrees_to_unit_xyz(*point);
            [p.x, p.y, p.z]
        })
        .collect();
    let mut centroid = [0.0_f64; 3];
    for p in &unit {
        centroid[0] += p[0];
        centroid[1] += p[1];
        centroid[2] += p[2];
    }
    let length = (centroid[0].powi(2) + centroid[1].powi(2) + centroid[2].powi(2)).sqrt();
    if length <= 1.0e-9 {
        return None;
    }
    let centroid = [
        centroid[0] / length,
        centroid[1] / length,
        centroid[2] / length,
    ];
    let mut grown = Vec::with_capacity(points.len());
    for p in &unit {
        let dot = (centroid[0] * p[0] + centroid[1] * p[1] + centroid[2] * p[2]).clamp(-1.0, 1.0);
        let arc = dot.acos();
        // A vertex on the centroid has no direction to grow along, and one more
        // than a quarter-sphere away means the centroid is not inside the ring.
        if arc <= 1.0e-9 || arc > std::f64::consts::FRAC_PI_2 {
            return None;
        }
        let grown_arc = arc + halo_meters / radius;
        if grown_arc >= std::f64::consts::FRAC_PI_2 {
            return None;
        }
        // Slide along the great circle from the centroid through this vertex.
        let tangent: Vec<f64> = (0..3).map(|i| p[i] - centroid[i] * dot).collect();
        let tangent_length = (tangent[0].powi(2) + tangent[1].powi(2) + tangent[2].powi(2)).sqrt();
        if tangent_length <= 1.0e-12 {
            return None;
        }
        let moved = earthmesh_mesh::CartesianPoint::new(
            centroid[0] * grown_arc.cos() + tangent[0] / tangent_length * grown_arc.sin(),
            centroid[1] * grown_arc.cos() + tangent[1] / tangent_length * grown_arc.sin(),
            centroid[2] * grown_arc.cos() + tangent[2] / tangent_length * grown_arc.sin(),
        );
        grown.push(earthmesh_mesh::xyz_to_lonlat_degrees(moved));
    }

    // The property, not a proxy for it: "centroid is inside" does not imply
    // star-shaped -- a spiral has its centroid inside and still fails -- and
    // star-shapedness is only the reason this works, not the thing needed.
    // What is needed is that the parent contains the child.
    let parent = crate::boundary_model::boundary_model_from_regions(&[RefinementRegion::Polygon {
        points: grown.clone(),
        level: 1,
    }]);
    if parent.loops.is_empty() {
        return None;
    }
    // Asked vertex by vertex of the boundary model, this is quadratic: a
    // 130,000-vertex basin outline spent hours here. Every vertex of both
    // rings is within a quarter-sphere of `centroid` (checked above), and in
    // the gnomonic projection about it great circles are straight lines, so
    // the planar ring is the spherical one and a banded ray cast answers the
    // same question in near-linear time.
    let grown_unit: Vec<[f64; 3]> = grown
        .iter()
        .map(|point| {
            let p = earthmesh_mesh::lonlat_degrees_to_unit_xyz(*point);
            [p.x, p.y, p.z]
        })
        .collect();
    if !gnomonic_ring_holds_all(&grown_unit, &unit, centroid) {
        return None;
    }
    Some(grown)
}

/// Whether the ring `parent` holds every point of `inside`, all of them
/// within a quarter-sphere of `centre`: even-odd ray casting in the gnomonic
/// projection about `centre`, with the edges bucketed into bands of `y`.
fn gnomonic_ring_holds_all(parent: &[[f64; 3]], inside: &[[f64; 3]], centre: [f64; 3]) -> bool {
    let axis = if centre[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let e1 = cross(centre, axis);
    let length = dot(e1, e1).sqrt();
    let e1 = [e1[0] / length, e1[1] / length, e1[2] / length];
    let e2 = cross(centre, e1);
    let project = |p: [f64; 3]| -> Option<(f64, f64)> {
        let z = dot(p, centre);
        (z > 1.0e-9).then(|| (dot(p, e1) / z, dot(p, e2) / z))
    };
    let Some(ring) = parent
        .iter()
        .map(|&p| project(p))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(queries) = inside
        .iter()
        .map(|&p| project(p))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if ring.len() < 3 {
        return false;
    }
    let (y_min, y_max) = ring
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, y)| {
            (lo.min(y), hi.max(y))
        });
    let bands = ((ring.len() as f64).sqrt().ceil() as usize).max(1);
    let height = ((y_max - y_min) / bands as f64).max(f64::MIN_POSITIVE);
    let band_of = |y: f64| (((y - y_min) / height).floor().max(0.0) as usize).min(bands - 1);
    let mut edges_in: Vec<Vec<usize>> = vec![Vec::new(); bands];
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        for band in band_of(a.1.min(b.1))..=band_of(a.1.max(b.1)) {
            edges_in[band].push(k);
        }
    }
    queries.iter().all(|&(x, y)| {
        if y < y_min || y > y_max {
            return false;
        }
        let mut crossings = 0usize;
        for &k in &edges_in[band_of(y)] {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            if (a.1 > y) != (b.1 > y) {
                let t = (y - a.1) / (b.1 - a.1);
                if a.0 + t * (b.0 - a.0) > x {
                    crossings += 1;
                }
            }
        }
        crossings % 2 == 1
    })
}

#[cfg(test)]
mod grown_ring_tests {
    use super::*;

    #[test]
    fn the_gnomonic_check_answers_as_the_boundary_model_does() {
        // The boundary model is the oracle: a concave ring, then every query
        // on a grid around it, one at a time.
        let mut points: Vec<LonLatDegrees> = (0..240)
            .map(|k| {
                let t = k as f64 / 240.0 * std::f64::consts::TAU;
                let r = 3.0 * (0.5 + 0.5 * (4.0 * t).sin().abs());
                LonLatDegrees::new(
                    100.0 + r * t.cos() / 40f64.to_radians().cos(),
                    40.0 + r * t.sin(),
                )
            })
            .collect();
        points.push(points[0]);
        let unit = |p: LonLatDegrees| {
            let p = earthmesh_mesh::lonlat_degrees_to_unit_xyz(p);
            [p.x, p.y, p.z]
        };
        let ring: Vec<[f64; 3]> = points.iter().map(|&p| unit(p)).collect();
        let centre = unit(LonLatDegrees::new(100.0, 40.0));
        let model =
            crate::boundary_model::boundary_model_from_regions(&[RefinementRegion::Polygon {
                points: points.clone(),
                level: 1,
            }]);
        let mut inside = 0;
        for i in 0..80 {
            for j in 0..80 {
                let q = LonLatDegrees::new(94.013 + i as f64 * 0.15, 36.007 + j as f64 * 0.1);
                let expected = model.contains(q.lon_degrees, q.lat_degrees);
                assert_eq!(
                    gnomonic_ring_holds_all(&ring, &[unit(q)], centre),
                    expected,
                    "{q:?}"
                );
                inside += usize::from(expected);
            }
        }
        assert!(inside > 100 && inside < 6400 - 100, "{inside}");
    }

    /// The ring in the form production hands to `grown_ring`: closed, with the
    /// first point repeated at the end.
    ///
    /// Measuring on the raw point list instead gave a different centroid and a
    /// different answer -- an L that production grows came back refused. A test
    /// that does not feed what the caller feeds is measuring another function.
    fn ring(points: &[(f64, f64)]) -> Vec<LonLatDegrees> {
        let mut ring: Vec<LonLatDegrees> = points
            .iter()
            .map(|&(lon, lat)| LonLatDegrees::new(lon, lat))
            .collect();
        if let Some(&first) = ring.first() {
            ring.push(first);
        }
        ring
    }

    /// A convex ring grows, and the grown one contains the original.
    #[test]
    fn a_convex_ring_grows_around_its_child() {
        let child = ring(&[(110.0, 15.0), (125.0, 15.0), (125.0, 30.0), (110.0, 30.0)]);
        let parent = grown_ring(&child, 200_000.0).expect("a convex ring grows");
        assert_eq!(parent.len(), child.len());

        let model =
            crate::boundary_model::boundary_model_from_regions(&[RefinementRegion::Polygon {
                points: parent,
                level: 1,
            }]);
        for point in &child {
            assert!(
                model.contains(point.lon_degrees, point.lat_degrees),
                "the parent must contain {point:?}"
            );
        }
    }

    /// A concave ring that is still star-shaped about its centroid grows too.
    ///
    /// Concavity alone does not break radial growth, which is why the check is
    /// on whether the parent contains the child rather than on whether the ring
    /// is convex -- an L would have been refused for nothing.
    #[test]
    fn a_concave_but_star_shaped_ring_still_grows() {
        let l_shape = ring(&[
            (110.0, 15.0),
            (125.0, 15.0),
            (125.0, 22.0),
            (117.0, 22.0),
            (117.0, 30.0),
            (110.0, 30.0),
        ]);
        assert!(grown_ring(&l_shape, 200_000.0).is_some());
    }

    /// A ring whose centroid sits in its own mouth gets no parent.
    ///
    /// Radial growth from a point the ring does not enclose is not growth: the
    /// result comes back not containing the child. Measured before this check
    /// existed, such a parent reached Method-C and failed with `Current nested
    /// grid crosses the parent boundary at M point 2795` -- a perimeter it
    /// could not walk, reported nowhere near the code that built it.
    #[test]
    fn a_deep_c_whose_centroid_is_outside_it_gets_no_parent() {
        let deep_c = ring(&[
            (110.0, 15.0),
            (125.0, 15.0),
            (125.0, 18.0),
            (114.0, 18.0),
            (114.0, 27.0),
            (125.0, 27.0),
            (125.0, 30.0),
            (110.0, 30.0),
        ]);
        assert!(
            grown_ring(&deep_c, 200_000.0).is_none(),
            "no parent is better than one that does not contain its child"
        );
    }
}
