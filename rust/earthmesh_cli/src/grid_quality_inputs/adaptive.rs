//! Reconcile the point+radius route's circles against the mesh it produced.
//!
//! Quality runs as its own step and cannot see the run's `AdaptiveNestReport`.
//! The refinement step leaves the circles it actually emitted in
//! `adaptive_refinement.json` beside the selected gridfile. A Project namelist
//! can live in a different directory, so it cannot locate this artifact.
//!
//! Reading the emitted circles rather than re-planning the demand is what makes
//! a mismatch mean something: it can only be a refinement failure, never a
//! difference in how the criteria were evaluated the second time.

use std::io;
use std::path::Path;

use earthmesh_quality::{AdaptiveConfigDiagnostics, MeshQualityReport, QualityMeshInput};

use crate::refinement_demand::nest::ADAPTIVE_REFINEMENT_FILE;
use crate::GridfileMeshPoints;

/// One circle a pass emitted.
#[derive(Clone, Copy, Debug, PartialEq)]
struct EmittedCircle {
    lon_degrees: f64,
    lat_degrees: f64,
    radius_meters: f64,
    level: u32,
}

/// What the refinement step recorded about its point+radius run.
#[derive(Clone, Debug, Default, PartialEq)]
struct EmittedRefinement {
    max_level: Option<u32>,
    base_m: Option<f64>,
    coastline: bool,
    pass_count: usize,
    circles: Vec<EmittedCircle>,
}

impl EmittedRefinement {
    /// Deepest level whose circles cover this point, zero where none do.
    ///
    /// Tests every circle. Kept as the oracle for [`CircleIndex`]; a quality run
    /// asks it once per cell, and a global red-green run emits ~190 000 circles
    /// for ~700 000 cells.
    #[cfg(test)]
    fn target_level_at(&self, lon_degrees: f64, lat_degrees: f64) -> u32 {
        self.circles
            .iter()
            .filter(|circle| circle.covers(lon_degrees, lat_degrees))
            .map(|circle| circle.level)
            .max()
            .unwrap_or(0)
    }
}

impl EmittedCircle {
    fn covers(&self, lon_degrees: f64, lat_degrees: f64) -> bool {
        earthmesh_hfield::great_circle_distance_m(
            self.lon_degrees,
            self.lat_degrees,
            lon_degrees,
            lat_degrees,
        ) <= self.radius_meters
    }
}

/// Circles bucketed by one-degree longitude/latitude cells, so a point tests
/// only the circles whose bounding box reaches its cell.
///
/// Every cell a qualifying point can lie in is listed conservatively and the
/// exact great-circle test still decides, so the answer is the full scan's.
/// Without it the quality step spent hours on a global run: every cell tested
/// every circle, 1.3e11 great-circle distances, single-threaded.
struct CircleIndex<'a> {
    circles: &'a [EmittedCircle],
    buckets: Vec<Vec<u32>>,
}

impl<'a> CircleIndex<'a> {
    const NLON: usize = 360;
    const NLAT: usize = 180;
    /// Degrees added to every box, far above floating-point noise in the exact
    /// test and far below a bucket.
    const MARGIN_DEGREES: f64 = 0.01;

