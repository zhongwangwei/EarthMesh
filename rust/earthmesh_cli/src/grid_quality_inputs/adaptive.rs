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

use earthmesh_refine::{CircleTargets, LeveledCircle};

use super::{target_levels_for_quality_cells, HexSample};
use crate::refinement_demand::nest::ADAPTIVE_REFINEMENT_FILE;
use crate::GridfileMeshPoints;

/// What the refinement step recorded about its point+radius run.
#[derive(Clone, Debug, Default, PartialEq)]
struct EmittedRefinement {
    max_level: Option<u32>,
    base_m: Option<f64>,
    coastline: bool,
    pass_count: usize,
    deepest_level: Option<u32>,
    stopped_on_empty_demand: bool,
    circles: Vec<LeveledCircle>,
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
        deepest_level: json_number(contents, "deepest_level").map(|value| value as u32),
        stopped_on_empty_demand: json_bool(contents, "stopped_on_empty_demand").unwrap_or(false),
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
                    emitted.circles.push(LeveledCircle {
                        lon_degrees,
                        lat_degrees,
                        radius_meters,
                        level: level as usize,
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
    target_levels_for_quality_cells(
        mesh,
        kind,
        HexSample::Centre,
        &CircleTargets::new(&emitted.circles),
        "point+radius",
    )
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
            deepest_level: emitted.deepest_level,
            stopped_on_empty_demand: emitted.stopped_on_empty_demand,
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
        assert_eq!(emitted.deepest_level, Some(2));
        assert!(!emitted.stopped_on_empty_demand);
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
        use earthmesh_refine::TargetLevelField;
        let emitted = parse_emitted_refinement(SAMPLE).expect("parse");
        let targets = CircleTargets::new(&emitted.circles);
        let target_level_at = |lon: f64, lat: f64| {
            targets
                .target_level(earthmesh_mesh::LonLatDegrees::new(lon, lat))
                .unwrap()
        };
        // Inside both rings.
        assert_eq!(target_level_at(114.0, 22.0), 2);
        // Inside the level-1 rings only. One degree of longitude at 22 north is
        // about 103 km, so 116 east is ~206 km from the inner ring's centre --
        // outside its 150 km radius, inside both 400 km ones.
        assert_eq!(target_level_at(116.0, 22.0), 1);
        // Outside everything.
        assert_eq!(target_level_at(0.0, 0.0), 0);
    }

    #[test]
    fn a_run_that_stopped_short_reads_back_the_depth_it_reached() {
        // The shape Method-C leaves when it cannot nest the second level: the
        // record asks for two, carries one pass, and the demand had not run out.
        let emitted = parse_emitted_refinement(
            r#"{"enabled":true,"max_level":2,"base_m":190626,"coastline":true,
            "deepest_level":1,"stopped_on_empty_demand":false,"passes":[
            {"level":1,"cell_meters":190626,"demanded_cells":0,"circles":[
                {"lon":110,"lat":30,"radius_m":1000000}]}]}"#,
        )
        .expect("parse");
        assert_eq!(emitted.max_level, Some(2));
        assert_eq!(emitted.deepest_level, Some(1));
        assert!(!emitted.stopped_on_empty_demand);
        assert_eq!(emitted.pass_count, 1);
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
