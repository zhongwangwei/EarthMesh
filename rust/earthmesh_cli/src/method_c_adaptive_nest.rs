//! Criteria-driven refinement on Method-C, from the CLI's inputs.
//!
//! The loop -- group each level's circles, refine group by group, drop or end
//! at what the transition template refuses -- is Method-C's
//! (`earthmesh_refine_method_c::spawn_nest_adaptive_levels`). What is here is
//! the rest of the run around it: each level is planned from the source
//! rasters (`refinement_demand::nest`), and every region group's attempt is
//! kept in `refinement_groups.jsonl` in the working directory, where a
//! diagnosis can pick one up.

use std::io;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::RefinementRegion;
use earthmesh_refine::nest::AdaptiveNestReport;
pub use earthmesh_refine_method_c::{AdaptiveNestSpring, METHOD_C_ADAPTIVE_SUSPENDED};
use earthmesh_refine_method_c::{GroupAttempt, MethodCMesh};

use crate::refinement_demand::ladder::MEASURED_PARENT_HALO_ROWS;
use crate::refinement_demand::nest::adaptive_demand_circles_for_level_windows;
use crate::refinement_demand::plan::DemandPlanInputs;

/// Refine `mesh` up to `max_level`, re-planning demand before every pass.
pub fn spawn_nest_adaptive_with_named_regions(
    mesh: &MethodCMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    named_regions: &[RefinementRegion],
    base_cell_meters: f64,
    max_level: usize,
    spring: Option<AdaptiveNestSpring>,
) -> io::Result<(MethodCMesh, AdaptiveNestReport)> {
    spawn_nest_adaptive_with_named_region_windows(
        mesh,
        refine,
        std::slice::from_ref(inputs),
        named_regions,
        base_cell_meters,
        max_level,
        spring,
    )
}

/// As [`spawn_nest_adaptive_with_named_regions`], with the criteria read over
/// several source windows.
pub fn spawn_nest_adaptive_with_named_region_windows(
    mesh: &MethodCMesh,
    refine: &RefineConfig,
    inputs: &[DemandPlanInputs<'_>],
    named_regions: &[RefinementRegion],
    base_cell_meters: f64,
    max_level: usize,
    spring: Option<AdaptiveNestSpring>,
) -> io::Result<(MethodCMesh, AdaptiveNestReport)> {
    earthmesh_refine_method_c::spawn_nest_adaptive_levels(
        mesh,
        named_regions,
        base_cell_meters,
        max_level,
        MEASURED_PARENT_HALO_ROWS,
        spring,
        &mut |level| {
            adaptive_demand_circles_for_level_windows(
                refine,
                inputs,
                level,
                base_cell_meters,
                max_level,
            )
        },
        &mut record_group_attempt,
    )
}

/// Append one region group's attempt to `refinement_groups.jsonl`.
///
/// Replaying one group locally takes seconds; re-running the globe to reach
/// the same failure takes most of an hour. Best effort: a run that cannot
/// write the file still refines.
fn record_group_attempt(attempt: &GroupAttempt<'_>) {
    use std::io::Write;
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("refinement_groups.jsonl")
    else {
        return;
    };
    let _ = writeln!(file, "{}", group_attempt_line(attempt));
}

/// One line of `refinement_groups.jsonl`: the group's circles, and whether
/// and why it was refused.
fn group_attempt_line(attempt: &GroupAttempt<'_>) -> String {
    let circles: Vec<String> = attempt
        .group
        .iter()
        .filter_map(|region| match region {
            RefinementRegion::Circle {
                center,
                radius_meters,
                ..
            } => Some(format!(
                "{{\"lon\":{},\"lat\":{},\"r\":{}}}",
                center.lon_degrees, center.lat_degrees, radius_meters
            )),
            _ => None,
        })
        .collect();
    let status = if attempt.refused.is_some() {
        "refused"
    } else {
        "ok"
    };
    format!(
        "{{\"level\":{},\"order\":{},\"status\":\"{status}\",\"faces\":{},\"reason\":{:?},\"circles\":[{}]}}",
        attempt.level,
        attempt.order,
        attempt.faces,
        attempt.refused.unwrap_or_default(),
        circles.join(",")
    )
}

/// Adaptive refinement with no regions named outright.
pub fn spawn_nest_adaptive(
    mesh: &MethodCMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<(MethodCMesh, AdaptiveNestReport)> {
    spawn_nest_adaptive_with_named_regions(
        mesh,
        refine,
        inputs,
        &[],
        base_cell_meters,
        max_level,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_mesh::LonLatDegrees;

    #[test]
    fn a_group_attempt_is_one_json_line_listing_its_circles() {
        let group = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(10.5, -20.25),
                radius_meters: 1000.0,
                level: 2,
            },
            RefinementRegion::Bbox {
                west_degrees: 0.0,
                east_degrees: 1.0,
                south_degrees: 0.0,
                north_degrees: 1.0,
                level: 2,
            },
        ];
        let attempt = |refused| GroupAttempt {
            level: 2,
            order: 3,
            group: &group,
            faces: 1234,
            refused,
        };
        let refused = group_attempt_line(&attempt(Some("crosses \"the\" parent")));
        assert_eq!(
            refused,
            r#"{"level":2,"order":3,"status":"refused","faces":1234,"reason":"crosses \"the\" parent","circles":[{"lon":10.5,"lat":-20.25,"r":1000}]}"#
        );
        let built = group_attempt_line(&attempt(None));
        assert_eq!(
            built,
            r#"{"level":2,"order":3,"status":"ok","faces":1234,"reason":"","circles":[{"lon":10.5,"lat":-20.25,"r":1000}]}"#
        );
        for line in [refused, built] {
            serde_json::from_str::<serde_json::Value>(&line).expect("one JSON object per line");
        }
    }
}