    fn new(circles: &'a [EmittedCircle]) -> Self {
        let mut buckets = vec![Vec::new(); Self::NLON * Self::NLAT];
        for (index, circle) in circles.iter().enumerate() {
            let angle = circle.radius_meters.max(0.0) / earthmesh_hfield::EARTH_RADIUS_METERS;
            let angle_degrees = angle.to_degrees() + Self::MARGIN_DEGREES;
            let south = circle.lat_degrees - angle_degrees;
            let north = circle.lat_degrees + angle_degrees;
            // A circle reaching a pole, or one wide enough that its longitude
            // extent is not bounded, spans every longitude.
            let cos_lat = circle.lat_degrees.to_radians().cos();
            let lon_half = if north >= 90.0 || south <= -90.0 || angle.sin() >= cos_lat {
                None
            } else {
                Some((angle.sin() / cos_lat).asin().to_degrees() + Self::MARGIN_DEGREES)
            };
            let lat_rows = Self::lat_row(south)..=Self::lat_row(north);
            let lon_cols: Vec<usize> = match lon_half {
                Some(half) if 2.0 * half < 359.0 => {
                    let west = (circle.lon_degrees - half).floor() as i64;
                    let east = (circle.lon_degrees + half).floor() as i64;
                    (west..=east)
                        .map(|lon| (lon + 180).rem_euclid(Self::NLON as i64) as usize)
                        .collect()
                }
                _ => (0..Self::NLON).collect(),
            };
            for row in lat_rows {
                for &col in &lon_cols {
                    buckets[row * Self::NLON + col].push(index as u32);
                }
            }
        }
        Self { circles, buckets }
    }

    fn lat_row(lat_degrees: f64) -> usize {
        ((lat_degrees + 90.0).floor().max(0.0) as usize).min(Self::NLAT - 1)
    }

    fn lon_col(lon_degrees: f64) -> usize {
        ((lon_degrees + 180.0).floor() as i64).rem_euclid(Self::NLON as i64) as usize
    }

    fn target_level_at(&self, lon_degrees: f64, lat_degrees: f64) -> u32 {
        let bucket = Self::lat_row(lat_degrees) * Self::NLON + Self::lon_col(lon_degrees);
        self.buckets[bucket]
            .iter()
            .map(|&index| &self.circles[index as usize])
            .filter(|circle| circle.covers(lon_degrees, lat_degrees))
            .map(|circle| circle.level)
            .max()
            .unwrap_or(0)
    }
}

fn json_number(source: &str, key: &str) -> Option<f64> {
    let start = source.find(&format!("\"{key}\":"))? + key.len() + 3;
    let rest = &source[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == '+'))
        .unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
}

fn json_bool(source: &str, key: &str) -> Option<bool> {
    let start = source.find(&format!("\"{key}\":"))? + key.len() + 3;
    let rest = source[start..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Parse the file the refinement step wrote.
///
/// Hand-parsed rather than pulled through a JSON crate because the shape is
/// fixed and written by the same code that reads it; a dependency here would
/// buy nothing the format does not already guarantee.
fn parse_emitted_refinement(contents: &str) -> io::Result<EmittedRefinement> {
    let invalid = |message: &str| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{ADAPTIVE_REFINEMENT_FILE}: {message}"),
        )
    };
    if json_bool(contents, "enabled") != Some(true) {
        return Err(invalid("missing or false \"enabled\""));
    }
    let mut emitted = EmittedRefinement {
        max_level: json_number(contents, "max_level").map(|value| value as u32),
        base_m: json_number(contents, "base_m"),
        coastline: json_bool(contents, "coastline").unwrap_or(false),
        pass_count: 0,
        circles: Vec::new(),
    };
    // Each pass is `{"level":N,...,"circles":[...]}`; walk them in order so a
    // circle is attributed to the pass that emitted it.
    for pass in contents.split("{\"level\":").skip(1) {
        let Some(level) = json_number(&format!("{{\"level\":{pass}"), "level") else {
            continue;
        };
        emitted.pass_count += 1;
        let Some(circles_start) = pass.find("\"circles\":[") else {
            continue;
        };
        let circles = &pass[circles_start + "\"circles\":[".len()..];
        let end = circles.find(']').unwrap_or(circles.len());
        for circle in circles[..end]
            .split("},")
            .filter(|item| item.contains("lon"))
        {
            match (
                json_number(circle, "lon"),
                json_number(circle, "lat"),
                json_number(circle, "radius_m"),
            ) {
                (Some(lon_degrees), Some(lat_degrees), Some(radius_meters)) => {
                    emitted.circles.push(EmittedCircle {
                        lon_degrees,
                        lat_degrees,
                        radius_meters,
                        level: level as u32,
                    })
                }
                _ => return Err(invalid("a circle is missing lon, lat or radius_m")),
            }
        }
    }
    Ok(emitted)
}

