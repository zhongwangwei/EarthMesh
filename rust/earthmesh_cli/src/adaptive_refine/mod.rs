//! The `&adaptive` namelist group: refine by point+radius, one level at a time.
//!
//! ```text
//!   adaptive_on         .true./.false.  master switch (`&adaptive` is opt-in)
//!   adaptive_max_level  depth, 1..=5; 0 = use the run's max level
//!   adaptive_base_m     base cell size in meters; 0/absent = 2piR/(5*NXP)
//!   adaptive_coastline  .true./.false.  chase the land/sea boundary (default true)
//! ```
//!
//! This is the other consumer of the same criteria the h-field takes. Where the
//! h-field composes every criterion into one gradient-limited width field and
//! quantises it once, this asks the criteria again before each pass and covers
//! what they demand with circles. Both start from the same enabled-criterion
//! list, which is what makes them comparable on the same run.

use std::io;

use crate::namelist_reader::{namelist_assignments, namelist_has_section};

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Settings for the adaptive point+radius route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveRefineOptions {
    /// `None` = follow the run's computed max refinement level.
    pub max_level: Option<usize>,
    /// `None` = 2piR/(5*NXP).
    pub base_m: Option<f64>,
    /// Refine the land/sea boundary. The namelist expresses coastal demand
    /// through `th_sea_ratio` for the h-field; the circle route can chase the
    /// boundary itself, and this says whether it should.
    pub coastline: bool,
}

impl Default for AdaptiveRefineOptions {
    fn default() -> Self {
        Self {
            max_level: None,
            base_m: None,
            coastline: true,
        }
    }
}

fn parse_bool(field: &str, value: &str) -> io::Result<bool> {
    match value.trim().trim_matches('.').to_ascii_lowercase().as_str() {
        "true" | "t" => Ok(true),
        "false" | "f" => Ok(false),
        other => Err(invalid(format!(
            "{field} must be .true. or .false., got {other}"
        ))),
    }
}

fn parse_f64(field: &str, value: &str) -> io::Result<f64> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|_| invalid(format!("{field} must be a number, got {value}")))
}

fn parse_usize(field: &str, value: &str) -> io::Result<usize> {
    value.trim().parse::<usize>().map_err(|_| {
        invalid(format!(
            "{field} must be a non-negative integer, got {value}"
        ))
    })
}

/// Read the `&adaptive` group. Absent group or `adaptive_on = .false.` yields
/// `Ok(None)`.
pub fn read_adaptive_refine_options(contents: &str) -> io::Result<Option<AdaptiveRefineOptions>> {
    if !namelist_has_section(contents, "adaptive") {
        return Ok(None);
    }
    let mut enabled = true;
    let mut max_level = 0usize;
    let mut base_m = 0.0_f64;
    let mut coastline = true;
    for assignment in namelist_assignments(contents, "adaptive")? {
        match assignment.field.as_str() {
            "adaptive_on" => enabled = parse_bool(&assignment.field, &assignment.value)?,
            "adaptive_max_level" => max_level = parse_usize(&assignment.field, &assignment.value)?,
            "adaptive_base_m" => base_m = parse_f64(&assignment.field, &assignment.value)?,
            "adaptive_coastline" => coastline = parse_bool(&assignment.field, &assignment.value)?,
            other => return Err(invalid(format!("unknown &adaptive field '{other}'"))),
        }
    }
    if !enabled {
        return Ok(None);
    }
    if max_level > 5 {
        return Err(invalid(format!(
            "adaptive_max_level must be in 0..=5, got {max_level}"
        )));
    }
    if base_m < 0.0 || !base_m.is_finite() {
        return Err(invalid(format!(
            "adaptive_base_m must be a non-negative finite length, got {base_m}"
        )));
    }
    Ok(Some(AdaptiveRefineOptions {
        max_level: (max_level > 0).then_some(max_level),
        base_m: (base_m > 0.0).then_some(base_m),
        coastline,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_group_leaves_the_route_off() {
        assert_eq!(read_adaptive_refine_options("&mkgrd\n/\n").unwrap(), None);
    }

    #[test]
    fn an_explicit_false_leaves_the_route_off() {
        assert_eq!(
            read_adaptive_refine_options("&adaptive\n adaptive_on = .false.\n/\n").unwrap(),
            None
        );
    }

    #[test]
    fn a_bare_group_takes_the_defaults() {
        assert_eq!(
            read_adaptive_refine_options("&adaptive\n/\n").unwrap(),
            Some(AdaptiveRefineOptions::default())
        );
    }

    #[test]
    fn every_field_reads_back() {
        let options = read_adaptive_refine_options(
            "&adaptive\n adaptive_on = .true.\n adaptive_max_level = 3\n \
             adaptive_base_m = 400000.0\n adaptive_coastline = .false.\n/\n",
        )
        .unwrap()
        .expect("group present");
        assert_eq!(options.max_level, Some(3));
        assert_eq!(options.base_m, Some(400_000.0));
        assert!(!options.coastline);
    }

    #[test]
    fn zero_means_follow_the_run() {
        let options = read_adaptive_refine_options(
            "&adaptive\n adaptive_max_level = 0\n adaptive_base_m = 0.0\n/\n",
        )
        .unwrap()
        .expect("group present");
        assert_eq!(options.max_level, None);
        assert_eq!(options.base_m, None);
    }

    #[test]
    fn a_bad_field_is_named_rather_than_ignored() {
        let error = read_adaptive_refine_options("&adaptive\n adaptive_depth = 3\n/\n")
            .expect_err("unknown field");
        assert!(error.to_string().contains("adaptive_depth"), "{error}");
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert!(read_adaptive_refine_options("&adaptive\n adaptive_max_level = 6\n/\n").is_err());
        assert!(read_adaptive_refine_options("&adaptive\n adaptive_base_m = -1.0\n/\n").is_err());
        assert!(read_adaptive_refine_options("&adaptive\n adaptive_on = yes\n/\n").is_err());
    }
}
