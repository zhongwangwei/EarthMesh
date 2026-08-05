//! Circle radii for a nested chain, one per level.
//!
//! A level's circle has to clear the next level's by enough that Method-C can
//! lay transition rows between them; too little and the child is refused for
//! crossing the parent boundary, or a transition patch comes out with no solid
//! split edge.
//!
//! The engine already states how much "enough" is. `region_sources::circle`
//! expands a parent circle by `rows * base_spacing / 2^(t-1)` summed over the
//! transition levels between parent and child, `rows` coming from the
//! namelist's `halo` / `max_transition_row`. Both of those default to zero,
//! which makes every parent circle the same size as its child — which is why
//! chains built on the defaults failed. So the formula is the engine's and only
//! the row count is supplied here, measured rather than argued.
//!
//! Nothing here knows what asked for the refinement. A criterion whose demand
//! shrinks with the level being decided (land-cover heterogeneity, sub-grid
//! variance) nests on its own; a criterion whose demand is identical at every
//! level (`sst > 28`, `slope > 15`) has to borrow the separation from here.
//!
//! # What is and is not new here
//!
//! The expansion formula is not. It predates this module, lives in
//! `region_sources::circle`, and states something geometrically obvious: a
//! transition level `t` has cell spacing `base / 2^(t-1)`, so `rows` rows of it
//! are that much wide, summed over the levels a parent has to clear. The
//! namelist's `halo` / `max_transition_row` exist so a hand-written circle can
//! declare that width.
//!
//! What this module adds is running it backwards -- given the innermost radius
//! and a depth, produce the whole chain -- and supplying a row count, because
//! both namelist fields default to zero, which makes every parent circle the
//! same size as its child and every chain fail. Do not describe the formula as
//! a contribution; do report the row count, which is a measurement.

use std::io;

use super::materializable_radius_meters;

/// Transition rows to leave between a level and its parent.
///
/// Measured against `spawn_nest`, not derived. The sweep covered NXP 21, 40 and
/// 81, centres over the South China Sea, the North Atlantic and the equator,
/// innermost radii at 0.4 and 0.6 of a base cell, and depths two through five:
///
/// | rows | outcome over the 18 configurations |
/// |---|---|
/// | 2.0 | six failed, at depth 2 or 5, scattered across resolution and place |
/// | 2.5 | all passed |
/// | 3.0 | all passed |
///
/// Three is taken for the margin. The number cannot be argued from the ring
/// count, and it cannot be guessed generously either: the admissible set is
/// **not** upward closed — at NXP 21 a 200 km innermost radius nests five
/// levels at 1.5 rows and fails at 2.0 — so a larger value is not safe by
/// being larger. Only the sweep settles it, which is why the spawn test in
/// `tests/refinement_ladder_spawn.rs` runs the ladder rather than checking it
/// against the rule it came from.
pub const MEASURED_PARENT_HALO_ROWS: f64 = 3.0;

/// Radii for levels 1..=`max_level`, coarsest first, by the engine's halo rule.
///
/// `innermost_radius_meters` is what the deepest level actually has to cover;
/// every coarser level is that plus the halo the transition rows need.
pub fn nested_circle_radii_meters_with_halo_rows(
    base_cell_meters: f64,
    max_level: usize,
    innermost_radius_meters: f64,
    halo_rows: f64,
) -> io::Result<Vec<f64>> {
    if !base_cell_meters.is_finite() || base_cell_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "base cell size must be positive and finite",
        ));
    }
    if !innermost_radius_meters.is_finite() || innermost_radius_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "innermost circle radius must be positive and finite",
        ));
    }
    if !halo_rows.is_finite() || halo_rows <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "parent halo rows must be positive and finite",
        ));
    }
    if !(1..=5).contains(&max_level) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Method-C refinement level {max_level} must be in 1..=5"),
        ));
    }
    Ok((1..=max_level)
        .map(|parent_level| {
            let halo: f64 = (parent_level..max_level)
                .map(|transition_level| {
                    halo_rows * base_cell_meters / 2f64.powi((transition_level - 1) as i32)
                })
                .sum();
            innermost_radius_meters + halo
        })
        .collect())
}

/// Radii for a chain that only has to be materializable at its deepest level.
///
/// The innermost radius is the floor for the cell size the last pass refines,
/// and the rest follows [`MEASURED_PARENT_HALO_ROWS`].
pub fn nested_circle_radii_meters(base_cell_meters: f64, max_level: usize) -> io::Result<Vec<f64>> {
    let innermost = materializable_radius_meters(base_cell_meters);
    nested_circle_radii_meters_with_halo_rows(
        base_cell_meters,
        max_level,
        innermost,
        MEASURED_PARENT_HALO_ROWS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NXP 21: 2 pi R / (5 * 21).
    const NXP21_BASE_M: f64 = 381_000.0;

    #[test]
    fn one_level_is_just_the_innermost_radius() {
        let radii = nested_circle_radii_meters(NXP21_BASE_M, 1).expect("ladder");
        assert_eq!(radii.len(), 1);
        assert!((radii[0] - materializable_radius_meters(NXP21_BASE_M)).abs() < 1.0);
    }

    #[test]
    fn radii_close_in_as_the_levels_go_deeper() {
        let radii = nested_circle_radii_meters(NXP21_BASE_M, 5).expect("ladder");
        assert_eq!(radii.len(), 5);
        for pair in radii.windows(2) {
            assert!(pair[0] > pair[1], "got {radii:?}");
        }
        // The halo halves every level, so the gaps shrink -- that is what keeps
        // a five-level ladder from running away.
        let gaps: Vec<f64> = radii.windows(2).map(|pair| pair[0] - pair[1]).collect();
        for pair in gaps.windows(2) {
            assert!(pair[0] > pair[1], "gaps must shrink, got {gaps:?}");
        }
    }

    #[test]
    fn the_halo_matches_the_engines_own_expansion() {
        // region_sources::circle sums rows * base_spacing / 2^(t-1) over the
        // transition levels; the ladder must agree with it or a chain built
        // here and a chain built there would nest differently.
        let rows = 2.0;
        let radii =
            nested_circle_radii_meters_with_halo_rows(NXP21_BASE_M, 3, 150_000.0, rows).unwrap();
        let expected_level_1 = 150_000.0 + rows * NXP21_BASE_M / 1.0 + rows * NXP21_BASE_M / 2.0;
        let expected_level_2 = 150_000.0 + rows * NXP21_BASE_M / 2.0;
        assert!((radii[0] - expected_level_1).abs() < 1.0, "got {radii:?}");
        assert!((radii[1] - expected_level_2).abs() < 1.0, "got {radii:?}");
        assert!((radii[2] - 150_000.0).abs() < 1.0, "got {radii:?}");
    }

    #[test]
    fn an_impossible_request_is_rejected() {
        assert!(nested_circle_radii_meters(NXP21_BASE_M, 0).is_err());
        assert!(nested_circle_radii_meters(NXP21_BASE_M, 6).is_err());
        assert!(nested_circle_radii_meters(0.0, 1).is_err());
        assert!(nested_circle_radii_meters(f64::NAN, 1).is_err());
        assert!(
            nested_circle_radii_meters_with_halo_rows(NXP21_BASE_M, 2, 100.0, 0.0).is_err(),
            "zero halo rows leave a parent the same size as its child"
        );
    }
}