/// Target level per quality cell, sampled at the cell centre.
///
/// The h-field takes the maximum over a hex cell's corners, which suits a field
/// that varies smoothly. A circle has a hard edge, so a corner can sit inside
/// while the centre does not — and Method-C selects faces by centre containment,
/// so the corner reading would claim a level the engine never intended to give
/// that cell. Measured on a real run: corner sampling reported 140 of 1643 hex
/// cells short of their target, centre sampling reports what the engine
/// actually promised.
fn adaptive_target_levels_for_quality_cells(
    mesh: &GridfileMeshPoints,
    kind: &str,
    emitted: &EmittedRefinement,
) -> io::Result<Vec<u32>> {
    let index = CircleIndex::new(&emitted.circles);
    match kind.trim() {
        "tri" => Ok(super::gridfile::tri_quality_cells_from_gridfile(mesh)?
            .into_iter()
            .map(|(mi, _)| index.target_level_at(mesh.m_lon[mi], mesh.m_lat[mi]))
            .collect()),
        "hex" => Ok(super::gridfile::hex_quality_cells_from_gridfile(mesh)?
            .into_iter()
            .map(|(wi, _corners)| index.target_level_at(mesh.w_lon[wi], mesh.w_lat[wi]))
            .collect()),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("point+radius diagnostics support tri or hex view, got {other}"),
        )),
    }
}

/// Attach point+radius diagnostics, if this run took that route.
///
/// Returns whether anything was attached, so a caller can tell "not this route"
/// from "this route, nothing wrong".
pub fn attach_adaptive_diagnostics_from_gridfile_path(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    mesh: &GridfileMeshPoints,
    kind: &str,
    gridfile_path: &Path,
) -> io::Result<bool> {
    let Some(directory) = gridfile_path.parent() else {
        return Ok(false);
    };
    let path = directory.join(ADAPTIVE_REFINEMENT_FILE);
    if !path.is_file() {
        return Ok(false);
    }
    let emitted = parse_emitted_refinement(&std::fs::read_to_string(&path)?)?;
    let target_levels = adaptive_target_levels_for_quality_cells(mesh, kind, &emitted)?;
    if target_levels.len() != input.cells.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "point+radius diagnostics sampled {} cells for a {} cell mesh",
                target_levels.len(),
                input.cells.len()
            ),
        ));
    }
    earthmesh_quality::attach_adaptive_diagnostics(
        report,
        input,
        &target_levels,
        AdaptiveConfigDiagnostics {
            enabled: true,
            max_level: emitted.max_level,
            base_m: emitted.base_m,
            coastline: emitted.coastline,
            pass_count: emitted.pass_count,
            circle_count: emitted.circles.len(),
        },
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"enabled":true,"max_level":2,"base_m":381000,"coastline":true,
        "deepest_level":2,"stopped_on_empty_demand":false,"passes":[
        {"level":1,"cell_meters":381000,"demanded_cells":12,"circles":[
            {"lon":114,"lat":22,"radius_m":400000},{"lon":115,"lat":22,"radius_m":400000}]},
        {"level":2,"cell_meters":190500,"demanded_cells":12,"circles":[
            {"lon":114,"lat":22,"radius_m":150000}]}]}"#;

    #[test]
    fn every_circle_reads_back_with_the_level_that_emitted_it() {
        let emitted = parse_emitted_refinement(SAMPLE).expect("parse");
        assert_eq!(emitted.max_level, Some(2));
        assert_eq!(emitted.base_m, Some(381_000.0));
        assert!(emitted.coastline);
        assert_eq!(emitted.pass_count, 2);
        assert_eq!(emitted.circles.len(), 3);
        assert_eq!(
            emitted.circles.iter().filter(|c| c.level == 1).count(),
            2,
            "{:?}",
            emitted.circles
        );
        assert_eq!(emitted.circles.iter().filter(|c| c.level == 2).count(), 1);
    }

    #[test]
    fn the_target_level_is_the_deepest_circle_covering_a_point() {
        let emitted = parse_emitted_refinement(SAMPLE).expect("parse");
        // Inside both rings.
        assert_eq!(emitted.target_level_at(114.0, 22.0), 2);
        // Inside the level-1 rings only. One degree of longitude at 22 north is
        // about 103 km, so 116 east is ~206 km from the inner ring's centre --
        // outside its 150 km radius, inside both 400 km ones.
        assert_eq!(emitted.target_level_at(116.0, 22.0), 1);
        // Outside everything.
        assert_eq!(emitted.target_level_at(0.0, 0.0), 0);
    }

    #[test]
    fn the_circle_index_answers_exactly_as_the_full_scan() {
        // Circles at the poles, across the antimeridian, near the equator and
        // wide enough to span every longitude; points on a fine global lattice
        // plus each circle's own centre and rim.
        let mut circles = Vec::new();
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        for level in 1..=3u32 {
            for _ in 0..400 {
                circles.push(EmittedCircle {
                    lon_degrees: next() * 360.0 - 180.0,
                    lat_degrees: next() * 180.0 - 90.0,
                    radius_meters: 5_000.0 + next() * 400_000.0,
                    level,
                });
            }
        }
        for (lon, lat, radius) in [
            (0.0, 90.0, 300_000.0),
            (37.0, -89.5, 120_000.0),
            (179.95, 10.0, 200_000.0),
            (-179.99, -45.0, 250_000.0),
            (180.0, 60.0, 150_000.0),
            (10.0, 0.0, 6_000_000.0),
            (0.0, 88.0, 50_000.0),
        ] {
            circles.push(EmittedCircle {
                lon_degrees: lon,
                lat_degrees: lat,
                radius_meters: radius,
                level: 4,
            });
        }
        let emitted = EmittedRefinement {
            circles: circles.clone(),
            ..EmittedRefinement::default()
        };
        let index = CircleIndex::new(&emitted.circles);
        let mut points = Vec::new();
        for i in 0..720 {
            for j in 0..=360 {
                points.push((i as f64 * 0.5 - 180.0 + 0.123, j as f64 * 0.5 - 90.0));
            }
        }
        for circle in &circles {
            let angle = (circle.radius_meters / earthmesh_hfield::EARTH_RADIUS_METERS).to_degrees();
            points.push((circle.lon_degrees, circle.lat_degrees));
            points.push((
                circle.lon_degrees,
                (circle.lat_degrees + angle * 0.999).min(90.0),
            ));
            points.push((circle.lon_degrees + 360.0, circle.lat_degrees));
        }
        let mut covered = 0usize;
        for (lon, lat) in points {
            let expected = emitted.target_level_at(lon, lat);
            covered += usize::from(expected > 0);
            assert_eq!(index.target_level_at(lon, lat), expected, "({lon}, {lat})");
        }
        assert!(covered > 1000, "fixture must exercise covered points");
    }

    #[test]
    fn a_file_that_is_not_this_route_is_rejected_rather_than_read_as_empty() {
        // Reading a disabled or foreign file as "no circles" would report a
        // clean reconciliation for a run this never described.
        assert!(parse_emitted_refinement(r#"{"enabled":false}"#).is_err());
        assert!(parse_emitted_refinement("{}").is_err());
    }

    #[test]
    fn a_truncated_circle_is_rejected() {
        let broken = r#"{"enabled":true,"passes":[{"level":1,"circles":[{"lon":114,"lat":22}]}]}"#;
        let error = parse_emitted_refinement(broken).expect_err("truncated circle");
        assert!(error.to_string().contains("radius_m"), "{error}");
    }
}
